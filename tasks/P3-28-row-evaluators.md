---
id: P3-28
title: Row evaluators (weapons, armour, spells, sundry, shops, classes, races)
model: sonnet
phase: 3
depends: [P3-09, P3-10, P3-05, P3-11, P3-04, P3-44]
touches:
  - crates/touchstone-rules/src/rows.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/rows.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/rows.json
  - fixtures/rules/rows.json
  - fixtures/known_gaps.json
  - tools/oracle/gen/rows.py
  - tools/oracle/
---

## Goal
Port FAM-21's list-row builders except monsters: SetArmourACDRSortTags, ArmourACDRTagsAreByDR,
AddArmour2LV, AddOtherItem2LV, AddWeapon2LV, AddSpell2LV, AddRace2LV, AddShop2LV, AddClass2LV and
RaceColorCode. Each builds one ListView row (the item text, its tag and every sub-item with its text,
tag and colour) for one record, given the current character. The weapon row calls `CalculateAttack`,
the spell row `CalculateSpellCast`.

## Read
- `docs/porting.md`, `docs/inventory.md` row P3-28 and FAM-21, `docs/testing.md`, `tools/oracle/README.md`.
- MME through the index: modMain `AddWeapon2LV`, `AddArmour2LV`, `AddOtherItem2LV`, `AddSpell2LV`,
  `AddRace2LV`, `AddShop2LV`, `AddClass2LV`, `RaceColorCode`, `SetArmourACDRSortTags`,
  `ArmourACDRTagsAreByDR`, `Get_Enc_Ratio`; modSyntaxsFunc `RoundUp`, `Truncate`; modMMudFunc the
  `Get*Enum` text helpers; frmMain `FilterWeapons`, `FilterSpells` (how the callers build the profile).
- `crates/touchstone-rules/src/pipeline.rs` (doc comment first) for the character inputs.

## Decisions
- A row is `Row { text, tag, cells }`; a `Cell` is `{ text, tag, fore_color }` (`ListSubItems(i)` is
  `cells[i - 1]`). Texts are what VB6 stores in the String property (`CStr` of the assigned value), tags
  are `CStr` of the Variant assigned, `fore_color` is set only where MME assigns one (race colours).
  A builder returns `None` where MME skips the record (empty name, `sdf...`) and the row built so far
  where a VB6 error stops it, as MME's `Resume out` does.
- The weapon and spell rows take the profile `PopulateCharacterProfile` made (`attack::CharacterProfile`
  and `SpellCasterProfile`) plus a `RowEnv` (world, equipped globals, caption, `gAvgLevelMaxAllStats`,
  accuracy tag, global level text) and an options struct that stands for the `frmMain` controls the
  procedures read (`chkWeaponOptions(3)`, `(4)`, `txtWeaponExtras(2..4)`, `chkGlobalFilter`,
  `txtGlobalLevel(1)`, `chkSpellOptions(0)`, `(2)`, `txtSpellOptions(0)`).
- "No character" is `PopulateCharacterProfile` with the global filter off; `rows::no_character_profile`
  builds that profile. The default view is that case.
- `addbless` adds an entry to `cmbCharBless`; the spell row returns it as `bless_entry`.
- Oracle: a recording ListView shim, `AddItem` on controls, and harness steps (`seek`, `each`, saved
  Type arguments, a control as an argument, `%lv.` reads). Characters come from the pipeline scenario's
  cases (a warrior, a mage and a mystic), RefreshAll runs once per case and the rows of many records
  are built inside that case.
- Sample: every record with no character (weapons, armour, sundry, shops, classes, races; spells are a
  stride sample to stay under 1.5 MB), and for each of the three characters about 150 weapons in several
  attack modes and about 200 spells (armour, sundry, shops, classes and races do not read the character).

## Done when
`cargo test --workspace` passes, `oracle.py check` reports every family current, the Python unit tests
pass, and the rows test runs every fixture case and every cell.
