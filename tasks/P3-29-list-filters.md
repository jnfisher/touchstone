---
id: P3-29
title: List filters (weapons, armour, sundry, inventory, global)
model: sonnet
phase: 3
depends: [P3-28, P3-11, P3-03]
touches:
  - crates/touchstone-rules/src/filters.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/src/rows.rs
  - crates/touchstone-rules/tests/all/filters.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/filters.json
  - fixtures/rules/filters.json
  - fixtures/known_gaps.json
  - tools/oracle/gen/filters.py
  - tools/oracle/
  - bin/touchstone/src/evaluate.rs
  - docs/differences.md
---

## Goal
Port FAM-23's FilterWeapons, FilterArmour, FilterSundry, FilterInvenItems and FilterAll: which records each
explorer lists, in what order MME adds them, and with which `Add*2LV` arguments, given the filter controls
(a `ListFilter` struct with MME's defaults) and the global filter (class, race, level, alignment, minimum item
level, use-character: `ItemIsUsableByChar`). The fixtures pin the row set, not the UI. The P4-05 evaluate
endpoint switches from its simplified filters to these.

## Read
- `docs/porting.md`, `docs/inventory.md` row P3-29 and FAM-23, `docs/testing.md`, `tools/oracle/README.md`.
- MME through the index: frmMain `FilterWeapons`, `FilterArmour`, `FilterSundry`, `FilterInvenItems`, `FilterAll`,
  `ResetFilterOptions`, `ItemIsUsableByChar`, `InvenAddEquip`, `InvenSetupEquip`, `LoadInvenItems`;
  modMain `AddWeapon2LV`, `AddArmour2LV`, `AddOtherItem2LV`; modMMudDatabase `GetClassStealth`, `GetRaceStealth`.
- `crates/touchstone-rules/src/rows.rs`, `gear.rs` (`item_is_usable_by_char`), `pipeline.rs`.

## Decisions
- The output of a filter is the ordered list of `Add*2LV` calls the procedure makes (record number, ability,
  attack kind, which profile the call used) plus the rows FilterWeapons builds itself (the martial-arts rows),
  not UI state. Row order is the order MME adds rows; the sort that follows is the grid's job.
- MsgBox answers in FilterWeapons (martial arts without skill, maximum effort, backstab without stealth) become
  input fields; a cancelled dialog leaves the list as it was (`cancelled`).
- FilterAll is `ResetFilterOptions` (every control back to its default) followed by the lists: `filter_all`
  returns weapons, armour and the inventory combos. FilterSpells (P3-30) and RefreshAll are outside it;
  the oracle stubs them. The cmbGlobalClass_Click, chkGlobalFilter_Click and SetupClass UI refreshes are not
  list semantics.
- Oracle: frmMain's Filter* procedures run as cut out by `frm.py`, into recording ListViews (`lvWeapons`,
  `lvArmour`, `lvOtherItems`) and recording combos (`cmbEquip`); `LV_RefreshSort` and the `*_ItemClick`
  handlers are stubbed. Characters are the pipeline scenario's warrior, mage and mystic.
- Fixture rows are `[record number, tag]` per added row (full rows only in a few small cases that also prove
  `add_weapon_row` agrees when fed the filter's arguments).

## Done when
`cargo fmt/clippy/test` pass, `oracle.py check` reports every family current, the Python unit tests pass,
and the filters test runs every fixture case. `bin/touchstone` evaluate tests still pass.
