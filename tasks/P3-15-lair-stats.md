---
id: P3-15
title: Lair statistics
model: sonnet
phase: 3
depends: [P3-12]
touches:
  - crates/touchstone-rules/src/lairs.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/lairs.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/lairs.json
  - fixtures/rules/lairs.json
  - tools/oracle/gen/lairs.py
  - tools/oracle/harness.py
  - tools/oracle/shims.py
---

## Goal
Port the statistics half of FAM-11 (stock only): the procedures listed for P3-15 in `docs/inventory.md`
section 8a. Given the lair text in a monster's "Summoned By" column, produce the per-monster lair
averages (`LairInfoType`), the lair counts by regen, and the outlier/median helpers behind the walk time.
`GetLairInfo` (P3-16) is a callee: it is a trait the caller supplies, not part of this card.

## Read
- `docs/porting.md`, `docs/inventory.md` rows P3-15 / P3-16 and FAM-11, `tools/oracle/README.md`.
- MME through the index, modMMudDatabase: `GetLairAveragesFromLocs`, `CountLairsByMaxRegen`,
  `RemoveOutliers`, `GetMedian`, `GetMedianAbsDev`, `GetStdDev`, `QuickSort`, `DICT_ModeFromCounts`,
  `DICT_AddToCount`, `DICT_BumpCount`, `CalculateAverageLairs`; callees `CalcAverageNonZero`
  (already ported in `combat.rs`), `RoundUp`, `AutoAppend`, `RemoveDuplicateNumbersFromString`,
  `InstrCount`.
- Already ported, to be called and not redone: `combat::calc_average_non_zero`, `combat::DamageOutput`,
  the `DF_*` flag constants.

## Decisions
- The lair text is the monster's "Summoned By" column (MME reads `tabMonsters.Fields("Summoned By")`);
  the `[a-b-c][regen]Group(lair): n/m` entries are found by a small scanner that matches what the
  VBScript pattern matches. No existing parser returns those fields, so none is reused.
- `GetLairInfo` and `GetDamageOutput` are injected through a `LairEnv` trait. The oracle replaces both
  with table-driven shims (a case's `lair_env`), so the fixture pins the statistics, not P3-16.
  The shim records every `GetDamageOutput` call as an event; the test compares the arguments (this is
  how the defence flags are pinned).
- `LairInfo` mirrors `LairInfoType` field for field (Currency stays `Currency`); P3-16 reuses it.
- `QuickSort` is ported as MME writes it (Hoare partition, middle pivot), not `sort_by`.
- A VB6 run-time error inside `GetLairAveragesFromLocs` returns the fields assigned so far, as MME's
  handler does; the port assigns into the result as it goes.
- `CalculateAverageLairs` (writes the global per-monster arrays, needs the Monsters table) is ported as
  `average_lairs_for_monster` for one monster's text: it returns the (possible spawns, spawn chance)
  pair MME stores. `DICT_AddToCount` has no caller in the stock path and is ported for completeness.
- Oracle changes are additive: a `Scripting.Dictionary` shim and the opt-in `lairs` shims; the generator
  version is not bumped, so no other fixture changes.
- Fixtures: real "Summoned By" strings (deduplicated, stratified by lair count, long ones cut to a
  whole entry) plus hand-built strings for every branch, and numeric grids for the helpers.
  Scenario plus fixture stay under 1.5 MB.

## Done when
`cargo test --workspace` passes, `oracle.py check` is current for every family, the Python unit tests
pass, and the lairs test runs every fixture case.
