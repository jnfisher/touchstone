---
id: Q-005
title: Engine vector generator and fixture layout
model: sonnet
phase: 6
depends: [Q-001, Q-002, Q-003]
touches:
  - tools/qs-vectors/
  - fixtures/quicksilver/
  - crates/touchstone-rules/tests/all/quicksilver_identity.rs
  - crates/touchstone-rules/tests/all/quicksilver_vectors.rs
  - crates/touchstone-rules/tests/all/main.rs
  - crates/touchstone-rules/tests/all/stats.rs       # run() takes a profile; additive
  - crates/touchstone-rules/tests/all/accuracy.rs    # same
  - crates/touchstone-rules/tests/all/attack.rs      # same
  - docs/testing.md
---

## Goal
A local generator links Quicksilver's own rules crates and writes JSON test vectors under
`fixtures/quicksilver/`; the first family is the knees (R11 crit, R12 magic resistance, R13 dodge).
A layer-1 harness runs the existing MME fixtures through `Profile::Quicksilver(&Balance::NEUTRAL)` and
expects the stock answers. Nothing here changes a formula: Q-006 makes the knee vectors pass under
the V120 profile.

## Read
- `docs/quicksilver-profile.md` parts 1 (R11 to R13), 4 and 5 (this is the Q-005 row).
- `docs/testing.md`, `tools/oracle/README.md` (fixture header style), `tools/mme-fetch.sh` (the
  shape of a fetch-nothing script).
- `crates/touchstone-rules/src/quicksilver/` and `tests/all/{stats,accuracy,attack}.rs`.
- Quicksilver checkout (read-only, `QUICKSILVER_DEV`): `combat/attack.rs` (`effective_crit`),
  `combat/fighter.rs` (`player_fighter`), `player/secondary.rs` (`secondary`), and the `Balance`
  type. Summarise in your own words; paste nothing.

## Do
1. `tools/qs-vectors/`: a Rust crate that is its own workspace root (so the touchstone workspace
   and `cargo deny` never see it), with path dependencies on Quicksilver's types, rules and engine
   crates under `QUICKSILVER_DEV` (default `/home/user/quicksilver-dev`). `run.sh` builds it into
   `QS_VECTORS_TARGET` (default `~/.cache/touchstone/qs-vectors/`), never writes into the checkout,
   and runs it. It is never part of PR CI.
2. Output: `fixtures/quicksilver/<family>.json` with a header (family, Quicksilver commit, content
   version, the V120 and neutral values of the keys used, units) and cases sorted by id. Each case
   has `inputs`, `v120` and `neutral` outputs from the engine's own functions. Byte-identical on
   rerun. The generator's `--check` mode regenerates in memory and compares with the committed file.
3. Knees family: `crit` (the engine's crit-chance function), `mr` (the engine's secondary-stats
   function, magic resistance), `dodge` (the engine's player fighter builder, dodge rating with
   no light-load bonus). Rolls need no scripted source here; the source of scripted rolls arrives
   with the random-rule families (R3, R4, R6, R9).
4. `quicksilver_vectors.rs`: loads the knee vectors, checks the structure and header against
   `Balance::V120` / `Balance::NEUTRAL`, and checks the neutral half against the stock MME
   functions that exist today (`calc_mr`, `calc_dodge`). A neutral case where stock disagrees is
   the screening failure part 4 describes. The V120 half is Q-006's test.
5. `quicksilver_identity.rs`: the layer-1 harness. The stats, accuracy and attack fixture tests are
   reworked into `pub(crate)` functions taking a `Profile`; the stock tests call them with
   `Stock`, the harness with `Quicksilver(&Balance::NEUTRAL)`. New parametric families register
   here as their cards land.
6. `docs/testing.md`: a short section on the Quicksilver vectors and the identity harness.

## Done when
- `tools/qs-vectors/run.sh` runs (with `QS_VECTORS_TARGET` under `/tmp`) and a second run changes
  no byte; `run.sh --check` passes.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `cargo deny check` and `tools/guard.sh` pass.
- `fixtures/quicksilver/knees.json` is at most 1.5 MB, holds only numbers, and every neutral case
  matches the stock MME function.
- The identity harness passes for the stats, accuracy and attack MME fixtures.
