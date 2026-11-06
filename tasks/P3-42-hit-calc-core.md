---
id: P3-42
title: Hit calculator core
model: sonnet
phase: 3
depends: [P3-08, P3-11, P3-02]
touches:
  - crates/touchstone-rules/src/hit_calc.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/hit_calc.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/hit_calc.json
  - fixtures/rules/hit_calc.json
  - fixtures/known_gaps.json
  - tools/oracle/gen/hit_calc.py
  - tools/oracle/
---

## Goal
Port the arithmetic of the Hit Calculator window (FAM-26) as pure functions: frmHitCalc `DoHitCalc`,
`SetHitCalcVals`, `GetMonsterData`, `GetVileWardValue` and `ModifyHitCalcValues`. The window keeps its
logic inside event code and reads and writes its own controls, so the port has a `HitCalcForm` that
mirrors those controls, a `HitCalcState` for the form's module-level variables, and functions from
(form, state, character, tables) to the new form and state. `hit_calc_input_from_character` maps a
`character::Character` and its `CharStats` to what `SetHitCalcVals` reads from the main window.

## Read
- `docs/porting.md`, `docs/inventory.md` section 4/4a/8 rows for FAM-26 and P3-42, `tools/oracle/README.md`.
- Through the index: frmHitCalc `DoHitCalc`, `SetHitCalcVals` (it lives in frmHitCalc, not frmMain),
  `GetMonsterData`, `GetVileWardValue`, `ModifyHitCalcValues`, the module declarations; callees
  `GetClassStealth`, `GetItemStrReq`, `ItemHasAbility`, `CalculateBackstabAccuracy`,
  `CalculateAttackDefense`, `GetHitMin`, `GetHitCap`, `GetDodgeCap`, `AutoAppend`,
  `GetCharActiveVileWard`, `bCharIsEvilAligned`.
- Already ported, to be called and not redone: `accuracy::{calculate_attack_defense,
  calculate_backstab_accuracy, get_hit_min, get_hit_cap, get_dodge_cap}`, `abilities::item_has_ability`,
  `stats::get_class_stealth`, `character::{Character, CharStats, get_char_active_vile_ward}`.

## Decisions
- The window's controls are plain data (`HitCalcForm`): option buttons are `Option<enum>` (none selected
  is possible and makes `SetHitCalcVals` leave early), text boxes are strings, `Enabled` flags that
  `DoHitCalc` reads are fields. Labels MME writes are strings; a label the procedure did not write keeps
  the string it had. The module variables are `HitCalcState`; `DoHitCalc` writes some of them (the
  "player" defender memory) and `SetHitCalcVals` reads them, so both functions take and return state.
- A VB6 error MME's handler catches stops the procedure where MME's `On Error ... Resume out` does and is
  reported in `errors` (source procedure, error number); what had been written by then is returned.
  `SetHitCalcVals` always finishes with `DoHitCalc` (its `out:` label), also after an error.
- Events (a Change or Click on a control fires `DoHitCalc`) are the caller's: `bDontRefresh` suppresses
  them inside `SetHitCalcVals`, and after `ModifyHitCalcValues` the caller runs `do_hit_calc`.
- Not ported: GreaterMUD branches (`bGreaterMUD` false: vile-ward visibility, `cmbEvil` use in `DoHitCalc`,
  the evil-aligned `cmbEvil` pick, GreaterMUD caps text), the monster list loading, window placement,
  fonts, colours, the repeat-button timers.
- Oracle: the five procedures and the form's module variables are cut out of `frmHitCalc.frm` with
  `frm.py`; the form's controls resolve to the frmMain shim like the P3-43 calculators. A case sets the
  controls and globals it needs; module variables are set as globals.
- Rules plus scenario stay under 1.5 MB.

## Done when
`docs/porting.md` "Done when"; `oracle.py check hit_calc` current and `oracle.py check` for the other
families still current; every `hit_calc` fixture case passes.
