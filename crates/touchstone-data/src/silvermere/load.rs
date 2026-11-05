//! Table-by-table mapping from silvermere's schema into the model. Every choice that is not a
//! straight column copy is listed in `docs/data-gaps.md`.
use std::collections::HashMap;

use rusqlite::{Connection, Row};
use touchstone_model::{
    AbilitySlot, Class, DropSlot, Info, Item, Lair, MidSpell, Monster, MonsterAttack, Race, Room,
    Shop, ShopSlot, Spell, TbInfo, World, WorldParts,
};

use super::error::{Error, Result};

/// `(slot, a, b)` rows of a child table, grouped by the parent's key.
type Children = HashMap<i64, Vec<(i64, i64, i64)>>;

/// Typed column access that names the table and column when a value does not fit.
struct R<'a> {
    row: &'a Row<'a>,
    table: &'static str,
}

impl R<'_> {
    fn i(&self, col: &'static str) -> Result<i32> {
        to_i32(self.table, col, self.row.get::<_, i64>(col)?)
    }
    fn s(&self, col: &'static str) -> Result<String> {
        Ok(self.row.get(col)?)
    }
    fn b(&self, col: &'static str) -> Result<bool> {
        Ok(self.row.get::<_, i64>(col)? != 0)
    }
}

fn to_i32(table: &'static str, column: &'static str, value: i64) -> Result<i32> {
    i32::try_from(value).map_err(|_| Error::OutOfRange {
        table,
        column,
        value,
    })
}

/// Run `sql` and map each row.
fn query<T>(
    conn: &Connection,
    table: &'static str,
    sql: &str,
    mut f: impl FnMut(&R) -> Result<T>,
) -> Result<Vec<T>> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(f(&R { row, table })?);
    }
    Ok(out)
}

/// `SELECT key, slot, a, b FROM table`, grouped by key.
fn children(conn: &Connection, table: &str, key: &str, a: &str, b: &str) -> Result<Children> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {key}, slot, {a}, {b} FROM {table} ORDER BY {key}, slot"
    ))?;
    let mut rows = stmt.query([])?;
    let mut out: Children = HashMap::new();
    while let Some(r) = rows.next()? {
        out.entry(r.get(0)?)
            .or_default()
            .push((r.get(1)?, r.get(2)?, r.get(3)?));
    }
    Ok(out)
}

fn slot_index(table: &'static str, slot: i64, len: usize) -> Result<usize> {
    usize::try_from(slot)
        .ok()
        .filter(|&s| s < len)
        .ok_or(Error::OutOfRange {
            table,
            column: "slot",
            value: slot,
        })
}

fn abilities<const N: usize>(
    table: &'static str,
    rows: Option<&Vec<(i64, i64, i64)>>,
) -> Result<[AbilitySlot; N]> {
    let mut out = [AbilitySlot::default(); N];
    for &(slot, ability, value) in rows.into_iter().flatten() {
        out[slot_index(table, slot, N)?] = AbilitySlot {
            ability: to_i32(table, "ability", ability)?,
            value: to_i32(table, "value", value)?,
        };
    }
    Ok(out)
}

/// A single-number slot list such as `item_classes`.
fn numbers<const N: usize>(
    table: &'static str,
    rows: Option<&Vec<(i64, i64, i64)>>,
) -> Result<[i32; N]> {
    let mut out = [0; N];
    for &(slot, v, _) in rows.into_iter().flatten() {
        out[slot_index(table, slot, N)?] = to_i32(table, "value", v)?;
    }
    Ok(out)
}

pub(super) fn load(conn: &Connection) -> Result<World> {
    let messages = message_texts(conn)?;
    let rooms_raw = rooms(conn)?;
    let items = items(conn)?;
    let monsters = monsters(conn, &messages)?;
    let shops = shops(conn, &rooms_raw)?;
    let tbinfo = tbinfo(conn, &monsters, &rooms_raw)?;
    let parts = WorldParts {
        info: info(conn)?,
        items,
        monsters,
        spells: spells(conn)?,
        rooms: rooms_raw.into_iter().map(|r| r.room).collect(),
        shops,
        classes: classes(conn)?,
        races: races(conn)?,
        // Lairs are NMR's computed summary of the room/monster data; silvermere has no table.
        lairs: Vec::<Lair>::new(),
        tbinfo,
    };
    Ok(World::new(parts)?)
}

