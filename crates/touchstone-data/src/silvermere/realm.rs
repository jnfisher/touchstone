//! The realm: every character in a live silvermere/Quicksilver database (card R-001).
//!
//! Only game fields are read, and each column is named. `accounts` (password hash, e-mail), the
//! suicide password, the login name kept in `characters.userid` and `character_profiles` are never
//! selected, so nothing private can reach a caller or an API response, whatever the caller does.
use std::collections::HashMap;

use rusqlite::Connection;

use super::error::Result;

/// A worn, carried or key slot: the item number and the uses it has left (`-1` is "no charges").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slotted {
    pub slot: i32,
    pub item: i32,
    pub uses: i32,
}

/// One spell cast on the character that is still running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastSpell {
    pub slot: i32,
    pub spell: i32,
    /// The value the cast rolled; an ability the spell gives without a fixed value uses it.
    pub value: i32,
    pub rounds: i32,
}

/// A character as the game stores it, game fields only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealmCharacter {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub title: String,
    /// Experience in full: the billions and the remainder joined.
    pub exp: i64,
    pub race: i32,
    pub class: i32,
    pub level: i32,
    /// STR INT WIL AGL HEA CHM, the order of MME's stat boxes (the game stores INT WIL STR HEA AGI CHA).
    pub base_stats: [i32; 6],
    /// The same order: current values, with the effects of gear and running spells.
    pub current_stats: [i32; 6],
    pub max_hp: i32,
    pub hp: i32,
    pub max_mana: i32,
    pub mana: i32,
    pub max_mr: i32,
    pub mr: i32,
    pub max_weight: i32,
    pub weight: i32,
    pub max_energy: i32,
    pub energy: i32,
    pub encumbrance_pct: i32,
    pub perception: i32,
    pub stealth: i32,
    pub martial: i32,
    pub thievery: i32,
    pub spellcast: i32,
    pub traps: i32,
    pub disarm: i32,
    pub picklocks: i32,
    pub tracking: i32,
    pub hp_bonus: i32,
    pub mana_bonus: i32,
    pub random_hp: i32,
    pub martial_base: i32,
    /// "Alter AV", "Alter Defence Value", "Alter Damage", "Alter Critical Hit Chance", "Alter Damage
    /// Resistance", "Alter DR by percent", "Alter Energy Usage", "Protective Shield", "Defence
    /// Modifier", "Alter healing rate", "Alter Mana Regeneration", "Alter Room Light" totals.
    pub av_bonus: i32,
    pub ac_bonus: i32,
    pub dmg_bonus: i32,
    pub crit_bonus: i32,
    pub dr_bonus: i32,
    pub dr_pct: i32,
    pub energy_use: i32,
    pub shield: i32,
    pub def_mod: i32,
    pub heal_rate: i32,
    pub mana_rate: i32,
    pub room_light: i32,
    pub evil: i32,
    pub lawful: i32,
    pub lives: i32,
    pub gang: String,
    pub map: i32,
    pub room: i32,
    /// Unix seconds, 0 for never.
    pub last_on: i64,
    pub death_date: i64,
    pub unspent_cp: i32,
    pub total_cp: i32,
    /// The wielded weapon's item number, 0 for none.
    pub weapon: i32,
    /// The inventory slot of the readied light, -1 for none.
    pub readied: i32,
    /// Runic, platinum, gold, silver, copper.
    pub coins: [i64; 5],
    pub worn: Vec<Slotted>,
    pub inventory: Vec<Slotted>,
    pub spellbook: Vec<i32>,
    /// (ability code, value) of the character's own ability slots (running spell effects land here).
    pub abilities: Vec<(i32, i32)>,
    pub spells_cast: Vec<CastSpell>,
}

/// Every character of one read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Realm {
    pub characters: Vec<RealmCharacter>,
}

impl Realm {
    pub fn get(&self, id: i64) -> Option<&RealmCharacter> {
        self.characters.iter().find(|c| c.id == id)
    }
}

fn group<T>(
    conn: &Connection,
    sql: &str,
    mut f: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<(i64, T)>,
) -> Result<HashMap<i64, Vec<T>>> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query([])?;
    let mut out: HashMap<i64, Vec<T>> = HashMap::new();
    while let Some(r) = rows.next()? {
        let (id, v) = f(r)?;
        out.entry(id).or_default().push(v);
    }
    Ok(out)
}

fn slotted(conn: &Connection, table: &str) -> Result<HashMap<i64, Vec<Slotted>>> {
    group(
        conn,
        &format!(
            "SELECT character_id, slot, item, uses FROM {table} WHERE item <> 0 \
             ORDER BY character_id, slot"
        ),
        |r| {
            Ok((
                r.get(0)?,
                Slotted {
                    slot: clamp(r.get(1)?),
                    item: clamp(r.get(2)?),
                    uses: clamp(r.get(3)?),
                },
            ))
        },
    )
}

/// A column this reader cannot fit in 32 bits is a corrupt row, not worth refusing the whole realm.
fn clamp(v: i64) -> i32 {
    i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX })
}

