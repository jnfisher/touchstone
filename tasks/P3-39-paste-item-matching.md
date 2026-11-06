---
id: P3-39
title: Parsed-item matching, best shop price, MegaMUD stat-line parser
model: sonnet
phase: 3
depends: [P3-38, P3-05, P3-18]
touches:
  - crates/touchstone-rules/src/game_text/
  - crates/touchstone-rules/src/game_text.rs
  - crates/touchstone-rules/src/mega_stat.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/
  - tools/oracle/gen/mega_stat.py
  - tools/oracle/gen/item_match.py
  - tools/oracle/harness.py
  - tools/oracle/shims.py
  - fixtures/scenarios/mega_stat.json
  - fixtures/scenarios/item_match.json
  - fixtures/rules/mega_stat.json
  - fixtures/rules/item_match.json
---

## Goal
Port what P3-38 left of FAM-22: the MegaMUD stat-line parser of frmSwingCalc, and the half of
modItemParse that turns parsed names into Items matches, the best shop (price, name, value text) for
each, and the Item Manager rows (Obtained From text, shops, Usable). Two oracle families:
`mega_stat` (pure text) and `item_match` (stock database).

## Read
- `docs/inventory.md` sections 4a and 8 (FAM-22), `docs/porting.md`, `docs/testing.md`.
- Through the index: frmSwingCalc ParseMegaStatLine, GetMegaStatLine, FirstMegaToken, LastMegaToken,
  IsMegaPlaceholder, IsMegaWholeNumberToken, CleanMegaNumber, cmdPasteMega_Click; modItemParse
  GetItemsByExactNameArr, ExtractShopsFromObtainedFrom, ExtractFirstNumber, EvaluateBestPriceForHit,
  GetBestShopNumForItem, AddOneRow, AddListViewRowsForItem, AddSectionItems,
  SumInvOrCarriedQtyForName, PopulateItemManagerFromParsed, LV_AddRowByItemNumber, NzStr, NzLong;
  modMMudDatabase GetShopRoomNames, GetRoomName; modListViewExt ParseActionAndQty; frmMain
  ItemIsUsableByChar (ported in P3-18).

## Decisions
- P3-38 already ported the text half (the collectors, tokenisers, ConsolidateList, ParseNameAndQty,
  ItemNameInParseResult and the small array helpers; AccumulateMax/Sum, FindKeyIndex, IndexOf live
  inside its consolidation). NzStr and NzLong (NULL handling) have no Rust counterpart: the model
  has no NULLs. IsArrayInitialized and HasAnyContent reduce to `safe_array_has_data`.
- New code: `src/mega_stat.rs` (stat-line parser, module line in lib.rs) and
  `src/game_text/matching.rs` (re-exported from `game_text`).
- The Item Manager ListView becomes `Vec<ItemManagerRow>` (the eleven visible columns plus the Value
  and Worn sort tags). The hidden row-sequence column (`LV_AssignRowSeqIfMissing`) is UI plumbing and
  is a no-op in the oracle and absent in Rust.
- MsgBox prompts in PopulateItemManagerFromParsed become parameters (`import_equipped`,
  `import_keys`, `clear_non_flagged`); the oracle's MsgBox returns the default button, so the
  fixtures use the defaults (Yes, Yes, No) and the port is also exercised with Yes for the clear.
- GetShopRoomNames / GetRoomName are DB-ACC procedures nobody has ported; EvaluateBestPriceForHit
  needs them for the shop name, so they are ported privately in `matching.rs`.
- Character state for `Usable` goes through `gear::item_is_usable_by_char` (`Character`, equipped
  slots); the charm comes from `txtCharStats(5).Tag`.
- Oracle changes are limited to a mutable ListView shim and a `%lv.` read expression; every other
  family must stay byte-identical.

## Done when
`docs/porting.md` "Done when"; `oracle.py check` current for all families; 300-800 cases.
