//! Which rules a load follows: stock MME, or Quicksilver with a balance.
//!
//! The choice comes from the structured content version of the database
//! (`Info::content_version`), never from the display caption, and from `[rules]` in
//! `touchstone.toml`. Design: `docs/quicksilver-profile.md` part 3.

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use touchstone_model::Info;
use touchstone_rules::Profile;
use touchstone_rules::quicksilver::{Balance, V120_BALANCE};

/// `[rules]`.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct RulesConfig {
    pub profile: ProfileSetting,
    /// Only the keys that differ from the version's balance; Quicksilver's own `[balance]` key
    /// names, so its table can be pasted unchanged. Unknown keys fail startup.
    pub quicksilver_balance: Option<Balance>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProfileSetting {
    /// Decide from the database's content version.
    #[default]
    Auto,
    /// Stock rules whatever the database is, for comparisons.
    Stock,
    /// The 1.20 rules whatever the database is.
    Quicksilver,
}

/// The content versions this build has rules for. A retune is a new content version and a new
/// `Balance` constant, so this is where a new version is added.
const KNOWN_FAMILY: &str = "1.20.N";

/// Whether `v` is `1.20.N` with N one or more digits. Quicksilver's 1.20.1 still reports 1.20.0,
/// but any patch number of the 1.20 line carries the same rules.
#[must_use]
pub fn is_v120(v: &str) -> bool {
    v.strip_prefix("1.20.")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// What `/api/info` says about the rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileJson {
    /// `"stock"` or `"quicksilver"`.
    pub name: &'static str,
    /// The `[rules] profile` setting that produced it.
    pub setting: ProfileSetting,
    /// The database's structured content version, when it has one.
    pub content_version: Option<String>,
    /// Quicksilver only: the balance keys whose value differs from the frozen 1.20.0 one (the
    /// config override at work), sorted. Empty for stock and for an unmodified balance.
    pub balance_overrides: Vec<String>,
}

/// The validated `[rules]` section, ready to pick a profile for each load.
#[derive(Debug, Clone, Copy)]
pub struct ProfileChoice {
    setting: ProfileSetting,
    balance: &'static Balance,
}

impl Default for ProfileChoice {
    fn default() -> Self {
        Self {
            setting: ProfileSetting::Auto,
            balance: &V120_BALANCE,
        }
    }
}

impl ProfileChoice {
    /// Validates the override and, if it differs from the 1.20.0 values, leaks it: one small
    /// allocation per process, because this runs once at startup and a live reload reuses it.
    ///
    /// # Errors
    /// The override fails `Balance::validate`.
    pub fn new(cfg: &RulesConfig) -> anyhow::Result<Self> {
        let balance = match &cfg.quicksilver_balance {
            Some(b) => {
                b.validate()
                    .map_err(|e| anyhow::anyhow!("{e}"))
                    .context("[rules.quicksilver_balance]")?;
                if b.is_v120() {
                    &V120_BALANCE
                } else {
                    Box::leak(Box::new(*b))
                }
            }
            None => &V120_BALANCE,
        };
        Ok(Self {
            setting: cfg.profile,
            balance,
        })
    }

    /// The profile for a freshly loaded world.
    ///
    /// # Errors
    /// The database carries a content version this build has no rules for (unless the setting is
    /// `stock`, which exists to ignore the data's rules).
    pub fn choose(&self, info: &Info) -> anyhow::Result<(Profile, ProfileJson)> {
        let version = info.content_version.as_deref();
        let quicksilver = match self.setting {
            ProfileSetting::Stock => false,
            ProfileSetting::Auto | ProfileSetting::Quicksilver => {
                if let Some(v) = version {
                    if !is_v120(v) {
                        bail!(
                            "this Quicksilver database has content version {v:?}, and this \
                             touchstone only knows the {KNOWN_FAMILY} rules; update touchstone, \
                             or set [rules] profile = \"stock\" to ignore Quicksilver's rules"
                        );
                    }
                    true
                } else {
                    self.setting == ProfileSetting::Quicksilver
                }
            }
        };
        let json = |name, balance_overrides| ProfileJson {
            name,
            setting: self.setting,
            content_version: version.map(str::to_owned),
            balance_overrides,
        };
        Ok(if quicksilver {
            (
                Profile::Quicksilver(self.balance),
                json("quicksilver", override_keys(self.balance)),
            )
        } else {
            (Profile::Stock, json("stock", Vec::new()))
        })
    }
}

