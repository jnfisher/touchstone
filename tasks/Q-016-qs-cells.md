---
id: Q-016
title: End-to-end cells against the balance tool
model: sonnet
phase: 6
depends: [Q-013b, Q-014]
touches:
  - tools/qs-vectors/src/cells.rs
  - tools/qs-vectors/src/main.rs                         # register the module; additive
  - tools/qs-vectors/Cargo.toml.in                       # the balance crate as a path dependency
  - fixtures/quicksilver/cells.json
  - crates/touchstone-rules/tests/all/quicksilver_cells.rs
  - crates/touchstone-rules/tests/all/main.rs            # registers the test module; additive
  - docs/differences.md                                  # append only
  - docs/testing.md
---

## Goal
Row Q-016 of `docs/quicksilver-profile.md` part 5 (layer 3 of part 4). Quicksilver's balance tool builds a whole
character per race, class and level (stats, a gear kit, standing buffs) and reports its melee damage a round against
a band target. A small set of cells (classes x levels, a few races) is recorded with the tool's figure beside the
inputs needed to rebuild the same character, and a test rebuilds each one through `CharacterState`, `refresh_all`
and `get_damage_output` under `Profile::quicksilver_v120()` and compares within a stated tolerance. Every
structural difference (steady state against the tool's simulation, the killing blow, the energy floor of 200) is
explained in `docs/differences.md`.

## Read
- `docs/quicksilver-profile.md` part 4, `docs/testing.md`, `tasks/Q-013b-qs-weave-round.md`.
- `crates/touchstone-rules/src/{pipeline,combat}.rs`, `tests/all/{pipeline,monster_rows,quicksilver_melee}.rs`.
- Quicksilver (read-only): `docs/balance/README.md`, `engine-check.md`, `tools/balance/src/{grid,sim,fight,character}.rs`.
  Summarise, paste nothing.

## Do
1. `tools/qs-vectors/src/cells.rs`: link the balance crate's library (`quicksilver_balance::grid::Grid::compute` for the
   figure; `character::cell_builds` for the gear), pick the cells, and write for each the inputs (race, class, level,
   trained stats, the wielded and worn items, the standing buffs, the target monster), the tool's melee figures and
   the tool's version. Never parse the CLI output.
2. The cells fixture is self-contained (the rules crate does no IO and the database is not in CI): it holds the item,
   spell, class, race and monster rows the character needs, as numbers.
3. The test builds a `World` from those rows, a `Character`, runs `refresh_all` and `get_damage_output` with a
   `DamageRequest` for the target, and compares `average_damage` with the tool's damage a round.
4. Tolerance and every cell outside it are stated in `docs/differences.md`; a local bug in the Quicksilver profile
   found on the way is fixed, anything else is a follow-up.

## Done when
- `QS_VECTORS_TARGET=/tmp/Q-016/qsv tools/qs-vectors/run.sh` writes `fixtures/quicksilver/cells.json` (at most 200 KB);
  a second run and `--check` change nothing.
- Workspace checks and `tools/guard.sh` pass; every other fixture and the identity harness are unchanged.
