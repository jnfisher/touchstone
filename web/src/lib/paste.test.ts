import { describe, expect, it, vi } from "vitest";
import { newCharacter } from "./character";
import {
  characterChanges,
  createPasteClient,
  emptyPartyForm,
  megaDamage,
  partyMembers,
  pasteIntoBox,
  PasteError,
  rowCells,
  runCharacterPaste,
  runPartyContinue,
  runPartyPaste,
  VB_YES,
  type CharacterPasteResponse,
  type PartyPasteResponse,
  type Prompt,
} from "./paste";

const q = (text: string): Prompt => ({
  kind: "yes_no_cancel",
  text,
  default: 2,
  answer: null,
});

const done = (over: Partial<CharacterPasteResponse> = {}): CharacterPasteResponse => ({
  character: newCharacter("Zed"),
  events: [],
  prompts: [],
  needs_answer: null,
  sort_after: false,
  rows: [],
  sim_ac: null,
  sim_dr: null,
  ...over,
});

describe("character prompt loop", () => {
  it("posts again with each answer appended until nothing is asked", async () => {
    const seen: unknown[][] = [];
    const client = {
      character: vi.fn(async (b: { answers: unknown[] }) => {
        seen.push([...b.answers]);
        if (b.answers.length === 0) return done({ needs_answer: q("one?") });
        if (b.answers.length === 1) return done({ needs_answer: q("two?") });
        return done();
      }),
    };
    const asked: string[] = [];
    const res = await runCharacterPaste(
      client,
      { character: newCharacter(), text: "x" },
      async (p) => {
        asked.push(p.text);
        return asked.length === 1 ? VB_YES : 7;
      },
    );
    expect(res?.character.name).toBe("Zed");
    expect(asked).toEqual(["one?", "two?"]);
    expect(seen).toEqual([[], [6], [6, 7]]);
  });

  it("stops when the question is dismissed", async () => {
    const client = { character: vi.fn(async () => done({ needs_answer: q("?") })) };
    const res = await runCharacterPaste(
      client,
      { character: newCharacter(), text: "x" },
      async () => undefined,
    );
    expect(res).toBeUndefined();
    expect(client.character).toHaveBeenCalledTimes(1);
  });

  it("gives up on a server that never stops asking", async () => {
    const client = { character: vi.fn(async () => done({ needs_answer: q("?") })) };
    await expect(
      runCharacterPaste(client, { character: newCharacter(), text: "x" }, async () => 7),
    ).rejects.toBeInstanceOf(PasteError);
  });
});

const partyRes = (over: Partial<PartyPasteResponse> = {}): PartyPasteResponse => ({
  form: emptyPartyForm(),
  status: "done",
  messages: [],
  prompts: [],
  needs_answer: null,
  error: false,
  ...over,
});

describe("party prompt loop", () => {
  it("answers the no-data question, then each attack prompt in order", async () => {
    const calls: { a: unknown; n: unknown }[] = [];
    const client = {
      party: vi.fn(async (b: { attack_answers?: unknown[]; no_data_continue?: boolean | null }) => {
        calls.push({ a: [...(b.attack_answers ?? [])], n: b.no_data_continue });
        if (b.no_data_continue === null) return partyRes({ needs_answer: { kind: "no_data", text: "go on?" } });
        const n = b.attack_answers?.length ?? 0;
        if (n < 2)
          return partyRes({ needs_answer: { kind: "attack", index: n, who: `m${n}`, default: "bash" } });
        return partyRes();
      }),
    };
    const res = await runPartyPaste(
      client,
      { text: "party" },
      async () => true,
      async (a) => (a.index === 0 ? "strk" : null),
    );
    expect(res?.status).toBe("done");
    expect(calls).toEqual([
      { a: [], n: null },
      { a: [], n: true },
      { a: ["strk"], n: true },
      { a: ["strk", null], n: true },
    ]);
  });

  it("continue asks its question once and re-posts with the answer", async () => {
    const client = {
      partyContinue: vi.fn(async (b: { answer?: number }) =>
        b.answer === undefined
          ? { status: "needs_answer" as const, filter: null, question: q("update?"), message: null }
          : { status: "applied" as const, filter: null, question: null, message: null },
      ),
    };
    const res = await runPartyContinue(
      client,
      { form: emptyPartyForm(), current_damage: 5, current_damage_out: [0, 0] },
      async () => 7,
    );
    expect(res?.status).toBe("applied");
    expect(client.partyContinue).toHaveBeenLastCalledWith(expect.objectContaining({ answer: 7 }));
  });
});

describe("client", () => {
  it("posts JSON and turns a failure into a PasteError with the server's reason", async () => {
    const fetchFn = vi.fn(async () => new Response("bad paste", { status: 400 }));
    const c = createPasteClient(fetchFn as unknown as typeof fetch);
    await expect(c.character({ character: newCharacter(), text: "x", answers: [] })).rejects.toMatchObject({
      status: 400,
      message: "bad paste",
    });
    const init = (fetchFn.mock.calls[0] as unknown as [string, RequestInit])[1];
    expect(init.method).toBe("POST");
  });
});

describe("text helpers", () => {
  it("puts the clipboard above existing text unless the box is cleared", () => {
    expect(pasteIntoBox("", "clip", false)).toBe("clip");
    expect(pasteIntoBox("old", "clip", true)).toBe("clip");
    expect(pasteIntoBox("old", "clip", false)).toBe("clip\r\n\r\nold");
  });

  it("reads MegaMUD damage averages", () => {
    expect(megaDamage("Round: 3 hits Avg:142 more", false)).toBe("142");
    expect(megaDamage("nothing", false)).toBeUndefined();
    expect(megaDamage("Pre-cast x\r\nstuff Avg:77 tail\r\nRound 1", true)).toBe("77");
  });

  it("lists members that have a name", () => {
    const f = emptyPartyForm();
    f.name[1] = "A";
    f.name[3] = " ";
    f.name[4] = "B";
    expect(partyMembers(f)).toEqual([1, 4]);
  });

  it("reads the visible cells of an Item Manager row", () => {
    const subs = Array.from({ length: 10 }, (_, i) => ({ text: `c${i}` }));
    expect(rowCells({ text: "5", subs })).toMatchObject({ number: "5", name: "c0", qty: "c2", value: "c8", shop: "c9" });
  });
});

describe("preview of changes", () => {
  const names = {
    classes: new Map([[2, "Mage"]]),
    races: new Map([[1, "Human"]]),
    items: new Map([[9, "sword"]]),
  };
  it("lists only what differs, with names", () => {
    const before = newCharacter("Old");
    const after = { ...newCharacter("Zed"), class: 2, race: 1, level: 14 };
    after.equipped = [...after.equipped];
    after.equipped[16] = 9;
    after.stats = [10, 0, 0, 0, 0, 0];
    const got = characterChanges(before, after, names);
    expect(got).toEqual(
      expect.arrayContaining([
        { label: "Name", before: "Old", after: "Zed" },
        { label: "Class", before: "(none)", after: "Mage" },
        { label: "Race", before: "(none)", after: "Human" },
        { label: "Level", before: "1", after: "14" },
        { label: "Strength", before: "0", after: "10" },
        { label: "Weapon", before: "(none)", after: "sword" },
      ]),
    );
    expect(got.find((c) => c.label === "Charm")).toBeUndefined();
  });

  it("is empty for the same character", () => {
    const c = newCharacter("A");
    expect(characterChanges(c, { ...c }, names)).toEqual([]);
  });
});
