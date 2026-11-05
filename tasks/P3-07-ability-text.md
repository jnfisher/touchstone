---
id: P3-07
title: Ability text
model: sonnet
phase: 3
depends: [P3-04, P3-06]
touches:
  - crates/touchstone-rules/src/ability_text.rs
  - crates/touchstone-rules/src/lib.rs        # one line: pub mod ability_text;
  - crates/touchstone-rules/tests/all/ability_text.rs
  - crates/touchstone-rules/tests/all/main.rs # one line: mod ability_text;
  - fixtures/scenarios/ability_text.json
  - fixtures/rules/ability_text.json
  - tools/oracle/gen/ability_text.py
---

## Goal
Port FAM-03's text half (stock only): the procedures listed for P3-07 in `docs/inventory.md` section 8a,
`GetAbilityName`, `GetAbilityStats`, `GetAbilityList` and `GetAbilDiffText`. Follow `docs/porting.md`
exactly.

## Read
- `docs/porting.md`, `docs/inventory.md` sections 4, 4a and 8a (row P3-07), `docs/testing.md`.
- Through the index: `modMMudFunc` `GetAbilityName`, `GetAbilityStats`, `GetAbilityList`; `modMain`
  `GetAbilDiffText`; `modMMudDatabase` `GetSpellName`, `GetMonsterName`, `GetItemName`,
  `GetClassName` (the name lookups these call).
- `crates/touchstone-rules/src/spells.rs` (`SpellEnv`, `SpellText`, `PullArgs`) and `src/textblock.rs`
  (`get_textblock_action`, `TextblockData`); `tools/oracle/gen/spells.py` for the generator style.

## Do
- `get_ability_name` is the number-to-text table (stock branch; `Profile` match, no GreaterMUD names).
- `GetAbilityStats` and `GetAbilDiffText` call `PullSpellEQ` and `GetTextblockAction`; use the ported
  `spells::SpellEnv::pull_spell_eq` and `textblock::get_textblock_action`. Database names (spells,
  monsters, items, classes) come through a small trait the caller supplies.
- An `AbilityEnv` struct holds the globals (`bHideRecordNumbers`, `bQuickSpell`, `nSpellNest`, the
  global level box, `gAvgLevelMaxAllStats`) and implements `SpellText`, so it can be handed to
  `SpellEnv` and the recursion `GetAbilityStats` -> `PullSpellEQ` -> `GetAbilityStats` is ported whole.
  Say in the report how `SpellText` should be wired to it.
- ListView side effects (`lv`) are UI and not ported, as in P3-04.
- Fixture: every (ability, value) pair in the stock Items, Spells, Races, Classes and Monsters, the
  variants that change the text (hidden record numbers, calc level, quick spell, percent column,
  global level), representative values for every ability number, `GetAbilityName` over the whole range
  with and without `bForceAll`, `GetAbilityList`, and `GetAbilDiffText` grids. Text must match exactly.
  Rules plus scenario stay under 1.5 MB.
- Do not edit `fixture.rs` or `Cargo.toml`. Keep oracle changes minimal and report them. Wire nothing
  into the server or UI.

## Done when
`docs/porting.md` "Done when".
