---
id: P4-05
title: Computed explorer columns (weapons, armour, spells, items, races, classes, shops)
model: sonnet
phase: 4
depends: [P3-28, P3-44, P4-20]
touches:
  - bin/touchstone/src/evaluate.rs
  - bin/touchstone/src/api.rs
  - bin/touchstone/src/lib.rs
  - bin/touchstone/src/state.rs
  - bin/touchstone/tests/
  - web/src/lib/
  - web/src/lib/components/ExplorerView.svelte
  - web/scripts/
  - docs/design.md
  - tasks/P4-05-computed-columns.md
---

## Goal
Every non-monster explorer shows MME's list: the rows and columns the `Add*2LV` builders produce
(ported in `crates/touchstone-rules/src/rows.rs`), for the Character tab's current character or, with
none, MME's no-character view. The raw columns and `pending` lists of those seven tabs in
`web/src/lib/explorers.ts` give way to the server's cells.

## Read
`crates/touchstone-rules/src/rows.rs` (Row/Cell, RowEnv, the `add_*_row` functions,
`no_character_profile`, `race_color_code`), `pipeline.rs` (`CharacterState::profile_for`,
`equipped_globals`), `bin/touchstone/src/character.rs` (the wire character, `evaluate`),
`web/src/lib/character.svelte.ts` (`getCharacterStore`), MME frmMain `FilterWeapons`, `FilterArmour`,
`FilterSpells`, `FilterSundry`, the `lv*_ColumnClick` handlers (sort kinds), modListViewExt
`BuildSecondaryKey` (numeric sort is `Val` of the text) and the `ColumnHeaders.Add` block of frmMain.

## Decisions
- `POST /api/evaluate/{weapons|armour|spells|items|races|classes|shops}` with
  `{character?, options}`; unknown tab 404, malformed body or option 400, a character without class
  and race 422 (as the Character tab's evaluate). Response `{columns, rows}`: `columns[0]` is the `#`
  column, so a row's `cells` line up one to one with `columns`. A cell is `{text, sort, color?}`:
  `sort` is a number for numeric columns (`Val` of the text, as MME's numeric sort, or of the cell
  tag where MME sorts by it: weapons Dmg/1st, armour AC/DR) and the text for text columns.
  `color` is the cell's VB `ForeColor` as MME assigns it (race stat cells); the web maps it to
  theme tokens.
- Headers are the `ColumnHeaders.Add` texts of frmMain, not P4-00's guesses. The spell column 7 is
  "Diff", or "Cast" when the character is used and the spell level box is above 0 (as `FilterSpells`).
- Character use follows `chkGlobalFilter`: a character is used only when it is sent with
  `global_filter: true`. The explorer sends it that way when "Use character" is ticked, which stands
  for MME's checkbox.
- Options (MME's defaults when absent): weapons `calc_combat`, `slow_85`, `attack` (5 normal, 4
  backstab, 6 bash, 7 smash), `vs_ac`, `vs_dr`, `vs_dodge`; weapons and armour `ability`, `ability_op`
  (`le`/`ge`, default `ge`), `ability_value` (default 1), where ability -1 is "Negates (= Spell#)";
  spells `level` (txtGlobalLevel(1), default the character's level), `calc_combat`, `anti_magic`,
  `vs_mr`. A backstab list ends with the surprise punch row, as `FilterWeapons`.
- Which records are listed stays as P4-00 (in game only, name rules); MME's character-usability
  filters and the Martial Arts / Maximum effort lists are follow-ups.
- Responses are cached in memory per (load count, tab, request) so a tab switch is instant; the
  cache is dropped when the data reloads.

## Do
1. `bin/touchstone/src/evaluate.rs` and the route; a small cache in `AppState`.
2. Tests: endpoint tests pinned to `fixtures/rules/rows.json` case ids (no-character lists, the
   race colours, a loaded warrior's weapons and spells), option and error cases.
3. Web: an evaluate client, `ExplorerView` takes the payload for these tabs (sort by `sort`, show
   `text`), re-fetch on character change (debounced), theme-token cell colours, vitest for the mapping.
4. `docs/design.md` API list.
5. Screenshots (not committed) `/tmp/P4-05/shots/{weapons,spells,armour}.png`.

## Done when
`cargo fmt/clippy/test`, `tools/guard.sh`, `npm run check`, `npm test`, `npm run build` pass;
endpoint tests pass; screenshots exist.
