# World model (`touchstone-model`)

`World` holds one `Vec` per MME table plus number-to-index maps; `get_item(n)`, `get_monster(n)`,
`get_spell(n)`, `get_room(map, room)`, `get_shop(n)`, `get_class(n)`, `get_race(n)`,
`get_lair(group_index)` and `get_tbinfo(n)` look up by the MME record number, not by position.
`World::new(WorldParts)` builds the indexes and rejects a repeated key. `Info` is a single row.

Conventions for loaders:

- Field names are the column names in snake_case (`Follow%` -> `follow_pct`, `Not Droppable` ->
  `not_droppable`, `mINT` -> `m_int`, column `Type` -> `r#type`).
- Integer columns are `i32` everywhere; every observed value fits `i16`, but later data versions may
  not, and one width avoids casts in ported formulas. `REAL` columns are `f64`. Columns that only
  hold 0 or 1 are `bool` (loader: non-zero is true).
- Text columns that contain NULL in the stock data are `Option<String>`; a NULL is `None`, an empty
  string stays `Some("")`. Other text columns are `String`.
- Numbered series are fixed arrays indexed by the column's number: `Abil-n` plus `AbilVal-n` form
  `abilities[n]` (`AbilitySlot { ability, value }`, 10 slots, 20 for Items). Unused slots are 0.
  A few Spells rows have a non-zero `AbilVal-n` beside `Abil-n` = 0 (for example 8224); keep it as read.
- Monsters group `AttName..AttHitSpell-n` into `attacks[n]`, `MidSpell*` into `mid_spells[n]`,
  `DropItem*` into `drops[n]`; Shops group `Item/Max/Time/Amount/%-n` into `stock[n]`.
- Rooms exit columns (`N`..`D`) stay as MME's text (`0`, `1/3`, `1/1381 (Door)`); parsing them is a
  rules/loader concern.
- Enums (`ItemType`, `WeaponType`, `ClassWeaponType`, `ArmourType`, `WornSlot`, `Currency`,
  `SpellTargets`, `SpellAttackType`, `ShopType`, `MonsterAttackType`, `MonsterType`, `Alignment`,
  `Magery`) exist only where `modMMudFunc` names the codes. Rows keep the raw number; accessors such
  as `Item::item_type_enum()` return `Option<Enum>` (`None` for codes MME calls unknown).

## Column mapping

Every column of every table, once. Series columns are listed one by one.

## Classes -> `Class`

| Column | Field | Type |
|---|---|---|
| `Number` | `number` | `i32` |
| `Name` | `name` | `String` |
| `MinHits` | `min_hits` | `i32` |
| `MaxHits` | `max_hits` | `i32` |
| `ExpTable` | `exp_table` | `i32` |
| `MageryType` | `magery_type` | `i32` |
| `MageryLVL` | `magery_lvl` | `i32` |
| `WeaponType` | `weapon_type` | `i32` |
| `ArmourType` | `armour_type` | `i32` |
| `CombatLVL` | `combat_lvl` | `i32` |
| `Abil-0` | `abilities[0].ability` | `i32` |
| `AbilVal-0` | `abilities[0].value` | `i32` |
| `Abil-1` | `abilities[1].ability` | `i32` |
| `AbilVal-1` | `abilities[1].value` | `i32` |
| `Abil-2` | `abilities[2].ability` | `i32` |
| `AbilVal-2` | `abilities[2].value` | `i32` |
| `Abil-3` | `abilities[3].ability` | `i32` |
| `AbilVal-3` | `abilities[3].value` | `i32` |
| `Abil-4` | `abilities[4].ability` | `i32` |
| `AbilVal-4` | `abilities[4].value` | `i32` |
| `Abil-5` | `abilities[5].ability` | `i32` |
| `AbilVal-5` | `abilities[5].value` | `i32` |
| `Abil-6` | `abilities[6].ability` | `i32` |
| `AbilVal-6` | `abilities[6].value` | `i32` |
| `Abil-7` | `abilities[7].ability` | `i32` |
| `AbilVal-7` | `abilities[7].value` | `i32` |
| `Abil-8` | `abilities[8].ability` | `i32` |
| `AbilVal-8` | `abilities[8].value` | `i32` |
| `Abil-9` | `abilities[9].ability` | `i32` |
| `AbilVal-9` | `abilities[9].value` | `i32` |

