---
id: T-015
title: Stock parity check between the .mdb and silvermere loaders
model: sonnet
phase: 4
depends: [T-013]
touches:
  - tools/silvermere-stock.sh
  - crates/touchstone-data/examples/parity.rs
  - docs/data-parity.md
  - docs/data-gaps.md
---

## Goal
A repeatable dev-time comparison of the World loaded from the stock `.mdb` with the World loaded
from silvermere's own import of the same stock game, field by field. It settles the uncertain
mappings in `docs/data-gaps.md` and is the test bed for T-014. Not part of CI.

## Do
1. `tools/silvermere-stock.sh`: fetch public `jnfisher/silvermere` at a pinned commit into
   `~/.cache/touchstone/silvermere/<sha>/`, build it (release), run its stock import into
   `~/.cache/touchstone/silvermere-stock.sqlite` (its first run downloads the stock game from
   MBBSEmu; see its README). Idempotent; prints the database path.
2. `cargo run -p touchstone-data --example parity -- MDB_SQLITE SILVERMERE_DB` loads both and writes,
   per table and field: match count, mismatch count, and up to five example record numbers with both
   values. Write the summary (counts only, no game values) to `docs/data-parity.md`.
3. Fix the adapter mappings the report shows are wrong (the four flagged as uncertain:
   `exp_multi`, exit suffix text, attack `true_pct`, spell `type_of_resists`), and update
   data-gaps.md.

## Done when
The report exists; every directly mapped field matches 100% or has an explained difference in
data-gaps.md; workspace checks pass.
