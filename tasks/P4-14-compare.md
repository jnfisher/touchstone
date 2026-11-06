---
id: P4-14
title: Compare tab (weapons, armour, spells, monsters)
model: sonnet
phase: 4
depends: [P3-27, P3-28, P3-35, P3-36, P3-37, P4-05, P4-08, P4-DP, P4-24]
touches:
  - bin/touchstone/src/evaluate.rs
  - bin/touchstone/src/compare.rs
  - bin/touchstone/src/api.rs
  - bin/touchstone/src/lib.rs
  - bin/touchstone/tests/
  - web/src/lib/
  - web/src/lib/components/
  - web/src/routes/
  - docs/design.md
  - tasks/P4-14-compare.md
---

## Goal
MME's Compare tab works: four lists (weapons, armour, spells, monsters), each with its explorer's computed columns, a
detail pane and the reference rows. Items get there through "Add to compare" on explorer rows, the equipment tab's "add
all equipped" and the shop's "compare" button. Remove, Clear and the nav buttons manage the lists, and the lists are
saved with the character in its `.mmec` `[Compare]` section.

## Read
- MME through the index (frmMain): `CompareAddItem`, `CompareAddMonster`, `CompareAddShopItems`, `ReloadMonsterCompare`,
  `cmdCompareNav_Click`, `cmdCompareClear_Click`, `RemovePopupItemCompare`, `RemovePopUpSpellCompare`,
  `RemovePopUpMonsterCompare`, `lv*Compare_ItemClick`, `cmdEquipButtons_Click` (case 3), and the `[Compare]` part of
  `SaveCharacter` / `LoadCharacter`.
- `bin/touchstone/src/{evaluate,panes}.rs`, `web/src/lib/components/{ExplorerView,DetailPane}.svelte`,
  `web/src/lib/{character,character.svelte,explorers,evaluate,detail}.ts`, `crates/touchstone-rules/src/mmec.rs`.

## Decisions
- A compare list is the explorer's row for each record, with no filter: `AddWeapon2LV`, `AddArmour2LV`, `AddSpell2LV`
  and `AddMonster2LV` are called straight from the record. `options.numbers` on `POST /api/evaluate/{tab}` lists exactly
  those records, in that order, repeats included, skipping records that are gone (`tabItems.NoMatch` on load).
  Weapons use the profile `PopulateCharacterProfile(tChar, False, True)` gives (the loaded character when the global
  filter is on); monster rows are made with an empty `tChar` and `is_main_list` off, so lair mode never applies.
- No compare-only arithmetic exists in MME: `CompareAddItem` and friends are list bookkeeping around those row procedures
  (the "Compared to ..." text of the detail pane is `CompareArmor`/`CompareWeapons`, ported and pinned by P3-36). So no
  oracle family is added; the row values are pinned to `rows.json` (weapons, armour, spells) through `numbers`.
- The bookkeeping is a pure TypeScript module (`compare.ts`): add toggles (an item already on the list is removed, as
  `CompareAddItem` calls `RemovePopupItemCompare`), the list is chosen by `ItemType` (armour 0, weapon 1, others
  ignored), `CompareAddShopItems` appends without the toggle and takes armour only when `Worn` is set, the list holds at
  most 101 numbers (the loop in `SaveCharacter`), and a number is a positive record. The item type comes from
  `POST /api/compare/classify` (the one question that needs the item table).
- Persistence: the four lists live in the character's `extra` as the `[Compare]` keys `WeaponCompare`, `ArmourCompare`,
  `SpellCompare`, `MonsterCompare` (comma-terminated text, `1,2,3,`), which `mmec` already writes back in
  `SaveCharacter`'s place, so autosave, export and import keep them. Helpers read and write those entries.
- "Add to compare" is a row action in the explorers (a button in the pane header for the selected row). Context menus are
  P4-15/16.

## Done when
Workspace and web checks pass; endpoint tests pinned to `rows.json` through `numbers` pass; web tests cover the list
bookkeeping and the round trip of the lists through the character document and `.mmec` export/import; screenshot
`/tmp/P4-14/shots/compare.png`.
