---
id: P4-32
title: Spell tools, Spell Book window, chest contents
model: sonnet
phase: 4
depends: [P3-28, P3-30, P3-37, P4-15, P4-16, P4-12]
touches:
  - crates/touchstone-rules/src/spell_tools.rs
  - crates/touchstone-rules/src/spell_book.rs
  - crates/touchstone-rules/src/chests.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/all/
  - tools/oracle/
  - fixtures/scenarios/
  - fixtures/rules/
  - bin/touchstone/src/
  - bin/touchstone/tests/
  - web/src/
  - docs/differences.md
---

## Goal
The spell features MME has and touchstone lacked: the counters finder (`FindNegates`), "what casts this
spell" (`LookUpSpellCast`), the Spell Book window (the spells a class and level may learn or cast, with
the character filter) and the Sundry explorer's chest contents (`cmdSundryChests_Click`, "Copy Chest").
MME shows the first two in its Results window; touchstone shows them as list dialogs whose rows link to
the records (the textblock tree window stays out of scope).

## Read
- `docs/porting.md`, `docs/testing.md`, `docs/inventory.md` (FEAT-SPELL, FEAT-SPELLBOOK, FEAT-SUND).
- MME through the index: frmMain `FindNegates`, `LookUpSpellCast`, `chkSpellOptions_Click`,
  `cmdSundryChests_Click`, the "find negate" case of `mnuSpellsPopUpItem_Click` and `mnuAuxPopUpItem_Click`
  (they build the arrays `FindNegates` takes); `frmSpellBook/` (`cmdListSpells_Click`, `cmbClass_Click`,
  `CopyGlobalChar`, `Form_Load`); `modMMudDatabase GetChestItems`.
- Merged siblings: `locations.rs` (`GetLocations`), `textblock.rs` (`get_chest_items`),
  `filters/spells.rs`, `rows.rs add_spell_row`, `spell_cast.rs` (`SpellIsUsable`), `bin/touchstone/src/evaluate.rs`.

## Decisions
- `chkSpellOptions_Click` only enables the Magery-level controls and the third option while option 0 is
  ticked; it is web-only. The Spells explorer's options already carry the class/magery filter, so the
  card wires the enable/disable rule and nothing else.
- `FindNegates` runs over the item and spell tables in table order (progress bar and Results window are UI
  and skipped). The rules take the three arrays MME builds (`nSpells`, `nAbils`, level) and return rows
  `Item: name (number)` / `Spell: name (number)`. A second function ports the array building done by the
  two menu handlers (the spell's own abilities, the `endcast` spells and the level prompt), so the server
  endpoint takes only a spell number and an optional level. The level prompt is a request field: MME asks
  only when the spell's min and max are both 0 and it has a per-level increase.
- `LookUpSpellCast` is `GetLocations` over the spell's `Casted By` text; the server returns the location rows.
- Chest rows are sorted by percent descending in MME (`SortListViewByTag`). The rules return the rows in
  MME's insertion order and the server sorts them (stable), a ListView tie order being unknowable.
- The Spell Book lists every spell `SpellIsUsable` accepts for the class, level and alignment, with
  `AddSpell2LV` rows. The character profile is used only when the global filter is on, the class is the
  global class and the book's magery equals the character's (the first block of `cmdListSpells_Click`).
- The builder's Spellbook button opens the book at level 999 with that class, magery and magery level.

## Do
- `touchstone-rules`: `spell_tools` (counters, what-casts), `chests`, `spell_book`; oracle families
  `spell_tools`, `spell_book`, `chests`, each at most 1.5 MB, deterministic.
- Server: endpoints pinned to fixture case ids.
- Web: Spell Book dialog under the toolbar's Tools, "What casts this" and "Find counters" dialogs,
  the Chests action on the Items explorer, the context-menu items and builder button enabled.
- Screenshots `/tmp/P4-32/shots/spell-book.png` and `what-casts.png` (stock db, 1440x900, dsf 2, dark).

## Done when
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `tools/guard.sh`, web `npm ci && npm run check && npm test && npm run build`,
`oracle.py check --jobs 2` current for every family, all new cases pass, the screenshots exist.
