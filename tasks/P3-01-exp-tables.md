---
id: P3-01
title: Exp tables and training cost
model: sonnet
phase: 3
depends: [T-022]
touches:
  - crates/touchstone-rules/src/exp.rs
  - crates/touchstone-rules/tests/all/
  - fixtures/scenarios/exp.json
  - fixtures/rules/exp.json
  - tools/oracle/
---

## Goal
Port FAM-01 (stock only): `CalcExpNeeded_STOCK`, `GetExpModifiers_STOCK`, `CalcExpNeededByRaceClass`
and the training-cost procedures. Follow `docs/porting.md`.

## Decisions
- Inputs: level, race and class exp tables (Races/Classes `ExpTable`), `nGlobalDatVer` from Info.
- Scenarios: every race x class pair x levels 1 to 255 (or MME's max) for exp needed; training cost
  over the same grid. Results are Currency/Double: compare Currency exactly.
- GreaterMUD exp variants are not ported.

## Done when
`docs/porting.md` "Done when".
