use touchstone_model::ModelError;

/// Why a silvermere/Quicksilver database could not be opened or read.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("not a silvermere database: no _sqlx_migrations table (or it has no rows)")]
    NoMigrations,
    #[error("migration {0} did not finish applying (success = 0)")]
    MigrationFailed(i64),
    #[error(
        "unsupported schema: database has migration version(s) {found:?}, this build supports {supported:?}"
    )]
    UnsupportedMigrations {
        found: Vec<i64>,
        supported: Vec<i64>,
    },
    #[error("the last world import did not finish (world_import.finished_at is NULL)")]
    ImportIncomplete,
    #[error("{table}.{column} value {value} does not fit the model")]
    OutOfRange {
        table: &'static str,
        column: &'static str,
        value: i64,
    },
    #[error(transparent)]
    Model(#[from] ModelError),
}

pub type Result<T> = std::result::Result<T, Error>;
