---
id: P3-17
title: Gear aggregation I - encumbrance, bless, equipped items
model: sonnet
phase: 3
depends: [P3-02, P3-03]
touches:
  - crates/touchstone-rules/src/gear.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/gear.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/gear.json
  - fixtures/rules/gear.json
  - tools/oracle/gen/gear.py
  - tools/oracle/
---

## Goal
Port the first half of frmMain `CalcCharacterStats` (FAM-04) as a pure function over a `GearInput`
that returns a typed `CharStats` replacing `lblInvenCharStat(0..46)`, `txtCharStats(i).Tag` and
the `nGlobalChar*` globals. Follow `docs/porting.md`.

## Split with P3-18
The procedure's section banners, in order: manual adjustment and class/race abilities, `'ENCUM FIRST`,
rest of manual and bless stats, `'equipped items`, (hit magic and shadow AC), `'stealth`, `'quest
bonuses`, `'ACCURACY CALCULATION`, `'CRITICAL HIT`, `'Magic Resistance`, `'DODGE`, then tooltips and
refresh calls. P3-17 ends just before `'stealth` (after the equipped-items loop, the weapon hit-magic
fold-in and the shadow AC). P3-18 continues on the same `CharStats`; the locals it needs
(`encum_pct`, `class_stealth`, `race_stealth`) are fields.

## Read
- `docs/inventory.md` section 4 (FAM-04), 4a, 5a, 5b, 5c.
- Through the index: frmMain `CalcCharacterStats` (lines 1-698), `InvenResetStats`, `InvenCalcEncum`,
  `AdjMainStatBonus`, `RefreshCharBless`, `ItemIsUsableByChar`, `ItemIsCarryActive`,
  `ItemIsFlaggedCarried`, `AccumItemStatBonus`, `PastedStatIsBuffed`.
- Already ported: `abilities::get_ability_stat_slot`, `stats::calc_encum`, `stats::get_race_hp_bonus`,
  `spells::get_current_spell_min_max`.

## Decisions
- `RefreshCharBless` (listed under P3-19) and `AdjMainStatBonus`/`ItemIsUsableByChar` (listed under
  P3-18) are ported here because this half cannot run without them. P3-19 adds the mana-per-round total.
- Inputs are `Character` (level, class, race, alignment, min item level), `base_stats`, the 20
  `equipped` slots, carried Item Manager rows (`carried_items` parses the CARRIED / ` xN` text),
  `adjustments` (`char_StatAdjustments`), ten bless spell numbers, and the martial-arts flag.
  Items, spells, classes and races come from `&World`.
- Captions are stored as `Val(CStr(x))` after each update (MME re-reads label text). AC/DR sums use
  `f32` where MME declares `Single`.
- An error that MME's handler leaves the procedure on (for example a bless spell of duration 0,
  encumbrance percent beyond Integer) stops at the same point; the state so far is returned with
  `CharStats::error`.
- Display text is not ported: StatTips, tool tips, colours, `cmbEquip` re-selection.
- Oracle: `frm` scenario key runs procedures cut out of `frmMain.frm` (`tools/oracle/frm.py`); the
  scenario cuts `CalcCharacterStats` at `'stealth` and copies its locals into a probe control.
  `db_extra` adds invented rows (numbers 9001+) to reach branches no stock row has.

## Done when
`docs/porting.md` "Done when"; `fixtures/rules/gear.json` and its scenario stay under 1.5 MB together;
`oracle.py check` reports every family current.
