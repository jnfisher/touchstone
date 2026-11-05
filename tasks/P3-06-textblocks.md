---
id: P3-06
title: Textblock and room-command parsing
model: sonnet
phase: 3
depends: [T-022]
touches:
  - crates/touchstone-rules/src/textblock.rs
  - crates/touchstone-rules/src/textblock/
  - crates/touchstone-rules/tests/all/textblock.rs
  - fixtures/scenarios/textblock.json
  - fixtures/rules/textblock.json
---

## Goal
Port FAM-19 (stock only): the procedures listed for P3-06 in `docs/inventory.md` section 8a, with the
inputs in section 4a. Follow `docs/porting.md` exactly.

## Decisions
- Pure text parsing over TBInfo and Rooms columns. Scenarios over every stock textblock and every
  room with a command, so the fixture covers the real syntax.
- `GetLocations` is P3-31, not here.
- The module `src/textblock.rs` and test file `tests/all/textblock.rs` already exist as stubs; do not edit
  `lib.rs`, `tests/all/main.rs`, `tests/all/fixture.rs` or `Cargo.toml`.
- Change `tools/oracle` only for a real oracle bug, minimally, and say so in the report.

## Done when
`docs/porting.md` "Done when".
