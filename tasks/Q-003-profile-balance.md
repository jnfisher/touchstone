---
id: Q-003
title: Profile variant, Balance type and V120
model: sonnet
phase: 6
depends: [Q-001]
touches:
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/src/quicksilver/      # new
  - crates/touchstone-rules/Cargo.toml            # serde for Balance
  - every `let Profile::Stock` / `match profile` site in crates/touchstone-rules/src/, made exhaustive with no behaviour change
  - crates/touchstone-rules/tests/all/quicksilver_balance.rs
  - crates/touchstone-rules/tests/all/main.rs
---

## Goal
`Profile` gains a `Quicksilver(&'static Balance)` variant. `Balance` (the 1.20 tuning numbers, with
`Balance::V120`, a neutral setting and validation) lives under `touchstone-rules::quicksilver`. Every
place in the rules crate that matched `Profile::Stock` alone now matches exhaustively, and a
Quicksilver profile behaves exactly as stock there. No other crate changes behaviour.

## Read
- `docs/quicksilver-profile.md`, parts 1 to 5 (this is the Q-003 row), `docs/design.md`,
  `docs/data-gaps.md`.
- Quicksilver's config docs and its `Balance` type in the read-only checkout (key names, ranges,
  1.20.0 values). Summarise in your own words; paste nothing.

## Do
- `quicksilver/mod.rs` re-exports; `quicksilver/balance.rs` holds `Balance`, `LevelPct`, `Anchors`, the
  three per-class tables, `RankWeights`, `BalanceError`, `Balance::validate`, `Balance::knee_third`,
  `Balance::is_v120`; `quicksilver/v120.rs` holds one `pub const` per 1.20.0 value, each with a doc
  comment naming its Quicksilver key, and `Balance::V120` is built from them.
- Every Quicksilver key is carried, including the monster-targeting and `guard` keys that touchstone
  does not use (rank weights, guard chance), so a pasted `[balance]` table deserialises and an unknown
  key still fails (`deny_unknown_fields`). Missing keys default to V120, as in Quicksilver.
- `Balance::NEUTRAL`: every parametric key at its stock-equivalent (part 4 layer 1): crit knee 40,
  stealth divisor 10, level percentage 100, other knees 65 535, class shares 100, opportunist and weave
  chances 0, area cut 0, rest and meditate multiples 1, spell stat bonus 0. Keys with no stock
  counterpart keep the V120 value; the soft-cap keys are floor 0 and bands at 10 a point, the closest
  the type can come to MME's ladder (identical up to the race maximum; Q-007 decides above it).
- `Profile::Quicksilver(&'static Balance)`, still `Copy`/`Eq`/`Default` (Stock). Helpers:
  `Profile::quicksilver_v120()` and `Profile::balance() -> Option<&'static Balance>`.
- Sites: `let Profile::Stock = p;` becomes an or-pattern naming both variants; `match` arms become
  `Profile::Stock | Profile::Quicksilver(_)`. A comment names the Q row where a later card changes
  the site. Nothing computes differently.
- Tests in `tests/all/quicksilver_balance.rs`: V120 values pinned to the 1.20.0 figures; NEUTRAL valid;
  validation errors name the key; partial override and unknown-key rejection; anchor interpolation and
  truncation; class lookups; `Profile` equality/copy/default; Quicksilver at a stock site answers
  as stock.

## Done when
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `tools/guard.sh`, `cargo deny check` pass.
- `python3 -I tools/oracle/oracle.py check --jobs 2` reports every family current.
- `grep -rn "let Profile::Stock" crates/touchstone-rules/src` finds nothing.
