---
id: T-024
title: Wire the real text lookups and drop the hand-written regex
model: sonnet
phase: 3
depends: [P3-07, P3-09, P3-10]
touches:
  - Cargo.toml
  - Cargo.lock
  - crates/touchstone-rules/Cargo.toml
  - crates/touchstone-rules/src/ability_text.rs
  - crates/touchstone-rules/src/attack.rs
  - crates/touchstone-rules/src/attack/
  - crates/touchstone-rules/tests/all/
  - fixtures/scenarios/attack.json
  - fixtures/rules/attack.json
  - fixtures/scenarios/spells.json
  - fixtures/rules/spells.json
  - tools/oracle/gen/attack.py
  - tools/oracle/gen/spells.py
  - tools/oracle/gen/spell_cast.py
---

## Goal
Callers get production implementations of the text traits (`spells::SpellText`, `attack::CastText`,
which `spell_cast` also uses) backed by `ability_text::AbilityEnv` and `NameLookup` over a `World`.
The spells, attack and spell_cast fixture tests run the real implementations instead of replaying
oracle answers, with outputs unchanged. `attack` uses the `regex` crate if every pattern MME builds
for `RegExpFindv2` in `CalculateAttack` is supported.

## Read
- `docs/porting.md`; `crates/touchstone-rules/src/ability_text.rs`, `attack.rs`.
- MME through the index: modMMudFunc `CalculateAttack`, `RegExpFindv2`.

## Do
- `impl NameLookup for World`, `impl CastText for AbilityEnv`, `AbilityEnv::for_world`.
- Extend the attack and spells generators so the "table" case carries the Spells, name and
  textblock rows the text port reads; the tests check `GetAbilityStats`, `GetSpellName`,
  `PullSpellEQ` cases against the port and feed it to the callers.
- Replace `attack/mini_regex.rs` with the `regex` crate (no backreferences or lookaround in the
  three patterns). Note the ASCII-digit and ASCII-case differences in comments.

## Done when
`cargo test --workspace`, `oracle.py check` current, and the three families run no replayed text.
