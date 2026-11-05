---
id: P3-04
title: Spell damage equations
model: sonnet
phase: 3
depends: [P3-01, P3-03]
touches:
  - crates/touchstone-rules/src/spells.rs
  - crates/touchstone-rules/src/lib.rs        # one line: pub mod spells;
  - crates/touchstone-rules/tests/all/spells.rs
  - crates/touchstone-rules/tests/all/main.rs # one line: mod spells;
  - fixtures/scenarios/spells.json
  - fixtures/rules/spells.json
  - tools/oracle/gen/spells.py
---

## Goal
Port FAM-10 (stock only): the procedures listed for P3-04 in `docs/inventory.md` section 8a, with the
inputs in section 4a: `GetSpellAttackType`, `GetCurrentSpellMinMax`, `PullSpellEQ`,
`GetSpellMinDamage`, `GetSpellMaxDamage`, `GetSpellDuration`, `SpellDoesDamage`,
`SpellIsAreaAttack`, `SpellIsBlessSpell`, plus `GetMaxLevel` (which `GetCurrentSpellMinMax` calls).
Follow `docs/porting.md` exactly.

## Read
- `docs/porting.md`, `docs/inventory.md` sections 4, 4a and 8a (row P3-04), `docs/testing.md`.
- Through the index (`modMMudDatabase`): the procedures above and `GetMaxLevel`; `AutoAppend`
  (`modSyntaxsFunc`) and `RoundUpTo5`; `GetAbilityStats` (`modMMudFunc`) only to see how it calls
  `PullSpellEQ` back.
- `crates/touchstone-rules/src/abilities.rs` (P3-03) for the row-lookup style, `src/exp.rs` for the
  Currency helpers.

## Do
- The functions take `&[Spell]` (a failed `tabSpells.Seek` is `None`) and a small environment
  struct for the globals (`bQuickSpell`, `nSpellNest`, the global level box, `gAvgLevelMaxAllStats`),
  not MME's globals and controls.
- `GetCurrentSpellMinMax` works in Currency (`Fix`, `Round(x, 2)` on a Currency, `&` on a Currency);
  keep that, do not substitute floats. Errors take MME's `Resume out` path and return what the
  procedure had computed.
- `PullSpellEQ`'s detail text needs `GetAbilityStats`, `GetSpellName` and `GetMonsterName`, which are
  P3-07's and P3-06's. Make them a trait the caller supplies. The test supplies the oracle's own
  answers, recorded by calling those procedures in the same scenario. The ListView side effects of
  `PullSpellEQ` (teleport, summon and textblock rows) are UI, not ported (the `lv` argument is
  always `Nothing`); say so in the module docs.
- `GetMaxLevel` returns a Result (its `RoundUpTo5(CLng(..))` can overflow).
- Scenarios: the whole stock Spells table once as a table case (compact rows), then grids over spell
  numbers, levels, bonuses, overrides and the optional arguments. The generator must be
  deterministic and keep rules plus scenario under 1.5 MB.
- The oracle needs one minimal change: a `{"t": "missing"}` argument that leaves an Optional
  argument out (`PullSpellEQ`'s `lv`). Say so in the report.
- Do not edit `fixture.rs` or `Cargo.toml`. Wire nothing into the server or UI.

## Done when
`docs/porting.md` "Done when".
