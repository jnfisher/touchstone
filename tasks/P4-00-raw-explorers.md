---
id: P4-00
title: Explorers with raw columns from the tables API
model: sonnet
phase: 4
depends: [T-013]
touches:
  - web/src/
---

## Goal
Every MME section tab shows its list from the server's tables API in a DataGrid, with the raw
database columns MME shows (no computed, character-dependent columns yet), and a detail pane
beside it with the selected record's fields. This is the visible skeleton the later P4 cards fill.

## Read
`docs/inventory.md` section 2 (UI map) and 3 (features: which columns each list shows), the
`AddWeapon2LV`/`AddArmour2LV`/`AddSpell2LV`/`AddMonster2LV`/`AddShop2LV` style row builders in the
index for column order and headers only, `bin/touchstone` for the API shapes, the existing web
shell (`web/src/lib/sections.ts`, `grid/DataGrid.svelte`).

## Do
1. A typed API client (`web/src/lib/api.ts`) with types matching the server's JSON, fetching each
   table once and caching it (revalidate by ETag).
2. Per section: which records belong (MME's item-type split into Weapons, Armour, Sundry), the raw
   columns in MME's order with MME's headers, number formatting, default sort.
3. Split view: list left, detail right (raw fields grouped sensibly), resizable, selection kept per
   tab. Keyboard: arrows move selection, F1 to F11 switch tabs.
4. A computed column MME shows but the API lacks yet is listed in a `pending` array per section,
   not faked.
5. Vitest for the section definitions and the API client (mocked fetch).

## Done when
`npm run check`, `npm test` and `npm run build` pass; with the stock database, `touchstone serve`
shows every tab populated (check by fetching `/api/*` and by a Playwright smoke script run locally
that loads each tab and asserts a non-empty grid; the script lives in `web/scripts/` and is not run
in CI).
