---
id: P3-26
title: Exp/hr dispatcher
model: sonnet
phase: 3
depends: [P3-20, P3-21, P3-22, P3-24, P3-12]
touches:
  - crates/touchstone-rules/src/exp_per_hour/dispatch.rs
  - crates/touchstone-rules/src/exp_per_hour/mod.rs
  - crates/touchstone-rules/tests/all/exp_per_hour.rs
  - tools/oracle/gen/exp_dispatch.py
  - fixtures/scenarios/exp_dispatch.json
  - fixtures/rules/exp_dispatch.json
---

## Goal
Port `CalcExpPerHour` (modExpPerHour), the public entry point that runs the enabled exp/hr models
(A, B, C, D), averages them and builds the text MME shows. The model flags, recovery-only and
show-all flags and the four knobs become a settings struct argument; there are no globals.

## Read
- `docs/inventory.md` section 8a row P3-26, `tasks/P3-20-exphr-model-d.md`, `tasks/P3-24-exphr-model-b.md`.
- Index procedures: `modExpPerHour/CalcExpPerHour`, `modMain/IsMobKillable`, `modSyntaxsFunc/RoundUp`
  and `AutoAppend`, `frmMain/LoadExpPerHourKnobs`, `frmMain/Form_Load` (exp/hr defaults),
  `frmSettings/cmdSave_Click` (the none-ticked fallback).
- `crates/touchstone-rules/src/exp_per_hour/` and `combat::is_mob_killable`.

## Decisions
- `calc_exp_per_hour(&ExpPerHourInput, &ExpPerHourSettings, Profile) -> Result<ExpPerHourInfo, RulesError>`.
- Defaults (parity with the code, not with `docs/exp-per-hour-models.md`, which says D is off by
  default): `Form_Load` starts with A, B, C on and D off, but `LoadExpPerHourKnobs` runs right after
  and reads each `cephModel*` key with default 1 (ReadINI writes the default back and rereads), so
  with no INI keys all four models are on. `ExpPerHourSettings::default()` is all four on,
  recovery-only and show-all off, knobs 1.0. `ExpPerHourSettings::at_form_load()` is the A+B+C start
  state. The settings dialog (`cmdSave_Click`) turns A+B+C on when none is ticked; that is
  `normalized()`. `CalcExpPerHour` itself with no model on returns the empty record. Noted in the PR body (`docs/differences.md` does not exist yet).
- Arguments are converted at the call as VB6 does: `nCharHPRegen` goes to `IsMobKillable` as an
  `Integer` and `nMobHP - nSurpriseDMG` as a `Long` (error 6 escapes `CalcExpPerHour`, which has no handler); `nSpellCost`
  and `nSurpriseChance` are `Integer` parameters.
- Parity quirks kept: `nRTC` is summed twice per model and then divided by the model count once
  per sum (so a single model reports twice its RTC), every field is `Round`ed to 2 decimals only
  when more than one model ran, the exp/hr to a whole number.
- Fixtures: `CalcExpPerHour` called with 25 arguments; named edge and error cases, every lair shape
  under all 16 model combinations with and without show-all, recovery-only, knob sets and a seeded sample.

## Done when
`docs/porting.md` "Done when"; the `calc_exp_per_hour_matches_oracle` test runs every case of
`fixtures/rules/exp_dispatch.json` with exact comparison; `oracle.py check` is current for all families.
