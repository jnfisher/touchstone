---
id: P3-04b
title: pull_spell_eq integer handling (Long/Integer overflow parity)
model: sonnet
phase: 3
depends: [P3-04]
touches:
  - crates/touchstone-rules/src/spells.rs
  - tools/oracle/gen/spells.py
  - fixtures/scenarios/spells.json
  - fixtures/rules/spells.json
  - crates/touchstone-rules/tests/all/spells.rs
  - docs/differences.md
---

## Goal
`PullSpellEQ` and its callees mix VB6 `Integer` (error 6 on overflow), `Long`, `Currency` and `Double`
arithmetic. The port must overflow, truncate and round exactly where MME does. The original follow-up
note ("`pull_spell_eq` Long handling") is lost, so this is an investigation: list every expression whose
VB type differs from the Rust type, probe each with oracle cases, fix any mismatch.

## Read
- `PullSpellEQ`, `GetCurrentSpellMinMax`, `GetMaxLevel`, `RoundUpTo5` through the index; the signatures of
  `GetAbilityStats`, `GetSpellName` and `GetMonsterName` (Integer, Long, Long parameters).
- `crates/touchstone-rules/src/spells.rs`, `tools/oracle/gen/spells.py`, `docs/testing.md`.

## Do
- Add invented Spells rows (scenario `db_extra`, numbers from 80001, also carried in the `table` case)
  with extreme and negative min/max/duration/level fields, caps of 0 and 9999, a required level or cap
  above Integer range, ability values above Integer range and EndCast/Summon ranges at the Long edge.
- Probe them at caster levels 0, 1, 255 and 9999, with bonus 0/25/32767, overrides at the Long limits,
  global-level texts that overflow or round, `gAvgLevelMaxAllStats` values, and arguments outside
  Integer range (error 6 raised at the call, before the port is reached).
- Fix each mismatch in `spells.rs`; record any real-game difference in `docs/differences.md`.

## Done when
Workspace checks pass; `oracle.py check spells` is current; the report lists each expression checked
and its verdict.
