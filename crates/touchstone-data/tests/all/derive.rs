// Tests for the NMR-derived fields (T-014). All records here are invented; the expected text
// formats are the stock .mdb's, learned by the T-015 parity run (docs/data-parity.md).
use rusqlite::Connection;

use super::silvermere::{i, insert, load, new_db, t};

fn monster(conn: &Connection, number: i64, montype: i64, genindex: i64, regenhrs: i64) {
    insert(
        conn,
        "monsters",
        &[
            ("number", i(number)),
            ("name", t("beast")),
            ("montype", i(montype)),
            ("genindex", i(genindex)),
            ("regenhrs", i(regenhrs)),
            ("level", i(1)),
            ("exp", i(100 * number)),
            ("maxhp", i(10 * number)),
            ("ac", i(number)),
        ],
    );
}

fn room(conn: &Connection, id: i64, map: i64, room: i64, set: &[(&str, i64)]) {
    let mut v = vec![("id", i(id)), ("map", i(map)), ("room", i(room))];
    v.extend(set.iter().map(|(k, x)| (*k, i(*x))));
    insert(conn, "rooms", &v);
}

/// Two lair rooms of one group, a plain room with a range, a room with a specific monster, a
/// shop room, and the items and spells that point at them.
fn seed(conn: &Connection) {
    // Type 6, generation indexes 1, 1, 2, 2, and monster 3 has a regeneration time.
    for (n, idx, regen) in [(1, 1, 0), (2, 1, 0), (3, 2, 5), (4, 2, 0)] {
        monster(conn, n, 6, idx, regen);
    }
    monster(conn, 5, 7, 1, 0);
    let lair = [
        ("room_type", 3),
        ("mongroup", 6),
        ("minlevel", 1),
        ("maxlevel", 2),
    ];
    let mut a = lair.to_vec();
    a.extend([("maxmonsters", 2), ("regendelay", 4)]);
    room(conn, 1, 1, 10, &a);
    let mut b = lair.to_vec();
    b.extend([("maxmonsters", 2), ("regendelay", 5)]);
    room(conn, 2, 1, 11, &b);
    room(
        conn,
        3,
        1,
        12,
        &[("mongroup", 6), ("minlevel", 2), ("maxlevel", 2)],
    );
    room(conn, 4, 1, 13, &[("specmon", 1)]);
    room(conn, 5, 1, 14, &[("room_type", 1), ("shopnum", 3)]);
    // The occupant spell of room 1/10.
    conn.execute("UPDATE rooms SET roomspell = 51 WHERE id = 1", [])
        .unwrap();
    insert(conn, "shops", &[("number", i(3)), ("name", t("store"))]);
    insert(
        conn,
        "shop_items",
        &[
            ("shop", i(3)),
            ("slot", i(0)),
            ("item", i(7)),
            ("maxqty", i(5)),
            ("regentime", i(2)),
            ("regennum", i(1)),
            ("regenpct", i(100)),
        ],
    );
    insert(conn, "items", &[("number", i(7)), ("name", t("scroll"))]);
    insert(
        conn,
        "item_abilities",
        &[
            ("item", i(7)),
            ("slot", i(0)),
            ("ability", i(42)),
            ("value", i(50)),
        ],
    );
    insert(
        conn,
        "room_placed_items",
        &[("room_id", i(1)), ("slot", i(0)), ("item", i(7))],
    );
    insert(
        conn,
        "monster_items",
        &[
            ("monster", i(4)),
            ("slot", i(0)),
            ("item", i(7)),
            ("uses", i(-1)),
            ("pct", i(20)),
        ],
    );
    insert(
        conn,
        "monster_spells",
        &[
            ("monster", i(4)),
            ("slot", i(0)),
            ("spell", i(51)),
            ("pct", i(100)),
            ("level", i(1)),
        ],
    );
    insert(
        conn,
        "spells",
        &[("number", i(50)), ("name", t("learn me"))],
    );
    insert(conn, "spells", &[("number", i(51)), ("name", t("cast me"))]);
}

