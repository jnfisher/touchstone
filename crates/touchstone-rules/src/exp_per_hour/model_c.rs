//! Model C: the per-lair cycle macro-simulation (combat profile, cycle profile, per-hour).
#![allow(clippy::manual_clamp, clippy::needless_late_init)] // MME's guarded assignments, kept in its order.

use touchstone_vb6::format;

use super::helpers::{
    SEC_PER_MEDI_TICK, SEC_PER_REGEN_TICK, SEC_PER_REST_TICK, SEC_PER_ROUND, ceph_c_ceil,
};
use super::{ExpPerHourInfo, ExpPerHourInput, ExpPerHourKnobs, UNSUPPORTED_UNDER_QUICKSILVER, vb};
use crate::stats::regen_on;
use crate::{Profile, RulesError};

const DENSITY_K: f64 = 0.2;
const XP_KNOB: f64 = 1.05;
const REST_KNOB: f64 = 0.8;
const SLACK_KNOB: f64 = 0.2;
const HP_REST_START_FRAC: f64 = 0.75;
const HP_REST_TARGET_FRAC: f64 = 0.9;
const MP_REST_START_FRAC: f64 = 0.25;
const MP_REST_TARGET_FRAC: f64 = 0.9;
const MAX_LAIRS_PER_CYCLE: i32 = 200;
/// `STOCK_MOB_HPREGEN_ROUNDS` (modMMudFunc): rounds between mob HP-regen ticks.
const STOCK_MOB_HPREGEN_ROUNDS: f64 = 18.0;

/// `tCephC_CombatProfile` (the two fields nothing reads are not kept).
struct CombatProfile {
    rtk_mob: f64,
    rtc_lair: f64,
    overkill_frac: f64,
    slowdown_frac: f64,
}

/// `tCephC_CycleProfile` (`ExpPerCycle` and `roamSecs` are never read).
#[derive(Default)]
struct CycleProfile {
    cycle_secs: f64,
    attack_secs: f64,
    move_secs: f64,
    rest_hp_secs: f64,
    rest_mp_secs: f64,
}

