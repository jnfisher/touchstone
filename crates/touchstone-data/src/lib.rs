//! Loaders that build a `World`: from a converted MME `.mdb` (SQLite produced by mdbtools) and
//! from a silvermere-schema SQLite database, read-only and live.

pub mod mdb;
pub mod silvermere;
