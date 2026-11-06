---
id: P3-35
title: Item detail and compare text (PullItemDetail, CompareArmor, CompareWeapons)
model: sonnet
phase: 3
depends: [P3-05, P3-07, P3-09, P3-31]
touches:
  - crates/touchstone-rules/src/detail/mod.rs
  - crates/touchstone-rules/src/detail/item.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/src/rows.rs
  - crates/touchstone-rules/tests/all/item_detail.rs
  - crates/touchstone-rules/tests/all/main.rs
  - fixtures/scenarios/item_detail.json
  - fixtures/rules/item_detail.json
  - fixtures/known_gaps.json
  - tools/oracle/gen/item_detail.py
  - tools/oracle/harness.py
  - docs/differences.md
---

## Goal
Port both halves of modMain `PullItemDetail` (`docs/inventory.md` rows P3-35 and P3-36) and FAM-20's
`CompareArmor` / `CompareWeapons`: the text MME writes into an item's detail box (flags, uses,
abilities, casts, class/race restrictions, negated spells, the "Compared to <equipped item>" blocks,
the weapon-damage or armour block) and the reference rows it writes into the item's location list
(Obtained From, References, "Casts:" rows, spell/teleport/summon rows the abilities add). The port returns
structured pieces (kind, label, value) plus the exact text MME shows and the location rows in MME's order.

## Read
- modMain: `PullItemDetail`, `CompareArmor`, `CompareWeapons`, `GetAbilDiffText`, `GetLocations`,
  `getval_array_long_3d`, `in_array_long_md`.
- modMMudFunc: `GetAbilityStats`, `CalculateAttack`, `GetWornTypeEnum`, `GetArmourTypeEnum`,
  `GetWeaponTypeEnum`; modMMudDatabase: `PullSpellEQ` (the ListView-adding parts), `GetClassStealth`,
  `GetRaceStealth`, `GetClassName`, `GetRaceName`, `GetSpellName`; modSyntaxsFunc `AutoAppend`,
  `AutoPrepend`, `RoundUp`, `Truncate`.
- `crates/touchstone-rules/src/{ability_text,attack,locations,rows,pipeline,spells}.rs`,
  `tools/oracle/gen/{rows,locations}.py`.

## Do
- `detail/item.rs`: `pull_item_detail(env, number, mud, full_details, character, lairs) -> ItemDetail`.
  `ItemDetail` holds the detail text, structured rows, the location rows and sorts, and the VB errors that
  reached MME's handler. The frmMain controls and globals the procedure reads are `ItemDetailOptions`
  (global filter, weapon options, shops-first menu check, the equipped-item array `nEquippedItem`, ...).
- Character-dependent parts (`PopulateCharacterProfile` for weapons, the stealth check that forces a
  backstab calculation, the strength notice) take `Option<&pipeline::CharacterState>`.
- The ListView rows `GetAbilityStats` and `PullSpellEQ` add when given a list (spell, teleport,
  summon and "Execute: Textblock" rows) were left out of P3-04/P3-07. They are part of the item's
  location list, so `detail/item.rs` walks the same control flow for rows only (no text).
- ListView sorting stays a skipped feature (`SKIP-LV`): the oracle binds `LV_RefreshSort` /
  `LV_RefreshSort_ShopsFirst` to recorders and the port reports the row counts, as P3-31 does for
  `SortListView`.
- GreaterMUD and Paramud branches (aura-item AC/DR line, Ganghouse shop value, cast-on-kill percent)
  are not ported: `Profile` is `Stock`.
- Oracle: the scenario runs `PullItemDetail` itself through a `rows` step (one case per character, and a
  family of "no character" cases), capturing the detail text box per key. One additive harness
  option (`capture`) reads a control's property after each key of a `rows` step. The characters are the
  pipeline scenario's ladder warrior and mage; their `nEquippedItem` gets a weapon (slot 16) and rings and
  wrists for the compare blocks.
- Scenarios: ~150 items stratified (weapons by type, armour by slot, sundry, keys, scrolls, chests, items with
  many abilities, class-restricted), each with no character and with two characters, plus compare pairs
  (every worn slot, two-slot rings/wrists, a weapon). Combined size under 1.5 MB, generation under ~5 min.

## Done when
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `tools/guard.sh`; `python3 -I tools/oracle/oracle.py check` current for
every family; Python unit tests pass; `tests/all/item_detail.rs` runs every fixture case.
