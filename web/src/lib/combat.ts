// The combat setup dialog (frmPopUpOptions): wire types of `/api/character/combat-*`, the form the
// dialog edits, and the mapping between that form and the character's attack. Saving rules and
// every number come from the server (`bin/touchstone/src/combat_setup.rs`);
// this file only arranges controls.
import type { AttackDoc, CharacterDoc } from "./character";

type FetchFn = typeof fetch;

export interface Choice {
  number: number;
  label: string;
}

export interface CombatOptions {
  nmr_ver: number;
  recovery_only: boolean;
  current_weapon: string;
  attack_learned: Choice[];
  attack_any: Choice[];
  heal_learned: Choice[];
  heal_any: Choice[];
  backstab_weapons: Choice[];
}

export interface CombatPreview {
  error_text: string | null;
  attack: {
    caption: string;
    oom_caption: string;
    damage: number;
    spell_value: number;
    backstab_damage: number;
    oom: number;
    hit_chance: number | null;
    swings: number | null;
    cast_level: number | null;
    avg_cast: number | null;
    cast_chance: number | null;
    mana_cost: number | null;
  };
  heal: {
    caption: string;
    mana_caption: string;
    oom_caption: string;
    value: number;
    cost: number;
    oom: number;
    rounds: number;
  };
  /** What Save stores. */
  applied: AttackDoc;
  attack_name: string;
  attack_name_long: string;
}

/** Radio buttons: 0 one-shot, 1 weapon, 2 learned spell, 3 any spell, 4 martial art, 5 manual. */
export const ATTACK_RADIOS = [
  "One-shot",
  "Weapon",
  "Learned spell",
  "Any spell",
  "Martial arts",
  "Manual",
] as const;
export const HEAL_RADIOS = [
  "Infinite",
  "None (passive)",
  "Learned spell",
  "Any spell",
  "Manual",
] as const;
export const MARTIAL_ARTS = ["(none)", "Punch", "Kick", "Jumpkick"] as const;

/** The dialog's controls. `base` is the saved attack, for what the controls do not overwrite. */
export interface CombatForm {
  base: AttackDoc;
  radio: number;
  bash: boolean;
  smash: boolean;
  learnedSpell: number;
  anySpell: number;
  spellLvl: number;
  martialArt: number;
  manualP: number;
  manualM: number;
  backstab: boolean;
  /** 0 equipped weapon, -1 fists, else an item. */
  backstabWeapon: number;
  meditate: boolean;
  healRadio: number;
  healLearned: number;
  healAny: number;
  healLvl: number;
  healRounds: number;
  healManual: number;
}

const inList = (list: Choice[], n: number) => list.some((c) => c.number === n);

/** `SetupChooseAttack`: the controls start from the saved attack. */
export function formFromAttack(
  a: AttackDoc,
  opts: CombatOptions,
  filterLevel: number | undefined,
): CombatForm {
  const t = a.attack_type;
  return {
    base: { ...a },
    radio: t === 6 || t === 7 ? 1 : t >= 0 && t <= 5 ? t : 0,
    bash: t === 6,
    smash: t === 7,
    learnedSpell: inList(opts.attack_learned, a.spell_num) ? a.spell_num : 0,
    anySpell: inList(opts.attack_any, a.spell_num) ? a.spell_num : 0,
    spellLvl: a.spell_lvl > 0 ? a.spell_lvl : (filterLevel ?? 0),
    martialArt: a.martial_art > 0 && a.martial_art <= 3 ? a.martial_art : 0,
    manualP: a.manual_p,
    manualM: a.manual_m,
    backstab: a.backstab,
    backstabWeapon:
      a.backstab_weapon < 0 || inList(opts.backstab_weapons, a.backstab_weapon)
        ? a.backstab_weapon
        : 0,
    meditate: a.use_meditate,
    healRadio: a.heal_type,
    healLearned: inList(opts.heal_learned, a.heal_spell_num)
      ? a.heal_spell_num
      : 0,
    healAny: inList(opts.heal_any, a.heal_spell_num) ? a.heal_spell_num : 0,
    healLvl: a.heal_spell_lvl > 0 ? a.heal_spell_lvl : (filterLevel ?? 0),
    healRounds: a.heal_rounds,
    healManual: a.heal_manual,
  };
}

/** The attack the controls describe; the server decides what saving makes of it. */
export function buildDraft(f: CombatForm): AttackDoc {
  const spell =
    f.radio === 2 ? f.learnedSpell : f.radio === 3 ? f.anySpell : f.base.spell_num;
  const heal =
    f.healRadio === 2
      ? f.healLearned
      : f.healRadio === 3
        ? f.healAny
        : f.base.heal_spell_num;
  return {
    attack_type: f.radio === 1 ? (f.bash ? 6 : f.smash ? 7 : 1) : f.radio,
    backstab: f.backstab,
    backstab_weapon: f.backstabWeapon,
    martial_art: f.martialArt,
    spell_num: spell,
    spell_lvl: f.spellLvl,
    manual_p: f.manualP,
    manual_m: f.manualM,
    use_meditate: f.meditate,
    heal_type: f.healRadio,
    heal_spell_num: heal,
    heal_spell_lvl: f.healLvl,
    heal_rounds: f.healRounds,
    heal_manual: f.healManual,
  };
}

/** `chkBashing` and `chkSmashing` are exclusive. */
export function setBash(f: CombatForm, on: boolean) {
  f.bash = on;
  if (on) f.smash = false;
}
export function setSmash(f: CombatForm, on: boolean) {
  f.smash = on;
  if (on) f.bash = false;
}

/** `cmdHealRoundsMod_Click`: one step, kept to 1..99. */
export function stepRounds(n: number, by: number): number {
  return Math.min(99, Math.max(1, n + by));
}

/** A number box as MME's `Val` reads it, kept to `[min, max]`. */
export function boxInt(text: string, min: number, max: number): number {
  const m = /^\s*([+-]?\d+)/.exec(text);
  const n = m ? Number(m[1]) : 0;
  return Math.min(max, Math.max(min, n));
}

/** The exp/hr "No recovery" setting in force (the character's own block when it has one in use). */
export function recoveryOnly(
  doc: Pick<CharacterDoc, "exp_hr">,
  s: { exp_per_hour: { global: { recovery_only: boolean }; by_character: boolean } },
): boolean {
  const per = s.exp_per_hour.by_character && doc.exp_hr;
  return (per ? doc.exp_hr : s.exp_per_hour.global)?.recovery_only ?? false;
}

export function createCombatClient(fetchFn: FetchFn = (...a) => fetch(...a)) {
  async function post<T>(path: string, body: unknown): Promise<T> {
    const res = await fetchFn(path, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) {
      const reason = (await res.text().catch(() => "")).trim();
      throw new Error(reason || `HTTP ${res.status}`);
    }
    return (await res.json()) as T;
  }
  return {
    options: (character: CharacterDoc, recovery_only: boolean) =>
      post<CombatOptions>("/api/character/combat-options", {
        character,
        recovery_only,
      }),
    preview: (
      character: CharacterDoc,
      draft: AttackDoc,
      recovery_only: boolean,
    ) =>
      post<CombatPreview>("/api/character/combat-preview", {
        character,
        draft,
        recovery_only,
      }),
  };
}

export type CombatClient = ReturnType<typeof createCombatClient>;
export const combatClient = createCombatClient();