/// The keys of `b` that are not the 1.20.0 value, by Quicksilver's names.
fn override_keys(b: &Balance) -> Vec<String> {
    if b.is_v120() {
        return Vec::new();
    }
    let (Ok(now), Ok(base)) = (serde_json::to_value(b), serde_json::to_value(Balance::V120)) else {
        return Vec::new();
    };
    let (Some(now), Some(base)) = (now.as_object(), base.as_object()) else {
        return Vec::new();
    };
    let mut keys: Vec<String> = now
        .iter()
        .filter(|(k, v)| base.get(*k) != Some(v))
        .map(|(k, _)| k.clone())
        .collect();
    keys.sort();
    keys
}

/// One stat's limits for a race.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct StatCap {
    /// The race maximum (or its minimum, if that is higher).
    pub race_max: i32,
    /// Where training leaves the plain price: `race_max`, or under Quicksilver the larger of that and the soft cap
    /// floor (R14).
    pub soft_cap: i32,
    /// The highest value training reaches.
    pub train_max: i32,
}

/// What `GET /api/races/{number}/stat-caps` answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StatCapsJson {
    /// Whether this load prices stats with soft caps (Quicksilver). Stock does not, and `stats` is then empty:
    /// MME's boxes take any value up to three digits.
    pub soft_caps: bool,
    /// Strength, Intellect, Willpower, Agility, Health, Charm, as the Char tab lists them.
    pub stats: Vec<StatCap>,
}

/// The stat limits of `race` under `profile`.
#[must_use]
pub fn stat_caps(profile: Profile, race: &touchstone_model::Race) -> StatCapsJson {
    use touchstone_rules::stats::{soft_cap, train_max};
    let maxes = [
        (race.m_str, race.x_str),
        (race.m_int, race.x_int),
        (race.m_wil, race.x_wil),
        (race.m_agl, race.x_agl),
        (race.m_hea, race.x_hea),
        (race.m_chm, race.x_chm),
    ];
    let stats: Vec<StatCap> = maxes
        .iter()
        .filter_map(|&(min, max)| {
            let race_max = max.max(min);
            train_max(profile, race_max).map(|t| StatCap {
                race_max,
                soft_cap: soft_cap(profile, race_max),
                train_max: t,
            })
        })
        .collect();
    StatCapsJson {
        soft_caps: !stats.is_empty(),
        stats,
    }
}