/// Port of modExpPerHour `cephC_BuildCombatProfile` (stock path).
///
/// Quirks kept: the min-damage tail is added unconditionally, rounds-to-kill is integer-ceiled,
/// mob HP regen applies from a fixed 6 rounds of fight, and a missed surprise wastes a full round.
fn build_combat_profile(input: &ExpPerHourInput, num_mobs: f64, char_dmg: f64) -> CombatProfile {
    let n_mob_hp = input.n_mob_hp;
    let first_round = input.n_char_first_round_dmg;
    let min_round = input.n_min_round_dmg;
    let surprise_dmg = input.n_surprise_dmg;
    let surprise_min = input.n_surprise_min_dmg;
    let surprise_chance = input.n_surprise_chance;

    let mob_hp = f64::from(n_mob_hp) / num_mobs;

    // The two normal-combat damages stay 0 on the degenerate path, as VB6 locals do.
    let mut first_dmg_norm = 0.0;
    let mut avg_dmg_norm = 0.0;
    let rtk_avg;

    if mob_hp <= 0.0 || (char_dmg <= 0.0 && first_round <= 0.0) {
        rtk_avg = 1.0;
    } else {
        first_dmg_norm = first_round;
        if first_dmg_norm <= 0.0 {
            first_dmg_norm = char_dmg;
        }
        avg_dmg_norm = char_dmg;
        if avg_dmg_norm <= 0.0 {
            avg_dmg_norm = first_dmg_norm;
        }

        let approx_rtk_no_regen = if avg_dmg_norm > 0.0 {
            mob_hp / avg_dmg_norm
        } else {
            0.0
        };

        let mut avg_dmg_for_seq = avg_dmg_norm;
        if input.n_mob_hp_regen > 0 && approx_rtk_no_regen >= 6.0 {
            let per_mob_regen_tick = f64::from(input.n_mob_hp_regen) / num_mobs;
            let regen_per_round =
                per_mob_regen_tick * (SEC_PER_ROUND / (STOCK_MOB_HPREGEN_ROUNDS * SEC_PER_ROUND));
            avg_dmg_for_seq = avg_dmg_norm - regen_per_round;
            if avg_dmg_for_seq < avg_dmg_norm * 0.1 {
                avg_dmg_for_seq = avg_dmg_norm * 0.1;
            }
        }

        let rtk_normal_base = if mob_hp <= first_dmg_norm {
            1.0
        } else {
            let remain_hp = mob_hp - first_dmg_norm;
            let extra_rounds = if avg_dmg_for_seq > 0.0 {
                ceph_c_ceil(remain_hp / avg_dmg_for_seq)
            } else {
                0.0
            };
            1.0 + extra_rounds
        };

        let mut extra_prob_normal = 0.0;
        if avg_dmg_norm > 0.0 && min_round > 0.0 && min_round < avg_dmg_norm {
            extra_prob_normal = (avg_dmg_norm - min_round) / avg_dmg_norm;
            if extra_prob_normal < 0.0 {
                extra_prob_normal = 0.0;
            }
            if extra_prob_normal > 1.0 {
                extra_prob_normal = 1.0;
            }
        }
        let rtk_normal = rtk_normal_base + extra_prob_normal;

        let mut rtk_first = rtk_normal;
        if surprise_dmg > 0.0 && surprise_chance > 0 {
            let rtk_surprise_base = if mob_hp <= surprise_dmg {
                1.0
            } else {
                let remain_hp = mob_hp - surprise_dmg;
                let extra_rounds = if avg_dmg_for_seq > 0.0 {
                    ceph_c_ceil(remain_hp / avg_dmg_for_seq)
                } else {
                    0.0
                };
                1.0 + extra_rounds
            };

            let mut extra_prob_surprise = 0.0;
            if surprise_dmg > 0.0 && surprise_min > 0.0 && surprise_min < surprise_dmg {
                extra_prob_surprise = (surprise_dmg - surprise_min) / surprise_dmg;
                if extra_prob_surprise < 0.0 {
                    extra_prob_surprise = 0.0;
                }
                if extra_prob_surprise > 1.0 {
                    extra_prob_surprise = 1.0;
                }
            }
            let rtk_surprise_hit = rtk_surprise_base + extra_prob_surprise;
            let rtk_surprise_miss = 1.0 + rtk_normal;

            let mut p_backstab = f64::from(surprise_chance) / 100.0;
            if p_backstab < 0.0 {
                p_backstab = 0.0;
            }
            if p_backstab > 1.0 {
                p_backstab = 1.0;
            }
            rtk_first = p_backstab * rtk_surprise_hit + (1.0 - p_backstab) * rtk_surprise_miss;
        }

        rtk_avg = (rtk_first + (num_mobs - 1.0) * rtk_normal) / num_mobs;
    }

    // Overkill is a per-mob figure from the final round only.
    let mut overkill_frac = 0.0;
    if mob_hp > 0.0 {
        let last_round_dmg;
        let hp_before_last;
        if mob_hp <= first_dmg_norm {
            last_round_dmg = first_dmg_norm;
            hp_before_last = 0.0;
        } else {
            let remain_det = mob_hp - first_dmg_norm;
            let extra_det = if avg_dmg_norm > 0.0 {
                ceph_c_ceil(remain_det / avg_dmg_norm)
            } else {
                0.0
            };
            let mut h = mob_hp - first_dmg_norm - (extra_det - 1.0) * avg_dmg_norm;
            if h < 0.0 {
                h = 0.0;
            }
            hp_before_last = h;
            last_round_dmg = avg_dmg_norm;
        }
        let mut spill = last_round_dmg - hp_before_last;
        if spill < 0.0 {
            spill = 0.0;
        }
        if last_round_dmg > 0.0 {
            overkill_frac = spill / last_round_dmg;
            if overkill_frac < 0.0 {
                overkill_frac = 0.0;
            }
            if overkill_frac > 1.0 {
                overkill_frac = 1.0;
            }
        }
    }

    let mut slowdown_frac = 0.0;
    if rtk_avg > 0.0 {
        slowdown_frac = (rtk_avg - 1.0) / rtk_avg;
        if slowdown_frac < 0.0 {
            slowdown_frac = 0.0;
        }
        if slowdown_frac > 1.0 {
            slowdown_frac = 1.0;
        }
    }

    CombatProfile {
        rtk_mob: rtk_avg,
        rtc_lair: rtk_avg * num_mobs,
        overkill_frac,
        slowdown_frac,
    }
}

