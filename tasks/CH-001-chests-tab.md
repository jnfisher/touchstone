---
id: CH-001
title: Chests tab
model: sonnet
phase: 4
depends: [P4-32, T-016, P3-05, P3-31]
touches:
  - crates/touchstone-rules/src/chests.rs
  - crates/touchstone-rules/src/item_value.rs
  - crates/touchstone-rules/tests/all/
  - bin/touchstone/src/chests.rs
  - bin/touchstone/src/api.rs
  - bin/touchstone/src/lib.rs
  - bin/touchstone/tests/all/
  - web/src/
  - docs/differences.md
---

## Goal
A Chests tab next to Items: every chest item in the game (item type 8, MME's `ItemIsChest`), and for each the
items it can give with their drop chances and sell prices, the average sale value of one opening at charm 50, and
which monsters drop the chest. Today chest contents only show from the Items explorer (`cmdSundryChests_Click`,
`GetChestItems`, ported as `chests::chest_contents`).

## Read
- `chests.rs` (`chest_contents`, `gather`), `textblock.rs get_chest_items`, `item_value.rs get_item_value`,
  `locations.rs get_locations` (the Items pane's reference rows), `game_text` `extract_shops_from_obtained_from`.
- MME through the index: `modMMudDatabase ItemIsChest`, `GetItemValue`, `GetChestItems`, frmMain
  `cmdSundryChests_Click`.
- `docs/differences.md` (P4-32 section), `web/src/lib/components/RoomsTab.svelte` and `ExplorerView.svelte` for tab
  and grid style.

## Decisions
- Chances are `chest_contents`' (MME's, rounded to one decimal for display). The average uses the unrounded
  chance MME computes before it rounds, so a line's shown chance times its shown price can differ by a hair from
  the shown expected value. The average is not an MME number; docs/differences.md has the formula.
- Sell price: `get_item_value` with no shop and no buying (a shop's markup only moves the buy price) at the tab's
  charm. An item "can't be sold" when no shop stocks or takes it (no `Shop`, `Shop(sell)` or `Shop(nogen)` entry in
  its Obtained From) or its sell value is 0; it adds 0 to the average.
- "Dropped by" count: the `Monster #n` entries of the chest's Obtained From. The detail's droppers are the Items
  pane's reference rows (`get_locations` with a percent column), sorted by chance, so monsters, rooms, textblocks
  and NPCs all show.
- The tab has no F-key: the existing F1..F11 bindings keep their meaning.
- No oracle family: nothing here is an MME number. Tests use invented chests with hand-worked values and a few
  stock chests (worked by hand in the test comments).

## Do
- Rules: `chests::chest_summary`, `chests::chest_detail`; `item_value::friendly_copper` (price wording for an
  average).
- Server: `GET /api/chests?charm=50`, `GET /api/chests/{item}?charm=50`.
- Web: Chests tab (list, contents grid, "Dropped by" grid, charm box), store and tests.
- docs/differences.md: the average-sale formula and its assumptions.

## Done when
`cargo fmt`, `clippy -D warnings`, `cargo test --workspace`, `tools/guard.sh` and the web checks pass; the
screenshot `/tmp/CH-001/shots/chests.png` exists.
