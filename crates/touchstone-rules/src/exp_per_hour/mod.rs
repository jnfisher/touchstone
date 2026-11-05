//! Exp/hr prediction (modExpPerHour): shared helpers and the four models. Cards P3-20 to P3-26.
//!
//! [`ExpPerHourInput`] and [`ExpPerHourInfo`] are shared by all four models; each model is a pure
//! function of the input and the four user knobs.

use touchstone_vb6::{Currency, Vb6Error};

use crate::RulesError;

mod helpers;
mod model_a;
mod model_d;

pub use helpers::{
    SEC_PER_MEDI_TICK, SEC_PER_REGEN_TICK, SEC_PER_REST_TICK, SEC_PER_ROUND, ceph_b_saturate,
    ceph_b_smooth_step, ceph_c_ceil, clamp_dbl, max_dbl, min_dbl, safe_div,
};
pub use model_a::model_a;
pub use model_d::{ceph_d_overkill_frac, model_d};

/// The arguments of `CalcExpPerHour`, in MME's order and with MME's VB6 types (the model
/// arguments that are narrower in a model's own signature are checked there).
///
/// Unset VB6 optional arguments are zero except `n_total_lairs` (-1) and `n_walk_speed` (1.25),
/// which is what [`Default`] gives.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpPerHourInput {
    /// Exp per clear of the whole lair (all mobs).
    pub n_exp: Currency,
    /// Lair regen time in minutes (hours for a boss).
    pub n_regen_time: f64,
    pub n_num_mobs: f64,
    pub n_total_lairs: i32,
    pub n_poss_spawns: i32,
    /// Rounds to kill a single mob.
    pub n_rtk: f64,
    pub n_char_dmg: f64,
    pub n_char_hp: i32,
    /// Resting-rate form (already x3).
    pub n_char_hp_regen: i32,
    pub n_mob_dmg: f64,
    pub n_mob_hp: i32,
    pub n_mob_hp_regen: i32,
    /// Negative means recovery-only / basic damage.
    pub n_damage_threshold: i32,
    pub n_spell_cost: i16,
    pub n_spell_overhead: f64,
    pub n_char_mana: i32,
    pub n_char_mp_regen: i32,
    pub n_meditate_rate: i32,
    pub n_avg_walk: f64,
    pub n_walk_speed: f64,
    pub n_surprise_dmg: f64,
    pub n_surprise_min_dmg: f64,
    pub n_surprise_chance: i16,
    pub n_char_first_round_dmg: f64,
    pub n_min_round_dmg: f64,
}

impl Default for ExpPerHourInput {
    fn default() -> Self {
        Self {
            n_exp: Currency::from_scaled(0),
            n_regen_time: 0.0,
            n_num_mobs: 0.0,
            n_total_lairs: -1,
            n_poss_spawns: 0,
            n_rtk: 0.0,
            n_char_dmg: 0.0,
            n_char_hp: 0,
            n_char_hp_regen: 0,
            n_mob_dmg: 0.0,
            n_mob_hp: 0,
            n_mob_hp_regen: 0,
            n_damage_threshold: 0,
            n_spell_cost: 0,
            n_spell_overhead: 0.0,
            n_char_mana: 0,
            n_char_mp_regen: 0,
            n_meditate_rate: 0,
            n_avg_walk: 0.0,
            n_walk_speed: 1.25,
            n_surprise_dmg: 0.0,
            n_surprise_min_dmg: 0.0,
            n_surprise_chance: 0,
            n_char_first_round_dmg: 0.0,
            n_min_round_dmg: 0.0,
        }
    }
}

/// The four user knobs, `nGlobal_ceph{XP,DMG,Mana,Move}_Knob`. Default 1.0 (neutral).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExpPerHourKnobs {
    pub xp: f64,
    pub dmg: f64,
    pub mana: f64,
    pub mv: f64,
}

impl Default for ExpPerHourKnobs {
    fn default() -> Self {
        Self {
            xp: 1.0,
            dmg: 1.0,
            mana: 1.0,
            mv: 1.0,
        }
    }
}

/// `tExpPerHourInfo`: the result of a model (all zero / empty when MME returns an empty record).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExpPerHourInfo {
    pub n_exp_per_hour: f64,
    pub n_hitpoint_recovery: f64,
    pub n_mana_recovery: f64,
    pub n_time_recovering: f64,
    pub n_overkill: f64,
    pub n_move: f64,
    pub n_rtc: f64,
    pub n_attack_time: f64,
    pub n_slowdown_time: f64,
    pub n_roam_time: f64,
    pub s_hitpoint_recovery: String,
    pub s_mana_recovery: String,
    pub s_time_recovering: String,
    pub s_rtc_text: String,
    pub s_move_text: String,
    pub s_exp_all: String,
}

pub(crate) fn vb(e: Vb6Error) -> RulesError {
    RulesError::Vb6(e.code() as i32)
}
