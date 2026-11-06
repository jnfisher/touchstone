---
id: P3-43
title: Calculator cores - swings, backstab, coin, exp
model: sonnet
phase: 3
depends: [P3-09, P3-01, P3-02]
touches:
  - crates/touchstone-rules/src/calculators.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/calculators.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/calculators.json
  - fixtures/rules/calculators.json
  - fixtures/known_gaps.json
  - tools/oracle/gen/calculators.py
  - tools/oracle/
---

## Goal
Port the arithmetic of the four small calculator windows (FAM-27) as pure functions: `CalcSwings`
(frmSwingCalc), `CalcBS` (frmBSCalc), `CalcCoin` with `ConvertCoin` (frmCoinConvert) and `CalcExp`
(frmExpCalc). The forms keep this arithmetic inside the event code and read their controls, so each
function takes an input struct that mirrors the form's fields (the text of each box, the selected
item of each combo) and returns an output struct that mirrors what the form shows, including the
text MME builds. Follow `docs/porting.md`.

## Read
- `docs/inventory.md` section 4 and 4a (FAM-27), the 8a row for P3-43 and the FEAT-SWING, FEAT-BS,
  FEAT-EXPCALC and FEAT-COIN rows.
- Through the index: frmSwingCalc `CalcSwings`; frmBSCalc `CalcBS`; frmCoinConvert `CalcCoin`,
  `ConvertCoin` and the `eCoins` enum in `_declarations`; frmExpCalc `CalcExp`; modMain
  `PopulateCharacterProfile` (what the global filter fills in), `GetAbilityStatSlot`.
- Already ported: `attack::{calc_energy_used, calc_quick_and_deadly_bonus, calc_bs_damage,
  calculate_attack}`, `stats::{calc_encumbrance_percent, adjust_speed_for_slowness}`,
  `abilities::get_ability_stat_slot`.

## Decisions
- Inputs are plain data. Tables come in as `&[Item]`, `&[Class]`, `&[Race]`; the weapon is the number
  `cmbWeapon.ItemData(ListIndex)` (`None` for no selection). Text boxes stay `&str` and are read with
  `Val`, as MME does. Globals (`nGlobalCharWeapon*`, `chkGlobalFilter`) are fields.
- `CalcSwings` with the global filter on calls `PopulateCharacterProfile`, which belongs to the
  character glue. The input carries the resulting `CharacterProfile` instead, and `AttackEnv` comes
  from the caller as it does for `calculate_attack`.
- Outputs are the strings MME writes to labels and boxes (`"Energy per swing: 450"`, the clamped
  text of an input box, `"+5-2"` adjustment captions). A label the procedure did not write is an
  empty string. A run-time error MME's handler (or the caller) leaves the procedure on stops at the
  same point and is reported in `error`, with the state so far returned.
- Not ported: the GreaterMUD / Paramud branches (`bGreaterMUD` is false throughout), `bDontRefresh`,
  the Paste-MegaMUD parser, the exp table listing (`cmdCalcExp_Click`), colours and tool tips.
- Oracle: `frm` in a scenario may be a list of form specs (one per form file) and a spec may name
  `enums` to cut out (frmCoinConvert's `eCoins`). All four forms' controls resolve to the frmMain
  shim, which is fine because a case runs one calculator. Cast-spell text for weapons comes from
  recorded `GetSpellName` / `PullSpellEQ` cases, as in the attack family.

## Done when
`docs/porting.md` "Done when"; `fixtures/rules/calculators.json` and its scenario stay under 1.5 MB
together; `oracle.py check` reports every family current.