/// Port of modExpPerHour `cephC_EstimateMoveSecs`: seconds of walking per lair, with a mild
/// density effect for extremely sparse zones.
fn estimate_move_secs(
    n_total_lairs: i32,
    n_poss_spawns: i32,
    n_avg_walk: f64,
    n_walk_speed: f64,
) -> f64 {
    let mut eff_walk_rooms = n_avg_walk;
    if eff_walk_rooms <= 0.0 || n_walk_speed <= 0.0 {
        return 0.0;
    }

    let density_factor;
    if n_total_lairs > 0 && n_poss_spawns >= 0 {
        let mut dens =
            f64::from(n_total_lairs) / (f64::from(n_total_lairs) + f64::from(n_poss_spawns));
        if dens < 0.0 {
            dens = 0.0;
        }
        if dens > 1.0 {
            dens = 1.0;
        }
        if dens < 0.05 {
            // dens is positive here, so VB6's `^` cannot fail.
            let mut f = dens.powf(DENSITY_K);
            if f < 0.9 {
                f = 0.9;
            }
            if f > 1.1 {
                f = 1.1;
            }
            density_factor = f;
        } else {
            density_factor = 1.0;
        }
    } else {
        density_factor = 1.0;
    }

    eff_walk_rooms *= density_factor;
    if eff_walk_rooms < 0.0 {
        eff_walk_rooms = 0.0;
    }
    eff_walk_rooms * n_walk_speed
}

