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
- `Quicksilver`: later. Stock plus the Quicksilver balance changes.

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
