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
    let monsters = monsters(conn, &messages, &spell_names(conn)?)?;
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
            // Both keep -1 for "no charges" (checked against the stock database).
            use_count: r.i("charges")?,
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

/// `messages.text0` (the line the attacker's target is shown, a format string such as
/// "The %s slashes you for %d damage!") by message number; `attack_name` reduces it to MME's
/// "slashes you".
fn message_texts(conn: &Connection) -> Result<HashMap<i64, String>> {
    let mut stmt = conn.prepare("SELECT number, text0 FROM messages")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// Spell names by number, for spell attacks (MME names those after the spell).
fn spell_names(conn: &Connection) -> Result<HashMap<i64, String>> {
    let mut stmt = conn.prepare("SELECT number, name FROM spells")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// MME's attack name: "None" for kind 0, the spell's name for a spell attack, the hit message
/// without its "The %s " lead and " for %d damage!" tail for a weapon attack ("Physical" when it
/// has no message), "Unknown" for any other kind.
fn attack_name(
    kind: i64,
    accuracy: i64,
    hitmsg: i64,
    messages: &HashMap<i64, String>,
    spells: &HashMap<i64, String>,
) -> String {
    match kind {
        0 => "None".to_string(),
        2 => spells.get(&accuracy).cloned().unwrap_or_default(),
        1 | 3 => {
            let text = messages.get(&hitmsg).map_or("", String::as_str);
            // Everything up to the monster's own name is dropped ("The %s ", "A corpse held by %s ").
            let text = text.split_once("%s ").map_or(text, |(_, rest)| rest);
            let damage_tail = [" for %d damage!", " for %d damage.", " for %s damage!"]
                .into_iter()
                .find_map(|t| text.strip_suffix(t));
            let name = match damage_tail {
                Some(head) => head,
                // A message without a damage figure is named by its verb.
                None => text.split(' ').next().unwrap_or(""),
            };
            if name.is_empty() {
                "Physical".to_string()
            } else if name.chars().count() > 49 {
                // The one long message in the stock data is cut to 46 characters plus "...".
                format!("{}...", name.chars().take(46).collect::<String>())
            } else {
                name.to_string()
            }
        }
        _ => "Unknown".to_string(),
    }
}

/// An empty string is a NULL in the stock `.mdb` for the optional text columns.
fn nonempty(s: String) -> Option<String> {
    (!s.is_empty()).then_some(s)
}

fn monsters(
    conn: &Connection,
    messages: &HashMap<i64, String>,
    spells: &HashMap<i64, String>,
) -> Result<Vec<Monster>> {
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
                name: nonempty(r.s("name")?),
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
                // MME's Type is the follower role (solo, leader, follower, stationary), not montype.
                r#type: r.i("grouprole")?,
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
                attacks: monster_attacks(attacks.remove(&key), messages, spells)?,
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
    spells: &HashMap<i64, String>,
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
            name: attack_name(v[1], v[2], v[6], messages, spells),
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
                name: nonempty(r.s("name")?),
                short: nonempty(r.s("shortnm")?),
                req_level: r.i("level")?,
                energy_cost: r.i("energy")?,
                mana_cost: r.i("mana")?,
                min_base: r.i("minval")?,
                max_base: r.i("maxval")?,
                diff: r.i("diff")?,
                // MME's TypeOfResists is silvermere's resmode (0 magic, 1 stat, 2 other), not resstat.
                type_of_resists: r.i("resmode")?,
                targets: r.i("target")?,
                dur: r.i("duration")?,
                att_type: r.i("element")?,
                magery: r.i("magery")?,
                magery_lvl: r.i("magelvl")?,
                // A byte in MME; silvermere keeps the 16-bit field, whose high byte is padding.
                cap: r.i("levelcap")? & 0xFF,
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

/// `EXIT_*` codes (silvermere's `exit_type`) whose text MME builds from the four parameters.
const EXIT_KEY: i64 = 2;
const EXIT_ITEM: i64 = 3;
const EXIT_TOLL: i64 = 4;
const EXIT_HIDDEN: i64 = 6;
const EXIT_DOOR: i64 = 7;
const EXIT_MAPCHANGE: i64 = 8;
const EXIT_TRAP: i64 = 9;
const EXIT_DOOR_ALT: i64 = 11;
const EXIT_CLASS: i64 = 13;
const EXIT_RACE: i64 = 14;
const EXIT_LEVEL: i64 = 15;
const EXIT_TICKET: i64 = 17;
const EXIT_CAST: i64 = 22;
const EXIT_ABILITY: i64 = 23;
const EXIT_SPELLTRAP: i64 = 24;

/// `par1..par4` of a `room_exits` row.
type ExitPars = [i64; 4];

/// The lock part of a door or key exit: a negative `par2` is a picklock/strength requirement of
/// `1 - par2`, a positive one means any attempt works, zero means none is listed.
fn lock_text(par2: i64, strength: bool) -> String {
    let unit = if strength {
        "picklocks/strength"
    } else {
        "picklocks"
    };
    match par2 {
        0 => String::new(),
        p if p < 0 => format!("{} {unit}", 1 - p),
        _ => format!("any {unit}"),
    }
}

/// The text after `map/room`. The kinds MME describes with words from a text block ("Text:"),
/// an action's item and direction ("Action [...]") or a timer stay a generic `(Other)`; the rest
/// is rebuilt from the parameters and checked against the stock `.mdb` by the parity example.
fn exit_suffix(exit_type: i64, p: ExitPars) -> String {
    match exit_type {
        0 | 1 | 5 | EXIT_MAPCHANGE | 19 => String::new(),
        EXIT_KEY => {
            let lock = lock_text(p[2], false);
            if lock.is_empty() {
                format!(" (Key: {})", p[0])
            } else {
                format!(" (Key: {} [or {lock}])", p[0])
            }
        }
        EXIT_ITEM => format!(" (Item: {})", p[0]),
        EXIT_TOLL => format!(" (Toll: {})", p[0]),
        // par1 is a bit set: 1 passable, 2 searchable, higher bits the actions needed (par2
        // is their count, negative when the order does not matter).
        EXIT_HIDDEN => match p[0] {
            1 => " (Hidden/Passable)".to_string(),
            2 => " (Hidden/Searchable)".to_string(),
            _ if p[1] != 0 => format!(
                " (Hidden/Needs {} Actions, {} order)",
                p[1].abs(),
                if p[1] < 0 { "any" } else { "specific" }
            ),
            _ => " (Hidden/Unknown)".to_string(),
        },
        EXIT_DOOR | EXIT_DOOR_ALT => {
            let lock = lock_text(p[1], true);
            match (p[3], lock.is_empty()) {
                (0, true) => " (Door)".to_string(),
                (0, false) => format!(" (Door [{lock}])"),
                (k, true) => format!(" (Key: {k})"),
                (k, false) => format!(" (Key: {k} [or {lock}])"),
            }
        }
        EXIT_TRAP => format!(" (Trap, {} damage)", p[0]),
        EXIT_CLASS => format!(" (Class: {} OK, {} NO)", p[0], p[1]),
        EXIT_RACE => format!(" (Race: {} OK, {} NO)", p[0], p[1]),
        EXIT_LEVEL => format!(" (Level: {} to {})", p[0], p[1]),
        EXIT_TICKET => format!(" (Ticket/Item: {})", p[0]),
        EXIT_CAST => format!(" (Cast: pre-{}, post-{})", p[0], p[1]),
        EXIT_ABILITY => format!(" (Ability: {} w/value {} to {})", p[0], p[1], p[2]),
        EXIT_SPELLTRAP => format!(" (Spell Trap: {})", p[0]),
        _ => " (Other)".to_string(),
    }
}

fn exit_text(map: i32, target: i64, exit_type: i64, pars: ExitPars) -> String {
    // A hidden exit with no parameters at all is an unused slot in MME.
    if target == 0 || (exit_type == EXIT_HIDDEN && pars == [0; 4]) {
        return "0".to_string();
    }
    let dest_map = if exit_type == EXIT_MAPCHANGE {
        pars[0]
    } else {
        i64::from(map)
    };
    format!("{dest_map}/{target}{}", exit_suffix(exit_type, pars))
}

fn rooms(conn: &Connection) -> Result<Vec<RoomRow>> {
    let mut exits = HashMap::<i64, Vec<(i64, i64, i64, ExitPars)>>::new();
    {
        let mut stmt = conn.prepare(
            "SELECT room_id, slot, target, exit_type, par1, par2, par3, par4 FROM room_exits",
        )?;
        let mut rows = stmt.query([])?;
        while let Some(r) = rows.next()? {
            exits.entry(r.get(0)?).or_default().push((
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                [r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?],
            ));
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
            for (slot, target, kind, pars) in exits.remove(&id).into_iter().flatten() {
                dirs[slot_index("room_exits", slot, 10)?] = exit_text(map, target, kind, pars);
            }
            let [n, s, e, w, ne, nw, se, sw, u, d] = dirs;
            // MME lists placed items as "939," (each followed by a comma).
            let placed_items = placed.remove(&id).map(|rows| {
                rows.iter()
                    .filter(|x| x.1 != 0)
                    .map(|x| format!("{},", x.1))
                    .collect::<String>()
            });
            // MME only records the shop on the shop room itself (room_type 1); the vault, locker
            // and inn side rooms carry the number in silvermere but not in the stock .mdb.
            let shopnum = if r.i("room_type")? == 1 {
                r.i("shopnum")?
            } else {
                0
            };
            let cmdtext = r.i("cmdtext")?;
            Ok(RoomRow {
                room: Room {
                    map_number: map,
                    room_number: r.i("room")?,
                    name: nonempty(r.s("name")?),
                    light: r.i("light")?,
                    shop: shopnum,
                    npc: r.i("specmon")?,
                    cmd: cmdtext,
                    spell: r.i("roomspell")?,
                    lair: None,
                    // An unset delay (0) reads as the default 5 in MME.
                    delay: match r.i("regendelay")? {
                        0 => 5,
                        d => d,
                    },
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
        let mut homes: Vec<String> = rooms
            .iter()
            .filter(|x| x.shopnum == number)
            .map(|x| format!("Room {}/{}", x.room.map_number, x.room.room_number))
            .collect();
        // MME lists the rooms in text order ("Room 10/271" before "Room 3/542").
        homes.sort();
        Ok(Shop {
            number,
            name: r.s("name")?,
            shop_type: r.i("shop_type")?,
            min_lvl: r.i("minlevel")?,
            max_lvl: r.i("maxlevel")?,
            markup_pct: r.i("markup")?,
            // A byte in MME; the high byte of the 16-bit field is padding.
            class_rest: r.i("trainclass")? & 0xFF,
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