/// Port of modExpPerHour `cephC_BuildCycleProfile`.
#[allow(clippy::too_many_lines)]
fn build_cycle_profile(
    input: &ExpPerHourInput,
    knobs: &ExpPerHourKnobs,
    n_walk_speed: f64,
    n_char_mana: i16,
    combat: &CombatProfile,
) -> CycleProfile {
    let n_total_lairs = input.n_total_lairs;
    let n_regen_time = input.n_regen_time;
    let n_damage_threshold = input.n_damage_threshold;
    let mut c = CycleProfile::default();

    let mut attack_secs_per_lair = combat.rtc_lair * SEC_PER_ROUND;
    if attack_secs_per_lair < 0.0 {
        attack_secs_per_lair = 0.0;
    }

    // Boss: no movement or recovery, only the spawn timer (hours) matters.
    if n_total_lairs <= 0 && n_regen_time > 0.0 {
        let mut regen_secs = n_regen_time * 3600.0;
        if regen_secs < attack_secs_per_lair {
            regen_secs = attack_secs_per_lair;
        }
        c.attack_secs = attack_secs_per_lair;
        c.cycle_secs = regen_secs;
        return c;
    }

    let mut move_secs_per_lair;
    if n_total_lairs < 0 && n_regen_time == 0.0 {
        move_secs_per_lair = 0.0;
    } else {
        move_secs_per_lair = estimate_move_secs(
            n_total_lairs,
            input.n_poss_spawns,
            input.n_avg_walk,
            n_walk_speed,
        ) * knobs.mv;
        if move_secs_per_lair < 0.0 {
            move_secs_per_lair = 0.0;
        }
    }

    // HP regen is in resting-rate form: passive is a third of it per regen tick.
    let mut hp_nat_per_sec = (f64::from(input.n_char_hp_regen) / 3.0) / SEC_PER_REGEN_TICK;
    let mut hp_rest_extra_per_sec = f64::from(input.n_char_hp_regen) / SEC_PER_REST_TICK;

    let mut mp_nat_per_sec = f64::from(input.n_char_mp_regen) / SEC_PER_REGEN_TICK;
    let mut mp_med_extra_per_sec = 0.0;
    if input.n_meditate_rate > 0 {
        mp_med_extra_per_sec = f64::from(input.n_meditate_rate) / SEC_PER_MEDI_TICK;
    }

    let mut eff_mob_dmg_per_round = input.n_mob_dmg * REST_KNOB * knobs.dmg;
    if eff_mob_dmg_per_round < 0.0 {
        eff_mob_dmg_per_round = 0.0;
    }
    let mut eff_dmg_after_threshold = eff_mob_dmg_per_round - f64::from(n_damage_threshold);
    if eff_dmg_after_threshold < 0.0 {
        eff_dmg_after_threshold = 0.0;
    }
    let mut hp_drain_attack = eff_dmg_after_threshold * combat.rtc_lair;

    let mut eff_spell_per_round =
        (f64::from(input.n_spell_cost) + input.n_spell_overhead) * knobs.mana;
    if eff_spell_per_round < 0.0 {
        eff_spell_per_round = 0.0;
    }
    let mut mp_drain_attack = eff_spell_per_round * combat.rtc_lair;

    // A negative threshold means unlimited HP/MP: no drain and no recovery.
    if n_damage_threshold < 0 {
        hp_nat_per_sec = 0.0;
        hp_rest_extra_per_sec = 0.0;
        mp_nat_per_sec = 0.0;
        mp_med_extra_per_sec = 0.0;
        hp_drain_attack = 0.0;
        mp_drain_attack = 0.0;
    }

    let hp_max = f64::from(input.n_char_hp);
    let mp_max = f64::from(n_char_mana);
    let use_hp = hp_max > 0.0
        && (hp_drain_attack > 0.0 || hp_nat_per_sec > 0.0 || hp_rest_extra_per_sec > 0.0);
    let use_mp = mp_max > 0.0
        && (mp_drain_attack > 0.0 || mp_nat_per_sec > 0.0 || mp_med_extra_per_sec > 0.0);

    let (th_hp, target_hp) = if use_hp {
        let mut t = HP_REST_TARGET_FRAC * hp_max;
        if t > hp_max {
            t = hp_max;
        }
        (HP_REST_START_FRAC * hp_max, t)
    } else {
        (0.0, 0.0)
    };
    let (th_mp, target_mp) = if use_mp {
        let mut t = MP_REST_TARGET_FRAC * mp_max;
        if t > mp_max {
            t = mp_max;
        }
        (MP_REST_START_FRAC * mp_max, t)
    } else {
        (0.0, 0.0)
    };

    if !use_hp && !use_mp {
        c.attack_secs = attack_secs_per_lair;
        c.move_secs = move_secs_per_lair;
        c.cycle_secs = attack_secs_per_lair + move_secs_per_lair;
        return c;
    }

    // Macro-cycle: fight lairs from full until HP < 75% or MP < 25%, then rest, then average.
    let mut hp_cur = hp_max;
    let mut mp_cur = mp_max;
    let mut tot_attack = 0.0;
    let mut tot_move = 0.0;
    let mut tot_rest_hp = 0.0;
    let mut tot_rest_mp = 0.0;
    let mut lairs_cleared: i32 = 0;

    for _ in 1..=MAX_LAIRS_PER_CYCLE {
        if move_secs_per_lair > 0.0 {
            if use_hp && hp_nat_per_sec > 0.0 {
                hp_cur += hp_nat_per_sec * move_secs_per_lair;
                if hp_cur > hp_max {
                    hp_cur = hp_max;
                }
            }
            if use_mp && mp_nat_per_sec > 0.0 {
                mp_cur += mp_nat_per_sec * move_secs_per_lair;
                if mp_cur > mp_max {
                    mp_cur = mp_max;
                }
            }
            tot_move += move_secs_per_lair;
        }

        if use_hp {
            hp_cur -= hp_drain_attack;
            if hp_nat_per_sec > 0.0 {
                hp_cur += hp_nat_per_sec * attack_secs_per_lair;
            }
            if hp_cur < 0.0 {
                hp_cur = 0.0;
            }
            if hp_cur > hp_max {
                hp_cur = hp_max;
            }
        }
        if use_mp {
            mp_cur -= mp_drain_attack;
            if mp_nat_per_sec > 0.0 {
                mp_cur += mp_nat_per_sec * attack_secs_per_lair;
            }
            if mp_cur < 0.0 {
                mp_cur = 0.0;
            }
            if mp_cur > mp_max {
                mp_cur = mp_max;
            }
        }

        tot_attack += attack_secs_per_lair;
        lairs_cleared += 1;

        if use_hp && hp_cur <= th_hp {
            break;
        }
        if use_mp && mp_cur <= th_mp {
            break;
        }
    }
    if lairs_cleared <= 0 {
        lairs_cleared = 1;
    }

    if use_hp && hp_cur < target_hp {
        let hp_regen_per_sec_rest = hp_nat_per_sec + hp_rest_extra_per_sec;
        if hp_regen_per_sec_rest > 0.0 {
            let mut hp_deficit = target_hp - hp_cur;
            if hp_deficit < 0.0 {
                hp_deficit = 0.0;
            }
            let mut rest_hp_secs = hp_deficit / hp_regen_per_sec_rest;
            if rest_hp_secs < 0.0 {
                rest_hp_secs = 0.0;
            }
            tot_rest_hp += rest_hp_secs;
            // MME also refills HP here, but the value is never read afterwards.
            if use_mp && mp_nat_per_sec > 0.0 {
                mp_cur += mp_nat_per_sec * rest_hp_secs;
                if mp_cur > mp_max {
                    mp_cur = mp_max;
                }
            }
        }
    }

    if use_mp && mp_cur < target_mp {
        let mp_regen_per_sec_rest = mp_nat_per_sec + mp_med_extra_per_sec;
        if mp_regen_per_sec_rest > 0.0 {
            let mut mp_deficit = target_mp - mp_cur;
            if mp_deficit < 0.0 {
                mp_deficit = 0.0;
            }
            let mut rest_mp_secs = mp_deficit / mp_regen_per_sec_rest;
            if rest_mp_secs < 0.0 {
                rest_mp_secs = 0.0;
            }
            tot_rest_mp += rest_mp_secs;
            // MME also credits natural HP here, but the value is never read afterwards.
        }
    }

    let lairs = f64::from(lairs_cleared);
    c.attack_secs = tot_attack / lairs;
    c.move_secs = tot_move / lairs;
    c.rest_hp_secs = tot_rest_hp / lairs;
    c.rest_mp_secs = tot_rest_mp / lairs;
    c.cycle_secs = c.attack_secs + c.move_secs + c.rest_hp_secs + c.rest_mp_secs;
    c
}