#[test]
fn lair_text_lists_the_whole_pool_in_index_order() {
    let (_d, path, conn) = new_db(&[1, 2]);
    seed(&conn);
    let w = load(&path);
    let r = w.get_room(1, 10).unwrap();
    // Text form: stock Rooms.Lair, "(Max n): mobs,[group-min-max-n]"; the unique (3) is listed.
    assert_eq!(r.lair.as_deref(), Some("(Max 2): 1,2,3,4,[6-1-2-2]"));
    assert_eq!(w.get_room(1, 11).unwrap().lair, r.lair);
    // Not a lair room: no lair text.
    assert_eq!(w.get_room(1, 12).unwrap().lair, None);
}

#[test]
fn lairs_table_leaves_out_uniques_and_averages_the_group() {
    let (_d, path, conn) = new_db(&[1, 2]);
    seed(&conn);
    let w = load(&path);
    assert_eq!(w.lairs.len(), 1);
    let l = &w.lairs[0];
    // Stock Lairs: GroupIndex "group-min-max"; MobList without monsters that regenerate.
    assert_eq!(l.group_index, "6-1-2");
    assert_eq!(l.mob_list, "1,2,4");
    assert_eq!((l.mobs, l.total_lairs), (3, 2));
    // Delays 4 and 5 average 4.5; VB rounds half to even.
    assert_eq!(l.avg_delay, 4);
    // Monster exp is 100 * number (exp_multi 1); the mean of 1, 2, 4 is 700/3.
    assert_eq!(l.avg_exp, 233.0);
    assert_eq!(l.avg_hp, 23.0);
    assert_eq!(l.avg_ac, 2);
}

#[test]
fn summoned_by_lists_rooms_then_groups() {
    let (_d, path, conn) = new_db(&[1, 2]);
    seed(&conn);
    let w = load(&path);
    // Stock Monsters.SummonedBy: "Room m/r, " then groups joined by "," (lair rooms carry
    // their group and maximum).
    assert_eq!(
        w.get_monster(1).unwrap().summoned_by.as_deref(),
        Some("Room 1/13, [6-1-2][2]Group(lair): 1/10,[6-1-2][2]Group(lair): 1/11")
    );
    assert_eq!(
        w.get_monster(4).unwrap().summoned_by.as_deref(),
        Some("[6-1-2][2]Group(lair): 1/10,[6-1-2][2]Group(lair): 1/11,Group: 1/12")
    );
    // A monster of another type is generated nowhere.
    assert_eq!(w.get_monster(5).unwrap().summoned_by, None);
}

#[test]
fn obtained_from_lists_rooms_shops_and_monsters() {
    let (_d, path, conn) = new_db(&[1, 2]);
    seed(&conn);
    let w = load(&path);
    // Stock Items.ObtainedFrom order: rooms, shops, monsters; a restocking shop is "Shop".
    assert_eq!(
        w.get_item(7).unwrap().obtained_from.as_deref(),
        Some("Room 1/10, Shop #3, Monster #4(20%)")
    );
}

#[test]
fn spells_get_their_sources() {
    let (_d, path, conn) = new_db(&[1, 2]);
    seed(&conn);
    let w = load(&path);
    let learn = w.get_spell(50).unwrap();
    assert!(learn.learnable);
    assert_eq!(learn.learned_from.as_deref(), Some("Item #7"));
    // An item with no class restriction teaches every class.
    assert_eq!(learn.classes.as_deref(), Some("(*)"));
    let cast = w.get_spell(51).unwrap();
    assert!(!cast.learnable);
    // Rooms come before monsters in the stock Spells.CastedBy.
    assert_eq!(cast.casted_by.as_deref(), Some("Room 1/10, Monster #4"));
}

#[test]
fn long_text_ends_in_a_plus() {
    let (_d, path, conn) = new_db(&[1, 2]);
    seed(&conn);
    // 400 rooms hide item 7; the stock column holds 1996 characters, the last being "+".
    for n in 0..400 {
        room(&conn, 100 + n, 2, n + 1, &[]);
        insert(
            &conn,
            "room_floor_items",
            &[
                ("room_id", i(100 + n)),
                ("hidden", i(1)),
                ("slot", i(0)),
                ("item", i(7)),
                ("uses", i(-1)),
            ],
        );
    }
    let w = load(&path);
    let text = w.get_item(7).unwrap().obtained_from.clone().unwrap();
    assert_eq!(text.len(), 1996);
    assert!(text.ends_with('+'));
}

