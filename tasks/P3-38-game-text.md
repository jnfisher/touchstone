---
id: P3-38
title: Game-text inventory parser
model: sonnet
phase: 3
depends: [T-022]
touches:
  - crates/touchstone-rules/src/game_text.rs
  - crates/touchstone-rules/src/game_text/
  - crates/touchstone-rules/tests/all/game_text.rs
  - fixtures/scenarios/game_text.json
  - fixtures/rules/game_text.json
---

## Goal
Port FAM-22 (stock only): the procedures listed for P3-38 in `docs/inventory.md` section 8a, with the
inputs in section 4a. Follow `docs/porting.md` exactly.

## Decisions
- Pure text in, parsed groups out. Scenarios are invented MajorMUD-style inventory, key and notice
  texts covering every branch (do not paste real game logs); include odd spacing, plurals, counts,
  wrapped lines.
- Item matching against Items rows (`ItemNameInParseResult` and friends) is in scope only where it
  needs no character state; the rest is P3-39.
- The module `src/game_text.rs` and test file `tests/all/game_text.rs` already exist as stubs; do not edit
  `lib.rs`, `tests/all/main.rs`, `tests/all/fixture.rs` or `Cargo.toml`.
- Change `tools/oracle` only for a real oracle bug, minimally, and say so in the report.

## Done when
`docs/porting.md` "Done when".
