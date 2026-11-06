---
id: P3-19
title: Char panel derivations - HP, mana, CPs, picklocks, MR, class defaults
model: sonnet
phase: 3
depends: [P3-17, P3-18, P3-01, P3-02, P3-04]
touches:
  - crates/touchstone-rules/src/char_panel.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/char_panel.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/char_panel.json
  - fixtures/rules/char_panel.json
  - tools/oracle/gen/char_panel.py
  - tools/oracle/
---

## Goal
Port the formulas that live in frmMain's Char-tab event code (FAM-05): the Refresh* procedures that turn
the aggregate stats (`CharStats`, P3-17/18) and the typed stats into the values the panel shows, and
`SetupClass`. MME keeps these in control `.Text`/`.Caption`/`.Tag` strings; the port takes plain numbers
and returns a typed struct per panel. Follow `docs/porting.md`.

## Read
- `docs/inventory.md` section 4 (FAM-05), 5a, 5b, 5c, 6, 8a row P3-19.
- Through the index (frmMain): `RefreshCPs`, `RefreshHitPoints`, `RefreshMagic`, `RefreshPicklocks`,
  `RefreshMagicRes`, `RefreshDodge`, `RefreshMainStatBonuses`, `RefreshCharBless` (the mana-per-round
  total only), `SetupClass`, `lblInvenCharStat_Change`, `CharStatAdjustmentPrompt`, `InvenResetStats`.
- Already ported and called here: `stats::{calc_resting_rate, calc_max_hp, calc_mana_regen, calc_max_mana,
  calc_spell_casting, calc_picklocks, calc_mr, calc_cp_level, get_race_cp, get_class_*}`,
  `exp::calc_exp_needed_by_race_class`, `spells::get_current_spell_min_max`, `gear::CharStats`.

## Decisions
- Inputs are plain: level, class and race numbers (0 or less is the combo's "(none)" entry; a race of
  `None` is `ListIndex < 0`), the `CharStats` the gear cards return, the typed stat texts, the
  `txtCharMaxStats(i).Tag` ceilings. The full `Character` type belongs to P3-11 and is not used.
- Stock engine only; `bGreaterMUD` branches (CP cost cap of 9999, quest options 6th/dread/renfry) are
  skipped as in MME's stock path.
- `RefreshCharBless`'s stat totals are P3-17's `refresh_char_bless`; this card adds the mana-per-round
  total (`lblCharBless`). `InvenResetStats` is `CharStats::reset` (P3-17); its INI read of the saved
  weapon number is IO and is not ported.
- Not ported (display only): tool tips, control enable states, colours, font sizes, the
  `txtInvenStrength`/`txtShopCharm` mirroring, `GetDodgeSummary` and `PutCommas` text (the exp
  requirement is returned as a number), `PopUpChooseCombatGUI`, `RefreshAll` after an adjustment.
- An error MME's handler leaves a procedure on stops there; the port returns what was computed so far
  with an `error` field, as the gear cards do.
- Oracle: scenario key `frm` cuts the procedures from `frmMain.frm`; controls come from the case's forms
  and the captions/tags the procedures write are read back. One minimal oracle addition: a `subs`
  entry in `frm.py` replaces whole lines of a cut procedure (used to feed `CharStatAdjustmentPrompt`
  its `InputBox` answer from a control). Invented class, race and spell rows (9001+) reach branches no
  stock row has.

## Done when
`docs/porting.md` "Done when"; `fixtures/rules/char_panel.json` and its scenario stay under 1.5 MB
together; `oracle.py check char_panel` reports it current; the gear and gear2 tests still pass.
