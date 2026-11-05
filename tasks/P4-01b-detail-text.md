---
id: P4-01b
title: Detail panes show MME's text: ability lines, spell damage and duration, monster max damage
model: sonnet
phase: 4
depends: [P4-01a, P3-07, P3-13]
touches:
  - bin/touchstone/
  - web/
  - tasks/P4-01b-detail-text.md
---

## Goal
The detail panes stop showing raw numbers where MME shows text. Items, spells, races, classes and
monsters list their abilities as MME prints them ("Damage +5", "CastsSp [Name(12), Damage 5 to 9]"),
the Spells pane shows damage range and duration at a chosen caster level, and the Monsters pane
shows the maximum damage per round.

## Read
`docs/design.md` ("Server and API", "Web UI"); `touchstone-rules` `ability_text`, `abilities`,
`spells` and `monster_sim` public API; MME `PullItemDetail` (ability loop), `PullRaceDetail`,
`PullClassDetail`, `PullMonsterDetail` (ability loop), `PullSpellDetail`, `GetAbilityStats`,
`PopulateMonsterDataToAttackSim`, `CalculateMonsterItemBonuses`, `CalculateMonsterAvgDmg`
(through the index); the P4-05L layout conventions in `web/src/lib/components/`.

## Do
1. `GET /api/abilities/{items|spells|races|classes|monsters}/{n}`: one line per ability slot with
   the ability number, value, `GetAbilityName`, the full `GetAbilityStats` text and the part after
   the name. The slot rules follow each pane's MME loop: items use `bPercentColumn` and fold ability
   114 (cast percent) into the next ability 43 text; ability 59 on an item is listed as `class_ok`;
   classes skip 59; monsters skip 146; message-only abilities print nothing.
2. `GET /api/spells/{n}/effect?level=L`: min and max damage (and heal) and duration at `L`
   (default the spell's required level), plus the `PullSpellEQ` text at that level.
3. `GET /api/monsters/{n}/max-damage`: fill a `MonsterSimInput` exactly as
   `PopulateMonsterDataToAttackSim` does (stock branch, weapon and drop item bonuses, spell
   attacks, between-round spells, `ResetValues` defaults) and answer `MonsterSim::max_damage`.
4. Web: replace the raw "Ability n: v" rows with the text; Spells gets a caster-level box with
   the damage and duration; Monsters gets a max damage row. Keep the detail key/value grid and
   non-wrapping inner tables.
5. Rust tests with `oneshot` over an invented World, with a few cases pinned to fixture numbers;
   vitest for the client path.
6. `web/scripts/detail-shots.cjs` takes the screenshots (git-ignored output).

## Done when
`cargo fmt/clippy/test`, `tools/guard.sh`, and in `web/` `npm run check`, `npm test`,
`npm run build` pass; against the stock database the three endpoints answer and the Weapons,
Spells and Monsters panes render the new text; `web/scripts/smoke.cjs` passes.
