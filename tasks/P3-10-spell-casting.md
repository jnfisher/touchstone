---
id: P3-10
title: Spell casting
model: sonnet
phase: 3
depends: [P3-02, P3-03, P3-04]
touches:
  - crates/touchstone-rules/src/spell_cast.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/spell_cast.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/spell_cast.json
  - fixtures/rules/spell_cast.json
  - tools/oracle/gen/spell_cast.py
---

## Goal
Port FAM-09 (stock only): the procedures listed for P3-10 in `docs/inventory.md` section 8a, with the
inputs in section 4a. Follow `docs/porting.md` exactly.

## Read
- `docs/porting.md`, `docs/inventory.md` rows P3-10 and FAM-09, `tools/oracle/README.md`.
- MME through the index: modMMudFunc `CalculateSpellCast`, `CalculateResistDamage`,
  `ResistPct_SignedOfBase`, `NegResistPct_ShareOfTotal`, `SpellIsInGame`, `SpellIsUsable`,
  `SpellAttackTypeEnum`, `GetSpellCastChance`; modMain `CalcRoundsToOOM`, `CalcManaRecoveryRounds`;
  modMMudDatabase `SpellSeek`, `GetClassMagery`, `GetClassMageryLVL`.
- Already ported, to be called and not redone: `spells::get_current_spell_min_max`,
  `stats::get_class_magery{,_lvl}`, `accuracy::get_spell_hit_cap`.

## Decisions
- `GMUD_GetSpDmgMultiplierFromSC` and the GreaterMUD-only ability cases are not ported.
- `CalculateSpellCast` reads five `tCharacterProfile` fields (spell damage bonus, spellcasting, max
  mana, spell overhead, mana regen). Define them as a small `SpellCasterProfile` in `spell_cast.rs`;
  P3-11 unifies it with the full profile later.
- The "LVL Increases" text calls `GetAbilityStats(n, 0)` (P3-07's). Take it through the existing
  `spells::SpellText` trait; the fixture records the answers (`as-*` cases).
- Where MME's `On Error ... Resume out` handler returns a partly filled result or 0, so does the port.
- Spells and Classes rows are slices (the fixture's `table` case); globals (`nNMRVer`, `bOnlyInGame`,
  `bDisableKaiAutolearn`, `gAvgLevelMaxAllStats`) are struct fields.
- Scenarios cover: every spell attack type, the elemental resists, MR and anti-magic thresholds,
  cast-level clamps (negative, cap, over cap, Integer overflow), mana/OOM and aura durations, cast
  chance, area spells and each ability shape in the damage loop. Rules plus scenario stay under 1.5 MB.
- Change `tools/oracle` only for a real oracle bug, minimally, and say so in the report.

## Done when
`docs/porting.md` "Done when": `cargo test --workspace` passes, `oracle.py check spell_cast` reports the
fixture current, and the family's test runs every fixture case.
