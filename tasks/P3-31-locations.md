---
id: P3-31
title: Reference rows (GetLocations)
model: sonnet
phase: 3
depends: [P3-06, P3-05, P3-16]
touches:
  - crates/touchstone-rules/src/locations.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/locations.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/locations.json
  - fixtures/rules/locations.json
  - tools/oracle/gen/locations.py
  - tools/oracle/harness.py
  - docs/differences.md
---

## Goal
Port the rest of FAM-19 (`docs/inventory.md` row P3-31): `GetLocations`, `AddRoomCmdLocationRow` and
`ExtractRoomActions`, the "where is this / obtained from" rows MME shows under items, monsters and
spells. The result is a list of structured rows (kind, record number, text cells, percent, tags) for
the detail panes (P3-32..37) and explorers (P4-08..P4-11).

## Read
- modMain: `GetLocations`, `AddRoomCmdLocationRow`, `ExtractRoomActions`.
- modMMudDatabase: `GetRoomName`, `GetItemName`, `GetMonsterName`, `GetSpellName`, `GetShopLocation`,
  `GetRoomRegen`, `ItemIsChest`, `GetShopMarkup`, `GetItemValue`, `GetItemShopRegenPCT`, `GetLairInfo`.
- modSyntaxsFunc `ExtractNumbersFromString`, `RegExpFindv2`; modMMudFunc `SplitRoomCmdSources`.
- `crates/touchstone-rules/src/{textblock,item_value,lairs}.rs`, `tools/oracle/gen/lairs.py` (lair shims).

## Decisions
- The port walks the same twelve keyword scans over the lower-cased text, in MME's order, and builds
  `LocationRow`s in the order MME adds ListView rows. Recursion (shop rooms, chest contents) shares one
  row list as MME shares one ListView.
- ListView sorting is a skipped feature (`SKIP-LV`): the grid sorts. The port returns, next to the
  rows, `sorts`: for each place MME calls `SortListView(lv, 1, string, ascending)`, the number of rows
  the list held then. The oracle binds `SortListView` to a recorder and compares those counts.
- Database reads go through the `LocationData` trait (item, shop, monster, spell and room rows);
  `World` implements it. `GetLairInfo` is a callout (`LocationEnv`), answered in the oracle by the lair
  shim from the scenario's `data.lairs` (as P3-15 does) and in Rust from the same table.
- Frontend state MME reads (`chkGlobalFilter`, charm tag, `bHideRecordNumbers`, `nGlobalAttackTypeMME`,
  `nNMRVer`) is the `LocationOptions` struct.
- The scenario uses the real stock database. Case strings are the stock Obtained From / Summoned By /
  Learned From / Assigned To texts of ~200 items, ~150 monsters and ~100 spells, plus hand-written
  strings for the syntaxes the stock data lacks. Each option combination (one column, two columns,
  percent column, header/footer, limit) gets cases.
- GreaterMUD/Paramud branches are not ported (`bGreaterMUD` is False).

## Done when
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `tools/guard.sh`; `python3 -I tools/oracle/oracle.py check`
current for every family; Python unit tests pass; the locations test runs every fixture case.
