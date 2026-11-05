---
id: P3-03
title: Ability effects and lookups
model: sonnet
phase: 3
depends: [T-022]
touches:
  - crates/touchstone-rules/src/abilities.rs
  - crates/touchstone-rules/src/abilities/
  - crates/touchstone-rules/tests/all/abilities.rs
  - fixtures/scenarios/abilities.json
  - fixtures/rules/abilities.json
---

## Goal
Port FAM-03 (stock only): the procedures listed for P3-03 in `docs/inventory.md` section 8a, with the
inputs in section 4a. Follow `docs/porting.md` exactly.

## Decisions
- Data-driven: ability number to stat slot and effect. Grid the fixture over every ability number
  that appears in the stock Items, Spells, Races and Classes rows plus 0 and out-of-range values.
- `GetAbilityName` and `GetAbilityStats` text belong to P3-07, not here.
- The module `src/abilities.rs` and test file `tests/all/abilities.rs` already exist as stubs; do not edit
  `lib.rs`, `tests/all/main.rs`, `tests/all/fixture.rs` or `Cargo.toml`.
- Change `tools/oracle` only for a real oracle bug, minimally, and say so in the report.

## Done when
`docs/porting.md` "Done when".
