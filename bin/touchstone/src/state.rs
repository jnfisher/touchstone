//! The loaded game data, swapped atomically when it changes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use arc_swap::ArcSwap;
use axum::body::Bytes;
use serde::Serialize;
use touchstone_model::World;
use touchstone_rules::Profile;
use touchstone_rules::lairs::LairCache;

use crate::settings::{self, Settings};

pub const TABLES: [&str; 8] = [
    "items", "monsters", "spells", "rooms", "shops", "classes", "races", "lairs",
];

/// One table, serialised once at load so requests only copy bytes.
pub struct TableJson {
    pub name: &'static str,
    pub body: Bytes,
    pub etag: String,
    pub rows: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceInfo {
    pub kind: &'static str,
    pub path: String,
}

/// Everything derived from one load of the data.
pub struct Loaded {
    /// Kept for the computed endpoints, which look rows up by MME record number.
    pub world: World,
    pub tables: Vec<TableJson>,
    pub info: InfoJson,
    /// MME's `LoadLairInfo` cache, built when a reference list first needs it.
    lair_cache: OnceLock<LairCache>,
    /// The computed reachability of this load (a reload starts without one).
    pub reach: crate::reach::ReachSlot,
}

#[derive(Debug, Clone, Serialize)]
pub struct InfoJson {
    pub source: SourceInfo,
    pub loaded_at: String,
    /// SQLite's `data_version` for live sources; absent for a static database.
    pub data_version: Option<i64>,
    /// How many times the data has been (re)loaded since start, counting the first.
    pub load_count: u64,
    pub version: VersionJson,
    pub counts: Vec<(String, usize)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionJson {
    pub nmr_version: String,
    pub dat_file_version: String,
    pub date: String,
    pub time: String,
    pub custom: String,
}

impl Loaded {
    pub fn build(
        world: World,
        source: SourceInfo,
        data_version: Option<i64>,
        load_count: u64,
    ) -> Result<Self, serde_json::Error> {
        let mut tables = Vec::new();
        for name in TABLES {
            let body = match name {
                "items" => serde_json::to_vec(&world.items)?,
                "monsters" => serde_json::to_vec(&world.monsters)?,
                "spells" => serde_json::to_vec(&world.spells)?,
                "rooms" => serde_json::to_vec(&world.rooms)?,
                "shops" => serde_json::to_vec(&world.shops)?,
                "classes" => serde_json::to_vec(&world.classes)?,
                "races" => serde_json::to_vec(&world.races)?,
                _ => serde_json::to_vec(&world.lairs)?,
            };
            let rows = match name {
                "items" => world.items.len(),
                "monsters" => world.monsters.len(),
                "spells" => world.spells.len(),
                "rooms" => world.rooms.len(),
                "shops" => world.shops.len(),
                "classes" => world.classes.len(),
                "races" => world.races.len(),
                _ => world.lairs.len(),
            };
            // The tag is a hash of the bytes, so it stays right across restarts (a SQLite
            // data_version restarts from a small number) and stays equal when a reload finds
            // a table unchanged.
            let etag = format!("\"{:016x}\"", fnv1a(&body));
            tables.push(TableJson {
                name,
                body: Bytes::from(body),
                etag,
                rows,
            });
        }
        let v = &world.info;
        let version = VersionJson {
            nmr_version: v.nmr_version.clone(),
            dat_file_version: v.dat_file_version.clone(),
            date: v.date.clone(),
            time: v.time.clone(),
            custom: v.custom.clone(),
        };
        let counts = tables.iter().map(|t| (t.name.to_owned(), t.rows)).collect();
        Ok(Self {
            world,
            tables,
            lair_cache: OnceLock::new(),
            reach: crate::reach::ReachSlot::default(),
            info: InfoJson {
                source,
                loaded_at: iso_now(),
                data_version,
                load_count,
                version,
                counts,
            },
        })
    }

    /// The lair cache of this load.
    pub fn lair_cache(&self) -> &LairCache {
        self.lair_cache.get_or_init(|| {
            let nmr = crate::detail::nmr_ver(&self.world.info.nmr_version);
            LairCache::load(&self.world, Profile::Stock, nmr)
        })
    }

