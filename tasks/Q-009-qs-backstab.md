---
id: Q-009
title: Backstab damage and crit
model: sonnet
phase: 6
depends: [Q-003, Q-005, Q-006]
touches:
  - crates/touchstone-rules/src/attack.rs          # calc_bs_damage, the stab branch and crit amounts of calculate_attack
  - crates/touchstone-rules/src/calculators.rs     # calc_bs takes the profile
  - crates/touchstone-rules/src/detail/            # backstab lines come from calculate_attack; checked, no change needed
  - crates/touchstone-rules/src/quicksilver/       # NEUTRAL: no scale cap, no stab crit (additive)
  - bin/touchstone/src/calc.rs                     # the one call site of calc_bs
  - crates/touchstone-rules/tests/
  - tools/qs-vectors/src/backstab.rs
  - tools/qs-vectors/src/main.rs
  - fixtures/quicksilver/backstab.json
  - docs/differences.md
---

## Goal
Rows R1 and R3 of `docs/quicksilver-profile.md` are live under `Profile::Quicksilver`. A stab's
damage divides stealth by `backstab_stealth_div`, multiplies by `100 + L * backstab_level_pct / 100`
percent, and reads the level held to `scalecap` in both level terms (R1). A stab can crit at the
normal crit chance (knee and 99 cap, no Quick and Deadly bonus); a critical stab rolls from the stab's
maximum up to `backstab_crit_max_pct` percent of it (R3). Under `Stock` and
`Quicksilver(&Balance::NEUTRAL)` every existing fixture gives the answer it gives now.

## Read
- `docs/quicksilver-profile.md` parts 1 (R1, R3), 4 and 5 (the Q-009 row).
- `docs/testing.md` (Quicksilver vectors, identity harness), `tasks/Q-005-qs-vectors.md`, `tasks/Q-006-qs-knees.md`.
- MME through the index: modMMudFunc `CalcBSDamage`, the backstab and crit blocks of `CalcAttack`;
  frmBSCalc `CalcBS`.
- `crates/touchstone-rules/src/attack.rs`, `calculators.rs`, `detail/item.rs` and `detail/monster.rs`
  (where backstab lines come from).

## Do
1. `calc_bs_damage` and `calc_bs` take a `Profile` first. The stab's flat term and level multiplier
   move into two small helpers shared with `calculate_attack`; stock keeps MME's 16-bit steps and
   their error order, Quicksilver reads the balance (whole-percent truncation of the multiplier,
   stealth term truncating toward zero).
2. `calculate_attack` type 4: under Quicksilver the crit chance after the knee and 99 cap is kept
   (no Quick and Deadly term, as before). The crit range in the shared tail is `max .. max * pct / 100`
   (64-bit truncating, raised to the minimum if inverted) instead of 2x..4x. The stab detail text
   gains the usual `Avg/Max Crit` part. Swings, energy and accuracy of a stab stay as they are (Q-010).
3. Detail builders (`detail/item.rs`, `detail/monster.rs`): the backstab lines are built from
   `calculate_attack`'s result with the profile they already hold, so they follow with no code change.
   `bin/touchstone/src/calc.rs` passes `loaded.profile` to `calc_bs`.
4. NEUTRAL gets `scalecap` out of reach (MME's stab has no level cap) and `backstab_crit_max_pct` 0,
   which touchstone reads as "a stab never crits" (MME's stab never does). The identity harness
   then covers stabs; it also gains the calculators fixtures.
5. Vector family `backstab` in `tools/qs-vectors/src/backstab.rs`: `bs` cases (level, stealth, weapon
   min and max, Backstab Min and Max, class stealth) giving the engine's stab min and max under V120
   and a neutral balance; `crit` cases (stab maximum, crit percent) giving the engine's critical
   stab range under V120 (no neutral column). Generated with `tools/qs-vectors/run.sh`.
6. Tests: `quicksilver_backstab.rs` asserts the v120 column through `calc_bs_damage` and through
   `calculate_attack` (stab min/max, crit range), the neutral column under Stock and NEUTRAL.
7. `docs/differences.md`: the neutral stab crit convention; steady-state crit treatment of a stab.

## Done when
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `tools/guard.sh` pass.
- Every `backstab.json` case passes in the columns that exist; the attack, calculators, paste_party
  and other registered fixtures pass under `Stock` and NEUTRAL.
- `python3 -I tools/oracle/oracle.py check --jobs 2` reports every family current.
