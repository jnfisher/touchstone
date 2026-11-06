---
id: M-001
title: Map layout engine
model: sonnet
phase: 6
depends: [P4-13, P3-45]
touches:
  - crates/touchstone-rules/src/map.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/map.rs
  - crates/touchstone-rules/tests/all/main.rs
  - tools/oracle/gen/map.py
  - tools/oracle/harness.py
  - tools/oracle/shims.py
  - tools/oracle/frm.py
  - fixtures/scenarios/map.json
  - fixtures/rules/map.json
  - bin/touchstone/src/
  - bin/touchstone/tests/
  - docs/differences.md
  - tasks/M-001-map-engine.md
---

## Goal
`map::layout(world, start, options)` lays rooms out on MME's cell grid exactly as `MapStartMapping`,
`MapActivateCell` and `MapMapExits` do (first-come placement, duplicate handling, overwrite passes),
and returns a `MapGrid`: the placed cells with their room, colour category and markers, the exit
segments, and what did not fit. `go_direction`, `leads_here`, `presets` and `legend` port the walking,
"what leads here", preset and colour-key code. The web view is M-002; this card ends at the JSON endpoints.

## Read
- MME (through the index): frmMain `MapStartMapping`, `MapActivateCell`, `MapDrawOnRoom`, `MapMapExits`,
  `MapGoDirection`, `cmdDrawMap_Click`, `cmdMapLeadsHere_Click`, `chkMapOptions_Click`, `optAlsoMark_Click`,
  `LoadPresets`, `cmdMapPreset_Click`, `cmdMapPresetSelect_Click`, `EditPreset`; frmMap `ResizeMap`,
  `cmbMapSize_Click`, `cmdMapShowUnused_Click`, `cmdMove_Click`, `MapStartMapping`, `MapActivateCell`,
  `MapDrawOnRoom`; `frmMapLegend` (designer block); modMMudDatabase `GetMonsterMapNumber`; `QBColorCode`.
- `crates/touchstone-rules/src/{rooms,textblock}.rs`, `tools/oracle/gen/rooms.py`, `tools/oracle/frm.py`.

## Decisions
- One engine, two surfaces. frmMain draws a fixed 30-column, 23-row grid (690 cells, stride 30, centre 345);
  frmMap's `cmbMapSize` picks 20x20, 30x30, 30x50, 50x50 (stride 50, cells numbered row*50+col) or the two
  zoom grids (stride 28, cells from 9001). The port is `MapSurface { Main, Size(0..5) }` driving a geometry
  (first cell, stride, row length, last cell, centre). The two forms differ only in geometry, in the colour
  an alignment exit gets (frmMain's table tests a number its own exit-type table never produces, so it draws
  such exits in the default colour) and in which option checkbox shows all exits in the tooltip; the options
  are one struct.
- What MME draws is recorded by the oracle as an ordered list of `MapDrawOnRoom` calls (cell, shape, size,
  QBColor index) plus each cell's final room, state and `BackColor`. `MapGrid.draws` is that list; the
  semantic view (`cells`, `segments`, `off_grid`) is derived from the same state by the same rules, in the
  port. Colours are categories (`CellColor`, `SegmentKind`, `Marker`); QBColor numbers stay in `draws`.
- Not ported: the MegaMUD "known rooms" star (`optAlsoMark(3)`, needs a MegaMUD rooms file the server does
  not read), tooltips (the room tooltip is `rooms::room_tooltip`; a cell only carries its key and whether it
  replaced another room), `DoEvents`/screen plumbing.
- Oracle: both forms' own procedures are cut from the .frm files (`frmMap` copies renamed with a prefix by a
  small `rename` option of `frm.py`), the drawing procedure and `picMap.Cls` are replaced by recorders, the
  grid arrays are read back. The Rooms table is the subset the cases reach (a pre-pass of the generator on the
  full table finds it).

## Done when
Workspace checks pass; `oracle.py check` is current for every family; the `map` family (30-50 cases) passes;
endpoint tests are pinned to `map.json` case ids.
