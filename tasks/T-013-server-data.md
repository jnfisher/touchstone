---
id: T-013
title: Serve the World over the API with live reload
model: sonnet
phase: 1
depends: [T-011, T-012]
touches:
  - bin/touchstone/
  - packaging/touchstone.example.toml
---

## Goal
`touchstone serve` loads a World from the configured source and serves the tables in
`docs/design.md` "Server and API"; with a silvermere source it reloads when the game's data
changes.

## Do
1. Config `[source]`: `kind = "mdb"` with `path` (a `.mdb`, converted on start through
   `tools/mdb2sqlite.py` logic reimplemented by invoking mdbtools, or an already converted `.sqlite`)
   or `kind = "silvermere"` with `path` to the game's database file. `packaging/touchstone.example.toml`
   documents every key.
2. `GET /api/info`, `GET /api/{items,monsters,spells,rooms,shops,classes,races,lairs}` as JSON,
   compressed (`tower-http` compression), with `ETag` from the data version so the browser
   revalidates cheaply.
3. World in `arc-swap`. For silvermere sources a task polls `data_version` every
   `reload_interval_secs` (default 10), rebuilds off the request path and swaps; a failed rebuild
   keeps the old World and logs. `/api/info` shows loaded-at and the version.
4. Tests with `oneshot` against a small invented World.

## Done when
Workspace checks pass; with the stock SQLite, `curl /api/monsters | head -c 300` returns JSON and
`/api/info` reports the counts.
