---
id: P4-01a
title: First computed values in the UI: shop prices and exp tables
model: sonnet
phase: 4
depends: [P4-00, P3-01, P3-05]
touches:
  - bin/touchstone/
  - web/
  - tasks/P4-01a-first-computed.md
---

## Goal
The ported formulas start showing in the UI through the server: the Shops tab lists a shop's stock
with MME's buy and sell prices at a chosen Charm, and the Races and Classes tabs show an exp table
(exp needed and training cost per level).

## Read
`docs/design.md` ("Server and API", "Web UI"); `touchstone-rules` `item_value` and `exp` public API
and their tests; MME `PullShopDetail`, `lvShops_ItemClick`, `txtShopCharm_Change`, `PutCommas`
(through the index).

## Do
1. `GET /api/shops/{n}/prices?charm=N`: per stocked item the buy and sell price from
   `get_item_value` (Stock, shop markup, charm default 0 as MME's box), the regen text and the charm
   labels; for a training shop (type 8) the level range with `calc_money_required_to_train`.
2. `GET /api/exp?race=R&class=C&from=1&to=255[&markup=M]`: exp needed per level
   (`calc_exp_needed_by_race_class`, shown as `PutCommas(.., True)`) and the training cost text.
3. Shops detail pane: Charm box with +/-, Show Buying / Show Selling, live prices. Races and
   Classes detail pane: exp table with a class (or race) dropdown and trainer markup.
4. Rust tests with `oneshot` over an invented World, pinned to fixture cases; vitest for the client.
5. `web/scripts/computed-shots.cjs` takes the two screenshots (git-ignored output).

## Done when
`cargo fmt/clippy/test`, `tools/guard.sh`, and in `web/` `npm run check`, `npm test`,
`npm run build` pass; against the stock database both endpoints answer and both tabs render.
