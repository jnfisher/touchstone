---
id: P3-40
title: Paste character
model: sonnet
phase: 3
depends: [P3-38, P3-39, P3-17, P3-02]
touches:
  - crates/touchstone-rules/src/paste_char.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/src/textblock.rs
  - crates/touchstone-rules/tests/all/paste_char.rs
  - crates/touchstone-rules/tests/all/main.rs
  - tools/oracle/gen/paste_char.py
  - tools/oracle/shims.py
  - tools/oracle/harness.py
  - fixtures/scenarios/paste_char.json
  - fixtures/rules/paste_char.json
---

## Goal
Port the character half of FAM-25: frmMain `PasteCharacter`, `PasteInventoryManager`, `PasteSpells`,
`ApplyPastedStat`, `PastedStatIsBuffed`, `PasteParty`, and modMMudFunc `TestPasteChar` and
`TestAlphaChar`. MME writes straight into Char-tab controls; the port takes what those controls and
globals hold ([`PasteState`]) and returns the new state plus the boxes MME would show ([`PasteOutcome`]),
so the web Paste Character dialog (P4-26) can preview and apply it.

## Read
- `docs/porting.md`, `docs/testing.md`, `docs/inventory.md` FAM-25 and P3-40, `tools/oracle/README.md`.
- MME through the index: frmMain `PasteCharacter`, `PasteInventoryManager`, `PasteSpells`,
  `ApplyPastedStat`, `PastedStatIsBuffed`, `PasteParty`, `InvenAddEquip`, `cmbEquip_Click`,
  `cmbGlobalRace_Click`, `cmbGlobalClass_Click`, `ItemIsFlaggedCarried`; modMMudFunc `TestPasteChar`,
  `TestAlphaChar`, `SpellIsUsable`; modSyntaxsFunc `ExtractValueFromString`, `RemoveCharacter`,
  `AutoAppend`; frmPasteChar `cmdPaste_Click`, `cmdContinue_Click`.
- Already ported, called and not redone: `game_text` (`parse_game_text_inventory`,
  `item_name_in_parse_result`, `populate_item_manager_from_parsed`), `spell_cast::UsableEnv`,
  `gear::item_is_flagged_carried`, `textblock` `extract_value`.

## Decisions
- Stock only. `bGreaterMUD` is false, so the item stat-bonus block of `PasteCharacter`
  (`AccumItemStatBonus`, the carried-stat anomaly text) is skipped and `ApplyPastedStat` always gets a
  bonus of 0; the Paramud/GreaterMUD branch is not ported.
- `PasteParty` only opens `frmPasteChar` in party mode and has no logic; it is UI for P4-26 and has no
  Rust function. `ParsePasteParty` and `CalculateAverageParty` are P3-41. `TestPasteChar` and
  `TestAlphaChar` are ported here as pure functions (P3-41 can call them).
- The dialog itself (`Show vbModal`, the Cancel/Continue tag, the placeholder text) is UI. The text
  the user pasted is the input; fewer than 10 characters cancels, as in MME.
- Events MME relies on when it assigns a control are ported where they change state:
  `cmbEquip_Click` (records `nEquippedItem`), `cmbGlobalRace_Click` (raises stats below the race minimum,
  then clears `bDontRefresh`, which makes a later class change ask "Reset learned spells?"), and
  `cmbGlobalClass_Click` (keeps the two class combos in step). VB6 raises a combo's Click only when the
  selection really changes, so assigning the same index is a no-op. Refresh-only handlers are not.
- Combo contents are explicit state: each `cmbEquip(x)` is the list of item numbers it holds after its
  "None" entry; `InvenAddEquip` appends to it (rings go to both finger combos, wrists to both wrist
  combos), and the outcome returns the lists. The class and race combos are the world's tables behind
  an "Any" entry at index 0, as `LoadClasses`/`LoadRaces` build them.
- Prompts become an `Asker` (Yes/No/Cancel and InputBox answers in order) and every box MME shows is an
  `Event` in order. `SaveCharacter` is an `Event::Save` (success assumed).
- The Item Manager is `Vec<ListRow>` (P3-39). `PasteInventoryManager`'s final sort by the Value tag and
  the colour refresh are presentation: the outcome carries `sort_after`, the rows stay in insertion
  order, and the oracle's sort is a no-op. P4-26 sorts.
- `frmMonsterAttackSim` AC/DR boxes: the outcome carries `sim_ac`/`sim_dr`; the caller applies them
  when that form is open (the oracle says it is).
- Oracle: `PasteCharacter` and friends are cut from `frmMain.frm`; the dialog lines, `Load`, the
  combos' `Click` events and the final sort are replaced through `subs`. Shims added: a ComboBox
  (`AddItem`, `NewIndex`, `ItemData`, `ListCount`) and `UBound`/`item` on control arrays, an InputBox
  hook, and a `%f.` read of another form's control. Every other family's fixture stays byte-identical.

## Done when
`docs/porting.md` "Done when"; `oracle.py check` current for all families; every case passes.
