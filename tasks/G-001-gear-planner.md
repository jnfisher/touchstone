---
id: G-001
title: Gear Planner
model: opus
phase: 4
depends: [W-001, P3-17, P3-44]
touches:
  - crates/touchstone-rules/src/gear_plan.rs
  - crates/touchstone-rules/src/acquire.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/
  - bin/touchstone/src/gear_plan.rs
  - bin/touchstone/src/usable.rs
  - bin/touchstone/src/weapon_sim.rs
  - bin/touchstone/src/character.rs
  - bin/touchstone/src/api.rs
  - bin/touchstone/src/lib.rs
  - bin/touchstone/tests/all/
  - web/src/lib/gearplan.ts
  - web/src/lib/weaponsim.ts
  - web/src/lib/calc.ts
  - web/src/lib/components/calc/
  - web/src/routes/calc/
  - docs/design.md
  - docs/differences.md
---

## Goal
A second planning tool under Calculators, the Gear Planner: for the character in use and a goal (melee damage,
backstab, armour class, dodge, spellcasting, mana regen, hit points, stealth) it fills every worn slot but the
weapon with the gear that does most for that goal, shows each swap beside what is worn now with what it is worth,
and says where to get the suggested items: the monsters to kill, the chests they drop, the shops, and the rooms,
people and scripts. The Weapon Simulator also gains "Obtainable only", which this card shares with it.

## Read
- `pipeline.rs refresh_all`, `equip.rs list_slots` and the paired fingers and wrists, `find_best.rs` (MME's nearest
  tool), `attack.rs calculate_attack`, `gear.rs item_is_usable_by_char`.
- `docs/data-gaps.md` on `obtained_from`; `bin/touchstone/src/weapon_sim.rs` and `WeaponSimCalc.svelte` (W-001).

## Decisions
- **Whole sets, MME's numbers.** A set is scored by evaluating the character wearing it (`refresh_all`, and
  `CalculateAttack` for the damage goals), never by adding up item abilities: weight, strength and accuracy then
  count for what they do. One evaluation costs about 50 microseconds.
- **The fast search** (John's choice): a climb, slot by slot. Every candidate of a slot is tried in the set as it
  stands; later passes retry each slot's best twelve until a pass changes nothing (at most six). It finds a set no
  single swap improves; two items that only pay off together can be missed, and the page says so.
- **Only real gains.** An item replaces what is worn only if it does more for the goal (or breaks a limit less).
  A tie never does, so a damage plan leaves armour that is irrelevant to damage alone.
- **Limits.** "AC at least" and "encumbrance at most" rule sets out before the goal is compared; a limit nothing
  meets is reported and the plan gets as close as it can. Slots can be locked. The weapon is never planned (that is
  the Weapon Simulator); a two-handed weapon closes the off-hand; the second wrist is opt-in as in MME.
- **Next best.** A button on each suggested item plans again without it (the request's `skipped` list), so a
  slot can be walked down its list; the skipped items show as chips that put them back.
- **Which items**: the Weapon Simulator's switches (ignore level, ignore alignment, limited items, obtainable only),
  now in one place (`bin/touchstone/src/usable.rs`).
- **Where to get it.** The suggested item's Obtained From line is read into sources. A chest is followed one level
  to the monsters that drop it, with the chance multiplied through; a shop that only buys the item is no source.
  The page groups them into a kill list (the monster that yields most of the wanted items first), shops, and the
  rest. Kills are `100 / chance`.

## Do
`gear_plan.rs` and `acquire.rs` in the rules crate, the route, the page, tests, docs.

## Done when
The repository checks in CLAUDE.md pass. On the stock database a plan answers in well under 100 ms on a desktop core.
