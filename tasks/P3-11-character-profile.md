---
id: P3-11
title: Character profile and attack config
model: sonnet
phase: 3
depends: [P3-02, P3-03, P3-08, P3-10]
touches:
  - crates/touchstone-rules/src/character.rs
  - crates/touchstone-rules/src/character/
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/character.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/character.json
  - fixtures/rules/character.json
  - tools/oracle/gen/character.py
---

## Goal
Port the modMain half of FAM-05 (stock only): the eleven procedures listed for P3-11 in
`docs/inventory.md` section 8a (`PopulateCharacterProfile`, `SetCurrentAttackTypeConfig`,
`RefreshCombatHealingValues`, `bCharIsEvilAligned`, `GetCharActiveVileWard`,
`SetCharDefenseDescription`, `GetCurrentAttackName`, `ClearLearnedSpells`, `LearnOrUnlearnSpell`,
`LearnSpell`, `UnLearnSpell`), and define the Rust `Character` (the user's character as an `.mmec`
file stores it, inventory section 5) and `CharacterProfile` (all of `tCharacterProfile`) with the
mapping between them. Follow `docs/porting.md` exactly. The Char-tab refreshers (`RefreshHitPoints`,
`RefreshMagic`, ...) are P3-19's and gear aggregation (`CalcCharacterStats`) is P3-17/18's.

## Read
- `docs/porting.md`, `docs/inventory.md` rows P3-11 and FAM-05, sections 4a and 5, `tools/oracle/README.md`.
- MME through the index: the eleven modMain procedures above; callees `GetClassCombat`,
  `GetClassStealth`, `GetSpellManaCost`, `GetSpellDuration`, `GetSpellName`, `GetSpellShort`,
  `GetItemName`, `ItemHasAbility`, `IsTwoHandedWeapon`, `GetItemStrReq`, `GetMaxLevel`,
  `CalcEncumbrancePercent`, `CalcMovementSpeed`, `CalculateBackstabAccuracy`, `CalculateSpellCast`.
- Already ported, to be called and not redone: `stats::{calc_encumbrance_percent, calc_movement_speed,
  get_class_combat, get_class_stealth}`, `accuracy::calculate_backstab_accuracy`,
  `abilities::item_has_ability`, `spells::get_spell_duration`, `exp::get_max_level`,
  `spell_cast::SpellCastEnv::calculate_spell_cast`.

## Decisions
- The procedures read `frmMain` controls and module globals. The rules take them as plain inputs:
  `Character` (what a `.mmec` stores, including the `nGlobalAttack*` choice), `CharStats` (what
  `CalcCharacterStats` and the Char-tab panels leave in control `.Tag`/`.Caption` text and in the
  `nGlobalChar*` weapon globals, as `val()` numbers), `PartyFilter` (lair-mode controls) and tables
  as slices. The fixture records each case's controls and globals as inputs.
- `PopulateCharacterProfile` fills a `CharacterProfile` in place (callers reuse one), and on a VB6
  error leaves it partly filled with no final clamps, as MME's handler does.
- A combo with nothing selected (`ItemData(-1)`, run-time error 381) is `Character.class/race: None`.
  The oracle's control shim now raises 381 there like VB6 (it used to read 0).
- `bGreaterMUD` branches are not ported; `Profile::Stock` only. The `CalculateAccuracy` branch of
  `PopulateCharacterProfile` is reachable only for GreaterMUD and is not ported.
- Cache invalidation (`ClearSavedDamageVsMonster`, `bDontPromptCalcCharMonsterDamage`) is reported as
  a flag in the result, not performed; `RefreshLearnedSpellColors` and `Unload frmPopUpOptions` are UI.
- Change `tools/oracle` only for a real oracle bug or a missing UI no-op, minimally, and say so in the
  report.
- Rules plus scenario stay under 1.5 MB.

## Done when
`docs/porting.md` "Done when": `cargo test --workspace` passes, `oracle.py check character` reports the
fixture current (and `oracle.py check` for the other families still does), and the family's test runs
every fixture case.
