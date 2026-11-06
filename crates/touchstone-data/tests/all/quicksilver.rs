// Loads a real Quicksilver 1.20 database (built by tools/qs-fetch.sh, kept in the cache and never
// committed) and pins what the census in docs/data-gaps.md says about it. Without the database
// the test says so and passes, so a machine that has not built one still runs the workspace.
use std::path::PathBuf;

use rusqlite::{Connection, OpenFlags};
use touchstone_data::silvermere;
use touchstone_model::AbilitySlot;

fn qs_path() -> Option<PathBuf> {
    let p = match std::env::var_os("TOUCHSTONE_QS_SQLITE") {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from(std::env::var_os("HOME")?)
            .join(".cache/touchstone/quicksilver-1.20.sqlite"),
    };
    if p.is_file() {
        Some(p)
    } else {
        eprintln!(
            "skipped: no Quicksilver database at {} (run tools/qs-fetch.sh)",
            p.display()
        );
        None
    }
}

fn scalar(conn: &Connection, sql: &str) -> usize {
    conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap() as usize
}

fn used(slots: &[AbilitySlot]) -> usize {
    slots.iter().filter(|s| s.ability != 0).count()
}

#[test]
fn quicksilver_world_loads_with_every_row_and_ability() {
    let Some(path) = qs_path() else { return };
    let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let w = silvermere::open(&path).unwrap().load_world().unwrap();

    // Census, docs/data-gaps.md "Quicksilver 1.20 census": the 1.20.0 content, in the database's
    // migration 9001 column.
    assert_eq!(w.info.dat_file_version, "1.20.0");

    // Every table row reaches the model.
    assert_eq!(w.items.len(), scalar(&conn, "SELECT count(*) FROM items"));
    assert_eq!(
        w.monsters.len(),
        scalar(&conn, "SELECT count(*) FROM monsters")
    );
    assert_eq!(w.spells.len(), scalar(&conn, "SELECT count(*) FROM spells"));
    assert_eq!(w.rooms.len(), scalar(&conn, "SELECT count(*) FROM rooms"));
    assert_eq!(w.shops.len(), scalar(&conn, "SELECT count(*) FROM shops"));
    assert_eq!(
        w.classes.len(),
        scalar(&conn, "SELECT count(*) FROM classes")
    );
    assert_eq!(w.races.len(), scalar(&conn, "SELECT count(*) FROM races"));
    // The 1.20 content adds rows to the stock counts (census table).
    assert_eq!(
        (w.items.len(), w.spells.len(), w.races.len()),
        (1974, 1457, 13)
    );

    // No ability row is dropped or merged into a slot.
    let in_db = |t: &str| {
        scalar(
            &conn,
            &format!("SELECT count(*) FROM {t} WHERE ability <> 0"),
        )
    };
    assert_eq!(
        w.items.iter().map(|r| used(&r.abilities)).sum::<usize>(),
        in_db("item_abilities")
    );
    assert_eq!(
        w.monsters.iter().map(|r| used(&r.abilities)).sum::<usize>(),
        in_db("monster_abilities")
    );
    assert_eq!(
        w.spells.iter().map(|r| used(&r.abilities)).sum::<usize>(),
        in_db("spell_abilities")
    );
    assert_eq!(
        w.classes.iter().map(|r| used(&r.abilities)).sum::<usize>(),
        in_db("class_abilities")
    );
    assert_eq!(
        w.races.iter().map(|r| used(&r.abilities)).sum::<usize>(),
        in_db("race_abilities")
    );

    // touchstone names abilities 0 to 187 (rules::ability_text); the content uses none above.
    let top = [
        "item_abilities",
        "monster_abilities",
        "spell_abilities",
        "class_abilities",
        "race_abilities",
    ]
    .iter()
    .map(|t| scalar(&conn, &format!("SELECT max(ability) FROM {t}")))
    .max()
    .unwrap();
    assert!(top <= 187, "ability {top} has no name in touchstone");
    assert_eq!(
        scalar(
            &conn,
            "SELECT count(*) FROM item_abilities WHERE ability < 0"
        ),
        0
    );
}
