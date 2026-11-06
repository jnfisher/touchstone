---
id: P3-12
title: Damage vs monster and combat rounds
model: sonnet
phase: 3
depends: [P3-03, P3-09, P3-10, P3-11]
touches:
  - crates/touchstone-rules/src/combat.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/combat.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/combat.json
  - fixtures/rules/combat.json
  - tools/oracle/gen/combat.py
  - tools/oracle/
---

## Goal
Port FAM-08 (stock only): "how much damage does the character do to monster N per round, and how many
rounds to kill it". Used by the monster list, lairs (P3-15/16), the exp/hr dispatcher (P3-26) and the
row evaluators. Follow `docs/porting.md` exactly.

## Read
- `docs/porting.md`, `docs/inventory.md` rows P3-12 and FAM-08, `tools/oracle/README.md`.
- MME through the index: modMMudFunc `CalcCombatRounds`, `ccr_Saturate`, `ccr_SmoothStep`, `ccr_Lerp`,
  `ccr_SafeDiv`, `ccr_Min`, `ccr_Max`; modMain `GetDamageOutput`, `IsMobKillable`, `CalcAverageNonZero`,
  `ClearSavedDamageVsMonster`.
- Already ported, to be called and not redone: `attack::calculate_attack`,
  `spell_cast::SpellCastEnv::calculate_spell_cast`, `spell_cast::calculate_resist_damage`,
  `character::populate_character_profile`, `abilities::item_has_ability`.

## Decisions
- `GetDamageOutput` fills six module-level arrays as a per-monster cache. The port is a pure function
  that returns the value: no cache read, no cache write. `ClearSavedDamageVsMonster` only resets that
  cache and copies the attack-config key, so nothing in the port calls it and it is not ported
  (`character::AttackConfigKey::changed` already reports when MME would call it).
- Inputs are the existing P3-11 types, not a new pipeline. `DamageEnv` carries a `character::Context`
  (tables, stats, lair-mode controls), the `Character` (attack choice), the `EquippedGlobals` and
  `CastText` that `calculate_attack` takes, and the Monsters and Races tables. MME calls
  `PopulateCharacterProfile` up to three times per call on one reused `tCharacterProfile`, with
  different attack kinds; the port does the same on one profile, so the fields a later call leaves
  alone keep the earlier values.
- `nSingleMonster` below 1 means "use the AC/DR/MR/... passed in"; otherwise the Monsters row is read
  (not found gives the all `-9999` start value, as MME's early exit does). The `nSpeedAdj` argument is
  overwritten with 100 by MME, so the port has none.
- A VB6 error inside `GetDamageOutput` leaves the start value (`-9999` for average, first-round and
  surprise damage, 0 for the rest): MME assigns the result before the work and `Resume out` returns it.
  `CalcCombatRounds` keeps the partly filled result, as the other handler-ed procedures do.
- The ByVal Long arguments of `CalcCombatRounds` are `i32`; callers that hold a Currency or Double
  convert with `CLng` themselves (P3-15/16/26).
- Weapons that cast spells (ability 43) are left out of the character pool: their cast text needs the
  spell text tables and is covered by the attack family. `CastText` is a panic stub in the test.
- GreaterMUD branches are not ported (`Profile` has only `Stock`).

## Done when
`cargo test --workspace` passes, `python3 -I tools/oracle/oracle.py check combat` reports the fixture
current, and the family's test runs every fixture case. Scenarios plus rules stay under 1.5 MB.
