---
id: W-001
title: Weapon Simulator
model: opus
phase: 4
depends: [P3-09, P3-11, P3-17, R-003]
touches:
  - crates/touchstone-rules/src/weapon_sim.rs
  - crates/touchstone-rules/src/attack.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/
  - bin/touchstone/src/weapon_sim.rs
  - bin/touchstone/src/api.rs
  - bin/touchstone/src/lib.rs
  - bin/touchstone/tests/all/
  - web/src/lib/weaponsim.ts
  - web/src/lib/calc.ts
  - web/src/lib/components/calc/WeaponSimCalc.svelte
  - web/src/routes/calc/
  - docs/design.md
  - docs/differences.md
---

## Goal
A new tool under Calculators, the Weapon Simulator: pick the active character, open it, and see the best weapons
for that character with the damage to expect from each and its details. Regular rounds (attack and bash) and
backstab rounds are simulated apart and each has its own sortable view. It is touchstone's own tool (MME has
none) and must stay cheap on a Raspberry Pi 5 beside the game server.

## Read
- `attack.rs calculate_attack` and `calc_stab_opener`, `calculators.rs calc_swings` (the swings of successive
  rounds), `gear.rs item_is_usable_by_char`, `filters.rs weapon_steps` (MME's "maximum effort" weapon list: the
  profile per attack, which weapons stab), `pipeline.rs CharacterState::profile_for`.
- `bin/touchstone/src/monster_sim.rs` (a character-driven calculator route), `web/.../calc/current.svelte.ts`,
  `ChestsTab.svelte` (grid beside a detail pane).

## Decisions
- **Blows come from MME.** Every number a blow is rolled from is `CalculateAttack`'s for that character, weapon,
  attack and target (the same calls the Weapons tab makes in maximum-effort mode), so the mean of the simulation
  lands on MME's damage a round and the response carries both. The simulation adds what MME has no figure for:
  the spread of a round, how often it does nothing, the rounds to bring a target down.
- **Shortlist first.** Every weapon the character may wield (`ItemIsUsableByChar`) is scored with
  `CalculateAttack`, which costs microseconds; the best N by attack, by bash and by backstab, the weapon in hand
  and any pinned weapons are simulated. N is 10, 25 (default) or 50.
- **Restrictions.** "Ignore level" keeps weapons above the character's level (a level raised to the weapon's
  minimum only lifts the minimum-level rule) and flags them; damage still uses today's stats. "Ignore alignment"
  is `ItemIsUsableByChar`'s own skip. "Limited items" is on by default. The minimum item level of the global
  filter is not applied.
- **Regular rounds** carry leftover energy from round to round as `CalcSwings` does (at most five swings).
  **Backstab rounds** are opening rounds, each on its own: the stab and, under Quicksilver, the normal blows the
  rest of the round pays for. Only weapons with backstab accuracy stab, and only for a class or race with stealth,
  as in MME's lists.
- **Target.** AC, DR, dodge, backstab defense and hit points, typed or filled from a monster. With hit points the
  rounds to kill are simulated (a stab opens, normal attacks follow); a fight that would run past 60 rounds is
  estimated from the average instead of rolled.
- **Cost.** One request at a time (a second waits), off the async threads; rounds at most 50,000, shortlist at
  most 60 an attack, 20 pinned. Dice are a seeded xoshiro256++ in the rules crate (no new dependency), one stream
  per weapon and attack, so a result repeats and "Roll again" shows the noise.

## Do
`weapon_sim.rs` in the rules crate (dice, round, opener and fight statistics) and in the server (the route),
`min_crit` and `energy` on `AttackDamage`, the page, tests, docs.

## Done when
The repository checks in CLAUDE.md pass. On the stock database a default request answers in well under 100 ms on
a desktop core and the worst allowed request in about half a second.