fn info(conn: &Connection) -> Result<Info> {
    // world_import is absent of rows in a seeded world; content_version exists only in
    // Quicksilver (migration 9001).
    let has_content_version = conn
        .prepare("SELECT 1 FROM pragma_table_info('world_import') WHERE name = 'content_version'")?
        .exists([])?;
    let sql = if has_content_version {
        "SELECT finished_at, content_version FROM world_import"
    } else {
        "SELECT finished_at, NULL FROM world_import"
    };
    let row: Option<(Option<String>, Option<String>)> = conn
        .query_row(sql, [], |r| Ok((r.get(0)?, r.get(1)?)))
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            e => Err(e),
        })?;
    let (finished, content_version) = match row {
        Some((None, _)) => return Err(Error::ImportIncomplete),
        Some((f, c)) => (f, c),
        None => (None, None),
    };
    // finished_at is RFC 3339 UTC: "2026-10-14T09:30:00Z".
    let (date, time) = match finished.as_deref().and_then(|f| f.split_once('T')) {
        Some((d, t)) => (d.to_string(), t.trim_end_matches('Z').to_string()),
        None => (String::new(), String::new()),
    };
    Ok(Info {
        nmr_version: String::new(),
        dat_file_version: content_version.unwrap_or_else(|| "silvermere".to_string()),
        date,
        time,
        custom: "silvermere".to_string(),
        legit: true,
        disable_kai: false,
        update_url: String::new(),
    })
}

fn items(conn: &Connection) -> Result<Vec<Item>> {
    let mut abil = children(conn, "item_abilities", "item", "ability", "value")?;
    let mut cls = children(conn, "item_classes", "item", "class", "0")?;
    let mut races = children(conn, "item_races", "item", "race", "0")?;
    let mut negs = children(conn, "item_negates", "item", "spell", "0")?;
    query(conn, "items", "SELECT * FROM items ORDER BY number", |r| {
        let number = r.i("number")?;
        let key = i64::from(number);
        Ok(Item {
            number,
            name: r.s("name")?,
            limit: r.i("limit_copies")?,
            encum: r.i("weight")?,
            item_type: r.i("item_type")?,
            // charges is -1 for "none"; MME's UseCount is 0 then.
            use_count: r.i("charges")?.max(0),
            price: r.i("cost")?,
            currency: r.i("currency")?,
            min: r.i("mindmg")?,
            max: r.i("maxdmg")?,
            armour_class: r.i("ac")?,
            damage_resist: r.i("dr")?,
            weapon_type: r.i("wpntype")?,
            armour_type: r.i("armtype")?,
            worn: r.i("worn")?,
            accy: r.i("accuracy")?,
            gettable: r.b("gettable")?,
            str_req: r.i("strreq")?,
            speed: r.i("speed")?,
            not_droppable: r.b("nodrop")?,
            destroy_on_death: r.b("destroy")?,
            retain_after_uses: r.b("retain")?,
            class_rest: numbers("item_classes", cls.remove(&key).as_ref())?,
            race_rest: numbers("item_races", races.remove(&key).as_ref())?,
            negate_spell: numbers("item_negates", negs.remove(&key).as_ref())?,
            abilities: abilities("item_abilities", abil.remove(&key).as_ref())?,
            in_game: true,
            obtained_from: None,
            references: None,
        })
    })
}

