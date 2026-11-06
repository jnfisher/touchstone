---
id: Q-012
title: Spell damage: casting stat, class share, area resist
model: sonnet
phase: 6
depends: [Q-003, Q-005, Q-006]
touches:
  - crates/touchstone-rules/src/spell_cast.rs         # SpellCasterProfile gains casting stat, class, level
  - crates/touchstone-rules/src/character.rs          # populates them
  - crates/touchstone-rules/src/detail/spell.rs
  - crates/touchstone-rules/src/paste_party.rs        # one struct literal gains the new fields; additive
  - tools/qs-vectors/src/main.rs                      # register the family; additive
  - tools/qs-vectors/src/spells.rs
  - fixtures/quicksilver/spells.json
  - crates/touchstone-rules/tests/all/quicksilver_vectors.rs
  - crates/touchstone-rules/tests/all/quicksilver_identity.rs
  - crates/touchstone-rules/tests/all/spell_cast.rs   # run takes a profile; additive
  - crates/touchstone-rules/tests/all/character.rs
  - docs/differences.md                               # append only
  - docs/testing.md
---

## Goal
Rows R7, R8 and R9 of `docs/quicksilver-profile.md` are live under `Profile::Quicksilver`: a player's damage
spells scale with the casting stat and the class share, and area damage spells lose the expected share a
resisting monster cuts. `calculate_spell_cast` shows it in the round damage, the min/avg/max cast and the
combat text. Under `Profile::Stock` and `Profile::Quicksilver(&Balance::NEUTRAL)` every existing fixture gives
the answer it gives now; under `Balance::V120` the building blocks equal the engine vectors in
`fixtures/quicksilver/spells.json`.

## Read
- `docs/quicksilver-profile.md` parts 1 (R7 to R9), 4 and 5 (this is the Q-012 row).
- `docs/testing.md` (vectors and the identity harness), `tasks/Q-005-qs-vectors.md`, `tasks/Q-006-qs-knees.md`.
- MME through the index: modMMudFunc `CalculateSpellCast`, `CalculateResistDamage`; modMain `PopulateCharacterProfile`.
- `crates/touchstone-rules/src/{spell_cast,character}.rs`, `detail/spell.rs`, `quicksilver/`.
- Quicksilver checkout (read-only): `silvermere-rules/src/spells/{mod,apply,resist}.rs`, `regen.rs` (`class_stat`),
  `silvermere-engine/src/spells/cast.rs` (the area monster loop). Summarise in your own words; paste nothing.

## Do
1. `SpellCasterProfile` gains `casting_stat`, `class` and `level` (the character's, not the cast level).
   `CharacterProfile` gains `casting_stat`, set by `populate_character_profile` from the class's magery and the
   stats with gear (Mage Int, Priest Wil, Druid their mean, Bard Cha, 0 for Kai and no magic; the same stat
   `CalcManaRegen` multiplies). `spell_caster()` copies the three. `paste_party` fills them for a member.
2. Pub functions in `spell_cast.rs`: `casting_stat`, `scale_spell_damage(profile, amount, who)` (stat bonus
   with the knee, then the class share, both truncating, the engine's order), `area_resist_keep(profile, mr)`.
   Stock returns the input.
3. `cast_inner`: the scaling applies to the damage a monster takes from Inflict Damage (ability 1), Inflict
   Damage on Armour (17) and a drain (8) that does not last (the engine leaves lasting drains alone). It is
   applied after the spell-damage bonus and before magic resistance and elements, like the engine's order, to
   the per-cast `damage`, `min_damage`, the drain's heal, and the cast min/max shown. Heals and buffs are not
   scaled. MME has no per-target division of area spells, so the full share is scaled.
4. Area resist (R9): an area spell that reaches monsters (targets 3, 5, 9, 11, 12) with an ability 1 or 17 slot
   multiplies the round damage by `1 - p * cut / 100`; `p` is the engine's roll chance (the monster's
   `min(MR/2, 98)` out of the 99 values `genrdn(1,100)` can return) and `cut` is `min(max_pct, MR / div)`.
   MME's own full-resist figure is kept as it is. The text gains a clause only when the factor is below 1.
5. Vectors (`tools/qs-vectors/src/spells.rs`, family `spells`): `stat` (scale_spell_damage), `class`
   (class_spell_pct), `damage` (both, in the engine's order), `casting` (class_stat) and `area` (the engine's
   resist roll enumerated over every roll under a scripted source: the exact sum of the integer shares).
   `neutral` is the engine with the permille 0, class shares 100 and cut 0.
6. Tests: `quicksilver_vectors.rs` asserts v120 through `Profile::Quicksilver(V120)` and neutral through Stock
   and NEUTRAL; the area kind compares the closed form with the exact mean within the truncation allowance.
   `spell_cast.rs` fixture test takes a profile and is registered in the identity harness, with `character.rs`.
   Plus `calculate_spell_cast` cases under V120 on a built spell.
7. `docs/differences.md`, `docs/testing.md`.

## Done when
- `tools/qs-vectors/run.sh` (with `QS_VECTORS_TARGET=/tmp/Q-012/qsv`) writes `fixtures/quicksilver/spells.json`
  (at most 1.5 MB, numbers only); a second run and `--check` change nothing.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `tools/guard.sh` pass.
- Every spells vector passes in both columns; the spell_cast, character and other registered fixtures pass under
  Stock and NEUTRAL.
- `python3 -I tools/oracle/oracle.py check --jobs 2` reports every family current.
