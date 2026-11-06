---
id: T-016
title: Textblock script parser for the remaining derived fields
model: sonnet
phase: 4
depends: [T-014, T-015, Q-002]
touches:
  - crates/touchstone-data/src/derive/
  - crates/touchstone-data/tests/all/derive.rs
  - docs/data-gaps.md
  - docs/data-parity.md
---

## Goal
A World loaded from a live silvermere or Quicksilver database derives the fields NMR gets out of
text-block scripts: `obtained_from` (`Textblock`, `NPC`), `summoned_by` (`Textblock`), the spell
`learnable`/`classes`/`learned_from`/`casted_by` script entries, and TBInfo `action` and
`called_from`. Loading stays fast enough for a live reload on a Pi.

## Read
`docs/data-gaps.md` (every *partial* row), `docs/data-parity.md`, `crates/touchstone-data/src/derive/`,
the stock `.mdb` SQLite (the field text is the format reference), and silvermere's public script
engine for the grammar.

## Do
1. `derive/script.rs`: line and clause reader for text blocks. A line is a random line (the last
   word before the first `:` is a number, the entry percentage is the difference to the previous
   random line), a clause line (the first word is a keyword) or a pattern line (everything else;
   its first segment is the ask or command pattern). Clauses that point at blocks: a bare
   number or `text n` (display), `random n`, `testskill s v n` and `checkspell s n` (run as a
   script when the target contains an action keyword other than `message` and `addevil`).
2. `derive/walk.rs`: walk the blocks from the roots (room commands and room-cast spells, monsters
   and spells that are in the game) with a smallest-block-first queue, in two phases (what is in
   the game before any script is read, then the rest), recording who calls whom, and the effects
   of the clauses (`summon`, `giveitem`, `cast`, `learnspell`). Anything deeper than two links
   from a root waits for the second phase.
3. Fixed-point loop in `derive::run`: effects make more monsters, spells and items part of the
   game, which makes more scripts reachable.
4. Emit the entries in NMR's order and format, drop TBInfo rows no root reaches (NMR has none),
   fill `action` with the block text for blocks that are scripts.
5. Re-run the T-015 parity tool, update `docs/data-parity.md` and the rows of `docs/data-gaps.md`;
   load the Quicksilver database and time it.

## Decisions
- Parity over correctness: quirks of NMR's reading (a bare number at the start of a clause counts
  as a reference; the last word of the first segment is the percentage) are reproduced.
- Entry percentages are line differences, so a line out of order gives a negative one.
- What the formats cannot settle (the `Item #n(p%)` chest probabilities, the order of a few
  called-from lists) is explained in `docs/data-gaps.md` rather than guessed.

## Done when
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace` and `tools/guard.sh` pass; each field above matches the stock `.mdb` for
at least 99% of records or every remaining mismatch class is in `docs/data-gaps.md`; the
Quicksilver load time is reported.

## Result
Stock parity (`docs/data-parity.md`): TBInfo `action` 100%, `called_from` 99.4%; `learnable`,
`learned_from`, `classes` 99.35%; `summoned_by` 97.5%; `casted_by` 97.5%; `obtained_from` 87.5%.
Every miss is explained in `docs/data-gaps.md`: the 26 rooms silvermere has beyond the `.mdb`,
the order of a few lists, and the `Item #n(p%)` chest percentages (185 items), whose rule behind
NMR's figures is unknown. Quicksilver 1.20: the derivation settles in six passes; a whole
`load_world()` takes 0.55 to 0.67 s in a release build (about 0.3 s of it the derivation).
