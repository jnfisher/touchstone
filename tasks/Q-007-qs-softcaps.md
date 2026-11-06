---
id: Q-007
title: Stat soft caps and training prices
model: sonnet
phase: 6
depends: [Q-003, Q-005]
touches:
  - crates/touchstone-rules/src/stats.rs
  - crates/touchstone-rules/src/char_panel.rs
  - tools/qs-vectors/src/main.rs                      # register the family; additive
  - tools/qs-vectors/src/training.rs
  - fixtures/quicksilver/training.json
  - crates/touchstone-rules/tests/all/quicksilver_vectors.rs
  - crates/touchstone-rules/tests/all/quicksilver_identity.rs
  - crates/touchstone-rules/tests/all/char_panel.rs   # run takes a profile; additive
  - crates/touchstone-rules/tests/all/stats.rs
  - docs/differences.md                               # append only
  - docs/testing.md
---

## Goal
Under `Profile::Quicksilver` a stat can be trained past the race maximum to a soft cap and 30 points
beyond, at Quicksilver's prices (R14). The creation-point panel (`RefreshCPs`) prices a stat above
the race maximum that way, and the soft cap and training maximum are available as functions for the
character editor. Under `Stock` and `Quicksilver(&Balance::NEUTRAL)` every answer is MME's.

## Read
- `docs/quicksilver-profile.md` part 1 (R14), parts 4 and 5 (this is the Q-007 row).
- MME `frmMain RefreshCPs`, `modMMudDatabase CP_CostForRaise` (through the index).
- `crates/touchstone-rules/src/{stats,char_panel}.rs`, `tests/all/{quicksilver_identity,quicksilver_vectors,char_panel}.rs`,
  `tools/qs-vectors/src/knees.rs` (the shape of a family).
- Quicksilver checkout (read-only): `silvermere-rules/src/progression.rs` (`soft_cap`, `train_max`,
  `train_cost`). Summarise in your own words; paste nothing.

## Do
1. `stats.rs`: `soft_cap`, `train_max` and `train_cost(profile, race_min, race_max, from, to)`. The cost
   from the race minimum to a value is the stock ladder up to the race maximum, then the floor price
   a point up to the soft cap `max(race_max, softcap_floor)`, then three bands of ten points at the band
   prices; past the last band the last price continues. `train_cost` is the difference of two such
   totals. `cp_cost_for_raise` keeps its signature: its only caller never raises past the race range.
2. Decision: **"soft caps off" is read from the balance.** MME prices points above the race maximum on
   the stock ladder (it has no notion of a maximum in the price). `Balance::NEUTRAL` cannot say that
   (its flat 10 a point matches the ladder only from 90 points above the minimum), so a balance whose
   three soft-cap keys equal NEUTRAL's (floor 0, floor price 10, bands 10/10/10) is treated as "off" and
   prices on the stock ladder throughout, with no training maximum. This keeps the identity harness exact.
3. `char_panel.rs` `cps_steps`: when the profile has soft caps on and the race is known, a stat above the
   race maximum costs the ladder to the maximum plus the part above it. No race, or an unknown race:
   the stock ladder, as before. The per-stat cost keeps MME's 32-bit overflow behaviour.
4. Vectors: `tools/qs-vectors/src/training.rs`, family `training`: kinds `cost` (race min and max,
   from, to), `softcap`, `train_max`. `neutral` is the engine at neutral settings; it is `null` where
   the engine and MME cannot agree (a target above the race maximum), and the vectors test checks that
   rule and the stock agreement elsewhere.
5. Tests: a `training` test in `quicksilver_vectors.rs` (header, V120 column against `Profile::Quicksilver(V120)`,
   neutral column against the stock path); a `RefreshCPs` test under V120 with a built world; the
   `char_panel` fixtures registered in the identity harness (`check_fixture(profile)`).
6. `docs/differences.md`: the "off" reading of the soft-cap keys; `docs/testing.md`: one line.

## Done when
- `tools/qs-vectors/run.sh` (with `QS_VECTORS_TARGET=/tmp/Q-007/qsv`) writes `fixtures/quicksilver/training.json`
  (at most 1.5 MB, numbers only), and a second run, and `--check`, change nothing.
- The V120 column equals `train_cost`/`soft_cap`/`train_max` under `Profile::Quicksilver(&Balance::V120)` for
  every case; the neutral column equals the stock path where present.
- Every existing fixture and the identity harness (now including `char_panel`) pass under `Stock` and
  `Quicksilver(&Balance::NEUTRAL)`; `python3 -I tools/oracle/oracle.py check --jobs 2` is current.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `tools/guard.sh` pass.
