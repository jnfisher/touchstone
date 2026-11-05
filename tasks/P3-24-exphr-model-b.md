---
id: P3-24
title: Exp/hr Model B
model: sonnet
phase: 3
depends: [P3-20]
touches:
  - crates/touchstone-rules/src/exp_per_hour/model_b*
  - crates/touchstone-rules/src/exp_per_hour/mod.rs
  - crates/touchstone-rules/tests/all/exp_per_hour.rs
  - fixtures/scenarios/exphr_b.json
  - fixtures/rules/exphr_b.json
  - tools/oracle/gen/exphr_b.py
---

## Goal
Port Model B (`ceph_ModelB`), the smoothed, band-aware closed form, with its Model-B-only helpers
(`cephB_BandWeight`, `cephB_Lerp`, `cephB_MulBlend`, `cephB_CalcDensity`, `cephB_CalcOverkill`,
`cephB_CalcTravelLoopSecs`, `cephB_ApplySlackWindow`). This merges inventory rows P3-24 and P3-25:
the procedure is one piece. Follow `docs/porting.md`; read MME's `docs/exp-per-hour-models.md` first.
Reuse `ExpPerHourInput`, `ExpPerHourKnobs`, `ExpPerHourInfo` and the helpers P3-20 built.

## Read
- `docs/inventory.md` section 8a rows P3-24 and P3-25, `tasks/P3-20-exphr-model-d.md`,
  `tasks/P3-21-exphr-model-c.md`.
- Index procedures in `modExpPerHour/`: `ceph_ModelB`, the `cephB_*` procedures above,
  `cephA_InCombatMPFrac` (B inlines its own copy of the meditate branch), and the `cephB_*` and
  `DEFAULT_CEPHB_*` constants in `_declarations.vb`; `RoundUp` in `modSyntaxsFunc`.
- `crates/touchstone-rules/src/exp_per_hour/` and `fixtures/scenarios/exphr_c.json`.

## Decisions
- `model_b(&ExpPerHourInput, &ExpPerHourKnobs, Profile)`. B reads only the first 21 arguments of
  `CalcExpPerHour`; the surprise-minimum, surprise-chance, first-round and minimum-round fields are
  ignored. It leaves every text field empty.
- The hand-tuned patches MB1..MB5 and every zone-specific constant are behaviour: keep them verbatim
  and in MME's order of operations. Debug logging is not ported (no effect on results).
- Early exits: `nExp = 0` and `nTotalLairs = 0 and nRegenTime = 0` return an empty record; the boss
  shortcut returns only `nExpPerHour = Round(nExp * (1 / nRegenTime))`. All of these return the
  function value, not the local `r`.
- Run-time errors inside `ceph_ModelB` go to its handler, which leaves the function value empty:
  `Currency` overflow of `nExp * nTotalLairs` gives an empty record. In `cephB_CalcOverkill` a failing
  `Exp` gives 0, and in `cephB_CalcTravelLoopSecs` a failing `Log` (walk of -1 or less) gives 0.
- `RoundUp` (modSyntaxsFunc) is not ported by another card yet: B keeps a private copy.
- Stock path only (`bGreaterMUD` false, mob regen rounds 18). `nCharMana` and `nSpellCost` are
  `Integer`: out of range is error 6.
- Scenario: call the Private `ceph_ModelB` directly, as for Models C and D, with 21 positional
  arguments. `tools/oracle/gen/exphr_b.py` writes it (named edge cases, grids over the mid-band caster
  region the patches target, surprise, tiny-chain and route shapes, seeded samples); at least 500
  cases; scenario and fixture at most 1.5 MB together.

## Done when
`docs/porting.md` "Done when"; the `model_b_matches_oracle` test runs every case of
`fixtures/rules/exphr_b.json` with exact comparison.
