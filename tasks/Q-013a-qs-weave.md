---
id: Q-013a
title: Weave, first half: the pick, the budget and the volley expectation
model: sonnet
phase: 6
depends: [Q-011, Q-012]
touches:
  - crates/touchstone-rules/src/quicksilver/weave.rs     # new: weaver, candidates, volley, budget, power, pick, expected damage
  - crates/touchstone-rules/src/quicksilver/mod.rs       # registers the module; additive
  - crates/touchstone-rules/src/spell_cast.rs            # small pub exposure only, if a helper is needed
  - tools/qs-vectors/src/main.rs                         # register the family; additive
  - tools/qs-vectors/src/weave.rs
  - fixtures/quicksilver/weave.json
  - crates/touchstone-rules/tests/all/quicksilver_weave.rs
  - crates/touchstone-rules/tests/all/main.rs            # registers the test module; additive
  - docs/differences.md                                  # append only
  - docs/testing.md
---

## Goal
The first half of row R6 of `docs/quicksilver-profile.md` (the Q-013 row, first half): for a hybrid class,
`quicksilver::weave` answers which spell a weave proc fires (the best single-target attack spell the character
can cast), how big a proc is allowed to be at a level (the budget), at what percentage power the volley is rolled,
and what one volley is expected to deal to a given target through its element and magic resistance, a Bard's
single song hit included. Nothing is wired into the round: `get_damage_output`, `combat.rs` and exp/hr are Q-013b.
Under `Profile::Stock` there is no weave (`None`, 0). Every existing fixture and the identity harness are
untouched.

## Read
- `docs/quicksilver-profile.md` parts 1 (R6, R7, R8), 4 and 5 (the Q-013 row), `docs/testing.md`, `docs/differences.md` (Q-010, Q-011).
- `tasks/Q-012-qs-spells.md` and `crates/touchstone-rules/src/spell_cast.rs`: `scale_spell_damage`, `casting_stat`,
  `UsableEnv::spell_is_usable`, `SpellCasterProfile`.
- `crates/touchstone-rules/src/spells.rs` (the spell equations) and `quicksilver/{balance,v120}.rs`.
- Quicksilver checkout (read-only): `crates/silvermere-rules/src/v120/weave.rs` (the rule),
  `crates/silvermere-engine/src/combat/weave.rs` (how the proc's hits are applied),
  `silvermere-rules/src/spells/{apply,resist,mod}.rs`. Summarise in your own words; paste nothing.

## Do
1. `quicksilver/weave.rs`, all pure functions over `Profile`:
   - `weaver(profile, class)`: the weaving classes are those with magery type Mage, Priest, Druid or Bard at class
     magery level 1 (chance `weave_pct_magery1`) or 2 (`weave_pct`); level 3 casters, Kai and no magic do not.
     Returns the chance, whether it is a Bard and the class number. `None` under Stock.
   - `is_candidate`: an instant (duration 0) spell aimed at one target (targets User, Monster or Monster or User,
     never an area), with an Inflict Damage (1) or Inflict Damage on Armour (17) slot, without Kill Dead,
     Teleport Room or Teleport Map, that `spell_is_usable` says the class, level and alignment can use.
   - `volley`: the hits one auto-combat round of the spell makes on 1000 energy (at most 20 casts), following a
     sure "cast on ending" chain (the chain takes over the rest of the energy), with twice the summed average damage.
   - `budget`: `weave_budget_per_level` times the level held to `scalecap`. `power_pct`: `budget * 100 / average`,
     clamped to the power minimum and maximum; a volley that deals nothing is rolled at the maximum.
   - `pick`: the candidate (not a spell the target is immune to or spares) with the highest volley average, ties to
     the higher spell level then the lower spell number; its volley and power.
   - `expected_proc_damage`: exact mean over the spell's roll range of the damage one unresisted-or-resisted
     proc deals, hit by hit and slot by slot, in the order the engine applies them (roll times power, the class
     weave share, the target's element, then the "Alter spell damage" bonus, the casting stat with the weave's share
     of the bonus and the class spell share, then magic resistance for a slot that ignores armour not), times the
     chance the one resist roll lets it through. A Bard's proc is one hit of the class share of the budget.
     `expected_per_hit` multiplies by the proc chance.
2. Spell levels use the engine's integer form (`base + inc * level / lvls`, the level held to the spell cap) so the
   vectors can be exact; this differs from MME's currency form by at most one at a few levels (`docs/differences.md`).
3. Vectors (`tools/qs-vectors/src/weave.rs`, family `weave`): `weaver` (chance and Bard flag by magery and level,
   neutral chance 0), `budget`, `power`, `pick` (the engine's pick over a small synthetic book for several classes
   and levels) and `proc` (the engine's own apply code, run for every roll of the spell's range, the exact sum, and
   the engine's resist roll enumerated over its 99 values). `neutral` exists only for `weaver` and `proc`
   (chance 0).
4. Tests (`tests/all/quicksilver_weave.rs`): the vector columns through `Profile::quicksilver_v120()`, neutral
   through Stock and NEUTRAL, vector reach, plus hand-built cases on real-shaped spells.
5. `docs/differences.md`, `docs/testing.md`.

## Done when
- `QS_VECTORS_TARGET=/tmp/Q-013a/qsv tools/qs-vectors/run.sh` writes `fixtures/quicksilver/weave.json` (at most
  1.5 MB, numbers only); a second run and `--check` change nothing.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `tools/guard.sh` pass.
- Every weave vector passes in both columns; every other fixture and the identity harness are unchanged.
- `python3 -I tools/oracle/oracle.py check --jobs 2` reports every family current.
