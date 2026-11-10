// The realm reader (card R-001). All records here are invented.
use rusqlite::Connection;
use touchstone_data::silvermere;

use crate::silvermere::{i, insert, new_db, t};

fn seed_world(conn: &Connection) {
    // a finished import is all load_snapshot needs of the world
    insert(
        conn,
        "world_import",
        &[("finished_at", t("2026-01-01T00:00:00Z"))],
    );
}

fn character(conn: &Connection, id: i64, first: &str) {
    insert(
        conn,
        "accounts",
        &[
            ("id", i(id)),
            ("username", t(&format!("login-of-{first}"))),
            ("password_hash", t("$argon2id$SECRET-HASH")),
            ("email", t("someone@example.invalid")),
            ("created", t("2026-01-01T00:00:00Z")),
        ],
    );
    insert(
        conn,
        "characters",
        &[
            ("id", i(id)),
            ("account_id", i(id)),
            ("userid", t(&format!("login-of-{first}"))),
            ("firstname", t(first)),
            ("lastname", t("Tester")),
            ("title", t("Squire")),
            ("race", i(2)),
            ("class", i(3)),
            ("level", i(12)),
            ("expbil", i(2)),
            ("exprem", i(345)),
            ("basstr", i(61)),
            ("basint", i(42)),
            ("baswil", i(43)),
            ("basagi", i(64)),
            ("bashea", i(55)),
            ("bascha", i(46)),
            ("str", i(65)),
            ("maxhp", i(210)),
            ("gang", t("The Invented")),
            ("lastdate", i(1_700_000_000)),
            ("suicpw", t("SUICIDE-SECRET")),
            ("weapon", i(77)),
            ("copper", i(9)),
        ],
    );
    insert(
        conn,
        "character_worn",
        &[("character_id", i(id)), ("slot", i(0)), ("item", i(5)), ("uses", i(-1))],
    );
    insert(
        conn,
        "character_worn",
        &[("character_id", i(id)), ("slot", i(1)), ("item", i(0)), ("uses", i(0))],
    );
    insert(
        conn,
        "character_inventory",
        &[("character_id", i(id)), ("slot", i(3)), ("item", i(9)), ("uses", i(4))],
    );
    insert(
        conn,
        "character_spellbook",
        &[("character_id", i(id)), ("slot", i(0)), ("spell", i(31))],
    );
    insert(
        conn,
        "character_abilities",
        &[("character_id", i(id)), ("slot", i(0)), ("ability", i(22)), ("value", i(7))],
    );
    insert(
        conn,
        "character_spells_cast",
        &[
            ("character_id", i(id)),
            ("slot", i(2)),
            ("spell", i(31)),
            ("value", i(5)),
            ("rounds", i(40)),
        ],
    );
    insert(
        conn,
        "character_profiles",
        &[
            ("account_id", i(id)),
            ("userid", t("PROFILE-USERID")),
            ("statfmt", t("PROFILE-STATLINE")),
        ],
    );
}

#[test]
fn reads_every_character_with_its_lists() {
    let (_d, path, conn) = new_db(&[1, 2]);
    seed_world(&conn);
    character(&conn, 1, "Alice");
    character(&conn, 2, "Bob");
    let (_world, realm) = silvermere::open(&path).unwrap().load_snapshot().unwrap();
    assert_eq!(realm.characters.len(), 2);
    let a = realm.get(1).unwrap();
    assert_eq!((a.first_name.as_str(), a.last_name.as_str()), ("Alice", "Tester"));
    assert_eq!((a.race, a.class, a.level), (2, 3, 12));
    // the stat boxes' order is Str Int Wil Agl Hea Chm
    assert_eq!(a.base_stats, [61, 42, 43, 64, 55, 46]);
    assert_eq!(a.current_stats[0], 65);
    assert_eq!(a.exp, 2_000_000_345);
    assert_eq!((a.gang.as_str(), a.weapon, a.coins[4]), ("The Invented", 77, 9));
    // an empty slot (item 0) is not a worn item
    assert_eq!(a.worn.len(), 1);
    assert_eq!((a.worn[0].slot, a.worn[0].item, a.worn[0].uses), (0, 5, -1));
    assert_eq!(a.inventory[0].item, 9);
    assert_eq!(a.spellbook, vec![31]);
    assert_eq!(a.abilities, vec![(22, 7)]);
    assert_eq!(
        (a.spells_cast[0].spell, a.spells_cast[0].value, a.spells_cast[0].rounds),
        (31, 5, 40)
    );
    assert_eq!(realm.get(2).unwrap().first_name, "Bob");
    assert!(realm.get(3).is_none());
}

#[test]
fn an_empty_realm_is_an_empty_list() {
    let (_d, path, conn) = new_db(&[1, 2]);
    seed_world(&conn);
    let (_w, realm) = silvermere::open(&path).unwrap().load_snapshot().unwrap();
    assert!(realm.characters.is_empty());
}

#[test]
fn nothing_private_is_read() {
    let (_d, path, conn) = new_db(&[1, 2]);
    seed_world(&conn);
    character(&conn, 1, "Alice");
    let (_w, realm) = silvermere::open(&path).unwrap().load_snapshot().unwrap();
    let all = format!("{realm:?}");
    for secret in [
        "SECRET-HASH",
        "SUICIDE-SECRET",
        "someone@example.invalid",
        "login-of-Alice",
        "PROFILE-USERID",
        "PROFILE-STATLINE",
    ] {
        assert!(!all.contains(secret), "{secret} reached the realm: {all}");
    }
}
