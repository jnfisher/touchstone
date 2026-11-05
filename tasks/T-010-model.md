---
id: T-010
title: World model types from the MME database schema
model: sonnet
phase: 1
depends: [T-001, T-002]
touches:
  - crates/touchstone-model/
  - docs/model.md
---

## Goal
`touchstone-model` defines typed Rust structs for every table MME reads, and a `World` that holds
them with lookups by record number, so the loaders (T-011, T-012) and ported formulas share one
model.

## Read
The converted stock SQLite from `tools/mdb-fetch.sh` (`.schema` and a few rows per table, via
python3 `sqlite3`). How MME reads the fields: `index.json` `tables` entries, and `grep` in
`<cache>/mme-index/<sha>/` for `tabItems!`, `tabMonsters!` etc. `docs/design.md` "Data".

## Do
1. One struct per table: Items, Monsters, Spells, Rooms, Shops, Classes, Races, Lairs, Info, TBInfo.
   Field names are the `.mdb` column names in snake_case. Types follow the SQLite column type and
   the range of values actually present (i16/i32/i64, f64, String, bool).
2. Numbered column series (for example `Abil-0` .. `Abil-19` with `AbilVal-0` ..) become fixed
   arrays or `Vec`s of small structs, named for what they are. Keep a mapping table, column by
   column, in `docs/model.md` so loaders are mechanical.
3. `World { info, items, monsters, ... }` with `Vec`s plus number -> index maps, and `get_item(n)`
   style accessors. Derive `Debug, Clone, Serialize` (serde) on everything.
4. Enums for well-known codes MME switches on (item type, weapon type, spell targets ...), only where
   MME's code names or comments them; keep the raw number available too.
5. No IO in this crate.

## Done when
Workspace checks pass. `docs/model.md` lists every column of every table exactly once (a
python3 check comparing the SQLite schema against the doc's table must print nothing missing).
