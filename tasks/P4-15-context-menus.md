---
id: P4-15
title: Context menus and clipboard actions
model: sonnet
phase: 4
depends: [P4-14, P4-08, P4-17, P4-19, P4-22, P4-27a, P4-27b, P4-28a]
touches:
  - crates/touchstone-rules/src/clipboard.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/
  - tools/oracle/
  - fixtures/scenarios/clipboard.json
  - fixtures/rules/clipboard.json
  - bin/touchstone/src/
  - bin/touchstone/tests/
  - web/src/lib/
  - web/src/lib/components/
  - docs/design.md
---

## Goal
MME's right-click menus (FEAT-CTX), built by `PopUpAuxMenu`, `PopUpItemsMenu` and `PopUpSpellsMenu` and dispatched by
`mnuAuxPopUpItem_Click`, `mnuItemsPopUpItem_Click`, `mnuSpellsPopUpItem_Click` and `mnuEquipGotoItem_Click`, as one
`RowContextMenu` on every list that has one, plus the clipboard texts those menus (and the Char tab) copy:
`CopyChartoClip`, `CopyMonsterToClipboard`, `CopyShopToClipboard` and `CopyLVLinetoClipboard`.
Merges the inventory's P4-15 and P4-16 rows (the map menu has no map to attach to yet).

## Read
- MME through the index: frmMain `PopUpAuxMenu`, `PopUpItemsMenu`, `PopUpSpellsMenu`, `PopUpMapMenu`,
  `mnuAuxPopUpItem_Click`, `mnuItemsPopUpItem_Click`, `mnuSpellsPopUpItem_Click`, `mnuEquipGotoItem_Click`,
  `CopyChartoClip`, `InvenCopytoClipboard`, `CopyMonsterToClipboard`, `CopyShopToClipboard`,
  `PopUpChooseCombatGUI`; modMain `CopyLVLinetoClipboard`, `GetEquipCaption`; frmPopUpOptions `SetupChooseAttack`.
- `crates/touchstone-rules/src/detail/shop.rs`, `pipeline.rs`, `char_panel.rs`; `bin/touchstone/src/panes.rs`,
  `character.rs`; `web/src/lib/components/ExplorerView.svelte`, `DetailPane.svelte`, `grid/DataGrid.svelte`.

## Do
Decisions:
- The text builders are ported over plain inputs (strings the form's controls would hold), because MME builds them from
  control state: `char_to_clip` (which also appends `InvenCopytoClipboard`'s equipment block, as MME does), `monster_to_clip`,
  `shop_to_clip`, `lines_to_clip`. A VB6 error (a negative `String()` count when a name is too long, a missing sub-item)
  is `Clip::error`; MME's handler shows a dialog and the clipboard keeps what an earlier step set. The server answers 200 `{text, error}`.
- Oracle family `clipboard` runs the four procedures unchanged over a clipboard-recording ListView shim (additive;
  `SetClipboardText` now records, so `Clipboard.GetText` in the append works). Per-row detail text and references of
  `CopyLVLinetoClipboard` (it clicks each row first) are one shared pane in the oracle; the Rust function takes them per row.
- Endpoints `POST /api/clipboard/{character,monster,shop,lines}` answer `{text}`; `null` means MME copies nothing.
  The shop builder runs `pull_shop_detail` itself; the monster and lines builders take what the page shows.
- Menu logic is pure TypeScript (`contextmenu.ts`: which items, which captions, which are disabled, per list and record)
  and tested; `RowContextMenu.svelte` renders it with shadcn ContextMenu. Captions and order are MME's design-time ones.
- An item is disabled, with a tooltip, only where the feature is not in touchstone yet: What casts / counters
  (Results window), Where is this monster summoned, Copy Chest, Set Row(s) to This Shop, the map menu.
- The clipboard uses `navigator.clipboard`; without a secure context a selectable dialog shows the text.
- Calculators and the combat dialog open prefilled through a small hand-off store read once on mount.

## Done when
Workspace and web checks pass; `oracle.py check clipboard` current with every case passing; screenshot
`/tmp/P4-15/shots/context-menu.png` (stock db, level-50 warrior, dark, 1440x900 at 2x) shows a weapon row's menu.
