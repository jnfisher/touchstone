---
id: Q-017
title: Alter Energy Usage (ability 87) under Quicksilver
model: sonnet
phase: 6
depends: [Q-016]
touches:
  - crates/touchstone-rules/src/gear.rs                  # aggregate ability 87 (Quicksilver only)
  - crates/touchstone-rules/src/character.rs             # carry it into the profile
  - crates/touchstone-rules/src/pipeline.rs
  - crates/touchstone-rules/src/attack.rs                # energy per blow
  - tools/qs-vectors/src/energy.rs
  - tools/qs-vectors/src/main.rs                         # register the module; additive
  - fixtures/quicksilver/energy.json
  - fixtures/quicksilver/cells.json
  - crates/touchstone-rules/tests/all/quicksilver_energy.rs
  - crates/touchstone-rules/tests/all/quicksilver_cells.rs
  - docs/differences.md
---

## Goal
Q-016 found that Quicksilver applies ability 87 (Alter Energy Usage) from standing buffs and gear to the energy a
blow costs, and MME has no such stat, so Warlock, Bard and Mystic kits swing about 7% less in touchstone. Under
Quicksilver only, aggregate the ability the way the engine does and apply it to a normal blow's energy.

## Read
- `docs/differences.md` "end-to-end cells (Q-016)", `docs/quicksilver-profile.md`.
- `crates/touchstone-rules/src/{gear,attack,character,pipeline}.rs`, `tests/all/quicksilver_cells.rs`.
- Quicksilver (read-only): its ability fold and its player-fighter energy step. Summarise, paste nothing.

## Decisions
- Stock also has the ability (the stock engine applies it in the same way), so MME's omission is a parity matter:
  Stock keeps ignoring it and `docs/differences.md` says so. Only `Profile::Quicksilver` reads it.
- Aggregation is a running average, not a sum: sources apply in the order bless spells, race, class, worn items
  (slot order, the weapon after them), carried items that apply; each source with the ability gives
  `(running + value) / 2` once the running value is non-zero, else its own value. A spell slot of 0 uses the
  spell's average cast. "Negate ability" is not modelled.
- Applied to a normal blow after the Quick and Deadly bonus is judged (the engine judges that without abilities):
  `energy * pct / 100` when pct is positive. Not applied to fixed-1000 attacks. The 200 floor stays unmodelled.

## Do
1. `CharStats.energy_use` (gear), carried via the profile to `attack::CharacterProfile.energy_pct`.
2. Apply in `attack_steps` under Quicksilver.
3. `tools/qs-vectors` `energy` family: ability-87 fixtures from the engine's own fold and fighter step.
4. Re-run the cells, tighten the allowance, update `docs/differences.md`.

## Done when
Workspace checks and `tools/guard.sh` pass, the identity harness is exact, the energy family is byte-stable, and the
cells test is tightened.