/// `messages.text1` (the line the target sees) by message number: MME's attack names read
/// "slashes you".
fn message_texts(conn: &Connection) -> Result<HashMap<i64, String>> {
    let mut stmt = conn.prepare("SELECT number, text1 FROM messages")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

fn monsters(conn: &Connection, messages: &HashMap<i64, String>) -> Result<Vec<Monster>> {
    let mut abil = children(conn, "monster_abilities", "monster", "ability", "value")?;
    let mut mid = HashMap::<i64, Vec<(i64, i64, i64, i64)>>::new();
    {
        let mut stmt =
            conn.prepare("SELECT monster, slot, spell, pct, level FROM monster_spells")?;
        let mut rows = stmt.query([])?;
        while let Some(r) = rows.next()? {
            mid.entry(r.get(0)?)
                .or_default()
                .push((r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?));
        }
    }
    let mut drops = HashMap::<i64, Vec<(i64, i64, i64)>>::new();
    {
        let mut stmt = conn.prepare("SELECT monster, slot, item, pct FROM monster_items")?;
        let mut rows = stmt.query([])?;
        while let Some(r) = rows.next()? {
            drops
                .entry(r.get(0)?)
                .or_default()
                .push((r.get(1)?, r.get(2)?, r.get(3)?));
        }
    }
    let mut attacks = HashMap::<i64, Vec<MonsterAttackRow>>::new();
    {
        let mut stmt = conn.prepare(
            "SELECT monster, slot, kind, accuracy, pct, min, max, hitmsg, energy, hitspell \
             FROM monster_attacks ORDER BY monster, slot",
        )?;
        let mut rows = stmt.query([])?;
        while let Some(r) = rows.next()? {
            let mut v = [0i64; 9];
            for (i, x) in v.iter_mut().enumerate() {
                *x = r.get(i + 1)?;
            }
            attacks.entry(r.get(0)?).or_default().push(v);
        }
    }
    query(
        conn,
        "monsters",
        "SELECT * FROM monsters ORDER BY number",
        |r| {
            let number = r.i("number")?;
            let key = i64::from(number);
            Ok(Monster {
                number,
                name: Some(r.s("name")?),
                weapon: r.i("weapon")?,
                armour_class: r.i("ac")?,
                damage_resist: r.i("dr")?,
                follow_pct: r.i("followpct")?,
                magic_res: r.i("magicres")?,
                bs_defense: r.i("bsdefense")?,
                exp: f64::from(r.i("exp")?),
                // silvermere awards exp * level; MME splits that into EXP and ExpMulti.
                exp_multi: f64::from(r.i("level")?),
                hp: r.i("maxhp")?,
                energy: r.i("energy")?,
                avg_dmg: 0.0,
                avg_dmg_phys: 0.0,
                avg_dmg_spell: 0.0,
                greet_txt: r.i("talktext")?,
                hp_regen: r.i("hpregen")?,
                charm_lvl: r.i("charmlvl")?,
                r#type: r.i("montype")?,
                undead: r.i("undead")?,
                align: r.i("alignment")?,
                regen_time: r.i("regenhrs")?,
                game_limit: r.i("maxalive")?,
                avg_lair_exp: 0.0,
                script_value: 0.0,
                r: r.i("cash_runic")?,
                p: r.i("cash_platinum")?,
                g: r.i("cash_gold")?,
                s: r.i("cash_silver")?,
                c: r.i("cash_copper")?,
                death_spell: r.i("deathspell")?,
                create_spell: r.i("createspell")?,
                attacks: monster_attacks(attacks.remove(&key), messages)?,
                mid_spells: {
                    let mut out = [MidSpell::default(); 5];
                    for (slot, spell, pct, lvl) in mid.remove(&key).into_iter().flatten() {
                        out[slot_index("monster_spells", slot, 5)?] = MidSpell {
                            spell: to_i32("monster_spells", "spell", spell)?,
                            pct: to_i32("monster_spells", "pct", pct)?,
                            lvl: to_i32("monster_spells", "level", lvl)?,
                        };
                    }
                    out
                },
                drops: {
                    let mut out = [DropSlot::default(); 10];
                    for (slot, item, pct) in drops.remove(&key).into_iter().flatten() {
                        out[slot_index("monster_items", slot, 10)?] = DropSlot {
                            item: to_i32("monster_items", "item", item)?,
                            pct: to_i32("monster_items", "pct", pct)?,
                        };
                    }
                    out
                },
                abilities: abilities("monster_abilities", abil.remove(&key).as_ref())?,
                in_game: true,
                summoned_by: None,
            })
        },
    )
}

/// slot, kind, accuracy, pct, min, max, hitmsg, energy, hitspell
type MonsterAttackRow = [i64; 9];

fn monster_attacks(
    rows: Option<Vec<MonsterAttackRow>>,
    messages: &HashMap<i64, String>,
) -> Result<[MonsterAttack; 5]> {
    const T: &str = "monster_attacks";
    // MME names an unused slot "None".
    let mut out = std::array::from_fn(|_| MonsterAttack {
        name: "None".to_string(),
        att_type: 0,
        acc: 0,
        pct: 0,
        true_pct: 0.0,
        min: 0,
        max: 0,
        energy: 0,
        hit_spell: 0,
    });
    let mut prev_pct = 0;
    for v in rows.into_iter().flatten() {
        let pct = to_i32(T, "pct", v[3])?;
        // pct is cumulative in both schemas; the true chance of this slot is the step.
        let true_pct = f64::from((pct - prev_pct).max(0));
        prev_pct = pct.max(prev_pct);
        out[slot_index(T, v[0], 5)?] = MonsterAttack {
            name: messages.get(&v[6]).cloned().unwrap_or_default(),
            att_type: to_i32(T, "kind", v[1])?,
            acc: to_i32(T, "accuracy", v[2])?,
            pct,
            true_pct,
            min: to_i32(T, "min", v[4])?,
            max: to_i32(T, "max", v[5])?,
            energy: to_i32(T, "energy", v[7])?,
            hit_spell: to_i32(T, "hitspell", v[8])?,
        };
    }
    Ok(out)
}

fn spells(conn: &Connection) -> Result<Vec<Spell>> {
    let mut abil = children(conn, "spell_abilities", "spell", "ability", "value")?;
    query(
        conn,
        "spells",
        "SELECT * FROM spells ORDER BY number",
        |r| {
            let number = r.i("number")?;
            Ok(Spell {
                number,
                name: Some(r.s("name")?),
                short: Some(r.s("shortnm")?),
                req_level: r.i("level")?,
                energy_cost: r.i("energy")?,
                mana_cost: r.i("mana")?,
                min_base: r.i("minval")?,
                max_base: r.i("maxval")?,
                diff: r.i("diff")?,
                type_of_resists: r.i("resstat")?,
                targets: r.i("target")?,
                dur: r.i("duration")?,
                att_type: r.i("element")?,
                magery: r.i("magery")?,
                magery_lvl: r.i("magelvl")?,
                cap: r.i("levelcap")?,
                max_inc_lvls: r.i("maxlvls")?,
                max_inc: r.i("maxinc")?,
                min_inc_lvls: r.i("minlvls")?,
                min_inc: r.i("mininc")?,
                dur_inc_lvls: r.i("durlvls")?,
                dur_inc: r.i("durinc")?,
                abilities: abilities("spell_abilities", abil.remove(&i64::from(number)).as_ref())?,
                learnable: false,
                learned_from: None,
                casted_by: None,
                classes: None,
            })
        },
    )
}

fn classes(conn: &Connection) -> Result<Vec<Class>> {
    let mut abil = children(conn, "class_abilities", "class", "ability", "value")?;
    query(
        conn,
        "classes",
        "SELECT * FROM classes ORDER BY number",
        |r| {
            let number = r.i("number")?;
            Ok(Class {
                number,
                name: r.s("name")?,
                min_hits: r.i("basehp")?,
                max_hits: r.i("hplevel")?,
                exp_table: r.i("exp")?,
                magery_type: r.i("magery")?,
                magery_lvl: r.i("magelvl")?,
                weapon_type: r.i("weapon")?,
                armour_type: r.i("armour")?,
                combat_lvl: r.i("combat")?,
                abilities: abilities("class_abilities", abil.remove(&i64::from(number)).as_ref())?,
            })
        },
    )
}

fn races(conn: &Connection) -> Result<Vec<Race>> {
    let mut abil = children(conn, "race_abilities", "race", "ability", "value")?;
    query(conn, "races", "SELECT * FROM races ORDER BY number", |r| {
        let number = r.i("number")?;
        Ok(Race {
            number,
            name: r.s("name")?,
            m_int: r.i("minint")?,
            m_wil: r.i("minwil")?,
            m_str: r.i("minstr")?,
            m_hea: r.i("minhea")?,
            m_agl: r.i("minagi")?,
            m_chm: r.i("mincha")?,
            x_int: r.i("maxint")?,
            x_wil: r.i("maxwil")?,
            x_str: r.i("maxstr")?,
            x_hea: r.i("maxhea")?,
            x_agl: r.i("maxagi")?,
            x_chm: r.i("maxcha")?,
            hp_per_lvl: r.i("hpbonus")?,
            exp_table: r.i("exp")?,
            base_cp: r.i("basecp")?,
            abilities: abilities("race_abilities", abil.remove(&i64::from(number)).as_ref())?,
        })
    })
}

/// A room plus the silvermere-side facts other tables are derived from.
struct RoomRow {
    room: Room,
    shopnum: i32,
    cmdtext: i32,
}

/// `EXIT_*` codes with their own text in MME's exit column.
const EXIT_DOOR: i64 = 7;
const EXIT_MAPCHANGE: i64 = 8;

fn exit_text(map: i32, target: i64, exit_type: i64, par1: i64) -> String {
    if target == 0 {
        return "0".to_string();
    }
    let dest_map = if exit_type == EXIT_MAPCHANGE {
        par1
    } else {
        i64::from(map)
    };
    let suffix = match exit_type {
        0 | EXIT_MAPCHANGE => "",
        EXIT_DOOR => " (Door)",
        // The other kinds carry parameters MME prints in several shapes; only the kind is kept.
        1 => " (Spell)",
        2 => " (Key)",
        3 => " (Item)",
        4 => " (Toll)",
        6 => " (Hidden)",
        _ => " (Other)",
    };
    format!("{dest_map}/{target}{suffix}")
}

fn rooms(conn: &Connection) -> Result<Vec<RoomRow>> {
    let mut exits = HashMap::<i64, Vec<(i64, i64, i64, i64)>>::new();
    {
        let mut stmt =
            conn.prepare("SELECT room_id, slot, target, exit_type, par1 FROM room_exits")?;
        let mut rows = stmt.query([])?;
        while let Some(r) = rows.next()? {
            exits
                .entry(r.get(0)?)
                .or_default()
                .push((r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?));
        }
    }
    let mut placed = children(conn, "room_placed_items", "room_id", "item", "0")?;
    query(
        conn,
        "rooms",
        "SELECT * FROM rooms ORDER BY map, room",
        |r| {
            let id: i64 = r.row.get("id")?;
            let map = r.i("map")?;
            let mut dirs: [String; 10] = std::array::from_fn(|_| "0".to_string());
            for (slot, target, kind, par1) in exits.remove(&id).into_iter().flatten() {
                dirs[slot_index("room_exits", slot, 10)?] = exit_text(map, target, kind, par1);
            }
            let [n, s, e, w, ne, nw, se, sw, u, d] = dirs;
            // MME lists placed items as "939," (each followed by a comma).
            let placed_items = placed.remove(&id).map(|rows| {
                rows.iter()
                    .filter(|x| x.1 != 0)
                    .map(|x| format!("{},", x.1))
                    .collect::<String>()
            });
            let shopnum = r.i("shopnum")?;
            let cmdtext = r.i("cmdtext")?;
            Ok(RoomRow {
                room: Room {
                    map_number: map,
                    room_number: r.i("room")?,
                    name: Some(r.s("name")?),
                    light: r.i("light")?,
                    shop: shopnum,
                    npc: r.i("specmon")?,
                    cmd: cmdtext,
                    spell: r.i("roomspell")?,
                    lair: None,
                    delay: r.i("regendelay")?,
                    placed: placed_items.filter(|p| !p.is_empty()),
                    n,
                    s,
                    e,
                    w,
                    ne,
                    nw,
                    se,
                    sw,
                    u,
                    d,
                },
                shopnum,
                cmdtext,
            })
        },
    )
}

fn shops(conn: &Connection, rooms: &[RoomRow]) -> Result<Vec<Shop>> {
    let mut stock = HashMap::<i64, Vec<[i64; 6]>>::new();
    {
        let mut stmt = conn.prepare(
            "SELECT shop, slot, item, maxqty, regentime, regennum, regenpct FROM shop_items",
        )?;
        let mut rows = stmt.query([])?;
        while let Some(r) = rows.next()? {
            let mut v = [0i64; 6];
            for (i, x) in v.iter_mut().enumerate() {
                *x = r.get(i + 1)?;
            }
            stock.entry(r.get(0)?).or_default().push(v);
        }
    }
    query(conn, "shops", "SELECT * FROM shops ORDER BY number", |r| {
        let number = r.i("number")?;
        let mut slots = [ShopSlot::default(); 20];
        for v in stock.remove(&i64::from(number)).into_iter().flatten() {
            const T: &str = "shop_items";
            slots[slot_index(T, v[0], 20)?] = ShopSlot {
                item: to_i32(T, "item", v[1])?,
                max: to_i32(T, "maxqty", v[2])?,
                time: to_i32(T, "regentime", v[3])?,
                amount: to_i32(T, "regennum", v[4])?,
                pct: to_i32(T, "regenpct", v[5])?,
            };
        }
        let homes: Vec<String> = rooms
            .iter()
            .filter(|x| x.shopnum == number)
            .map(|x| format!("Room {}/{}", x.room.map_number, x.room.room_number))
            .collect();
        Ok(Shop {
            number,
            name: r.s("name")?,
            shop_type: r.i("shop_type")?,
            min_lvl: r.i("minlevel")?,
            max_lvl: r.i("maxlevel")?,
            markup_pct: r.i("markup")?,
            class_rest: r.i("trainclass")?,
            stock: slots,
            in_game: !homes.is_empty(),
            assigned_to: if homes.is_empty() {
                None
            } else {
                Some(homes.join(", "))
            },
        })
    })
}

/// One row per text block number, from its first part. `called_from` lists what points at it.
fn tbinfo(conn: &Connection, monsters: &[Monster], rooms: &[RoomRow]) -> Result<Vec<TbInfo>> {
    let mut refs: HashMap<i32, Vec<String>> = HashMap::new();
    for m in monsters {
        refs.entry(m.greet_txt)
            .or_default()
            .push(format!("Monster #{}", m.number));
    }
    for r in rooms {
        refs.entry(r.cmdtext)
            .or_default()
            .push(format!("Room {}/{}", r.room.map_number, r.room.room_number));
    }
    let blocks = query(
        conn,
        "text_blocks",
        "SELECT number, link FROM text_blocks WHERE part = \
         (SELECT MIN(part) FROM text_blocks t WHERE t.number = text_blocks.number) \
         ORDER BY number",
        |r| Ok((r.i("number")?, r.i("link")?)),
    )?;
    for &(number, link) in &blocks {
        if link != 0 {
            refs.entry(link)
                .or_default()
                .push(format!("Textblock #{number}"));
        }
    }
    Ok(blocks
        .into_iter()
        .map(|(number, link)| TbInfo {
            number,
            link_to: link,
            action: None,
            called_from: refs.remove(&number).unwrap_or_default().join(", "),
        })
        .collect())
}
