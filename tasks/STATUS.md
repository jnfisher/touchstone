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
| P3-12 | Damage vs monster and combat rounds | #53 |
| P3-14a | Monster sim glue, monster side | #45 |
| P3-14b | Monster sim glue, character side | #58 |
| P3-15 | Lair statistics | #63 |
| P3-16 | Lair info and party damage | #68 |
| P3-26 | Exp/hr dispatcher | #56 |
| P3-28 | Row evaluators: weapons, armour, spells, sundry, shops, classes, races | #59 |
| P3-29 | List filters: weapons, armour, sundry, inventory, global | #70 |
| P3-39 | Parsed-item matching, best shop price, MegaMUD stat line | #55 |
| P3-40 | Paste character | #64 |
| P3-41 | Paste party | #57 |
| P3-42 | Hit calculator core | #50 |
| P3-44 | Character pipeline (RefreshAll) | #51 |
| P4-05 | Computed explorer columns (non-monster tabs) | #62 |
| P4-17 | EQ tab on MME's slot rules | #67 |
| P4-20 | Character tab, character API and persistence | #54 |
| P4-24/25 | .mmec import and export | #65 |
| P3-27 | Row evaluator: monsters | #77 |
| P3-30 | List filters: spells and monsters | #83 |
| P3-31 | Reference rows (GetLocations) | #76 |
| P3-32..34 | Monster detail pane rules | #82 |
| P3-35/36 | Item detail and compare text | #80 |
| P3-37 | Spell, shop, class, race and NPC-command detail | #79 |
| P4-19 | Item Manager | #78 |
| P4-21 | Global filter | #75 |
| P4-22/23 | Combat setup dialog | #74 |
| P4-28b | Monster Attack Simulator window | #73 |
| P4-DP | Detail panes in the explorers | #81 |
| P4-26 | Paste Character dialog | #71 |
| P4-27a | Hit calculator panel | #52 |
| P4-27b | Calculators use the current character; MegaMUD stat-line paste | #60 |
| P4-28a | Calculators UI: exp, coin, swing, backstab | #49 |
| P4-29/30 | Settings dialog, exp/hr knobs, settings.ini options table | #69 |
| T-025 | Make the oracle fast | #61 |
| T-050 | Release workflow and Raspberry Pi packaging (dry run passed) | #66 |

## In progress

| Card | Title | State |
|---|---|---|
| P4-08/09 | Monsters explorer: rows, monster filters, detail pane | agent |
| P4-18 | Find Best | agent |

## Next

| Card | Title | Depends |
|---|---|---|
| P4-10..13 | Class/race builder, rooms explorer | P3-31, P3-37 |
| P4-14..16 | Compare panes, context menus | explorers |
| T-016 | Textblock script parser for T-014's remaining derived fields | T-014 |
| P3-04b | `pull_spell_eq` Long handling | P3-04 |

## Follow-ups noted

- Show monster names in spell "Cast by".
- One shared `RoundUp` helper.
- Monster reachability data behind the `Reachability` / `ReachRows` traits (P3-30, P3-32).
- GreaterMUD/Paramud branches of the `Pull*` detail procedures (P3-32).
