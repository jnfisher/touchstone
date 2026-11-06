---
id: F-001
title: Follow-ups bundle (server and web)
model: sonnet
phase: 6
depends: [P4-13, P3-45, P4-30, P4-33, Q-007]
touches:
  - bin/touchstone/src/
  - bin/touchstone/tests/
  - web/src/
  - crates/touchstone-rules/src/pipeline.rs           # a parameter threaded through; no formula change
  - crates/touchstone-rules/src/monster_glue.rs       # cancel check between monsters
  - crates/touchstone-rules/src/textblock.rs          # `extract_value` made pub
  - docs/differences.md
---

## Goal
Five small follow-ups that merged cards left in their PR bodies and in `tasks/STATUS.md`: a route dialog
from the Rooms tab and the monster pane, record numbers hidden on the remaining paths, a Cancel for the
monster damage calculation, the Quicksilver stat range in the character editor, and the real F-key
mapping in the info popover.

## Read
- MME `frmMain` `ShowMonsterRoute`, `ShowRoomRoute`, `ShowRouteResults`, `RouteCaptionText`,
  `MonsterRouteRoomText`; `modMain` `FormatRouteLabel`, `RouteLabelNames`, `RouteLabelClassRace`
  (through the index).
- `crates/touchstone-rules/src/reachability.rs` (`route`, `room_route`, `room_status`, `monster_info`).
- `bin/touchstone/src/{reach,monster_damage,paste,item_manager,combat_setup}.rs`, `web/src/lib/components/{RoomsTab,DetailPane,MonsterStatus,InfoPopover}.svelte`.

## Do
1. Route: `POST /api/reachability/route` takes `{monster | map+room, alt, level, hide_record_numbers?}` and
   answers the dialog (caption, `Step`/`Location` rows, each with the room it points at) or MME's message box
   text. Row building follows `ShowRouteResults` (waypoint interval from settings). Decision: "from the
   selected start room" means the room shown on the Rooms tab is the destination, as in MME's
   `cmdRoomsButtons_Click` case 6; sources are the nearest points of interest, as `ReachRoomRoute` chooses.
   The dialog is the existing results dialog, extended with a route row kind that opens the room.
   A pane line tagged `reach_route` / `reach_route_alt` opens the route.
2. Hide record numbers: `PipelineOptions.hide_record_numbers` feeds the three pipeline contexts; the Item
   Manager, paste and combat-setup requests take an optional `hide_record_numbers` that defaults to the setting.
3. Cancel: `Running` holds a cancel flag; `POST /api/monster-damage/cancel` sets it; the loop in
   `calculate_monster_damage_vs_char_all_with` stops when its progress callback says so; a cancelled run
   stores nothing and answers `cancelled: true`. The Options status line shows a Cancel button.
4. Stat range: `GET /api/races/{number}/stat-caps` answers the soft cap and training maximum per stat
   (`null` for stock). The editor reads `/api/info`'s profile; under Quicksilver the inputs clamp to
   `train_max` and show the soft cap. Stock stays 0 to 999.
5. Keys: the info popover lists the real mapping (F1 Weapons ... F11 Compare) with MME's key for each tab.
   No change to the bindings.

## Done when
- Workspace and web checks pass; each item has a test.
- Screenshots `/tmp/F-001/shots/route.png` and `damage-cancel.png`.
