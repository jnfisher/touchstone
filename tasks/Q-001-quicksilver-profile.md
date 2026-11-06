---
id: Q-001
title: Quicksilver profile, inventory and design
model: sonnet
phase: 6
depends: [stock rules (phase 3)]
touches:
  - docs/quicksilver-profile.md
  - docs/design.md          # the "Formulas and profiles" section only
---

## Goal
touchstone reproduces MME's stock formulas through `touchstone-rules`, which takes a `Profile`. The
next profile is `Quicksilver`: stock plus the formula changes of John's server. This card works out
exactly which formula changes Quicksilver makes, which touchstone functions each one touches, how
the server learns it is talking to Quicksilver, how a Quicksilver branch is tested when the oracle
is MME, and the cards that implement it. Docs only; no code.

## Read
- Quicksilver's own docs in the read-only checkout at `/home/user/quicksilver-dev`:
  `docs/differences.md`, `docs/differences-1.20.md`, `docs/balance/`, `docs/config.md`
  (`[balance]`), and its rules crates for combat, spells, regen, progression and the
  `[balance]` type.
- The open-source engine Quicksilver forks, in `~/.cache/touchstone/silvermere/`, to see by diff
  what Quicksilver changed.
- touchstone: `docs/design.md`, `docs/differences.md`, `docs/inventory.md` section 4 and 8,
  `Profile` and every `match` on it in `crates/touchstone-rules`, `touchstone-data`'s
  `world_import.content_version` detection, `bin/touchstone/src/state.rs` and `config.rs`.

## Decisions
- Quicksilver's text, tables and code are not pasted: the doc summarises and names its files by
  path only. Nothing from the reconstructed game source is named or used.
- Content (items, monsters, spells, rooms, shops, race abilities) needs no profile: it arrives in
  the live database. The doc lists it so nobody ports it by mistake.
- Rules that MME has no function for (party targeting weights, `guard`, the Witchunter's
  friendly-cast exemption) are listed as not modelled, not carded.
- Parity target: Quicksilver's own pure rule functions. The differential source is a local
  generator that links Quicksilver's rules crate and writes small JSON vectors, committed as
  fixtures. It never runs in PR CI.

## Do
1. Write `docs/quicksilver-profile.md` with five parts: the change table, the no-profile list,
   profile detection and selection, the test strategy, and the card list (Q-002 on).
2. Point `docs/design.md`'s profile section at it, in three lines at most.

## Done when
- `docs/quicksilver-profile.md` exists with the five parts and every card has `touches`,
  dependencies and a size.
- `tools/guard.sh` passes. No code changed. The final report lists the card titles.
