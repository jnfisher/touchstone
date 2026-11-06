---
id: Q-014
title: Exp/hr under Quicksilver (Model D)
model: sonnet
phase: 6
depends: [Q-008, Q-010, Q-013b]
touches:
  - crates/touchstone-rules/src/exp_per_hour/            # profile gate for Models A to C, Model D recovery, the dispatch
  - crates/touchstone-rules/src/detail/monster.rs        # the monster pane shows the dispatch's notice (additive)
  - crates/touchstone-rules/src/stats.rs                 # `regen_on` is shared with the exp/hr models (visibility only)
  - crates/touchstone-rules/tests/all/exp_per_hour.rs    # the oracle tests take a profile
  - crates/touchstone-rules/tests/all/quicksilver_exphr.rs
  - crates/touchstone-rules/tests/all/quicksilver_identity.rs
  - crates/touchstone-rules/tests/all/main.rs            # registers the test module; additive
  - tools/qs-vectors/src/exphr_d.rs
  - tools/qs-vectors/src/main.rs                         # register the family; additive
  - fixtures/quicksilver/exphr_d.json
  - web/src/lib/components/settings/SettingsDialog.svelte # a notice, and Models A to C shown as unavailable
  - web/src/lib/                                          # helper and test for that notice
  - docs/differences.md                                   # append only
  - docs/testing.md
---

## Goal
Row Q-014 of `docs/quicksilver-profile.md` part 5 (R2, R6 and R10 in the exp/hr models). Under
`Profile::Quicksilver` with the 1.20 rest and meditate multiples, Model D recovers HP and mana in the same
phase (rest or meditate, whichever the character can do, both heal at the rest rate and both end when the bars
are full), Models A to C return `RulesError::Unsupported` because they assume rest-or-meditate, and
`calc_exp_per_hour` runs Model D alone and reports a notice. The monster pane and the settings dialog show the
notice as plain text, never as an error. The opener (R2) and the opportunist and weave procs (R4, R6) already
reach Model D through its inputs (the round-count credit, damage per round, first-round damage); this card does
not duplicate them. Under `Stock` and `Quicksilver(&Balance::NEUTRAL)` every `exphr_*` and `exp_dispatch`
fixture gives exactly the answer it gives now.

## Read
- `docs/quicksilver-profile.md` (all), `docs/design.md`, `docs/data-gaps.md`, `docs/testing.md`.
- `tasks/Q-008-qs-regen.md`, `tasks/Q-010-qs-opener.md`, `tasks/Q-013b-qs-weave-round.md`;
  `crates/touchstone-rules/src/{stats.rs (qs_boost_tick, regen_on), spell_cast.rs (recovery rounds)}`,
  `exp_per_hour/{dispatch,model_d}.rs`, `tests/all/{exp_per_hour,quicksilver_identity,quicksilver_regen}.rs`,
  `tools/qs-vectors/src/regen.rs`.
- MME through the index: modExpPerHour `ceph_ModelD`, `CalcExpPerHour`.
- Quicksilver (read-only): `crates/silvermere-rules/src/regen.rs`, `crates/silvermere-engine/src/cycles/fast.rs`
  (rest and meditate ticks), `tools/balance/src` (checked: no exp/hr figure). Summarise; paste nothing.

## Do
1. **The gate.** Quicksilver exp/hr changes follow `stats::regen_on` (R10 on: a balance whose rest or meditate
   multiple is not 1), the convention of Q-008: NEUTRAL reads as off and everything is stock.
2. **Models A to C** return `RulesError::Unsupported` when R10 is on, before they read the input.
3. **`calc_exp_per_hour`** with R10 on runs Model D only (when any model was ticked), whatever the settings say,
   and sets `ExpPerHourInfo::s_notice` (a new field, empty in Stock) when Models A to C were requested.
4. **Model D recovery** with R10 on: one recovery phase. Mana comes back at `mult * standing / 30` per second
   (`rest_mana_mult` resting, `meditate_mana_mult` when the character has a meditation rate), HP at MME's rest
   rate plus the passive rate. The phase lasts until both targets are met: the longer of the two times. The HP
   share is its own time (with the heavy-hit relief); the mana share is what it adds beyond that. Nothing else in
   the model changes.
5. **Killing blow.** Opportunist and weave fire only on a hit that leaves the target alive. Model D takes the
   rounds to kill as an input (`n_rtk`) and the damage per round already holds the procs as the Q-011 and Q-013b
   steady-state expectation; it does not track monster HP per blow, so it cannot stop a proc on the last blow.
   Decision: keep the steady state and say so in `docs/differences.md`.
6. **Monster pane** prints `s_notice` as a plain row after the movement text.
7. **Web**: the settings dialog reads `/api/info`'s profile; under Quicksilver Models A to C show as unavailable
   (unchecked, disabled) with a one-line notice, and Model D is shown on.
8. **Vectors** (`tools/qs-vectors/src/exphr_d.rs`, family `exphr_d`). The balance tool has no exp/hr figure, so
   the vectors are cross-checks from the engine's own rest, meditate and standing ticks: `cycle` (hit points and
   mana gained over one whole 210-second cycle, resting and meditating, exact), and `recovery` (a second-by-second
   run of HP and mana together to both targets, in seconds). Columns `v120` and `neutral` (both multiples 1).
9. **Tests**: the vectors through Model D's recovery helper, Models A to C unsupported, dispatch D-only and the
   notice, Stock and NEUTRAL identical (the `exphr_*` and `exp_dispatch` tests take a profile and are registered
   in `quicksilver_identity.rs`). `docs/differences.md`, `docs/testing.md`.

## Done when
- `QS_VECTORS_TARGET=/tmp/Q-014/qsv tools/qs-vectors/run.sh` writes `fixtures/quicksilver/exphr_d.json` (at most
  1.5 MB); a second run and `--check` change nothing.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `tools/guard.sh` and the web checks pass.
- Every other fixture and the identity harness are unchanged.
- `python3 -I tools/oracle/oracle.py check --jobs 2` reports every family current.
