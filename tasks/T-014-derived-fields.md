---
id: T-014
title: Compute NMR-derived fields for live silvermere/Quicksilver sources
model: sonnet
phase: 4
depends: [T-015]
touches:
  - crates/touchstone-data/src/derive/
  - crates/touchstone-data/tests/all/derive.rs
  - docs/data-gaps.md
---

## Goal
A World loaded from a live silvermere database has the fields MME's `.mdb` gets from NMR's
pre-computation: the Lairs table and `Rooms.lair`, item `obtained_from` and references, spell
`learned_from`, `casted_by` and `classes`, monster `summoned_by`, TBInfo `action`. Monster average
damage, average lair exp and script value are computed by phase 3 rules and are out of scope here.

## Read
`docs/data-gaps.md` (what is missing and where its inputs live), the T-015 parity report
`docs/data-parity.md`, and the stock `.mdb` SQLite to see each field's exact text format.

## Do
1. One module per derived field in `src/derive/`, run after `silvermere::load_world()`.
2. Match NMR's output format exactly (field text, ordering, separators), learned from the stock
   `.mdb` values. Never guess silently: an unclear rule goes in data-gaps.md.
3. Test with the T-015 parity tool: stock-through-silvermere with derivation must equal the stock
   `.mdb` World for these fields; record the match rate per field in `docs/data-parity.md`.

## Done when
Workspace checks pass; each derived field matches the stock `.mdb` for at least 99% of records,
and every mismatch class is explained in data-gaps.md.
