---
id: P3-41
title: Paste party
model: sonnet
phase: 3
depends: [P3-38]
touches:
  - crates/touchstone-rules/src/paste_party.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/paste_party.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/paste_party.json
  - fixtures/rules/paste_party.json
  - tools/oracle/gen/paste_party.py
  - tools/oracle/
---

## Goal
Port FAM-25's party half (stock only): frmPasteChar `ParsePasteParty` and `CalculateAverageParty`, plus the
`TestPasteChar` filter they call. Party text in (MegaMUD party display or the game's stat and inventory output,
one character after another), per-member rows and the party averages out. The averages are what the party
monster-damage filter (P3-16, P4-26) reads. Follow `docs/porting.md`.

## Read
- `docs/porting.md`, `docs/inventory.md` rows P3-41 and FAM-25, `tools/oracle/README.md`.
- MME through the index: frmPasteChar `ParsePasteParty`, `CalculateAverageParty`, the `PartyCalc` enum and
  `bHoldPartyRefresh` (declarations); modMMudFunc `TestPasteChar`; modSyntaxsFunc `RemoveCharacter`,
  `RegExpFindv2`; modMMudDatabase `GetSpellByShort`, `ClassHasAbility`, `RaceHasAbility`.
- Already ported, to be called and not redone: `stats::{calc_resting_rate, calc_mana_regen, get_class_combat,
  get_class_magery{,_lvl}}`, `accuracy::{calc_dodge, calculate_accuracy}`, `abilities::{class,race}_has_ability`,
  `attack::{calc_energy_used, calc_quick_and_deadly_bonus, calculate_attack}`,
  `spell_cast::{SpellCastEnv::calculate_spell_cast, UsableEnv}`.

## Decisions
- The procedures write straight into frmPasteChar's text boxes, check boxes and option buttons. The port keeps
  that shape as a plain `PartyForm` (index 0 is the average box, 1 to 6 the members), taken in and given back, so
  state MME leaves alone (heals, damage, swings, spell damage, the attack-last option) survives as it does there.
- `ParsePasteParty`'s two dialogs become inputs and outputs. The "no matching data" `MsgBox` answers its default
  (No) unless the caller says otherwise; each member's `InputBox` attack prompt is an answer taken from
  `attack_answers` (`None` is "accept the default"); the prompts MME would show, and every `MsgBox` text, come back
  in `PasteOutcome`.
- The regular expression uses a look-ahead, which the `regex` crate lacks, so the matcher is written out by hand
  (leftmost, alternatives in order, ASCII classes, ignore-case keywords). The `Select Case` on the captured keyword
  is case-sensitive, so a lower-case `hits:` matches the pattern and is then ignored, as in MME.
- Text is walked as UTF-16 units and the loop counters are 16-bit like MME's `Integer`s, so a paste of 32768
  characters or more overflows into the error handler. A VB6 error stops the procedure where it happened; the
  `out:` tail (recount the averages) still runs. Quirks are kept: per-field slot counters re-synced at each `Name:`,
  duplicate `Case 34` (the dodge branch for items is unreachable), a skipped second-wrist item abandoning the
  rest of its item scan, the stale `nCombat` and `nEnergy` carried between members.
- Item rows, classes, races, spells, the class and race combo contents and the globals involved (`bOnlyInGame`,
  `bInvenUse2ndWrist`, `gAvgLevelMaxAllStats`) are inputs. The rows are the fixture's `data` (the scenario holds
  them once). Weapons that cast (ability 43) are left out of the scenarios: the cast text needs the spell-text
  tables, and `CalculateAttack` is already fixture-tested with them.
- Oracle: `ParsePasteParty` and `CalculateAverageParty` are cut from `frmPasteChar.frm`; one `subs` line adds probe
  code that copies the locals the form never stores (tCharacter, weapon, accuracy sums) into captions. Two small
  harness additions: an `InputBox` hook that answers from the case's `inputbox` list (default when `null`), and
  `$radio` control arrays, where setting one `Value` true clears its siblings as VB does.

## Done when
`docs/porting.md` "Done when": `cargo test --workspace` passes, `oracle.py check` reports every family current,
`fixtures/rules/paste_party.json` and its scenario stay under 1.5 MB together, 200 to 500 cases, and the family's
test runs every case.
