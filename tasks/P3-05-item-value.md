---
id: P3-05
title: Item value and shop price
model: sonnet
phase: 3
depends: [T-022]
touches:
  - crates/touchstone-rules/src/item_value.rs
  - crates/touchstone-rules/src/item_value/
  - crates/touchstone-rules/tests/all/item_value.rs
  - fixtures/scenarios/item_value.json
  - fixtures/rules/item_value.json
---

## Goal
Port FAM-18 (stock only): the procedures listed for P3-05 in `docs/inventory.md` section 8a, with the
inputs in section 4a. Follow `docs/porting.md` exactly.

## Decisions
- Currency arithmetic: compare exactly (`fixture::cur`).
- Grid over every stock shop x the items it stocks, plus charm values 0, 50, 100 and edge values.
- The GreaterMUD gang-house markup is not ported.
- The module `src/item_value.rs` and test file `tests/all/item_value.rs` already exist as stubs; do not edit
  `lib.rs`, `tests/all/main.rs`, `tests/all/fixture.rs` or `Cargo.toml`.
- Change `tools/oracle` only for a real oracle bug, minimally, and say so in the report.

## Done when
`docs/porting.md` "Done when".
