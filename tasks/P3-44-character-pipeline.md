---
id: P3-44
title: Character pipeline (RefreshAll)
model: sonnet
phase: 3
depends: [P3-02, P3-11, P3-17, P3-18, P3-19]
touches:
  - crates/touchstone-rules/src/pipeline.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/pipeline.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/pipeline.json
  - fixtures/rules/pipeline.json
  - fixtures/known_gaps.json
  - tools/oracle/gen/pipeline.py
  - tools/oracle/frm.py
  - tools/oracle/
---

## Goal
Chain the per-procedure character ports (`gear`, `char_panel`, `character`, `attack`, `spell_cast`)
the way frmMain `RefreshAll` does, and expose one entry point
`pipeline::refresh_all(profile, world, &character::Character, &PipelineOptions) -> Result<CharacterState, RulesError>`.
`CharacterState` holds everything the Char tab shows (stat vector, CP panel, HP, mana, MR, picklocks,
dodge, stat labels, encumbrance level, bless mana total, combat healing values), the populated
`character::CharacterProfile`, and adapters to `attack::CharacterProfile` +
`attack::EquippedGlobals` and `spell_cast::SpellCasterProfile`. Backbone of P4-20, `/api/evaluate`
and the row evaluators.

## Read
- `docs/porting.md`, `docs/testing.md`, `docs/inventory.md` sections 4/4a/5/8 (FAM-04, FAM-05).
- Through the index (frmMain): `RefreshAll`, `SetupClass`, `RefreshCPs`, `CalcCharacterStats` (its
  tail: `RefreshMagic`, `RefreshMainStatBonuses`, `RefreshDodge`, `lblInvenCharStat_Change`),
  `RefreshHitPoints`, `RefreshMagicRes`, `RefreshPicklocks`. modMain: `RefreshCombatHealingValues`,
  `PopulateCharacterProfile`.
- Existing ports, called and not redone: `char_panel::*`, `gear::{carried_items, calc_character_stats}`,
  `character::{populate_character_profile, refresh_combat_healing_values}`.

## Decisions
- Order is MME's: SetupClass, RefreshCPs, CalcCharacterStats (whose tail runs RefreshMagic, then
  RefreshMainStatBonuses, RefreshDodge and the encumbrance level), RefreshHitPoints, RefreshMagicRes,
  RefreshPicklocks, RefreshCombatHealingValues. RefreshShops and the list-colour refreshes are display
  and skipped. Values one step leaves in a control are threaded to the next exactly as MME's order
  produces them (SetupClass writes the 2nd-quest option CalcCharacterStats reads; CalcCharacterStats
  writes `txtCharHPRegen`/`txtCharManaRegen`, which RefreshHitPoints/RefreshMagic read).
- `txtCharMaxStats(i).Tag` (the race stat floors RefreshCPs subtracts) comes from the race row, as
  `cmbGlobalRace_Click` writes it; 0 when no race row.
- A class or race of `None` (combo `ListIndex < 0`, a state MME only passes through while loading)
  returns `RulesError::Unsupported`.
- Control event cascades (a `.Text` change re-running a Refresh*) re-run a procedure on values that are
  already final, so the port runs each step once.
- `lblInvenCharStat(46)` (damage output, `GetDamageOutput`) is not computed, as in the gear cards.
  `SetCharDefenseDescription` (a cache key for a UI prompt) and tool tips are not part of the state.
- The state's profile is `PopulateCharacterProfile(force_use_char, force_no_party, Mud None, weapon 0)`;
  `CharacterState::profile_for(mud, weapon)` builds the others (backstab).
- `PipelineOptions` carries the view toggles with MME defaults: hide class/race stats, Item Manager
  rows, minimum item level, data-version caption, `nNMRVer`, `gAvgLevelMaxAllStats`, the lair-mode
  controls, and RefreshAll's own `bSetupClass`.
- Oracle: the scenario cuts `RefreshAll` and the procedures it calls from `frmMain.frm` and runs it
  whole, then `PopulateCharacterProfile`. One additive `frm.py` option (`tail`) lets a cut keep the
  `out:` section of `CalcCharacterStats`. Scenario characters are built from stock rows (every class
  across races, levels 1/15/50/max, empty and full gear, bless, quests, anti-magic, add-weight, every
  attack and heal type, carried items).

## Done when
`docs/porting.md` "Done when"; scenario + rules stay under 1.5 MB together; `oracle.py check` current for
every family; `tests/all/pipeline.rs` runs every case and all pass (or are in `fixtures/known_gaps.json`).
