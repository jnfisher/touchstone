//! A temp silvermere database with invented rows, started through the real source code.
use std::path::PathBuf;
use std::sync::Arc;

use rusqlite::{Connection, types::Value};
use tempfile::TempDir;
use touchstone::config::{Config, SourceConfig, SourceKind};
use touchstone::source::{self, LiveSource};
use touchstone::{AppState, app};

const MIGRATIONS: [&str; 2] = [
    include_str!("../../../../crates/touchstone-data/vendor/silvermere-migrations/0001_schema.sql"),
    include_str!(
        "../../../../crates/touchstone-data/vendor/silvermere-migrations/0002_world_import.sql"
    ),
];

pub struct Fixture {
    pub dir: TempDir,
    pub path: PathBuf,
    pub writer: Connection,
    pub state: Arc<AppState>,
    pub live: Arc<LiveSource>,
}

impl Fixture {
    pub fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("world.db");
        let writer = Connection::open(&path).unwrap();
        writer.pragma_update(None, "journal_mode", "WAL").unwrap();
        for sql in MIGRATIONS {
            writer.execute_batch(sql).unwrap();
        }
        writer
            .execute_batch(
                "CREATE TABLE _sqlx_migrations (version BIGINT PRIMARY KEY, description TEXT NOT NULL, \
                 installed_on TEXT NOT NULL DEFAULT '', success BOOLEAN NOT NULL, \
                 checksum BLOB NOT NULL DEFAULT x'', execution_time BIGINT NOT NULL DEFAULT 0); \
                 INSERT INTO _sqlx_migrations (version, description, success) VALUES (1,'m',1),(2,'m',1);",
            )
            .unwrap();
        add_item(&writer, 7, "test blade");
        let cfg = Config {
            source: Some(config_for(&path)),
            ..Config::default()
        };
        let started = source::start(&cfg).unwrap();
        Self {
            dir,
            path,
            writer,
            state: started.state,
            live: started.live.unwrap(),
        }
    }

    pub fn app(&self) -> axum::Router {
        app(self.state.clone())
    }
}

pub fn config_for(path: &std::path::Path) -> SourceConfig {
    SourceConfig {
        kind: SourceKind::Silvermere,
        path: path.to_owned(),
        reload_interval_secs: 10,
        converter: None,
    }
}

/// Insert an item, giving every column not named a zero or empty string (no schema defaults).
pub fn add_item(conn: &Connection, number: i64, name: &str) {
    let mut stmt = conn
        .prepare("SELECT name, type FROM pragma_table_info('items')")
        .unwrap();
    let cols: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut names = Vec::new();
    let mut vals = Vec::new();
    for (col, ty) in cols {
        let v = match col.as_str() {
            "id" => continue,
            "number" => Value::Integer(number),
            "name" => Value::Text(name.to_owned()),
            _ if ty == "TEXT" => Value::Text(String::new()),
            _ => Value::Integer(0),
        };
        names.push(col);
        vals.push(v);
    }
    let marks = vec!["?"; names.len()].join(",");
    conn.execute(
        &format!("INSERT INTO items ({}) VALUES ({marks})", names.join(",")),
        rusqlite::params_from_iter(vals),
    )
    .unwrap();
}
