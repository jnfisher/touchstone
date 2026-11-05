---
id: T-023
title: VB6 IsNumeric fidelity
model: sonnet
phase: 2
depends: [T-020, T-021]
touches:
  - tools/oracle/vbi/
  - tools/oracle/tests/
  - fixtures/vb6-semantics.json
  - fixtures/rules/
  - crates/touchstone-vb6/
  - crates/touchstone-rules/src/game_text.rs
  - crates/touchstone-rules/src/game_text/
---

## Goal
The oracle and `touchstone-vb6` share one `IsNumeric` (and string `CDbl`) rule that matches what VB6
accepts under the US locale, and every ported function that tests "is this a number" uses it.

## Read
`fixtures/vb6-semantics.json`, `tools/oracle/vbi/values.py` (`str_to_double`),
`tools/oracle/vbi/builtins.py` (`f_isnumeric`), `crates/touchstone-rules/src/game_text/`.

## Do
1. Add `IsNumeric` and `CDblStr` cases to the semantics table: whitespace, leading or trailing sign,
   `$`, thousands separators, `E`/`D` exponents, `&H`/`&O` literals, parentheses for negative, empty,
   lone point, overflow. Mark undocumented behaviour `"confidence":"low"`.
2. One parser in the oracle (`parse_vb_number`) behind `IsNumeric` and string coercion; the same
   grammar as `is_numeric` / `cdbl_str` in `touchstone-vb6` (new error 13, type mismatch).
3. Replace the private numeric check in `game_text` with `touchstone_vb6::is_numeric`.
4. Regenerate all fixture families and report what changed.

## Done when
`cargo test --workspace`, the Python unit tests and `oracle.py check` pass; the diff of
`fixtures/rules/` is explained in the report.
