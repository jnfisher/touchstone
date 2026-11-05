---
id: P3-22
title: Exp/hr Model A
model: sonnet
phase: 3
depends: [P3-20]
touches:
  - crates/touchstone-rules/src/exp_per_hour/
  - crates/touchstone-rules/tests/all/exp_per_hour.rs
  - fixtures/scenarios/exphr_a.json
  - fixtures/rules/exphr_a.json
  - tools/oracle/gen/exphr_a.py
---

## Goal
Port Model A of `modExpPerHour`: `ceph_ModelA` (one 1,072-line procedure with section banners) and
its two Model-A-only helpers, `cephA_CalcHPRecoveryRounds` and `cephA_InCombatMPFrac`. This merges
inventory rows P3-22 and P3-23. Follow `docs/porting.md`; read MME's `docs/exp-per-hour-models.md`
in the MME cache first.

## Decisions
- Model A is a pure function of `CalcExpPerHour`'s arguments: reuse P3-20's `ExpPerHourInput`,
  `ExpPerHourKnobs`, `ExpPerHourInfo` and the shared helpers in `exp_per_hour/helpers.rs`. Model A
  takes only the first 21 arguments (it ignores the surprise-minimum, surprise-chance, first-round
  and minimum-round fields) and sets only the numeric fields of the result; the text fields stay
  empty.
- `ceph_ModelA` has `On Error GoTo error ... Resume out`, so a VB6 run-time error inside it is
  swallowed and the caller gets whatever was assigned so far, which is an empty record (the result
  is only assigned at the end). The port returns `Ok(ExpPerHourInfo::default())` in that case. The
  same holds inside `cephA_CalcHPRecoveryRounds`, which then returns 0 and lets Model A carry on.
- Keep the quirks: both branches of `IIf` are evaluated (a zero divisor in the unselected branch is
  still error 11), `nDamageThreshold = -1` (exactly) is "basic damage" and any other negative is
  not, a negative sign on `nMove` flags limited movement and on `nAttackTime` a worse-than-normal
  surprise opener (the caller `CalcExpPerHour` strips it), Round is banker's rounding, the
  unused mana-credit variable stays zero.
- Split the port by MME's section banners (`model_a/`): surprise opener, HP recovery, mana, movement,
  overlap and totals. A GoTo becomes a flag. GreaterMUD only changes the mob-regen round constant
  (6 instead of 18); match on `Profile` as in Model D.
- Scenario (at least 500 cases, `tools/oracle/gen/exphr_a.py` writes it): grids and seeded samples
  over lair counts and spawns, regen times, walk speed and distance, rounds-to-kill (0 derives it),
  damage taken and thresholds, HP/mana/regen, spells, surprise damage, knobs, plus the edge cases
  (bail-outs, boss, basic damage, cluster detection, overflowing counts). Rules plus scenario at most
  1.5 MB together.

## Done when
`docs/porting.md` "Done when".
