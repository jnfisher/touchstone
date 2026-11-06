---
id: P4-08
title: Monsters explorer (rows, filters, detail pane)
model: sonnet
phase: 4
depends: [P3-27, P3-30, P3-32, P3-33, P3-34, P4-DP, P4-05, P4-21, P4-20]
touches:
  - bin/touchstone/src/evaluate.rs
  - bin/touchstone/src/panes.rs
  - bin/touchstone/tests/
  - crates/touchstone-rules/src/detail/mod.rs
  - crates/touchstone-rules/src/detail/monster.rs
  - web/src/lib/explorers.ts
  - web/src/lib/evaluate.ts
  - web/src/lib/detail.ts
  - web/src/lib/components/
  - web/src/lib/
  - docs/differences.md
  - tasks/P4-08-monsters-explorer.md
---

## Goal
The Monsters tab works as MME's does. The grid holds the rows `AddMonster2LV` produces, with MME's columns, computed
cells and fore colours, for the current character and exp/hr settings. The list is filtered by `FilterMonsters`
with MME's monster filter controls (regen, HP, exp, damage, magic, lair or party mode and the "Monster Filters" window's
extras). Clicking a row shows `PullMonsterDetail`: its rows, the damage-out text and the reference rows.

## Read
- `tasks/P3-27-monster-rows.md`, `tasks/P3-30-filters-spells-monsters.md`, `tasks/P3-32-monster-detail.md`,
  `tasks/P4-DP-detail-panes.md`.
- `crates/touchstone-rules/src/{filters/monsters.rs,rows/monster.rs,detail/monster.rs,lairs.rs}`,
  `bin/touchstone/src/{evaluate,panes}.rs`.
- `web/src/lib/components/{ExplorerView,DetailPane}.svelte`, `web/src/lib/{explorers,evaluate,detail}.ts`.

## Decisions
- One `DetailRow`: `detail::monster` keeps its own row type (it carries tag, colours, bold and sub-items, which the other
  panes' label/value rows cannot) under the names `MonsterRow`, `MonsterCell` and `MonsterRowKind`, with
  `DetailRow`/`DetailCell`/`DetailKind` kept there as aliases so the oracle tests are untouched. `panes.rs` maps a
  `MonsterRow` to the shared pane JSON (`rows` with label/value/kind, `references` for the "Spawns via" list).
- Wire (list): `options.monsters` holds every control of `MonsterFilter` / `MonsterExtras` plus the mode
  (`monster` | `lair` | `party`) and the lair/party boxes; the response gains `caption` ("Monsters" or "*Monsters*") and the
  column headers follow `MonsterList.headers`.
- Wire (pane): `GET/POST /api/detail/monsters/{n}`; the POST body is `DetailBody`, with `options.monsters` for the
  party boxes the damage text reads.
- Damage caches are rebuilt per request: the server has no "calculate monster damage" action, so nothing writes them and
  the rebuilt arrays give the text MME shows before a calculation. Lair averages (`possy`, `spawn_chance`) are the
  start-up pass `CalculateAverageLairs` makes, computed once per load.
- Reachability stays `AllReachable`; the filter's "reachable only" box is shown but changes nothing until the
  reachability module is ported.
- Web: the Monsters explorer is a computed tab like the others. The filter controls are a collapsible bar above the grid.
  The pane renders the server rows (grouped, with the row colours as theme classes) and links item, spell and monster
  rows to their records.

## Done when
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `tools/guard.sh`, web `npm ci && npm run check && npm test && npm run build`;
`oracle.py check` current for every family; endpoint tests pinned to `filters_sm.json` monster cases and
`monster_detail.json` pass; screenshots `monsters.png` and `monsters-filtered.png` in `/tmp/P4-08/shots/`.
