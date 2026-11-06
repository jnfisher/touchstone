---
id: P3-14b
title: Monster sim glue, character side
model: sonnet
phase: 3
depends: [P3-14a, P3-44, P3-12]
touches:
  - crates/touchstone-rules/src/monster_glue.rs
  - crates/touchstone-rules/src/pipeline.rs
  - crates/touchstone-rules/tests/all/monster_glue_char.rs
  - crates/touchstone-rules/tests/all/pipeline.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/monster_glue_char.json
  - fixtures/rules/monster_glue_char.json
  - tools/oracle/gen/monster_glue_char.py
---

## Goal
Port the character side of FAM-13 (`docs/inventory.md` section 8a row P3-14 and appendix A FAM-13),
which P3-14a left out: `SetupMonsterAttackSimWithCharStats` (the character's AC, DR, dodge, MR,
anti-magic, protection from evil, resists and vile ward, or the lair filter's party values, into the
simulator), `CalculateMonsterDamageVsChar` (solo and party), `CalculateMonsterDamageVsCharALL`,
`GetPreCalculatedMonsterDamage`, `GetMonsterAvgDmgFromDB`. The result is "how much damage does
monster N do to this character per round", for the monster list, lairs (P3-16) and the detail pane.

## Read
- modMain: `SetupMonsterAttackSimWithCharStats`, `CalculateMonsterDamageVsChar`,
  `CalculateMonsterDamageVsCharALL`, `ClearMonsterDamageVsCharALL`, `GetPreCalculatedMonsterDamage`,
  `GetCharActiveVileWard`; modMMudDatabase `GetMonsterAvgDmgFromDB`, `TestMonExpMulti`.
- `crates/touchstone-rules/src/monster_glue.rs` (P3-14a), `pipeline.rs`, `character::PartyFilter`.

## Decisions
- The character is the controls MME reads, as a small `CharSimInputs` struct (class, AC and DR
  captions, MR text, dodge tag, anti-magic box, protection-from-evil tag, five resist tags).
  `CharacterState::sim_inputs()` in `pipeline.rs` is the additive adapter. A resist tag that is
  still empty (`None`) makes MME's Variant-to-Long assignment fail with error 13; the handler ends
  the setup there, so later resists keep `ResetValues`' zeros.
- Party values come from `character::PartyFilter.lair_text` (party size 0, AC 1, DR 2, MR 3, dodge 4,
  anti-magic count 6), as `Val(...)`. They are `Val` Doubles assigned to Long/Integer fields, so
  they round half to even and can overflow (error 6, which ends the setup or the damage call).
- MME's damage cache (the three module arrays, `sMonsterDamageVsCharDefenseConfig`, the
  "don't prompt" flags, `ClearMonsterDamageVsCharALL`) is dropped: the calculators are pure and
  return the value MME would store, `Round(x, 1)`, next to the function's own (unrounded) result.
  `GetPreCalculatedMonsterDamage` is the cache's reader, so it takes the three arrays as slices; a
  monster number past an array's end is VB error 9, which the handler turns into 0 and no label.
- `nGlobalMonsterSimRounds` is a parameter, clamped to 100..10000 as MME does on every call.
- Fixtures run random-mode sims (`bDynamic` False, as `CalculateMonsterDamageVsChar` passes) on the
  oracle's seeded `Rnd`, one case per (character, monster).
- `CalculateMonsterDamageVsCharALL` walks the Monsters table with a progress-bar form and
  `DoEvents`; the oracle has no shim for that form, so ALL is tested in Rust against the
  per-monster function (same monster order, same shared generator) rather than against a fixture.
- Characters come from the pipeline fixture (`fixtures/rules/pipeline.json`): the generator copies
  the control values the oracle's own `RefreshAll` left, and the Rust test rebuilds the character
  with `refresh_all` and checks the adapter reproduces those values. Synthetic characters (empty
  controls, fractional captions) cover the assignment quirks.
- `TestMonExpMulti` (does the Monsters table have an `ExpMulti` column) is a data-loader concern,
  not a formula; the model already carries `exp_multi`.

## Done when
`docs/porting.md` "Done when"; `oracle.py check` current for every family; every
`fixtures/rules/monster_glue_char.json` case passes; scenarios plus rules under 1.5 MB, generation
under about 5 minutes.