## Info -> `Info`

| Column | Field | Type |
|---|---|---|
| `NMR Version` | `nmr_version` | `String` |
| `Dat File Version` | `dat_file_version` | `String` |
| `Date` | `date` | `String` |
| `Time` | `time` | `String` |
| `Custom` | `custom` | `String` |
| `Legit` | `legit` | `bool` |
| `DisableKai` | `disable_kai` | `bool` |
| `UpdateURL` | `update_url` | `String` |

## Items -> `Item`

| Column | Field | Type |
|---|---|---|
| `Number` | `number` | `i32` |
| `Name` | `name` | `String` |
| `Limit` | `limit` | `i32` |
| `Encum` | `encum` | `i32` |
| `ItemType` | `item_type` | `i32` |
| `UseCount` | `use_count` | `i32` |
| `Price` | `price` | `i32` |
| `Currency` | `currency` | `i32` |
| `Min` | `min` | `i32` |
| `Max` | `max` | `i32` |
| `ArmourClass` | `armour_class` | `i32` |
| `DamageResist` | `damage_resist` | `i32` |
| `WeaponType` | `weapon_type` | `i32` |
| `ArmourType` | `armour_type` | `i32` |
| `Worn` | `worn` | `i32` |
| `Accy` | `accy` | `i32` |
| `Gettable` | `gettable` | `bool` |
| `StrReq` | `str_req` | `i32` |
| `Speed` | `speed` | `i32` |
| `Not Droppable` | `not_droppable` | `bool` |
| `Destroy On Death` | `destroy_on_death` | `bool` |
| `Retain After Uses` | `retain_after_uses` | `bool` |
| `ClassRest-0` | `class_rest[0]` | `i32` |
| `ClassRest-1` | `class_rest[1]` | `i32` |
| `ClassRest-2` | `class_rest[2]` | `i32` |
| `ClassRest-3` | `class_rest[3]` | `i32` |
| `ClassRest-4` | `class_rest[4]` | `i32` |
| `ClassRest-5` | `class_rest[5]` | `i32` |
| `ClassRest-6` | `class_rest[6]` | `i32` |
| `ClassRest-7` | `class_rest[7]` | `i32` |
| `ClassRest-8` | `class_rest[8]` | `i32` |
| `ClassRest-9` | `class_rest[9]` | `i32` |
| `RaceRest-0` | `race_rest[0]` | `i32` |
| `RaceRest-1` | `race_rest[1]` | `i32` |
| `RaceRest-2` | `race_rest[2]` | `i32` |
| `RaceRest-3` | `race_rest[3]` | `i32` |
| `RaceRest-4` | `race_rest[4]` | `i32` |
| `RaceRest-5` | `race_rest[5]` | `i32` |
| `RaceRest-6` | `race_rest[6]` | `i32` |
| `RaceRest-7` | `race_rest[7]` | `i32` |
| `RaceRest-8` | `race_rest[8]` | `i32` |
| `RaceRest-9` | `race_rest[9]` | `i32` |
| `NegateSpell-0` | `negate_spell[0]` | `i32` |
| `NegateSpell-1` | `negate_spell[1]` | `i32` |
| `NegateSpell-2` | `negate_spell[2]` | `i32` |
| `NegateSpell-3` | `negate_spell[3]` | `i32` |
| `NegateSpell-4` | `negate_spell[4]` | `i32` |
| `NegateSpell-5` | `negate_spell[5]` | `i32` |
| `NegateSpell-6` | `negate_spell[6]` | `i32` |
| `NegateSpell-7` | `negate_spell[7]` | `i32` |
| `NegateSpell-8` | `negate_spell[8]` | `i32` |
| `NegateSpell-9` | `negate_spell[9]` | `i32` |
| `Abil-0` | `abilities[0].ability` | `i32` |
| `AbilVal-0` | `abilities[0].value` | `i32` |
| `Abil-1` | `abilities[1].ability` | `i32` |
| `AbilVal-1` | `abilities[1].value` | `i32` |
| `Abil-2` | `abilities[2].ability` | `i32` |
| `AbilVal-2` | `abilities[2].value` | `i32` |
| `Abil-3` | `abilities[3].ability` | `i32` |
| `AbilVal-3` | `abilities[3].value` | `i32` |
| `Abil-4` | `abilities[4].ability` | `i32` |
| `AbilVal-4` | `abilities[4].value` | `i32` |
| `Abil-5` | `abilities[5].ability` | `i32` |
| `AbilVal-5` | `abilities[5].value` | `i32` |
| `Abil-6` | `abilities[6].ability` | `i32` |
| `AbilVal-6` | `abilities[6].value` | `i32` |
| `Abil-7` | `abilities[7].ability` | `i32` |
| `AbilVal-7` | `abilities[7].value` | `i32` |
| `Abil-8` | `abilities[8].ability` | `i32` |
| `AbilVal-8` | `abilities[8].value` | `i32` |
| `Abil-9` | `abilities[9].ability` | `i32` |
| `AbilVal-9` | `abilities[9].value` | `i32` |
| `Abil-10` | `abilities[10].ability` | `i32` |
| `AbilVal-10` | `abilities[10].value` | `i32` |
| `Abil-11` | `abilities[11].ability` | `i32` |
| `AbilVal-11` | `abilities[11].value` | `i32` |
| `Abil-12` | `abilities[12].ability` | `i32` |
| `AbilVal-12` | `abilities[12].value` | `i32` |
| `Abil-13` | `abilities[13].ability` | `i32` |
| `AbilVal-13` | `abilities[13].value` | `i32` |
| `Abil-14` | `abilities[14].ability` | `i32` |
| `AbilVal-14` | `abilities[14].value` | `i32` |
| `Abil-15` | `abilities[15].ability` | `i32` |
| `AbilVal-15` | `abilities[15].value` | `i32` |
| `Abil-16` | `abilities[16].ability` | `i32` |
| `AbilVal-16` | `abilities[16].value` | `i32` |
| `Abil-17` | `abilities[17].ability` | `i32` |
| `AbilVal-17` | `abilities[17].value` | `i32` |
| `Abil-18` | `abilities[18].ability` | `i32` |
| `AbilVal-18` | `abilities[18].value` | `i32` |
| `Abil-19` | `abilities[19].ability` | `i32` |
| `AbilVal-19` | `abilities[19].value` | `i32` |
| `In Game` | `in_game` | `bool` |
| `Obtained From` | `obtained_from` | `Option<String>` |
| `References` | `references` | `Option<String>` |

