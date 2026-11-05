// Tests for the silvermere/Quicksilver adapter (T-012). All records here are invented.
use std::path::{Path, PathBuf};

use rusqlite::{Connection, types::Value};
use tempfile::TempDir;
use touchstone_data::silvermere::{self, Error};

// The vendored silvermere migrations build the real schema.
const MIGRATIONS: [(i64, &str); 2] = [
    (
        1,
        include_str!("../../vendor/silvermere-migrations/0001_schema.sql"),
    ),
    (
        2,
        include_str!("../../vendor/silvermere-migrations/0002_world_import.sql"),
    ),
];

/// A WAL-mode database with the schema applied and `versions` recorded the way sqlx records
/// them.
fn new_db(versions: &[i64]) -> (TempDir, PathBuf, Connection) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("world.db");
    let conn = Connection::open(&path).unwrap();
    conn.pragma_update(None, "journal_mode", "WAL").unwrap();
    for (_, sql) in MIGRATIONS {
        conn.execute_batch(sql).unwrap();
    }
    conn.execute_batch(
        "CREATE TABLE _sqlx_migrations (version BIGINT PRIMARY KEY, description TEXT NOT NULL, \
         installed_on TEXT NOT NULL DEFAULT '', success BOOLEAN NOT NULL, \
         checksum BLOB NOT NULL DEFAULT x'', execution_time BIGINT NOT NULL DEFAULT 0)",
    )
    .unwrap();
    for v in versions {
        conn.execute(
            "INSERT INTO _sqlx_migrations (version, description, success) VALUES (?, 'm', 1)",
            [v],
        )
        .unwrap();
    }
    (dir, path, conn)
}

/// Insert a row, giving every column not named a zero or empty string (the schema has no
/// defaults).
fn insert(conn: &Connection, table: &str, set: &[(&str, Value)]) {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT name, type FROM pragma_table_info('{table}')"
        ))
        .unwrap();
    let cols: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut names = Vec::new();
    let mut vals = Vec::new();
    for (name, ty) in cols {
        if name == "id" && !set.iter().any(|(n, _)| *n == "id") {
            continue; // autoincrement
        }
        let v = set
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| {
                if ty == "TEXT" {
                    Value::Text(String::new())
                } else {
                    Value::Integer(0)
                }
            });
        names.push(name);
        vals.push(v);
    }
    let marks = vec!["?"; names.len()].join(",");
    conn.execute(
        &format!("INSERT INTO {table} ({}) VALUES ({marks})", names.join(",")),
        rusqlite::params_from_iter(vals),
    )
    .unwrap();
}

fn i(v: i64) -> Value {
    Value::Integer(v)
}
fn t(v: &str) -> Value {
    Value::Text(v.to_string())
}

