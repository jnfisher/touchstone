// Settings the server keeps (`GET/PUT /api/settings`, bin/touchstone/src/settings.rs) and the
// validation the dialog shows while typing. The server applies MME's save rules and answers with
// what it stored; the helpers here only predict that so the page can say "will be saved as 10".
// The text boxes stay text, as in MME, which reads them with Val().

type FetchFn = typeof fetch;

export interface ExpHrBlock {
  model_a: boolean;
  model_b: boolean;
  model_c: boolean;
  model_d: boolean;
  recovery_only: boolean;
  show_all: boolean;
  knob_xp: number;
  knob_dmg: number;
  knob_mana: number;
  knob_move: number;
}

export interface Settings {
  exp_per_hour: { global: ExpHrBlock; by_character: boolean };
  monster_sim_rounds: number;
  route_waypoint_interval: number;
  auto_load_last_char: boolean;
  /** `HideRecordNumbers` (`bHideRecordNumbers`): names print without their "(n)". */
  hide_record_numbers: boolean;
}

export const KNOB_MIN = 0.01;
export const KNOB_MAX = 10;
export const SIM_ROUNDS_MIN = 100;
export const SIM_ROUNDS_MAX = 10000;
export const WAYPOINT_DEFAULT = 25;
export const WAYPOINT_MAX = 1000;

export const MODELS = ["model_a", "model_b", "model_c", "model_d"] as const;
export type ModelKey = (typeof MODELS)[number];
export const KNOBS = ["knob_xp", "knob_dmg", "knob_mana", "knob_move"] as const;
export type KnobKey = (typeof KNOBS)[number];

/** What MME runs with when nothing is saved. */
export function defaultBlock(): ExpHrBlock {
  return {
    model_a: true,
    model_b: true,
    model_c: true,
    model_d: true,
    recovery_only: false,
    show_all: false,
    knob_xp: 1,
    knob_dmg: 1,
    knob_mana: 1,
    knob_move: 1,
  };
}

export function defaultSettings(): Settings {
  return {
    exp_per_hour: { global: defaultBlock(), by_character: false },
    monster_sim_rounds: 500,
    route_waypoint_interval: WAYPOINT_DEFAULT,
    auto_load_last_char: true,
    hide_record_numbers: false,
  };
}

