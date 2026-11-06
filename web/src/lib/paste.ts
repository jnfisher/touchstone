// The Paste Character dialog (frmPasteChar): the wire types of `/api/paste/*`, the prompt loop that
// answers the server's questions one round trip at a time, the preview of what a paste changes,
// and the small text helpers of the party screen. Nothing game-related is computed here.
import type { CharacterDoc } from "./character";

type FetchFn = typeof fetch;

// ---------- wire types (snake_case, as `bin/touchstone/src/paste.rs`) ----------

/** A button (`vbYes` 6, `vbNo` 7, `vbCancel` 2) or the text of an input box ("" is Cancel). */
export type Answer = number | string;

export const VB_YES = 6;
export const VB_NO = 7;
export const VB_CANCEL = 2;

export type PromptKind = "info" | "yes_no" | "yes_no_cancel" | "input";

export interface Prompt {
  kind: PromptKind;
  text: string;
  default: Answer;
  answer: Answer | null;
}

export interface SubCell {
  text: string;
  tag?: number | string | null;
}

/** An Item Manager row: `text` is the item number; cells are Name, Flag, QTY, Source, Enc, Type,
 * Worn, Usable, Value, Shop. */
export interface ItemRow {
  text: string;
  subs: SubCell[];
}

export type PasteEvent =
  | { kind: "message"; text: string }
  | { kind: "save"; prompt: boolean; file: string }
  | { kind: "clear_character" }
  | { kind: "handled_error"; source: string; number: number };

export interface CharacterPasteRequest {
  character: CharacterDoc;
  text: string;
  answers: Answer[];
  rows?: ItemRow[];
  char_loaded?: boolean;
  unequip_missing?: boolean;
  hold?: boolean[];
  only_in_game?: boolean;
  second_wrist?: boolean;
}

export interface CharacterPasteResponse {
  character: CharacterDoc;
  events: PasteEvent[];
  prompts: Prompt[];
  needs_answer: Prompt | null;
  sort_after: boolean;
  rows: ItemRow[];
  sim_ac: number | null;
  sim_dr: number | null;
}

export interface PartyForm {
  name: string[];
  ac: string[];
  dr: string[];
  mr: string[];
  dodge: string[];
  hitpoints: string[];
  rest_hp: string[];
  regen_hp: string[];
  accy: string[];
  heals: string[];
  dmg: string[];
  spell_dmg: string[];
  swings: string[];
  anti_magic: number[];
  atk_last: boolean[];
  atk_last_tag: string[];
  am_total: string;
  party_total: string;
}

/** The text columns of the party table, in MME's left-to-right order. */
export const PARTY_COLUMNS = [
  { key: "ac", label: "AC", help: "Armour class" },
  { key: "dr", label: "DR", help: "Damage resistance" },
  { key: "mr", label: "MR", help: "Magic resistance" },
  { key: "dodge", label: "Dodge", help: "Dodge" },
  { key: "hitpoints", label: "HP", help: "Hit points" },
  {
    key: "regen_hp",
    label: "Regen",
    help: "Non-resting hp regen per 30 seconds; divided by 6 for a per-round rate when you continue",
  },
  { key: "rest_hp", label: "Rest", help: "Hp regen while resting, per 20 seconds" },
  { key: "accy", label: "Accy", help: "Accuracy" },
  {
    key: "heals",
    label: "Heals",
    help: "Sustainable healing per round from spells, without needing to rest",
  },
  {
    key: "dmg",
    label: "Phys",
    help: "Average per-round physical damage, as if against 0/0/0 AC/DR/Dodge",
  },
  {
    key: "spell_dmg",
    label: "Magic",
    help: "Average per-round spell damage, as if against 50 MR",
  },
  { key: "swings", label: "Swings", help: "Average swings per round of each melee character" },
] as const satisfies readonly { key: keyof PartyForm; label: string; help: string }[];

export type PartyColumn = (typeof PARTY_COLUMNS)[number]["key"];

export interface PartyAsk {
  kind: "no_data" | "attack";
  text?: string;
  index?: number;
  member?: number;
  who?: string;
  default?: string;
}

export interface PartyPasteRequest {
  text: string;
  form?: PartyForm;
  attack_answers?: (string | null)[];
  no_data_continue?: boolean | null;
  only_in_game?: boolean;
  second_wrist?: boolean;
}

export interface PartyPasteResponse {
  form: PartyForm;
  status: "canceled" | "no_data_exit" | "done";
  messages: string[];
  prompts: { member: number; who: string; default: string }[];
  needs_answer: PartyAsk | null;
  error: boolean;
}

