// Tests for the MME .mdb loader (T-011).
//
// The expected values are `fixtures/data/stock.json` (made by `tools/data-fixtures.py` from the
// converted stock database): row counts and a short hash per row of a canonical serialisation.
// Here the loaded `World` is serialised the same way, going from model fields back to columns.
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use rusqlite::{Connection, OpenFlags, types::ValueRef};
use touchstone_data::mdb;
use touchstone_model::{AbilitySlot, World};

/// A row as (column, canonical text) pairs in schema order.
type Cells = Vec<(String, String)>;

#[derive(Default)]
struct Row(Cells);

impl Row {
    fn i(&mut self, col: &str, v: i32) {
        self.0.push((col.into(), v.to_string()));
    }
    fn b(&mut self, col: &str, v: bool) {
        self.0.push((col.into(), u8::from(v).to_string()));
    }
    // Same text as Python's repr(float) for the finite values in the data.
    fn f(&mut self, col: &str, v: f64) {
        self.0.push((col.into(), format!("{v:?}")));
    }
    fn s(&mut self, col: &str, v: &str) {
        self.0.push((col.into(), v.into()));
    }
    fn o(&mut self, col: &str, v: &Option<String>) {
        self.0.push((col.into(), v.clone().unwrap_or_default()));
    }
    fn abil<const N: usize>(&mut self, a: &[AbilitySlot; N]) {
        for (n, s) in a.iter().enumerate() {
            self.i(&format!("Abil-{n}"), s.ability);
            self.i(&format!("AbilVal-{n}"), s.value);
        }
    }
    fn series(&mut self, prefix: &str, v: &[i32]) {
        for (n, x) in v.iter().enumerate() {
            self.i(&format!("{prefix}-{n}"), *x);
        }
    }
}

