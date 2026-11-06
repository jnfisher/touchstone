---
id: P4-13
title: Rooms explorer (text) and room finder
model: sonnet
phase: 4
depends: [P3-06, P3-31, P4-DP]
touches:
  - crates/touchstone-rules/src/rooms.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/rooms.rs
  - crates/touchstone-rules/tests/all/main.rs
  - tools/oracle/gen/rooms.py
  - tools/oracle/shims.py
  - fixtures/scenarios/rooms.json
  - fixtures/rules/rooms.json
  - bin/touchstone/src/
  - bin/touchstone/tests/
  - web/src/
  - docs/differences.md
  - tasks/P4-13-rooms.md
---

## Goal
The Rooms tab (FEAT-ROOMS) works as MME's text half does: a map/room lookup that shows the room's details, a room
name finder (`cmdMapFindText_Click`), a "rooms with these exits" finder (`FindRoomWithDirections` with the room-find
half of frmPopUpOptions), the list of rooms behind a room-command row (`ShowRoomCmdRoomList`), and the Go Back history
of `cmdRoomsButtons_Click`. Reference rows elsewhere that point at a room (`GotoLocation`) open this tab on that room.
The graphical map (frmMap, the cell grid and its drawing) is out of scope.

## Read
- MME (through the index): frmMain `GotoLocation`, `FindRoomWithDirections`, `cmdRoomsButtons_Click`,
  `cmdMapFindText_Click`, `ShowRoomCmdRoomList`, `MapStartMapping`, `lblRoomCell_MouseDown`, `MapGetRoomLoc` (the room's
  reference list), `MapMapExits` (the room's tooltip text; its drawing and cell mapping are skipped); modMain
  `AddRoomNPCCommandRefs`; frmPopUpOptions `ResetRoomFind`, `cmdRoomFindDir_Click`, `optRoomFindMatch_Click`.
- `crates/touchstone-rules/src/{locations,textblock,lairs}.rs`, `detail/shop.rs` (`add_room_npc_command_refs`),
  `bin/touchstone/src/panes.rs`, `tools/oracle/gen/{misc_detail,find_best}.py`, `web/src/lib/components/ExplorerView.svelte`,
  `web/src/lib/detail.ts`.

## Decisions
- What a room shows. MME has no room "pane": the map's centre cell shows the title "Rooms -- Name (map/room)", the
  reference list `MapGetRoomLoc` fills (NPC, shop, placed items, room spell, teleports, commands, exit items/spells/
  commands, lair monsters) and the cell tooltip `MapMapExits` builds (name, also here, light, shop, items, spell, exits,
  remote actions, commands, regen, lair totals). The port returns all three: `room_title`, `room_refs`, `room_tooltip`.
- The oracle runs MME's own `MapGetRoomLoc`, `MapMapExits`, `cmdMapFindText_Click`, `FindRoomWithDirections` and
  `ShowRoomCmdRoomList` cut from frmMain.frm, with the dialogs replaced by reads of scenario controls (InputBox, the
  frmPopUpOptions buttons), `frmResults.lvResults` a recording ListView, and the drawing, tool-tip and
  cell-mapping calls blanked (`subs`). `GetLairInfo` is the lair shim; `GetPreCalculatedMonsterDamage` answers from
  the case (the server has no calculation, so it is 0 there).
- Rooms table. 26,694 rooms do not fit the fixture budget, so the oracle database holds a deterministic subset
  (`db_only` gains a `Rooms` form: a list of `[map, room]`): every room a case names, their exit targets, a sample of
  every map and the rooms for the finders. The rows sit once under `data`; the Rust test builds a `World` from them.
- Text find: MME steps (Find, then Find Next from the last hit). The port is `find_text(world, query, after)`; the
  oracle case runs Find and then Find Next up to N times and records each hit. The server and web list every hit up to a
  limit (default 100) by stepping the same function; that limit is the web's, not MME's (MME has none).
- Exit find: `find_exits` returns up to 101 rooms (MME checks `> 100` before adding) and says when it stopped.
- `GotoLocation` is already the `target` mapping in `panes.rs` (row prefix to tab and key); the Rooms tab adds the
  "room cmd" row (several rooms) as `POST /api/rooms/cmd-rooms`.
- Go Back (`cmdRoomsButtons_Click` 4) is client state (`nMapLastMap`, 20 entries) ported in `rooms.ts` with tests.
- Not ported: the map drawing buttons (0, 1, 2, 3, 5), and "Route to room" (6), which needs the reachability module.
- Wire: `GET /api/rooms/{map}/{room}` (+ `hide_numbers`, `hide_npc`, `hide_lairs`, `hide_cmds`, `all_exits`, `illu`),
  `GET /api/rooms/find-text?q=&after=&limit=`, `POST /api/rooms/find-exits`, `POST /api/rooms/cmd-rooms`.

## Done when
Workspace checks and web checks pass; `oracle.py check` current for every family; `rooms` family 30-50 cases, all
pass; endpoint tests pinned to `rooms.json` case ids; screenshot `/tmp/P4-13/shots/rooms.png`.