    pub fn table(&self, name: &str) -> Option<&TableJson> {
        self.tables.iter().find(|t| t.name == name)
    }
}

/// Shared by every request; `loaded` is replaced whole on reload.
pub struct AppState {
    loaded: ArcSwap<Loaded>,
    data_dir: PathBuf,
    /// `settings.json` as last read or written; the text says why it could not be read.
    settings: Mutex<Result<Settings, String>>,
    /// Finished `/api/evaluate` lists, each tagged with the load it was built from.
    evaluate_cache: Mutex<HashMap<String, (u64, Bytes)>>,
    /// The calculated monster damage (P4-30), reset by the load number it is tagged with.
    monster_damage: crate::monster_damage::MonsterDamage,
}

/// Lists kept for instant tab switches; the world is small, so a few dozen is plenty.
const EVALUATE_CACHE_MAX: usize = 64;

impl AppState {
    pub fn new(loaded: Loaded) -> Arc<Self> {
        Self::with_data_dir(loaded, PathBuf::from("data"))
    }

    /// `data_dir` is where saved characters live (`<data_dir>/characters/`).
    pub fn with_data_dir(loaded: Loaded, data_dir: PathBuf) -> Arc<Self> {
        let settings = Mutex::new(settings::load(&data_dir));
        Arc::new(Self {
            loaded: ArcSwap::from_pointee(loaded),
            data_dir,
            settings,
            evaluate_cache: Mutex::new(HashMap::new()),
            monster_damage: crate::monster_damage::MonsterDamage::default(),
        })
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// The saved settings, or why `settings.json` cannot be used.
    pub fn settings(&self) -> Result<Settings, String> {
        self.settings
            .lock()
            .map_err(|_| "settings lock poisoned".to_owned())?
            .clone()
    }

    /// `bHideRecordNumbers`; a damaged settings file reads as the default (off).
    pub fn hide_record_numbers(&self) -> bool {
        self.settings().is_ok_and(|s| s.hide_record_numbers)
    }

    /// Writes `next` (already normalised) and makes it current. The lock is held across the
    /// write so two saves cannot land out of order. A damaged file is moved aside, not lost.
    pub fn put_settings(&self, next: Settings) -> Result<Settings, String> {
        let mut guard = self
            .settings
            .lock()
            .map_err(|_| "settings lock poisoned".to_owned())?;
        settings::store(&self.data_dir, &next, guard.is_err()).map_err(|e| e.to_string())?;
        *guard = Ok(next);
        Ok(next)
    }

    pub fn monster_damage(&self) -> &crate::monster_damage::MonsterDamage {
        &self.monster_damage
    }

    pub fn current(&self) -> Arc<Loaded> {
        self.loaded.load_full()
    }

    /// A cached list, if it was built from load number `load`.
    pub fn evaluate_cache_get(&self, key: &str, load: u64) -> Option<Bytes> {
        let cache = self.evaluate_cache.lock().ok()?;
        cache
            .get(key)
            .filter(|(l, _)| *l == load)
            .map(|(_, b)| b.clone())
    }

    pub fn evaluate_cache_put(&self, key: String, load: u64, body: Bytes) {
        if let Ok(mut cache) = self.evaluate_cache.lock() {
            cache.retain(|_, (l, _)| *l == load);
            if cache.len() >= EVALUATE_CACHE_MAX {
                cache.clear();
            }
            cache.insert(key, (load, body));
        }
    }

    pub fn swap(&self, loaded: Loaded) {
        self.loaded.store(Arc::new(loaded));
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn iso_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    iso_from_unix(secs)
}

/// UTC `YYYY-MM-DDTHH:MM:SSZ` from Unix seconds (days-to-civil, proleptic Gregorian).
fn iso_from_unix(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::iso_from_unix;

    #[test]
    fn iso_dates() {
        assert_eq!(iso_from_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_from_unix(951_782_400 + 3661), "2000-02-29T01:01:01Z");
        assert_eq!(iso_from_unix(1_700_000_000), "2023-11-14T22:13:20Z");
    }
}
