//! Loads a `World` from the SQLite file `tools/mdb2sqlite.py` makes from an MME `.mdb` (T-011).
//! Column names map to model fields as listed in `docs/model.md`.
use std::path::Path;

use rusqlite::{Connection, OpenFlags, Row};
use touchstone_model::{
    AbilitySlot, Class, DropSlot, Info, Item, Lair, MidSpell, ModelError, Monster, MonsterAttack,
    Race, Room, Shop, ShopSlot, Spell, TbInfo, World, WorldParts,
};

/// Loading a database failed.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("table {table}: {source}")]
    Sqlite {
        table: &'static str,
        source: rusqlite::Error,
    },
    #[error("cannot open {path}: {source}")]
    Open {
        path: String,
        source: rusqlite::Error,
    },
    #[error("table Info must have exactly one row, found {0}")]
    InfoRows(usize),
    #[error(transparent)]
    Model(#[from] ModelError),
}

type Res<T> = Result<T, rusqlite::Error>;

/// Read the database at `path` (opened read-only) into a [`World`].
pub fn load(path: impl AsRef<Path>) -> Result<World, LoadError> {
    let path = path.as_ref();
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|source| LoadError::Open {
        path: path.display().to_string(),
        source,
    })?;
    let mut infos = read(&conn, "Info", info)?;
    if infos.len() != 1 {
        return Err(LoadError::InfoRows(infos.len()));
    }
    Ok(World::new(WorldParts {
        info: infos.remove(0),
        items: read(&conn, "Items", item)?,
        monsters: read(&conn, "Monsters", monster)?,
        spells: read(&conn, "Spells", spell)?,
        rooms: read(&conn, "Rooms", room)?,
        shops: read(&conn, "Shops", shop)?,
        classes: read(&conn, "Classes", class)?,
        races: read(&conn, "Races", race)?,
        lairs: read(&conn, "Lairs", lair)?,
        tbinfo: read(&conn, "TBInfo", tbinfo)?,
    })?)
}

