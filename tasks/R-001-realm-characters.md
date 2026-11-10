---
id: R-001
title: Realm characters, read, map and serve
model: sonnet
phase: realm
depends: [Q-004, P3-44, P4-20]
touches:
  - crates/touchstone-data/src/silvermere/
  - crates/touchstone-data/tests/
  - bin/touchstone/src/realm.rs
  - bin/touchstone/src/api.rs
  - bin/touchstone/src/lib.rs
  - bin/touchstone/src/state.rs
  - bin/touchstone/src/source.rs
  - bin/touchstone/src/character.rs
  - bin/touchstone/tests/
  - tools/qs-vectors/
  - fixtures/quicksilver/
  - fixtures/known_gaps.json
  - docs/design.md
  - docs/differences.md
---

## Goal
John runs touchstone beside his Quicksilver server. The server side of the Characters tab: every
character in the realm is listed, and each one's full stat sheet is served, touchstone's own
calculation beside the game's stored values with each difference flagged. Cards R-002 and R-003
do the web. A realm character is read-only and always live.

## Read
- `docs/design.md` (live silvermere path), `docs/quicksilver-profile.md` part 3 (profile choice),
  `docs/testing.md`, `docs/differences.md` tail.
- `bin/touchstone/src/character.rs` (`CharacterDto`, `refresh`, `equip_slot`, `usable_env`),
  `crates/touchstone-rules/src/pipeline.rs` (`refresh_all`, `CharacterState`).
- The Quicksilver checkout, read-only, in own words only: the characters and `character_*` table
  comments in migration 0001, how a worn item is stored (first free slot, not by location), the
  balance tool's character builder and `secondary`/`dynamic_stats` (which stored values it writes).
- `tools/qs-vectors/src/cells.rs` for how a generator module builds characters.

## Decisions
- **Reader** (`touchstone-data`): `Silvermere::load_snapshot()` returns the `World` and the
  `Realm` (every character, with worn, inventory, spellbook, abilities and active spells) from one
  consistent read. Columns are named one by one, never `SELECT *`; `accounts`, `suicpw`, `userid`
  (the login name) and `character_profiles` are never read. An `.mdb` source has no realm: the
  API answers `{available: false}`.
- **Mapping** (`bin/touchstone/src/realm.rs`): game worn slots are a list (first free slot), so each
  worn item goes where MME's own `equip_slot` puts it by its worn type, and the wielded weapon goes
  in slot 16. An item for which MME has no slot, or a second item for a slot MME holds one in,
  goes in `unmapped` with the reason. Inventory not worn or wielded becomes `carried` stacks.
  Spellbook is the learned list; an active spell that is a bless spell fills a bless slot, any
  other active spell is `unmapped` (its effect still shows in the stored values).
  Alignment comes from the evil points (good below -50, evil from 40, else neutral).
- **Comparison**: `{field, label, game, touchstone, differs, tolerance}`. Exact for integers both
  sides compute; max HP is within MME's min..max range (the game rolls HP at training); the
  Alter bonuses are compared against the plain ability sums over class, race, worn items and
  carried items plus the active spells' abilities.
- **Endpoints**: `GET /api/realm/characters`, `/{id}`, `/{id}/input`. List is cached per load.
- **Fixture**: `tools/qs-vectors` `realm` module builds 8 to 12 characters with the balance
  tool's builder (training ladder, engine `secondary`, can-use checks), adds a caster with active
  spells and an item that cannot map, and writes `fixtures/quicksilver/realm_characters.sql`
  (insert statements, one stub account row with blank secrets each). Tests apply it to a copy of
  the cached Quicksilver database and skip, saying so, when that is absent.

## Done when
Workspace checks pass; the endpoints return the fixture characters with sheets and comparisons;
the privacy test passes; every differing field in the pinned comparison has a cause in
`docs/differences.md` or an entry in `fixtures/known_gaps.json`.
