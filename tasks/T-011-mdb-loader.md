---
id: T-011
title: Load a World from a converted MME database, with data fixtures
model: sonnet
phase: 1
depends: [T-010]
touches:
  - crates/touchstone-data/
  - tools/data-fixtures.py
  - fixtures/data/
  - .github/workflows/ci.yml
---

## Goal
`touchstone_data::mdb::load(path) -> Result<World>` reads the SQLite produced by
`tools/mdb2sqlite.py`. Its output is checked against fixtures produced independently from the
same SQLite, without committing any game data.

## Read
`docs/model.md` (column mapping), `docs/testing.md` layer 4, `crates/touchstone-model`.

## Do
1. Loader with `rusqlite` (feature `bundled`), read-only open.
2. `tools/data-fixtures.py SQLITE OUT_DIR` (stdlib): for each table writes row count, and per row
   the record number and the first 12 hex digits of sha256 of a canonical serialisation (columns in
   schema order, `docs/model.md` names, values normalised: integers as decimal, reals with `repr`,
   text as-is, null as empty). Output `fixtures/data/stock.json`. **Hashes only, never values.**
3. A Rust test serialises the loaded `World` the same canonical way (from the model fields back
   through the mapping) and compares counts and every row hash, reporting mismatching records by
   number and column.
4. Tests find the SQLite through `TOUCHSTONE_STOCK_SQLITE`, or the default cache path; when it is
   missing the test **fails** with a message naming `tools/mdb-fetch.sh` (never skips).
5. CI (`rust` job): cache `~/.cache/touchstone/mdb` keyed on the pinned sha256; on a miss,
   `sudo apt-get install -y mdbtools` and run `tools/mdb-fetch.sh`. Must add well under a minute.

## Done when
Workspace checks pass with the stock SQLite present; loading the stock World takes under one
second in release mode (print it from a test with `--nocapture` once and report it).