/** What Continue writes into the monster filter; a field is null when MME leaves it alone. */
export interface PartyFilter {
  party_size: string | null;
  damage: string | null;
  ac: string | null;
  dr: string | null;
  mr: string | null;
  dodge: string | null;
  hitpoints: string | null;
  anti_magic: string | null;
  rest_hp: string | null;
  accy: string | null;
  swings: string | null;
  damage_out_phys: string | null;
  damage_out_spell: string | null;
}

export interface PartyContinueRequest {
  form: PartyForm;
  current_damage: number;
  current_damage_out: [number, number];
  answer?: number;
}

export interface PartyContinueResponse {
  status: "applied" | "canceled" | "needs_answer" | "info";
  filter: PartyFilter | null;
  question: Prompt | null;
  message: string | null;
}

// ---------- client ----------

export class PasteError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

export function createPasteClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
  async function post<T>(path: string, body: unknown): Promise<T> {
    const res = await fetchFn(path, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) {
      const reason = (await res.text().catch(() => "")).trim();
      throw new PasteError(res.status, reason || `HTTP ${res.status}`);
    }
    return (await res.json()) as T;
  }
  return {
    character: (b: CharacterPasteRequest) =>
      post<CharacterPasteResponse>("/api/paste/character", b),
    party: (b: PartyPasteRequest) => post<PartyPasteResponse>("/api/paste/party", b),
    partyContinue: (b: PartyContinueRequest) =>
      post<PartyContinueResponse>("/api/paste/party/continue", b),
  };
}

export type PasteClient = ReturnType<typeof createPasteClient>;
export const pasteClient = createPasteClient();

// ---------- the prompt loop ----------

/** Shows a question and resolves to the reply, or `undefined` when the dialog was closed. */
export type AskFn = (p: Prompt) => Promise<Answer | undefined>;

/** MME asks a handful of questions per paste; this only stops a runaway server. */
const MAX_ROUNDS = 40;

/**
 * Posts the paste, and while the server answers with a question asks it and posts again with the
 * reply appended. Resolves to the final response, or `undefined` if the user closed a question.
 */
export async function runCharacterPaste(
  client: Pick<PasteClient, "character">,
  req: Omit<CharacterPasteRequest, "answers">,
  ask: AskFn,
): Promise<CharacterPasteResponse | undefined> {
  const answers: Answer[] = [];
  for (let round = 0; round < MAX_ROUNDS; round++) {
    const res = await client.character({ ...req, answers });
    if (!res.needs_answer) return res;
    const a = await ask(res.needs_answer);
    if (a === undefined) return undefined;
    answers.push(a);
  }
  throw new PasteError(0, "the server kept asking questions");
}

/** The same loop for the party text: the "no data" question, then each member's attack prompt. */
export async function runPartyPaste(
  client: Pick<PasteClient, "party">,
  req: Omit<PartyPasteRequest, "attack_answers" | "no_data_continue">,
  askYesNo: (text: string) => Promise<boolean | undefined>,
  askAttack: (a: PartyAsk) => Promise<string | null | undefined>,
): Promise<PartyPasteResponse | undefined> {
  const attack: (string | null)[] = [];
  let noData: boolean | null = null;
  for (let round = 0; round < MAX_ROUNDS; round++) {
    const res = await client.party({
      ...req,
      attack_answers: attack,
      no_data_continue: noData,
    });
    const q = res.needs_answer;
    if (!q) return res;
    if (q.kind === "no_data") {
      const a = await askYesNo(q.text ?? "");
      if (a === undefined) return undefined;
      noData = a;
    } else {
      const a = await askAttack(q);
      if (a === undefined) return undefined;
      attack.push(a);
    }
  }
  throw new PasteError(0, "the server kept asking questions");
}

/** `cmdContinue_Click`: posts, and asks its one question if it has it. */
export async function runPartyContinue(
  client: Pick<PasteClient, "partyContinue">,
  req: Omit<PartyContinueRequest, "answer">,
  ask: AskFn,
): Promise<PartyContinueResponse | undefined> {
  let res = await client.partyContinue(req);
  if (res.status === "needs_answer" && res.question) {
    const a = await ask(res.question);
    if (a === undefined || typeof a !== "number") return undefined;
    res = await client.partyContinue({ ...req, answer: a });
  }
  return res;
}

// ---------- the clipboard button (cmdPaste_Click) ----------

