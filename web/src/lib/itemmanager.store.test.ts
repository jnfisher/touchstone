import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { newCharacter, type CharacterClient, type CharacterDoc } from "./character";
import { createCharacterStore } from "./character.svelte";
import type { ImResponse, ItemManagerClient } from "./itemmanager";
import { createItemManager } from "./itemmanager.svelte";
import { createPasteStore } from "./paste.svelte";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

function setup(reply: (req: Record<string, unknown>) => Partial<ImResponse>) {
  const files = new Map<string, CharacterDoc>();
  const chars = {
    evaluate: vi.fn().mockResolvedValue({}),
    list: vi.fn(async () => []),
    get: vi.fn(async (n: string) => structuredClone(files.get(n)!)),
    put: vi.fn(async (n: string, d: CharacterDoc) => (files.set(n, structuredClone(d)), d)),
    remove: vi.fn(),
    importMmec: vi.fn(),
  };
  const char = createCharacterStore(chars as unknown as CharacterClient);
  char.edit((d) => (d.name = "Zed"));
  const paste = createPasteStore();
  const call = vi.fn(async (req: Record<string, unknown>) => ({
    character: structuredClone(req.character as CharacterDoc),
    rows: req.rows as ImResponse["rows"],
    needs_answer: null,
    selected: req.selected as number[],
    carried_changed: false,
    stash_changed: false,
    messages: [],
    ...reply(req),
  }));
  const client = {
    rows: call,
    action: call,
    remove: call,
    add: call,
    copy: vi.fn(async () => ({ text: "drop a\r\ndrop a" })),
  } as unknown as ItemManagerClient;
  const write = vi.fn(async () => undefined);
  const im = createItemManager(char, { paste, client, writeClipboard: write });
  return { char, paste, im, call, write, chars };
}

const rows = [
  { text: "1", subs: [{ text: "a" }, { text: "" }] },
  { text: "2", subs: [{ text: "b" }, { text: "DROP" }] },
  { text: "3", subs: [{ text: "c" }, { text: "" }] },
];

describe("item manager store", () => {
  it("sends the rows and selection and keeps what comes back", async () => {
    const { im, paste, call } = setup(() => ({
      rows: [{ text: "1", subs: [{ text: "a" }, { text: "DROP" }] }],
      selected: [0],
    }));
    paste.setRows(rows);
    im.select([0, 2]);
    await im.action(9, async () => undefined);
    expect(call.mock.calls[0][0]).toMatchObject({ selected: [0, 2], action: 9, answers: [] });
    expect(paste.rows).toHaveLength(1);
    expect(im.selected).toEqual([0]);
  });

  it("writes the saved lists and weight into the character, which autosaves", async () => {
    const { im, paste, char, chars } = setup((req) => ({
      character: {
        ...(req.character as CharacterDoc),
        carried: [{ item: 5, qty: 2 }],
        add_weight: 75,
      },
    }));
    paste.setRows(rows);
    im.select([0]);
    await im.action(12, async () => undefined);
    expect(char.doc.carried).toEqual([{ item: 5, qty: 2 }]);
    expect(char.doc.add_weight).toBe(75);
    await vi.advanceTimersByTimeAsync(1000);
    expect(chars.put).toHaveBeenCalled();
  });

  it("does not touch the character when nothing saved changed", async () => {
    const { im, paste, char, chars } = setup(() => ({}));
    paste.setRows(rows);
    im.select([1]);
    await char.flush();
    chars.put.mockClear();
    await im.action(9, async () => undefined);
    await vi.advanceTimersByTimeAsync(1000);
    expect(chars.put).not.toHaveBeenCalled();
  });

  it("does nothing without a selection", async () => {
    const { im, call } = setup(() => ({}));
    await im.action(9, async () => undefined);
    await im.remove(async () => undefined);
    expect(call).not.toHaveBeenCalled();
  });

  it("inverts and selects all locally", () => {
    const { im, paste } = setup(() => ({}));
    paste.setRows(rows);
    im.select([1]);
    im.invert();
    expect(im.selected).toEqual([0, 2]);
    im.selectAll();
    expect(im.selected).toEqual([0, 1, 2]);
  });

  it("copies the commands and says how many", async () => {
    const { im, paste, write } = setup(() => ({}));
    paste.setRows(rows);
    im.select([1]);
    await im.copy();
    expect(write).toHaveBeenCalledWith("drop a\r\ndrop a");
    expect(im.note).toBe("Copied 2 commands to the clipboard.");
  });

  it("reports a server error and recovers", async () => {
    const { im, paste, call } = setup(() => ({}));
    call.mockRejectedValueOnce(new Error("boom"));
    paste.setRows(rows);
    im.select([0]);
    await im.remove(async () => undefined);
    expect(im.problem).toBe("boom");
    expect(im.busy).toBe(false);
  });

  it("counts a different character as a new load, not a paste", async () => {
    const { char } = setup(() => ({}));
    const before = char.loadEpoch;
    await char.applyPasted(newCharacter("Zed"));
    expect(char.loadEpoch).toBe(before);
    char.startNew();
    expect(char.loadEpoch).toBe(before + 1);
  });
});
