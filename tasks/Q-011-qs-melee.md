---
id: Q-011
title: Class melee share and Thief opportunist
model: sonnet
phase: 6
depends: [Q-003, Q-005, Q-009]
touches:
  - crates/touchstone-rules/src/attack.rs          # class melee share on every non-stab blow; the opportunist term of calculate_attack
  - crates/touchstone-rules/src/combat.rs          # get_damage_output checked (the term arrives through round_total); detail text
  - crates/touchstone-rules/tests/
  - tools/qs-vectors/src/melee.rs
  - tools/qs-vectors/src/main.rs
  - fixtures/quicksilver/melee.json
  - docs/differences.md
---

## Goal
Rows R4 and R5 of `docs/quicksilver-profile.md` are live under `Profile::Quicksilver`.

- R5: every non-stab blow (normal, bash, smash, punch, kick, jumpkick) deals the class's `class_melee_pct`
  share of its damage range, the percentage read at the character's real level (anchors interpolate in
  a straight line and the product truncates, `LevelPct::scale`). The share applies to the range as the
  engine's fighter holds it (weapon, damage bonuses and martial-arts damage included), before the move's
  own multiplier (kick and jumpkick pre-roll, bash and smash tripling/quintupling), the critical range and
  damage reduction. A stab keeps its own figures.
- R4: a Thief's landed non-stab blow that leaves the target alive may add a bonus hit (`opportunist_pct`
  chance): `(2L + stealth / backstab_stealth_div + Backstab Min..Max)` raised by `100 + L *
  opportunist_level_pct / 100` percent, `L` held to `scalecap`, less the target's damage reduction. It never
  crits and the weapon's damage plays no part. `calculate_attack` adds its expectation to `round_total`,
  `first_round_damage` (and so the stab opener's follow-up blows); `min_round_damage` is untouched (a proc is
  a chance).

Under `Stock` and `Quicksilver(&Balance::NEUTRAL)` (class shares 100, `opportunist_pct` 0) every existing
fixture gives the answer it gives now.

## Read
- `docs/quicksilver-profile.md` (all; this is the Q-011 row), `docs/design.md`, `docs/data-gaps.md`,
  `docs/testing.md` (vectors and the identity harness), `tasks/Q-009-qs-backstab.md`, `tasks/Q-010-qs-opener.md`,
  `tools/qs-vectors/`.
- MME through the index: modMMudFunc `CalcAttack` (damage block, martial arts, bash/smash, the first-round
  tail) and `CalcTrueAverage`; modMain `GetDamageOutput` (physical branches).
- Quicksilver (read-only): `crates/silvermere-rules/src/combat/fighter.rs` (where the share is applied),
  `combat/attack.rs` (`resolve`, `move_mods`), `v120/opportunist.rs`, and the engine's blow loop in
  `crates/silvermere-engine/src/combat/attack.rs` (where an opening is rolled). Summarise in your own words.

## Do
1. **Share** in `attack_steps`: for every non-manual blow that is not a stab and has a known class
   (`ch.class` 1..=15), scale `dmg_min` and `dmg_max` with `class_melee_pct.scale(class, level, ..)` after the
   martial-arts damage is added and before `finish` applies the pre-roll, DR and multiplier. A manual
   damage figure (`specify_damage`) and the unclassed max-level mock are not scaled.
2. **Opportunist**: `attack::opportunist_bonus` (pure, public) gives the bonus range and the expected bonus of
   one proc after damage reduction; `attack_steps` computes the range for a Thief (class 8) from the same
   level, stealth and Backstab Min/Max it uses for a stab, and `finish` turns it into
   `swings * hit_chance * pct * (share of landed blows with positive damage) * mean bonus`. New
   `AttackDamage` fields `opportunist_chance` (percent) and `opportunist_avg` (mean bonus of one proc), and an
   `Opportunist` text in the detail line.
3. **combat.rs**: `get_damage_output` needs no new branch (the term arrives through `round_total` and
   `first_round_damage`); checked with a Thief case. Detail text for the opener follows.
4. **NEUTRAL** needs no change: `class_melee_pct` is already 100 for every class and `opportunist_pct` 0.
5. **Vector family `melee`** (`tools/qs-vectors/src/melee.rs`): `share` (the engine's class share of a range:
   class, level, min, max), `fighter` (the engine's fighter builder for a normal blow: class, level, weapon,
   plus-max-damage), `opening` (the engine's opportunist: class, level, stealth, Backstab Min/Max, target
   DR; the bonus range and, over the whole roll space under a scripted source, the sum of bonus damage and
   the roll count). Columns `v120`, `alt` (off-grid balance) and `neutral`. Generated with
   `tools/qs-vectors/run.sh`, building into `/tmp/Q-011/qsv`.
6. **Tests**: `quicksilver_melee.rs` asserts the columns through the real functions and `calculate_attack`; the
   identity harness gains the new `AttackDamage` fields where it compares them (unchanged under NEUTRAL).
7. **docs/differences.md**: share applied to the range before pre-roll; manual damage unscaled; steady-state
   opportunist (target assumed to survive, expected value, landed blows with positive damage only); no
   change to `calc_true_average` (its inputs are the caller's numbers).

## Done when
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `tools/guard.sh` pass.
- Every `melee.json` case passes in the columns that exist; every registered fixture passes under `Stock`
  and NEUTRAL.
- `python3 -I tools/oracle/oracle.py check --jobs 2` reports every family current.
