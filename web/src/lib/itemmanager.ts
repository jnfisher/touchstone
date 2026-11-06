// The Item Manager (MME's lvItemManager): wire types of `/api/item-manager/*`, the grid rows and
// columns, the Action buttons, and the question loop. The rules (what each button does to a row,
// what a delete or copy produces, which rows are saved) all run on the server.
import type { ColumnSpec } from "./explorers";
import type { CharacterDoc } from "./character";
import type { Answer, ItemRow, Prompt } from "./paste";

type FetchFn = typeof fetch;

export interface ImRequest {
  character: CharacterDoc;
  /** The list as the page holds it; omitted, the server rebuilds it from the saved lists. */
  rows?: ItemRow[];
  /** Selected row positions (0-based). */
  selected?: number[];
  action?: number;
  answers?: Answer[];
  item?: number;
}

export interface ImResponse {
  character: CharacterDoc;
  rows: ItemRow[];
  needs_answer: Prompt | null;
  selected: number[];
  carried_changed: boolean;
  stash_changed: boolean;
  messages: string[];
}

export class ItemManagerError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

export function createItemManagerClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
  async function post<T>(path: string, body: unknown): Promise<T> {
    const res = await fetchFn(path, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) {
      const reason = (await res.text().catch(() => "")).trim();
      throw new ItemManagerError(res.status, reason || `HTTP ${res.status}`);
    }
    return (await res.json()) as T;
  }
  return {
    rows: (b: ImRequest) => post<ImResponse>("/api/item-manager/rows", b),
    action: (b: ImRequest) => post<ImResponse>("/api/item-manager/action", b),
    remove: (b: ImRequest) => post<ImResponse>("/api/item-manager/delete", b),
    add: (b: ImRequest) => post<ImResponse>("/api/item-manager/add", b),
    copy: (b: ImRequest) => post<{ text: string | null }>("/api/item-manager/copy", b),
  };
}

export type ItemManagerClient = ReturnType<typeof createItemManagerClient>;
export const itemManagerClient = createItemManagerClient();

/** Shows a question and resolves to the reply, or `undefined` when it was dismissed. */
export type AskFn = (p: Prompt) => Promise<Answer | undefined>;

const MAX_ROUNDS = 40;

/** Posts the request, and while the server asks a question asks it and posts again with the reply. */
export async function runWithQuestions(
  call: (answers: Answer[]) => Promise<ImResponse>,
  ask: AskFn,
): Promise<ImResponse | undefined> {
  const answers: Answer[] = [];
  for (let round = 0; round < MAX_ROUNDS; round++) {
    const res = await call(answers);
    if (!res.needs_answer) return res;
    const a = await ask(res.needs_answer);
    if (a === undefined) return undefined;
    answers.push(a);
  }
  throw new ItemManagerError(0, "the server kept asking questions");
}

// ---------- buttons ----------

export interface ActionButton {
  /** `cmdCompareNav` index MME's LV_SetActionCell switches on. */
  action: number;
  label: string;
  title: string;
}

export const ACTION_BUTTONS: ActionButton[] = [
  { action: 9, label: "Drop", title: "Flag DROP, then HIDE, then clear" },
  { action: 10, label: "Pickup", title: "Flag PICKUP, then USE, then clear" },
  { action: 11, label: "Sell", title: "Flag SELL, then BUY, then clear" },
  { action: 12, label: "Carry", title: "Mark as carried: its abilities and weight count for the character" },
  { action: 17, label: "Stash", title: "Flag STASH, then clear" },
  { action: 15, label: "−", title: "One less of the flagged quantity" },
  { action: 16, label: "+", title: "One more of the flagged quantity" },
  { action: 18, label: "Clear", title: "Clear the flag" },
];

export const HELP_TEXT =
  "Use the paste button to paste items from your inventory and what you see on the ground. " +
  "The drop/pickup/sell/stash buttons flag items for sorting and copying actions to your clipboard. " +
  "The carry button marks an item as carried in your inventory so its abilities and weight count for your character. " +
  "Items flagged CARRIED or STASH are the only items from this list that are saved with the character (limit 50 each). " +
  "The copy button copies the game commands for the selected rows, based on how they are flagged.";

// ---------- grid rows ----------

/** One Item Manager row as the grid shows it (the eleven visible columns of MME's ListView). */
export interface ImRow {
  /** Position in the list: the row's id and what the server calls its index. */
  idx: number;
  number: number;
  name: string;
  flag: string;
  qty: number;
  source: string;
  enc: number;
  type: string;
  worn: string;
  usable: string;
  value: string;
  /** The Value column's sort tag (copper), as MME sorts by it. */
  valueSort: number;
  shop: string;
}

const num = (s: string | undefined) => {
  const n = Number.parseFloat(s ?? "");
  return Number.isFinite(n) ? n : 0;
};

export function toGridRows(rows: ItemRow[]): ImRow[] {
  return rows.map((r, idx) => {
    const t = (i: number) => r.subs[i]?.text ?? "";
    return {
      idx,
      number: num(r.text),
      name: t(0),
      flag: t(1),
      qty: num(t(2)),
      source: t(3),
      enc: num(t(4)),
      type: t(5),
      worn: t(6),
      usable: t(7),
      value: t(8),
      valueSort: num(String(r.subs[8]?.tag ?? "0")),
      shop: t(9),
    };
  });
}

/** MME's column order and headers; Num, QTY, Enc and Value sort numerically (lvItemManager_ColumnClick). */
export const IM_COLUMNS: ColumnSpec<ImRow>[] = [
  { id: "number", header: "Num", numeric: true, get: (r) => r.number },
  { id: "name", header: "Name", get: (r) => r.name },
  { id: "flag", header: "Action", get: (r) => r.flag },
  { id: "qty", header: "QTY", numeric: true, get: (r) => r.qty },
  { id: "source", header: "Source", get: (r) => r.source },
  { id: "enc", header: "Enc", numeric: true, get: (r) => r.enc },
  { id: "type", header: "Type", get: (r) => r.type },
  { id: "worn", header: "Worn", get: (r) => r.worn },
  { id: "usable", header: "Usable", get: (r) => r.usable },
  { id: "value", header: "Value", get: (r) => r.value, sortKey: (r) => r.valueSort },
  { id: "shop", header: "Shop", get: (r) => r.shop },
];

/** `LV_InvertSelection`: every row not selected becomes selected, and the other way round. */
export function invertSelection(selected: number[], count: number): number[] {
  const on = new Set(selected);
  const out: number[] = [];
  for (let i = 0; i < count; i++) if (!on.has(i)) out.push(i);
  return out;
}

/** Grid row ids are the row positions as text. */
export const rowId = (r: ImRow) => String(r.idx);
export const idsToSelection = (ids: string[]) =>
  ids.map((s) => Number.parseInt(s, 10)).filter((n) => Number.isInteger(n) && n >= 0).sort((a, b) => a - b);
export const selectionToIds = (sel: number[]) => sel.map(String);

/** Muted rows are items the character cannot use, as MME greys the Usable column. */
export function rowState(r: ImRow): "muted" | undefined {
  return r.usable === "No" ? "muted" : undefined;
}

/** True when the flag marks the row as carried, which changes the character's stats. */
export const isCarried = (flag: string) => /carried/i.test(flag);
