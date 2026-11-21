//! `touchstone.toml`.

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;

use crate::profile::RulesConfig;

pub const DEFAULT_BIND: &str = "0.0.0.0:8370";

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub bind: String,
    pub data_dir: PathBuf,
    /// Required to serve; optional here so an empty file still parses.
    pub source: Option<SourceConfig>,
    /// Which game rules to follow (`[rules]`).
    pub rules: RulesConfig,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    /// An MME database: a converted `.sqlite`, or a `.mdb` converted on start.
    Mdb,
    /// A running silvermere/Quicksilver server's SQLite file, read live.
    Silvermere,
}

impl SourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mdb => "mdb",
            Self::Silvermere => "silvermere",
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceConfig {
    pub kind: SourceKind,
    pub path: PathBuf,
    /// No longer read: a live source reloads only when asked (`POST /api/reload`). Still
    /// accepted so a config written for an older release starts; the server warns that it is set.
    #[serde(default)]
    pub reload_interval_secs: Option<u64>,
    /// Mdb only: path to `mdb2sqlite.py` when it is not next to the binary or in `./tools`.
    pub converter: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: DEFAULT_BIND.to_owned(),
            data_dir: PathBuf::from("data"),
            source: None,
            rules: RulesConfig::default(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading config {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing config {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Config {
        toml::from_str(s).unwrap()
    }

    #[test]
    fn empty_file_gives_defaults() {
        let c = parse("");
        assert_eq!(c.bind, "0.0.0.0:8370");
        assert_eq!(c.data_dir, PathBuf::from("data"));
        assert!(c.source.is_none());
        assert_eq!(c.rules, RulesConfig::default());
    }

    #[test]
    fn rules_section_parses() {
        use crate::profile::ProfileSetting;
        let c =
            parse("[rules]\nprofile = \"stock\"\n[rules.quicksilver_balance]\ncrit_knee = 50\n");
        assert_eq!(c.rules.profile, ProfileSetting::Stock);
        assert_eq!(c.rules.quicksilver_balance.unwrap().crit_knee, 50);
        assert!(toml::from_str::<Config>("[rules]\nprofile = \"x\"\n").is_err());
        assert!(toml::from_str::<Config>("[rules]\nbogus = 1\n").is_err());
    }

    #[test]
    fn source_parses_and_still_accepts_the_old_interval() {
        let c =
            parse("bind = \"127.0.0.1:1\"\n[source]\nkind = \"silvermere\"\npath = \"/g/w.db\"\n");
        let s = c.source.unwrap();
        assert_eq!(c.bind, "127.0.0.1:1");
        assert_eq!(s.kind, SourceKind::Silvermere);
        assert_eq!(s.reload_interval_secs, None);
        let old =
            parse("[source]\nkind = \"silvermere\"\npath = \"p\"\nreload_interval_secs = 10\n");
        assert_eq!(old.source.unwrap().reload_interval_secs, Some(10));
    }

    #[test]
    fn unknown_kind_and_keys_are_rejected() {
        assert!(toml::from_str::<Config>("[source]\nkind = \"x\"\npath = \"p\"\n").is_err());
        assert!(toml::from_str::<Config>("[source]\nkind = \"mdb\"\n").is_err());
        assert!(toml::from_str::<Config>("[source]\nkind=\"mdb\"\npath=\"p\"\nbogus=1\n").is_err());
    }
}
