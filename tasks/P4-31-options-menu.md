---
id: P4-31
title: Options menu and shell actions
model: sonnet
phase: 4
depends: [P4-29, P4-30, P3-45, P4-20, P4-24, P4-21]
touches:
  - web/
  - bin/touchstone/src/settings.rs
  - bin/touchstone/tests/all/settings.rs
  - docs/differences.md
---

## Goal
One app-level Options menu in the header, with MME's captions and order (`mnuOptionsItems_Click` and the two
checked items under it), replacing the Monsters-toolbar damage menu and the reachability button. Plus the shell bits
a web app should have: back/forward navigation history between tabs and records, and the data-version info.

## Read
- frmMain `mnuOptionsItems_Click`, `RemoveFilters`, `FilterAll`, `ClearLearnedSpells`, `NavHistory*`, `LoadInfo`,
  `mnuJumpToCompare_Click`, `mnuShopsFirst_Click`, `LoadSettings`/`SaveSettings` (through the index).
- `docs/inventory.md` section 2 (Options menu), `bin/touchstone/src/settings.rs`, `web/src/lib/settings*.ts`.

## Do
- Header `Options` dropdown: Paste Character, Calculate Reachability, Calc. Monster Dmg vs Char / vs Party, Clear
  Calculated Monster Dmg, Clear Learned Spells, Reset All Character Fields, Remove All Filters, Settings, then the
  two checkable items "Jump to Equip/Lists on Add" and "Show shops first in item references". The NMR import/export
  and Reload items are not ported (the web app has its own character import and no reload).
- The two toggles are new fields of `settings.json` (`jump_to_compare`, `shops_first`, both default off as
  `LoadSettings` reads a missing key). Jump navigates to the Compare tab after an add; shops-first is sent as
  `shops_first` in the item pane request.
- Confirmations as in MME: "Are you sure?" (default No) for Clear Learned Spells and Reset All Character Fields, and
  the "Clear saved ... lists?" question when a compare list has entries. The "unload character file first" question
  is dropped (see differences.md).
- Remove All Filters resets the global filter, the monster boxes and, through a shared epoch, every explorer's
  search box and grid column filters.
- The Monsters tab keeps status chips ("Damage calculated vs ...", "Reachability computed"); the actions move.
- Navigation history: a linear history of tab plus selected record, toolbar back/forward buttons, Alt+Left and
  Alt+Right (only intercepted when there is somewhere to go), capped at MME's 50 entries; moves replace the
  browser history entry instead of adding to it.
- Info popover in the header: data version, creation date, NMR version, custom label and row counts from `/api/info`.

## Done when
- `cargo fmt/clippy/test`, `tools/guard.sh`, and `web` `npm run check && npm test && npm run build` pass.
- `/tmp/P4-31/shots/options-menu.png` shows the open menu (stock db, 1440x900, dsf 2, dark).
