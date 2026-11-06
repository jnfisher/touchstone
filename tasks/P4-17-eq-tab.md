---
id: P4-17
title: EQ tab: equipment slots and stat totals
model: sonnet
phase: 4
depends: [P3-17, P3-18, P3-44, P4-20]
touches:
  - crates/touchstone-rules/src/equip.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/
  - fixtures/scenarios/equip.json
  - fixtures/rules/equip.json
  - tools/oracle/
  - bin/touchstone/src/
  - bin/touchstone/tests/
  - web/src/lib/
  - web/src/lib/components/character/
  - docs/differences.md
---

## Goal
MME's EQ tab rules in the app: equip and unequip items into the 20 slots with MME's slot mapping, the
paired finger and wrist logic, the lists each slot offers (all items, or only the ones the character can
use when the global filter is on), the warning flags, and the stat totals beside the slots. P4-20's
pickers filter by an inferred `worn` mapping; this replaces it with the real rules.

## Read
- `docs/porting.md`, `docs/testing.md`, `tools/oracle/README.md`, `tasks/P4-20-character-tab.md`,
  `crates/touchstone-rules/src/gear.rs` (module doc, `item_is_usable_by_char`), `bin/touchstone/src/character.rs`.
- MME through the index (frmMain): `InvenEquipItem`, `InvenAddEquip`, `InvenSetupEquip`, `InvenClear`,
  `cmbEquip_Click`, `LoadInvenItems`, `FilterInvenItems`, `cmdArmNextSlot_Click`, the `chkEquipHold`
  colouring in `CalcCharacterStats`, `ItemIsUsableByChar`; `cmdEquipButtons_Click` for the buttons.

## Decisions
- The brief says two-handed weapons clear the off-hand. MME does not: `InvenEquipItem` never touches
  the off-hand, and `CalcCharacterStats` only paints the weapon and off-hand boxes red when a
  two-handed weapon (type 1 or 3) is held with an off-hand item. Parity over correctness: the port
  keeps both items and reports the flag (`docs/differences.md` records the game behaviour).
- "Class/race usability" is `ItemIsUsableByChar` (class, level, alignment, minimum item level); MME
  has no race rule. The usable filter is the global filter (`FilterInvenItems`): armour and weapons
  only, in-game only, usable only. Without it `LoadInvenItems` lists every in-game item.
- MME's slot lists are the choices; `cmbEquip_Click` stores whatever list entry is picked. The port's
  `set_slot` accepts an item that belongs in that slot's list (`InvenAddEquip`'s mapping) and, when
  the filter is on, is usable; `equip_item` is `InvenEquipItem` (which also adds the item to its list
  when the filter had dropped it, so it never refuses an item the mapping allows).
- MME's combos are `Sorted`; the sort changes display order only (nothing in stock data sorts before
  "(none)"). The port returns the lists in table order and the web sorts them by name.
- `cmdArmNextSlot_Click` belongs to the Armour explorer's worn filter. It is ported as a pure
  function over "is the list empty for this index"; wiring it into the Armour explorer is out of scope.
- The hold checkboxes are UI state; `InvenClear` is ported as `clear_unheld` and not exposed yet.
- Oracle: a `$combo` control array (click event on `ListIndex` change, `Clear`, `UBound`) and a
  `%combo.` read; additive, other families unchanged.

## Do
- `touchstone-rules::equip`: slot mapping, list membership, `equip_item`, `set_slot`, `clear_all`,
  `clear_unheld`, `slot_choices`, `slot_flags`, `next_armour_worn`. Pure; messages are returned.
- Oracle family `equip`: scenarios + `tools/oracle/gen/equip.py`; Rust test over every case.
- Server: `POST /api/character/equip` `{character, item, slot?, unequip?}` -> `{character, messages, slot}`
  and per-slot lists for the picker (`GET /api/character/equip-choices`).
- Web: Equipment panel with the slot grid, per-slot search over the real lists, unequip, flags and the
  stat totals; screenshots dark and light at `/tmp/P4-17/shots/`.

## Done when
`cargo fmt/clippy/test`, `tools/guard.sh`, web `npm ci && npm run check && npm test && npm run build`,
`oracle.py check` current for every family, every `equip` fixture case passes, screenshots exist.
