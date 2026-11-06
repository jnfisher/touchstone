---
id: P4-29
title: Settings dialog and exp/hr knobs (with P4-30's options table)
model: sonnet
phase: 4
depends: [P3-26]
touches:
  - bin/touchstone/src/
  - bin/touchstone/tests/
  - web/src/lib/
  - web/src/lib/components/settings/
  - web/src/lib/components/ui/dialog/
  - web/src/routes/+layout.svelte
  - web/scripts/
  - docs/design.md
  - tasks/P4-29-settings.md
---

## Goal
MME's Settings window (frmSettings) as a dialog in the shell: the exp/hr models A to D, the
recovery-only and show-all switches, the four knobs (global, or per character), monster sim rounds,
route waypoint interval and auto-load of the last character. Options that change computed results
live on the server (`GET/PUT /api/settings`, a JSON file in the data directory); view options live
in browser storage. P4-30's job (which settings.ini options survive and where) is the table below.

## Read
- MME through the index: `frmSettings` (Form_Load, PopulateExpFields, cmdSave_Click,
  cmdModelQ_Click, cmdCEPHA_Q_Click, cmdCEPHB_Q_Click, chkExpHrCalcByCharacter_Click, text box
  KeyPress handlers), `frmMain` (LoadExpPerHourKnobs, LoadSettings, SaveSettings), `modSettings`
  (ReadINI, CleanSettings).
- `crates/touchstone-rules/src/exp_per_hour/dispatch.rs`, `docs/design.md`, `bin/touchstone/src/character.rs`
  (atomic store pattern), `web/src/lib/theme.svelte.ts` (view-option pattern).

## Do
1. Server `settings.rs`: `Settings { exp_per_hour, monster_sim_rounds, route_waypoint_interval,
   auto_load_last_char }`, `ExpHrSettings` (four models, recovery_only, show_all, four knobs,
   by_character). `PUT` applies the rules `cmdSave_Click` applies rather than rejecting: knobs
   `clamp_knob` (rounded to 2 places, 0.01 to 10), sim rounds 100 to 10000, waypoint interval below 1
   becomes 25 and above 1000 becomes 1000, no model ticked turns A, B and C on (`normalized`). 
   malformed body or unknown field is a 400. The reply is what was stored. Defaults are MME's code
   behaviour: all four models on, knobs 1, 500 rounds, interval 25, auto-load on. Atomic write
   (temp file + rename) to `<data_dir>/settings.json`; a missing file means defaults, a corrupt one is
   reported by GET as 500 and never silently replaced.
2. Per character: `CharacterDto.exp_hr` (optional; models, switches and knobs, clamped the same way)
   is MME's per-character `ceph*` keys. `AppState::effective_exp_per_hour(character)` returns the
   character's block when `by_character` is on and the character has one, else the global block, as an
   `ExpPerHourSettings` for `calc_exp_per_hour`. `GET /api/settings?character=NAME` also returns this
   `effective` block. No endpoint computes exp/hr per row yet, so nothing else reads it today.
3. Web: `settings.svelte.ts` store (load, edit draft, live validation mirroring the server clamps,
   save), `components/settings/SettingsDialog.svelte` with sections like MME (Exp/hr models, Knobs,
   Simulation and routes, Character, View), help texts in our own words, gear button next to
   Calculators. By-character saving writes the block to the open character through the character
   store; otherwise to the global settings. `auto_load_last_char` gates `openLastCharacter`.
   View options (`touchstone.view`, try/catch): "character name in the page title" (MME NameInTitle);
   theme stays on its own key and gets a toggle in the dialog.
4. Tests: server validation, clamps, persistence round trip, corrupt file, effective settings; Vitest
   for the store and validators. Screenshot `/tmp/P4-29/shots/settings.png` (1440x900, dsf 2, dark).

## settings.ini options (P4-30)
Decision per key: **server** (changes computed results, stored in `settings.json`), **browser**
(view only, local storage), **later** (a view option for a feature another card ports; decided here,
built there), **n/a** (Windows, files, window geometry, or superseded).

| INI key | MME meaning | Decision |
|---|---|---|
| LoadItems, LoadSpells, LoadMonsters, LoadShops (cmdAll/cmdNone) | skip loading tables to save memory | n/a: the server loads everything |
| OnlyInGame | hide records not in game | later (browser, Explorer filter) |
| FilterAll | apply the global filter right after auto-load | n/a: the global filter is a per-character flag already |
| AutoLoadLastChar | open the last character at start | server `auto_load_last_char` |
| AutoSaveLastChar | save the character on exit | n/a: the Character tab autosaves |
| SwapMapButtons | swap map mouse buttons | later (browser, Map card) |
| NoAlwaysOnTop, DisableWindowSnap, SwapWindowTitle | window behaviour | n/a |
| HideRecordNumbers | hide record numbers in lists | later (browser, grid) |
| Use2ndWrist | second wrist slot in Equip | later (server, with the Equip card that reads it) |
| NameInTitle | character name in the title bar | browser `name_in_title` |
| DontSpanNavButtons | back/forward buttons span tabs | later (browser, navigation) |
| RemoveListEquip, JumpToCompare, ShopsFirstInRefs | list/compare behaviour | later (browser) |
| AutoCalcMonDamage | compute monster damage automatically | later (browser: a performance toggle) |
| DontLookupMonsterRegen | skip the monster-regen lookup | later (server, with the lair card that reads it) |
| MobPrintCharDamageOutFirst | order of the two damage lines in monster detail | later (server, detail text) |
| DarkMode | dark theme, needs restart | browser: existing theme toggle, no restart |
| MonsterSimRounds | attack-sim rounds (100 to 10000, default 500) | server `monster_sim_rounds` |
| RouteWaypointInterval | MegaMUD route waypoint spacing (1 to 1000, default 25) | server `route_waypoint_interval` |
| ExpPerHourKnobsByCharacter | knobs stored in the character file | server `exp_per_hour.by_character` |
| cephModelA..D, cephRecovery, cephShowAll, cephDMG, cephMana, cephMove, cephXP | exp/hr models, switches, knobs | server `exp_per_hour.*`; per character in `CharacterDto.exp_hr` |
| LastCharFile, Recent1..5, DataFile | file paths | n/a: characters live in the data directory, the source in touchstone.toml |
| Top, Left, Width, Height, Maximized, *Split* | window and splitter geometry | n/a (splitters already use their own browser keys) |
| MapFollowMap, MapNoHidden, MapNoLairs, MapNoNPC, MapNoCMD, MapNoTooltips, MapOverwrite, MapShowAllExits, MapDrawDupes, MapAlsoMark | map options | later (browser, Map card) |
| UseGlobalFilter | global filter checkbox | n/a: per-character `global_filter` |
| LabelFont*/StatFont* | fonts | n/a |
| INI plumbing (CleanSettings, CreateSettings, CheckINIReadOnly, ResolveSettingsPath, Recreate INI) | settings.ini upkeep | n/a: replaced by `settings.json` and touchstone.toml |

## Done when
`cargo fmt --check`, clippy, `cargo test --workspace`, `tools/guard.sh`, and in `web/` `npm run check`,
`npm test`, `npm run build` pass; the settings endpoints and effective-settings rules are tested; the
screenshot exists.