fn read<T>(
    conn: &Connection,
    table: &'static str,
    build: impl Fn(&Row<'_>) -> Res<T>,
) -> Result<Vec<T>, LoadError> {
    let wrap = |source| LoadError::Sqlite { table, source };
    // rowid order is the .mdb's record order, which the fixtures also use.
    let mut stmt = conn
        .prepare(&format!("SELECT * FROM \"{table}\" ORDER BY rowid"))
        .map_err(wrap)?;
    let rows = stmt.query_map([], |r| build(r)).map_err(wrap)?;
    rows.collect::<Res<Vec<T>>>().map_err(wrap)
}

fn int(r: &Row<'_>, col: &str) -> Res<i32> {
    r.get(col)
}
fn flag(r: &Row<'_>, col: &str) -> Res<bool> {
    Ok(r.get::<_, i64>(col)? != 0)
}
fn real(r: &Row<'_>, col: &str) -> Res<f64> {
    r.get(col)
}
fn text(r: &Row<'_>, col: &str) -> Res<String> {
    r.get(col)
}
fn opt_text(r: &Row<'_>, col: &str) -> Res<Option<String>> {
    r.get(col)
}

/// Build a fixed array from numbered columns `0..N`.
fn series<T, const N: usize>(mut f: impl FnMut(usize) -> Res<T>) -> Res<[T; N]> {
    let mut v = Vec::with_capacity(N);
    for n in 0..N {
        v.push(f(n)?);
    }
    Ok(v.try_into().unwrap_or_else(|_| unreachable!()))
}

fn abilities<const N: usize>(r: &Row<'_>) -> Res<[AbilitySlot; N]> {
    series(|n| {
        Ok(AbilitySlot {
            ability: int(r, &format!("Abil-{n}"))?,
            value: int(r, &format!("AbilVal-{n}"))?,
        })
    })
}

fn ints<const N: usize>(r: &Row<'_>, prefix: &str) -> Res<[i32; N]> {
    series(|n| int(r, &format!("{prefix}-{n}")))
}

fn class(r: &Row<'_>) -> Res<Class> {
    Ok(Class {
        number: int(r, "Number")?,
        name: text(r, "Name")?,
        min_hits: int(r, "MinHits")?,
        max_hits: int(r, "MaxHits")?,
        exp_table: int(r, "ExpTable")?,
        magery_type: int(r, "MageryType")?,
        magery_lvl: int(r, "MageryLVL")?,
        weapon_type: int(r, "WeaponType")?,
        armour_type: int(r, "ArmourType")?,
        combat_lvl: int(r, "CombatLVL")?,
        abilities: abilities(r)?,
    })
}

fn info(r: &Row<'_>) -> Res<Info> {
    Ok(Info {
        nmr_version: text(r, "NMR Version")?,
        dat_file_version: text(r, "Dat File Version")?,
        content_version: None,
        date: text(r, "Date")?,
        time: text(r, "Time")?,
        custom: text(r, "Custom")?,
        legit: flag(r, "Legit")?,
        disable_kai: flag(r, "DisableKai")?,
        update_url: text(r, "UpdateURL")?,
    })
}

fn item(r: &Row<'_>) -> Res<Item> {
    Ok(Item {
        number: int(r, "Number")?,
        name: text(r, "Name")?,
        limit: int(r, "Limit")?,
        encum: int(r, "Encum")?,
        item_type: int(r, "ItemType")?,
        use_count: int(r, "UseCount")?,
        price: int(r, "Price")?,
        currency: int(r, "Currency")?,
        min: int(r, "Min")?,
        max: int(r, "Max")?,
        armour_class: int(r, "ArmourClass")?,
        damage_resist: int(r, "DamageResist")?,
        weapon_type: int(r, "WeaponType")?,
        armour_type: int(r, "ArmourType")?,
        worn: int(r, "Worn")?,
        accy: int(r, "Accy")?,
        gettable: flag(r, "Gettable")?,
        str_req: int(r, "StrReq")?,
        speed: int(r, "Speed")?,
        not_droppable: flag(r, "Not Droppable")?,
        destroy_on_death: flag(r, "Destroy On Death")?,
        retain_after_uses: flag(r, "Retain After Uses")?,
        class_rest: ints(r, "ClassRest")?,
        race_rest: ints(r, "RaceRest")?,
        negate_spell: ints(r, "NegateSpell")?,
        abilities: abilities(r)?,
        in_game: flag(r, "In Game")?,
        obtained_from: opt_text(r, "Obtained From")?,
        references: opt_text(r, "References")?,
    })
}

fn lair(r: &Row<'_>) -> Res<Lair> {
    Ok(Lair {
        group_index: text(r, "GroupIndex")?,
        mob_list: text(r, "MobList")?,
        mobs: int(r, "Mobs")?,
        total_lairs: int(r, "TotalLairs")?,
        avg_delay: int(r, "AvgDelay")?,
        avg_walk: real(r, "AvgWalk")?,
        avg_exp: real(r, "AvgExp")?,
        avg_dmg: real(r, "AvgDmg")?,
        avg_dmg_phys: real(r, "AvgDmgPhys")?,
        avg_dmg_spell: real(r, "AvgDmgSpell")?,
        avg_hp: real(r, "AvgHP")?,
        avg_ac: int(r, "AvgAC")?,
        avg_dr: int(r, "AvgDR")?,
        avg_mr: int(r, "AvgMR")?,
        avg_dodge: int(r, "AvgDodge")?,
    })
}

fn monster(r: &Row<'_>) -> Res<Monster> {
    Ok(Monster {
        number: int(r, "Number")?,
        name: opt_text(r, "Name")?,
        weapon: int(r, "Weapon")?,
        armour_class: int(r, "ArmourClass")?,
        damage_resist: int(r, "DamageResist")?,
        follow_pct: int(r, "Follow%")?,
        magic_res: int(r, "MagicRes")?,
        bs_defense: int(r, "BSDefense")?,
        exp: real(r, "EXP")?,
        exp_multi: real(r, "ExpMulti")?,
        hp: int(r, "HP")?,
        energy: int(r, "Energy")?,
        avg_dmg: real(r, "AvgDmg")?,
        avg_dmg_phys: real(r, "AvgDmgPhys")?,
        avg_dmg_spell: real(r, "AvgDmgSpell")?,
        greet_txt: int(r, "GreetTXT")?,
        hp_regen: int(r, "HPRegen")?,
        charm_lvl: int(r, "CharmLVL")?,
        r#type: int(r, "Type")?,
        undead: int(r, "Undead")?,
        align: int(r, "Align")?,
        regen_time: int(r, "RegenTime")?,
        game_limit: int(r, "GameLimit")?,
        avg_lair_exp: real(r, "AvgLairExp")?,
        script_value: real(r, "ScriptValue")?,
        r: int(r, "R")?,
        p: int(r, "P")?,
        g: int(r, "G")?,
        s: int(r, "S")?,
        c: int(r, "C")?,
        death_spell: int(r, "DeathSpell")?,
        create_spell: int(r, "CreateSpell")?,
        attacks: series(|n| {
            Ok(MonsterAttack {
                name: text(r, &format!("AttName-{n}"))?,
                att_type: int(r, &format!("AttType-{n}"))?,
                acc: int(r, &format!("AttAcc-{n}"))?,
                pct: int(r, &format!("Att%-{n}"))?,
                true_pct: real(r, &format!("AttTrue%-{n}"))?,
                min: int(r, &format!("AttMin-{n}"))?,
                max: int(r, &format!("AttMax-{n}"))?,
                energy: int(r, &format!("AttEnergy-{n}"))?,
                hit_spell: int(r, &format!("AttHitSpell-{n}"))?,
            })
        })?,
        mid_spells: series(|n| {
            Ok(MidSpell {
                spell: int(r, &format!("MidSpell-{n}"))?,
                pct: int(r, &format!("MidSpell%-{n}"))?,
                lvl: int(r, &format!("MidSpellLVL-{n}"))?,
            })
        })?,
        drops: series(|n| {
            Ok(DropSlot {
                item: int(r, &format!("DropItem-{n}"))?,
                pct: int(r, &format!("DropItem%-{n}"))?,
            })
        })?,
        abilities: abilities(r)?,
        in_game: flag(r, "In Game")?,
        summoned_by: opt_text(r, "Summoned By")?,
    })
}

fn race(r: &Row<'_>) -> Res<Race> {
    Ok(Race {
        number: int(r, "Number")?,
        name: text(r, "Name")?,
        m_int: int(r, "mINT")?,
        m_wil: int(r, "mWIL")?,
        m_str: int(r, "mSTR")?,
        m_hea: int(r, "mHEA")?,
        m_agl: int(r, "mAGL")?,
        m_chm: int(r, "mCHM")?,
        x_int: int(r, "xINT")?,
        x_wil: int(r, "xWIL")?,
        x_str: int(r, "xSTR")?,
        x_hea: int(r, "xHEA")?,
        x_agl: int(r, "xAGL")?,
        x_chm: int(r, "xCHM")?,
        hp_per_lvl: int(r, "HPPerLVL")?,
        exp_table: int(r, "ExpTable")?,
        base_cp: int(r, "BaseCP")?,
        abilities: abilities(r)?,
    })
}

fn room(r: &Row<'_>) -> Res<Room> {
    Ok(Room {
        map_number: int(r, "Map Number")?,
        room_number: int(r, "Room Number")?,
        name: opt_text(r, "Name")?,
        light: int(r, "Light")?,
        shop: int(r, "Shop")?,
        npc: int(r, "NPC")?,
        cmd: int(r, "CMD")?,
        spell: int(r, "Spell")?,
        lair: opt_text(r, "Lair")?,
        delay: int(r, "Delay")?,
        placed: opt_text(r, "Placed")?,
        n: text(r, "N")?,
        s: text(r, "S")?,
        e: text(r, "E")?,
        w: text(r, "W")?,
        ne: text(r, "NE")?,
        nw: text(r, "NW")?,
        se: text(r, "SE")?,
        sw: text(r, "SW")?,
        u: text(r, "U")?,
        d: text(r, "D")?,
    })
}

fn shop(r: &Row<'_>) -> Res<Shop> {
    Ok(Shop {
        number: int(r, "Number")?,
        name: text(r, "Name")?,
        shop_type: int(r, "ShopType")?,
        min_lvl: int(r, "MinLVL")?,
        max_lvl: int(r, "MaxLVL")?,
        markup_pct: int(r, "Markup%")?,
        class_rest: int(r, "ClassRest")?,
        stock: series(|n| {
            Ok(ShopSlot {
                item: int(r, &format!("Item-{n}"))?,
                max: int(r, &format!("Max-{n}"))?,
                time: int(r, &format!("Time-{n}"))?,
                amount: int(r, &format!("Amount-{n}"))?,
                pct: int(r, &format!("%-{n}"))?,
            })
        })?,
        in_game: flag(r, "In Game")?,
        assigned_to: opt_text(r, "Assigned To")?,
    })
}

fn spell(r: &Row<'_>) -> Res<Spell> {
    Ok(Spell {
        number: int(r, "Number")?,
        name: opt_text(r, "Name")?,
        short: opt_text(r, "Short")?,
        req_level: int(r, "ReqLevel")?,
        energy_cost: int(r, "EnergyCost")?,
        mana_cost: int(r, "ManaCost")?,
        min_base: int(r, "MinBase")?,
        max_base: int(r, "MaxBase")?,
        diff: int(r, "Diff")?,
        type_of_resists: int(r, "TypeOfResists")?,
        targets: int(r, "Targets")?,
        dur: int(r, "Dur")?,
        att_type: int(r, "AttType")?,
        magery: int(r, "Magery")?,
        magery_lvl: int(r, "MageryLVL")?,
        cap: int(r, "Cap")?,
        max_inc_lvls: int(r, "MaxIncLVLs")?,
        max_inc: int(r, "MaxInc")?,
        min_inc_lvls: int(r, "MinIncLVLs")?,
        min_inc: int(r, "MinInc")?,
        dur_inc_lvls: int(r, "DurIncLVLs")?,
        dur_inc: int(r, "DurInc")?,
        abilities: abilities(r)?,
        learnable: flag(r, "Learnable")?,
        learned_from: opt_text(r, "Learned From")?,
        casted_by: opt_text(r, "Casted By")?,
        classes: opt_text(r, "Classes")?,
    })
}

fn tbinfo(r: &Row<'_>) -> Res<TbInfo> {
    Ok(TbInfo {
        number: int(r, "Number")?,
        link_to: int(r, "LinkTo")?,
        action: opt_text(r, "Action")?,
        called_from: text(r, "Called From")?,
    })
}