fn block(conn: &Connection, number: i64, link: i64, text: &str) {
    insert(
        conn,
        "text_blocks",
        &[
            ("part", i(0)),
            ("number", i(number)),
            ("link", i(link)),
            ("text", t(text)),
        ],
    );
}

/// Two monsters with rooms of their own; monster 1 has an ask tree that only shows text, monster
/// 3 an ask tree that gives an item and teaches a spell; a room command that rolls a random
/// block, which summons monster 2, gives an item and casts a spell.
fn seed_scripts(conn: &Connection) {
    for n in 1..=3 {
        monster(conn, n, 9, 1, 0);
    }
    for (n, text) in [(1, 300), (3, 310)] {
        conn.execute(
            "UPDATE monsters SET talktext = ?1 WHERE number = ?2",
            [text, n],
        )
        .unwrap();
    }
    room(conn, 1, 1, 20, &[("specmon", 1), ("cmdtext", 320)]);
    room(conn, 2, 1, 21, &[("specmon", 3)]);
    for (n, name) in [(70, "gem"), (71, "ring")] {
        insert(conn, "items", &[("number", i(n)), ("name", t(name))]);
    }
    for (n, name) in [(80, "lore"), (81, "flash")] {
        insert(conn, "spells", &[("number", i(n)), ("name", t(name))]);
    }
    block(conn, 300, 0, "hello:301\n");
    block(conn, 301, 0, "Welcome, traveller.\n");
    block(conn, 310, 0, "gem:random 311\n");
    block(conn, 311, 0, "40:giveitem 70\n100:class 4:learnspell 80\n");
    block(
        conn,
        320,
        0,
        "pull lever:random 321:testskill agility 0 322:testskill agility 0 323\n",
    );
    // Lines of 30 and 70 percent: the first summons monster 2, the second gives an item and casts.
    block(conn, 321, 0, "30:summon 2\n100:giveitem 71:cast 81\n");
    // Run as a script because it has an action; 323 only shows a message, so it is text.
    block(conn, 322, 0, "message 5:teleport 10 1\n");
    block(conn, 323, 0, "message 6\n");
    // Nothing reaches this one.
    block(conn, 900, 0, "giveitem 70\n");
}

#[test]
fn scripts_fill_the_tbinfo_rows() {
    let (_d, path, conn) = new_db(&[1, 2]);
    seed_scripts(&conn);
    let w = load(&path);
    // Only blocks something reaches have a row.
    let numbers: Vec<i32> = w.tbinfo.iter().map(|r| r.number).collect();
    assert_eq!(numbers, [300, 301, 310, 311, 320, 321, 322, 323]);
    let row = |n| w.get_tbinfo(n).unwrap();
    // Action is the script text of a block that is read as a script.
    assert_eq!(row(300).action.as_deref(), Some("hello:301\n"));
    assert_eq!(row(301).action, None);
    assert_eq!(
        row(322).action.as_deref(),
        Some("message 5:teleport 10 1\n")
    );
    assert_eq!(row(323).action, None);
    // Called From: roots first, then calling blocks; `random` is marked, a random line's share
    // follows in brackets.
    assert_eq!(row(300).called_from, "Monster #1");
    assert_eq!(row(301).called_from, "Textblock #300");
    assert_eq!(row(311).called_from, "Textblock(rndm) #310");
    assert_eq!(row(320).called_from, "Room 1/20");
    assert_eq!(row(321).called_from, "Textblock(rndm) #320");
    assert_eq!(row(322).called_from, "Textblock #320");
}

