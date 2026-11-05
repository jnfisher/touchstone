---
id: P3-18
title: Gear aggregation II - stealth, quests, accuracy, crits, MR, dodge, speed
model: sonnet
phase: 3
depends: [P3-17, P3-08, P3-09]
touches:
  - crates/touchstone-rules/src/gear.rs
  - crates/touchstone-rules/src/gear/
  - crates/touchstone-rules/tests/all/gear.rs
  - fixtures/scenarios/gear2.json
  - fixtures/rules/gear2.json
  - tools/oracle/gen/gear2.py
  - tools/oracle/
---

## Goal
Port the second half of frmMain `CalcCharacterStats` (FAM-04): everything from the `'stealth`
banner to the end of the value-producing code, continuing on the `CharStats` that P3-17 returns.
The result is one entry point that runs both halves and gives the final `lblInvenCharStat(0..46)`
captions and tags and the globals the procedure leaves behind. Follow `docs/porting.md`.

## Split with P3-17
P3-17 ends before `'stealth` (see its card). P3-18 covers, in order: `'stealth`; the strength bonus
to max damage and spell damage; `'quest bonuses`; `'ACCURACY CALCULATION` (with the bash/smash
accuracy adjustment); `'CRITICAL HIT` (including Quick and Deadly); `'Magic Resistance`; `'DODGE`;
the two regen text boxes; the movement speed; and the tag pass over `lblInvenCharStat`.

## Read
- `docs/inventory.md` section 4 (FAM-04), 5b, 8a row P3-18.
- Through the index: frmMain `CalcCharacterStats` (from `'stealth`), modMMudFunc `CalculateStealth`,
  `CalculateAccuracy`, `CalcMR`, `CalcDodge`, `CalcMovementSpeed`, `GetQuickAndDeadlyBonus`.
- Already ported and called here: `stats::calculate_stealth`, `stats::calc_mr`,
  `stats::calc_movement_speed`, `accuracy::calculate_accuracy`, `accuracy::calc_dodge`,
  `attack::get_quick_and_deadly_bonus`. P3-17's `gear.rs` for the types.

## Decisions
- Stock engine only. The GreaterMUD branches (quests 6 to 11, the combat-level crit, spell damage
  from spellcasting, the movement formula, the kick and jumpkick accuracy adjustments) are not
  ported; `Profile` has no other variant. Quests 6 to 11 therefore do nothing, as in MME's stock path
  (`x < 6 Or bGreaterMUD`).
- The procedure's `On Error` handler leaves for the end: an error raised in an argument conversion
  (a stat tag beyond Integer when passed to `CalculateStealth`, say) stops the calculation there and
  is recorded in `CharStats::error`, with the state so far. The port keeps MME's argument types
  (Integer, Long) so the same inputs raise the same error 6.
- Not ported because they belong to later cards: the calls after the tag pass (`RefreshMagic`,
  `InvenColorCodeStats`, `GetDamageOutput` for `lblInvenCharStat(46)`, `RefreshMainStatBonuses`,
  `RefreshDodge`, `lblInvenCharStat_Change`, `SetHitCalcVals`). Stat 46 stays "0" here; P3-12 fills it.
- Display text is dropped: `StatTips`, `GetDodgeSummary`, `SortInvenToolTips`, the effective-MR and
  diminishing-crit tip lines (the crit local is not read after `lblInvenCharStat(7)` is set), colours.
- New inputs are a `SecondHalfInput` (quest checkboxes and the four quest option boxes, attack type,
  global filter, anti-magic is tooltip only and not read). `GearInput` is unchanged so P3-17's
  entry point and fixture are unaffected.
- Oracle: scenario `gear2` runs `CalcCharacterStats` whole, cut at `Call RefreshMagic`, and copies
  the locals the port returns (walk speed, stealth/accuracy/dodge locals) into the `tsProbe` control.

## Done when
`docs/porting.md` "Done when"; `fixtures/rules/gear2.json` and its scenario stay under 1.5 MB
together; `oracle.py check gear gear2` reports both current; the P3-17 test still passes.