/** The text MME's own placeholder shows in an empty paste box counts as empty. */
export function isBlankPaste(text: string): boolean {
  return text.trim() === "";
}

/**
 * What the paste box holds after "Paste from clipboard": `clearFirst` is the answer to "Clear paste
 * area first?" (Yes replaces the box, No puts the clipboard above what is there).
 */
export function pasteIntoBox(box: string, clip: string, clearFirst: boolean): string {
  if (isBlankPaste(box) || clearFirst) return clip;
  return `${clip}\r\n\r\n${box}`;
}

// ---------- party helpers ----------

export function emptyPartyForm(): PartyForm {
  const col = () => Array<string>(7).fill("");
  return {
    name: col(),
    ac: col(),
    dr: col(),
    mr: col(),
    dodge: col(),
    hitpoints: col(),
    rest_hp: col(),
    regen_hp: col(),
    accy: col(),
    heals: col(),
    dmg: col(),
    spell_dmg: col(),
    swings: col(),
    anti_magic: Array<number>(7).fill(0),
    atk_last: Array<boolean>(7).fill(false),
    atk_last_tag: col(),
    am_total: "",
    party_total: "",
  };
}

/** `cmdPasteMegaDmg_Click`: the average out of a MegaMUD damage report; `spell` reads the
 * pre-round line of the spell report instead of the round line of the physical one. */
export function megaDamage(clip: string, spell: boolean): string | undefined {
  const re = spell
    ? /Pre[^\r\n]+[\r\n]+[^\r\n]+Avg:(\d+)[^\r\n]*[\r\n]+Round/
    : /Round:[^\r\n]+Avg:(\d+)/;
  return re.exec(clip)?.[1];
}

/** Members that have a name, 1 to 6. */
export function partyMembers(form: PartyForm): number[] {
  return [1, 2, 3, 4, 5, 6].filter((k) => (form.name[k] ?? "").trim() !== "");
}

// ---------- the preview ----------

export interface Change {
  label: string;
  before: string;
  after: string;
}

export interface Names {
  classes: Map<number, string>;
  races: Map<number, string>;
  items: Map<number, string>;
}

const STATS = ["Strength", "Intellect", "Willpower", "Agility", "Health", "Charm"];
const SLOTS = [
  "Head", "Ears", "Neck", "Back", "Torso", "Arms", "Wrist", "Wrist 2", "Hands", "Finger 1",
  "Finger 2", "Waist", "Legs", "Feet", "Worn", "Off-Hand", "Weapon", "Eyes", "Face", "Everywhere",
]; // prettier-ignore

/** What applying a paste would change on the character, for the preview. */
export function characterChanges(
  before: CharacterDoc,
  after: CharacterDoc,
  names: Names,
): Change[] {
  const out: Change[] = [];
  const add = (label: string, b: string | number | boolean, a: string | number | boolean) => {
    if (String(b) !== String(a)) out.push({ label, before: String(b), after: String(a) });
  };
  const nm = (m: Map<number, string>, n: number | null) =>
    n === null ? "(none)" : n === 0 ? "Any" : (m.get(n) ?? `#${n}`);
  add("Name", before.name || "(none)", after.name || "(none)");
  add("Race", nm(names.races, before.race), nm(names.races, after.race));
  add("Class", nm(names.classes, before.class), nm(names.classes, after.class));
  add("Level", before.level, after.level);
  add("Use character in filters", before.global_filter, after.global_filter);
  STATS.forEach((s, i) => add(s, before.stats[i] ?? 0, after.stats[i] ?? 0));
  const item = (n: number) => (n === 0 ? "(none)" : (names.items.get(n) ?? `#${n}`));
  SLOTS.forEach((s, i) => add(s, item(before.equipped[i] ?? 0), item(after.equipped[i] ?? 0)));
  add("Extra weight", before.add_weight, after.add_weight);
  add("Use extra weight", before.use_add_weight, after.use_add_weight);
  const learned = (d: CharacterDoc) => d.learned_spells.filter((n) => n > 0).length;
  add("Learned spells", learned(before), learned(after));
  return out;
}

/** A row's visible cells for the Item Manager table. */
export function rowCells(r: ItemRow): { number: string; name: string; flag: string; qty: string; source: string; value: string; shop: string } {
  const s = (i: number) => r.subs[i]?.text ?? "";
  return {
    number: r.text,
    name: s(0),
    flag: s(1),
    qty: s(2),
    source: s(3),
    value: s(8),
    shop: s(9),
  };
}
