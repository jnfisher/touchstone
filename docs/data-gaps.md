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

**Derived by T-014** (`touchstone_data::derive`, run inside `load_world()` on the same snapshot):
the `Lairs` table and `Rooms.lair`, monster `summoned_by`, item `obtained_from`, spell
`learnable`, `learned_from`, `casted_by` and `classes`. Match rates against the stock `.mdb` are
in `docs/data-parity.md`. They are *partial* where marked below: the part that comes out of
text-block scripts is not reproduced, so the text is a prefix-compatible subset of NMR's, never
a different list. The derivations read each other (monsters, then items, then spells), so an
item that only a script hands out reads as not obtainable and a spell it teaches loses that
source.

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
| `obtained_from` | derived, partial | `Room m/r` (placed or hidden in the room; sorted by room number then map, as the `.mdb`), `Shop #n` / `Shop(sell) #n` (shop buys it, no quantity) / `Shop(nogen) #n` (stocked, never restocked) for shops in the game, `Monster #n(p%)` for monsters in the game, joined with `, ` in that order, cut to 1,996 characters with the last replaced by `+`. 66% of items match exactly. Of the 659 that differ, 588 differ only by entries the loader does not produce: `Textblock #n(p%)` (an item given by a text-block line, p from the line's cumulative percentage), `NPC #n` (given through a monster's ask tree) and `Item #n(p%)` (a chest that opens into it). Those need a reachability walk of the text-block scripts. The other 71 are mostly monsters that are in the game only through a script (so they read as not in the game here), a few monsters the `.mdb` lists with a different slot percentage than silvermere's drop table, and the rooms and shop 65 that exist only in silvermere's world (the 26 extra rooms) |
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
| `summoned_by` | derived, partial | `Room m/r` (the room's specific monster), then the rooms whose generation range covers the monster, joined with `,`: `Group: m/r` or, for a lair room, `[group-min-max][max]Group(lair): m/r`; then `Spell #n` for summon spells (ability 12; the monster is its value, or the spell's base value when that is 0); rooms and spells ordered by room number then map and spell number. Cut at 20,000 characters like the `.mdb`. 89% of monsters match. The 116 that differ: 101 only by `Textblock #n(p%)` entries (scripts that summon the monster, not reproduced); the other 15 by the order or set of `Spell #n` entries, the one room silvermere has beyond the `.mdb` (monsters 266 to 268) and the 20,000 cut (monster 229) |

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
| `learnable` | derived, partial | true when some item in the game teaches the spell (item ability 42, value = spell). 96.7% match; the 46 others are taught only by an NPC (a monster's ask script with `learnspell`) |
| `learned_from` | derived, partial | `Item #n` for those items, by item number, joined with `, `. 96.6% match; the misses are the NPC-taught spells (`NPC #n`) and spells whose scroll the `.mdb` does not count as obtainable |
| `classes` | derived, partial | `(*)` when a teaching item has no class restriction, else `(c,c)` of the classes the items name. The `.mdb` also gives `(12)`-style values for NPC-taught spells (the class line of the ask script); those are not reproduced. 96.7% match |
| `casted_by` | derived, partial | `Monster #n` (mid spell, hit spell, spell attack, death or create spell; monsters in the game), then `Room m/r` (occupant spell), then `Spell #n` (spells with ability 151 naming it), joined with `, `, cut to 250 characters. 62% match: 317 of the 518 misses are only `Textblock`, `Item` or `NPC` entries (casts from scripts and item effects, not reproduced); most of the rest are the **order of the monsters**, which in the `.mdb` follows an NMR scan order the loader could not recover (the set is right, the loader sorts by monster number) |

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
| `action` | none | `None`; NMR's parsed ASK keywords/actions of the text. Not attempted by T-014: it needs the full text-block script parser (the same one `called_from` below and the `Textblock`/`NPC` entries above wait for) |
| `called_from` | derived | comma-joined `Monster #n` (monster `talktext`), `Room map/room` (room `cmdtext`) and `Textblock #n` (another block linking here); empty when nothing points at it. Item text references are not included (the model has no `textnum`). NMR also lists blocks referenced from inside another block's script (`Textblock #998(6%)`, `Textblock(rndm) #997`), which needs the script parser (not done by T-014, see `action` above); about half of the blocks that have a caller differ for that reason. The `.mdb` has 3 blocks silvermere lacks (11, 60, 134) and silvermere has 944 the `.mdb` lacks (block 0 and the 10000 series). |

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
