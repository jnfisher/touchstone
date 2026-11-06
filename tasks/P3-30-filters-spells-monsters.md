---
id: P3-30
title: List filters, spells and monsters
model: sonnet
phase: 3
depends: [P3-29, P3-27, P3-16, P3-14a]
touches:
  - crates/touchstone-rules/src/filters.rs
  - crates/touchstone-rules/src/filters/
  - crates/touchstone-rules/tests/all/filters_sm.rs
  - crates/touchstone-rules/tests/all/main.rs
  - crates/touchstone-rules/tests/all/monster_rows.rs
  - fixtures/scenarios/filters_sm.json
  - fixtures/rules/filters_sm.json
  - fixtures/known_gaps.json
  - tools/oracle/gen/filters_sm.py
  - tools/oracle/harness.py
  - tools/oracle/shims.py
  - tools/oracle/tests/test_shims.py
  - tools/oracle/README.md
  - bin/touchstone/src/evaluate.rs
  - bin/touchstone/tests/all/evaluate.rs
  - docs/differences.md
---

## Goal
Port frmMain `FilterSpells` and `FilterMonsters` (FAM-23): which spells and monsters the two explorers list, with
which `AddSpell2LV` / `AddMonster2LV` arguments, given the filter controls (a struct whose `Default` is what MME
starts with). Same pattern as P3-29: the output is the ordered list of `Add*2LV` calls MME makes. `/api/evaluate/spells`
goes through FilterSpells (and the P4-21 global filter) and `/api/evaluate/monsters` is new, through FilterMonsters with
default filters (P4-08/P4-09 add the UI).

## Read
- `docs/porting.md`, `docs/inventory.md` row P3-30 and FAM-23, `docs/testing.md`, `tools/oracle/README.md`,
  `tasks/P3-29-list-filters.md`, `tasks/P3-27-monster-rows.md`.
- MME through the index: frmMain `FilterSpells`, `FilterMonsters`, `RemoveFilters`, `optMonsterFilter_Click`,
  `txtMonsterEXP_Change`, `RefreshCombatHealingValues` (modMain); frmMonsterFilters `cmdExec_Click` (the extra filters);
  modMain `AddSpell2LV`, `AddMonster2LV`, `GetPreCalculatedMonsterDamage`, `GetDamageOutput`, `InstrCount`; modMMudFunc
  `SpellIsUsable`, `SpellIsInGame`; modMMudDatabase `GetLairAveragesFromLocs`, `CountLairsByMaxRegen`,
  `GetMonsterAttackSummary`; modExpPerHour `CalcExpPerHour`; modReachability `ReachIsMonsterOK`.
- `crates/touchstone-rules/src/{filters,rows,lairs,spell_cast}.rs`, `rows/monster.rs`.

## Decisions
- `filters/spells.rs` and `filters/monsters.rs`, re-exported from `filters`. `filter_spells` returns the spell numbers
  `AddSpell2LV` is called for plus the "*Spells*" flag and whether column 7 reads "Cast"; the profile (`tChar`) is the
  caller's, as `PopulateCharacterProfile` made it.
- `filter_monsters` runs on the `CharLairEnv` of P3-27 and takes the caller's `tChar` profile (after
  `RefreshCombatHealingValues`). It returns every `AddMonster2LV` call as a `MonsterAdd` (record, `MonsterRowArgs`,
  and the `tChar` HP and HP regen at that moment, because the no-character exp/hr filter overwrites them for the rest of
  the pass) and also hands each call to a callback with the env, so a caller can build the row exactly where MME does
  (the lair cache is shared between the two procedures).
- MsgBox questions (recalculate the monster damage first) are a callback; Cancel leaves the list as it was. A Yes only
  reports that MME would have recalculated: the caller owns the damage caches.
- Reachability (`ReachIsMonsterOK`) is a trait; the default says every monster is reachable. The oracle stubs the
  module with a scripted rule so that the plumbing (level passed, skip, show-all) is pinned.
- `RemoveFilters` -> `FilterMonsters(True)` is `remove_filter`: solo mode, defaults, extras off, every in-game monster.
  The locals MME declares outside its loop keep their values across iterations (they are only reset after the remove
  and hard-coded skips); the port keeps them in one struct to reproduce the arguments of such a call.
- Oracle: frmMain's own `FilterSpells`/`FilterMonsters` cut out by `frm.py`; `AddSpell2LV` and `AddMonster2LV` are
  recorded (record number and arguments) except in a few cases that run the real ones. The Monsters table is a
  stratified subset (new scenario key `db_only`, a list of record numbers per table) because the full Summoned By text
  is 2 MB. Characters are the pipeline scenario's.

## Done when
`cargo fmt/clippy/test` pass, `tools/guard.sh`, `oracle.py check` reports every family current, the Python unit tests pass,
every `filters_sm` case passes, scenario plus fixture under 1.5 MB, generation timed under about 5 minutes.
