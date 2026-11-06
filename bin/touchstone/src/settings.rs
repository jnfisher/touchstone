//! The options that change computed results: `GET/PUT /api/settings`, stored as
//! `<data_dir>/settings.json`. These are the `settings.ini` keys MME's Settings window edits that
//! reach a calculation (see `tasks/P4-29-settings.md` for the key-by-key table); view options stay
//! in the browser.
//!
//! `PUT` applies the rules MME's Save button applies instead of rejecting out-of-range numbers, and
//! answers with what it stored, so the page can show the clamped value.

use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};
use touchstone_rules::exp_per_hour::{ExpPerHourKnobs, ExpPerHourSettings, clamp_knob};

use crate::calc::{ApiResult, Body, bad, json};
use crate::character::CharacterDto;
use crate::state::AppState;

pub const FILE_NAME: &str = "settings.json";
/// `nGlobalMonsterSimRounds` is kept between these on load and on save, 500 until set.
pub const SIM_ROUNDS_MIN: i32 = 100;
pub const SIM_ROUNDS_MAX: i32 = 10_000;
pub const SIM_ROUNDS_DEFAULT: i32 = 500;
/// `nRouteWaypointInterval`: blank or 0 means 25, above 1000 is 1000.
pub const WAYPOINT_MAX: i32 = 1000;
pub const WAYPOINT_DEFAULT: i32 = 25;

/// The exp/hr block: the four models, the two display switches and the four knobs. The same shape
/// is stored per character (`CharacterDto.exp_hr`), as MME's `cephModel*`/`cephDMG` keys can live
/// in the character file.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ExpHrBlock {
    pub model_a: bool,
    pub model_b: bool,
    pub model_c: bool,
    pub model_d: bool,
    /// `cephRecovery`: leave out HP recovery detail (basic damage only).
    pub recovery_only: bool,
    /// `cephShowAll`: list each model's figure beside the average.
    pub show_all: bool,
    pub knob_xp: f64,
    pub knob_dmg: f64,
    pub knob_mana: f64,
    pub knob_move: f64,
}

impl Default for ExpHrBlock {
    /// What MME runs with when settings.ini has no `ceph*` keys (all four models on).
    fn default() -> Self {
        ExpPerHourSettings::default().into()
    }
}

impl From<ExpPerHourSettings> for ExpHrBlock {
    fn from(s: ExpPerHourSettings) -> Self {
        Self {
            model_a: s.model_a,
            model_b: s.model_b,
            model_c: s.model_c,
            model_d: s.model_d,
            recovery_only: s.recovery_only,
            show_all: s.show_all,
            knob_xp: s.knobs.xp,
            knob_dmg: s.knobs.dmg,
            knob_mana: s.knobs.mana,
            knob_move: s.knobs.mv,
        }
    }
}

impl ExpHrBlock {
    /// The save rules of `cmdSave_Click`: knobs rounded and clamped, A, B and C turned on when no
    /// model is. A knob that is not a finite number cannot come from JSON, but is treated as 1.
    #[must_use]
    pub fn normalized(self) -> Self {
        let knob = |v: f64| if v.is_finite() { clamp_knob(v) } else { 1.0 };
        let mut s = self.to_rules();
        s.knobs = ExpPerHourKnobs {
            xp: knob(self.knob_xp),
            dmg: knob(self.knob_dmg),
            mana: knob(self.knob_mana),
            mv: knob(self.knob_move),
        };
        s.normalized().into()
    }

