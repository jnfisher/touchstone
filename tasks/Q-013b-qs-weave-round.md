---
id: Q-013b
title: Weave, second half: the weave in the round
model: sonnet
phase: 6
depends: [Q-010, Q-011, Q-013a]
touches:
  - crates/touchstone-rules/src/combat.rs                # the weave term in the round, the opener's follow-up blows, the detail text
  - crates/touchstone-rules/src/attack.rs                # small additive field: expected damaging blows per swing
  - crates/touchstone-rules/src/detail/monster.rs        # the monster pane's own call of the same helper (additive)
  - tools/qs-vectors/src/weave_round.rs
  - tools/qs-vectors/src/weave.rs                        # one helper made visible to the new module
  - tools/qs-vectors/src/main.rs                         # register the family; additive
  - fixtures/quicksilver/weave_round.json
  - crates/touchstone-rules/tests/all/quicksilver_weave_round.rs
  - crates/touchstone-rules/tests/all/main.rs            # registers the test module; additive
  - crates/touchstone-rules/tests/all/attack.rs          # the new field is not an MME field
  - docs/differences.md                                  # append only
  - docs/testing.md
---

## Goal
The second half of row R6 of `docs/quicksilver-profile.md` (the Q-013 row): the weave of Q-013a is live in the round.
`combat::get_damage_output` (and the monster pane, which calls `calculate_attack` itself for a single character)
adds, for a hybrid class under `Profile::Quicksilver`, the expected weave damage of every landed normal blow
that leaves the target alive: `blows * P(blow lands and deals damage) * proc chance * expected proc`. The
term enters `average_damage` and `first_round_damage` the way Q-011's opportunist enters `round_total`, is
added to the follow-up blows of the stab opener (Q-010) and shows in the detail text as `Weave: N% for M`.
`min_round_damage` is untouched (a proc is a chance). Under `Stock` and `Quicksilver(&Balance::NEUTRAL)`
(weave chance 0) nothing changes. The killing blow stays with Q-014: this is the steady-state expectation, the
target assumed to survive.

## Read
- `docs/quicksilver-profile.md` (all), `docs/design.md`, `docs/data-gaps.md`, `docs/testing.md`.
- `tasks/Q-011-qs-melee.md`, `tasks/Q-010-qs-opener.md`, `tasks/Q-013a-qs-weave.md`;
  `crates/touchstone-rules/src/{combat,attack}.rs` (`finish`, the opportunist's `landed` share,
  `surprise_round`), `quicksilver/weave.rs`, `detail/monster.rs` (the single-character attack path).
- Quicksilver (read-only): `crates/silvermere-engine/src/combat/{attack,weave}.rs` (when a weave is
  rolled), `tools/balance/src/{fight,levelup}.rs` (what the balance tool computes). Summarise, paste nothing.

## Do
1. `AttackDamage::landed_per_swing`: the expected number of blows per swing that land and deal damage (hit
   chance times the share of landed blows, crits included, above the target's damage reduction), the share the
   opportunist already uses. Not an MME field.
2. `combat::add_weave` (pub(crate)): from the character's `CharacterProfile` and a `WeaveTarget` it asks
   `quicksilver::weave::expected_per_hit` and adds `round(per_hit * swings * landed_per_swing)` to `round_total`
   and `round(per_hit * whole_blows * landed_per_swing)` to `first_round_damage`, and the detail text. It returns
   early, changing nothing, when the per-hit figure is 0 (Stock, NEUTRAL, a class that does not weave, no spell).
   `combat::weave_target` builds the `WeaveTarget` from the request's magic resistance, defence flags, spell
   immunity and element resistances.
3. `damage_steps` calls it for the normal, bash, smash and martial-arts blows of a single character against a
   monster; `surprise_round` calls it for the whole normal blows after a stab; `detail/monster.rs` calls it on
   its own single-character blow so the pane agrees with `get_damage_output`.
4. Vectors (`tools/qs-vectors/src/weave_round.rs`, family `weave_round`). The balance tool exposes no closed
   per-round figure that matches (it works at mid-roll, whole kits), so the vectors are composed from the
   engine's own pieces: `landed` (the engine's blow resolution enumerated over the whole roll space with a
   certain hit, for a fighter, crit chance and damage reduction: the share of landed blows that deal
   damage) times the Q-013a `proc` figure (the engine's exact mean and resist rolls) and the weaver's chance.
   Columns `v120` and `neutral` (chance 0).
5. Tests: the vectors through the real functions (`add_weave` on a hand-built `AttackDamage`, and
   `calculate_attack` for the share), the combat fixture's characters through `get_damage_output` with the weave
   on and off (only hybrids move, only `average_damage` and `first_round_damage` grow), the opener, the detail
   text, Stock and NEUTRAL unchanged. Identity harness unchanged (no new parametric family: NEUTRAL's chance is 0).
6. `docs/differences.md`, `docs/testing.md`.

## Done when
- `QS_VECTORS_TARGET=/tmp/Q-013b/qsv tools/qs-vectors/run.sh` writes `fixtures/quicksilver/weave_round.json` (at
  most 1.5 MB); a second run and `--check` change nothing.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `tools/guard.sh` pass.
- Every other fixture and the identity harness are unchanged.
- `python3 -I tools/oracle/oracle.py check --jobs 2` reports every family current.
