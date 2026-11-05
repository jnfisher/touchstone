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
    assert_eq!(cast.casted_by.as_deref(), Some("Monster #4, Room 1/10"));
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
