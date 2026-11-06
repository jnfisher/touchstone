---
id: P3-32
title: Monster detail pane (PullMonsterDetail, parts 1 to 3)
model: sonnet
phase: 3
depends: [P3-14a, P3-14b, P3-07, P3-31, P3-12, P3-26, P3-16, P3-27]
touches:
  - crates/touchstone-rules/src/detail/
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/monster_detail.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/monster_detail.json
  - fixtures/rules/monster_detail.json
  - tools/oracle/gen/monster_detail.py
  - tools/oracle/harness.py
  - tools/oracle/shims.py
---

## Goal
Port modMain `PullMonsterDetail` (inventory rows P3-32 to P3-34, all three parts) and `AddMonsterDamageOutText`
(FAM-20): the Monsters detail pane, built as a list of structured rows (label, value, kind, colour, bold) that carry
the exact MME text. Attributes, abilities, drops, mob attacks, damage versus the character, scripting value and exp/hr,
lair stats and the "Spawns via" locations are all there.

## Read
- modMain `PullMonsterDetail`, `AddMonsterDamageOutText`, `AddMonsterReachRows` (stubbed); frmMain `LookUpMonsterRegen`.
- Reused, not redone: `monster_glue` (attack summary, damage versus character), `combat` (`calc_combat_rounds`,
  `get_damage_output`), `lairs` (`CharLairEnv`, `get_lair_averages_from_locs`), `exp_per_hour::calc_exp_per_hour`,
  `ability_text`, `spells::pull_spell_eq`, `attack::calculate_attack`, `spell_cast::calculate_spell_cast`,
  `locations::get_locations`, `rows/monster.rs` (shared text helpers).
- `docs/porting.md`, `tasks/P3-27-monster-rows.md`, `tools/oracle/gen/monster_rows.py`.

## Decisions
- `detail/monster.rs` holds `pull_monster_detail` and `add_monster_damage_out_text`. `detail/mod.rs` only declares the
  module (the orchestrator merges the other detail cards' lines).
- Output: `Vec<DetailRow>` (`label`, `kind` from the ListItem Tag, `value` cell with text, tag, colour, bold). Colours are
  the light-theme values (`TColor` is the identity).
- Inputs: the `CharLairEnv` (P3-27's), the `CharacterState` for the character-dependent parts, and a `MonsterDetailInputs`
  struct for the frmMain globals the procedure reads (`bAutoCalcMonDamage`, `bDontLookupMonRegen`, `UseExpMulti`, ...).
- Reach rows (`AddMonsterReachRows`) go through a `ReachRows` trait; the default adds nothing and the oracle stubs the
  procedure. "Spawns via ..." runs `locations::get_locations` with `LookUpMonsterRegen`'s arguments (two columns, not
  sorted), so no `sorts` are needed.
- Oracle: the recording ListView learns `Bold` (rows and sub-items) and a `rows` step can hand the record number to the
  procedure and keep every row of the list (`"lv": "list"`, `{"key": true}`), additive.
- Scenarios: about 120 monsters stratified over spells, drops, lairs, regen, undead, non-hostile, summons and guards; no
  character plus two characters (a melee and a caster), solo and party; data version 1.83 plus a few older.

## Done when
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`,
`tools/guard.sh`; `python3 -I tools/oracle/oracle.py check` current for every family; Python unit tests pass; every
`monster_detail` case passes; scenario plus fixture under 1.5 MB, generation under about 5 minutes.