pub(super) fn load(conn: &Connection) -> Result<Realm> {
    let mut worn = slotted(conn, "character_worn")?;
    let mut inventory = slotted(conn, "character_inventory")?;
    let mut spellbook = group(
        conn,
        "SELECT character_id, slot, spell FROM character_spellbook WHERE spell <> 0 \
         ORDER BY character_id, slot",
        |r| Ok((r.get(0)?, clamp(r.get(2)?))),
    )?;
    let mut abilities = group(
        conn,
        "SELECT character_id, slot, ability, value FROM character_abilities WHERE ability <> 0 \
         ORDER BY character_id, slot",
        |r| Ok((r.get(0)?, (clamp(r.get(2)?), clamp(r.get(3)?)))),
    )?;
    let mut cast = group(
        conn,
        "SELECT character_id, slot, spell, value, rounds FROM character_spells_cast \
         WHERE spell <> 0 ORDER BY character_id, slot",
        |r| {
            Ok((
                r.get(0)?,
                CastSpell {
                    slot: clamp(r.get(1)?),
                    spell: clamp(r.get(2)?),
                    value: clamp(r.get(3)?),
                    rounds: clamp(r.get(4)?),
                },
            ))
        },
    )?;

    // Named columns only: see the module comment for what is deliberately absent.
    let mut stmt = conn.prepare(
        "SELECT id, firstname, lastname, title, exp, expbil, exprem, race, class, level, \
         basstr, basint, baswil, basagi, bashea, bascha, \
         str, intel, wil, agi, hea, cha, \
         maxhp, hp, maxmana, mana, maxmr, mr, maxwgt, wgt, maxenergy, energy, encumpct, \
         perception, stealth, martial, thievery, spellcast, traps, disarm, picklocks, tracking, \
         hpbonus, manabonus, randomhp, mabase, \
         avbonus, acbonus, dmgbonus, critbonus, drbonus, drpct, energyuse, shield, defmod, \
         healrate, manarate, roomlight, evil, lawful, lives, gang, map, room, lastdate, deathdate, \
         cp, totalcp, weapon, readied, runic, platinum, gold, silver, copper \
         FROM characters ORDER BY id",
    )?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        let i = |n: &str| -> rusqlite::Result<i32> { Ok(clamp(r.get::<_, i64>(n)?)) };
        let l = |n: &str| -> rusqlite::Result<i64> { r.get::<_, i64>(n) };
        let id: i64 = r.get("id")?;
        let bil = l("expbil")?;
        let exp = if bil > 0 {
            bil.saturating_mul(1_000_000_000)
                .saturating_add(l("exprem")?)
        } else {
            l("exprem")?.max(l("exp")?)
        };
        out.push(RealmCharacter {
            id,
            first_name: r.get("firstname")?,
            last_name: r.get("lastname")?,
            title: r.get("title")?,
            exp,
            race: i("race")?,
            class: i("class")?,
            level: i("level")?,
            base_stats: [
                i("basstr")?,
                i("basint")?,
                i("baswil")?,
                i("basagi")?,
                i("bashea")?,
                i("bascha")?,
            ],
            current_stats: [
                i("str")?,
                i("intel")?,
                i("wil")?,
                i("agi")?,
                i("hea")?,
                i("cha")?,
            ],
            max_hp: i("maxhp")?,
            hp: i("hp")?,
            max_mana: i("maxmana")?,
            mana: i("mana")?,
            max_mr: i("maxmr")?,
            mr: i("mr")?,
            max_weight: i("maxwgt")?,
            weight: i("wgt")?,
            max_energy: i("maxenergy")?,
            energy: i("energy")?,
            encumbrance_pct: i("encumpct")?,
            perception: i("perception")?,
            stealth: i("stealth")?,
            martial: i("martial")?,
            thievery: i("thievery")?,
            spellcast: i("spellcast")?,
            traps: i("traps")?,
            disarm: i("disarm")?,
            picklocks: i("picklocks")?,
            tracking: i("tracking")?,
            hp_bonus: i("hpbonus")?,
            mana_bonus: i("manabonus")?,
            random_hp: i("randomhp")?,
            martial_base: i("mabase")?,
            av_bonus: i("avbonus")?,
            ac_bonus: i("acbonus")?,
            dmg_bonus: i("dmgbonus")?,
            crit_bonus: i("critbonus")?,
            dr_bonus: i("drbonus")?,
            dr_pct: i("drpct")?,
            energy_use: i("energyuse")?,
            shield: i("shield")?,
            def_mod: i("defmod")?,
            heal_rate: i("healrate")?,
            mana_rate: i("manarate")?,
            room_light: i("roomlight")?,
            evil: i("evil")?,
            lawful: i("lawful")?,
            lives: i("lives")?,
            gang: r.get("gang")?,
            map: i("map")?,
            room: i("room")?,
            last_on: l("lastdate")?,
            death_date: l("deathdate")?,
            unspent_cp: i("cp")?,
            total_cp: i("totalcp")?,
            weapon: i("weapon")?,
            readied: i("readied")?,
            coins: [
                l("runic")?,
                l("platinum")?,
                l("gold")?,
                l("silver")?,
                l("copper")?,
            ],
            worn: worn.remove(&id).unwrap_or_default(),
            inventory: inventory.remove(&id).unwrap_or_default(),
            spellbook: spellbook.remove(&id).unwrap_or_default(),
            abilities: abilities.remove(&id).unwrap_or_default(),
            spells_cast: cast.remove(&id).unwrap_or_default(),
        });
    }
    Ok(Realm { characters: out })
}
