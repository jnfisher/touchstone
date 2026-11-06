---
id: P4-19
title: Item Manager
model: sonnet
phase: 4
depends: [P3-38, P3-39, P4-17, P4-24, P4-26]
touches:
  - crates/touchstone-rules/src/item_manager.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/
  - fixtures/scenarios/item_manager.json
  - fixtures/rules/item_manager.json
  - tools/oracle/
  - bin/touchstone/src/
  - bin/touchstone/tests/
  - web/src/lib/
  - web/src/lib/components/character/
  - docs/differences.md
  - docs/design.md
---

## Goal
MME's Item Manager (FEAT-EQ + FEAT-LISTS): the list of items the character carries or stashes, the
Action cells (drop/hide, pickup/use, sell/buy, carried, stash, -/+ quantity, clear), "delete selected",
"invert selection", "copy actions to clipboard", the value and best-shop columns. It is filled by Paste
Character's inventory parse and persisted with the character (`carried`/`stash`, `IM_CARRIED`/`IM_STASH`),
and CARRIED rows feed the gear pass (abilities and encumbrance) as they do in `CalcCharacterStats`.

## Read
- MME through the index: modListViewExt `LV_SetActionCell`, `LV_CopySelectedActionsToClipboard`,
  `LV_DeleteSelected`, `LV_InvertSelection`, `ParseActionAndQty`, `ParseActionBase`, `LV_SetCellText`;
  frmMain `LoadInvenItems`, `lvItemManager_ColumnClick/DblClick/ItemClick`, `cmdCompareNav_Click`
  (the button map), `SaveCharacter`/`LoadCharacter` (the Item Manager lists), `cmdEquipButtons_Click`.
- `crates/touchstone-rules/src/game_text/matching.rs` (`ListRow`, `parse_action_and_qty`,
  `lv_add_row_by_item_number`), `gear.rs` (`carried_items`), `bin/touchstone/src/paste.rs`, `web/src/lib/paste.ts`.

## Decisions
- `ParseActionAndQty` is already ported and pinned by `item_match.json`; `ParseActionBase` is its action
  half (private in MME, a one-line wrapper). `LV_SetCellText` is only used by the sort code (not ported).
  `LoadInvenItems` fills the EQ slot combos and was ported by P4-17.
- New oracle family `item_manager`: `LV_SetActionCell`, `LV_CopySelectedActionsToClipboard`,
  `LV_DeleteSelected` and `LV_InvertSelection` run unchanged over a mutable ListView shim that gained
  `Selected`, `ColumnHeaders.Count`, `EnsureVisible`, `SetFocus` (additive; other families unchanged).
- The two MsgBoxes (the encumbrance question of the CARRIED action, the multi-row delete confirmation)
  become callbacks in the Rust port and answered prompts on the wire, like the paste dialog.
- Selection is the page's state (row indexes); the server is stateless. `RefreshAll` and `bPromptSave`
  are the caller's: the response reports `carried_changed`/`stash_changed` and the new character.
- Saved lists follow `SaveCharacter`: rows whose flag contains CARRIED / STASH, quantity from a
  trailing ` xN`, at most 50 each. They are rebuilt from the rows after every change and after a paste.
- Sorting by column is the grid's (AG Grid), including MME's numeric sort of Num/QTY/Enc/Value.

## Do
- `touchstone_rules::item_manager`: `set_action_cell`, `copy_selected_actions`, `delete_selected`,
  `invert_selection`, `saved_stacks`, `gear_rows`.
- Server: `/api/item-manager/{rows,action,delete,copy,add}`; the character DTO's `carried` feeds
  `refresh_all` in every evaluation; a paste's response carries the new `carried`/`stash`.
- Web: Item Manager panel (shared AG Grid wrapper, multi-select) with MME's buttons, on the Character tab.
- Tests pinned to `fixtures/rules/item_manager.json`; endpoint tests; Vitest for the client.
- Screenshot `/tmp/P4-19/shots/item-manager.png`.

## Done when
Workspace and web checks pass, `oracle.py check item_manager` is current, screenshot exists.
