---
id: P4-28a
title: Calculators UI I: exp, coin, swing, backstab
model: sonnet
phase: 4
depends: [P3-01, P3-43, P4-01a]
touches:
  - bin/touchstone/src/calc.rs
  - bin/touchstone/src/api.rs
  - bin/touchstone/tests/
  - web/src/lib/components/calc/
  - web/src/lib/calc.ts
  - web/src/routes/
  - web/src/lib/api.ts
  - web/scripts/
  - tasks/P4-28a-calculators-ui-1.md
---

## Goal
MME's Exp, Coin, Swing and Backstab calculator windows exist as web panels, backed by
`POST /api/calc/{exp,coin,swing,backstab}` over the ported cores in
`crates/touchstone-rules/src/calculators.rs`. Fields, defaults and output layout follow the MME
forms; the look is the app's shadcn-svelte style in dark and light.

## Read
`docs/design.md`; `crates/touchstone-rules/src/calculators.rs`; MME `frmExpCalc` (Form_Load,
CalcExp, cmdCalcExp_Click), `frmCoinConvert` (Form_Load, CalcCoin, txtCoin_Change),
`frmSwingCalc` (Form_Load, LoadWeapons, optSpeed_Click, txtStrength_Change),
`frmBSCalc` (Form_Load, LoadWeapons, CalcBS), through the index; `bin/touchstone/src/calc.rs`
and `web/src/lib/components/ExpTable.svelte` for style.

## Do
1. Server (`calc.rs`, routes in `api.rs`): JSON request/response structs for the four cores.
   Numeric text boxes arrive as strings or numbers (MME reads them with `Val`); a bad selector
   (combat outside 1..5, unknown weapon, coin kind outside 0..4) is a 400. Decisions:
   - exp: the table number comes from class and race (`calc_exp`) unless the request gives one
     (MME's box is editable). The level list follows `cmdCalcExp_Click`: start clamps to 2..500,
     end to 10..500, "needed" is the difference from the previous row, the first row counting
     from 0 (MME's `nLastExp` starts at 0). The loop stops at the first VB error.
   - coin: two boxes and two rows of coin buttons; the response carries the other box, the coin
     it was converted to and the "Weight" labels (`Fix(n/3)` while above 0). Charm button needs
     a loaded character and is left for the Character tab.
   - swing: `AttackEnv` is built over the world with no character (global filter off) and an
     empty equipped set; the dat version caption is the data file version. "Max Enc." follows
     strength like `txtStrength_Change` when `max_encum_auto` is set.
   - backstab: global filter off, no equipped weapon; the weapon list is weapons with ability
     116 (BS accuracy).
2. Integration tests in `bin/touchstone/tests/all/calc.rs`, values pinned to
   `fixtures/rules/calculators.json` cases; 400 cases for each endpoint.
3. Web: a Calculators button next to the tabs (menu of four) and route `/calc/<name>`; one panel
   per calculator, inputs left, results right, debounced live recompute. Swing and backstab
   have a searchable weapon picker over the already loaded items table and plain inputs for the
   character fields with MME's defaults (the Character tab does not exist yet). Request mapping
   lives in `web/src/lib/calc.ts` with vitest cases.
4. Screenshots by `web/scripts/calc-shots.cjs` into `/tmp/P4-28a/shots/` (not committed).

## Done when
`cargo fmt/clippy/test`, `tools/guard.sh`, and in `web/` `npm run check`, `npm test`,
`npm run build` pass; each endpoint has tests; the four screenshots exist.
