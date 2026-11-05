---
id: P3-02
title: Stats, vitals, creation points and movement
model: sonnet
phase: 3
depends: [T-022]
touches:
  - crates/touchstone-rules/src/stats.rs
  - crates/touchstone-rules/src/stats/
  - crates/touchstone-rules/tests/all/stats.rs
  - fixtures/scenarios/stats.json
  - fixtures/rules/stats.json
---

## Goal
Port FAM-02 (stock only): the procedures listed for P3-02 in `docs/inventory.md` section 8a, with the
inputs in section 4a. Follow `docs/porting.md` exactly.

## Decisions
- Functions take plain inputs (level, stats, race/class model rows, encumbrance) rather than reading
  globals; the fixture records each case's globals and form values (`chkGlobalFilter`,
  `cmbGlobalRace/Class`, `lblDatVer`) as inputs.
- `sReturnText` outputs (tooltip text) are part of the fixture; match them exactly.
- The module `src/stats.rs` and test file `tests/all/stats.rs` already exist as stubs; do not edit
  `lib.rs`, `tests/all/main.rs`, `tests/all/fixture.rs` or `Cargo.toml`.
- Change `tools/oracle` only for a real oracle bug, minimally, and say so in the report.

## Done when
`docs/porting.md` "Done when".
