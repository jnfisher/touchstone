---
id: P3-13
title: Monster attack simulator
model: sonnet
phase: 3
depends: [T-022]
touches:
  - crates/touchstone-rules/src/monster_sim.rs
  - crates/touchstone-rules/tests/all/
  - fixtures/scenarios/monster_sim.json
  - fixtures/rules/monster_sim.json
  - tools/oracle/
---

## Goal
Port `clsMonsterAttackSim` (FAM-12): the round simulator, max damage, resist helpers. Follow
`docs/porting.md`.

## Decisions
- The class's property accessors become fields of a `MonsterSimInput` struct plus a `MonsterSim`
  state struct; methods `run`, `max_damage`, `is_spell_resisted`, `resisted_damage`.
- Randomness: the simulator's `RandomNumber` uses VB6 `Rnd`. Fixtures seed it (the oracle's `Rnd`
  and `touchstone-vb6::Rnd` share the generator and the T-020 seeding rules), so a seeded run is
  reproduced exactly, roll for roll. Add fixtures for the non-random averaging mode too.
- The combat log text is part of the output; match it exactly.
- GreaterMUD branches (4 procedures) are not ported.

## Done when
`docs/porting.md` "Done when", with at least 300 cases including seeded multi-round runs.