## Lairs -> `Lair`

| Column | Field | Type |
|---|---|---|
| `GroupIndex` | `group_index` | `String` |
| `MobList` | `mob_list` | `String` |
| `Mobs` | `mobs` | `i32` |
| `TotalLairs` | `total_lairs` | `i32` |
| `AvgDelay` | `avg_delay` | `i32` |
| `AvgWalk` | `avg_walk` | `f64` |
| `AvgExp` | `avg_exp` | `f64` |
| `AvgDmg` | `avg_dmg` | `f64` |
| `AvgDmgPhys` | `avg_dmg_phys` | `f64` |
| `AvgDmgSpell` | `avg_dmg_spell` | `f64` |
| `AvgHP` | `avg_hp` | `f64` |
| `AvgAC` | `avg_ac` | `i32` |
| `AvgDR` | `avg_dr` | `i32` |
| `AvgMR` | `avg_mr` | `i32` |
| `AvgDodge` | `avg_dodge` | `i32` |

## Monsters -> `Monster`

| Column | Field | Type |
|---|---|---|
| `Number` | `number` | `i32` |
| `Name` | `name` | `Option<String>` |
| `Weapon` | `weapon` | `i32` |
| `ArmourClass` | `armour_class` | `i32` |
| `DamageResist` | `damage_resist` | `i32` |
| `Follow%` | `follow_pct` | `i32` |
| `MagicRes` | `magic_res` | `i32` |
| `BSDefense` | `bs_defense` | `i32` |
| `EXP` | `exp` | `f64` |
| `ExpMulti` | `exp_multi` | `f64` |
| `HP` | `hp` | `i32` |
| `Energy` | `energy` | `i32` |
| `AvgDmg` | `avg_dmg` | `f64` |
| `AvgDmgPhys` | `avg_dmg_phys` | `f64` |
| `AvgDmgSpell` | `avg_dmg_spell` | `f64` |
| `GreetTXT` | `greet_txt` | `i32` |
| `HPRegen` | `hp_regen` | `i32` |
| `CharmLVL` | `charm_lvl` | `i32` |
| `Type` | `r#type` | `i32` |
| `Undead` | `undead` | `i32` |
| `Align` | `align` | `i32` |
| `RegenTime` | `regen_time` | `i32` |
| `GameLimit` | `game_limit` | `i32` |
| `AvgLairExp` | `avg_lair_exp` | `f64` |
| `ScriptValue` | `script_value` | `f64` |
| `R` | `r` | `i32` |
| `P` | `p` | `i32` |
| `G` | `g` | `i32` |
| `S` | `s` | `i32` |
| `C` | `c` | `i32` |
| `DeathSpell` | `death_spell` | `i32` |
| `CreateSpell` | `create_spell` | `i32` |
| `AttName-0` | `attacks[0].name` | `String` |
| `AttType-0` | `attacks[0].att_type` | `i32` |
| `AttAcc-0` | `attacks[0].acc` | `i32` |
| `Att%-0` | `attacks[0].pct` | `i32` |
| `AttTrue%-0` | `attacks[0].true_pct` | `f64` |
| `AttMin-0` | `attacks[0].min` | `i32` |
| `AttMax-0` | `attacks[0].max` | `i32` |
| `AttEnergy-0` | `attacks[0].energy` | `i32` |
| `AttHitSpell-0` | `attacks[0].hit_spell` | `i32` |
| `AttName-1` | `attacks[1].name` | `String` |
| `AttType-1` | `attacks[1].att_type` | `i32` |
| `AttAcc-1` | `attacks[1].acc` | `i32` |
| `Att%-1` | `attacks[1].pct` | `i32` |
| `AttTrue%-1` | `attacks[1].true_pct` | `f64` |
| `AttMin-1` | `attacks[1].min` | `i32` |
| `AttMax-1` | `attacks[1].max` | `i32` |
| `AttEnergy-1` | `attacks[1].energy` | `i32` |
| `AttHitSpell-1` | `attacks[1].hit_spell` | `i32` |
| `AttName-2` | `attacks[2].name` | `String` |
| `AttType-2` | `attacks[2].att_type` | `i32` |
| `AttAcc-2` | `attacks[2].acc` | `i32` |
| `Att%-2` | `attacks[2].pct` | `i32` |
| `AttTrue%-2` | `attacks[2].true_pct` | `f64` |
| `AttMin-2` | `attacks[2].min` | `i32` |
| `AttMax-2` | `attacks[2].max` | `i32` |
| `AttEnergy-2` | `attacks[2].energy` | `i32` |
| `AttHitSpell-2` | `attacks[2].hit_spell` | `i32` |
| `AttName-3` | `attacks[3].name` | `String` |
| `AttType-3` | `attacks[3].att_type` | `i32` |
| `AttAcc-3` | `attacks[3].acc` | `i32` |
| `Att%-3` | `attacks[3].pct` | `i32` |
| `AttTrue%-3` | `attacks[3].true_pct` | `f64` |
| `AttMin-3` | `attacks[3].min` | `i32` |
| `AttMax-3` | `attacks[3].max` | `i32` |
| `AttEnergy-3` | `attacks[3].energy` | `i32` |
| `AttHitSpell-3` | `attacks[3].hit_spell` | `i32` |
| `AttName-4` | `attacks[4].name` | `String` |
| `AttType-4` | `attacks[4].att_type` | `i32` |
| `AttAcc-4` | `attacks[4].acc` | `i32` |
| `Att%-4` | `attacks[4].pct` | `i32` |
| `AttTrue%-4` | `attacks[4].true_pct` | `f64` |
| `AttMin-4` | `attacks[4].min` | `i32` |
| `AttMax-4` | `attacks[4].max` | `i32` |
| `AttEnergy-4` | `attacks[4].energy` | `i32` |
| `AttHitSpell-4` | `attacks[4].hit_spell` | `i32` |
| `MidSpell-0` | `mid_spells[0].spell` | `i32` |
| `MidSpell%-0` | `mid_spells[0].pct` | `i32` |
| `MidSpellLVL-0` | `mid_spells[0].lvl` | `i32` |
| `MidSpell-1` | `mid_spells[1].spell` | `i32` |
| `MidSpell%-1` | `mid_spells[1].pct` | `i32` |
| `MidSpellLVL-1` | `mid_spells[1].lvl` | `i32` |
| `MidSpell-2` | `mid_spells[2].spell` | `i32` |
| `MidSpell%-2` | `mid_spells[2].pct` | `i32` |
| `MidSpellLVL-2` | `mid_spells[2].lvl` | `i32` |
| `MidSpell-3` | `mid_spells[3].spell` | `i32` |
| `MidSpell%-3` | `mid_spells[3].pct` | `i32` |
| `MidSpellLVL-3` | `mid_spells[3].lvl` | `i32` |
| `MidSpell-4` | `mid_spells[4].spell` | `i32` |
| `MidSpell%-4` | `mid_spells[4].pct` | `i32` |
| `MidSpellLVL-4` | `mid_spells[4].lvl` | `i32` |
| `DropItem-0` | `drops[0].item` | `i32` |
| `DropItem%-0` | `drops[0].pct` | `i32` |
| `DropItem-1` | `drops[1].item` | `i32` |
| `DropItem%-1` | `drops[1].pct` | `i32` |
| `DropItem-2` | `drops[2].item` | `i32` |
| `DropItem%-2` | `drops[2].pct` | `i32` |
| `DropItem-3` | `drops[3].item` | `i32` |
| `DropItem%-3` | `drops[3].pct` | `i32` |
| `DropItem-4` | `drops[4].item` | `i32` |
| `DropItem%-4` | `drops[4].pct` | `i32` |
| `DropItem-5` | `drops[5].item` | `i32` |
| `DropItem%-5` | `drops[5].pct` | `i32` |
| `DropItem-6` | `drops[6].item` | `i32` |
| `DropItem%-6` | `drops[6].pct` | `i32` |
| `DropItem-7` | `drops[7].item` | `i32` |
| `DropItem%-7` | `drops[7].pct` | `i32` |
| `DropItem-8` | `drops[8].item` | `i32` |
| `DropItem%-8` | `drops[8].pct` | `i32` |
| `DropItem-9` | `drops[9].item` | `i32` |
| `DropItem%-9` | `drops[9].pct` | `i32` |
| `Abil-0` | `abilities[0].ability` | `i32` |
| `AbilVal-0` | `abilities[0].value` | `i32` |
| `Abil-1` | `abilities[1].ability` | `i32` |
| `AbilVal-1` | `abilities[1].value` | `i32` |
| `Abil-2` | `abilities[2].ability` | `i32` |
| `AbilVal-2` | `abilities[2].value` | `i32` |
| `Abil-3` | `abilities[3].ability` | `i32` |
| `AbilVal-3` | `abilities[3].value` | `i32` |
| `Abil-4` | `abilities[4].ability` | `i32` |
| `AbilVal-4` | `abilities[4].value` | `i32` |
| `Abil-5` | `abilities[5].ability` | `i32` |
| `AbilVal-5` | `abilities[5].value` | `i32` |
| `Abil-6` | `abilities[6].ability` | `i32` |
| `AbilVal-6` | `abilities[6].value` | `i32` |
| `Abil-7` | `abilities[7].ability` | `i32` |
| `AbilVal-7` | `abilities[7].value` | `i32` |
| `Abil-8` | `abilities[8].ability` | `i32` |
| `AbilVal-8` | `abilities[8].value` | `i32` |
| `Abil-9` | `abilities[9].ability` | `i32` |
| `AbilVal-9` | `abilities[9].value` | `i32` |
| `In Game` | `in_game` | `bool` |
| `Summoned By` | `summoned_by` | `Option<String>` |

