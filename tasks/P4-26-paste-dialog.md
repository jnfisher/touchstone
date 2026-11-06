---
id: P4-26
title: Paste Character dialog
model: sonnet
phase: 4
depends: [P3-40, P3-41, P4-20]
touches:
  - bin/touchstone/src/
  - bin/touchstone/tests/
  - web/src/lib/
  - web/src/lib/components/paste/
  - web/src/lib/components/character/
  - docs/design.md
---

## Goal
MME's Paste Character window (frmPasteChar, FEAT-PASTE): the user pastes the game's `stat`,
`inventory` and `spells` output, or a party display, and touchstone fills the character, showing
what MME shows (prompts, messages, the parsed result) before anything is applied.

## Read
- `crates/touchstone-rules/src/paste_char.rs` (`paste_character`, `Prompter`, `PasteEvent`,
  `PasteOutcome`, `PasteState`) and `paste_party.rs` (`parse_paste_party`, `PasteInput`,
  `PasteOutcome`, `calculate_average_party`), and the follow-ups in `tasks/P3-40-*.md`, `P3-41-*.md`.
- MME through the index: frmPasteChar `cmdPaste_Click`, `cmdContinue_Click`, `cmdCancel_Click`,
  `Form_Load`; `bin/touchstone/src/character.rs` (the DTO, `evaluate`), `web/src/lib/character*.ts`.

## Do
- Server (`bin/touchstone/src/paste.rs`):
  - `POST /api/paste/character` {character, text, answers[], rows?, char_loaded?, unequip_missing?}
    -> {character, events, prompts, needs_answer, sort_after, rows, ...}. The port asks through a
    `Prompter`; `answers` is the list of replies to the questions MME asks (Yes/No/Cancel boxes and
    InputBoxes), in order. When a question has no reply yet the run stops, discards its result and
    answers with `needs_answer` (the prompt), and the page re-posts with the reply appended. Stateless.
  - The Char-tab state the port wants is built from the stored character: stat boxes from `stats`,
    the charm tag and its bonus from `refresh_all`, one entry per equipped item in the slot combos,
    the Item Manager rows from `carried`/`stash` (or the `rows` the page sent), autosave on.
  - `PasteEvent::ClearCharacter` (the "Unload current character file first?" Yes) resets the
    Char tab: the paste is run again from a blank character, with the answer that caused the reset
    dropped (it is not asked a second time because the character is no longer loaded).
  - `POST /api/paste/party` {text, answers[], no_data_continue, attack_answers[], ...}
    -> {members[], averages, messages, prompts, needs_answer, filter}. `no_data_continue` and the
    attack answers are the party card's inputs; `filter` is `cmdContinue_Click`'s result (what the
    party filter fields become), including the "Update the [HEALS] field?" question.
- Web: a modal (`components/paste/PasteDialog.svelte`) with a Character tab and a Party tab, a big
  paste box, "Paste from clipboard" (MME's "Clear paste area first?" question when the box already
  holds text), a preview of what changes, prompts as dialogs, and Apply, which writes into the
  character store (autosaves). Sorting the Item Manager by Value (descending) when `sort_after`
  is set is done in `paste.ts`; the Char tab gets a "Paste character" button.
- Tests pinned to case ids of `fixtures/rules/paste_char.json` and `paste_party.json`; Vitest for
  the client and the prompt loop.

## Done when
Workspace and web checks pass; endpoint tests pass; screenshots exist in `/tmp/P4-26/shots/`.
