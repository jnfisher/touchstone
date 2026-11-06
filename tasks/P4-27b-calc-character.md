---
id: P4-27b
title: Calculators use the current character; MegaMUD stat-line paste
model: sonnet
phase: 4
depends: [P4-28a, P4-27a, P4-20, P3-39]
touches:
  - bin/touchstone/src/calc.rs
  - bin/touchstone/src/api.rs
  - bin/touchstone/tests/
  - web/src/lib/calc.ts
  - web/src/lib/calc.test.ts
  - web/src/lib/character.svelte.ts
  - web/src/lib/components/calc/
  - web/src/lib/components/character/CharacterTab.svelte
  - web/scripts/calc-shots.cjs
  - tasks/P4-27b-calc-character.md
---

## Goal
The Swing, Backstab, Hit and Coin calculators take the Character tab's current character the way
MME's windows read the main form (Form_Load of frmSwingCalc / frmBSCalc, SetHitCalcVals, the charm
button): class combat level, level, strength, agility, encumbrance, equipped weapon, max damage,
stealth, BS damage, accuracy, AC, dodge, protection, charm. A "use current character" switch (on by
default once a character is loaded) turns the fields read-only with the character's values; off, the
manual boxes work as before. The Swing calculator gains MME's "Paste MegaMUD stat line" and the
true-average readout, backed by `POST /api/calc/mega-stat`.

## Read
`tasks/P4-28a-*.md`, `tasks/P4-27a-*.md` (which fields were stand-ins); `web/src/lib/character.svelte.ts`,
`character.ts` (`EvalState`); MME frmSwingCalc `Form_Load`, `cmdPasteMega_Click`, `txtTrueAVG_Change`,
frmBSCalc `Form_Load`, frmCoinConvert `RefreshCharmButton` / `cmdCharm_Click`, through the index;
`crates/touchstone-rules/src/mega_stat.rs`, `attack::calc_true_average`, `fixtures/rules/mega_stat.json`.

## Do
1. Server: `POST /api/calc/mega-stat` `{text}` -> `{ok, fields[6], message, error, error_text}` over
   `paste_mega_stat_line`. A rejection carries MME's "Incomplete/Missing MegaMUD Statistics in
   Clipboard" and no fields; a VB error is reported as `error`/`error_text` and changes nothing.
   `POST /api/calc/swing` takes the six paste boxes (`hit_pct`, `hit_avg`, `extra_pct`, `extra_avg`,
   `crit_pct`, `crit_avg`) and returns `true_average` (`txtTrueAVG(7)` via `calc_true_average` with
   the swing count). Tests pinned to the `cmdPasteMega_Click` case ids (every `p*` case is run).
2. Web: one shared character store (`getCharacterStore()`; the Character tab uses it too) so a
   calculator sees what the tab edited. `calc.ts` gets pure mappers from `CharacterDoc` + `EvalState`
   to each calculator's inputs (vitest). Derived numbers (accuracy, stealth, encumbrance, ...) are
   read from the evaluated state, never recomputed in TS.
3. Decisions: the Swing and Backstab servers keep global filter off (the character's numbers arrive as
   the box values, which is what MME's Form_Load copies); folding the character's gear into
   `CalculateAttack` through a server-side profile is a follow-up. The Hit calculator's
   character section is filled from the character, and the class / Backstab weapon / strike /
   shadow AC come from the document. Coin: the charm button multiplies the bottom amount by the
   charm discount/markup (`RefreshCharmButton` rule) and is disabled with no charm effect.
4. Screenshots `/tmp/P4-27b/shots/swing-char.png` and `swing-mega.png` by `web/scripts/calc-shots.cjs`.

## Done when
`cargo fmt/clippy/test`, `tools/guard.sh`, `npm run check`, `npm test`, `npm run build` pass; the
endpoint tests pass; both screenshots exist.
