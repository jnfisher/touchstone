---
id: T-012
title: Read a live silvermere/Quicksilver SQLite database into a World
model: sonnet
phase: 1
depends: [T-010]
touches:
  - crates/touchstone-data/src/silvermere/
  - crates/touchstone-data/tests/
  - crates/touchstone-data/vendor/silvermere-migrations/
  - docs/data-gaps.md
---

## Goal
`touchstone_data::silvermere::open(path)` opens a running server's SQLite database read-only and
builds the same `World` the `.mdb` loader builds, with a cheap way to tell when it changed.

## Read
`jnfisher/silvermere` (public): `migrations/sqlite/*.sql`, `docs/schema.md`, `docs/import.md`
(how stock records map into its tables). `jnfisher/quicksilver-dev` differs only by
`migrations/sqlite/9001_content_version.sql`. `docs/model.md` in this repository.

## Do
1. Vendor silvermere's `migrations/sqlite/*.sql` (MIT; keep its copyright notice in a `NOTICE` file
   in the vendor directory) for tests.
2. Open with `file:PATH?mode=ro` (URI) via `rusqlite`. Read `_sqlx_migrations`; accept the known
   versions (silvermere's, plus Quicksilver's 9001); otherwise fail with a message naming the version
   found and the versions supported.
3. Map silvermere's tables (items + item_abilities + item_classes + ..., monsters + monster_attacks +
   ..., spells, rooms + room_exits, shops + shop_items, classes, races) into the model.
4. `docs/data-gaps.md`: a table of every model field: the silvermere source, or `none` with what the
   loader fills in (and why that is safe), for example NMR-computed references.
5. `fn data_version(&self) -> Result<i64>` (`PRAGMA data_version` on the held connection) for the
   server's reload loop.
6. Tests: create a temp SQLite in WAL mode from the vendored migrations, insert a few **invented**
   records (made-up names and numbers, not game data), load, assert the mapping; open while a
   second writer connection is mid-transaction to show reads do not block or fail; bump data and
   see `data_version` change; reject an unknown migration version.

## Done when
Workspace checks pass; `docs/data-gaps.md` covers every model field.
