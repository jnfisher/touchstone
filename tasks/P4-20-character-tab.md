---
id: P4-20
title: Character tab, character API and persistence
model: sonnet
phase: 4
depends: [P3-44, P3-11, P3-19]
touches:
  - bin/touchstone/src/
  - bin/touchstone/tests/
  - web/src/lib/
  - web/src/lib/components/character/
  - web/src/routes/[section=section]/
  - docs/design.md
---

## Goal
The Character tab (F10): MME's Char tab in content, in the app's look. Class, race, level and
alignment; the six stats with bless/gear-applied values and bonus labels; the CP panel; HP range and
regen; mana and regen; MR and anti-magic; dodge; picklocks; encumbrance; ten bless slots; quest
checkboxes and options; 20 equipment slots as searchable item pickers; and the derived stat list
(`lblInvenCharStat`, `docs/inventory.md` 5b). Every derived value comes from the server, which runs
`touchstone_rules::pipeline::refresh_all`. Characters are saved server-side as JSON files.

## Read
- `crates/touchstone-rules/src/pipeline.rs` (doc comment), `character.rs` (`Character`,
  `AttackConfig`), `char_panel.rs` and `gear.rs` output types.
- MME through the index (frmMain): `RefreshCPs`, `RefreshHitPoints`, `RefreshMagic`, `RefreshMagicRes`,
  `RefreshDodge`, `RefreshPicklocks`, `RefreshCharBless`, `cmdCharButtons_Click`, `FilterBlessSpells`,
  `EquipBlessSpell`; `docs/inventory.md` sections 5 and 8b (P4-20).
- `bin/touchstone/src/calc.rs` (request/response style), `web/src/lib/components/calc/` (UI style).

## Do
- Server (`bin/touchstone/src/character.rs`, routes in `api.rs`): wire format is snake_case like the
  rest of the API. The serde structs live in the binary, not in `touchstone-rules` (the vectors there
  are 47 and 201 long, which serde's array impls do not cover); `rules` is untouched.
  - `POST /api/character/evaluate`: a character (plus optional `options`: `hide_class_race`,
    `min_item_level`) in, a UI-shaped state out: the formatted captions MME shows, the 47 stat
    captions, per-panel numbers. No class or race (`null`) -> 422 "pick a class and race". The data
    version caption, `nNMRVer` and the average-max-stats global come from the loaded world, as the
    calculators do.
  - `GET /api/character/bless-spells?class=&level=&alignment=&all=`: the rule of `FilterBlessSpells`
    (it calls `SpellIsUsable`, which is not a plain predicate over spell columns), so it is
    answered by the server using `spell_cast::UsableEnv` with `bOnlyInGame` on, as MME defaults.
  - `GET /api/characters`, `GET/PUT/DELETE /api/characters/{name}`: `<data_dir>/characters/<name>.json`.
    Names: 1-64 chars, no path separators, no control or Windows-reserved characters, no leading dot
    or trailing space/dot. Atomic write (temp file in the same directory, then rename).
  - `AppState` carries the data dir (the config already has `data_dir`, default `data`).
- Web: a runes character store (`$lib/character.svelte.ts`) holding the current character, autosaved
  with a debounced PUT under its name, with a picker to load/new/delete; edits re-evaluate with a
  debounced POST and show the server's error inline. The quest and equipment-slot tables and the
  request mapping are plain TS in `$lib/character.ts` (vitest). Equipment pickers reuse the
  searchable-combobox pattern; the full EQ tab is P4-17.
- Tests: endpoint tests pin 3+ cases of `fixtures/rules/pipeline.json` by id; CRUD round trip in a temp
  dir; bad names rejected; vitest for the mapping and store.
- Screenshots (not committed): `/tmp/P4-20/shots/character-{dark,light}.png`.

## Done when
`cargo fmt/clippy/test` pass, `tools/guard.sh`, web `npm run check && npm test && npm run build`;
endpoint tests pass; screenshots exist.
