---
id: T-022
title: Oracle shims, scenarios and the first fixture family
model: sonnet
phase: 2
depends: [T-021, T-011]
touches:
  - tools/oracle/
  - fixtures/rules/
  - fixtures/scenarios/
  - .github/workflows/oracle-check.yml
---

## Goal
The oracle runs MME's computations for a described character and target, and writes deterministic
fixtures. The first family (chosen in `docs/inventory.md`, T-003; start with the smallest family
that has no form dependencies) is generated.

## Do
1. Shims: DAO `Recordset` over the converted stock SQLite (`Index`, `Seek "="`, `NoMatch`,
   `MoveFirst/Next/Last`, `EOF/BOF`, `Fields("x")`, `rs!x`, `RecordCount`, `Bookmark` as needed),
   matching how MME opens each `tab*` global; module globals initialised as VB6 would (zero/empty);
   form controls the formulas read, as values supplied by the scenario.
2. Scenario format `fixtures/scenarios/<family>.json`: global settings, a character (the globals and
   inputs MME sets), and cases (procedure, arguments). Documented in `tools/oracle/README.md`.
   Generate cases systematically (grids over levels, stats, classes, races, record numbers) rather
   than by hand; a few hundred per family is enough.
3. `oracle.py gen FAMILY` writes `fixtures/rules/<family>.json`: header (MME sha, tolerance), and
   per case the inputs and outputs. Byte-identical on a re-run.
4. `oracle.py check` regenerates all families to a temp dir and diffs with the committed fixtures.
   `.github/workflows/oracle-check.yml` (`workflow_dispatch` only) runs it.

## Done when
`oracle.py gen <family>` and `oracle.py check` pass; the fixture holds at least 200 cases; report
the run time.