## Races -> `Race`

| Column | Field | Type |
|---|---|---|
| `Number` | `number` | `i32` |
| `Name` | `name` | `String` |
| `mINT` | `m_int` | `i32` |
| `mWIL` | `m_wil` | `i32` |
| `mSTR` | `m_str` | `i32` |
| `mHEA` | `m_hea` | `i32` |
| `mAGL` | `m_agl` | `i32` |
| `mCHM` | `m_chm` | `i32` |
| `xINT` | `x_int` | `i32` |
| `xWIL` | `x_wil` | `i32` |
| `xSTR` | `x_str` | `i32` |
| `xHEA` | `x_hea` | `i32` |
| `xAGL` | `x_agl` | `i32` |
| `xCHM` | `x_chm` | `i32` |
| `HPPerLVL` | `hp_per_lvl` | `i32` |
| `ExpTable` | `exp_table` | `i32` |
| `BaseCP` | `base_cp` | `i32` |
| `Abil-0` | `abilities[0].ability` | `i32` |
| `AbilVal-0` | `abilities[0].value` | `i32` |
| `Abil-1` | `abilities[1].ability` | `i32` |
| `AbilVal-1` | `abilities[1].value` | `i32` |
| `Abil-2` | `abilities[2].ability` | `i32` |
| `AbilVal-2` | `abilities[2].value` | `i32` |
| `Abil-3` | `abilities[3].ability` | `i32` |
| `AbilVal-3` | `abilities[3].value` | `i32` |
| `Abil-4` | `abilities[4].ability` | `i32` |
| `AbilVal-4` | `abilities[4].value` | `i32` |
| `Abil-5` | `abilities[5].ability` | `i32` |
| `AbilVal-5` | `abilities[5].value` | `i32` |
| `Abil-6` | `abilities[6].ability` | `i32` |
| `AbilVal-6` | `abilities[6].value` | `i32` |
| `Abil-7` | `abilities[7].ability` | `i32` |
| `AbilVal-7` | `abilities[7].value` | `i32` |
| `Abil-8` | `abilities[8].ability` | `i32` |
| `AbilVal-8` | `abilities[8].value` | `i32` |
| `Abil-9` | `abilities[9].ability` | `i32` |
| `AbilVal-9` | `abilities[9].value` | `i32` |