/// `GET /api/races/{number}/stat-caps`.
pub async fn stat_caps_route(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<crate::state::AppState>>,
    axum::extract::Path(number): axum::extract::Path<i32>,
) -> crate::calc::ApiResult {
    let loaded = state.current();
    let race = loaded
        .world
        .get_race(number)
        .ok_or_else(|| crate::calc::not_found(format!("no race {number}")))?;
    crate::calc::json(&stat_caps(loaded.profile, race))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(v: Option<&str>) -> Info {
        Info {
            nmr_version: String::new(),
            dat_file_version: v.unwrap_or("x").to_owned(),
            content_version: v.map(str::to_owned),
            date: String::new(),
            time: String::new(),
            custom: String::new(),
            legit: true,
            disable_kai: false,
            update_url: String::new(),
        }
    }

    fn cfg(s: &str) -> RulesConfig {
        toml::from_str(s).unwrap()
    }

    #[test]
    fn v120_family_is_one_twenty_dot_digits() {
        for ok in ["1.20.0", "1.20.1", "1.20.17"] {
            assert!(is_v120(ok), "{ok}");
        }
        for bad in [
            "",
            "1.20",
            "1.20.",
            "1.20.x",
            "1.21.0",
            "1.2.0",
            "2.20.0",
            "1.20.0-test",
            " 1.20.0",
            "silvermere",
            "v1.11p",
            "01.20.0",
        ] {
            assert!(!is_v120(bad), "{bad:?}");
        }
    }

    #[test]
    fn auto_follows_the_content_version() {
        let c = ProfileChoice::new(&RulesConfig::default()).unwrap();
        let (p, j) = c.choose(&info(None)).unwrap();
        assert_eq!((p, j.name), (Profile::Stock, "stock"));
        let (p, j) = c.choose(&info(Some("1.20.0"))).unwrap();
        assert_eq!(p, Profile::quicksilver_v120());
        assert_eq!(j.name, "quicksilver");
        assert_eq!(j.content_version.as_deref(), Some("1.20.0"));
        assert!(j.balance_overrides.is_empty());
        let (p, _) = c.choose(&info(Some("1.20.1"))).unwrap();
        assert_eq!(p, Profile::quicksilver_v120());
    }

    #[test]
    fn unknown_versions_refuse_with_the_version_named() {
        let c = ProfileChoice::new(&RulesConfig::default()).unwrap();
        for v in ["1.21.0", "1.20.0-test", "2.0.0"] {
            let e = c.choose(&info(Some(v))).unwrap_err().to_string();
            assert!(e.contains(v) && e.contains("1.20.N"), "{e}");
        }
        let q = ProfileChoice::new(&cfg("profile = \"quicksilver\"")).unwrap();
        assert!(q.choose(&info(Some("1.21.0"))).is_err());
    }

    #[test]
    fn stock_setting_ignores_the_data() {
        let c = ProfileChoice::new(&cfg("profile = \"stock\"")).unwrap();
        for v in [None, Some("1.20.0"), Some("9.9.9")] {
            let (p, j) = c.choose(&info(v)).unwrap();
            assert_eq!(p, Profile::Stock);
            assert_eq!(j.setting, ProfileSetting::Stock);
            assert_eq!(j.content_version.as_deref(), v);
        }
    }

    #[test]
    fn quicksilver_setting_forces_the_1_20_rules_on_stock_data() {
        let c = ProfileChoice::new(&cfg("profile = \"quicksilver\"")).unwrap();
        let (p, j) = c.choose(&info(None)).unwrap();
        assert_eq!(p, Profile::quicksilver_v120());
        assert_eq!(j.content_version, None);
    }

    #[test]
    fn an_override_is_applied_reported_and_leaked_once() {
        let c = ProfileChoice::new(&cfg(
            "[quicksilver_balance]\ncrit_knee = 55\nscalecap = 40\n",
        ))
        .unwrap();
        let (p, j) = c.choose(&info(Some("1.20.0"))).unwrap();
        let b = p.balance().unwrap();
        assert_eq!((b.crit_knee, b.scalecap), (55, 40));
        assert_eq!(b.weave_pct, Balance::V120.weave_pct);
        assert_eq!(j.balance_overrides, ["crit_knee", "scalecap"]);
        // A second load shares the same allocation.
        let (p2, _) = c.choose(&info(Some("1.20.0"))).unwrap();
        assert!(std::ptr::eq(b, p2.balance().unwrap()));
        // Stock data under auto ignores it.
        assert_eq!(c.choose(&info(None)).unwrap().0, Profile::Stock);
    }

    #[test]
    fn an_override_equal_to_the_defaults_is_not_one() {
        let c = ProfileChoice::new(&cfg("[quicksilver_balance]\n")).unwrap();
        let (p, j) = c.choose(&info(Some("1.20.0"))).unwrap();
        assert_eq!(p, Profile::quicksilver_v120());
        assert!(j.balance_overrides.is_empty());
    }

    #[test]
    fn bad_config_fails_startup() {
        assert!(toml::from_str::<RulesConfig>("bogus = 1").is_err());
        assert!(toml::from_str::<RulesConfig>("profile = \"greatermud\"").is_err());
        assert!(toml::from_str::<RulesConfig>("[quicksilver_balance]\nnot_a_key = 1").is_err());
        let e =
            ProfileChoice::new(&cfg("[quicksilver_balance]\naoe_resist_cut_div = 0")).unwrap_err();
        assert!(format!("{e:#}").contains("aoe_resist_cut_div"), "{e:#}");
    }
}
