//! Loading the World from the configured source, and keeping a live source fresh.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, bail};
use touchstone_data::silvermere::{self, Silvermere};

use crate::config::{Config, SourceConfig, SourceKind};
use crate::profile::ProfileChoice;
use crate::state::{AppState, Loaded, SourceInfo};

/// A live silvermere database plus what is needed to rebuild from it.
pub struct LiveSource {
    handle: Mutex<Silvermere>,
    seen_version: AtomicI64,
    loads: AtomicU64,
    info: SourceInfo,
    rules: ProfileChoice,
}

pub struct Started {
    pub state: Arc<AppState>,
    /// Present for a silvermere source; drive it with [`spawn_reload`].
    pub live: Option<Arc<LiveSource>>,
}

/// Load the configured source once. Blocking: call before serving, or from `spawn_blocking`.
pub fn start(cfg: &Config) -> anyhow::Result<Started> {
    let Some(src) = &cfg.source else {
        bail!(
            "no [source] in the config: set kind = \"mdb\" or \"silvermere\" and a path \
             (see packaging/touchstone.example.toml)"
        );
    };
    let rules = ProfileChoice::new(&cfg.rules)?;
    match src.kind {
        SourceKind::Mdb => {
            let sqlite = resolve_mdb(src, &cfg.data_dir)?;
            let world = touchstone_data::mdb::load(&sqlite)
                .with_context(|| format!("loading {}", sqlite.display()))?;
            let info = SourceInfo {
                kind: "mdb",
                path: src.path.display().to_string(),
            };
            let loaded = Loaded::build_with(world, info, None, 1, &rules)?;
            Ok(Started {
                state: AppState::with_data_dir(loaded, cfg.data_dir.clone()),
                live: None,
            })
        }
        SourceKind::Silvermere => {
            let handle = silvermere::open(&src.path)
                .with_context(|| format!("opening silvermere database {}", src.path.display()))?;
            let version = handle.data_version()?;
            let (world, realm) = handle.load_snapshot()?;
            let info = SourceInfo {
                kind: "silvermere",
                path: src.path.display().to_string(),
            };
            let loaded = Loaded::build_with(world, info.clone(), Some(version), 1, &rules)?
                .with_realm(realm);
            Ok(Started {
                state: AppState::with_data_dir(loaded, cfg.data_dir.clone()),
                live: Some(Arc::new(LiveSource {
                    handle: Mutex::new(handle),
                    seen_version: AtomicI64::new(version),
                    loads: AtomicU64::new(1),
                    info,
                    rules,
                })),
            })
        }
    }
}

impl LiveSource {
    /// Check `data_version` and, if the game committed since the last look, rebuild the World
    /// and swap it in. Returns whether it swapped. On any error the old World stays. Blocking.
    pub fn poll(&self, state: &AppState) -> anyhow::Result<bool> {
        let handle = self
            .handle
            .lock()
            .map_err(|_| anyhow::anyhow!("poisoned"))?;
        let version = handle.data_version()?;
        if version == self.seen_version.load(Ordering::Relaxed) {
            return Ok(false);
        }
        let (world, realm) = handle.load_snapshot()?;
        let n = self.loads.load(Ordering::Relaxed) + 1;
        let loaded =
            match Loaded::build_with(world, self.info.clone(), Some(version), n, &self.rules) {
                Ok(l) => l.with_realm(realm),
                Err(e) => {
                    // The game will not change this content under us; asking again every poll
                    // would only repeat the message. A later commit is looked at afresh.
                    self.seen_version.store(version, Ordering::Relaxed);
                    return Err(e.context("keeping the previous game data"));
                }
            };
        state.swap(loaded);
        // Only after the swap: a failed attempt is retried on the next poll.
        self.seen_version.store(version, Ordering::Relaxed);
        self.loads.store(n, Ordering::Relaxed);
        Ok(true)
    }
}

/// Poll every `every` on a background task until the process ends.
pub fn spawn_reload(live: Arc<LiveSource>, state: Arc<AppState>, every: Duration) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(every);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        tick.tick().await; // the first tick is immediate; the data was just loaded
        loop {
            tick.tick().await;
            let (l, s) = (live.clone(), state.clone());
            match tokio::task::spawn_blocking(move || l.poll(&s)).await {
                Ok(Ok(true)) => eprintln!("touchstone: game data changed, reloaded"),
                Ok(Ok(false)) => {}
                Ok(Err(e)) => eprintln!("touchstone: reload failed, keeping old data: {e:#}"),
                Err(e) => eprintln!("touchstone: reload task failed: {e}"),
            }
        }
    });
}

pub fn reload_interval(src: &SourceConfig) -> Duration {
    Duration::from_secs(src.reload_interval_secs.max(1))
}

/// The SQLite file to load for an `mdb` source: the path itself when it is already converted,
/// or a conversion of the `.mdb` kept under `<data_dir>/converted/`.
fn resolve_mdb(src: &SourceConfig, data_dir: &Path) -> anyhow::Result<PathBuf> {
    let ext = src
        .path
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("sqlite" | "sqlite3" | "db") => Ok(src.path.clone()),
        Some("mdb") => convert_mdb(src, data_dir),
        _ => bail!(
            "mdb source {}: expected a .mdb or an already converted .sqlite file",
            src.path.display()
        ),
    }
}

fn convert_mdb(src: &SourceConfig, data_dir: &Path) -> anyhow::Result<PathBuf> {
    let stem = src.path.file_stem().unwrap_or(OsStr::new("data"));
    let out = data_dir
        .join("converted")
        .join(Path::new(stem).with_extension("sqlite"));
    let fresh = match (mtime(&src.path), mtime(&out)) {
        (Some(a), Some(b)) => b >= a,
        _ => false,
    };
    if fresh {
        return Ok(out);
    }
    if !src.path.is_file() {
        bail!("mdb source {} does not exist", src.path.display());
    }
    let script = find_converter(src.converter.as_deref()).context(
        "converting a .mdb needs tools/mdb2sqlite.py (set source.converter), or point \
         source.path at an already converted .sqlite (tools/mdb-fetch.sh makes one)",
    )?;
    std::fs::create_dir_all(out.parent().unwrap_or(data_dir))?;
    eprintln!(
        "touchstone: converting {} with mdbtools",
        src.path.display()
    );
    let status = Command::new("python3")
        .arg("-I")
        .arg(&script)
        .arg(&src.path)
        .arg(&out)
        .status()
        .context("running python3 (needed to convert a .mdb)")?;
    if !status.success() {
        bail!(
            "converting {} failed (is mdbtools installed? apt install mdbtools)",
            src.path.display()
        );
    }
    Ok(out)
}

fn mtime(p: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn find_converter(configured: Option<&Path>) -> anyhow::Result<PathBuf> {
    if let Some(p) = configured {
        if p.is_file() {
            return Ok(p.to_owned());
        }
        bail!("source.converter {} does not exist", p.display());
    }
    let mut dirs = Vec::new();
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_owned))
    {
        dirs.push(dir.clone());
        if let Some(up) = dir.parent() {
            dirs.push(up.to_owned());
        }
    }
    dirs.push(PathBuf::from("."));
    dirs.iter()
        .map(|d| d.join("tools").join("mdb2sqlite.py"))
        .find(|p| p.is_file())
        .ok_or_else(|| anyhow::anyhow!("tools/mdb2sqlite.py not found"))
}
