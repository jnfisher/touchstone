---
id: P3-14a
title: Monster sim glue, monster side
model: sonnet
phase: 3
depends: [P3-13a, P3-13b, P3-03, P3-04, P3-10]
touches:
  - crates/touchstone-rules/src/monster_glue.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/monster_glue.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/monster_glue.json
  - fixtures/rules/monster_glue.json
  - tools/oracle/gen/monster_glue.py
  - tools/oracle/harness.py
---

## Goal
Port the monster side of FAM-13 (`docs/inventory.md` section 8a row P3-14, sections 4 and 4a):
`PopulateMonsterDataToAttackSim` (a monster's attacks, spells and item bonuses into the simulator's
inputs), `CalculateMonsterItemBonuses`, `CalculateMonsterAvgDmg` against MME's default defender,
`GetMonsterAttackSummary`, and the max-damage path. The character side
(`SetupMonsterAttackSimWithCharStats`, `CalculateMonsterDamageVsChar/Party`, the damage cache) is a
later card. Follow `docs/porting.md` exactly.

## Read
- modMMudDatabase: `PopulateMonsterDataToAttackSim`, `CalculateMonsterItemBonuses`,
  `CalculateMonsterAvgDmg`, `GetMonsterAttackSummary` (through the index).
- clsMonsterAttackSim `ResetValues`, `GetMaxDamage`; the ported `monster_sim`, `spells`,
  `spell_cast`, `abilities` and `accuracy` modules.
- `bin/touchstone/src/detail.rs` holds a provisional hand-written mapping; read it for contrast
  only and do not edit `bin/`. List its differences from MME in the report.

## Decisions
- The functions take a `&World`, a small `GlueEnv` (profile, `nNMRVer`) and the persistent
  simulator state as `&mut MonsterSimInput`, because MME's one global simulator keeps durations and
  hit-spell names across monsters (`ResetValues` does not clear them).
- The scenario's `table` key holds the Monsters, Spells and Items rows the test needs, so they are
  stored once; the oracle ignores it. Cases are grids over monster numbers.
- Every stock monster gets the max-damage path and the attack summary; seeded subsets get the full
  filled state, older data versions (1.7, 1.75), flag combinations, 50/200/500-round runs and
  two-monster sequences on one simulator.
- `rules` plus `scenario` stay under 1.5 MB.
- Oracle change: `harness.py` accepts `{"obj": name}` as a call argument and `"global"` on an
  object spec, so a case can pass its simulator by reference and stand it in for `clsMonAtkSim`.

## Done when
`docs/porting.md` "Done when"; `fixtures/rules/monster_glue.json` has at least 1500 cases, all pass.
