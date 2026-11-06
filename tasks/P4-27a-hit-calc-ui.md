---
id: P4-27a
title: Hit calculator panel
model: sonnet
phase: 4
depends: [P3-42, P4-28a]
touches:
  - bin/touchstone/src/calc.rs
  - bin/touchstone/src/api.rs
  - bin/touchstone/src/detail.rs
  - bin/touchstone/tests/
  - web/src/lib/calc.ts
  - web/src/lib/calc.test.ts
  - web/src/lib/components/calc/
  - web/src/routes/calc/
  - web/scripts/calc-shots.cjs
  - tasks/P4-27a-hit-calc-ui.md
---

## Goal
MME's Hit Calculator window (frmHitCalc) is the fifth entry of the Calculators menu, backed by
`POST /api/calc/hit` over `crates/touchstone-rules/src/hit_calc.rs`. The window's controls and
module variables travel in the request and come back in the response, so the endpoint is stateless.

## Read
`crates/touchstone-rules/src/hit_calc.rs`; MME `frmHitCalc` Form_Load, LoadMonsters,
cmbMonsterList_Click, optHitCalcType_Click, optAttacker_Click, optDefender_Click, txtHitCalc_Change,
chkShadow_Click, chkSeeHidden_Click, cmdRefreshMonster_Click, SetHitCalcVals, DoHitCalc,
GetMonsterData, through the index; `fixtures/rules/hit_calc.json`; P4-28a's `calc.rs`,
`calc.ts` and `components/calc/`.

## Do
1. Server: `POST /api/calc/hit`. The request names an `event` that stands for the handler MME runs:
   `calc` (a box, Shadow or See Hidden changed: DoHitCalc), `refresh` (an option button or the
   refresh-set button: SetHitCalcVals), `monster` (monster combo pick: GetMonsterData then
   SetHitCalcVals for the monster half) and `modify` with `index` 0..=11 (an arrow button:
   ModifyHitCalcValues then DoHitCalc). Decisions:
   - The monster side of the state is always rebuilt from `monster` (the module variables only ever
     change through GetMonsterData); the defender memory (`state`) is echoed by the client.
   - The enabled flags are control state written by SetHitCalcVals; the client echoes `enabled`,
     and when absent the server derives them by running SetHitCalcVals on a copy.
   - Backstab with Monster as attacker becomes Manual, as `optHitCalcType_Click` does.
   - Character-side numbers (accuracy, AC, dodge, protection, stealth, BS accuracy, agility,
     shadow AC, class, backstab weapon, bash/smash/kick) are request fields with defaults of 0,
     standing in for the loaded character until the Character tab exists.
   - 400: unknown event/option text, bad `index`, `monster` event without a monster, unknown
     monster, class or backstab weapon, `evil_index` outside 0..2.
2. Integration tests in `bin/touchstone/tests/all/calc.rs`, values pinned to
   `fixtures/rules/hit_calc.json` case ids, plus the 400 cases.
3. Web: `HitCalc.svelte` panel (monster picker over the loaded monsters table, option groups,
   six boxes with arrow buttons, Shadow / See Hidden, the three percentages and MME's text
   lines, a Character section of plain inputs), request mapping and defaults in `calc.ts` with
   vitest cases, route and menu entry.
4. `web/scripts/calc-shots.cjs` takes `hit.png` and `hit-light.png` into `/tmp/P4-27a/shots/`.

## Done when
`cargo fmt/clippy/test`, `tools/guard.sh`, and in `web/` `npm run check`, `npm test`,
`npm run build` pass; endpoint tests pass; both screenshots exist.