fn seed(conn: &Connection) {
    // Item 7: a charged weapon limited to two class numbers and one race, with two abilities.
    insert(
        conn,
        "items",
        &[
            ("number", i(7)),
            ("name", t("test blade")),
            ("limit_copies", i(3)),
            ("weight", i(12)),
            ("item_type", i(1)),
            ("charges", i(-1)),
            ("cost", i(250)),
            ("mindmg", i(2)),
            ("maxdmg", i(9)),
            ("ac", i(4)),
            ("dr", i(1)),
            ("wpntype", i(3)),
            ("worn", i(13)),
            ("accuracy", i(5)),
            ("gettable", i(1)),
            ("strreq", i(30)),
            ("speed", i(1200)),
            ("nodrop", i(1)),
            ("currency", i(2)),
            ("cash_copper", i(99)),
        ],
    );
    insert(
        conn,
        "items",
        &[
            ("number", i(8)),
            ("name", t("test charm")),
            ("charges", i(5)),
        ],
    );
    for (slot, ability, value) in [(0, 17, 4), (19, 31, -2)] {
        insert(
            conn,
            "item_abilities",
            &[
                ("item", i(7)),
                ("slot", i(slot)),
                ("ability", i(ability)),
                ("value", i(value)),
            ],
        );
    }
    insert(
        conn,
        "item_classes",
        &[("item", i(7)), ("slot", i(0)), ("class", i(2))],
    );
    insert(
        conn,
        "item_classes",
        &[("item", i(7)), ("slot", i(1)), ("class", i(5))],
    );
    insert(
        conn,
        "item_races",
        &[("item", i(7)), ("slot", i(3)), ("race", i(4))],
    );
    insert(
        conn,
        "item_negates",
        &[("item", i(7)), ("slot", i(9)), ("spell", i(77))],
    );

    // Monster 21 with one weapon attack (message 3 = "pokes you"), a second attack, a
    // mid-fight spell and a drop.
    insert(
        conn,
        "messages",
        &[
            ("number", i(3)),
            ("text0", t("The %s pokes you for %d damage!")),
            ("text1", t("pokes you")),
            ("text2", t("pokes")),
        ],
    );
    insert(
        conn,
        "monsters",
        &[
            ("number", i(21)),
            ("name", t("test gremlin")),
            ("montype", i(9)),
            ("grouprole", i(2)),
            ("level", i(3)),
            ("weapon", i(7)),
            ("dr", i(2)),
            ("ac", i(11)),
            ("followpct", i(25)),
            ("magicres", i(40)),
            ("exp", i(22)),
            ("maxhp", i(31)),
            ("energy", i(1000)),
            ("hpregen", i(2)),
            ("maxalive", i(4)),
            ("alignment", i(1)),
            ("regenhrs", i(6)),
            ("cash_silver", i(10)),
            ("cash_copper", i(20)),
            ("talktext", i(31)),
            ("charmlvl", i(5)),
            ("deathspell", i(8)),
        ],
    );
    for (slot, kind, acc, pct, min, max, hitmsg) in
        [(0, 1, 20, 40, 2, 11, 3), (2, 1, 25, 100, 1, 5, 0)]
    {
        insert(
            conn,
            "monster_attacks",
            &[
                ("monster", i(21)),
                ("slot", i(slot)),
                ("kind", i(kind)),
                ("accuracy", i(acc)),
                ("pct", i(pct)),
                ("min", i(min)),
                ("max", i(max)),
                ("hitmsg", i(hitmsg)),
                ("energy", i(900)),
                ("hitspell", i(6)),
            ],
        );
    }
    insert(
        conn,
        "monster_spells",
        &[
            ("monster", i(21)),
            ("slot", i(1)),
            ("spell", i(77)),
            ("pct", i(30)),
            ("level", i(4)),
        ],
    );
    insert(
        conn,
        "monster_items",
        &[
            ("monster", i(21)),
            ("slot", i(4)),
            ("item", i(8)),
            ("uses", i(1)),
            ("pct", i(15)),
        ],
    );
    insert(
        conn,
        "monster_abilities",
        &[
            ("monster", i(21)),
            ("slot", i(0)),
            ("ability", i(50)),
            ("value", i(3)),
        ],
    );

    insert(
        conn,
        "spells",
        &[
            ("number", i(77)),
            ("name", t("test bolt")),
            ("shortnm", t("tbol")),
            ("level", i(3)),
            ("energy", i(1000)),
            ("mana", i(5)),
            ("minval", i(4)),
            ("maxval", i(12)),
            ("diff", i(15)),
            ("resstat", i(1)),
            ("resmode", i(0)),
            ("target", i(8)),
            ("duration", i(2)),
            ("element", i(4)),
            ("magery", i(1)),
            ("magelvl", i(1)),
            // The high byte is padding (8224 = 0x2020 reads as 32 in MME).
            ("levelcap", i(0x2006)),
            ("maxinc", i(1)),
            ("maxlvls", i(2)),
        ],
    );
    insert(
        conn,
        "spell_abilities",
        &[
            ("spell", i(77)),
            ("slot", i(2)),
            ("ability", i(9)),
            ("value", i(8224)),
        ],
    );

    insert(
        conn,
        "classes",
        &[
            ("number", i(2)),
            ("name", t("Tester")),
            ("basehp", i(6)),
            ("hplevel", i(4)),
            ("exp", i(30)),
            ("magery", i(1)),
            ("magelvl", i(2)),
            ("weapon", i(8)),
            ("armour", i(9)),
            ("combat", i(6)),
        ],
    );
    insert(
        conn,
        "class_abilities",
        &[
            ("class", i(2)),
            ("slot", i(1)),
            ("ability", i(51)),
            ("value", i(10)),
        ],
    );
    insert(
        conn,
        "races",
        &[
            ("number", i(4)),
            ("name", t("Gnomeish")),
            ("minint", i(30)),
            ("maxint", i(90)),
            ("minagi", i(31)),
            ("maxagi", i(91)),
            ("hpbonus", i(1)),
            ("basecp", i(100)),
            ("exp", i(20)),
        ],
    );
    insert(
        conn,
        "race_abilities",
        &[
            ("race", i(4)),
            ("slot", i(0)),
            ("ability", i(36)),
            ("value", i(10)),
        ],
    );

    insert(
        conn,
        "shops",
        &[
            ("number", i(5)),
            ("name", t("Test Store")),
            ("shop_type", i(10)),
            ("minlevel", i(1)),
            ("maxlevel", i(5)),
            ("markup", i(120)),
            ("trainclass", i(0x7100 + 2)),
        ],
    );
    insert(
        conn,
        "shops",
        &[("number", i(6)), ("name", t("Unplaced Store"))],
    );
    insert(
        conn,
        "shop_items",
        &[
            ("shop", i(5)),
            ("slot", i(19)),
            ("item", i(8)),
            ("maxqty", i(50)),
            ("regentime", i(120)),
            ("regennum", i(10)),
            ("regenpct", i(100)),
        ],
    );

    // Room 1/10: north to 1/11 (plain), up through a door to 1/12; room 1/11 maps to map 9.
    insert(
        conn,
        "rooms",
        &[
            ("id", i(1)),
            ("map", i(1)),
            ("room", i(10)),
            ("name", t("Test Hall")),
            ("light", i(-1)),
            ("room_type", i(1)),
            ("shopnum", i(5)),
            ("specmon", i(21)),
            ("cmdtext", i(40)),
            ("roomspell", i(77)),
            ("regendelay", i(5)),
        ],
    );
    insert(
        conn,
        "rooms",
        &[
            ("id", i(2)),
            ("map", i(1)),
            ("room", i(11)),
            ("name", t("Test Closet")),
            // Not a shop room, so MME records no shop; an unset delay reads as 5.
            ("shopnum", i(5)),
            ("regendelay", i(0)),
        ],
    );
    for (slot, target, kind, par1) in [(0, 11, 0, 0), (8, 12, 7, 2)] {
        insert(
            conn,
            "room_exits",
            &[
                ("room_id", i(1)),
                ("slot", i(slot)),
                ("direction", i(slot)),
                ("target", i(target)),
                ("exit_type", i(kind)),
                ("par1", i(par1)),
            ],
        );
    }
    insert(
        conn,
        "room_exits",
        &[
            ("room_id", i(2)),
            ("slot", i(1)),
            ("direction", i(1)),
            ("target", i(3)),
            ("exit_type", i(8)),
            ("par1", i(9)),
        ],
    );
    insert(
        conn,
        "room_placed_items",
        &[("room_id", i(1)), ("slot", i(0)), ("item", i(7))],
    );
    insert(
        conn,
        "room_placed_items",
        &[("room_id", i(1)), ("slot", i(1)), ("item", i(8))],
    );

    insert(
        conn,
        "text_blocks",
        &[
            ("part", i(0)),
            ("number", i(31)),
            ("link", i(32)),
            ("text", t("hello")),
        ],
    );
    insert(
        conn,
        "text_blocks",
        &[
            ("part", i(1)),
            ("number", i(31)),
            ("link", i(99)),
            ("text", t("more")),
        ],
    );
    insert(
        conn,
        "text_blocks",
        &[("part", i(0)), ("number", i(32)), ("text", t("bye"))],
    );
}