## Rooms -> `Room`

| Column | Field | Type |
|---|---|---|
| `Map Number` | `map_number` | `i32` |
| `Room Number` | `room_number` | `i32` |
| `Name` | `name` | `Option<String>` |
| `Light` | `light` | `i32` |
| `Shop` | `shop` | `i32` |
| `NPC` | `npc` | `i32` |
| `CMD` | `cmd` | `i32` |
| `Spell` | `spell` | `i32` |
| `Lair` | `lair` | `Option<String>` |
| `Delay` | `delay` | `i32` |
| `Placed` | `placed` | `Option<String>` |
| `N` | `n` | `String` |
| `S` | `s` | `String` |
| `E` | `e` | `String` |
| `W` | `w` | `String` |
| `NE` | `ne` | `String` |
| `NW` | `nw` | `String` |
| `SE` | `se` | `String` |
| `SW` | `sw` | `String` |
| `U` | `u` | `String` |
| `D` | `d` | `String` |

## Shops -> `Shop`

| Column | Field | Type |
|---|---|---|
| `Number` | `number` | `i32` |
| `Name` | `name` | `String` |
| `ShopType` | `shop_type` | `i32` |
| `MinLVL` | `min_lvl` | `i32` |
| `MaxLVL` | `max_lvl` | `i32` |
| `Markup%` | `markup_pct` | `i32` |
| `ClassRest` | `class_rest` | `i32` |
| `Item-0` | `stock[0].item` | `i32` |
| `Max-0` | `stock[0].max` | `i32` |
| `Time-0` | `stock[0].time` | `i32` |
| `Amount-0` | `stock[0].amount` | `i32` |
| `%-0` | `stock[0].pct` | `i32` |
| `Item-1` | `stock[1].item` | `i32` |
| `Max-1` | `stock[1].max` | `i32` |
| `Time-1` | `stock[1].time` | `i32` |
| `Amount-1` | `stock[1].amount` | `i32` |
| `%-1` | `stock[1].pct` | `i32` |
| `Item-2` | `stock[2].item` | `i32` |
| `Max-2` | `stock[2].max` | `i32` |
| `Time-2` | `stock[2].time` | `i32` |
| `Amount-2` | `stock[2].amount` | `i32` |
| `%-2` | `stock[2].pct` | `i32` |
| `Item-3` | `stock[3].item` | `i32` |
| `Max-3` | `stock[3].max` | `i32` |
| `Time-3` | `stock[3].time` | `i32` |
| `Amount-3` | `stock[3].amount` | `i32` |
| `%-3` | `stock[3].pct` | `i32` |
| `Item-4` | `stock[4].item` | `i32` |
| `Max-4` | `stock[4].max` | `i32` |
| `Time-4` | `stock[4].time` | `i32` |
| `Amount-4` | `stock[4].amount` | `i32` |
| `%-4` | `stock[4].pct` | `i32` |
| `Item-5` | `stock[5].item` | `i32` |
| `Max-5` | `stock[5].max` | `i32` |
| `Time-5` | `stock[5].time` | `i32` |
| `Amount-5` | `stock[5].amount` | `i32` |
| `%-5` | `stock[5].pct` | `i32` |
| `Item-6` | `stock[6].item` | `i32` |
| `Max-6` | `stock[6].max` | `i32` |
| `Time-6` | `stock[6].time` | `i32` |
| `Amount-6` | `stock[6].amount` | `i32` |
| `%-6` | `stock[6].pct` | `i32` |
| `Item-7` | `stock[7].item` | `i32` |
| `Max-7` | `stock[7].max` | `i32` |
| `Time-7` | `stock[7].time` | `i32` |
| `Amount-7` | `stock[7].amount` | `i32` |
| `%-7` | `stock[7].pct` | `i32` |
| `Item-8` | `stock[8].item` | `i32` |
| `Max-8` | `stock[8].max` | `i32` |
| `Time-8` | `stock[8].time` | `i32` |
| `Amount-8` | `stock[8].amount` | `i32` |
| `%-8` | `stock[8].pct` | `i32` |
| `Item-9` | `stock[9].item` | `i32` |
| `Max-9` | `stock[9].max` | `i32` |
| `Time-9` | `stock[9].time` | `i32` |
| `Amount-9` | `stock[9].amount` | `i32` |
| `%-9` | `stock[9].pct` | `i32` |
| `Item-10` | `stock[10].item` | `i32` |
| `Max-10` | `stock[10].max` | `i32` |
| `Time-10` | `stock[10].time` | `i32` |
| `Amount-10` | `stock[10].amount` | `i32` |
| `%-10` | `stock[10].pct` | `i32` |
| `Item-11` | `stock[11].item` | `i32` |
| `Max-11` | `stock[11].max` | `i32` |
| `Time-11` | `stock[11].time` | `i32` |
| `Amount-11` | `stock[11].amount` | `i32` |
| `%-11` | `stock[11].pct` | `i32` |
| `Item-12` | `stock[12].item` | `i32` |
| `Max-12` | `stock[12].max` | `i32` |
| `Time-12` | `stock[12].time` | `i32` |
| `Amount-12` | `stock[12].amount` | `i32` |
| `%-12` | `stock[12].pct` | `i32` |
| `Item-13` | `stock[13].item` | `i32` |
| `Max-13` | `stock[13].max` | `i32` |
| `Time-13` | `stock[13].time` | `i32` |
| `Amount-13` | `stock[13].amount` | `i32` |
| `%-13` | `stock[13].pct` | `i32` |
| `Item-14` | `stock[14].item` | `i32` |
| `Max-14` | `stock[14].max` | `i32` |
| `Time-14` | `stock[14].time` | `i32` |
| `Amount-14` | `stock[14].amount` | `i32` |
| `%-14` | `stock[14].pct` | `i32` |
| `Item-15` | `stock[15].item` | `i32` |
| `Max-15` | `stock[15].max` | `i32` |
| `Time-15` | `stock[15].time` | `i32` |
| `Amount-15` | `stock[15].amount` | `i32` |
| `%-15` | `stock[15].pct` | `i32` |
| `Item-16` | `stock[16].item` | `i32` |
| `Max-16` | `stock[16].max` | `i32` |
| `Time-16` | `stock[16].time` | `i32` |
| `Amount-16` | `stock[16].amount` | `i32` |
| `%-16` | `stock[16].pct` | `i32` |
| `Item-17` | `stock[17].item` | `i32` |
| `Max-17` | `stock[17].max` | `i32` |
| `Time-17` | `stock[17].time` | `i32` |
| `Amount-17` | `stock[17].amount` | `i32` |
| `%-17` | `stock[17].pct` | `i32` |
| `Item-18` | `stock[18].item` | `i32` |
| `Max-18` | `stock[18].max` | `i32` |
| `Time-18` | `stock[18].time` | `i32` |
| `Amount-18` | `stock[18].amount` | `i32` |
| `%-18` | `stock[18].pct` | `i32` |
| `Item-19` | `stock[19].item` | `i32` |
| `Max-19` | `stock[19].max` | `i32` |
| `Time-19` | `stock[19].time` | `i32` |
| `Amount-19` | `stock[19].amount` | `i32` |
| `%-19` | `stock[19].pct` | `i32` |
| `In Game` | `in_game` | `bool` |
| `Assigned To` | `assigned_to` | `Option<String>` |

