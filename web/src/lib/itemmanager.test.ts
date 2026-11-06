import { describe, expect, it, vi } from "vitest";
import { newCharacter } from "./character";
import {
  ACTION_BUTTONS,
  IM_COLUMNS,
  ItemManagerError,
  createItemManagerClient,
  idsToSelection,
  invertSelection,
  isCarried,
  rowState,
  runWithQuestions,
  selectionToIds,
  toGridRows,
  type ImResponse,
} from "./itemmanager";
import type { ItemRow, Prompt } from "./paste";

const cell = (text: string, tag?: number | string) => ({ text, tag });
const row = (n: string, flag = "CARRIED x2"): ItemRow => ({
  text: n,
  subs: [
    cell("sword"),
    cell(flag),
    cell("2"),
    cell("Inventory"),
    cell("40"),
    cell("Weapon"),
    cell("One-Handed", 2),
    cell("No"),
    cell("1 gold", "10000"),
    cell("Shop"),
  ],
});

const q: Prompt = { kind: "yes_no", text: "sure?", default: 7, answer: null };
const reply = (over: Partial<ImResponse> = {}): ImResponse => ({
  character: newCharacter("Zed"),
  rows: [],
  needs_answer: null,
  selected: [],
  carried_changed: false,
  stash_changed: false,
  messages: [],
  ...over,
});

describe("grid rows", () => {
  it("reads MME's eleven columns and the value tag", () => {
    const [r] = toGridRows([row("325")]);
    expect(r).toMatchObject({
      idx: 0,
      number: 325,
      name: "sword",
      flag: "CARRIED x2",
      qty: 2,
      source: "Inventory",
      enc: 40,
      type: "Weapon",
      worn: "One-Handed",
      usable: "No",
      value: "1 gold",
      valueSort: 10000,
      shop: "Shop",
    });
    expect(IM_COLUMNS.map((c) => c.header)).toEqual([
      "Num", "Name", "Action", "QTY", "Source", "Enc", "Type", "Worn", "Usable", "Value", "Shop",
    ]); // prettier-ignore
  });

  it("copes with short rows", () => {
    const [r] = toGridRows([{ text: "7", subs: [cell("x")] }]);
    expect(r).toMatchObject({ number: 7, name: "x", flag: "", qty: 0, valueSort: 0 });
  });

  it("sorts the Value column by its tag, not its text", () => {
    const [a, b] = toGridRows([row("1"), { ...row("2"), subs: row("2").subs.map((s, i) => (i === 8 ? cell("9 silver", "900") : s)) }]);
    const value = IM_COLUMNS.find((c) => c.id === "value")!;
    expect(value.sortKey!(a)).toBeGreaterThan(value.sortKey!(b));
  });

  it("greys rows the character cannot use", () => {
    const [r] = toGridRows([row("1")]);
    expect(rowState(r)).toBe("muted");
    expect(rowState({ ...r, usable: "Yes" })).toBeUndefined();
  });
});

describe("selection", () => {
  it("inverts like LV_InvertSelection", () => {
    expect(invertSelection([1, 3], 5)).toEqual([0, 2, 4]);
    expect(invertSelection([], 3)).toEqual([0, 1, 2]);
    expect(invertSelection([0, 1], 2)).toEqual([]);
    expect(invertSelection([], 0)).toEqual([]);
  });

  it("round-trips grid ids", () => {
    expect(idsToSelection(["10", "2", "x", "-1"])).toEqual([2, 10]);
    expect(selectionToIds([3, 4])).toEqual(["3", "4"]);
  });

  it("knows a carried flag", () => {
    expect(isCarried("CARRIED x3")).toBe(true);
    expect(isCarried("carried")).toBe(true);
    expect(isCarried("STASH")).toBe(false);
  });
});

describe("buttons", () => {
  it("use the action indexes of cmdCompareNav", () => {
    // frmMain cmdCompareNav_Click: 9 drop, 10 pickup, 11 sell, 12 carry, 15 -, 16 +, 17 stash, 18 clear
    expect(ACTION_BUTTONS.map((b) => b.action).sort((a, b) => a - b)).toEqual([9, 10, 11, 12, 15, 16, 17, 18]);
  });
});

describe("question loop", () => {
  it("posts again with each answer appended", async () => {
    const seen: unknown[][] = [];
    const call = vi.fn(async (answers: unknown[]) => {
      seen.push([...answers]);
      return answers.length === 0 ? reply({ needs_answer: q }) : reply();
    });
    const ask = vi.fn(async () => 6);
    const out = await runWithQuestions(call, ask);
    expect(out?.needs_answer).toBeNull();
    expect(seen).toEqual([[], [6]]);
    expect(ask).toHaveBeenCalledWith(q);
  });

  it("stops when the question is dismissed", async () => {
    const out = await runWithQuestions(async () => reply({ needs_answer: q }), async () => undefined);
    expect(out).toBeUndefined();
  });

  it("gives up on a server that never stops asking", async () => {
    await expect(runWithQuestions(async () => reply({ needs_answer: q }), async () => 6)).rejects.toBeInstanceOf(ItemManagerError);
  });
});

describe("client", () => {
  it("posts json and reports the server's reason", async () => {
    const fetchFn = vi.fn(async (_u: string, init?: RequestInit) =>
      init?.body && JSON.parse(String(init.body)).action === 5
        ? new Response("5 is not an Action button", { status: 400 })
        : Response.json(reply()),
    );
    const c = createItemManagerClient(fetchFn as unknown as typeof fetch);
    await c.action({ character: newCharacter("Zed"), action: 12 });
    expect(fetchFn.mock.calls[0][0]).toBe("/api/item-manager/action");
    await expect(c.action({ character: newCharacter("Zed"), action: 5 })).rejects.toMatchObject({
      status: 400,
      message: "5 is not an Action button",
    });
  });
});
