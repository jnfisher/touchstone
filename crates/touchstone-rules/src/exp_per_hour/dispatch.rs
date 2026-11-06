//! `CalcExpPerHour`: runs the enabled exp/hr models, averages them and builds the display text.

use touchstone_vb6 as vb6;

use super::{
    ExpPerHourInfo, ExpPerHourInput, ExpPerHourKnobs, model_a, model_b, model_c, model_d, vb,
};
use crate::combat::is_mob_killable;
use crate::{Profile, RulesError};

/// The exp/hr settings MME keeps in globals (`bGlobal_cephModelA..D`, `bGlobal_cephRecoveryOnly`,
/// `bGlobal_cephShowAll` and the four `nGlobal_ceph*_Knob`), read by `CalcExpPerHour`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExpPerHourSettings {
    pub model_a: bool,
    pub model_b: bool,
    pub model_c: bool,
    pub model_d: bool,
    /// `bGlobal_cephRecoveryOnly`: force the damage threshold to -1 (basic damage).
    pub recovery_only: bool,
    /// `bGlobal_cephShowAll`: add each model's own figure to the text when several ran.
    pub show_all: bool,
    pub knobs: ExpPerHourKnobs,
}

impl Default for ExpPerHourSettings {
    /// What MME runs with when `settings.ini` has no `cephModel*` keys: `Form_Load` starts with
    /// A, B and C on and D off, but `LoadExpPerHourKnobs` runs straight after and reads each
    /// missing key with the default 1, so all four models are on. The doc file
    /// `exp-per-hour-models.md` says D is off by default; the code does not.
    fn default() -> Self {
        Self {
            model_a: true,
            model_b: true,
            model_c: true,
            model_d: true,
            recovery_only: false,
            show_all: false,
            knobs: ExpPerHourKnobs::default(),
        }
    }
}

impl ExpPerHourSettings {
    /// The state `frmMain Form_Load` sets before the INI is read: A, B and C on, D off.
    #[must_use]
    pub fn at_form_load() -> Self {
        Self {
            model_d: false,
            ..Self::default()
        }
    }

    /// The settings dialog's save rule (`frmSettings cmdSave_Click`): with no model ticked, A, B
    /// and C are turned on. `calc_exp_per_hour` itself does not do this.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        if !(self.model_a || self.model_b || self.model_c || self.model_d) {
            self.model_a = true;
            self.model_b = true;
            self.model_c = true;
        }
        self
    }
}

/// `LoadExpPerHourKnobs` clamp for one knob: `Round(value, 2)`, then at least 0.01 and at most 10.
#[must_use]
pub fn clamp_knob(value: f64) -> f64 {
    vb6::round(value, 2).unwrap_or(value).clamp(0.01, 10.0)
}

/// `Round(x * 100) & "%"`.
fn pct(x: f64) -> String {
    format!("{}%", vb6::cstr(vb6::round_half_even(x * 100.0)))
}

/// Port of modSyntaxsFunc `AutoAppend`.
fn auto_append(base: &str, add: &str, glue: &str) -> String {
    if base.is_empty() {
        add.to_string()
    } else if add.trim().is_empty() {
        base.to_string()
    } else {
        format!("{base}{glue}{add}")
    }
}

/// Port of modSyntaxsFunc `RoundUp` (ceiling via Int).
fn round_up(n: f64) -> f64 {
    if 0.0 < n - n.floor() {
        n.floor() + 1.0
    } else {
        n.floor()
    }
}

/// The compact exp/hr shown per model in the show-all text.
fn short_exp(x: f64) -> Result<String, RulesError> {
    Ok(if x > 1_000_000.0 {
        format!("{}M", vb6::format(x / 1_000_000.0, "#,#.00").map_err(vb)?)
    } else if x > 1000.0 {
        format!("{}K", vb6::format(x / 1000.0, "#,#.0").map_err(vb)?)
    } else if x > 0.0 {
        vb6::format(round_up(x), "#,#").map_err(vb)?
    } else {
        "0".to_string()
    })
}

