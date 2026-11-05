---
id: P3-08
title: Accuracy, hit chance and dodge
model: sonnet
phase: 3
depends: [P3-02]
touches:
  - crates/touchstone-rules/src/accuracy.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/accuracy.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/accuracy.json
  - fixtures/rules/accuracy.json
  - tools/oracle/gen/accuracy.py
---

## Goal
Port FAM-06 (stock only): the 12 procedures `docs/inventory.md` section 4 lists for it
(CalculateAttackDefense, CalculateBackstabAccuracy, CalculateAccuracy, CalcDodgeVSAccuracy, CalcDodge,
GMUD_DiminishingReturns, GetDodgeCap, GetSpellHitCap, GetHitMin, GMUD_IsLightArmourClass, GetHitCap,
DodgeMaxAccForPercent) as listed for P3-08 in section 8a. Follow `docs/porting.md` exactly.

## Read
- `CLAUDE.md`, `docs/porting.md`, `docs/inventory.md` sections 4 and 8a, `tools/oracle/README.md`.
- The 12 procedures in `modMMudFunc` through the index, and `GetClassCombat` in `modMMudDatabase`
  (already ported as `stats::get_class_combat`; use it where CalculateAccuracy calls it).
- `tools/oracle/gen/stats.py` as the model for a generator, and `tests/all/stats.rs` for the test style.

## Do
- `tools/oracle/gen/accuracy.py` writes `fixtures/scenarios/accuracy.json` (stdlib, deterministic, seeded
  splitmix64, no Python-version-dependent draws). Cover every stock branch with grids over class, level,
  stats, encumbrance, accuracy, AC, dodge and the backstab/shadow/see-hidden/vs-player flags, plus edge
  values (0, caps, negatives, Integer and Long overflow, clamps that MME's one-line `If a: b` quirk leaves
  unreachable). Keep `fixtures/rules/accuracy.json` and the scenario under 1.5 MB together.
- Cases that read a Classes row carry it as an extra `class` input (written from the stock database),
  because the Rust port never reads SQLite.
- Port into `src/accuracy.rs`. Functions take plain inputs (a small input struct for the long argument
  lists) and `Profile`; an `On Error` handler's leftover result is returned, not an error, as in
  `stats.rs`. `sReturnText` is part of the result and must match exactly.
- GreaterMUD branches and arguments that only they read (`eEvil`, intellect, charm, attack type) are not
  ported; the fixture still passes them to show they do not matter for stock.
- Add `pub mod accuracy;` to `src/lib.rs` and `mod accuracy;` to `tests/all/main.rs`, alphabetical.
  Do not edit `tests/all/fixture.rs` or `Cargo.toml`.
- Change `tools/oracle` only for a real oracle bug, minimally, and say so in the report.

## Done when
`docs/porting.md` "Done when", and `fixtures/rules/accuracy.json` + `fixtures/scenarios/accuracy.json`
together are at most 1.5 MB.