/// Port of modExpPerHour `ceph_ModelC`.
///
/// Only the stock profile exists. Model C ignores `n_rtk`: it recomputes rounds-to-kill itself
/// (MME's own notes list the bugs of that recompute as open, and they are kept). The
/// narrow VB6 arguments (`nCharMana`, `nSpellCost`, `nSurpriseChance` are `Integer`) are checked
/// by the caller's types; mana outside the `Integer` range is run-time error 6.
///
/// # Errors
/// [`RulesError::Vb6`] for overflow (mana) or a failing `Format`.
#[allow(clippy::too_many_lines)]
pub fn model_c(
    input: &ExpPerHourInput,
    knobs: &ExpPerHourKnobs,
    profile: Profile,
) -> Result<ExpPerHourInfo, RulesError> {
    if regen_on(profile).is_some() {
        return Err(RulesError::Unsupported(UNSUPPORTED_UNDER_QUICKSILVER));
    }
    let mut ret = ExpPerHourInfo::default();

    let n_total_lairs = input.n_total_lairs;
    let n_regen_time = input.n_regen_time;
    let n_exp = input.n_exp;
    let n_char_dmg = input.n_char_dmg;
    let mut n_num_mobs = input.n_num_mobs;
    let mut n_walk_speed = input.n_walk_speed;
    let n_char_mana = i16::try_from(input.n_char_mana).map_err(|_| RulesError::Vb6(6))?;

    if n_num_mobs <= 0.0 {
        n_num_mobs = 1.0;
    }
    if n_walk_speed <= 0.0 {
        n_walk_speed = 1.0;
    }

    let combat = build_combat_profile(input, n_num_mobs, n_char_dmg);
    let mut cycle = build_cycle_profile(input, knobs, n_walk_speed, n_char_mana, &combat);

    let is_boss = n_total_lairs <= 0 && n_regen_time > 0.0;

    // Slack: per-kill overhead, only when the cycle has no explicit rest. Attack and move are
    // scaled together so the move share is kept.
    if !is_boss && cycle.rest_hp_secs <= 0.0 && cycle.rest_mp_secs <= 0.0 {
        let base_am = cycle.attack_secs + cycle.move_secs;
        if base_am > 0.0 {
            let mut rtk_eff = combat.rtk_mob;
            if rtk_eff <= 0.0 {
                rtk_eff = 1.0;
            }
            let slack_secs = SLACK_KNOB * (cycle.attack_secs / rtk_eff);
            if slack_secs > 0.0 {
                let scale_factor = (base_am + slack_secs) / base_am;
                cycle.attack_secs *= scale_factor;
                cycle.move_secs *= scale_factor;
                cycle.cycle_secs =
                    cycle.attack_secs + cycle.move_secs + cycle.rest_hp_secs + cycle.rest_mp_secs;
            }
        }
    }

    let total_secs = cycle.cycle_secs;
    if total_secs <= 0.0 {
        return Ok(ret);
    }

    // Per-hour assembly with spawn-supply limiting (no +0.25 here, unlike Model D).
    let lairs_per_hour_un = 3600.0 / total_secs;
    let mut lairs_per_hour = lairs_per_hour_un;
    if n_total_lairs > 0 && n_regen_time > 0.0 {
        let lairs_per_hour_supply = f64::from(n_total_lairs) * (60.0 / n_regen_time);
        if lairs_per_hour_supply > 0.0 && lairs_per_hour > lairs_per_hour_supply {
            lairs_per_hour = lairs_per_hour_supply;
        }
    }
    if lairs_per_hour < 0.0 {
        lairs_per_hour = 0.0;
    }

    let attack_per_hour = cycle.attack_secs * lairs_per_hour;
    let move_per_hour = cycle.move_secs * lairs_per_hour;
    let rest_hp_per_hour = cycle.rest_hp_secs * lairs_per_hour;
    let rest_mp_per_hour = cycle.rest_mp_secs * lairs_per_hour;

    let mut roam_per_hour =
        3600.0 - (attack_per_hour + move_per_hour + rest_hp_per_hour + rest_mp_per_hour);
    if roam_per_hour < 0.0 {
        roam_per_hour = 0.0;
    }
    let used_secs =
        attack_per_hour + move_per_hour + rest_hp_per_hour + rest_mp_per_hour + roam_per_hour;
    if used_secs <= 0.0 {
        return Ok(ret);
    }

    ret.n_exp_per_hour = n_exp.to_f64() * lairs_per_hour;
    if !is_boss {
        ret.n_exp_per_hour = ret.n_exp_per_hour * XP_KNOB * knobs.xp;
    }

    let total_rest_per_hour = rest_hp_per_hour + rest_mp_per_hour;

    ret.n_attack_time = attack_per_hour / used_secs;
    ret.n_move = move_per_hour / used_secs;
    ret.n_roam_time = roam_per_hour / used_secs;

    if total_rest_per_hour <= 0.0 {
        ret.n_hitpoint_recovery = 0.0;
        ret.n_mana_recovery = 0.0;
        ret.n_time_recovering = 0.0;
    } else {
        ret.n_time_recovering = total_rest_per_hour / used_secs;

        if rest_hp_per_hour <= 0.0 && rest_mp_per_hour > 0.0 {
            ret.n_hitpoint_recovery = 0.0;
            ret.n_mana_recovery = ret.n_time_recovering;
        } else if rest_mp_per_hour <= 0.0 && rest_hp_per_hour > 0.0 {
            ret.n_hitpoint_recovery = ret.n_time_recovering;
            ret.n_mana_recovery = 0.0;
        } else {
            // Both rest: split the union by how hard each resource is being burned.
            let hp_max = f64::from(input.n_char_hp);
            let mp_max = f64::from(n_char_mana);

            let hp_nat_per_sec = (f64::from(input.n_char_hp_regen) / 3.0) / SEC_PER_REGEN_TICK;
            let mp_nat_per_sec = f64::from(input.n_char_mp_regen) / SEC_PER_REGEN_TICK;

            let eff_mob_dmg_per_round = input.n_mob_dmg * knobs.dmg;
            let mut eff_dmg_after_threshold =
                eff_mob_dmg_per_round - f64::from(input.n_damage_threshold);
            if eff_dmg_after_threshold < 0.0 {
                eff_dmg_after_threshold = 0.0;
            }
            let mut eff_spell_per_round =
                (f64::from(input.n_spell_cost) + input.n_spell_overhead) * knobs.mana;
            if eff_spell_per_round < 0.0 {
                eff_spell_per_round = 0.0;
            }

            let mut active_secs_per_lair = cycle.attack_secs + cycle.move_secs;
            if active_secs_per_lair < 0.0 {
                active_secs_per_lair = 0.0;
            }

            let mut net_hp_drain_per_lair = (eff_dmg_after_threshold * combat.rtc_lair)
                - (hp_nat_per_sec * active_secs_per_lair);
            if net_hp_drain_per_lair < 0.0 {
                net_hp_drain_per_lair = 0.0;
            }
            let mut net_mp_drain_per_lair =
                (eff_spell_per_round * combat.rtc_lair) - (mp_nat_per_sec * active_secs_per_lair);
            if net_mp_drain_per_lair < 0.0 {
                net_mp_drain_per_lair = 0.0;
            }

            let hp_press = if hp_max > 0.0 {
                net_hp_drain_per_lair / (hp_max + 1.0)
            } else {
                0.0
            };
            let mp_press = if mp_max > 0.0 {
                net_mp_drain_per_lair / (mp_max + 1.0)
            } else {
                0.0
            };

            let (hp_weight, mp_weight);
            if hp_press <= 0.0 && mp_press <= 0.0 {
                // Both pressures zero: fall back to the raw split.
                hp_weight = rest_hp_per_hour / total_rest_per_hour;
                mp_weight = rest_mp_per_hour / total_rest_per_hour;
            } else {
                hp_weight = hp_press / (hp_press + mp_press);
                mp_weight = mp_press / (hp_press + mp_press);
            }

            ret.n_hitpoint_recovery = ret.n_time_recovering * hp_weight;
            ret.n_mana_recovery = ret.n_time_recovering * mp_weight;
        }
    }

    ret.n_overkill = combat.overkill_frac;
    ret.n_slowdown_time = combat.slowdown_frac;
    ret.n_rtc = combat.rtc_lair;

    // A surprise round weaker than a normal one on a mob that outlasts a normal round flags the
    // attack share negative.
    if input.n_surprise_dmg > 0.0
        && n_char_dmg > 0.0
        && input.n_surprise_dmg < n_char_dmg
        && n_char_dmg < (f64::from(input.n_mob_hp) / n_num_mobs)
    {
        ret.n_attack_time *= -1.0;
    }

    let pct =
        |x: f64| -> Result<String, RulesError> { Ok(format(x * 100.0, "0.0").map_err(vb)? + "%") };
    ret.s_hitpoint_recovery = pct(ret.n_hitpoint_recovery)?;
    ret.s_mana_recovery = pct(ret.n_mana_recovery)?;
    ret.s_time_recovering = pct(ret.n_time_recovering)?;
    ret.s_move_text = pct(ret.n_move)?;
    ret.s_rtc_text = format!("RTC {}", format(ret.n_rtc, "0.00").map_err(vb)?);

    Ok(ret)
}