fn load(path: &Path) -> touchstone_model::World {
    silvermere::open(path).unwrap().load_world().unwrap()
}

#[test]
fn maps_items() {
    let (_d, path, conn) = new_db(&[1, 2]);
    seed(&conn);
    let w = load(&path);
    assert_eq!(w.items.len(), 2);
    let it = w.get_item(7).unwrap();
    assert_eq!(it.name, "test blade");
    assert_eq!(
        (it.limit, it.encum, it.item_type, it.price, it.currency),
        (3, 12, 1, 250, 2)
    );
    assert_eq!(
        (it.min, it.max, it.armour_class, it.damage_resist),
        (2, 9, 4, 1)
    );
    assert_eq!(
        (it.weapon_type, it.worn, it.accy, it.str_req, it.speed),
        (3, 13, 5, 30, 1200)
    );
    assert!(it.gettable && it.not_droppable && !it.destroy_on_death && !it.retain_after_uses);
    // Expected value from the stock-parity run (T-015): MME keeps -1 for "no charges" too.
    assert_eq!(it.use_count, -1);
    assert_eq!(w.get_item(8).unwrap().use_count, 5);
    assert_eq!(it.class_rest, [2, 5, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(it.race_rest[3], 4);
    assert_eq!(it.negate_spell[9], 77);
    assert_eq!((it.abilities[0].ability, it.abilities[0].value), (17, 4));
    assert_eq!((it.abilities[19].ability, it.abilities[19].value), (31, -2));
    assert_eq!(it.abilities[5].ability, 0);
    assert!(it.in_game && it.obtained_from.is_none());
}

#[test]
fn maps_monsters() {
    let (_d, path, conn) = new_db(&[1, 2]);
    seed(&conn);
    let w = load(&path);
    let m = w.get_monster(21).unwrap();
    assert_eq!(m.name.as_deref(), Some("test gremlin"));
    assert_eq!(
        (
            m.weapon,
            m.armour_class,
            m.damage_resist,
            m.follow_pct,
            m.magic_res
        ),
        (7, 11, 2, 25, 40)
    );
    assert_eq!((m.exp, m.exp_multi, m.hp, m.energy), (22.0, 3.0, 31, 1000));
    assert_eq!(
        (m.r#type, m.align, m.regen_time, m.game_limit, m.hp_regen),
        (2, 1, 6, 4, 2)
    );
    assert_eq!((m.r, m.p, m.g, m.s, m.c), (0, 0, 0, 10, 20));
    assert_eq!(
        (m.greet_txt, m.charm_lvl, m.death_spell, m.create_spell),
        (31, 5, 8, 0)
    );
    let a0 = &m.attacks[0];
    assert_eq!(a0.name, "pokes you");
    assert_eq!(
        (
            a0.att_type,
            a0.acc,
            a0.pct,
            a0.min,
            a0.max,
            a0.energy,
            a0.hit_spell
        ),
        (1, 20, 40, 2, 11, 900, 6)
    );
    assert_eq!(a0.true_pct, 40.0);
    // Slot 1 is unused; slot 2 follows with a cumulative 100, so its own chance is 60.
    assert_eq!(m.attacks[1].name, "None");
    assert_eq!(m.attacks[2].true_pct, 60.0);
    // A weapon attack whose message is empty is "Physical" in MME.
    assert_eq!(m.attacks[2].name, "Physical");
    assert_eq!(m.mid_spells[1].spell, 77);
    assert_eq!((m.mid_spells[1].pct, m.mid_spells[1].lvl), (30, 4));
    assert_eq!((m.drops[4].item, m.drops[4].pct), (8, 15));
    assert_eq!(m.abilities[0].ability, 50);
}

#[test]
fn maps_spells_classes_races() {
    let (_d, path, conn) = new_db(&[1, 2]);
    seed(&conn);
    let w = load(&path);
    let s = w.get_spell(77).unwrap();
    assert_eq!(
        (s.name.as_deref(), s.short.as_deref()),
        (Some("test bolt"), Some("tbol"))
    );
    assert_eq!(
        (
            s.req_level,
            s.energy_cost,
            s.mana_cost,
            s.min_base,
            s.max_base
        ),
        (3, 1000, 5, 4, 12)
    );
    assert_eq!(
        (s.diff, s.type_of_resists, s.targets, s.dur, s.att_type),
        (15, 0, 8, 2, 4)
    );
    assert_eq!(
        (s.magery, s.magery_lvl, s.cap, s.max_inc, s.max_inc_lvls),
        (1, 1, 6, 1, 2)
    );
    assert_eq!(s.abilities[2].value, 8224);
    assert!(!s.learnable && s.learned_from.is_none());

    let c = w.get_class(2).unwrap();
    assert_eq!(
        (
            c.min_hits,
            c.max_hits,
            c.exp_table,
            c.magery_type,
            c.magery_lvl
        ),
        (6, 4, 30, 1, 2)
    );
    assert_eq!((c.weapon_type, c.armour_type, c.combat_lvl), (8, 9, 6));
    assert_eq!((c.abilities[1].ability, c.abilities[1].value), (51, 10));

    let r = w.get_race(4).unwrap();
    assert_eq!((r.m_int, r.x_int, r.m_agl, r.x_agl), (30, 90, 31, 91));
    assert_eq!((r.hp_per_lvl, r.base_cp, r.exp_table), (1, 100, 20));
    assert_eq!(r.abilities[0].ability, 36);
}

#[test]
fn maps_rooms_shops_and_text() {
    let (_d, path, conn) = new_db(&[1, 2]);
    seed(&conn);
    let w = load(&path);
    let room = w.get_room(1, 10).unwrap();
    assert_eq!(room.name.as_deref(), Some("Test Hall"));
    assert_eq!(
        (
            room.light, room.shop, room.npc, room.cmd, room.spell, room.delay
        ),
        (-1, 5, 21, 40, 77, 5)
    );
    assert_eq!(room.n, "1/11");
    assert_eq!(room.u, "1/12 (Door)");
    assert_eq!((room.s.as_str(), room.d.as_str()), ("0", "0"));
    assert_eq!(room.placed.as_deref(), Some("7,8,"));
    assert!(room.lair.is_none());
    // A map-change exit takes its map from par1.
    assert_eq!(w.get_room(1, 11).unwrap().s, "9/3");
    assert_eq!(w.get_room(1, 11).unwrap().placed, None);
    assert_eq!(
        (
            w.get_room(1, 11).unwrap().shop,
            w.get_room(1, 11).unwrap().delay
        ),
        (0, 5)
    );

    let shop = w.get_shop(5).unwrap();
    assert_eq!(
        (
            shop.shop_type,
            shop.min_lvl,
            shop.max_lvl,
            shop.markup_pct,
            shop.class_rest
        ),
        (10, 1, 5, 120, 2)
    );
    let slot = shop.stock[19];
    assert_eq!(
        (slot.item, slot.max, slot.time, slot.amount, slot.pct),
        (8, 50, 120, 10, 100)
    );
    assert_eq!(shop.stock[0].item, 0);
    assert!(shop.in_game);
    assert_eq!(shop.assigned_to.as_deref(), Some("Room 1/10"));
    let unplaced = w.get_shop(6).unwrap();
    assert!(!unplaced.in_game && unplaced.assigned_to.is_none());

    let tb = w.get_tbinfo(31).unwrap();
    // The link comes from the first part; monster 21 and room 1/10 both read block 31.
    assert_eq!(tb.link_to, 32);
    assert_eq!(tb.called_from, "Monster #21");
    assert_eq!(w.get_tbinfo(32).unwrap().called_from, "Textblock #31");
    assert_eq!(w.tbinfo.len(), 2);
    assert!(w.lairs.is_empty());
}

#[test]
fn reads_do_not_block_on_an_open_writer() {
    let (_d, path, writer) = new_db(&[1, 2]);
    seed(&writer);
    let db = silvermere::open(&path).unwrap();
    assert_eq!(db.load_world().unwrap().items.len(), 2);

    writer.execute_batch("BEGIN IMMEDIATE").unwrap();
    insert(
        &writer,
        "items",
        &[("number", i(9)), ("name", t("uncommitted"))],
    );
    // The writer holds the write lock mid-transaction; the reader sees the last commit.
    let during = db.load_world().unwrap();
    assert_eq!(during.items.len(), 2);
    assert!(during.get_item(9).is_none());

    writer.execute_batch("COMMIT").unwrap();
    assert!(db.load_world().unwrap().get_item(9).is_some());
}

#[test]
fn data_version_changes_on_commit_only() {
    let (_d, path, writer) = new_db(&[1, 2]);
    seed(&writer);
    let db = silvermere::open(&path).unwrap();
    let v0 = db.data_version().unwrap();
    db.load_world().unwrap();
    assert_eq!(db.data_version().unwrap(), v0, "reading must not change it");

    writer.execute_batch("BEGIN IMMEDIATE").unwrap();
    insert(&writer, "items", &[("number", i(9)), ("name", t("later"))]);
    assert_eq!(
        db.data_version().unwrap(),
        v0,
        "uncommitted write is invisible"
    );
    writer.execute_batch("COMMIT").unwrap();
    assert_ne!(db.data_version().unwrap(), v0);
}

#[test]
fn rejects_unknown_migration_version() {
    let (_d, path, conn) = new_db(&[1, 2, 3]);
    drop(conn);
    let err = silvermere::open(&path).unwrap_err();
    let Error::UnsupportedMigrations { found, supported } = &err else {
        panic!("wrong error: {err}");
    };
    assert_eq!(found, &[1, 2, 3]);
    assert_eq!(supported, silvermere::SUPPORTED_MIGRATIONS);
    let msg = err.to_string();
    assert!(
        msg.contains("[1, 2, 3]") && msg.contains("[1, 2, 9001]"),
        "{msg}"
    );
}

#[test]
fn rejects_missing_base_failed_and_foreign_databases() {
    let (_d, path, _c) = new_db(&[1]);
    assert!(matches!(
        silvermere::open(&path),
        Err(Error::UnsupportedMigrations { .. })
    ));

    let (_d, path, conn) = new_db(&[1, 2]);
    conn.execute(
        "UPDATE _sqlx_migrations SET success = 0 WHERE version = 2",
        [],
    )
    .unwrap();
    assert!(matches!(
        silvermere::open(&path),
        Err(Error::MigrationFailed(2))
    ));

    let dir = TempDir::new().unwrap();
    let other = dir.path().join("other.db");
    Connection::open(&other)
        .unwrap()
        .execute_batch("CREATE TABLE x (a)")
        .unwrap();
    assert!(matches!(silvermere::open(&other), Err(Error::NoMigrations)));
    assert!(silvermere::open(dir.path().join("absent.db")).is_err());
}

#[test]
fn accepts_quicksilver_9001_and_reads_content_version() {
    let (_d, path, conn) = new_db(&[1, 2, 9001]);
    conn.execute_batch("ALTER TABLE world_import ADD COLUMN content_version TEXT")
        .unwrap();
    insert(
        &conn,
        "world_import",
        &[
            ("id", i(1)),
            ("started_at", t("2026-10-14T09:00:00Z")),
            ("finished_at", t("2026-10-14T09:30:15Z")),
            ("content_version", t("1.20.0-test")),
        ],
    );
    let w = load(&path);
    assert_eq!(w.info.dat_file_version, "1.20.0-test");
    assert_eq!(
        (w.info.date.as_str(), w.info.time.as_str()),
        ("2026-10-14", "09:30:15")
    );
}

#[test]
fn refuses_a_half_written_import() {
    let (_d, path, conn) = new_db(&[1, 2]);
    insert(
        &conn,
        "world_import",
        &[
            ("id", i(1)),
            ("started_at", t("2026-10-14T09:00:00Z")),
            ("finished_at", Value::Null),
        ],
    );
    let db = silvermere::open(&path).unwrap();
    assert!(matches!(db.load_world(), Err(Error::ImportIncomplete)));
}

#[test]
fn opens_paths_with_uri_special_characters() {
    let dir = TempDir::new().unwrap();
    let sub = dir.path().join("a b#c?d%e é");
    std::fs::create_dir(&sub).unwrap();
    let path = sub.join("world.db");
    let conn = Connection::open(&path).unwrap();
    conn.pragma_update(None, "journal_mode", "WAL").unwrap();
    conn.execute_batch(MIGRATIONS[0].1).unwrap();
    conn.execute_batch(MIGRATIONS[1].1).unwrap();
    conn.execute_batch(
        "CREATE TABLE _sqlx_migrations (version BIGINT PRIMARY KEY, description TEXT, success BOOLEAN);
         INSERT INTO _sqlx_migrations VALUES (1,'a',1),(2,'b',1);",
    )
    .unwrap();
    assert!(load(&path).items.is_empty());
}

#[test]
fn an_empty_world_loads() {
    let (_d, path, _c) = new_db(&[1, 2]);
    let w = load(&path);
    assert!(w.items.is_empty() && w.rooms.is_empty());
    assert_eq!(w.info.dat_file_version, "silvermere");
}

#[test]
fn rebuilds_exit_and_attack_text() {
    let (_d, path, conn) = new_db(&[1, 2]);
    seed(&conn);
    // Expected strings are the stock .mdb's, from the T-015 parity run.
    for (slot, target, kind, pars) in [
        (1, 20, 2, [593, 2, -80, 1]),
        (2, 21, 7, [2, -300, 0, 1124]),
        (3, 22, 6, [112, -3, 0, 0]),
        (4, 23, 9, [30, 0, 0, 0]),
        (5, 24, 6, [0, 0, 0, 0]),
        (6, 25, 10, [752, 0, 753, 1]),
    ] {
        insert(
            &conn,
            "room_exits",
            &[
                ("room_id", i(1)),
                ("slot", i(slot)),
                ("direction", i(slot)),
                ("target", i(target)),
                ("exit_type", i(kind)),
                ("par1", i(pars[0])),
                ("par2", i(pars[1])),
                ("par3", i(pars[2])),
                ("par4", i(pars[3])),
            ],
        );
    }
    // A spell attack is named after its spell; a long hit message is cut.
    insert(
        &conn,
        "monster_attacks",
        &[
            ("monster", i(21)),
            ("slot", i(3)),
            ("kind", i(2)),
            ("accuracy", i(77)),
            ("pct", i(100)),
        ],
    );
    let w = load(&path);
    let r = w.get_room(1, 10).unwrap();
    assert_eq!(r.s, "1/20 (Key: 593 [or 81 picklocks])");
    assert_eq!(r.e, "1/21 (Key: 1124 [or 301 picklocks/strength])");
    assert_eq!(r.w, "1/22 (Hidden/Needs 3 Actions, any order)");
    assert_eq!(r.ne, "1/23 (Trap, 30 damage)");
    assert_eq!(r.nw, "0");
    assert_eq!(r.se, "1/25 (Other)");
    assert_eq!(w.get_monster(21).unwrap().attacks[3].name, "test bolt");
}
