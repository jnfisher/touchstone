---
id: P4-33
title: Spells tab filters, hide record numbers, builder width
model: sonnet
phase: 4
depends: [P3-30, P4-32, P4-12, P4-29]
touches:
  - crates/touchstone-rules/src/rows.rs
  - bin/touchstone/src/evaluate.rs
  - bin/touchstone/src/settings.rs
  - bin/touchstone/src/state.rs
  - bin/touchstone/src/spell_tools.rs
  - bin/touchstone/src/panes.rs
  - bin/touchstone/src/rooms.rs
  - bin/touchstone/src/detail.rs
  - bin/touchstone/src/clipboard.rs
  - bin/touchstone/src/api.rs
  - bin/touchstone/tests/
  - web/src/
  - web/tests/
  - docs/differences.md
---

## Goal
Three follow-ups left by merged cards: the Spells explorer gets MME's filter controls (magery, magery level,
learnable only, attack type, target, contains-ability) through `options.spells` on `/api/evaluate/spells`;
`bHideRecordNumbers` becomes a `settings.json` option used by every call that takes it; the class/race
builder's text fits its detail pane.

## Read
- `docs/design.md`, `docs/testing.md`, `fixtures/rules/filters_sm.json` (spell cases), `crates/touchstone-rules/src/filters/spells.rs`.
- MME through the index: `frmSettings/Form_Load`, `frmSettings/cmdSave_Click`, `frmMain/Form_Load` (the key),
  `frmMain/FilterSpells`, `frmMain/ResetFilterOptions`.
- Web: `MonsterFilters.svelte`, `RoomsTab.svelte`, `ClassRaceBuilder.svelte`, `SettingsDialog`, `settings.ts`.

## Decisions
- Key `HideRecordNumbers` in `settings.ini`, default off (code behaviour: only an exact "1" or a positive `Val` turns it on).
  In `settings.json` it is `hide_record_numbers`.
- The server owns the value: every request path reads `AppState::hide_record_numbers()` instead of trusting a
  per-request flag, so the Rooms checkbox, `hide_numbers`/`hide_record_numbers` request fields are removed.
  The evaluate cache key includes it.
- `RowEnv` gets an additive `hide_record_numbers` field (default false) so list rows' ability text follows it.
- Spell filter controls ride in `options.spells`; fields left out take `ResetFilterOptions` values.
- The builder box widens to its text (the pane scrolls only when the window is too narrow) and the text is not altered.

## Done when
Workspace and web checks pass; endpoint tests pinned to `filters_sm.json` spell cases; screenshots
`/tmp/P4-33/shots/spells-filters.png` and `builder.png`.
