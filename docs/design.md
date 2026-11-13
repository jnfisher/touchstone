# Design

touchstone ports MMUD Explorer (MME) to a Rust server with a Svelte web UI. The goal is the same
answers as MME (parity), presented in a modern browser UI, served from one binary on a Raspberry
Pi 4 (aarch64, 4 GB) beside a silvermere/Quicksilver server.

## Reference sources

| Source | What it is | How it is used |
|---|---|---|
| MME | VB6 app, `jnfisher/MMUD-Explorer` pinned at `28d23d70caf4f971418aa7ea25471cdb0ab0888a` (2026-10-25, v2.3.5) | **The spec.** Formulas, screens, filters and columns are ported from it. Fetched by `tools/mme-fetch.sh` into `~/.cache/touchstone/mme/<sha>/`; never committed. |
| MME stock database | `data-v1.11p.mdb` in the same commit, sha256 `eba20f06af8dc08df755eb6dcd9763b023794f12e809cbab6762236d64d7345f` | Parity data. Fetched and converted by `tools/mdb-fetch.sh`; never committed. |
| silvermere | `jnfisher/silvermere` (public, MIT) | Schema of the live database; its migrations may be vendored for test fixtures. |
| Quicksilver | `jnfisher/quicksilver-dev` (private) | The deployment target's schema and content changes (`docs/balance`, `docs/differences-1.20.md`) feed the later `Quicksilver` profile. |
| Reconstructed game source | `jnfisher/silvermere-dev` (private) | Read-only reference for how the game actually behaves, used to explain MME's numbers and to build the `Quicksilver` profile. **Never** copied, quoted or named in this repository (see CLAUDE.md). |

The MME pin moves only by a deliberate card that bumps it and regenerates the fixtures.

## Crates and layout

```
crates/
  touchstone-model    plain data types: World { items, monsters, spells, rooms, shops, classes,
                      races, lairs, textblocks, info }, Character, enums. No IO.
  touchstone-vb6      VB6 numeric/string semantics (CInt banker's rounding, Fix/Int, `\`, Mod,
                      Integer/Long overflow, Val, Round) so ported formulas match exactly.
  touchstone-rules    MME's formulas, ported. Pure functions over model types plus a Profile.
  touchstone-data     loaders that build a World: from a converted MME .mdb (SQLite produced by
                      mdbtools), and from a silvermere-schema SQLite database, read-only and live.
bin/touchstone        axum server: JSON API + the built web UI, CLI (clap), config (TOML).
web/                  SvelteKit (adapter-static, SPA mode) + TypeScript + Tailwind + shadcn-svelte
                      + AG Grid Community.
tools/
  mme-fetch.sh        fetch MME source at the pin into the cache
  mdb-fetch.sh        fetch the stock .mdb at the pin, check sha256, convert to SQLite (mdbtools)
  mme-index/          split MME source into one file per procedure + an index (Python, stdlib)
  oracle/             VB6-subset interpreter that runs MME's own modules to produce fixtures
fixtures/             golden JSON produced by the oracle: inputs and expected outputs
tasks/                task cards (T-NNN-*.md), STATUS.md
```

Dependencies point downward: `model <- vb6 <- rules`, `model <- data`, and
`{rules, data} <- bin/touchstone`. `rules` and `model` never do IO.

## Data

**Internal model.** `World` mirrors the MME `.mdb` tables (Items, Monsters, Spells, Rooms, Shops,
Classes, Races, Lairs, Info, TBInfo) with typed fields, because MME's formulas are written against
those columns. Field names follow the `.mdb` column names (snake_cased), so a ported formula reads
like the original.

**MME `.mdb` path.** `tools/mdb-fetch.sh` converts the database to SQLite once with mdbtools
(`mdb-schema` + `mdb-export`), cached at `~/.cache/touchstone/mdb/<sha256>.sqlite`. The loader
reads that SQLite, so Rust never parses Jet files. For a user-supplied `.mdb`, the CLI runs the
same conversion (needs `mdbtools` installed: `apt install mdbtools`).

**Live silvermere/Quicksilver path.** The server opens the game's SQLite file read-only
(`mode=ro`). The game runs in WAL mode, so reads never block it; touchstone must run as the
game's Unix user or with group read access to the data directory (WAL readers need the `-shm`
file). On startup it checks the migration version it was built for and refuses with a clear
message if the schema is unknown. It polls `PRAGMA data_version` (every 10 s by default) and,
when it changes, rebuilds the `World` in the background and swaps it in (`arc-swap`). The adapter
maps silvermere's normalised tables (items + item_abilities + ...) into the same `World`. Fields
MME has and silvermere lacks (for example NMR's computed references) are listed in
`docs/data-gaps.md`, not guessed.

**Characters** are saved server-side as JSON files in the data directory, one per named
character. MME `.mmec` files (INI format) can be imported and exported.

## Formulas and profiles

`touchstone-rules` takes a `Profile`:

- `Stock`: the default and the only one tested for parity. It reproduces MME's stock (non-
  GreaterMUD) branches exactly, including VB6 rounding and integer behaviour (`touchstone-vb6`).
  Where MME disagrees with the real game, parity wins and the difference goes in
  `docs/differences.md`.
