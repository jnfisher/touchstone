import { describe, expect, it, vi } from "vitest";
import { defaultAttack } from "./character";
import {
  boxInt,
  buildDraft,
  createCombatClient,
  formFromAttack,
  recoveryOnly,
  setBash,
  setSmash,
  stepRounds,
  type CombatOptions,
} from "./combat";

const opts: CombatOptions = {
  nmr_ver: 1.83,
  recovery_only: false,
  current_weapon: "(no weapon equipped)",
  attack_learned: [{ number: 5, label: "Fire (5)" }],
  attack_any: [{ number: 5, label: "Fire (5) - LVL 1 Mage-1" }, { number: 9, label: "Ice (9)" }],
  heal_learned: [{ number: 7, label: "Heal (7)" }],
  heal_any: [{ number: 7, label: "Heal (7) - LVL 1 Priest-1" }],
  backstab_weapons: [{ number: 30, label: "Dirk (30)" }],
};

describe("SetupChooseAttack mapping", () => {
  it("shows bash and smash as the weapon radio with a box", () => {
    const f = formFromAttack({ ...defaultAttack(), attack_type: 7 }, opts, undefined);
    expect([f.radio, f.bash, f.smash]).toEqual([1, false, true]);
    expect(buildDraft(f).attack_type).toBe(7);
    setBash(f, true);
    expect([f.bash, f.smash]).toEqual([true, false]);
    setSmash(f, true);
    expect([f.bash, f.smash]).toEqual([false, true]);
  });

  it("selects the saved spell only in lists that offer it", () => {
    const f = formFromAttack({ ...defaultAttack(), attack_type: 3, spell_num: 9 }, opts, undefined);
    expect([f.radio, f.learnedSpell, f.anySpell]).toEqual([3, 0, 9]);
    expect(buildDraft(f).spell_num).toBe(9);
  });

  it("takes the character's level for an unset spell level only with the global filter", () => {
    expect(formFromAttack(defaultAttack(), opts, 12).spellLvl).toBe(12);
    expect(formFromAttack(defaultAttack(), opts, undefined).spellLvl).toBe(0);
    expect(formFromAttack({ ...defaultAttack(), spell_lvl: 3 }, opts, 12).spellLvl).toBe(3);
  });

  it("keeps the saved spell and heal spell when the radio does not use them", () => {
    const base = { ...defaultAttack(), spell_num: 5, heal_spell_num: 7, heal_type: 1 };
    const d = buildDraft(formFromAttack(base, opts, undefined));
    expect([d.spell_num, d.heal_spell_num, d.heal_type]).toEqual([5, 7, 1]);
  });

  it("drops a backstab weapon that is no longer offered", () => {
    const f = formFromAttack({ ...defaultAttack(), backstab_weapon: 99 }, opts, undefined);
    expect(f.backstabWeapon).toBe(0);
    expect(formFromAttack({ ...defaultAttack(), backstab_weapon: -1 }, opts, undefined).backstabWeapon).toBe(-1);
  });
});

describe("boxes", () => {
  it("reads like Val and clamps", () => {
    expect(boxInt("12abc", 0, 99)).toBe(12);
    expect(boxInt("", 0, 99)).toBe(0);
    expect(boxInt("500", 0, 99)).toBe(99);
    expect(boxInt("-4", 0, 99)).toBe(0);
  });
  it("steps the heal rounds within 1..99", () => {
    expect(stepRounds(1, -1)).toBe(1);
    expect(stepRounds(99, 1)).toBe(99);
    expect(stepRounds(5, 1)).toBe(6);
  });
  it("uses the character's recovery setting only when per-character is on", () => {
    const s = (by: boolean, g: boolean) => ({ exp_per_hour: { global: { recovery_only: g }, by_character: by } });
    const own = { exp_hr: { recovery_only: true } } as never;
    expect(recoveryOnly(own, s(true, false))).toBe(true);
    expect(recoveryOnly(own, s(false, false))).toBe(false);
    expect(recoveryOnly({}, s(true, true))).toBe(true);
  });
});

describe("client", () => {
  it("posts the character and draft, and surfaces the server's reason", async () => {
    const f = vi.fn(async (_u: string, init?: RequestInit) =>
      init && String(init.body).includes('"bad"')
        ? new Response("pick a class and race", { status: 422 })
        : new Response("{}", { status: 200 }),
    );
    const c = createCombatClient(f as never);
    await c.preview({ name: "a" } as never, defaultAttack(), false);
    expect(f.mock.calls[0][0]).toBe("/api/character/combat-preview");
    expect(JSON.parse(String(f.mock.calls[0][1]!.body)).draft.attack_type).toBe(0);
    await expect(c.options({ name: "bad" } as never, false)).rejects.toThrow("pick a class and race");
  });
});
