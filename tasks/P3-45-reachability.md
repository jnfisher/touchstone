---
id: P3-45
title: Reachability (modReachability)
model: sonnet
phase: 3
depends: [P3-31, P3-30, P3-32]
touches:
  - crates/touchstone-rules/src/reachability.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/reachability.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/reachability.json
  - fixtures/rules/reachability.json
  - tools/oracle/gen/reachability.py
  - tools/oracle/harness.py
  - tools/oracle/shims.py
  - tools/oracle/vbi/
  - bin/touchstone/src/
  - web/src/
  - docs/differences.md
---

## Goal
Port `modReachability` (the graph build, the per-level reachability bands, gate tracing and the route
search) and `modMain` `AddMonsterReachRows` / `AddRouteRows` / `AddRouteAltRow`, so the Monsters list's
"Reachable only" filter and the monster pane's "Reachable" rows work once the user has run
Options -> Calculate Reachability.

## Read
- `modReachability`: every procedure except `ReachWriteDebugCSV` and the progress-bar plumbing.
- `modMain`: `AddMonsterReachRows`, `AddRouteRows`, `AddRouteAltRow`, `RouteCoinText`, `RouteKindText`.
- `crates/touchstone-rules/src/{locations,filters/monsters,detail/monster}.rs` (the `Reachability`
  and `ReachRows` traits this fills), `docs/porting.md`, `docs/testing.md`.

## Do
- `reachability::compute(world) -> Reachability` builds rooms, edges (exits, room commands and spells,
  NPC greets, item-gated teleports), random groups, level breakpoints and per-monster hits, keeping
  MME's traversal order (edge order, dictionary insertion order, heap tie-break).
- Queries: `is_monster_ok`, `monster_info` (gate tracing), `room_status`, `route`, `room_route`, and
  `monster_reach_rows` (the detail rows). `Reachability` implements the filter trait; a small adapter
  (carrying the character level, `bHideRecordNumbers` and the waypoint interval) implements `ReachRows`.
- The oracle gets the pieces `modReachability` needs (Collection, snapshot recordsets over SQL,
  `Dim x As New`). The `reachability` family computes once per case and runs many queries against it.
- Server: `POST /api/reachability/compute`, `GET /api/reachability/status`, result kept in its own
  `AppState` field; the Monsters filter and pane use it when computed. Web: Options -> Calculate
  Reachability.
- Decisions: GreaterMUD/Paramud are not ported (the stock database has none of their data); progress-bar
  and error-handler paths are dropped (a build failure cannot happen on loaded tables).

## Done when
Workspace and web checks pass, `oracle.py check reachability` is current with every case passing, and the
screenshot `/tmp/P3-45/shots/reachability.png` shows a monster pane with reach rows.
