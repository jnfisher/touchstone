---
id: P4-21
title: Global filter bar
model: sonnet
phase: 4
depends: [P3-29, P4-20, P4-05]
touches:
  - web/src/lib/
  - web/src/lib/components/
  - web/src/routes/+layout.svelte
  - bin/touchstone/src/evaluate.rs
  - bin/touchstone/tests/
  - tasks/P4-21-global-filter.md
---

## Goal
MME's global filter (FEAT-GFILT) as one bar under the tabs: class, race, level, alignment, "Use
character" and minimum item level, applied to every explorer list at once. It replaces P4-05's
per-explorer "Use character" checkbox.

## Read
MME frmMain `chkGlobalFilter_Click`, `cmbGlobalClass_Click`, `cmbGlobalRace_Click`,
`txtGlobalLevel_Change`, `cmbGlobalAlignment_Click`, `txtGlobalMinLVL_Change`, `cmdFilter_Click`,
`ItemIsUsableByChar` (through the index); `crates/touchstone-rules/src/filters.rs` (`FilterEnv`,
`filter_armour`, `filter_sundry`, `filter_weapons`); `bin/touchstone/src/evaluate.rs`;
`web/src/lib/evaluate.ts`, `view.ts`.

## Decisions
- In MME the class, race, level and alignment boxes of the filter frame are the character's own
  boxes (the arrays are kept in step), and `chkGlobalFilter` turns the filter on. The web has a
  character document apart from the bar, so the bar has two modes. "Use character" on mirrors the
  character (class, race, level, alignment come from it, the bar shows them read-only). Off, the
  bar's own values apply.
- No separate on/off: the defaults (any class, level 999, any alignment) are the case
  `ItemIsUsableByChar` answers true for at once, so an untouched bar filters nothing.
- Wire: `{character?, global_filter?: {use_character, class, race, level, alignment,
  min_item_level}, options}`. `use_character` needs a character (400 without one) and uses its
  state for the row calculations as before; class/race/level/alignment then come from the
  character. With `use_character` false the character is ignored for the filter, the bar's values
  go to `ItemIsUsableByChar` through `FilterEnv`, and the rows use the no-character profile (MME's
  boxes would hold the same class and level, with the character's stats; a bar without a
  character has none). `min_item_level` always comes from the bar. Without `global_filter` the
  request means what P4-05 made it mean (`character.global_filter`).
- Validation: class and race 0..=32767, level and min level 0..=999, alignment 0..=3, else 400.
- Spell rows keep P4-05's behaviour: the character's level and cast chance only in character
  mode; FilterSpells (P3-30) is not ported, so the bar does not filter spells yet.
- Web: pure module `globalfilter.ts` (shape, defaults, persistence, request mapping) and a store;
  the bar persists to its own local-storage key like the other view options. Races and classes
  come from the tables the explorers already fetch.

## Done when
Workspace and web checks pass; endpoint tests pinned to `fixtures/rules/filters.json` armour and
sundry/weapon case ids pass; screenshot `/tmp/P4-21/shots/global-filter.png` exists.
