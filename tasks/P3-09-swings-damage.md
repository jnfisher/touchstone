---
id: P3-09
title: Swings, energy and weapon damage
model: sonnet
phase: 3
depends: [P3-01, P3-02, P3-03, P3-04, P3-08]
touches:
  - crates/touchstone-rules/src/attack.rs
  - crates/touchstone-rules/src/attack/
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/attack.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/attack.json
  - fixtures/rules/attack.json
  - tools/oracle/gen/attack.py
---

## Goal
Port FAM-07 (stock only): the procedures listed for P3-09 in `docs/inventory.md` section 8a
(CalculateAttack, CalcEnergyUsed and its Encum/Speed adjusters, CalcQuickAndDeadlyBonus,
GetQuickAndDeadlyBonus, CalcBSDamage, CalcTrueAverage), with the inputs in sections 4 and 4a.
Follow `docs/porting.md` exactly.

## Decisions
- `CharacterProfile` is plain data mirroring the `tCharacterProfile` fields CalculateAttack reads;
  the `nGlobalChar*` weapon arrays are an `EquippedGlobals` struct. P3-11 builds both from the
  character later. Item, class and race rows come in as model slices.
- Other cards' procedures are called from their modules (`stats`, `abilities`, `accuracy`, `exp`).
  The weapon's cast-spell text (GetSpellName, PullSpellEQ) comes in through a `CastText` trait; the
  fixture records the oracle's answers for the spells it needs.
- The crate takes no regex dependency, so the three spell-text patterns run on a small private
  backtracking matcher (`src/attack/`).
- Any run-time error inside CalculateAttack gives the all-zero result, as MME's handler does.
- The one oracle change allowed: a `RegExpFindv2` shim (VBScript.RegExp is not modelled) and
  fixed-array UDT fields. Say so in the report.

## Done when
`docs/porting.md` "Done when"; the fixture covers every attack type, the proxy weapons, martial
arts, the gear swap (one- and two-handed), Quick and Deadly, crits, the 5-swing cap and each
spell-text layout.