- `Quicksilver(&Balance)`: stock plus the Quicksilver balance changes, each rule arriving with its
  own card. The inventory of those changes, how it is tested and the cards are in
  `docs/quicksilver-profile.md`.

The server chooses the profile once per load (`bin/touchstone/src/profile.rs`) and every computed
endpoint reads `Loaded::profile`, so a live reload can change it and a request never mixes two. The
choice comes from `Info.content_version`, Quicksilver's structured content version, never from the
`dat_file_version` display caption: no content version (an `.mdb`, a stock silvermere) is `Stock`,
`1.20.N` is `Quicksilver` with the 1.20.0 balance, and any other value refuses to start (a reload
that meets one keeps the previous world and logs why). `touchstone.toml` can set `[rules] profile`
to `auto` (default), `stock` (ignore Quicksilver's rules, for comparisons; needs no known version) or
`quicksilver` (the 1.20 rules whatever the data says), and `[rules.quicksilver_balance]` overrides
individual balance keys under Quicksilver's own key names. The override is validated at startup,
leaked once per process, and `/api/info` lists the keys that differ from 1.20.0 under
`profile.balance_overrides`.

MME's GreaterMUD and Paramud branches are not ported unless a card asks for them; keep the
branching visible (a `match` on profile) so they can be added.

## Server and API

axum, JSON. The game data is small (about 2k items, 1.1k monsters, 1.4k spells, 27k rooms), so
the UI fetches whole tables once and filters and sorts in the browser:

- `GET /api/info`: data source, version, loaded-at, data_version.
- `GET /api/{items,monsters,spells,rooms,shops,classes,races,lairs}`: full tables.
- `GET /api/characters`, `GET/PUT/DELETE /api/characters/{name}`: saved characters as
  `<data_dir>/characters/<name>.json`, written atomically.
- `POST /api/characters/import` (raw `.mmec` body; `name`, `file`, `overwrite` query) and
  `GET /api/characters/{name}/export.mmec`: MME character files, parsed and written by
  `touchstone_rules::mmec`. Keys touchstone does not model ride along in the character's `extra`.
- `POST /api/character/evaluate`: a character in, MME's Char-tab captions and numbers out
  (`pipeline::refresh_all`); 422 "pick a class and race" without both.
- `GET /api/character/bless-spells?class=&level=&alignment=&all=&include=`: `FilterBlessSpells`.
- `POST /api/evaluate/{weapons,armour,spells,items,races,classes,shops}` with
  `{character?, options}`: MME's list for that tab (`Add*2LV` rows) as `{columns, rows}`; `cells`
  line up with `columns` (the `#` column first), each `{text, sort, color?}`. A character is used
  when sent with `global_filter: true`; without one, MME's no-character view. Cached per request
  until the data reloads. Monsters (exp/hr, damage) are a later card.
- `POST /api/calc/{swing,hit,backstab,exp,coin,monster-attack-sim}`: the calculator windows.
- `POST /api/calc/weapon-sim` with `{character, ignore_level, ignore_alignment, include_limited, target, rounds,
  candidates, seed, pinned}`: the Weapon Simulator, touchstone's own (see below).
- `POST /api/calc/gear-plan` with `{character, goal, ignore_level, ignore_alignment, include_limited,
  only_obtainable, second_wrist, target, min_ac, max_encum, locked, skipped}`: the Gear Planner, touchstone's own (see below).
- `GET/PUT /api/settings`: the options that change results (exp/hr models and knobs, monster sim
  rounds, route waypoint interval, open-last-character), stored as `<data_dir>/settings.json`. PUT
  applies MME's Save rules (knob clamp, round and interval ranges, A+B+C when no model is ticked)
  and returns what it stored; a damaged file is reported by GET and moved aside by the next PUT.
  `?character=NAME` adds the exp/hr block that character calculates with. With "by character" on,
  that is the character's own `exp_hr` block (missing keys read as defaults, as in MME). View
  options (theme, name in the page title) stay in browser storage. `tasks/P4-29-settings.md` maps
  every `settings.ini` key to server, browser or not applicable.

Config: `touchstone.toml` (data source, bind address, default `0.0.0.0:8370`, data dir). No
authentication: it is meant for a LAN or Tailscale.

In release builds the web UI is embedded in the binary (feature `embed-web`, `rust-embed`).
Debug builds and tests serve `web/build` from disk if present and do not need Node.

## Web UI

SvelteKit in SPA mode, TypeScript, Tailwind, shadcn-svelte for chrome, AG Grid Community for
every list (column filters, multi-sort, resize, pinning, virtual scrolling, copy). Dark by
default with a light toggle. Layout follows MME: one tab per explorer (Weapons, Armour, Spells,
Monsters, Shops, Items, Races, Classes, Rooms, Character, Compare), keyboard shortcuts F1 to F11, a
detail pane beside each list, and a compare pane. Columns, filters and the meaning of each value
copy MME one to one; the look is modern.

## Deployment

The release workflow builds `aarch64-unknown-linux-musl` and `x86_64-unknown-linux-musl`
binaries with the web UI embedded. `packaging/` holds a systemd unit and an example config.
Builds never happen on the Pi.

## Later (optional)

Map / room explorer, MegaMUD pathing, monster reachability and routes, live game state (what is
spawned now). Decide at the end of phase 4.

## Item Manager (P4-19)

The Item Manager list is the page's: `/api/item-manager/{rows,action,delete,copy,add}` take the rows and the
selected positions and answer the new rows plus the character with its saved `carried`/`stash` lists rebuilt
(`SaveCharacter`'s loops, 50 each). Rows live in the paste store so a paste and the panel share one list.
Questions go through the same stateless prompt loop as the paste dialog. The character's `carried` list feeds
`refresh_all` (`gear_rows`), which is how CARRIED rows count for abilities and encumbrance.

## Context menus and clipboard (P4-15)

`contextmenu.ts` decides which items MME's PopUp menus show per list (pure, tested); `menuactions.svelte.ts` runs them against
the stores; `RowContextMenu.svelte` renders them. Calculators and the combat setup open prefilled through `handoff.svelte.ts`.
`POST /api/clipboard/{character,monster,shop,lines}` answer `{text, error}` from `touchstone_rules::clipboard`; the page copies with
`navigator.clipboard` and falls back to a selectable dialog.

## Realm characters (R-001)

A live silvermere or Quicksilver source also has players. `Silvermere::load_snapshot` reads the world and the realm
(`characters` and its `character_*` tables) from one snapshot, so a reload brings new characters and changed gear with
it; an `.mdb` source has no realm. The reader names every column and never reads `accounts`, the suicide password, the
login name kept on a character, or `character_profiles`, so none of them can reach an API response. A realm character is
read-only: `bin/touchstone/src/realm.rs` maps it to the character input the Characters tab posts (worn gear by MME's own
equip rule, the rest of the pack as carried stacks, the spellbook as learned spells, running bless spells as bless slots,
coins as extra weight; what has no place goes in an `unmapped` list), evaluates it under the source's profile, and
compares the game's stored values with touchstone's. The sheets of a load are built on first use and kept until the next
reload.

- `GET /api/realm/characters`: `{available, characters}`, one summary row each (id, names, race, class, level, exp, gang,
  map, room, last on, lives, `differs`, the count of flagged comparisons). `available: false` for a source with no realm.
- `GET /api/realm/characters/{id}`: identity, the game's stored values, the evaluated sheet (the shape of
  `/api/character/evaluate`), worn gear with the MME slot each took, inventory, spellbook, running spells, abilities,
  `comparison` (`{field, label, game, touchstone, differs, tolerance, cause}`) and `unmapped`.
- `GET /api/realm/characters/{id}/input`: the mapped character, ready to copy into the sandbox editor or post to the
  calculators.

`docs/differences.md` ("Realm characters") lists the tolerances and the known kinds of difference.

## Weapon Simulator (W-001)

Calculators > Weapon Simulator lists the best weapons for the character in use. It is not an MME window. The server
(`bin/touchstone/src/weapon_sim.rs`) evaluates the character once, scores every weapon it may wield with
`CalculateAttack` (attack, bash and, for a weapon with backstab accuracy and a character with stealth, backstab),
keeps the best few by each plus the weapon in hand and any pinned ones, and rolls those round by round
(`touchstone_rules::weapon_sim`, seeded and deterministic). A row carries, for each attack, MME's damage a round, the
simulated mean, spread (low, median, high, the share of rounds that deal nothing) and, when the target has hit points,
the rounds to kill. The page has a view for regular rounds (ranked by the better of attack and bash) and one for
backstab rounds, and shows the selected weapon's numbers above its item pane.

It is sized for a Raspberry Pi: scoring is a few hundred closed-form calls, the dice are integer arithmetic on one
core, one request runs at a time, and the request's limits (50,000 rounds, 60 weapons an attack) bound the worst
case to a few hundred million dice. `docs/differences.md` ("Weapon simulator") says how the dice follow MME's numbers.

## Gear Planner (G-001)

Calculators > Gear Planner fills the worn slots (every slot but the weapon) with the gear that does most for one goal:
melee damage, backstab, armour class, dodge, spellcasting, mana regen, hit points or stealth. It is not an MME window;
MME's Find Best ranks one slot's items by one ability. The server (`bin/touchstone/src/gear_plan.rs`) lists what each
slot may hold under the request's switches (`usable.rs`, shared with the Weapon Simulator) and hands the search to
`touchstone_rules::gear_plan`: a climb that tries each candidate of a slot in the set as it stands, scoring a set by
evaluating the character wearing it, and repeats over each slot's best few until no swap helps. The answer has, per
slot, the item worn, the item suggested, what that swap alone is worth, the runners-up, and for a suggested item its
sources (`touchstone_rules::acquire`, read from the Obtained From line). The page shows the slots beside a
before-and-after of the character's numbers, and "Where to get it" groups the sources into a kill list, shops and the
rest.

A plan is a few hundred to a few thousand evaluations of about 50 microseconds each, on one core, in the same
one-at-a-time queue as the Weapon Simulator.
