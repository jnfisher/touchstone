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
monster `avg_dmg*`, `avg_lair_exp`, `script_value`; the whole `Lairs` table; `Rooms.lair`.

## Items (`items`, `item_abilities`, `item_classes`, `item_races`, `item_negates`)

| Field | Status | Source / fill |
|---|---|---|
| `number` | direct | `items.number` |
| `name` | direct | `items.name` |
| `limit` | direct | `items.limit_copies` |
| `encum` | direct | `items.weight` |
| `item_type` | direct | `items.item_type` (same ITEM_* codes) |
| `use_count` | derived | `items.charges`; silvermere's -1 ("no charges") becomes 0 |
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
| `obtained_from` | none | `None`. NMR's text lists shops, monsters and rooms that yield the item; a feature that shows it must compute it from `shop_items`, `monster_items` and `room_placed_items` |
| `references` | none | `None`; NMR's cross-reference text |

Not mapped (no model field): descriptions, hidden descriptions, `level`, `energy`, the `cash_*`
container coins, hit/miss/destroy messages, `textnum`, `robable`, `active`.

## Monsters (`monsters`, `monster_attacks`, `monster_spells`, `monster_items`, `monster_abilities`, `messages`)

| Field | Status | Source / fill |
|---|---|---|
| `number` | direct | `monsters.number` |
| `name` | direct | `monsters.name` (`Some`) |
| `weapon` | direct | `monsters.weapon` |
| `armour_class` | direct | `monsters.ac` |
| `damage_resist` | direct | `monsters.dr` |
| `follow_pct` | direct | `monsters.followpct` |
| `magic_res` | direct | `monsters.magicres` |
| `bs_defense` | direct | `monsters.bsdefense` |
| `exp` | direct | `monsters.exp` |
| `exp_multi` | derived | `monsters.level` (silvermere awards `exp * level`). This is the one reading of MME's EXP/ExpMulti split I could not confirm against the stock database, because the converted `.mdb` has no level column; a mismatch would show up in the oracle comparison |
| `hp` | direct | `monsters.maxhp` |
| `energy` | direct | `monsters.energy` |
| `avg_dmg`, `avg_dmg_phys`, `avg_dmg_spell` | none | `0.0`; NMR computes these from the attacks, accuracy and the monster's spells |
| `greet_txt` | direct | `monsters.talktext` |
| `hp_regen` | direct | `monsters.hpregen` |
| `charm_lvl` | direct | `monsters.charmlvl` |
| `type` | direct | `monsters.montype` |
| `undead` | direct | `monsters.undead` |
| `align` | direct | `monsters.alignment` |
| `regen_time` | direct | `monsters.regenhrs` |
| `game_limit` | direct | `monsters.maxalive` |
| `avg_lair_exp` | none | `0.0`; NMR averages over the lairs that spawn the monster |
| `script_value` | none | `0.0`; NMR's own ranking score |
| `r`, `p`, `g`, `s`, `c` | direct | `monsters.cash_runic`, `_platinum`, `_gold`, `_silver`, `_copper` (silvermere already reversed the record's runic-first order) |
| `death_spell` | direct | `monsters.deathspell` |
| `create_spell` | direct | `monsters.createspell` |
| `attacks[n].name` | derived | `messages.text1` (the line the target sees, "slashes you") of the attack's `hitmsg`; an unused slot is `"None"` as in MME; a used slot whose message is missing is `""` |
| `attacks[n].att_type` | direct | `monster_attacks.kind` |
| `attacks[n].acc` | direct | `monster_attacks.accuracy` |
| `attacks[n].pct` | direct | `monster_attacks.pct` (cumulative in both) |
| `attacks[n].true_pct` | derived | step between this slot's cumulative `pct` and the previous used slot's. MME's stored value is sometimes a little different (for example 49.2 against a step of 50), so this is an approximation of NMR's value |
| `attacks[n].min`, `.max` | direct | `monster_attacks.min`, `.max` |
| `attacks[n].energy` | direct | `monster_attacks.energy` |
| `attacks[n].hit_spell` | direct | `monster_attacks.hitspell` |
| `mid_spells[n]` | direct | `monster_spells.spell`, `.pct`, `.level` by slot |
| `drops[n]` | direct | `monster_items.item`, `.pct` by slot (`uses` is dropped) |
| `abilities[n]` | direct | `monster_abilities.ability`, `.value` by slot |
| `in_game` | none | always `true` |
| `summoned_by` | none | `None`; NMR's text of the groups and lairs that spawn the monster |

## Spells (`spells`, `spell_abilities`)

| Field | Status | Source / fill |
|---|---|---|
| `number` | direct | `spells.number` |
| `name` | direct | `spells.name` (`Some`) |
| `short` | direct | `spells.shortnm` |
| `req_level` | direct | `spells.level` |
| `energy_cost` | direct | `spells.energy` |
| `mana_cost` | direct | `spells.mana` |
| `min_base`, `max_base` | direct | `spells.minval`, `spells.maxval` |
| `diff` | direct | `spells.diff` |
| `type_of_resists` | direct | `spells.resstat` (RSTAT_*; 0 = magic resistance) |
| `targets` | direct | `spells.target` |
| `dur` | direct | `spells.duration` |
| `att_type` | direct | `spells.element` (ELEM_*; MME's AttType uses the same numbering, 4 = normal) |
| `magery`, `magery_lvl` | direct | `spells.magery`, `spells.magelvl` |
| `cap` | direct | `spells.levelcap` |
| `max_inc_lvls`, `max_inc` | direct | `spells.maxlvls`, `spells.maxinc` |
| `min_inc_lvls`, `min_inc` | direct | `spells.minlvls`, `spells.mininc` |
| `dur_inc_lvls`, `dur_inc` | direct | `spells.durlvls`, `spells.durinc` |
| `abilities[n]` | direct | `spell_abilities.ability`, `.value` by slot |
| `learnable` | none | `false`; NMR derives it from the scrolls and trainers that teach the spell |
| `learned_from`, `casted_by`, `classes` | none | `None`; NMR's cross-reference text. `classes` could be derived from class magery later |

Not mapped: descriptions, `resist`, `flags`, `spell_type` (SPT_*), `resmode`, `msgnum`. If the
oracle shows MME's `TypeOfResists` is `resmode` rather than `resstat` for some spells, change the
one line in the loader.

## Rooms (`rooms`, `room_exits`, `room_placed_items`)

| Field | Status | Source / fill |
|---|---|---|
| `map_number`, `room_number` | direct | `rooms.map`, `rooms.room` |
| `name` | direct | `rooms.name` (`Some`) |
| `light` | direct | `rooms.light` |
| `shop` | direct | `rooms.shopnum` |
| `npc` | direct | `rooms.specmon` |
| `cmd` | direct | `rooms.cmdtext` |
| `spell` | direct | `rooms.roomspell` |
| `lair` | none | `None`; NMR's text of the monsters a lair room generates (`Max n: ...[group]`), which depends on its lair grouping rules |
| `delay` | direct | `rooms.regendelay` |
| `placed` | derived | `room_placed_items.item` joined as `"7,8,"` (each followed by a comma, as MME writes it); `None` when empty |
| `n`, `s`, `e`, `w`, `ne`, `nw`, `se`, `sw`, `u`, `d` | derived | from `room_exits` (slot = direction). No exit is `"0"`. Otherwise `map/room`: the same map, or `par1` for a map-change exit. A suffix names the exit kind: ` (Door)`, ` (Spell)`, ` (Key)`, ` (Item)`, ` (Toll)`, ` (Hidden)`, ` (Other)`. MME prints the exit's parameters too (`(Key: 1124 [or 301 picklocks/strength])`, `(Trap, 30 damage)`, `Action [...]`), and the loader does not rebuild those: the destination is exact, the suffix text is not. Anything that parses exits for pathing only needs the leading `map/room` |

## Shops (`shops`, `shop_items`, `rooms`)

| Field | Status | Source / fill |
|---|---|---|
| `number`, `name` | direct | `shops.number`, `shops.name` |
| `shop_type` | direct | `shops.shop_type` |
| `min_lvl`, `max_lvl` | direct | `shops.minlevel`, `shops.maxlevel` |
| `markup_pct` | direct | `shops.markup` |
| `class_rest` | direct | `shops.trainclass` |
| `stock[n].item` | direct | `shop_items.item` by slot |
| `stock[n].max` | direct | `shop_items.maxqty` |
| `stock[n].time` | direct | `shop_items.regentime` |
| `stock[n].amount` | direct | `shop_items.regennum` |
| `stock[n].pct` | direct | `shop_items.regenpct` |
| `in_game` | derived | some room has `shopnum` equal to the shop |
| `assigned_to` | derived | those rooms as `Room map/room`, joined with `, ` |

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
| `group_index`, `mob_list`, `mobs`, `total_lairs`, `avg_delay`, `avg_walk`, `avg_exp`, `avg_dmg`, `avg_dmg_phys`, `avg_dmg_spell`, `avg_hp`, `avg_ac`, `avg_dr`, `avg_mr`, `avg_dodge` | none | `World.lairs` is empty. NMR builds the table from room lair data, monster stats and walking distance; silvermere has no equivalent. Safe because nothing in the loader or the model indexes depends on it; features that read lairs show nothing for a live database until a card computes them |

## TBInfo (`text_blocks`, `monsters`, `rooms`)

| Field | Status | Source / fill |
|---|---|---|
| `number` | direct | `text_blocks.number`, one row per block |
| `link_to` | direct | `text_blocks.link` of the block's first part |
| `action` | none | `None`; NMR's parsed ASK keywords/actions of the text |
| `called_from` | derived | comma-joined `Monster #n` (monster `talktext`), `Room map/room` (room `cmdtext`) and `Textblock #n` (another block linking here); empty when nothing points at it. Item text references are not included (the model has no `textnum`) |

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
