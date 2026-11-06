---
id: Q-002
title: Quicksilver database fetch and data census
model: sonnet
phase: 3
depends: [Q-001]
touches:
  - tools/qs-fetch.sh
  - tools/qs-census.py
  - docs/data-gaps.md
  - crates/touchstone-data/tests/
---

## Goal
A Quicksilver 1.20 SQLite database can be built into `~/.cache/touchstone/` by one script, and a
census compares what it holds with what touchstone's silvermere loader and ability tables expect.
Every gap found is written down in `docs/data-gaps.md`, and a data test pins the loader's behaviour
on that database.

## Read
- `docs/quicksilver-profile.md` (all; part 2 is the list to check, part 5 the Q-002 row),
  `docs/data-gaps.md`, `docs/design.md`, `tools/silvermere-stock.sh` (the model for the fetch script).
- Quicksilver checkout (read-only, `QUICKSILVER_DEV`, default `/home/user/quicksilver-dev`):
  `docs/import.md` ("Content build"), `docs/cli.md` (`import`), `migrations/sqlite/`.
- `crates/touchstone-data/src/silvermere/` and `tests/all/silvermere.rs`;
  `crates/touchstone-rules/src/ability_text.rs` (`get_ability_name`) for the ability numbers
  touchstone knows.

## Do
1. `tools/qs-fetch.sh`: build Quicksilver's own binary from `QUICKSILVER_DEV` with a target
   directory under the cache (nothing is written into the checkout), run its default import (the
   stock package plus the 1.20 content the binary carries) into
   `~/.cache/touchstone/quicksilver-1.20.sqlite`, and print the path. Idempotent; the stock
   package is taken from silvermere's cache when present (`QUICKSILVER_STOCK_URL` or an offline
   zip otherwise). Nothing from the checkout is copied into this repository.
2. `tools/qs-census.py` (stdlib only, run with `python3 -I`): open the database read-only and
   report, as text or `--json`: schema and content version; row counts per table against the
   stock silvermere database; every ability number used in the abilities tables of items,
   monsters, spells, classes and races, marked known or unknown to touchstone (known = a name in
   `ability_text.rs`); the unknown numbers with where they occur; reserved spell ranges (5000-5999
   monster copies, 6000-6020 mana regeneration) and the 1.20 race regeneration abilities; and
   columns that hold values outside what the loader's integer types accept.
3. A data test (`crates/touchstone-data/tests/all/quicksilver.rs`) that loads the database through
   `touchstone_data::silvermere` and checks: it loads, the content version, row counts are not
   below stock, no ability row is dropped, and the unknown-ability list equals the one recorded in
   `docs/data-gaps.md`. It skips with a message when the database is not in the cache.
4. `docs/data-gaps.md`: a "Quicksilver 1.20 census" section with the counts and every gap, and
   what each means for the Q cards.

## Done when
- `tools/qs-fetch.sh` prints a database path; a second run is instant.
- `python3 -I tools/qs-census.py` runs; its output backs the numbers in `docs/data-gaps.md`.
- `cargo test -p touchstone-data` passes with the database present and skips without it.
- `cargo fmt --all --check`, clippy `-D warnings`, `cargo test --workspace --locked`,
  `tools/guard.sh` pass. The database is never committed.