/// Table name, then (record key, cells) for every loaded row.
fn canonical(w: &World) -> Vec<(&'static str, Vec<(String, Cells)>)> {
    let mut t: Vec<(&'static str, Vec<(String, Cells)>)> = Vec::new();

    t.push((
        "Classes",
        w.classes
            .iter()
            .map(|c| {
                let mut r = Row::default();
                r.i("Number", c.number);
                r.s("Name", &c.name);
                r.i("MinHits", c.min_hits);
                r.i("MaxHits", c.max_hits);
                r.i("ExpTable", c.exp_table);
                r.i("MageryType", c.magery_type);
                r.i("MageryLVL", c.magery_lvl);
                r.i("WeaponType", c.weapon_type);
                r.i("ArmourType", c.armour_type);
                r.i("CombatLVL", c.combat_lvl);
                r.abil(&c.abilities);
                (c.number.to_string(), r.0)
            })
            .collect(),
    ));

    let i = &w.info;
    let mut r = Row::default();
    r.s("NMR Version", &i.nmr_version);
    r.s("Dat File Version", &i.dat_file_version);
    r.s("Date", &i.date);
    r.s("Time", &i.time);
    r.s("Custom", &i.custom);
    r.b("Legit", i.legit);
    r.b("DisableKai", i.disable_kai);
    r.s("UpdateURL", &i.update_url);
    t.push(("Info", vec![("0".into(), r.0)]));

    t.push((
        "Items",
        w.items
            .iter()
            .map(|x| {
                let mut r = Row::default();
                r.i("Number", x.number);
                r.s("Name", &x.name);
                r.i("Limit", x.limit);
                r.i("Encum", x.encum);
                r.i("ItemType", x.item_type);
                r.i("UseCount", x.use_count);
                r.i("Price", x.price);
                r.i("Currency", x.currency);
                r.i("Min", x.min);
                r.i("Max", x.max);
                r.i("ArmourClass", x.armour_class);
                r.i("DamageResist", x.damage_resist);
                r.i("WeaponType", x.weapon_type);
                r.i("ArmourType", x.armour_type);
                r.i("Worn", x.worn);
                r.i("Accy", x.accy);
                r.b("Gettable", x.gettable);
                r.i("StrReq", x.str_req);
                r.i("Speed", x.speed);
                r.b("Not Droppable", x.not_droppable);
                r.b("Destroy On Death", x.destroy_on_death);
                r.b("Retain After Uses", x.retain_after_uses);
                r.series("ClassRest", &x.class_rest);
                r.series("RaceRest", &x.race_rest);
                r.series("NegateSpell", &x.negate_spell);
                r.abil(&x.abilities);
                r.b("In Game", x.in_game);
                r.o("Obtained From", &x.obtained_from);
                r.o("References", &x.references);
                (x.number.to_string(), r.0)
            })
            .collect(),
    ));

    t.push((
        "Lairs",
        w.lairs
            .iter()
            .map(|l| {
                let mut r = Row::default();
                r.s("GroupIndex", &l.group_index);
                r.s("MobList", &l.mob_list);
                r.i("Mobs", l.mobs);
                r.i("TotalLairs", l.total_lairs);
                r.i("AvgDelay", l.avg_delay);
                r.f("AvgWalk", l.avg_walk);
                r.f("AvgExp", l.avg_exp);
                r.f("AvgDmg", l.avg_dmg);
                r.f("AvgDmgPhys", l.avg_dmg_phys);
                r.f("AvgDmgSpell", l.avg_dmg_spell);
                r.f("AvgHP", l.avg_hp);
                r.i("AvgAC", l.avg_ac);
                r.i("AvgDR", l.avg_dr);
                r.i("AvgMR", l.avg_mr);
                r.i("AvgDodge", l.avg_dodge);
                (l.group_index.clone(), r.0)
            })
            .collect(),
    ));

    t.push((
        "Monsters",
        w.monsters
            .iter()
            .map(|m| {
                let mut r = Row::default();
                r.i("Number", m.number);
                r.o("Name", &m.name);
                r.i("Weapon", m.weapon);
                r.i("ArmourClass", m.armour_class);
                r.i("DamageResist", m.damage_resist);
                r.i("Follow%", m.follow_pct);
                r.i("MagicRes", m.magic_res);
                r.i("BSDefense", m.bs_defense);
                r.f("EXP", m.exp);
                r.f("ExpMulti", m.exp_multi);
                r.i("HP", m.hp);
                r.i("Energy", m.energy);
                r.f("AvgDmg", m.avg_dmg);
                r.f("AvgDmgPhys", m.avg_dmg_phys);
                r.f("AvgDmgSpell", m.avg_dmg_spell);
                r.i("GreetTXT", m.greet_txt);
                r.i("HPRegen", m.hp_regen);
                r.i("CharmLVL", m.charm_lvl);
                r.i("Type", m.r#type);
                r.i("Undead", m.undead);
                r.i("Align", m.align);
                r.i("RegenTime", m.regen_time);
                r.i("GameLimit", m.game_limit);
                r.f("AvgLairExp", m.avg_lair_exp);
                r.f("ScriptValue", m.script_value);
                r.i("R", m.r);
                r.i("P", m.p);
                r.i("G", m.g);
                r.i("S", m.s);
                r.i("C", m.c);
                r.i("DeathSpell", m.death_spell);
                r.i("CreateSpell", m.create_spell);
                for (n, a) in m.attacks.iter().enumerate() {
                    r.s(&format!("AttName-{n}"), &a.name);
                    r.i(&format!("AttType-{n}"), a.att_type);
                    r.i(&format!("AttAcc-{n}"), a.acc);
                    r.i(&format!("Att%-{n}"), a.pct);
                    r.f(&format!("AttTrue%-{n}"), a.true_pct);
                    r.i(&format!("AttMin-{n}"), a.min);
                    r.i(&format!("AttMax-{n}"), a.max);
                    r.i(&format!("AttEnergy-{n}"), a.energy);
                    r.i(&format!("AttHitSpell-{n}"), a.hit_spell);
                }
                for (n, s) in m.mid_spells.iter().enumerate() {
                    r.i(&format!("MidSpell-{n}"), s.spell);
                    r.i(&format!("MidSpell%-{n}"), s.pct);
                    r.i(&format!("MidSpellLVL-{n}"), s.lvl);
                }
                for (n, d) in m.drops.iter().enumerate() {
                    r.i(&format!("DropItem-{n}"), d.item);
                    r.i(&format!("DropItem%-{n}"), d.pct);
                }
                r.abil(&m.abilities);
                r.b("In Game", m.in_game);
                r.o("Summoned By", &m.summoned_by);
                (m.number.to_string(), r.0)
            })
            .collect(),
    ));

    t.push((
        "Races",
        w.races
            .iter()
            .map(|x| {
                let mut r = Row::default();
                r.i("Number", x.number);
                r.s("Name", &x.name);
                r.i("mINT", x.m_int);
                r.i("mWIL", x.m_wil);
                r.i("mSTR", x.m_str);
                r.i("mHEA", x.m_hea);
                r.i("mAGL", x.m_agl);
                r.i("mCHM", x.m_chm);
                r.i("xINT", x.x_int);
                r.i("xWIL", x.x_wil);
                r.i("xSTR", x.x_str);
                r.i("xHEA", x.x_hea);
                r.i("xAGL", x.x_agl);
                r.i("xCHM", x.x_chm);
                r.i("HPPerLVL", x.hp_per_lvl);
                r.i("ExpTable", x.exp_table);
                r.i("BaseCP", x.base_cp);
                r.abil(&x.abilities);
                (x.number.to_string(), r.0)
            })
            .collect(),
    ));

    t.push((
        "Rooms",
        w.rooms
            .iter()
            .map(|x| {
                let mut r = Row::default();
                r.i("Map Number", x.map_number);
                r.i("Room Number", x.room_number);
                r.o("Name", &x.name);
                r.i("Light", x.light);
                r.i("Shop", x.shop);
                r.i("NPC", x.npc);
                r.i("CMD", x.cmd);
                r.i("Spell", x.spell);
                r.o("Lair", &x.lair);
                r.i("Delay", x.delay);
                r.o("Placed", &x.placed);
                for (c, v) in [
                    ("N", &x.n),
                    ("S", &x.s),
                    ("E", &x.e),
                    ("W", &x.w),
                    ("NE", &x.ne),
                    ("NW", &x.nw),
                    ("SE", &x.se),
                    ("SW", &x.sw),
                    ("U", &x.u),
                    ("D", &x.d),
                ] {
                    r.s(c, v);
                }
                (format!("{}/{}", x.map_number, x.room_number), r.0)
            })
            .collect(),
    ));

    t.push((
        "Shops",
        w.shops
            .iter()
            .map(|x| {
                let mut r = Row::default();
                r.i("Number", x.number);
                r.s("Name", &x.name);
                r.i("ShopType", x.shop_type);
                r.i("MinLVL", x.min_lvl);
                r.i("MaxLVL", x.max_lvl);
                r.i("Markup%", x.markup_pct);
                r.i("ClassRest", x.class_rest);
                for (n, s) in x.stock.iter().enumerate() {
                    r.i(&format!("Item-{n}"), s.item);
                    r.i(&format!("Max-{n}"), s.max);
                    r.i(&format!("Time-{n}"), s.time);
                    r.i(&format!("Amount-{n}"), s.amount);
                    r.i(&format!("%-{n}"), s.pct);
                }
                r.b("In Game", x.in_game);
                r.o("Assigned To", &x.assigned_to);
                (x.number.to_string(), r.0)
            })
            .collect(),
    ));

    t.push((
        "Spells",
        w.spells
            .iter()
            .map(|x| {
                let mut r = Row::default();
                r.i("Number", x.number);
                r.o("Name", &x.name);
                r.o("Short", &x.short);
                r.i("ReqLevel", x.req_level);
                r.i("EnergyCost", x.energy_cost);
                r.i("ManaCost", x.mana_cost);
                r.i("MinBase", x.min_base);
                r.i("MaxBase", x.max_base);
                r.i("Diff", x.diff);
                r.i("TypeOfResists", x.type_of_resists);
                r.i("Targets", x.targets);
                r.i("Dur", x.dur);
                r.i("AttType", x.att_type);
                r.i("Magery", x.magery);
                r.i("MageryLVL", x.magery_lvl);
                r.i("Cap", x.cap);
                r.i("MaxIncLVLs", x.max_inc_lvls);
                r.i("MaxInc", x.max_inc);
                r.i("MinIncLVLs", x.min_inc_lvls);
                r.i("MinInc", x.min_inc);
                r.i("DurIncLVLs", x.dur_inc_lvls);
                r.i("DurInc", x.dur_inc);
                r.abil(&x.abilities);
                r.b("Learnable", x.learnable);
                r.o("Learned From", &x.learned_from);
                r.o("Casted By", &x.casted_by);
                r.o("Classes", &x.classes);
                (x.number.to_string(), r.0)
            })
            .collect(),
    ));

    t.push((
        "TBInfo",
        w.tbinfo
            .iter()
            .map(|x| {
                let mut r = Row::default();
                r.i("Number", x.number);
                r.i("LinkTo", x.link_to);
                r.o("Action", &x.action);
                r.s("Called From", &x.called_from);
                (x.number.to_string(), r.0)
            })
            .collect(),
    ));
    t
}

// SHA-256, so the test needs no extra dependency; checked against the standard vector below.
fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for block in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, c) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for (a, b) in h.iter_mut().zip(v) {
            *a = a.wrapping_add(b);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

#[test]
fn sha256_matches_standard_vectors() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

fn row_hash(cells: &Cells) -> String {
    let joined = cells
        .iter()
        .map(|(_, v)| v.as_str())
        .collect::<Vec<_>>()
        .join("\u{1f}");
    sha256_hex(joined.as_bytes())[..12].to_string()
}

/// Mirrors the Python canonical text for a raw SQLite value, used only to name the column of a
/// mismatching row.
fn raw_text(v: ValueRef<'_>, real_col: bool) -> String {
    match v {
        ValueRef::Null => String::new(),
        ValueRef::Integer(i) if real_col => format!("{:?}", i as f64),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => format!("{f:?}"),
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
        ValueRef::Blob(_) => "<blob>".into(),
    }
}

fn stock_sqlite() -> PathBuf {
    if let Some(p) = std::env::var_os("TOUCHSTONE_STOCK_SQLITE") {
        return PathBuf::from(p);
    }
    let home = std::env::var_os("HOME").expect("HOME is not set");
    PathBuf::from(home).join(
        ".cache/touchstone/mdb/eba20f06af8dc08df755eb6dcd9763b023794f12e809cbab6762236d64d7345f.sqlite",
    )
}

fn stock_path() -> PathBuf {
    let p = stock_sqlite();
    assert!(
        p.is_file(),
        "the stock SQLite is missing at {}: run tools/mdb-fetch.sh (needs mdbtools) or set \
         TOUCHSTONE_STOCK_SQLITE",
        p.display()
    );
    p
}

/// Columns whose raw value differs from the loaded row, for one mismatching record.
fn diff_columns(conn: &Connection, table: &str, pos: usize, loaded: &Cells) -> String {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT * FROM \"{table}\" ORDER BY rowid LIMIT 1 OFFSET {pos}"
        ))
        .unwrap();
    let decl: Vec<bool> = conn
        .prepare(&format!("PRAGMA table_info(\"{table}\")"))
        .unwrap()
        .query_map([], |r| r.get::<_, String>(2))
        .unwrap()
        .map(|d| d.unwrap().eq_ignore_ascii_case("REAL"))
        .collect();
    let mut rows = stmt.query([]).unwrap();
    let row = rows.next().unwrap().expect("row exists");
    let mut out = String::new();
    for (idx, (name, val)) in loaded.iter().enumerate() {
        let raw = raw_text(row.get_ref(idx).unwrap(), decl[idx]);
        if &raw != val {
            let _ = write!(out, " [{name}: loaded {val:?}, database {raw:?}]");
        }
    }
    out
}

