# Silvermere/Quicksilver adapter: field mapping and gaps

`touchstone_data::silvermere` (T-012) builds the same `World` as the `.mdb` loader from a running
silvermere or Quicksilver SQLite database. silvermere stores the game's own records, normalised;
MME's `.mdb` is NMR's database, which adds computed and curated columns. This page lists every
model field, where its value comes from, and what the loader puts there when silvermere has no
source. Statuses: **direct** (a column copied), **derived** (computed by the loader from silvermere
data), **none** (no source; the loader fills a constant).

Supported schema: sqlx migration versions 1 and 2 (silvermere), plus 9001 (Quicksilver, adds
`world_import.content_version`). Any other version, a missing base migration, an unfinished
migration or a half-written import (`world_import.finished_at` NULL) is refused with a message.

Rules that must not trust a `none` field (all are values NMR computes from other tables, so a
rules function that needs one derives it from the base data instead of reading the field):
monster `avg_dmg*`, `avg_lair_exp`, `script_value`, `in_game`; `Lairs.avg_walk`, `avg_dmg*`.

**Derived by T-014 and T-016** (`touchstone_data::derive`, run inside `load_world()` on the same
snapshot): the `Lairs` table and `Rooms.lair`, monster `summoned_by`, item `obtained_from`, spell
`learnable`, `learned_from`, `casted_by` and `classes`, and TBInfo `action` and `called_from`.
Match rates against the stock `.mdb` are in `docs/data-parity.md`. T-016 added the part that comes
out of text-block scripts (see "Text-block scripts" below). The derivations read each other
(monsters, then items, then spells, then the scripts those make reachable), so they run again
until nothing changes (six passes on the stock game and on Quicksilver 1.20).

### Text-block scripts (T-016)

NMR reads every script the game can reach. `derive/script.rs` reads a block's lines and
`derive/walk.rs` walks the graph; the rules below were learned from the stock `.mdb` and
reproduce it exactly except where "Not reproduced" says otherwise.

- **Lines.** A line whose first segment (before the first `:`) starts with a clause keyword is a
  list of clauses. Otherwise, if the last word of that segment is a whole number, it is a
  *random line* and the number is a cumulative percentage (the entry's share is the difference
  to the previous random line, so an out-of-order line gives a negative share and a typo such
  as `40;message 2914` takes `2914` as the number); otherwise it is a pattern line (an ask or
  command pattern) and its first segment is skipped.
- **References.** A clause whose first word is a number, or `text n`, points at a block that is
  shown; `random n` at a random block (`Textblock(rndm) #n`); `testskill s v n` and
  `checkspell s n` at a block that is run. A block that is only shown has no `action` (its link
  is followed); a block that is run, or reached by a link from a shown block, is a script only if
  one of its clauses starts with a keyword other than `message` and `addevil` (the word must be
  the keyword itself). A `checkspell` branch counts only if the spell tested is in the game.
  Roots (room commands, monster ask trees, spell text blocks) are always scripts.
