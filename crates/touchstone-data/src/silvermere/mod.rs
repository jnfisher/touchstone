//! Reads a live silvermere/Quicksilver SQLite database, read-only, into a `World` (T-012).
//!
//! The game keeps the database in WAL mode, so a read-only connection never blocks it. The
//! mapping from silvermere's normalised tables to MME's flat ones, and what has no source, is
//! in `docs/data-gaps.md`.
mod error;
mod load;
mod realm;

use std::path::Path;

use rusqlite::{Connection, OpenFlags};
use touchstone_model::World;

pub use error::{Error, Result};
pub use realm::{CastSpell, Realm, RealmCharacter, Slotted};

/// Migration versions this build reads: silvermere's two, and Quicksilver's 9001
/// (`content_version` on `world_import`). A database may hold any prefix-closed subset that
/// includes version 1 and 2; anything else is refused.
pub const SUPPORTED_MIGRATIONS: &[i64] = &[1, 2, 9001];

/// A held read-only connection to a running server's database.
#[derive(Debug)]
pub struct Silvermere {
    conn: Connection,
}

/// Open the database at `path` read-only and check its schema version.
pub fn open(path: impl AsRef<Path>) -> Result<Silvermere> {
    let uri = format!("file:{}?mode=ro", encode_path(path.as_ref()));
    let conn = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    // A writer holding its lock for a moment must not fail a reload; WAL readers rarely wait.
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    check_migrations(&conn)?;
    Ok(Silvermere { conn })
}

impl Silvermere {
    /// Build a `World` from the current committed contents, from one consistent snapshot.
    pub fn load_world(&self) -> Result<World> {
        self.conn.execute_batch("BEGIN")?;
        let world = load::load(&self.conn);
        // A read-only transaction: COMMIT only ends the snapshot.
        self.conn.execute_batch("COMMIT")?;
        world
    }

    /// The world and the realm's characters from one consistent snapshot, so a character never
    /// names an item the world has not got yet.
    pub fn load_snapshot(&self) -> Result<(World, Realm)> {
        self.conn.execute_batch("BEGIN")?;
        let both = load::load(&self.conn).and_then(|w| Ok((w, realm::load(&self.conn)?)));
        self.conn.execute_batch("COMMIT")?;
        both
    }

    /// SQLite's `data_version` for this connection: it changes when another connection commits
    /// to the database, so the server's reload loop compares it with the value it last saw.
    pub fn data_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("PRAGMA data_version", [], |r| r.get(0))?)
    }
}

fn check_migrations(conn: &Connection) -> Result<()> {
    let exists: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
        [],
        |r| r.get(0),
    )?;
    if exists == 0 {
        return Err(Error::NoMigrations);
    }
    let mut stmt =
        conn.prepare("SELECT version, success FROM _sqlx_migrations ORDER BY version")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if rows.is_empty() {
        return Err(Error::NoMigrations);
    }
    let found: Vec<i64> = rows.iter().map(|r| r.0).collect();
    let unknown = found.iter().any(|v| !SUPPORTED_MIGRATIONS.contains(v));
    // Both base migrations are needed: the loader reads tables from each.
    let missing_base = !(found.contains(&1) && found.contains(&2));
    if unknown || missing_base {
        return Err(Error::UnsupportedMigrations {
            found,
            supported: SUPPORTED_MIGRATIONS.to_vec(),
        });
    }
    if let Some((v, _)) = rows.iter().find(|r| !r.1) {
        return Err(Error::MigrationFailed(*v));
    }
    Ok(())
}

/// Percent-encode the characters that are special in a SQLite URI path.
fn encode_path(path: &Path) -> String {
    let mut out = String::new();
    for b in path.to_string_lossy().bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~:\\".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
