---
id: P4-18
title: EQ tab: Find Best and Next Best
model: sonnet
phase: 4
depends: [P4-17, P4-20, P3-27, P3-28]
touches:
  - crates/touchstone-rules/src/find_best.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/
  - fixtures/scenarios/find_best.json
  - fixtures/rules/find_best.json
  - tools/oracle/
  - bin/touchstone/src/
  - bin/touchstone/tests/
  - web/src/lib/
  - web/src/lib/components/character/
  - docs/differences.md
---

## Goal
MME's Find Best on the EQ tab: pick a criterion from the Find Best menu (armour, attacking, mystic
attacks, attributes, resistances, stats), search the items each slot's list offers, pick the best one per
slot (two-handers against the off-hand, paired fingers and wrists, unique items, held slots), and
select the winners. "Next Best" repeats the last criterion one step down, excluding what is worn.

## Read
- `docs/porting.md`, `docs/testing.md`, `tasks/P4-17-eq-tab.md`, `crates/touchstone-rules/src/equip.rs`,
  `bin/touchstone/src/equip.rs`, `web/src/lib/components/character/EquipPanel.svelte`.
- MME through the index (frmMain): `InvenFindBest`, `InvenFindBestEval`, `InvenFindBestAbils`,
  `InvenFindBestDupeOK`, `InvenFindBestItemValue`, `InvenFindBestSelectItem`, `InvenFindBestExclude`,
  `InvenFindBestIsExcluded`, the six `mnuFindBest*_Click` handlers, `cmdEquipButtons_Click` (cases 11 and
  12) and `cmbEquip_Click`; `_declarations.vb` for `enmFindBest`, `TypeEquipWinner`, `TypeFindBestItem`;
  the menu captions from the frmMain designer block; `modMain Get_Enc_Ratio`.

## Decisions
- MME's "last find best" values are a local array, recomputed from the worn items on every Next Best. What
  survives between clicks is `nInvenLastIndex` (index and menu) and `nInvenExcludedItems`. The request
  carries those two, the result returns them updated, and also returns the per-slot values it used.
- `InvenFindBest` drives the combos, so the port mutates the worn slots as it goes: the paired-slot checks
  and the two-hander recheck read slots earlier in the same pass.
- The combos are `Sorted`, so a tie (same value, same encumbrance ratio) goes to the first entry by display
  text. The rules take the lists in the order given; the server orders them by display text, an approximation
  of the Windows sort recorded in `docs/differences.md`. The oracle's lists are in table order, so the fixture
  pins "first in the given list wins".
- Abilities of 1000 and up (VileWard) exist only on GreaterMUD/Paramud: on the stock profile the menu entry
  is hidden and `InvenFindBest` leaves quietly, after recording the last index. The port does the same.
- Next Best before any Find Best finds `nInvenExcludedItems` unallocated: VB error 9, handled, nothing
  changes. The request carries `excluded: Option<..>` for that; `None` is "unallocated".
- Oracle cases run MME's own procedures through the menu handlers and `cmdEquipButtons_Click(11/12)`
  (PopupMenu and screen calls blanked). Lists come from the fixture's `lists` read.
- The Find Best / Next Best button and menu are in the web EquipPanel, with the same captions and order as
  the designer block (VileWard shown only on a GreaterMUD profile, so never today). Exclude toggles on
  slot items edit the exclusion list kept in the character store.

## Do
- `touchstone-rules::find_best`: `find_best`, `next_best`, `criteria()` (menu groups for the UI).
- Oracle family `find_best`: scenario generator, fixture, Rust test over every case.
- Server: `POST /api/character/find-best`.
- Web: menu, Next Best, exclude toggle, helpers and tests; screenshot at `/tmp/P4-18/shots/find-best.png`.

## Done when
`cargo fmt/clippy/test`, `tools/guard.sh`, web `npm ci && npm run check && npm test && npm run build`,
`oracle.py check` current for every family, every `find_best` case passes, screenshot exists.
