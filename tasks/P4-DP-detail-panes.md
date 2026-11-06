---
id: P4-DP
title: Detail panes in the explorers
model: sonnet
phase: 4
depends: [P3-31, P3-35, P3-36, P3-37, P4-05, P4-20, P4-21]
touches:
  - bin/touchstone/src/
  - bin/touchstone/tests/
  - web/src/lib/components/DetailPane.svelte
  - web/src/lib/components/ExplorerView.svelte
  - web/src/lib/explorers.ts
  - web/src/lib/api.ts
  - web/src/lib/
  - docs/differences.md
  - tasks/P4-DP-detail-panes.md
---

## Goal
Each explorer's detail pane shows what MME shows when a row is clicked: `PullItemDetail` for
Weapons, Armour and Items (with the character when the global filter uses it), `PullSpellDetail`,
`PullShopDetail`, `PullClassDetail` and `PullRaceDetail`, with the "where is it found" reference
rows ordered as MME orders them, and a "Compare with equipped" toggle for weapons and armour.

## Read
`crates/touchstone-rules/src/detail/{item,spell,shop,class_race}.rs`, `locations.rs` (`sorts`),
`lairs.rs` (`LairCache`), `bin/touchstone/src/{detail,evaluate}.rs`, MME `SortListView`,
`LV_RefreshSort`, `LV_RefreshSort_ShopsFirst` (through the index), `web/src/lib/components/{DetailPane,ExplorerView}.svelte`.

## Decisions
- Wire: `GET /api/detail/{items|spells|shops|classes|races}/{n}` (no character, MME defaults) and
  `POST` with `{character?, global_filter?, options?, charm?, show_sell?, shops_first?,
  hide_record_numbers?, attack_type?, full_details?}`. `character`, `global_filter` and `options`
  mean what they do for `/api/evaluate`, and the same code resolves them (`evaluate::prepare`).
  Weapons, armour and sundry all use `items`.
- Response: `{kind, number, text, rows:[{label,value,kind}], references:{columns, rows, header?},
  compare:[{label,text}], errors}`. `text` is the exact text box. `rows` are the pieces; the
  "Compared to ..." pieces of an item move to `compare` (the toggle shows or hides them), so a pane
  without a compare toggle loses nothing. A reference row carries `kind`, the cells with their tags
  and, where MME's row stands for a record, `target:{tab, number}` for the link.
- The equipped items MME compares against are the character's `equipped` slots; they are read only
  when the character is in use (the pane then compares exactly as `PullItemDetail` does).
- Reference order: each `SortListView(lv, 1, string, True)` request of `GetLocations` sorts the
  rows held so far by their first cell, case-insensitively and stably (`LocationList::sorts`, in
  order); an item's final `LV_RefreshSort` / `LV_RefreshSort_ShopsFirst` sorts all rows by the
  first column's tag as a number, descending (shop rows first for the second). Rows with equal keys
  keep their order (MME's ListView leaves it undefined).
- `GetLairInfo` for the lair rows is answered from the lair cache as loaded (`LairQuery.startup`),
  so mobs, spawn percentage and average exp are MME's; the damage figures need a monster-tab
  calculation and read as zero (recorded in `docs/differences.md`). The cache is built once per load.
- Web: `DetailPane` renders server rows grouped by kind in theme tokens, the reference table with
  links (switch tab and select the record) and the compare toggle. `explorers.ts` drops the hand-built
  groups for items, spells, shops, classes and races; monsters and rooms keep theirs.
  `SpellEffect` (level picker) and `ExpTable` stay: nothing in `PullSpellDetail` replaces them.
  `ShopPrices` is replaced by the shop pane (charm and buy/sell controls feed the same request).

## Done when
`cargo fmt --all --check`, clippy, `cargo test --workspace --locked`, `tools/guard.sh`, web
`npm run check && npm test && npm run build`; endpoint tests pinned to `item_detail.json`,
`misc_detail.json` and `locations.json` case ids pass; screenshots `weapon-detail.png`,
`spell-detail.png`, `shop-detail.png` in `/tmp/P4-DP/shots/`.
