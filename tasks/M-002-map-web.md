---
id: M-002
title: Web map view (Rooms tab map and pop-out window)
model: sonnet
phase: 6
depends: [M-001, P4-13, P4-29]
touches:
  - web/src/lib/components/map/
  - web/src/lib/components/RoomsTab.svelte
  - web/src/routes/map/
  - web/src/routes/+layout.svelte
  - web/src/lib/map.ts
  - web/src/lib/map.test.ts
  - web/src/lib/settings.ts
  - web/src/lib/settings.svelte.ts
  - web/src/app.css
  - bin/touchstone/src/map.rs
  - bin/touchstone/src/settings.rs
  - bin/touchstone/src/state.rs
  - bin/touchstone/tests/all/
  - docs/differences.md
  - tasks/M-002-map-web.md
---

## Goal
The web face of MME's graphical map: the grid M-001 returns drawn as SVG on the Rooms tab and in a pop-out
window, with MME's walking, option, preset, legend and "what leads here" controls wired to the `/api/map*` endpoints.

## Read
- MME (through the index): frmMain `cmdDrawMap_Click`, `chkMapOptions_Click`, `cmdMapLeadsHere_Click`,
  `lblRoomCell_MouseDown`, `EditPreset`, `LoadPresets`, `cmdMapPreset_Click`, `cmdMapPresetSelect_Click`; frmMap
  `cmbMapSize_Click`, `optAlsoMark_Click`, `cmdMapShowUnused_Click`, `cmdMove_Click`, `Form_KeyPress`,
  `cmdResetPresets_Click`; the captions of frmMain, frmMap, frmMapLegend and frmEditPresets.
- `docs/differences.md` "Graphical map layout (M-001)"; `bin/touchstone/src/{map,settings}.rs`.

## Do (decisions)
- One component, `MapView.svelte`, with a `window` flag: the Rooms tab hosts it (main grid, frmMain captions
  "Not Hidden"), the `/map` route hosts it (all six frmMap sizes, "Don't ..." captions). The host owns the start room;
  the view asks `/api/map` for it and reports a new room through `onNavigate`.
- SVG, one `<rect>` per placed cell and one `<line>` per segment, coloured by CSS variables (`--map-*`) defined once
  for light and dark in `app.css`. Measured at 50x50 in Chromium (see the PR).
- Click a cell = select it in the Rooms explorer and re-centre (the card's reading of `MapGetRoomLoc` plus redraw).
  Right-click follows MME: up/down cells walk U/D, a cell with both asks which, shift+right-click re-centres on the
  cell (empty cells too) through `/api/map?center=`.
- Walking: ten compass buttons plus keyboard (arrows, numpad 1-9 and `.`, `<`/`>`) when the map has focus, through
  `/api/map/go` (`window` flag in the pop-out, which uses `cmdMove_Click`'s rules). A message result is shown as text.
- Options persist per browser in localStorage (try/catch), one key for the tab and one for the window. Presets persist in
  `settings.json` as `map_presets`; `/api/map/presets` merges them with `presets_with`.
- Pop-out is `window.open('/map?map=&room=&size=&options=')`; the root layout renders `/map` without the app chrome.

## Done when
Workspace and web checks pass; the four screenshots exist; every control above is wired or recorded in
`docs/differences.md` ("Web map view (M-002)").
