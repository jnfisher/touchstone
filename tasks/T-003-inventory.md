---
id: T-003
title: Inventory MME's features and map them to procedures
model: sonnet
phase: 1
depends: [T-002]
touches:
  - docs/inventory.md
---

## Goal
`docs/inventory.md`: every MME feature, the procedures that implement it, and a proposed card
list for phases 3 and 4. The orchestrator writes those cards from it.

## Read
`<cache>/mme-index/<sha>/index.md` and `index.json` (from T-002), MME's `README.md` and
`docs/*.md` in the MME cache (changelog for feature names, `exp-per-hour-models.md`). Open
individual procedure files only to confirm what a procedure does; do not read whole forms.

## Do
1. Features: walk `frmMain`'s menus, tabs (F1 to F11 sections), list columns, filters and detail
   panes, then every other form (calculators, character load/paste/save, spellbook, settings that
   change formulas). For each feature: what the user sees, the entry procedures (event handlers),
   and the computation procedures they reach (`mme_index.py --deps`), with total lines.
2. Formula families: group computation procedures (mostly `modMMudFunc`, `modSyntaxsFunc`,
   `clsMonsterAttackSim`, `modExpPerHour`, plus computation that lives inside `frmMain`) into families
   of 300 to 1,500 lines that can be ported and fixture-tested independently (for example: stats
   and accuracy, hit chance, swings and damage, spells, backstab, monster attacks, exp/hr). For each:
   procedures, inputs (globals, recordset fields, controls), outputs, dependencies on other
   families, and which use GreaterMUD/Paramud branches (stock only is ported).
3. Character state: list the `nGlobalChar*` and related globals, where they are set and from which
   inputs, and the `.mmec` INI keys.
4. Classify every feature: `port` (phase 3/4), `optional` (map, pathing, reachability, MegaMUD
   paths, room explorer), `skip` (Windows-only chrome: dark-mode subclassing, monitors, DPI, registry,
   window helpers), with one line why.
5. Proposed cards: a table of titles, phase, depends, the procedures each ports, and size
   (S under 300 lines, M under 1,000, L over: split L).

## Done when
Every procedure in `index.json` appears in exactly one feature or family, or in a `skip` list.
A script check: list the procedure names in `index.json` that the document does not mention
(must be empty, or listed in an "unclassified" appendix with a reason).
