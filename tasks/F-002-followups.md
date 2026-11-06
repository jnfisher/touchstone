---
id: F-002
title: Follow-ups bundle 2 (web header, live profile, reach row, party damage)
model: sonnet
phase: followups
depends: [Q-015, F-001, P4-30, P3-45]
touches:
  - web/src/
  - bin/touchstone/src/monster_damage.rs
  - bin/touchstone/tests/
  - docs/differences.md
  - tasks/F-002-followups.md
---

## Goal
The header fits from 1280 px up without clipping a tab; the profile badge, the info popover and the Quicksilver notice
follow a live reload of the database; the monster pane's "double-click to compute" reachability row runs Calculate
Reachability; and "Calc. Monster Dmg vs Party" works without a loaded character, as it does in MME.

## Read
- `web/src/routes/+layout.svelte`, `web/src/lib/components/{ProfileBadge,InfoPopover,ProfileModelNotice,OptionsMenu}.svelte`,
  `web/src/lib/{info,options.svelte,monsterdamage,monsterdamage.svelte}.ts`.
- MME through the index: `frmMain/mnuOptionsItems_Click`, `frmMain/CalculateMonsterPartyDamage`,
  `frmMain/RefreshMonsterCombatGUI`, `modMain/{CalculateMonsterDamageVsCharALL,CalculateMonsterDamageVsChar}`.

## Do (decisions)
1. Header: the tab strip stays scrollable, but the badge shows "QS 1.20.0" (full name in the tooltip) and the F-key
   hints go below the `2xl` width, so every tab fits at 1280 px. Screenshots at 1280 and 1440.
2. Live profile: one shared info store polls `/api/info` every 30 s while the tab is visible (and on becoming visible);
   the badge, popover and notice read it. A changed load number or created date replaces the info.
3. Reach row: a double-click handler on the `reach_compute` row runs `calculateReachability` and the pane refreshes
   through the existing reach version bump.
4. Party damage: MME enables the party item from the lair filter's party size (greater than 1) and the calculation reads
   no character, so it runs with none loaded. The server takes a classless/raceless character in party mode and fills the
   first class and race of the data (the party result reads only the party boxes; a test pins that). The page offers the
   item without a character and sends a blank one.

## Done when
Workspace and web checks pass; each item has a test; screenshots in `/tmp/F-002/shots/`.
