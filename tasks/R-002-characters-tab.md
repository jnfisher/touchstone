---
id: R-002
title: Characters tab, realm list, sheet, sandbox and the active-character picker
model: sonnet
phase: realm
depends: [R-001, P4-20]
touches:
  - web/src/lib/components/character/
  - web/src/lib/
  - web/src/routes/
  - docs/differences.md
---

## Goal
The Characters tab's main view lists every character of the realm (from the live database); selecting one
shows its full stat sheet, read-only and always live, with touchstone's calculation beside the game's stored
values and differences flagged. The MME-style editor stays as a what-if Sandbox that a realm character can
be copied into (paste and `.mmec` import/export live inside it). One global active character, picked in the
header, drives everything that uses "the character".

## Read
- `tasks/R-001-realm-characters.md`, `docs/design.md` (Realm characters), `docs/differences.md` (Realm
  characters), the endpoints in `bin/touchstone/src/realm.rs`.
- `web/src/lib/character.svelte.ts`, `components/character/CharacterTab.svelte`, `components/ChestsTab`
  (split layout), `grid/DataGrid.svelte`, `info.svelte.ts` (the load number).

## Decisions
- `createCharacterStore` stays the editor (the Sandbox) and gains a `readOnly` option (no autosave) for a
  second instance that holds the active realm character. `getCharacterStore()` now returns a facade with the
  same members that reads whichever of the two is active; `getSandboxStore()` is the editor itself.
  Replacing the whole character (paste, new, load, import) goes to the sandbox and makes it active; an edit
  made through the facade to a realm character (a menu's "set as combat spell") stays in memory (never saved) until that character changes in the
  realm or another is picked, as the realm is the source of truth.
- Realm data lives in its own store (`realm.svelte.ts`): the roster, the selected sheet and a `refresh()` the
  layout calls whenever the info load number changes, so the list, the open sheet and the active character
  follow a live reload.
- The choice persists per browser (`touchstone.active-character`: `sandbox` or `realm:<id>`); a missing
  character falls back to the sandbox. An `.mdb` source has no realm: the list says so, the picker offers only
  the Sandbox.
- "Copy to sandbox" loads the realm input into the editor through the paste path (the previous sandbox
  character is saved first) under the character's name, made unique among the saved ones so a copy never
  overwrites a saved what-if; the sandbox is shown, the active character is left alone.
- The sheet's cause tooltips word the R-001 cause codes in our own words (the codes are the contract).
- Unit tests cover the pure parts (filters, the sheet view model, cause wording) and the stores; there is no
  component-test harness in this repository, so rendering is checked through the screenshots.

## Do
Realm view (grid + split sheet), sheet, Sandbox view, header picker, stores and client, tests, screenshots
(`/tmp/R-002/shots/`: `realm-list.png`, `realm-sheet-light.png`, `active-picker.png`, `sandbox.png`).

## Done when
`npm ci && npm run check && npm test && npm run build` pass in `web/`, the workspace checks pass, and the
screenshots exist.
