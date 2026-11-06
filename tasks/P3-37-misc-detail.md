---
id: P3-37
title: Spell, shop, class, race and NPC-command detail
model: sonnet
phase: 3
depends: [P3-04, P3-10, P3-05, P3-31]
touches:
  - crates/touchstone-rules/src/detail/mod.rs
  - crates/touchstone-rules/src/detail/spell.rs
  - crates/touchstone-rules/src/detail/shop.rs
  - crates/touchstone-rules/src/detail/class_race.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/misc_detail.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/misc_detail.json
  - fixtures/rules/misc_detail.json
  - fixtures/known_gaps.json
  - tools/oracle/gen/misc_detail.py
  - tools/oracle/harness.py
  - tools/oracle/shims.py
  - tools/oracle/README.md
  - docs/differences.md
---

## Goal
Port the rest of FAM-20 that `docs/inventory.md` row P3-37 lists: modMain `PullClassDetail`,
`PullRaceDetail`, `AddRoomNPCCommandRefs`, `PullShopDetail` and `PullSpellDetail`, the detail panes of the
spell, shop, class and race explorers and the NPC command references of the map list. Each returns what MME
writes into its detail controls (the text box text, the ListView rows, the charm label) as structured rows
(label, value, kind) next to the exact text.

## Read
- modMain: the five procedures above; callees `GetLocations` (P3-31), `PullSpellEQ`, `GetAbilityStats`,
  `CalculateSpellCast`, `SpellIsUsable`, `GetItemValue`, `CalcMoneyRequiredToTrain`, `GetTextblockTeleport`,
  `GetTextblockLinkTo`, `GetRoomName`, `GetClassName`, `GetItemName`, `GetSpellTargetsEnum`,
  `SpellAttackTypeEnum`, `StringOfNumbersToArray`.
- `crates/touchstone-rules/src/{locations,spells,spell_cast,item_value,ability_text,textblock,pipeline,exp}.rs`,
  `tools/oracle/gen/{locations,pipeline}.py`.

## Decisions
- The procedures read `frmMain` controls and module globals; the rules take them as small option structs
  (`SpellDetailOptions`, `ShopDetailOptions`). A loaded character is a `pipeline::CharacterState` (its
  profile supplies level, spell bonus, spellcasting, mana, overhead and regen, and the bless/heal numbers
  the OOM line prints); without one the "no character" profile is used, as MME does when the global filter
  is off or the spell is not usable by the class.
- ListView column headers and the shop-type check boxes' `Enabled` flags are display and are not ported; the
  charm label caption is (it is text MME computes).
- `GetLocations` is called through `locations::get_locations`; the rows it adds are returned in the result.
- Oracle: one additive harness feature, a step `"capture"` (control reads taken right after a call), so one
  case can run many spells for one character without repeating the character's controls per spell. Two shim
  fixes the panes need: a recording ListView answers `ColumnHeaders`, and `ListItems.Add(Index, Key, Text)` now
  keeps its Text (it was dropped, which no earlier family exercised).
- `PullSpellEQ` adds teleport, summon, textblock and learn-spell rows to the list it is given; the P3-04 port
  returns only text, so `detail::spell::spell_eq_rows` walks the same abilities and emits those rows.
- Invented rows (numbers 9001 and up, textblocks 20101 and up) reach branches the stock data lacks: spell
  ability shapes with ranges, class lists, NPC greet trees with link-to teleports and bad numbers.
- Scenarios: all classes and races (plus a missing record and `bStartup`), all shops (several charm/sell
  combinations), NPC command rooms, ~120 spells with no character, with the combat option, and for two
  characters taken from the pipeline scenario. Rules plus scenario stay under 1.5 MB.
- GreaterMUD and Paramud branches are not ported.

## Done when
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `tools/guard.sh`; `python3 -I tools/oracle/oracle.py check` current for
every family; Python unit tests pass; the misc_detail test runs every fixture case.
