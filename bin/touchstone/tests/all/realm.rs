// Realm characters (card R-001): the endpoints over the fixture characters, applied to a copy of the
// cached Quicksilver 1.20 database, and over small invented realms that need no cache.
use std::path::PathBuf;
use std::sync::Arc;

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

struct Realm {
    _dir: TempDir,
    state: Arc<AppState>,
}

/// A copy of the Quicksilver database with the fixture characters applied, started through the
/// real source code.
fn fixture_realm() -> Option<Realm> {
    let src = qs_path()?;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("realm.db");
    {
        let conn = Connection::open_with_flags(&src, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        // VACUUM INTO folds the WAL in, so the copy is whole whatever the cache's state
        conn.execute("VACUUM INTO ?1", [path.to_str().unwrap()])
            .unwrap();
    }
    let sql = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/quicksilver/realm_characters.sql"),
    )
    .unwrap();
    Connection::open(&path).unwrap().execute_batch(&sql).unwrap();
    let cfg = Config {
        source: Some(config_for(&path)),
        data_dir: dir.path().join("data"),
        ..Config::default()
    };
    let started = source::start(&cfg).unwrap();
    Some(Realm {
        _dir: dir,
        state: started.state,
    })
}

async fn get(state: &Arc<AppState>, uri: &str) -> (StatusCode, Value) {
    let res = touchstone::app(state.clone())
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn explore() {
    let Some(r) = fixture_realm() else { return };
    let (st, list) = get(&r.state, "/api/realm/characters").await;
    assert_eq!(st, StatusCode::OK);
    for c in list["characters"].as_array().unwrap() {
        let id = c["id"].as_i64().unwrap();
        let (_, d) = get(&r.state, &format!("/api/realm/characters/{id}")).await;
        println!("== {} {} {} L{} differs {}", c["first_name"], c["race_name"], c["class_name"], c["level"], d["differs"]);
        if let Some(e) = d["sheet_error"].as_str() { println!("  sheet error: {e}"); }
        for x in d["comparison"].as_array().unwrap() {
            if x["differs"] == true {
                println!("  {:<12} game {} ts {} range {}", x["field"], x["game"], x["touchstone"], x["range"]);
            }
        }
        for u in d["unmapped"].as_array().unwrap() { println!("  unmapped {u}"); }
    }
}
