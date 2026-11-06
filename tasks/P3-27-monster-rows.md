---
id: P3-27
title: Row evaluator, monsters
model: sonnet
phase: 3
depends: [P3-12, P3-14a, P3-14b, P3-16, P3-26, P3-10, P3-04, P3-02]
touches:
  - crates/touchstone-rules/src/rows.rs
  - crates/touchstone-rules/src/rows/monster.rs
  - crates/touchstone-rules/src/pipeline.rs
  - crates/touchstone-rules/tests/all/monster_rows.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/monster_rows.json
  - fixtures/rules/monster_rows.json
  - fixtures/known_gaps.json
  - tools/oracle/gen/monster_rows.py
  - tools/oracle/harness.py
  - tools/oracle/shims.py
---

## Goal
Port modMain `AddMonster2LV` (FAM-21): the Monsters list row with the computed columns MME shows in solo, lair
and party modes (damage, exp/hr through `exp_per_hour::calc_exp_per_hour`, lair counts, spawn counts, attack
summary). The output is a `rows::Row`, like P3-28's. P3-34 ports the other copy of the exp/hr scripting value
(PullMonsterDetail); the two must agree.

## Read
- modMain `AddMonster2LV`, frmMain `FilterMonsters` (how it calls the row builder and what it passes), modMain
  `GetPreCalculatedMonsterDamage`, `GetDamageOutput`; modMMudDatabase `GetLairAveragesFromLocs`, `GetLairInfo`,
  `GetMonsterAttackSummary`; modExpPerHour `CalcExpPerHour`; modSyntaxsFunc `PutCommas`, `SortLettersWithSeparator`, `RoundUp`.
- `docs/inventory.md` section 6 note on the duplicated exp/hr value; `tasks/P3-16-lair-info.md`, `P3-28-row-evaluators.md`.
- `crates/touchstone-rules/src/{lairs,combat,monster_glue,pipeline,rows}.rs`.

## Decisions
- `rows/monster.rs` holds `add_monster_row`. It runs over a `CharLairEnv` (the lair cache with `tLastAvgLairInfo`, the
  `GetLairInfo` query, the damage environment and the three damage caches), because every callee the row needs is
  already reachable through it. The frmMain controls are the env's `PartyFilter` (`optMonsterFilter(1)`,
  `txtMonsterLairFilter(0)`) and `use_character`; the rest (`UseExpMulti`, `lvMonsters` or another list, the
  `nMonsterPossy`/`nMonsterSpawnChance` arrays, exp/hr settings, backstab flag) is a `MonsterRowInputs` struct.
- Optional arguments are a `MonsterRowArgs` with MME's defaults (-9999, -1, ...).
- A VB error stops the procedure where it happens and the row built so far stays, as `Resume out` does; an error
  before the row exists (an overflowing party size) gives `None`.
- `pipeline.rs` gains an owning `CharacterEnv` that builds `Context` and `DamageEnv` from a `CharacterState`
  (P3-16's follow-up), so the test and later callers need no hand-assembled pieces.
- Colour `RGB(193,0,232)` and grey `RGB(192,192,192)` are the light-theme values (`TColor` is the identity). The row's own
  ForeColor (non-matching filter) needs a field on `Row`: `fore_color`, additive.
- Oracle: `RGB` bound; the recording ListView takes `ForeColor` on rows and an `hwnd` (a list that is not
  `lvMonsters`). Characters come from the pipeline scenario; `nMonsterPossy`/`nMonsterSpawnChance` are what
  `CalculateAverageLairs` computes, written as family-wide arrays; damage caches are invented fixed values.
- Scenarios: no character, three characters, solo / lair / party / compare list, data versions 1.82 and
  older, passed optional arguments, exp/hr settings; about 150 stratified monsters in solo and about 40 in lair mode.

## Done when
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `tools/guard.sh`; `oracle.py check` current for every family; Python unit tests
pass; every `monster_rows` case passes; scenario plus fixture under 1.5 MB, generation timed under about 5 minutes.