    /// The argument `calc_exp_per_hour` takes. Call `normalized` first for stored values.
    #[must_use]
    pub fn to_rules(self) -> ExpPerHourSettings {
        ExpPerHourSettings {
            model_a: self.model_a,
            model_b: self.model_b,
            model_c: self.model_c,
            model_d: self.model_d,
            recovery_only: self.recovery_only,
            show_all: self.show_all,
            knobs: ExpPerHourKnobs {
                xp: self.knob_xp,
                dmg: self.knob_dmg,
                mana: self.knob_mana,
                mv: self.knob_move,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ExpHrSettings {
    /// The block used for every character without its own (or with "by character" off).
    pub global: ExpHrBlock,
    /// `ExpPerHourKnobsByCharacter`: the open character's own block wins over the global one.
    pub by_character: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub exp_per_hour: ExpHrSettings,
    pub monster_sim_rounds: i32,
    pub route_waypoint_interval: i32,
    /// `AutoLoadLastChar`: open the last character when the page starts.
    pub auto_load_last_char: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            exp_per_hour: ExpHrSettings::default(),
            monster_sim_rounds: SIM_ROUNDS_DEFAULT,
            route_waypoint_interval: WAYPOINT_DEFAULT,
            auto_load_last_char: true,
        }
    }
}

impl Settings {
    /// What `cmdSave_Click` would keep of these values.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.monster_sim_rounds = self
            .monster_sim_rounds
            .clamp(SIM_ROUNDS_MIN, SIM_ROUNDS_MAX);
        self.route_waypoint_interval = if self.route_waypoint_interval < 1 {
            WAYPOINT_DEFAULT
        } else {
            self.route_waypoint_interval.min(WAYPOINT_MAX)
        };
        self.exp_per_hour.global = self.exp_per_hour.global.normalized();
        self
    }

    /// The block exp/hr runs with: the character's own when "by character" is on and it has one
    /// (`LoadExpPerHourKnobs` reading the character file), else the global one.
    #[must_use]
    pub fn effective_exp_per_hour(&self, character: Option<&ExpHrBlock>) -> ExpPerHourSettings {
        match character {
            Some(own) if self.exp_per_hour.by_character => own.normalized().to_rules(),
            _ => self.exp_per_hour.global.to_rules(),
        }
    }
}

/// Reads `<dir>/settings.json`: defaults when it is missing, an error text when it is unreadable
/// or not valid, so a damaged file is reported and never quietly replaced by defaults.
pub fn load(dir: &FsPath) -> Result<Settings, String> {
    let path = dir.join(FILE_NAME);
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<Settings>(&bytes)
            .map(Settings::normalized)
            .map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

static TMP: AtomicU64 = AtomicU64::new(0);

/// Atomic write: a temp file next to the target, then a rename. A damaged existing file is kept
/// as `settings.json.damaged` first.
pub fn store(dir: &FsPath, settings: &Settings, replace_damaged: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let target: PathBuf = dir.join(FILE_NAME);
    if replace_damaged && target.exists() {
        std::fs::rename(&target, dir.join(format!("{FILE_NAME}.damaged")))?;
    }
    let bytes = serde_json::to_vec_pretty(settings).map_err(std::io::Error::other)?;
    let tmp = dir.join(format!(
        ".{}-{}.settings.tmp",
        std::process::id(),
        TMP.fetch_add(1, Ordering::Relaxed)
    ));
    let result = std::fs::write(&tmp, &bytes).and_then(|()| std::fs::rename(&tmp, &target));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[derive(Serialize)]
struct SettingsReply {
    #[serde(flatten)]
    settings: Settings,
    /// With `?character=NAME`: the exp/hr block that character's calculations use.
    #[serde(skip_serializing_if = "Option::is_none")]
    effective: Option<ExpHrBlock>,
}

#[derive(Deserialize)]
pub struct CharacterQuery {
    character: Option<String>,
}

fn server_error(msg: String) -> (axum::http::StatusCode, String) {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("settings: {msg}"),
    )
}

pub async fn get(State(state): State<Arc<AppState>>, Query(q): Query<CharacterQuery>) -> ApiResult {
    let settings = state.settings().map_err(server_error)?;
    let effective = match q.character {
        None => None,
        Some(name) => {
            crate::character::validate_name(&name).map_err(bad)?;
            let own = crate::character::read_dto_opt(&state, &name)
                .await?
                .and_then(|c: CharacterDto| c.exp_hr);
            Some(settings.effective_exp_per_hour(own.as_ref()).into())
        }
    };
    json(&SettingsReply {
        settings,
        effective,
    })
}

pub async fn put(State(state): State<Arc<AppState>>, Body(body): Body<Settings>) -> ApiResult {
    let stored = state
        .put_settings(body.normalized())
        .map_err(server_error)?;
    json(&SettingsReply {
        settings: stored,
        effective: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_mmes_code_behaviour() {
        let s = Settings::default();
        assert!(s.exp_per_hour.global.model_d);
        assert_eq!(s.monster_sim_rounds, 500);
        assert_eq!(s.route_waypoint_interval, 25);
        assert!(s.auto_load_last_char);
    }

    #[test]
    fn save_rules() {
        let mut s = Settings {
            monster_sim_rounds: 5,
            route_waypoint_interval: 0,
            ..Settings::default()
        };
        s.exp_per_hour.global.knob_dmg = 0.004;
        s.exp_per_hour.global.knob_xp = 12.0;
        s.exp_per_hour.global.knob_mana = 1.234;
        let n = s.normalized();
        assert_eq!(n.monster_sim_rounds, 100);
        assert_eq!(n.route_waypoint_interval, 25);
        assert!((n.exp_per_hour.global.knob_dmg - 0.01).abs() < 1e-12);
        assert!((n.exp_per_hour.global.knob_xp - 10.0).abs() < 1e-12);
        assert!((n.exp_per_hour.global.knob_mana - 1.23).abs() < 1e-12);
        s.monster_sim_rounds = 99_999;
        s.route_waypoint_interval = 5000;
        let n = s.normalized();
        assert_eq!(n.monster_sim_rounds, 10_000);
        assert_eq!(n.route_waypoint_interval, 1000);
    }

    #[test]
    fn no_model_ticked_turns_abc_on() {
        let mut s = Settings::default();
        let g = &mut s.exp_per_hour.global;
        (g.model_a, g.model_b, g.model_c, g.model_d) = (false, false, false, false);
        let g = s.normalized().exp_per_hour.global;
        assert!(g.model_a && g.model_b && g.model_c && !g.model_d);
    }
}