/// Port of modExpPerHour `CalcExpPerHour` (stock path).
///
/// Runs the models `settings` turns on, adds up their records (`nRTC` twice, as MME does) and,
/// with more than one model, divides by the count and rounds. Text fields are built here.
/// `nExp = 0` returns the empty record; a lair the character cannot kill returns exp/hr -1 with
/// HP-recovery and time-recovering 1. With no model on the record stays all zero. `nTotalLairs`
/// and the other model arguments follow the models' own conversions.
///
/// # Errors
/// [`RulesError::Vb6`] 6 when `n_char_hp_regen` does not fit the `Integer` parameter of
/// `IsMobKillable`, `n_mob_hp - n_surprise_dmg` does not fit a `Long`, or a model overflows. This
/// procedure has no error handler, so these escape to the caller.
#[allow(clippy::too_many_lines)]
pub fn calc_exp_per_hour(
    input: &ExpPerHourInput,
    settings: &ExpPerHourSettings,
    profile: Profile,
) -> Result<ExpPerHourInfo, RulesError> {
    match profile {
        Profile::Stock => {}
    }
    let mut ret = ExpPerHourInfo::default();
    if input.n_exp.scaled() == 0 {
        return Ok(ret);
    }
    let char_hp_regen = i16::try_from(input.n_char_hp_regen).map_err(|_| RulesError::Vb6(6))?;
    let mob_hp_left = vb6::clng(f64::from(input.n_mob_hp) - input.n_surprise_dmg).map_err(vb)?;
    if !is_mob_killable(
        profile,
        input.n_char_dmg,
        input.n_char_hp,
        input.n_mob_dmg,
        mob_hp_left,
        char_hp_regen,
        input.n_mob_hp_regen,
    ) {
        ret.n_exp_per_hour = -1.0;
        ret.n_hitpoint_recovery = 1.0;
        ret.n_time_recovering = 1.0;
        return Ok(ret);
    }

    let mut model_input = input.clone();
    if settings.recovery_only {
        model_input.n_damage_threshold = -1;
    }
    let knobs = &settings.knobs;

    let mut movement_limited = false;
    let mut surprise_less = false;
    let mut results: [Option<ExpPerHourInfo>; 4] = [None, None, None, None];
    let on = [
        settings.model_a,
        settings.model_b,
        settings.model_c,
        settings.model_d,
    ];
    for (x, slot) in results.iter_mut().enumerate() {
        if !on[x] {
            continue;
        }
        let mut r = match x {
            0 => model_a(&model_input, knobs, profile)?,
            1 => model_b(&model_input, knobs, profile)?,
            2 => model_c(&model_input, knobs, profile)?,
            _ => model_d(&model_input, knobs, profile)?,
        };
        if r.n_move < 0.0 {
            movement_limited = true;
            r.n_move *= -1.0;
        }
        if r.n_attack_time < 0.0 {
            surprise_less = true;
            r.n_attack_time *= -1.0;
        }
        *slot = Some(r);
    }

    let show_all = settings.show_all;
    let mut count = 0_i32;
    let (mut attack_all, mut recover_all, mut recover_all_hp) =
        (String::new(), String::new(), String::new());
    let (mut recovery_all_mana, mut move_all) = (String::new(), String::new());
    for (x, r) in results.iter().enumerate() {
        let Some(r) = r else { continue };
        count += 1;
        ret.n_exp_per_hour += r.n_exp_per_hour;
        ret.n_hitpoint_recovery += r.n_hitpoint_recovery;
        ret.n_mana_recovery += r.n_mana_recovery;
        ret.n_time_recovering += r.n_time_recovering;
        ret.n_overkill += r.n_overkill;
        ret.n_move += r.n_move;
        ret.n_rtc += r.n_rtc;
        ret.n_roam_time += r.n_roam_time;
        ret.n_slowdown_time += r.n_slowdown_time;
        ret.n_attack_time += r.n_attack_time;
        ret.n_rtc += r.n_rtc;

        if show_all {
            let prefix = ["A:", "B:", "C:", "D:"][x];
            let temp = short_exp(r.n_exp_per_hour)?;
            ret.s_exp_all = auto_append(&ret.s_exp_all, &format!("{prefix}{temp}"), "/");
            attack_all = auto_append(&attack_all, &pct(r.n_attack_time), "/");
            recover_all = auto_append(&recover_all, &pct(r.n_time_recovering), "/");
            recover_all_hp = auto_append(&recover_all_hp, &pct(r.n_hitpoint_recovery), "/");
            recovery_all_mana = auto_append(&recovery_all_mana, &pct(r.n_mana_recovery), "/");
            move_all = auto_append(&move_all, &pct(r.n_move), "/");
        }
    }
    if count > 1 {
        let n = f64::from(count);
        let r2 = |x: f64| vb6::round(x / n, 2).map_err(vb);
        ret.n_exp_per_hour = vb6::round_half_even(ret.n_exp_per_hour / n);
        ret.n_hitpoint_recovery = r2(ret.n_hitpoint_recovery)?;
        ret.n_mana_recovery = r2(ret.n_mana_recovery)?;
        ret.n_time_recovering = r2(ret.n_time_recovering)?;
        ret.n_overkill = r2(ret.n_overkill)?;
        ret.n_move = r2(ret.n_move)?;
        ret.n_rtc = r2(ret.n_rtc)?;
        ret.n_roam_time = r2(ret.n_roam_time)?;
        ret.n_slowdown_time = r2(ret.n_slowdown_time)?;
        ret.n_attack_time = r2(ret.n_attack_time)?;
        // MME divides nRTC a second time here (the line is duplicated in the source).
        ret.n_rtc = r2(ret.n_rtc)?;
        if show_all {
            for s in [
                &mut ret.s_exp_all,
                &mut attack_all,
                &mut recover_all,
                &mut recover_all_hp,
                &mut recovery_all_mana,
                &mut move_all,
            ] {
                if !s.is_empty() {
                    *s = format!(" ({s})");
                }
            }
        }
    } else {
        ret.s_exp_all.clear();
        attack_all.clear();
        recover_all.clear();
        recover_all_hp.clear();
        recovery_all_mana.clear();
        move_all.clear();
    }

    if ret.n_attack_time > 0.0 && ret.n_attack_time < 1.0 {
        ret.s_rtc_text = format!(
            "{} time spent attacking{attack_all}",
            pct(ret.n_attack_time)
        );
    }
    if ret.n_slowdown_time > 0.01 && ret.n_slowdown_time < 1.0 {
        ret.s_rtc_text = auto_append(
            &ret.s_rtc_text,
            &format!("{} slower kill speed", pct(ret.n_slowdown_time)),
            ", ",
        );
    }
    if ret.n_overkill > 0.01 && input.n_char_dmg < 9_999_999.0 {
        ret.s_rtc_text = auto_append(
            &ret.s_rtc_text,
            &format!("{} wasted overkill", pct(ret.n_overkill)),
            ", ",
        );
    }

    if ret.n_time_recovering > 0.01 {
        ret.s_time_recovering = format!(
            "{} time spent recovering{recover_all}",
            pct(ret.n_time_recovering)
        );
    }
    if ret.n_hitpoint_recovery > 0.01 {
        ret.s_hitpoint_recovery = format!(
            "{} reduction due to HP recovery{recover_all_hp}",
            pct(ret.n_hitpoint_recovery)
        );
    }
    if ret.n_mana_recovery > 0.01 {
        ret.s_mana_recovery = format!(
            "{} reduction due to mana recovery{recovery_all_mana}",
            pct(ret.n_mana_recovery)
        );
    }

    if ret.n_move > 0.01 {
        ret.s_move_text = format!("{} time spent moving{move_all}", pct(ret.n_move));
    }
    if ret.n_roam_time > 0.04 {
        ret.s_move_text = auto_append(
            &ret.s_move_text,
            &format!(
                "{} time lost due to insufficient lairs",
                pct(ret.n_roam_time)
            ),
            ", ",
        );
    }

    if movement_limited {
        ret.s_move_text = auto_append(
            &ret.s_move_text,
            "(cluster detected: movement limited)",
            " ",
        );
    }
    if surprise_less {
        ret.s_rtc_text = auto_append(&ret.s_rtc_text, "[backstab is worse than attack]", " ");
    }
    Ok(ret)
}
