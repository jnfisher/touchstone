---
id: P4-05L
title: "Explorer layout: adaptive split and detail pane polish"
model: sonnet
phase: 4
depends: [P4-04]
touches:
  - web/src/
---

## Goal
The explorer's list and detail panes share the width sensibly: the list is as wide as its columns,
the detail pane gets the rest, and nothing in the detail pane wraps awkwardly. The page stays
usable on a phone.

## Read
`CLAUDE.md` (web conventions), `docs/design.md` ("Web UI"), `web/src/lib/components/` (SplitPane,
DetailPane, ExplorerView, ShopPrices, ExpTable) and `web/src/lib/grid/DataGrid.svelte`.

## Do
1. Split view: draggable divider, position remembered per tab in localStorage. Default list width
   is the grid's total column width (capped at 60% of the view), detail gets the rest. The detail
   pane collapses and expands (remembered per tab). Below 1100 px the detail stacks under the list.
2. Detail pane: tighter type scale; key/value grid whose labels never wrap; sticky header with the
   record name; inner tables (shop prices, exp table) keep numeric and price cells on one line and
   scroll horizontally when they do not fit.
3. At 390 px wide the tab strip scrolls horizontally and the list and detail are stacked.
4. Colours come from theme tokens only; check dark and light.
5. Screenshots at deviceScaleFactor 2 into `web/scripts/screenshots/` (git-ignored): Shops, Monsters
   and Races dark at 1600x1000, Shops light, Monsters at 390x844.

## Done when
`npm run check`, `npm test`, `npm run build` pass; `web/scripts/smoke.cjs` and `grid-check.cjs`
pass against a running server; the Shops price column shows "22,500 Copper (225 Gold)" on one line.
