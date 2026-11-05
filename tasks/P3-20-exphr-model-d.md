---
id: P3-20
title: Exp/hr shared helpers and Model D
model: sonnet
phase: 3
depends: [T-022]
touches:
  - crates/touchstone-rules/src/exp_per_hour/
  - crates/touchstone-rules/tests/all/
  - fixtures/scenarios/exphr_d.json
  - fixtures/rules/exphr_d.json
  - tools/oracle/
---

## Goal
Port the shared exp/hr helpers in `modExpPerHour` and Model D (`ceph_ModelD`, the round-by-round
model MME recommends). Follow `docs/porting.md`; read MME's `docs/exp-per-hour-models.md` in the MME
cache first.

## Decisions
- Model D is a pure function of `CalcExpPerHour`'s arguments: make them one `ExpPerHourInput` struct
  shared by all four models (P3-21 to P3-26 reuse it), output `ExpPerHourInfo` mirroring
  `tExpPerHourInfo`.
- Scenario grid: vary rounds-to-kill, monster HP/exp, character HP/mana/regen, damage taken, lair
  counts and movement inputs across realistic and edge ranges; at least 500 cases.
- Model text formatters (`Format` patterns) are part of the output.

## Done when
`docs/porting.md` "Done when".