## Spells -> `Spell`

| Column | Field | Type |
|---|---|---|
| `Number` | `number` | `i32` |
| `Name` | `name` | `Option<String>` |
| `Short` | `short` | `Option<String>` |
| `ReqLevel` | `req_level` | `i32` |
| `EnergyCost` | `energy_cost` | `i32` |
| `ManaCost` | `mana_cost` | `i32` |
| `MinBase` | `min_base` | `i32` |
| `MaxBase` | `max_base` | `i32` |
| `Diff` | `diff` | `i32` |
| `TypeOfResists` | `type_of_resists` | `i32` |
| `Targets` | `targets` | `i32` |
| `Dur` | `dur` | `i32` |
| `AttType` | `att_type` | `i32` |
| `Magery` | `magery` | `i32` |
| `MageryLVL` | `magery_lvl` | `i32` |
| `Cap` | `cap` | `i32` |
| `MaxIncLVLs` | `max_inc_lvls` | `i32` |
| `MaxInc` | `max_inc` | `i32` |
| `MinIncLVLs` | `min_inc_lvls` | `i32` |
| `MinInc` | `min_inc` | `i32` |
| `DurIncLVLs` | `dur_inc_lvls` | `i32` |
| `DurInc` | `dur_inc` | `i32` |
| `Abil-0` | `abilities[0].ability` | `i32` |
| `AbilVal-0` | `abilities[0].value` | `i32` |
| `Abil-1` | `abilities[1].ability` | `i32` |
| `AbilVal-1` | `abilities[1].value` | `i32` |
| `Abil-2` | `abilities[2].ability` | `i32` |
| `AbilVal-2` | `abilities[2].value` | `i32` |
| `Abil-3` | `abilities[3].ability` | `i32` |
| `AbilVal-3` | `abilities[3].value` | `i32` |
| `Abil-4` | `abilities[4].ability` | `i32` |
| `AbilVal-4` | `abilities[4].value` | `i32` |
| `Abil-5` | `abilities[5].ability` | `i32` |
| `AbilVal-5` | `abilities[5].value` | `i32` |
| `Abil-6` | `abilities[6].ability` | `i32` |
| `AbilVal-6` | `abilities[6].value` | `i32` |
| `Abil-7` | `abilities[7].ability` | `i32` |
| `AbilVal-7` | `abilities[7].value` | `i32` |
| `Abil-8` | `abilities[8].ability` | `i32` |
| `AbilVal-8` | `abilities[8].value` | `i32` |
| `Abil-9` | `abilities[9].ability` | `i32` |
| `AbilVal-9` | `abilities[9].value` | `i32` |
| `Learnable` | `learnable` | `bool` |
| `Learned From` | `learned_from` | `Option<String>` |
| `Casted By` | `casted_by` | `Option<String>` |
| `Classes` | `classes` | `Option<String>` |

## TBInfo -> `TbInfo`

| Column | Field | Type |
|---|---|---|
| `Number` | `number` | `i32` |
| `LinkTo` | `link_to` | `i32` |
| `Action` | `action` | `Option<String>` |
| `Called From` | `called_from` | `String` |
