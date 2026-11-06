---
id: P4-12
title: Class/Race stat builder
model: sonnet
phase: 4
depends: [P3-01, P3-02, P3-28, P3-37, P4-DP]
touches:
  - crates/touchstone-rules/src/class_race_builder.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/
  - fixtures/scenarios/class_race_builder.json
  - fixtures/rules/class_race_builder.json
  - tools/oracle/
  - bin/touchstone/src/
  - bin/touchstone/tests/
  - web/src/lib/
  - web/src/lib/components/
  - docs/differences.md
---

## Goal
MME's Classes and Races tabs have a "Build" view. With a class and a race both selected,
`RefreshClassRaceBuilder` prints a text block: the combined exp chart and exp at four level steps, the HP
roll range and HP at each step for four HEA values, and swings/damage at each step for four AGI values.
`cmdClassRace_Click` holds the view buttons (Class, Race, Build) and two actions (Equip, Spellbook).

## Read
- `docs/porting.md`, `docs/testing.md`, `tasks/P4-18-find-best.md` (style), `crates/touchstone-rules/src/{stats,exp,attack,char_panel}.rs`.
- MME through the index (frmMain): `RefreshClassRaceBuilder`, `cmdClassRace_Click`,
  `txtClassRaceDetail_GotFocus`; modSyntaxsFunc `RoundUpTo5`, `Truncate`, `AutoAppend`, `PutCommas`.

## Decisions
- `class_race_builder::build` returns the exact text box (CRLF line breaks, padding included) and the
  pieces it computed (level steps, per-step numbers), so the web can show either.
- `gAvgLevelMaxSingleStat` is an input (the server computes it with `compute_avg_level_max_stats`).
- The attack columns call `calculate_attack` with the proxy weapons (-2 to -5) and a made-up profile
  (encumbrance 1/100), as MME does; no character or gear is involved.
- The view buttons are UI state (which list/box is shown); only Equip and Spellbook have effects. They
  are ported as `equip_selection` (class and race numbers that exist) and `spellbook_seed` (`NoMagic`
  or class, magery kind, magery level, and the fixed level 999 MME types in).
- `txtClassRaceDetail_GotFocus` selects all text; the web does the same on focus of the text block.
- Oracle cases run `RefreshClassRaceBuilder` itself cut out of frmMain.frm; the selected list rows are
  form controls set by the scenario. Equip and Spellbook are not oracle-runnable (they drive forms), so
  their rules are tested against MME's logic by direct cases (`GetClassMagery`, `GetClassMageryLVL`
  fixtures already exist).

## Do
- Rules `build`, `equip_selection`, `spellbook_seed`.
- Oracle family `class_race_builder` (about 30 to 50 cases, 1.5 MB or less).
- Server `GET /api/class-race-builder?class=N&race=M`, `POST /api/class-race-builder/{equip,spellbook}`.
- Web: a "Builder" panel in the Races and Classes explorers' detail area; screenshot
  `/tmp/P4-12/shots/class-race-builder.png`.

## Done when
Workspace and web checks pass, `oracle.py check class_race_builder` is current, the screenshot exists.
