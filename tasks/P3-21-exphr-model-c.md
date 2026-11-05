---
id: P3-21
title: Exp/hr Model C
model: sonnet
phase: 3
depends: [P3-20]
touches:
  - crates/touchstone-rules/src/exp_per_hour/
  - crates/touchstone-rules/tests/all/exp_per_hour.rs
  - fixtures/scenarios/exphr_c.json
  - fixtures/rules/exphr_c.json
  - tools/oracle/gen/exphr_c.py
---

## Goal
Port Model C (`ceph_ModelC` with its callees `cephC_BuildCombatProfile`, `cephC_BuildCycleProfile`
and `cephC_EstimateMoveSecs`), the per-lair cycle macro-simulation. Follow `docs/porting.md`; read
MME's `docs/exp-per-hour-models.md` first. Reuse `ExpPerHourInput`, `ExpPerHourKnobs`,
`ExpPerHourInfo` and the helpers P3-20 built.

## Read
- `docs/inventory.md` section 8a row P3-21, `tasks/P3-20-exphr-model-d.md`.
- Index procedures in `modExpPerHour/`: `ceph_ModelC`, `cephC_BuildCombatProfile`,
  `cephC_BuildCycleProfile`, `cephC_EstimateMoveSecs`, `cephC_Ceil`, and the `cephC_*` constants in
  `_declarations.vb`.
- `crates/touchstone-rules/src/exp_per_hour/` (P3-20) and `fixtures/scenarios/exphr_d.json`.

## Decisions
- Model C is a pure function of `CalcExpPerHour`'s arguments: `model_c(&ExpPerHourInput,
  &ExpPerHourKnobs, Profile)`. It ignores `n_rtk` and recomputes rounds-to-kill itself.
- Parity over correctness: keep the open bugs MME's docs list for the C combat profile (unconditional
  min-damage tail, integer-ceil RTK, fixed 6-round regen gate, full-round surprise miss).
- Stock path only (`bGreaterMUD` false, mob regen rounds 18).
- Scenario: call the Private `ceph_ModelC` directly, as for Model D. `tools/oracle/gen/exphr_c.py`
  writes the scenario (named edge cases, grids over the combat-profile inputs, seeded samples);
  at least 500 cases; fixtures at most 1.5 MB together with the scenario.
- `nCharMana`, `nSpellCost`, `nSurpriseChance` are `Integer`: out of range is error 6.

## Done when
`docs/porting.md` "Done when"; the `model_c_matches_oracle` test runs every case of
`fixtures/rules/exphr_c.json` with exact comparison.
