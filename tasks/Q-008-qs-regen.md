---
id: Q-008
title: Rest and meditate regeneration
model: sonnet
phase: 6
depends: [Q-003, Q-005]
touches:
  - crates/touchstone-rules/src/stats.rs
  - crates/touchstone-rules/src/spell_cast.rs     # calc_mana_recovery_rounds
  - crates/touchstone-rules/src/char_panel.rs
  - crates/touchstone-rules/tests/all/quicksilver_regen.rs
  - crates/touchstone-rules/tests/all/quicksilver_identity.rs
  - crates/touchstone-rules/tests/all/main.rs
  - crates/touchstone-rules/tests/all/spell_cast.rs   # call sites gain a profile
  - crates/touchstone-rules/tests/all/quicksilver_balance.rs
  - tools/qs-vectors/src/regen.rs
  - tools/qs-vectors/src/main.rs
  - fixtures/quicksilver/regen.json
  - docs/differences.md
---

## Goal
Under `Profile::Quicksilver` the Char tab's magic panel and the mana recovery time follow row R10 of
`docs/quicksilver-profile.md`: `rest` restores mana at `rest_mana_mult` times the standing rate,
`meditate` at `meditate_mana_mult` times, both heal at the rest rate, and they happen at the same time
as the HP recovery. Under `Profile::Stock` nothing changes by a byte. Engine vectors in
`fixtures/quicksilver/regen.json` pin the Quicksilver side.

## Read
- `docs/quicksilver-profile.md` (all; this is the Q-008 row), `docs/design.md`, `docs/data-gaps.md`,
  `docs/testing.md` (vectors and the identity harness), `tools/qs-vectors/`.
- MME through the index: `CalcManaRegen`, `CalcRestingRate`, `CalcManaRecoveryRounds`, `RefreshMagic`.
- Quicksilver (read-only checkout): `crates/silvermere-rules/src/regen.rs` and the rest and meditate
  ticks in `crates/silvermere-engine/src/cycles/fast.rs`. Summarise in your own words; paste nothing.

## Do
1. **What the engine does, in short.** The standing rate `N` is MME's `CalcManaRegen` (class stat,
   level, magery level, "Alter Mana Regeneration" included), paid every 30 s. Resting adds a mana
   payment on every 21 s rest tick, meditating one on every 15 s meditate tick, each sized so that
   the minute's total is `mult * N` with the slow tick counted. The payment is
   `(mult - 1) * N * period / 30`, whole numbers with the remainder (in thirtieths) carried to the
   next tick, so the total is exact. Meditating heals on the rest tick, as resting does. Both end when
   HP and mana are full. HP is unchanged: `calc_resting_rate` is already the rest rate.
2. **`stats.rs`** (additive, below `calc_mana_regen`):
   - `calc_mana_regen` keeps its MME meaning under both profiles. Its `meditating` flag is MME's stock
     10 s tick, which has no Quicksilver counterpart (the neutral balance cannot reproduce it), and
     the stock fixtures and Q-003's tests run it under the Quicksilver profile.
   - `qs_boost_tick(standing, mult, period, &mut carry)`: the engine's carry arithmetic, one tick.
   - `calc_rest_mana_regen(profile, input)` and `calc_meditate_mana_regen(profile, input)`: the
     average mana of one rest (21 s) or meditate (15 s) tick as a Currency, which is exact for both
     periods. Stock: rest gives 0, meditate is the stock tick. They count `input.mp_regen` and ignore
     `input.meditating`.
3. **`char_panel.rs`** `refresh_magic`: under Quicksilver `medi_ticks` is the meditate tick above with
   the regen buffs counted (MME passes 0 for them), a new `rest_ticks` field holds the rest tick, and the
   caption gains a "Rest Ticks" line. Stock output is untouched (`rest_ticks` is `None`).
4. **`spell_cast.rs`** `calc_mana_recovery_rounds` gains a leading `profile`. Stock: unchanged.
   Quicksilver: `calc_mana_recovery_rounds_qs(balance, meditating, ...)` runs the same 5 s round loop
   with the standing tick plus the rest tick (or the meditate tick), carry included. The profile form
   takes `meditate_rate > 0` as "meditating", else resting, because a Quicksilver character with no
   Meditate ability rests.
5. **Vectors**: `tools/qs-vectors/src/regen.rs` writes `regen.json` from the engine's own functions:
   `standing` (mana rate), `hp` (rest hp rate), `tick` (30 consecutive rest or meditate payments,
   carry included) and `recovery` (a second-by-second simulation from no mana to a target, in rounds).
   The `neutral` column is the same code at multiples of 1 (no extra mana), which is what stock MME
   answers with `meditate_rate = 0`, so `standing`, `hp` and `recovery` are screened against stock.
6. **Tests** (`tests/all/quicksilver_regen.rs`): header against `Balance`, every column above,
   stock/neutral screening, panel under Quicksilver and Stock, and the identity of the HP panel.
   Existing stock tests call the changed signature with `Profile::Stock`.
7. **docs/differences.md**: append the note on Q-008 (MME's "rest or meditate, never both" assumption
   no longer holds; the recovery model is one mode at a time on a 5 s grid).

## Done when
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `tools/guard.sh` pass.
- `tools/qs-vectors/run.sh` (with `QS_VECTORS_TARGET` under `/tmp`) twice changes no byte, `--check`
  passes, and `regen.json` is at most 1.5 MB.
- `python3 -I tools/oracle/oracle.py check --jobs 2` reports every family current.
- The stock fixtures (stats, char_panel, spell_cast, pipeline) and the identity harness pass unchanged.
