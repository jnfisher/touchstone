---
id: Q-010
title: Backstab round - energy and the opener
model: sonnet
phase: 6
depends: [Q-009]
touches:
  - crates/touchstone-rules/src/attack.rs          # stab energy, the opener (stab plus follow-up blows)
  - crates/touchstone-rules/src/combat.rs          # get_damage_output surprise fields; calc_combat_rounds checked
  - crates/touchstone-rules/src/detail/monster.rs  # the monster pane's backstab opener
  - crates/touchstone-rules/src/quicksilver/       # NEUTRAL: a stab fills the round (additive)
  - crates/touchstone-rules/tests/
  - tools/qs-vectors/src/opener.rs
  - tools/qs-vectors/src/main.rs
  - fixtures/quicksilver/opener.json
  - docs/differences.md
---

## Goal
Row R2 of `docs/quicksilver-profile.md` is live under `Profile::Quicksilver`. A stab costs
`backstab_energy_pct` percent of one normal blow's energy (never more than the round's 1000), and the rest
of the opening round is normal blows with normal damage, accuracy and crits. The surprise figure that
`get_damage_output` and the monster pane report is therefore the whole opening round (the stab plus those
blows), and `calc_combat_rounds` credits it as the one round it replaces. Still one stab a fight. Under
`Stock` and `Quicksilver(&Balance::NEUTRAL)` every existing fixture gives the answer it gives now.

## Read
- `docs/quicksilver-profile.md` (all; this is the Q-010 row), `docs/design.md`, `docs/data-gaps.md`,
  `docs/testing.md` (vectors and the identity harness), `tasks/Q-009-qs-backstab.md`, `tools/qs-vectors/`.
- MME through the index: modMMudFunc `CalcAttack` (energy, swings, the first-round tail) and `CalcEnergyUsed`;
  modMain `GetDamageOutput` (SURPRISE DAMAGE), `CalcCombatRounds` (surprise credit); frmMain monster detail
  (the backstab opener).
- Quicksilver (read-only): `crates/silvermere-rules/src/combat/energy.rs` (`backstab_energy`, `energy_round`),
  `combat/fighter.rs` (the stab's energy), `crates/silvermere-engine/src/combat/attack.rs` (the blow loop).
  Summarise in your own words; paste nothing.

## Do
1. **What the engine does.** A round starts at a full 1000 energy. The stab is the round's first blow and
   costs `min(N * pct / 100, 1000)`, `N` the same character's normal blow with the same weapon (truncating).
   Every later blow of the round is a normal blow costing `N`; blows go on while the energy lasts and the
   round's blow count (stab included) is at most 6. Auto-combat starts each round from the full pool.
2. **`attack.rs`** (additive):
   - `stab_energy(profile, normal)`: Quicksilver's cost; stock is the whole round (1000).
   - `calc_stab_opener(profile, env, ch, req, casts)`: for `Quicksilver` and a type-4 request, the follow-up
     blows of the opening round: `k = min((1000 - S) / N, 5)` whole blows (integer division; the opener is a
     first round, which MME already counts in whole swings), and their damage as MME counts a normal round:
     the type-5 per-swing average with hit chance and crit, from the same character profile and weapon.
     `None` for Stock, a non-stab request and `k = 0`. The stab's own `calculate_attack` result stays one swing.
   - `attack_steps` reports the normal blow's energy `N` (MME's, with its encumbrance step and no 200 floor).
3. **`combat.rs`**: a shared `surprise_round` builds `surprise_damage`, `surprise_min_damage` and the chance
   from the stab plus the opener; `get_damage_output` uses it. `calc_combat_rounds` needs no new branch: the
   surprise figure is already "damage of the one round it replaces". Its profile gate is kept and documented.
4. **`detail/monster.rs`**: the backstab opener block of the monster pane uses the same helper and the
   pane's backstab detail line names the follow-up blows.
5. **NEUTRAL**: `backstab_energy_pct` becomes 100000, the nearest the type comes to MME (any normal blow
   costs at least 1, so the stab fills the round and no follow-up blow fits).
6. **Vector family `opener`** (`tools/qs-vectors/src/opener.rs`): `cost` cases (normal energy and the
   percentage give the engine's stab energy), `round` cases (stab cost and normal cost give the engine's
   follow-up blow count by its energy rule) and `energy` cases (level, class combat, agility, weapon speed,
   encumbrance give the engine's normal and stab energy under V120, an off-grid balance and a neutral one).
   Generated with `tools/qs-vectors/run.sh`, building into `/tmp/Q-010/qsv`.
7. **Tests**: `quicksilver_opener.rs` asserts the columns, the neutral screening against MME's
   `calc_energy_used`, and `calc_stab_opener` / `get_damage_output` end to end. The identity harness gains the
   combat and monster-detail fixtures (the surprise path) under NEUTRAL.
8. **docs/differences.md**: whole-blow follow-up count, MME's missing 200 floor, the steady-state treatment of
   the stab's crit in the surprise figure, and the stab-only panels left as they are.

## Done when
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `tools/guard.sh` pass.
- Every `opener.json` case passes in the columns that exist; every registered fixture passes under `Stock`
  and NEUTRAL (combat and monster detail included).
- `python3 -I tools/oracle/oracle.py check --jobs 2` reports every family current.