#[test]
fn scripts_summon_give_cast_and_teach() {
    let (_d, path, conn) = new_db(&[1, 2]);
    seed_scripts(&conn);
    let w = load(&path);
    // Monster 2 is in no room; the random block's first line (30 percent) summons it.
    assert_eq!(
        w.get_monster(2).unwrap().summoned_by.as_deref(),
        Some("Textblock #321(30%)")
    );
    // The second line is 70 percent; the ask tree of monster 3 hands out its item as NPC #3.
    assert_eq!(
        w.get_item(71).unwrap().obtained_from.as_deref(),
        Some("Textblock #321(70%)")
    );
    assert_eq!(
        w.get_item(70).unwrap().obtained_from.as_deref(),
        Some("NPC #3")
    );
    let cast = w.get_spell(81).unwrap();
    assert_eq!(cast.casted_by.as_deref(), Some("Textblock #321(70%)"));
    // The class of the line that teaches the spell.
    let lore = w.get_spell(80).unwrap();
    assert!(lore.learnable);
    assert_eq!(lore.learned_from.as_deref(), Some("NPC #3"));
    assert_eq!(lore.classes.as_deref(), Some("(4)"));
}

#[test]
fn a_chest_lists_what_its_random_block_gives() {
    let (_d, path, conn) = new_db(&[1, 2]);
    seed_scripts(&conn);
    // Item 72 casts spell 82, whose block calls a random block twice; each call gives item 70
    // on a 25 percent line, so a use gives it with 1 - 0.75^2 = 43.75 percent.
    insert(&conn, "items", &[("number", i(72)), ("name", t("chest"))]);
    insert(
        &conn,
        "item_abilities",
        &[
            ("item", i(72)),
            ("slot", i(0)),
            ("ability", i(43)),
            ("value", i(82)),
        ],
    );
    insert(&conn, "spells", &[("number", i(82)), ("name", t("open"))]);
    insert(
        &conn,
        "spell_abilities",
        &[
            ("spell", i(82)),
            ("slot", i(0)),
            ("ability", i(148)),
            ("value", i(330)),
        ],
    );
    block(&conn, 330, 0, "random 331:random 331\n");
    block(&conn, 331, 0, "25:giveitem 70\n100:message 1\n");
    // The chest has to be obtainable: it lies in room 1/20.
    insert(
        &conn,
        "room_placed_items",
        &[("room_id", i(1)), ("slot", i(0)), ("item", i(72))],
    );
    let w = load(&path);
    // The block itself, then the chest with its chance to one decimal, then the monster whose
    // ask tree gives it, as in the stock Items.ObtainedFrom.
    assert_eq!(
        w.get_item(70).unwrap().obtained_from.as_deref(),
        Some("Textblock #331(25%), Item #72(43.8%), NPC #3")
    );
}

/// The Quicksilver 1.20 database (tools/qs-fetch.sh) derives its script fields, and does it in
/// time for a live reload on a small machine. Skipped without the database.
#[test]
fn quicksilver_derives_the_script_fields_quickly() {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let path = std::env::var_os("TOUCHSTONE_QS_SQLITE")
        .map(std::path::PathBuf::from)
        .or_else(|| home.map(|h| h.join(".cache/touchstone/quicksilver-1.20.sqlite")));
    let Some(path) = path.filter(|p| p.is_file()) else {
        eprintln!("skipped: no Quicksilver database (run tools/qs-fetch.sh)");
        return;
    };
    let db = touchstone_data::silvermere::open(&path).unwrap();
    let start = std::time::Instant::now();
    let w = db.load_world().unwrap();
    let took = start.elapsed();
    eprintln!("Quicksilver 1.20 load with derivation: {took:?}");
    // Generous: the release build takes well under a second; this guards against a walk that
    // does not end.
    assert!(took < std::time::Duration::from_secs(60), "{took:?}");
    assert!(w.tbinfo.iter().any(|r| r.action.is_some()));
    assert!(w.tbinfo.iter().any(|r| r.called_from.contains("Textblock")));
    assert!(w.monsters.iter().any(|m| {
        m.summoned_by
            .as_deref()
            .is_some_and(|s| s.contains("Textblock"))
    }));
    assert!(w.items.iter().any(|i| {
        i.obtained_from
            .as_deref()
            .is_some_and(|s| s.contains("Textblock"))
    }));
}
