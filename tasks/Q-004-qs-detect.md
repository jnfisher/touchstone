---
id: Q-004
title: Profile detection, selection and API
model: sonnet
phase: 6
depends: [Q-003]
touches:
  - crates/touchstone-model/src/tables.rs          # Info.content_version
  - crates/touchstone-data/src/                    # the loaders fill it
  - bin/touchstone/src/state.rs
  - bin/touchstone/src/config.rs
  - bin/touchstone/src/api.rs
  - bin/touchstone/src/profile.rs                  # new: choosing the profile
  - bin/touchstone/src/source.rs                   # the load paths call the chooser
  - bin/touchstone/src/lib.rs                      # module list
  - every `Profile::Stock` call site in bin/touchstone/src/, now reading the loaded profile
  - packaging/touchstone.example.toml              # the new [rules] section
  - docs/design.md
  - tests for the above (crates/*/tests, bin/touchstone/tests)
---

## Goal
The server decides once per load which rules apply and uses that profile everywhere. A Quicksilver
database whose structured content version is `1.20.N` gets `Profile::Quicksilver` with the 1.20
balance (plus any `touchstone.toml` override); every other source gets `Profile::Stock`; a
Quicksilver database with an unknown content version refuses to start (on a live reload it keeps the
previous world and logs why). `/api/info` reports the profile. No rule changes behaviour yet (Q-003
made Quicksilver answer as stock until the rule cards land), so every existing test and fixture stays
as it is.

## Read
- `docs/quicksilver-profile.md` part 3 (detection, where the server picks it, config, type) and the
  Q-004 row of part 5; `docs/design.md`; `docs/data-gaps.md` (the adapter's schema rules).
- `crates/touchstone-data/src/silvermere/load.rs` (`info`), `crates/touchstone-data/src/mdb.rs`
  (`info`), `bin/touchstone/src/{state,source,config,api}.rs`.
- `crates/touchstone-rules/src/quicksilver/` (`Balance`, `V120_BALANCE`, `validate`).
- Q-002's `crates/touchstone-data/tests/all/quicksilver.rs` for the skip-when-absent pattern.

## Do
- `Info.content_version: Option<String>`: the structured content version. The silvermere adapter sets
  it from `world_import.content_version` (migration 9001, NULL or empty reads as `None`); the mdb
  loader sets `None`. `dat_file_version` keeps its display value (it still feeds MME's caption
  checks), and nothing derives a profile from it.
- `bin/touchstone/src/profile.rs`: `RulesConfig` (`[rules]`: `profile = auto | stock | quicksilver`,
  `quicksilver_balance` as an optional `Balance`, unknown keys rejected, validated at startup) and
  `ProfileChoice::new(&RulesConfig)`, which leaks an override once per process (not per load: a
  live reload would otherwise leak every few seconds). `choose(&Info)` returns the `Profile` and a
  `ProfileJson` for `/api/info`.
  - `auto`: no content version -> Stock; `1.20.<digits>` -> Quicksilver(V120 or the override);
    anything else -> an error naming the version and the ones this build knows.
  - `stock`: always Stock, even on an unknown Quicksilver version (the switch exists for
    comparisons, so it must not need a known version).
  - `quicksilver`: the 1.20 balance whatever the data says, except that an unknown content version
    is still refused.
- `Loaded` gains `profile: Profile`; `InfoJson` gains `profile` (name, content version, the keys of
  an override that differ from the 1.20.0 values). The load paths in `source.rs` compute it once per
  load and a failure on a reload keeps the old world.
- Every `Profile::Stock` in `bin/touchstone/src` reads the loaded profile (passed down from the
  handler's `Loaded`).
- Tests: version parsing; the choice matrix; config parsing (defaults, override, unknown key, bad
  value); `/api/info` for stock and for Quicksilver; a reload to an unknown version keeps the world;
  the Q-002 database (skips when `~/.cache/touchstone/quicksilver-1.20.sqlite` is absent) loads as
  Quicksilver; the adapter's `content_version` for a seeded 9001 database.
- `docs/design.md` and the example config describe the section.

## Done when
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `tools/guard.sh` pass.
- `grep -rn "Profile::Stock" bin/touchstone/src` finds only the chooser and the stock-only paths
  that have no loaded profile (none expected).
- No fixture under `fixtures/` changes.
