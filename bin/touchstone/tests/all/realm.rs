// Realm characters (card R-001): the endpoints over the fixture characters, applied to a copy of the
// cached Quicksilver 1.20 database, and over small invented realms that need no cache.
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use tempfile::TempDir;
use touchstone::AppState;
use touchstone::config::Config;
use touchstone::source;
use tower::ServiceExt;

use crate::common::config_for;

/// The cached Quicksilver database (built by tools/qs-fetch.sh, never committed), if there is one.
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

/// The fixture realm, built once: tests only read it.
fn fixture_state() -> Option<Arc<AppState>> {
    static STATE: OnceLock<Option<Arc<AppState>>> = OnceLock::new();
    STATE.get_or_init(build_fixture).clone()
}

/// A copy of the Quicksilver database with the fixture characters applied, started through the
/// real source code.
fn build_fixture() -> Option<Arc<AppState>> {
    build_fixture_with("")
}

fn build_fixture_with(extra_sql: &str) -> Option<Arc<AppState>> {
    let src = qs_path()?;
    let dir = TempDir::new().unwrap().keep();
    let path = dir.join("realm.db");
    {
        let conn = Connection::open_with_flags(&src, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        // VACUUM INTO folds the WAL in, so the copy is whole whatever the cache's state
        conn.execute("VACUUM INTO ?1", [path.to_str().unwrap()])
            .unwrap();
    }
    let sql = std::fs::read_to_string(fixture_dir().join("realm_characters.sql")).unwrap();
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(&sql).unwrap();
    conn.execute_batch(extra_sql).unwrap();
    drop(conn);
    let cfg = Config {
        source: Some(config_for(&path)),
        data_dir: dir.join("data"),
        ..Config::default()
    };
    Some(source::start(&cfg).unwrap().state)
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/quicksilver")
}

async fn get(state: &Arc<AppState>, uri: &str) -> (StatusCode, Value) {
    let (st, bytes) = get_raw(state, uri).await;
    (st, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn get_raw(state: &Arc<AppState>, uri: &str) -> (StatusCode, Vec<u8>) {
    let res = touchstone::app(state.clone())
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, bytes.to_vec())
}

async fn post(state: &Arc<AppState>, uri: &str, body: &Value) -> (StatusCode, Value) {
    let res = touchstone::app(state.clone())
        .oneshot(
            Request::post(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// The detail of every fixture character, by first name.
async fn details(state: &Arc<AppState>) -> Vec<(String, Value)> {
    let (st, list) = get(state, "/api/realm/characters").await;
    assert_eq!(st, StatusCode::OK);
    let mut out = Vec::new();
    for c in list["characters"].as_array().unwrap() {
        let id = c["id"].as_i64().unwrap();
        let (st, d) = get(state, &format!("/api/realm/characters/{id}")).await;
        assert_eq!(st, StatusCode::OK);
        out.push((c["first_name"].as_str().unwrap().to_owned(), d));
    }
    out
}

fn comparison(d: &Value) -> &Vec<Value> {
    d["comparison"].as_array().unwrap()
}

/// `[field, game, touchstone, differs, cause]` per comparison, one line per character.
fn pin_of(d: &Value) -> Value {
    Value::Array(
        comparison(d)
            .iter()
            .map(|c| {
                serde_json::json!([
                    c["field"],
                    c["game"],
                    c["touchstone"],
                    c["differs"],
                    c["cause"]
                ])
            })
            .collect(),
    )
}

#[tokio::test]
async fn fixture_characters_are_listed_with_sheets_and_comparisons() {
    let Some(state) = fixture_state() else { return };
    let (st, list) = get(&state, "/api/realm/characters").await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(list["available"], true);
    let rows = list["characters"].as_array().unwrap();
    // the generator's character count (tools/qs-vectors/src/realm.rs SPECS)
    assert_eq!(rows.len(), 12);
    let first = &rows[0];
    for key in [
        "id",
        "first_name",
        "last_name",
        "race",
        "class",
        "level",
        "exp",
        "gang",
        "map",
        "room",
        "last_on",
        "lives",
        "differs",
    ] {
        assert!(first.get(key).is_some(), "summary lacks {key}: {first}");
    }
    assert_eq!(first["first_name"], "Aldric");
    assert_eq!(first["level"], 1);
    assert_eq!(first["gang"], "Ironwood");
    for (name, d) in details(&state).await {
        assert!(d["sheet"].is_object(), "{name}: {}", d["sheet_error"]);
        assert!(d["sheet"]["stats"].as_array().unwrap().len() == 47);
        assert_eq!(d["identity"]["first_name"], name.as_str());
        let n = comparison(&d).len();
        assert_eq!(n, 27, "{name}");
        let differs = comparison(&d)
            .iter()
            .filter(|c| c["differs"] == true)
            .count();
        assert_eq!(d["differs"], differs, "{name}");
    }
}

#[tokio::test]
async fn the_comparison_of_each_fixture_character_is_pinned() {
    let Some(state) = fixture_state() else { return };
    let mut pins = serde_json::Map::new();
    for (name, d) in details(&state).await {
        pins.insert(name, pin_of(&d));
    }
    let path = fixture_dir().join("realm_pins.json");
    if std::env::var_os("TOUCHSTONE_BLESS_REALM").is_some() {
        let mut text = String::from("{\n");
        let n = pins.len();
        for (i, (k, v)) in pins.iter().enumerate() {
            text.push_str(&format!("{k:?}: {v}{}\n", if i + 1 < n { "," } else { "" }));
        }
        text.push_str("}\n");
        std::fs::write(&path, text).unwrap();
        return;
    }
    // Expected: fixtures/quicksilver/realm_pins.json. The game column is the stored value of the
    // fixture record (built by Quicksilver's balance-tool builder); the touchstone column is this
    // pipeline's answer, written once by TOUCHSTONE_BLESS_REALM=1 and reviewed against
    // docs/differences.md ("Realm characters").
    let want: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(Value::Object(pins), want);
}

#[tokio::test]
async fn every_difference_has_a_known_cause() {
    let Some(state) = fixture_state() else { return };
    let mut all = 0;
    let mut agree = 0;
    for (name, d) in details(&state).await {
        for c in comparison(&d) {
            all += 1;
            if c["differs"] == false {
                agree += 1;
                assert!(c["cause"].is_null(), "{name} {c}");
            } else {
                // each kind of cause is written up in docs/differences.md, "Realm characters"
                assert!(c["cause"].is_string(), "{name}: unexplained difference {c}");
            }
        }
    }
    eprintln!("realm comparison: {agree} of {all} fields agree");
}

#[tokio::test]
async fn items_mme_has_no_place_for_are_listed_not_dropped() {
    let Some(state) = fixture_state() else { return };
    let all = details(&state).await;
    let of = |n: &str| &all.iter().find(|(k, _)| k == n).unwrap().1;

    // Cormac wears a second head item: MME holds one
    let cormac = of("Cormac");
    let un = cormac["unmapped"].as_array().unwrap();
    assert_eq!(un.len(), 1, "{un:?}");
    assert_eq!(un[0]["kind"], "worn");
    assert!(un[0]["reason"].as_str().unwrap().contains("Head slot"));
    let worn = cormac["worn"].as_array().unwrap();
    assert_eq!(worn.iter().filter(|w| w["mme_slot"].is_null()).count(), 1);

    // Hale wears a thrown item that has a hands location: no MME slot for its type
    let hale = of("Hale");
    let un = hale["unmapped"].as_array().unwrap();
    assert!(
        un.iter().any(|u| u["kind"] == "worn"
            && u["reason"].as_str().unwrap().contains("no equip slot")),
        "{un:?}"
    );

    // Fenwick's purse is heavier than MME's extra-weight box
    let fenwick = of("Fenwick");
    assert!(
        fenwick["unmapped"]
            .as_array()
            .unwrap()
            .iter()
            .any(|u| u["kind"] == "coins")
    );

    // everyone else maps whole
    for n in [
        "Aldric", "Brenna", "Dara", "Eamon", "Gwen", "Ilsa", "Joren", "Kestrel", "Lorne",
    ] {
        let u = of(n)["unmapped"].as_array().unwrap();
        assert!(u.is_empty(), "{n}: {u:?}");
    }
}

#[tokio::test]
async fn running_spells_that_are_bless_spells_take_a_bless_slot() {
    let Some(state) = fixture_state() else { return };
    let all = details(&state).await;
    let kestrel = &all.iter().find(|(k, _)| k == "Kestrel").unwrap().1;
    let active = kestrel["active_spells"].as_array().unwrap();
    assert_eq!(active.len(), 5);
    let id = kestrel["id"].as_i64().unwrap();
    let (_, input) = get(&state, &format!("/api/realm/characters/{id}/input")).await;
    let bless: Vec<i64> = input["bless"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b.as_i64().unwrap())
        .filter(|&b| b > 0)
        .collect();
    // a bless slot per running spell that is a bless spell, in game order; the rest are unmapped
    let slotted = active
        .iter()
        .filter(|a| a["bless_slot"].is_number())
        .count();
    assert_eq!(bless.len(), slotted);
    let unmapped_spells = kestrel["unmapped"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|u| u["kind"] == "active_spell")
        .count();
    assert_eq!(slotted + unmapped_spells, 5);
}

#[tokio::test]
async fn the_input_evaluates_to_the_sheet_the_detail_shows() {
    let Some(state) = fixture_state() else { return };
    for id in [1, 4, 6, 8, 11] {
        let (st, input) = get(&state, &format!("/api/realm/characters/{id}/input")).await;
        assert_eq!(st, StatusCode::OK);
        // the Characters tab's own endpoint, on the mapped input
        let (st, sheet) = post(&state, "/api/character/evaluate", &input).await;
        assert_eq!(st, StatusCode::OK, "{sheet}");
        let (_, d) = get(&state, &format!("/api/realm/characters/{id}")).await;
        assert_eq!(sheet, d["sheet"], "character {id}");
        // and it is a character the editor can hold
        assert_eq!(input["equipped"].as_array().unwrap().len(), 20);
        assert_eq!(input["stats"].as_array().unwrap().len(), 6);
    }
}

#[tokio::test]
async fn an_unknown_character_is_a_404() {
    let Some(state) = fixture_state() else { return };
    let (st, _) = get(&state, "/api/realm/characters/9999").await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (st, _) = get(&state, "/api/realm/characters/9999/input").await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[test]
fn alignment_follows_the_evil_points() {
    use touchstone::realm::alignment_of;
    // the game: Good below -50, Neutral to 29, Seedy 30 to 39, Outlaw from 40 (Evil in MME)
    assert_eq!(alignment_of(-201), 1);
    assert_eq!(alignment_of(-51), 1);
    assert_eq!(alignment_of(-50), 2);
    assert_eq!(alignment_of(0), 2);
    assert_eq!(alignment_of(39), 2);
    assert_eq!(alignment_of(40), 3);
    assert_eq!(alignment_of(300), 3);
}

#[tokio::test]
async fn a_changed_stored_value_is_flagged_as_a_difference_with_no_known_cause() {
    // Aldric's magic resistance and Brenna's stat knocked off what the game's own code stored, as a
    // spell or an edit could: touchstone's calculation does not follow, so the comparison flags it
    let Some(state) = build_fixture_with(
        "UPDATE characters SET maxmr = maxmr + 5 WHERE firstname = 'Aldric'; \
         UPDATE characters SET str = str + 7 WHERE firstname = 'Brenna';",
    ) else {
        return;
    };
    let all = details(&state).await;
    let of = |n: &str| &all.iter().find(|(k, _)| k == n).unwrap().1;
    let mr = comparison(of("Aldric"))
        .iter()
        .find(|c| c["field"] == "max_mr")
        .unwrap();
    assert_eq!(mr["differs"], true);
    assert_eq!(
        mr["game"].as_i64().unwrap() - mr["touchstone"].as_f64().unwrap() as i64,
        5
    );
    assert!(mr["cause"].is_null());
    let st = comparison(of("Brenna"))
        .iter()
        .find(|c| c["field"] == "str")
        .unwrap();
    assert_eq!(st["differs"], true);
    // the list counts them
    let (_, list) = get(&state, "/api/realm/characters").await;
    let row = |n: &str| {
        list["characters"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["first_name"] == n)
            .unwrap()
            .clone()
    };
    assert_eq!(row("Aldric")["differs"], of("Aldric")["differs"]);
    assert_eq!(row("Gwen")["differs"], 0);
}

// ---- tests that need no cached database: invented rows through the real source code ----

fn insert(conn: &Connection, table: &str, set: &[(&str, rusqlite::types::Value)]) {
    use rusqlite::types::Value;
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
    for (col, ty) in cols {
        let v = set
            .iter()
            .find(|(n, _)| *n == col)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| {
                if ty == "TEXT" {
                    Value::Text(String::new())
                } else {
                    Value::Integer(0)
                }
            });
        if col == "id" && !set.iter().any(|(n, _)| *n == "id") {
            continue;
        }
        names.push(col);
        vals.push(v);
    }
    let marks = vec!["?"; names.len()].join(",");
    conn.execute(
        &format!("INSERT INTO {table} ({}) VALUES ({marks})", names.join(",")),
        rusqlite::params_from_iter(vals),
    )
    .unwrap();
}

fn int(v: i64) -> rusqlite::types::Value {
    rusqlite::types::Value::Integer(v)
}
fn text(v: &str) -> rusqlite::types::Value {
    rusqlite::types::Value::Text(v.to_owned())
}

const SECRETS: [&str; 6] = [
    "ARGON-HASH-SECRET",
    "mail-secret@example.invalid",
    "SUICIDE-PASSWORD-SECRET",
    "login-secret-name",
    "PROFILE-STATLINE-SECRET",
    "PROFILE-USERID-SECRET",
];

/// An account and character carrying a marker in every private column.
fn add_secret_character(conn: &Connection, id: i64, first: &str) {
    insert(
        conn,
        "accounts",
        &[
            ("id", int(id)),
            ("username", text(SECRETS[3])),
            ("password_hash", text(SECRETS[0])),
            ("email", text(SECRETS[1])),
            ("created", text("2026-01-01T00:00:00Z")),
        ],
    );
    insert(
        conn,
        "characters",
        &[
            ("id", int(id)),
            ("account_id", int(id)),
            ("userid", text(SECRETS[3])),
            ("firstname", text(first)),
            ("lastname", text("Tester")),
            ("suicpw", text(SECRETS[2])),
            ("level", int(5)),
            ("race", int(1)),
            ("class", int(1)),
            ("weapon", int(7)),
        ],
    );
    insert(
        conn,
        "character_profiles",
        &[
            ("account_id", int(id)),
            ("userid", text(SECRETS[5])),
            ("statfmt", text(SECRETS[4])),
        ],
    );
    insert(
        conn,
        "character_worn",
        &[
            ("character_id", int(id)),
            ("slot", int(0)),
            ("item", int(7)),
            ("uses", int(-1)),
        ],
    );
    insert(
        conn,
        "character_inventory",
        &[
            ("character_id", int(id)),
            ("slot", int(0)),
            ("item", int(7)),
            ("uses", int(-1)),
        ],
    );
}

#[tokio::test]
async fn nothing_private_leaves_the_server() {
    let fx = crate::common::Fixture::new();
    add_secret_character(&fx.writer, 1, "Alice");
    assert!(fx.live.poll(&fx.state).unwrap());

    let mut bodies = Vec::new();
    for uri in [
        "/api/realm/characters",
        "/api/realm/characters/1",
        "/api/realm/characters/1/input",
    ] {
        let (st, body) = get_raw(&fx.state, uri).await;
        assert_eq!(st, StatusCode::OK, "{uri}");
        bodies.push((uri, String::from_utf8_lossy(&body).into_owned()));
    }
    // the character is really there (the test would pass vacuously on an empty realm)
    assert!(bodies[0].1.contains("Alice"));
    assert!(bodies[1].1.contains("Alice"));
    for (uri, body) in &bodies {
        for secret in SECRETS {
            assert!(!body.contains(secret), "{uri} returned {secret}");
        }
        // no private field name either
        for key in ["password", "email", "suicpw", "userid", "statfmt"] {
            assert!(!body.contains(key), "{uri} has a {key} field");
        }
    }
    // and nothing else in the API carries a realm secret
    for uri in ["/api/info", "/api/characters"] {
        let (_, body) = get_raw(&fx.state, uri).await;
        let body = String::from_utf8_lossy(&body);
        for secret in SECRETS {
            assert!(!body.contains(secret), "{uri} returned {secret}");
        }
    }
}

#[tokio::test]
async fn a_character_in_a_world_without_its_class_still_lists_and_says_why_it_has_no_sheet() {
    let fx = crate::common::Fixture::new();
    add_secret_character(&fx.writer, 1, "Alice");
    fx.live.poll(&fx.state).unwrap();
    let (_, list) = get(&fx.state, "/api/realm/characters").await;
    let row = &list["characters"][0];
    assert_eq!(row["first_name"], "Alice");
    assert!(row["differs"].is_null());
    let (_, d) = get(&fx.state, "/api/realm/characters/1").await;
    assert!(d["sheet"].is_null());
    assert!(
        d["sheet_error"]
            .as_str()
            .unwrap()
            .contains("class and race")
    );
    assert_eq!(d["comparison"], serde_json::json!([]));
    // the wielded item and the worn item are the test world's weapon (item 7 has no type set, so
    // MME has no slot for it)
    assert!(!d["unmapped"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn the_roster_follows_the_live_reload() {
    let fx = crate::common::Fixture::new();
    let (_, list) = get(&fx.state, "/api/realm/characters").await;
    assert_eq!(list["available"], true);
    assert_eq!(list["characters"], serde_json::json!([]));

    add_secret_character(&fx.writer, 1, "Alice");
    assert!(fx.live.poll(&fx.state).unwrap());
    let (_, list) = get(&fx.state, "/api/realm/characters").await;
    assert_eq!(list["characters"].as_array().unwrap().len(), 1);

    // a change to the character is seen by the next reload
    fx.writer
        .execute("UPDATE characters SET level = 9 WHERE id = 1", [])
        .unwrap();
    assert!(fx.live.poll(&fx.state).unwrap());
    let (_, list) = get(&fx.state, "/api/realm/characters").await;
    assert_eq!(list["characters"][0]["level"], 9);
}

#[tokio::test]
async fn a_source_without_a_realm_says_so() {
    use touchstone::state::{Loaded, SourceInfo};
    let fx = crate::paste::fixture("paste_char");
    let world = crate::paste::char_world(&fx["header"]["data"]);
    let info = SourceInfo {
        kind: "mdb",
        path: "test".into(),
    };
    let loaded = Loaded::build(world, info, None, 1).unwrap();
    let dir = TempDir::new().unwrap().keep();
    let state = AppState::with_data_dir(loaded, dir);
    for uri in [
        "/api/realm/characters",
        "/api/realm/characters/1",
        "/api/realm/characters/1/input",
    ] {
        let (st, v) = get(&state, uri).await;
        assert_eq!(st, StatusCode::OK, "{uri}");
        assert_eq!(v["available"], false, "{uri}");
        assert!(v.get("characters").is_none());
    }
}