/** VB6 `Val`: the number at the start of the text (spaces skipped), 0 when there is none. */
export function vbVal(text: string): number {
  const m = /^\s*[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?/.exec(text);
  if (!m) return 0;
  const n = Number(m[0].replace(/\s+/g, ""));
  return Number.isFinite(n) ? n : 0;
}

/** `Round(x, 2)` with VB's round-half-even on the decimal digits. */
export function round2(x: number): number {
  const scaled = Math.round(x * 1e6) / 1e4; // x * 100 with float noise trimmed
  const f = Math.floor(scaled);
  const diff = scaled - f;
  const r = Math.abs(diff - 0.5) < 1e-9 ? (f % 2 === 0 ? f : f + 1) : Math.round(scaled);
  return r / 100;
}

/** `LoadExpPerHourKnobs`/`cmdSave_Click`: rounded to two places, between 0.01 and 10. */
export function clampKnob(x: number): number {
  return Math.min(KNOB_MAX, Math.max(KNOB_MIN, round2(x)));
}

export function clampSimRounds(n: number): number {
  return Math.min(SIM_ROUNDS_MAX, Math.max(SIM_ROUNDS_MIN, Math.trunc(n)));
}

/** Blank or below 1 means the default 25; above 1000 is 1000. */
export function clampWaypoint(n: number): number {
  const i = Math.trunc(n);
  return i < 1 ? WAYPOINT_DEFAULT : Math.min(WAYPOINT_MAX, i);
}

/** Digits (and one point for knobs) only: `NumberKeysOnly`. */
export function filterNumberText(text: string, decimal: boolean): string {
  let seenPoint = false;
  let out = "";
  for (const c of text) {
    if (c >= "0" && c <= "9") out += c;
    else if (decimal && c === "." && !seenPoint) {
      seenPoint = true;
      out += c;
    }
  }
  return out;
}

export interface Predicted {
  value: number;
  /** Set when the value saved differs from what was typed. */
  note?: string;
}

const fmt = (n: number) => String(n);

export function predictKnob(text: string): Predicted {
  const typed = vbVal(text);
  const value = clampKnob(typed);
  if (text.trim() === "") return { value, note: `blank counts as 0, saved as ${fmt(value)}` };
  if (value !== typed) {
    return {
      value,
      note:
        typed < KNOB_MIN || typed > KNOB_MAX
          ? `kept between ${KNOB_MIN} and ${KNOB_MAX}, saved as ${fmt(value)}`
          : `rounded to two places, saved as ${fmt(value)}`,
    };
  }
  return { value };
}

export function predictSimRounds(text: string): Predicted {
  const typed = vbVal(text);
  const value = clampSimRounds(typed);
  if (value !== typed) {
    return { value, note: `kept between ${SIM_ROUNDS_MIN} and ${SIM_ROUNDS_MAX}, saved as ${value}` };
  }
  return { value };
}

export function predictWaypoint(text: string): Predicted {
  const typed = vbVal(text);
  const value = clampWaypoint(typed);
  if (value !== Math.trunc(typed)) {
    return {
      value,
      note:
        typed < 1
          ? `blank or 0 means the default, saved as ${value}`
          : `at most ${WAYPOINT_MAX}, saved as ${value}`,
    };
  }
  return { value };
}

/** The save rule: with no model ticked, A, B and C are turned on. */
export function normalizeModels(b: ExpHrBlock): ExpHrBlock {
  if (MODELS.some((m) => b[m])) return b;
  return { ...b, model_a: true, model_b: true, model_c: true };
}

export function noModelTicked(b: ExpHrBlock): boolean {
  return !MODELS.some((m) => b[m]);
}

// ---------- the dialog's draft ----------

/** The form as typed: numbers are text until saved. */
export interface Draft {
  block: ExpHrBlock;
  knobs: Record<KnobKey, string>;
  byCharacter: boolean;
  simRounds: string;
  waypoint: string;
  autoLoad: boolean;
  hideNumbers: boolean;
}

export function toDraft(s: Settings, block: ExpHrBlock = s.exp_per_hour.global): Draft {
  return {
    block: { ...block },
    knobs: {
      knob_xp: fmt(block.knob_xp),
      knob_dmg: fmt(block.knob_dmg),
      knob_mana: fmt(block.knob_mana),
      knob_move: fmt(block.knob_move),
    },
    byCharacter: s.exp_per_hour.by_character,
    simRounds: fmt(s.monster_sim_rounds),
    waypoint: fmt(s.route_waypoint_interval),
    autoLoad: s.auto_load_last_char,
    hideNumbers: s.hide_record_numbers,
  };
}

/** The exp/hr block a draft will save (the server repeats these clamps). */
export function draftBlock(d: Draft): ExpHrBlock {
  const b: ExpHrBlock = { ...d.block };
  for (const k of KNOBS) b[k] = clampKnob(vbVal(d.knobs[k]));
  return normalizeModels(b);
}

/** `current` supplies the global block when the draft's block goes to the character instead. */
export function draftSettings(d: Draft, globalBlock: ExpHrBlock): Settings {
  return {
    exp_per_hour: { global: globalBlock, by_character: d.byCharacter },
    monster_sim_rounds: clampSimRounds(vbVal(d.simRounds)),
    route_waypoint_interval: clampWaypoint(vbVal(d.waypoint)),
    auto_load_last_char: d.autoLoad,
    hide_record_numbers: d.hideNumbers,
  };
}

/** "Reset" in MME: knobs 1, all four models on, both switches off. */
export function resetExpFields(d: Draft): void {
  d.block = defaultBlock();
  d.knobs = { knob_xp: "1", knob_dmg: "1", knob_mana: "1", knob_move: "1" };
}

// ---------- client ----------

export class SettingsError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

export function createSettingsClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
  async function call<T>(path: string, init?: RequestInit): Promise<T> {
    const res = await fetchFn(path, init);
    if (!res.ok) {
      const reason = (await res.text().catch(() => "")).trim();
      throw new SettingsError(res.status, reason || `HTTP ${res.status}`);
    }
    return (await res.json()) as T;
  }
  return {
    get: () => call<Settings>("/api/settings"),
    /** The settings plus the exp/hr block `character` calculates with. */
    getFor: (character: string) =>
      call<Settings & { effective: ExpHrBlock }>(
        `/api/settings?character=${encodeURIComponent(character)}`,
      ),
    put: (s: Settings) =>
      call<Settings>("/api/settings", {
        method: "PUT",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(s),
      }),
  };
}

export type SettingsClient = ReturnType<typeof createSettingsClient>;
