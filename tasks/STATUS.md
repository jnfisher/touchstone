# Status

Kept by the orchestrator only. Cards not yet written as files are in `docs/inventory.md` section 8;
the orchestrator writes each card when its dependencies are close to merged.

## Done

| Card | Title | PR |
|---|---|---|
| T-001 | Scaffold the workspace, web app and CI | |
| T-002 | Fetch tools and the MME procedure index | |
| T-003 | Inventory MME's features | |
| T-010 | World model types | |
| T-011 | `.mdb` loader and data fixtures | |
| T-012 | Live silvermere/Quicksilver adapter | |
| T-013 | Serve the World with live reload | |
| T-014 | Derived (NMR) fields for live sources (lairs complete; script fields partial) | #29 |
| T-015 | Stock parity check, .mdb vs silvermere | |
| T-020 | VB6 semantics and touchstone-vb6 | |
| T-021 | Oracle interpreter | #13 |
| T-022 | Oracle shims and first fixtures | #15 |
| T-023 | VB6 IsNumeric fidelity | #31 |
| T-024 | Wire real text lookups into the rules adapters | #44 |
| P3-01 | Exp tables and training cost | #20 |
| P3-02 | Stats, vitals, CP and movement | #23 |
| P3-03 | Ability effects and lookups | #18 |
| P3-04 | Spell damage equations | #32 |
| P3-05 | Item value and shop price | #24 |
| P3-06 | Textblock and room-command parsing | #27 |
| P3-07 | Ability text | #37 |
| P3-08 | Accuracy, hit chance and dodge | #26 |
| P3-09 | Swings, energy and weapon damage | #38 |
| P3-10 | Spell casting | #36 |
| P3-11 | Character profile and attack config | #43 |
| P3-13 | Monster attack simulator (13a + 13b) | #19 |
| P3-17 | Gear aggregation I | #40 |
| P3-18 | Gear aggregation II | #41 |
| P3-19 | Char panel derivations | #42 |
| P3-20 | Exp/hr shared helpers and Model D | #21 |
| P3-21 | Exp/hr Model C | #30 |
| P3-22 | Exp/hr Model A (both parts; P3-23 folded in) | #33 |
| P3-24 | Exp/hr Model B (both parts; P3-25 folded in) | #35 |
| P3-38 | Game-text inventory parser | #25 |
| P3-43 | Calculator cores: swings, backstab, coin, exp | #46 |
| P4-00 | Explorers with raw columns | |
| P4-01a | First computed values in the UI: shop prices, exp tables | #28 |
| P4-01b | Detail panes: ability text, spell damage, monster max damage | #39 |
| P4-04 | Grid polish: column sizing, copy, find-as-you-type | #22 |
| P4-05L | Explorer layout: adaptive split and detail pane | #34 |

## In progress

| Card | Title | Depends | State |
|---|---|---|---|
| P3-14a | Monster sim glue | P3-13, P3-11 | PR #45 |
| P3-44 | Character pipeline (RefreshAll) | P3-11, P3-17..19 | agent |
| P3-12 | Damage vs monster and combat rounds | P3-09, P3-10, P3-11 | agent |
| P3-42 | Hit calculator core | P3-08, P3-11 | agent |
| P4-28a | Calculators UI: exp, coin, swing, backstab | P3-43 | agent |

## Next

| Card | Title | Depends |
|---|---|---|
| P4-20 | Char tab (with character save/load API) | P3-44 |
| P3-14b | Monster sim: character side | P3-14a, P3-44 |
| P3-15/16 | Lair statistics, lair info and party damage | P3-12 |
| P3-26 | Exp/hr dispatcher | P3-12 |
| P3-28 | Row evaluators: weapons, armour, spells, ... | P3-44 |
| P3-39, P3-41 | Parsed-item matching / MegaMUD stat line; paste party | P3-38 |
| P4-27a | Calculators UI: hit and backstab panels | P3-42 |
| T-016 | Textblock script parser for T-014's remaining derived fields | T-014 |
| T-050 | Release and Pi packaging | |

## Follow-ups noted

- `bin/touchstone/src/detail.rs` monster max damage is provisional; switch it to `monster_glue`
  (P3-14a found six differences).
- Show monster names in spell "Cast by".
- One shared `RoundUp` helper.