- **Roots.** Room commands (`Rooms.cmd`) and the text-block ability (148; the value, or the
  spell's base value when it is 0) of spells that are in the game; the ask tree of monsters in
  the game. A spell is in the game when an item teaches it or something casts it; a room casts
  through its occupant spell and through Cast (22) and Spell Trap (24) exits.
- **Order.** The walk takes the smallest block first, in two phases: first the room roots, the
  roots of monsters that are in the game through a room, a lair or a summon spell, and the spells
  monsters cast; then everything else. A block more than two links from its root waits for the
  second phase. Entries on a record are ordered by the pass of the derivation loop that first
  produced them, so what rooms, lairs and drops give comes before what only scripts reach.
- **Effects.** `summon n` (monster n), `giveitem n`, `cast n` and `learnspell n` are listed as
  `Textblock #b(p%)` on the target, with the share of the line when it is a random line, once
  for each block and share. An item given or a spell taught in a block that a monster's ask tree
  reaches is credited to the monster instead (`NPC #m`, once per monster).
- **Not reproduced.** (1) `Item #n(p%)` is estimated as the chance that one use of chest item
  n gives the item, from its spell's block, treating every `random` clause as an independent draw
  and a random block as one line picked by share; of several chests that cast one spell only the
  first is listed; the figure matches NMR when the chest's blocks give items directly and is
  higher than NMR's (by 1% to 25% of itself) when random blocks call random blocks, and the rule
  behind NMR's figure is not known. (2) A few lists are in a different order (see each field).
  (3) The 26 rooms silvermere has beyond the `.mdb` (record numbers in `docs/data-parity.md`)
  run scripts the `.mdb` never read; the monsters, items and spells those scripts reach, and the
  TBInfo rows for their blocks, appear only on the silvermere side.

## Items (`items`, `item_abilities`, `item_classes`, `item_races`, `item_negates`)

| Field | Status | Source / fill |
|---|---|---|
| `number` | direct | `items.number` |
| `name` | direct | `items.name` |
| `limit` | direct | `items.limit_copies` |
| `encum` | direct | `items.weight` |
| `item_type` | direct | `items.item_type` (same ITEM_* codes) |
| `use_count` | direct | `items.charges`; both keep -1 for "no charges" (checked by `docs/data-parity.md`) |
| `price` | direct | `items.cost` |
| `currency` | direct | `items.currency` |
| `min`, `max` | direct | `items.mindmg`, `items.maxdmg` |
| `armour_class` | direct | `items.ac` |
| `damage_resist` | direct | `items.dr` |
| `weapon_type` | direct | `items.wpntype` |
| `armour_type` | direct | `items.armtype` |
| `worn` | direct | `items.worn` |
| `accy` | direct | `items.accuracy` |
| `gettable` | direct | `items.gettable` |
| `str_req` | direct | `items.strreq` |
| `speed` | direct | `items.speed` |
| `not_droppable` | direct | `items.nodrop` |
| `destroy_on_death` | direct | `items.destroy` |
| `retain_after_uses` | direct | `items.retain` |
| `class_rest[0..9]` | direct | `item_classes.class` by slot; unused slots 0 |
| `race_rest[0..9]` | direct | `item_races.race` by slot |
| `negate_spell[0..9]` | direct | `item_negates.spell` by slot |
| `abilities[0..19]` | direct | `item_abilities.ability`, `.value` by slot |
| `in_game` | none | always `true`: a row in the server's table is in the game |
| `obtained_from` | derived, partial | `Room m/r` (placed or hidden in the room; sorted by room number then map, as the `.mdb`), `Shop #n` / `Shop(sell) #n` (shop buys it, no quantity) / `Shop(nogen) #n` (stocked, never restocked) for shops in the game, `Monster #n(p%)` for monsters in the game (a monster with two slots of the item and one percentage once), `Textblock #b(p%)` for scripts that give it, `Item #n(p%)` for a chest item that opens into it, then `NPC #m` for a monster whose ask tree gives it; joined with `, `, cut to 1,996 characters with the last replaced by `+`. Entries are ordered by the pass that first produced them (rooms, shops and drops of monsters that are in the game through a room or lair come first). 87.5% of items match exactly. Of the 244 that differ: 185 list the right chests with different percentages (`Item #n(p%)`, see "Not reproduced" above); 22 list a shop the `.mdb` does not (the shops in game because of silvermere's extra rooms); 16 have the right entries in another order (script entries of spells cast by items); 9 + 6 + 3 list a room, monster or block that exists only because of the 26 extra rooms; 3 others |
| `references` | none | `None`; NMR's cross-reference text |

Not mapped (no model field): descriptions, hidden descriptions, `level`, `energy`, the `cash_*`
container coins, hit/miss/destroy messages, `textnum`, `robable`, `active`.

## Monsters (`monsters`, `monster_attacks`, `monster_spells`, `monster_items`, `monster_abilities`, `messages`)

| Field | Status | Source / fill |
|---|---|---|
| `number` | direct | `monsters.number` |
| `name` | direct | `monsters.name`; an empty name is `None`, as the `.mdb` has NULL there |
| `weapon` | direct | `monsters.weapon` |
| `armour_class` | direct | `monsters.ac` |
| `damage_resist` | direct | `monsters.dr` |
| `follow_pct` | direct | `monsters.followpct` |
| `magic_res` | direct | `monsters.magicres` |
| `bs_defense` | direct | `monsters.bsdefense` |
| `exp` | direct | `monsters.exp` |
| `exp_multi` | derived | `monsters.level` (silvermere awards `exp * level`). Confirmed against the stock database by the T-015 parity run: `exp` and `exp_multi` match for every monster |
| `hp` | direct | `monsters.maxhp` |
| `energy` | direct | `monsters.energy` |
| `avg_dmg`, `avg_dmg_phys`, `avg_dmg_spell` | none | `0.0`; NMR computes these from the attacks, accuracy and the monster's spells |
| `greet_txt` | direct | `monsters.talktext` |
| `hp_regen` | direct | `monsters.hpregen` |
| `charm_lvl` | direct | `monsters.charmlvl` |
| `type` | direct | `monsters.grouprole` (solo, leader, follower, stationary). `montype` is a different, finer code and does not match; found by the T-015 parity run |
| `undead` | direct | `monsters.undead` |
| `align` | direct | `monsters.alignment` |
| `regen_time` | direct | `monsters.regenhrs` |
| `game_limit` | direct | `monsters.maxalive` |
| `avg_lair_exp` | none | `0.0`; NMR averages over the lairs that spawn the monster |
| `script_value` | none | `0.0`; NMR's own ranking score |
| `r`, `p`, `g`, `s`, `c` | direct | `monsters.cash_runic`, `_platinum`, `_gold`, `_silver`, `_copper` (silvermere already reversed the record's runic-first order) |
| `death_spell` | direct | `monsters.deathspell` |
| `create_spell` | direct | `monsters.createspell` |
| `attacks[n].name` | derived | by the attack's kind: 0 is `"None"` (also an unused slot); 2 (spell) is the name of the spell numbered by the accuracy field; 1 is the hit message's `text0` ("The %s slashes you for %d damage!") reduced to "slashes you" (text before `%s `, and the damage tail, removed; a message without a damage figure is named by its first word; over 49 characters cut to 46 plus `...`); an empty message is `"Physical"`; any other kind is `"Unknown"`. All but one stock monster name match |
| `attacks[n].att_type` | direct | `monster_attacks.kind` |
| `attacks[n].acc` | direct | `monster_attacks.accuracy` |
| `attacks[n].pct` | direct | `monster_attacks.pct` (cumulative in both) |
| `attacks[n].true_pct` | derived | step between this slot's cumulative `pct` and the previous used slot's. This is NMR's own computed value and is only approximated: NMR weighs the slots by their energy and appears to simulate the choice (values like 49.2 against a step of 50, 73.3 against 75), gives 0 to kind-0 slots, and gives spell attacks a different share. It matches exactly for about a third of the monsters (one attack, or equal energies and no spells) and is within 3 points for another third. A rules function that needs the real attack mix derives it from `pct`, `energy` and `att_type` |
| `attacks[n].min`, `.max` | direct | `monster_attacks.min`, `.max` |
| `attacks[n].energy` | direct | `monster_attacks.energy` |
| `attacks[n].hit_spell` | direct | `monster_attacks.hitspell` |
| `mid_spells[n]` | direct | `monster_spells.spell`, `.pct`, `.level` by slot |
| `drops[n]` | direct | `monster_items.item`, `.pct` by slot (`uses` is dropped). An empty slot is 0/0; the `.mdb` keeps a stale percentage on some empty slots (615 of them), which has no source and no meaning |
| `abilities[n]` | direct | `monster_abilities.ability`, `.value` by slot |
| `in_game` | none | always `true` |
| `summoned_by` | derived, partial | `Room m/r` (the room's specific monster), then the rooms whose generation range covers the monster, joined with `,`: `Group: m/r` or, for a lair room, `[group-min-max][max]Group(lair): m/r`; then `Spell #n` for summon spells that are in the game (ability 12; the monster is its value, or every monster from the spell's base value to its base maximum when that is 0), then `Textblock #b(p%)` for the scripts that summon it (`summon n`), each kind in the order its entries were produced; rooms and spells ordered by room number then map and spell number. Cut at 20,000 characters like the `.mdb`. 97.5% of monsters match. Of the 27 that differ, 24 involve the 26 rooms silvermere has beyond the `.mdb` (a `Group: 1/288` entry for monsters 266 to 268, and scripts of the extra rooms that summon monsters 1093 to 1112) and 3 list the right entries in another order (monsters 239, 891, 1315) |

## Spells (`spells`, `spell_abilities`)

| Field | Status | Source / fill |
|---|---|---|
| `number` | direct | `spells.number` |
| `name` | direct | `spells.name`; empty is `None` |
| `short` | direct | `spells.shortnm`; empty is `None` (the `.mdb` has NULL for 651 spells) |
| `req_level` | direct | `spells.level` |
| `energy_cost` | direct | `spells.energy` |
| `mana_cost` | direct | `spells.mana` |
| `min_base`, `max_base` | direct | `spells.minval`, `spells.maxval` |
| `diff` | direct | `spells.diff` |
| `type_of_resists` | direct | `spells.resmode` (0 magic resistance, 1 stat, 2 other). Settled by the parity run: `resstat` did not match |
| `targets` | direct | `spells.target` |
| `dur` | direct | `spells.duration` |
| `att_type` | direct | `spells.element` (ELEM_*; MME's AttType uses the same numbering, 4 = normal) |
| `magery`, `magery_lvl` | direct | `spells.magery`, `spells.magelvl` |
| `cap` | direct | `spells.levelcap` low byte; the field is 16 bits and 74 spells have padding in the high byte (8224 reads 32) |
| `max_inc_lvls`, `max_inc` | direct | `spells.maxlvls`, `spells.maxinc` |
| `min_inc_lvls`, `min_inc` | direct | `spells.minlvls`, `spells.mininc` |
| `dur_inc_lvls`, `dur_inc` | direct | `spells.durlvls`, `spells.durinc` |
| `abilities[n]` | direct | `spell_abilities.ability`, `.value` by slot |
| `learnable` | derived | true when some item in the game teaches the spell (item ability 42, value = spell) or a script in the game has a `learnspell` clause for it. 99.35% match; the 9 others are taught by a script of the extra rooms (block 4356) |
| `learned_from` | derived | `Item #n` for those items, by item number, then `NPC #m` (a monster whose ask tree reaches the `learnspell` clause, once per monster) or `Textblock #b` (any other script), joined with `, `. 99.35% match; the 9 others are the extra rooms' block 4356 |
| `classes` | derived | `(*)` when a teaching item has no class restriction or a `learnspell` line has no `class n` clause, else `(c,c)` of the classes the items and lines name. 99.35% match; the 9 others are the extra rooms' block 4356 |
| `casted_by` | derived, partial | `Room m/r` (occupant spell, or a Cast or Spell Trap exit), `Monster #n` (mid spell, hit spell, spell attack, death or create spell; monsters in the game), `Spell #n` (spells in the game with ability 151 or 153 naming it; with value 0 every spell from the base value to the base maximum), `Textblock #b(p%)` (`cast n` clauses of scripts) and `Item #n` (obtainable items with ability 43), joined with `, `, each entry placed by the pass that first produced it, cut to 249 characters (248 and a `+`). 97.5% match. Of the 34 that differ, 14 involve the 26 extra rooms (their monsters and blocks), 16 list the right entries in another order (the `.mdb` orders monsters and items by NMR's own rounds, which the passes only approximate) and 4 are other cases |

Not mapped: descriptions, `resist`, `flags`, `spell_type` (SPT_*), `msgnum`.

## Rooms (`rooms`, `room_exits`, `room_placed_items`)

| Field | Status | Source / fill |
|---|---|---|
| `map_number`, `room_number` | direct | `rooms.map`, `rooms.room` |
| `name` | direct | `rooms.name`; empty is `None` (140 rooms in the `.mdb`) |
| `light` | direct | `rooms.light` |
| `shop` | derived | `rooms.shopnum` for a shop room (`room_type` 1), else 0. Vaults, lockers and inn side rooms carry the number in silvermere but not in MME (1,447 rooms) |
| `npc` | direct | `rooms.specmon` |
| `cmd` | direct | `rooms.cmdtext` |
| `spell` | direct | `rooms.roomspell` |
| `lair` | derived | for a lair room (`room_type` 3) with a generation range (`maxlevel` > 0): `(Max n): 1,2,[group-min-max-n]`, where `n` is `maxmonsters`, the list is the known monsters of type `mongroup` (-1 any) whose generation index is within `minlevel..maxlevel` (0 means no lower limit), ordered by generation index then number, and the bracket repeats `mongroup-minlevel-maxlevel` and `n`. At most 15 monsters of one type and generation index are listed (the stock data has one pool of 18 where NMR lists 15; no other pool reaches the limit, so this is inferred from that one case). A room's `genmonster` is ignored: rooms that set it list the pool all the same. 100% of the 26,694 rooms match |
| `delay` | derived | `rooms.regendelay`, with 0 read as the default 5 (7,049 rooms) |
| `placed` | derived | `room_placed_items.item` joined as `"7,8,"` (each followed by a comma, as MME writes it); `None` when empty |
| `n`, `s`, `e`, `w`, `ne`, `nw`, `se`, `sw`, `u`, `d` | derived | from `room_exits` (slot = direction). No exit is `"0"` (also a hidden exit with all-zero parameters). Otherwise `map/room`: the same map, or `par1` for a map-change exit, followed by MME's suffix rebuilt from the exit kind and `par1..par4`: `(Door)` or `(Door [N picklocks/strength])`, `(Key: K ...)`, `(Item: N)`, `(Toll: N)`, `(Hidden/Passable)`, `(Hidden/Searchable)`, `(Hidden/Needs N Actions, any|specific order)`, `(Trap, N damage)`, `(Class: ..)`, `(Race: ..)`, `(Level: a to b)`, `(Ticket/Item: N)`, `(Cast: pre-a, post-b)`, `(Ability: ..)`, `(Spell Trap: N)`. The destination and these suffixes match the stock `.mdb`. Not rebuilt, and `(Other)` instead: text-triggered exits (`(Text: go path, ...)` takes its words from a text block), action exits (`Action [on the N exit ...]`), alignment-gated exits and timed exits; 547 of the 26,694 x 10 exit fields differ for this reason alone. A consumer parsing exits for pathing only needs the leading `map/room`, which is right for every field except those of the `Action` kind (MME prints no destination for them) |

## Shops (`shops`, `shop_items`, `rooms`)

| Field | Status | Source / fill |
|---|---|---|
| `number`, `name` | direct | `shops.number`, `shops.name` |
| `shop_type` | direct | `shops.shop_type` |
| `min_lvl`, `max_lvl` | direct | `shops.minlevel`, `shops.maxlevel` |
| `markup_pct` | direct | `shops.markup` |
| `class_rest` | direct | `shops.trainclass` low byte (padding in the high byte on some shops) |
| `stock[n].item` | direct | `shop_items.item` by slot |
| `stock[n].max` | direct | `shop_items.maxqty`. Shop 123 (the gang shop) has stock in silvermere and none in the `.mdb`; that is the only stock difference |
| `stock[n].time` | direct | `shop_items.regentime` |
| `stock[n].amount` | direct | `shop_items.regennum` |
| `stock[n].pct` | direct | `shop_items.regenpct` |
| `in_game` | derived | some room has `shop` equal to the shop. Three unused shops (39, 65, 85) read true because they are placed in rooms the stock `.mdb` does not have (silvermere's world has 26 more rooms than MME's) |
| `assigned_to` | derived | those rooms as `Room map/room`, in text order ("Room 10/271" before "Room 3/542", as MME), joined with `, `. Differs for the same three shops and for two more that also list one of the 26 extra rooms |

## Classes and races

| Field | Status | Source / fill |
|---|---|---|
| Class `number`, `name` | direct | `classes.number`, `.name` |
| Class `min_hits`, `max_hits` | direct | `classes.basehp`, `classes.hplevel` |
| Class `exp_table` | direct | `classes.exp` |
| Class `magery_type`, `magery_lvl` | direct | `classes.magery`, `classes.magelvl` |
| Class `weapon_type`, `armour_type` | direct | `classes.weapon`, `classes.armour` |
| Class `combat_lvl` | direct | `classes.combat` |
| Class `abilities[n]` | direct | `class_abilities` by slot |
| Race `number`, `name` | direct | `races.number`, `.name` |
| Race `m_int`, `m_wil`, `m_str`, `m_hea`, `m_agl`, `m_chm` | direct | `races.minint`, `minwil`, `minstr`, `minhea`, `minagi`, `mincha` |
| Race `x_int`, `x_wil`, `x_str`, `x_hea`, `x_agl`, `x_chm` | direct | `races.maxint`, `maxwil`, `maxstr`, `maxhea`, `maxagi`, `maxcha` |
| Race `hp_per_lvl` | direct | `races.hpbonus` |
| Race `exp_table` | direct | `races.exp` |
| Race `base_cp` | direct | `races.basecp` |
| Race `abilities[n]` | direct | `race_abilities` by slot |

## Lairs

| Field | Status | Source / fill |
|---|---|---|
| `group_index` | derived | `mongroup-minlevel-maxlevel` of the lair rooms (see `Rooms.lair`); one row per distinct value, in text order with the hyphens ignored (`10-10-10` before `10-1-1`) |
| `mob_list`, `mobs` | derived | the room pool without monsters that have a regeneration time (`regenhrs` > 0: NMR leaves uniques out of the table but not out of the room text); a group whose pool is then empty has no row (5 groups). 100% of 421 rows |
| `total_lairs` | derived | lair rooms of the group |
| `avg_delay` | derived | mean of those rooms' `delay`, rounded half to even |
| `avg_exp`, `avg_hp`, `avg_ac`, `avg_dr`, `avg_mr` | derived | mean over the listed monsters of `exp * exp_multi`, `hp`, `armour_class`, `damage_resist`, `magic_res`, rounded half to even |
| `avg_dodge` | derived | mean of the monsters' Dodge ability (34) value, 0 for monsters without it |
| `avg_dmg`, `avg_dmg_phys`, `avg_dmg_spell` | derived from monster fields | mean of the monsters' `avg_dmg*`, rounded; **0 until the phase-3 rules fill the monsters' damage**, and then the lairs must be re-derived. Checked against the stock monsters' values the match is only about 92%: the `.mdb` rounds the monster values to one decimal and the lair averages come from the unrounded ones |
| `avg_walk` | none | `0.0`. NMR's walking distance is not recoverable: for single-lair groups it takes values like 0, 2, 20, 84 and 150 that are not the distance to the nearest other lair, to the controlling room, or to any fixed room that was tried |

## TBInfo (`text_blocks`, `monsters`, `rooms`)

| Field | Status | Source / fill |
|---|---|---|
| `number` | direct | `text_blocks.number`, one row per block |
| `link_to` | direct | `text_blocks.link` of the block's first part |
| `action` | derived | the text of the block (all its parts joined) when the block is read as a script (a root, a `random` target, or a `testskill`, `checkspell` or link target with an action keyword), else `None`. 100% of the paired rows match |
| `called_from` | derived | `Room map/room` (room command; room number then map), `Monster #n` (ask tree of a monster in the game), `Spell #n` (text-block ability of a spell in the game), then `Textblock #b` (a bare number, `text n`, `testskill`, `checkspell` or a link), `Textblock(rndm) #b` (`random n`) and `(p%)` when the calling line is a random line; entries ordered by the pass that first produced them and by the walk; cut to 249 characters. Rows exist only for blocks the game can reach, as in the `.mdb` (silvermere has 3,267 blocks, the `.mdb` 2,326 rows): the walk reaches 34 blocks the `.mdb` has no row for, all from the commands of the 26 extra rooms, and the `.mdb` has 3 rows (11, 60, 134) silvermere has no block for. 99.4% of the paired rows match; the 13 others are 10 that list monsters of the extra rooms and 3 (500, 9383, 9396) with the right entries in another order. Item text references are not included (the model has no `textnum`) |

## Info

| Field | Status | Source / fill |
|---|---|---|
| `nmr_version` | none | `""` |
| `dat_file_version` | derived | Quicksilver's `world_import.content_version`, else `"silvermere"` |
| `date`, `time` | derived | `world_import.finished_at` (RFC 3339) split at `T`; `""` when there is no import row |
| `custom` | none | `"silvermere"` |
| `legit` | none | `true` |
| `disable_kai` | none | `false` |
| `update_url` | none | `""` |

## Quicksilver 1.20 census (Q-002)

`tools/qs-fetch.sh` builds Quicksilver's own binary from a local checkout (`QUICKSILVER_DEV`,
default `/home/user/quicksilver-dev`, read-only; the build goes to `~/.cache/touchstone/quicksilver/target`
or `CARGO_TARGET_DIR`), runs its default import (the stock package plus the 1.20 content the binary
carries) and leaves `~/.cache/touchstone/quicksilver-1.20.sqlite`. The stock package comes from the
silvermere work directory's cache when present, so the import runs offline. The first build takes
about ten minutes (Quicksilver pins its own toolchain); the import takes six seconds. The database is
never committed. `python3 -I tools/qs-census.py` prints everything below (`--json` for tooling); the
data test `crates/touchstone-data/tests/all/quicksilver.rs` pins the load (it skips, with a message,
when the database is not in the cache; `TOUCHSTONE_QS_SQLITE` names another path).

**Result: the loader reads the 1.20 database as it is. No change to `touchstone-data` is needed, and
there are no unknown ability numbers.**

| Check | Result |
|---|---|
| Schema | Migrations 1, 2 and 9001, all successful. The loader accepts them (`SUPPORTED_MIGRATIONS`) |
| Content version | `world_import.content_version` is `1.20.0`, so `Info.dat_file_version` is `"1.20.0"`. The 1.20.1 release changed no rule, balance or content and keeps `1.20.0`, so a 1.20.1 binary's world still reads `1.20.0`. Q-004 must match `1.20.N`, not the binary version |
| Row counts against stock | Items 1,974 (+24), spells 1,457 (+78), shops 178, shop items 1,258 (+23), races 13, race abilities 40 (+3), item abilities 6,800 (+48), item negates 101 (+4), spell abilities 6,129 (+467), messages 3,875 (+8). Monsters 1,101, rooms 26,720, classes 15, text blocks 3,467 and all monster tables are unchanged in count. The 1.20 content rewrites rows in place too (for example 93 spells have a different level cap); counts are not the whole delta |
| Slot capacity | No child table overflows the model's arrays. Highest slot used: item abilities 9 of 20, spell, class and item class/negate slots 9 of 10, shop items 19 of 20, race abilities 4 of 10 |
| Ability numbers | 145 distinct numbers in use, the same count as stock, none above 185, none new against stock. touchstone names 0 to 187 (`ability_text.rs`), so **every ability in the 1.20 content is known**. The six message-only abilities (101, 115, 120, 137, 144, 155) occur, as in stock |
| Race regeneration | Human gains HPRegen (123) 15 and ManaRgn (145) 15, Dwarf gains HPRegen 15 (in a slot stock filled with a stray value beside ability 0), Elf gains ManaRgn 15 and Gaunt One ManaRgn 25 (replacing a stray value beside ability 0). Both are stock abilities, 123 and 145 in `get_ability_stat_slot`, and no other race changes. The stray-value slots are the ones `AbilitySlot` already documents |
| Reserved spells | 57 spells in 5000 to 5999 (monster copies), 21 in 6000 to 6020 (mana regeneration). The regeneration spells use abilities 115 (DescMsg), 122 (RemovesSpell) and 145 (ManaRgn), all known, and every one is taught by an item (ability 42), so `learnable` and `learned_from` derive for them. Fifteen of the 57 copies are cast by a monster row; the other 42 are reached by negating items, removers or scripts, so `casted_by` is empty for them (the same partial `casted_by` as stock) |
| Spell columns | New values only in `maxinc` (13, 15, 19), `maxlvls` (20) and `minlvls` (13): ordinary increments from Spell Mod v2.1. 65 spells keep a padded 16-bit level cap (74 in stock); the loader's low-byte read covers them |
| Other enum columns | Item types, worn slots, weapon and armour types, spell elements, resist modes, targets, monster attack kinds, exit types and shop types hold no code stock does not |
| References | No row points at a missing spell or item, except one monster drop (the same one in stock) |
| Wide integers | `room_exits.par1` holds 14 values outside 32 bits, exactly as stock; the loader reads exit parameters as 64-bit, so nothing is lost |

What this means for the Q cards:

- Q-003 and Q-004 need no data work beyond the content-version check above. `Info.content_version`
  is the string `1.20.0` on this database.
- The spell, item and monster rows that 1.20 changes already arrive through the loader, so section 2
  of `docs/quicksilver-profile.md` holds: nothing in it needs porting.
- The derived fields (`obtained_from`, `learned_from`, `casted_by`, `summoned_by`, lairs, TBInfo
  `action` and `called_from`) keep their stock caveats. There is no NMR reference for a 1.20 world,
  so their match rates cannot be measured as `docs/data-parity.md` does for stock.
- T-016: the text-block derivation runs on this database (the data test
  `quicksilver_derives_the_script_fields_quickly` in `tests/all/derive.rs`). It settles in six
  passes. A whole `load_world()` takes about 0.6 s with the derivation in a release build on the
  development machine, 0.35 s of that the derivation (the base load was 0.3 s), so a live reload
  stays cheap on a small board.