#[test]
fn stock_world_matches_fixture() {
    let path = stock_path();
    let t0 = Instant::now();
    let world = mdb::load(&path).expect("load stock database");
    // Run with --nocapture (release) to see it.
    println!("stock World loaded in {:?}", t0.elapsed());

    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../../fixtures/data/stock.json")).unwrap();
    assert_eq!(fixture["hash_len"], 12);
    let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();

    let mut problems: Vec<String> = Vec::new();
    let mut seen = BTreeMap::new();
    for (table, rows) in canonical(&world) {
        seen.insert(table, rows.len());
        let fx = &fixture["tables"][table];
        let want_count = fx["count"].as_u64().unwrap() as usize;
        if rows.len() != want_count {
            problems.push(format!("{table}: {} rows, want {want_count}", rows.len()));
            continue;
        }
        let want_cols: Vec<&str> = fx["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap())
            .collect();
        let want_rows = fx["rows"].as_array().unwrap();
        for (pos, (key, cells)) in rows.iter().enumerate() {
            let got_cols: Vec<&str> = cells.iter().map(|(c, _)| c.as_str()).collect();
            if got_cols != want_cols {
                problems.push(format!(
                    "{table}: serialised columns differ from the fixture's"
                ));
                break;
            }
            let want_key = want_rows[pos][0].as_str().unwrap();
            let want_hash = want_rows[pos][1].as_str().unwrap();
            if key != want_key || row_hash(cells) != want_hash {
                problems.push(format!(
                    "{table} record {key} (fixture {want_key}):{}",
                    diff_columns(&conn, table, pos, cells)
                ));
                if problems.len() > 40 {
                    break;
                }
            }
        }
    }
    assert_eq!(seen.len(), 10, "every table is checked");
    assert!(problems.is_empty(), "mismatches:\n{}", problems.join("\n"));
}

#[test]
fn lookups_use_record_numbers() {
    let world = mdb::load(stock_path()).expect("load stock database");
    // Counts from the fixture's table sizes; lookups are by Number, not position.
    let first = &world.items[0];
    assert_eq!(world.get_item(first.number).unwrap().name, first.name);
    let room = &world.rooms[world.rooms.len() / 2];
    assert_eq!(
        world.get_room(room.map_number, room.room_number).unwrap(),
        room
    );
    assert!(world.get_item(i32::MAX).is_none());
}

#[test]
fn missing_file_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let err = mdb::load(dir.path().join("nope.sqlite")).unwrap_err();
    assert!(err.to_string().contains("cannot open"), "{err}");
}

#[test]
fn info_must_have_one_row() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("empty.sqlite");
    let conn = Connection::open(&p).unwrap();
    conn.execute_batch(
        r#"CREATE TABLE "Info" ("NMR Version" TEXT, "Dat File Version" TEXT, "Date" TEXT,
           "Time" TEXT, "Custom" TEXT, "Legit" INTEGER, "DisableKai" INTEGER, "UpdateURL" TEXT);"#,
    )
    .unwrap();
    drop(conn);
    let err = mdb::load(&p).unwrap_err();
    assert!(err.to_string().contains("exactly one row"), "{err}");
}
