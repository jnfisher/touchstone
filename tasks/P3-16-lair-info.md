---
id: P3-16
title: Lair info and party damage
model: sonnet
phase: 3
depends: [P3-15, P3-12, P3-14a, P3-14b]
touches:
  - crates/touchstone-rules/src/lairs.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/src/pipeline.rs
  - crates/touchstone-rules/tests/all/lair_info.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/lair_info.json
  - fixtures/rules/lair_info.json
  - tools/oracle/gen/lair_info.py
  - tools/oracle/harness.py
  - tools/oracle/shims.py
---

## Goal
Port the rest of FAM-11 (`docs/inventory.md` row P3-16 and appendix A): `GetLairInfoIndex`,
`GetLairInfo`, `SetLairInfo`, `LoadLairInfo` and `CalculateMonsterPartyDamage`. The per-group lair
cache that MME keeps in module globals becomes an explicit `LairCache` the caller builds once per
(world, character, party, options). P3-15 left `LairEnv` as a trait; this card implements the real one
on top of the cache, `combat::get_damage_output` and `monster_glue`.

## Read
- modMMudDatabase: `GetLairInfoIndex`, `GetLairInfo`, `SetLairInfo`, `LoadLairInfo`, `DICT_*` helpers;
  frmMain `CalculateMonsterPartyDamage`; modMain `SetCurrentAttackTypeConfig` (already ported).
- `tasks/P3-15-lair-stats.md` (its follow-up: `GetLairInfo` sets `nRTK`, `nRTC`, `nMaxRegen`),
  `crates/touchstone-rules/src/{lairs,combat,monster_glue,pipeline,character}.rs`.

## Decisions
- `LoadLairInfo` reads the `Lairs` table (421 rows in the stock data) and, per lair, walks its mob list
  through `Monsters` for abilities, undead, BS defence and melee accuracy. (The brief says
  "from Monsters and Rooms"; MME's table is the converted `Lairs`, and Rooms are not read.)
- `LairCache` holds `colLairs` (slot 0 is the unused slot MME dimensions) and `dictLairInfo`
  (case-insensitive text keys, `vbTextCompare`). `get_lair_info_index` appends like MME. It also owns
  `tLastAvgLairInfo`, which `CalculateMonsterPartyDamage` resets.
- `get_lair_info` takes a `LairQuery` (everything the procedure reads from frmMain and module
  globals: `bStartup`, the global-filter box, the party boxes, `sGlobalAttackConfig`) and a
  `LairDamage` trait for the three outside callees (`GetDamageOutput` for the average lair,
  `GetPreCalculatedMonsterDamage`, `CalcCombatRounds`'s profile). Types follow `LairInfoType`
  (Long/Integer/Currency) so conversions round and overflow where MME's do; a VB error returns what had
  been assigned, as MME's handler does.
- The real `LairEnv` is `CharLairEnv`: it owns `&mut LairCache` plus a `DamageEnv`, the three
  monster-damage arrays and the glue state, and answers `get_lair_info` from the cache and
  `get_damage_output` from `combat::get_damage_output`.
- `CalculateMonsterPartyDamage` is `CalculateMonsterDamageVsCharALL(True)` plus UI refresh; the
  port returns the party-damage array (the value MME would store per monster) and resets the cache's
  last-average lair. The oracle cannot run ALL (progress-bar form), so it is checked in Rust against the
  per-monster function, as P3-14b does.
- Characters come from the pipeline scenario: the generator copies a pipeline case's controls and
  globals, so the oracle's own `RefreshAll` sets the character up, then `SetCurrentAttackTypeConfig`,
  `LoadLairInfo` and the `GetLairInfo` calls run. The Rust test rebuilds the character with
  `refresh_all`. The damage-vs-character and damage-vs-party caches are family-wide arrays of invented
  but fixed values for the monsters in the chosen lairs.
- Oracle changes are additive: the Dictionary shim gains `CompareMode`/`RemoveAll` and optional
  case-insensitive keys; no other fixture changes.
- GreaterMUD/Paramud branches are not ported (`bGreaterMUD` is False).

## Done when
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `tools/guard.sh`; `python3 -I tools/oracle/oracle.py check` current
for every family; Python unit tests pass; the lair_info test runs every fixture case.
