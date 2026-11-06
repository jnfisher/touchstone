---
id: P4-30
title: Calculate / clear monster damage vs character and party
model: sonnet
phase: 4
depends: [P3-14b, P3-32, P4-08]
touches:
  - bin/touchstone/src/
  - bin/touchstone/tests/
  - crates/touchstone-rules/src/monster_glue.rs
  - crates/touchstone-rules/tests/
  - fixtures/scenarios/monster_damage_calc.json
  - fixtures/rules/monster_damage_calc.json
  - tools/oracle/
  - web/src/
  - docs/differences.md
  - tasks/P4-30-monster-damage.md
---

## Goal
MME's Options menu can calculate the damage every monster does to the character
(`CalculateMonsterDamageVsCharALL`) or to the party, and clear it (`ClearMonsterDamageVsCharALL`). The stored arrays
feed the Monsters list (damage column and filter), the monster pane and the exp/hr models. The server keeps them,
the page offers the three actions, and the lair-mode "calculate first?" question appears where MME asks it.

## Read
- MME through the index: `frmMain/mnuOptionsItems_Click`, `frmMain/CalculateMonsterPartyDamage`,
  `modMain/{CalculateMonsterDamageVsCharALL,ClearMonsterDamageVsCharALL,SetCharDefenseDescription}`, `frmMain/FilterMonsters`
  (the prompts) and every writer of `sMonsterDamageVsCharDefenseConfig` / `bDontPromptCalc*` / `bMonsterDamageVsPartyCalculated`.
- `crates/touchstone-rules/src/{monster_glue.rs,filters/monsters.rs,detail/monster.rs}`,
  `bin/touchstone/src/{state,evaluate,panes}.rs`, `web/src/lib/components/ExplorerView.svelte`.

## Decisions
- MME clears the arrays when a character is loaded or pasted, on "Reset all character fields" and on a database
  reload, and on the menu's Clear. It does **not** clear them on a character edit: it compares the stored defense
  description (`sMonsterDamageVsCharDefenseConfig`) with the current one and calls the sims stale. The server is
  stateless about characters, so it stores the arrays, the description a calculation ran for, the party flag and the
  "already asked" markers, and the page sends the character every time. The page clears on load/paste/new/reset;
  a reload of the data resets the store by itself (it is tagged with the load number).
- `bDontPromptCalc*` are "already asked" flags that MME resets whenever the defense description (or the party boxes)
  change. The server stores the description / party-box key at the moment of the answer and the flag is true while
  the key is unchanged.
- `/api/monster-damage/{calculate,clear,status,dismiss}`. Calculate runs in `spawn_blocking`, one at a time (409
  otherwise); `status` reports `running` with done/total monsters and elapsed milliseconds, which the page polls.
  The shared generator starts at MME's initial `Rnd` state, so a result is reproducible.
- `/api/evaluate/monsters` in lair mode reads the store's prompt state. When MME would ask, the response carries
  `prompt: {dialog, title, message, party}` and no rows; the page asks, then calls calculate (Yes), dismiss (No) or
  nothing (Cancel) and evaluates again.
- The evaluate cache key includes the store's generation, so a calculation or clear never serves a stale list.
- Oracle family `monster_damage_calc`: MME's `CalculateMonsterDamageVsCharALL` (progress form shimmed) over a small
  Monsters table, for a character and for a party, plus `ClearMonsterDamageVsCharALL`; the arrays, description and
  flags are pinned, in the rules crate and through the endpoint.

## Done when
Workspace and web checks pass; `oracle.py check` current for every family; screenshot
`/tmp/P4-30/shots/monster-damage.png`.
