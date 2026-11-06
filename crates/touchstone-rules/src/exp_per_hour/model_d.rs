//! Model D: the round-by-round exp/hr simulation MME recommends.

use touchstone_vb6::{clng, format};

use super::helpers::{
    SEC_PER_MEDI_TICK, SEC_PER_REGEN_TICK, SEC_PER_REST_TICK, SEC_PER_ROUND, ceph_b_smooth_step,
    ceph_c_ceil, safe_div,
};
use super::{ExpPerHourInfo, ExpPerHourInput, ExpPerHourKnobs, vb};
use crate::quicksilver::Balance;
use crate::stats::{QS_SLOW_SECS, regen_on};
use crate::{Profile, RulesError};

const HP_REST_START_FRAC: f64 = 0.75;
const HP_REST_TARGET_FRAC: f64 = 0.9;
const MP_REST_START_FRAC: f64 = 0.25;
const MP_REST_TARGET_FRAC: f64 = 0.9;
const MAX_LAIRS_PER_CYCLE: i32 = 200;
const KILL_OVERHEAD_SEC: f64 = 1.5;
const HEAVY_REST_RELIEF: f64 = 0.35;
const MEDITATE_EFF: f64 = 0.5;

/// Port of modExpPerHour `cephD_OverkillFrac`: share of the killing blow wasted on a mob.
#[must_use]
pub fn ceph_d_overkill_frac(per_mob_hp: f64, char_dmg: f64, char_first_round_dmg: f64) -> f64 {
    if per_mob_hp <= 0.0 {
        return 0.0;
    }
    let mut first_dmg = char_first_round_dmg;
    if first_dmg <= 0.0 {
        first_dmg = char_dmg;
    }
    let mut avg_dmg = char_dmg;
    if avg_dmg <= 0.0 {
        avg_dmg = first_dmg;
    }
    if avg_dmg <= 0.0 {
        return 0.0;
    }

    let last_dmg;
    let hp_before_last;
    if per_mob_hp <= first_dmg {
        last_dmg = first_dmg;
        hp_before_last = per_mob_hp;
    } else {
        let extra = ceph_c_ceil((per_mob_hp - first_dmg) / avg_dmg);
        let mut h = per_mob_hp - first_dmg - (extra - 1.0) * avg_dmg;
        if h < 0.0 {
            h = 0.0;
        }
        hp_before_last = h;
        last_dmg = avg_dmg;
    }

    let mut spill = last_dmg - hp_before_last;
    if spill < 0.0 {
        spill = 0.0;
    }
    if last_dmg > 0.0 {
        let f = spill / last_dmg;
        return f.clamp(0.0, 1.0);
    }
    0.0
}

/// Mana per second while recovering under Quicksilver (rule R10): the standing rate `standing` (paid every
/// 30 seconds) times the rest multiple, or the meditate multiple for a character with a meditation rate.
/// The standing ticks are part of the multiple, so nothing is added for them.
#[must_use]
pub fn qs_recovery_mana_per_sec(balance: &Balance, meditating: bool, standing: i32) -> f64 {
    let mult = if meditating {
        balance.meditate_mana_mult
    } else {
        balance.rest_mana_mult
    };
    f64::from(mult) * f64::from(standing.max(0)) / f64::from(QS_SLOW_SECS)
}

/// Seconds of the one Quicksilver recovery phase (rest or meditate heal HP and restore mana at the same
/// time and end when both are done): the longer of the two times. Returns `(hp_secs, mp_secs, phase_secs)`,
/// each time being zero when its bar needs nothing or cannot recover.
#[must_use]
pub fn qs_recovery_secs(
    hp_missing: f64,
    hp_per_sec: f64,
    mp_missing: f64,
    mp_per_sec: f64,
) -> (f64, f64, f64) {
    let time = |missing: f64, rate: f64| {
        if missing > 0.0 && rate > 0.0 {
            missing / rate
        } else {
            0.0
        }
    };
    let (hp, mp) = (time(hp_missing, hp_per_sec), time(mp_missing, mp_per_sec));
    (hp, mp, hp.max(mp))
}

/// Port of modExpPerHour `ceph_ModelD`.
///
/// The model never reads the engine variant (the canonical rounds-to-kill arrives in `n_rtk`).
/// Under a Quicksilver balance with rest and meditate mana on (R10, `regen_on`), HP and mana
/// recover in one phase instead of one after the other; everything else is the stock model. The
/// opener and the opportunist and weave procs reach it through its inputs. `n_char_mana` is an `Integer` in the VB6 signature, so a
/// value outside -32768..=32767 is run-time error 6, as when `CalcExpPerHour` passes it on.
///
/// # Errors
/// [`RulesError::Vb6`] for overflow (mana, or a round count beyond `Long`).
pub fn model_d(
    input: &ExpPerHourInput,
    knobs: &ExpPerHourKnobs,
    profile: Profile,
) -> Result<ExpPerHourInfo, RulesError> {
    let qs = regen_on(profile);
    let mut ret = ExpPerHourInfo::default();

    let n_exp = input.n_exp;
    let n_regen_time = input.n_regen_time;
    let mut n_num_mobs = input.n_num_mobs;
    let n_total_lairs = input.n_total_lairs;
    let n_rtk = input.n_rtk;
    let n_char_dmg = input.n_char_dmg;
    let n_mob_hp = input.n_mob_hp;
    let n_damage_threshold = input.n_damage_threshold;
    let mut n_walk_speed = input.n_walk_speed;
    let n_char_mana = i16::try_from(input.n_char_mana).map_err(|_| RulesError::Vb6(6))?;

    if n_num_mobs <= 0.0 {
        n_num_mobs = 1.0;
    }
    if n_walk_speed <= 0.0 {
        n_walk_speed = 1.0;
    }

    let is_boss = n_total_lairs <= 0 && n_regen_time > 0.0;
    let instant = n_total_lairs < 0 && n_regen_time == 0.0;
    let unlimited = n_damage_threshold < 0;

    // Combat rounds: the canonical RTK, or Model A's 0.5-round rule when it is absent.
    let mut rtk_avg = n_rtk;
    if rtk_avg <= 0.0 && n_char_dmg > 0.0 && n_mob_hp > 0 {
        let hp_per_mob = if n_num_mobs > 1.0 {
            f64::from(n_mob_hp) / n_num_mobs
        } else {
            f64::from(n_mob_hp)
        };
        rtk_avg = hp_per_mob / n_char_dmg;
        if rtk_avg > 1.0 {
            rtk_avg = -(-(rtk_avg * 2.0)).floor() / 2.0;
        }
    }
    if rtk_avg < 1.0 {
        rtk_avg = 1.0;
    }
    let mut rtc = rtk_avg * n_num_mobs;
    if rtc < 1.0 {
        rtc = 1.0;
    }
    let attack_secs = rtc * SEC_PER_ROUND;
    let per_mob_hp = f64::from(n_mob_hp) / n_num_mobs;

    // Undo the ramp-down and RTK scaling GetLairInfo baked into nMobDmg.
    let mut per_mob_dmg = input.n_mob_dmg;
    if n_num_mobs > 1.0 {
        per_mob_dmg *= (n_num_mobs + 1.0) / (2.0 * n_num_mobs);
    }
    if n_rtk > 1.0 {
        per_mob_dmg /= n_rtk;
    }
    if per_mob_dmg < 0.0 {
        per_mob_dmg = 0.0;
    }

    // Round-by-round HP loss with ramp-down and a per-round threshold.
    let mut hp_lost_per_lair = 0.0;
    if !unlimited {
        let mut rounds_ceil = clng(ceph_c_ceil(rtc)).map_err(vb)?;
        if rounds_ceil < 1 {
            rounds_ceil = 1;
        }
        for r in 1..=rounds_ceil {
            let killed_before = (f64::from(r - 1) / rtk_avg).floor();
            let mut mobs_alive = n_num_mobs - killed_before;
            if mobs_alive < 1.0 {
                mobs_alive = 1.0;
            }
            let mut incoming = (mobs_alive * per_mob_dmg) - f64::from(n_damage_threshold);
            if incoming < 0.0 {
                incoming = 0.0;
            }
            if r == rounds_ceil {
                let mut frac = rtc - f64::from(rounds_ceil - 1);
                if frac <= 0.0 {
                    frac = 1.0;
                }
                if frac > 1.0 {
                    frac = 1.0;
                }
                incoming *= frac;
            }
            hp_lost_per_lair += incoming;
        }
        hp_lost_per_lair *= knobs.dmg;
        if hp_lost_per_lair < 0.0 {
            hp_lost_per_lair = 0.0;
        }
    }

    // MP spent per lair.
    let mut eff_spell_per_round =
        (f64::from(input.n_spell_cost) + input.n_spell_overhead) * knobs.mana;
    if eff_spell_per_round < 0.0 {
        eff_spell_per_round = 0.0;
    }
    let mp_spent_per_lair = if unlimited {
        0.0
    } else {
        eff_spell_per_round * rtc
    };

    // Movement.
    let mut move_secs;
    if is_boss || instant {
        move_secs = 0.0;
    } else {
        move_secs = input.n_avg_walk * n_walk_speed * knobs.mv;
        if move_secs < 0.0 {
            move_secs = 0.0;
        }
    }

    // Recovery rates (HP regen is in resting-rate form).
    let passive_hp_ps = (f64::from(input.n_char_hp_regen) / 3.0) / SEC_PER_REGEN_TICK;
    let mut rest_hp_ps = f64::from(input.n_char_hp_regen) / SEC_PER_REST_TICK;
    rest_hp_ps += passive_hp_ps;

    let passive_mp_ps = f64::from(input.n_char_mp_regen) / SEC_PER_REGEN_TICK;
    let mut med_mp_ps = 0.0;
    if input.n_meditate_rate > 0 {
        med_mp_ps = MEDITATE_EFF * (f64::from(input.n_meditate_rate) / SEC_PER_MEDI_TICK);
    }
    med_mp_ps += passive_mp_ps;
    if let Some(b) = qs {
        // rest or meditate: the mode the character can do; the rate holds the standing ticks already
        med_mp_ps =
            qs_recovery_mana_per_sec(b, input.n_meditate_rate > 0, input.n_char_mp_regen);
    }

    let hp_max = f64::from(input.n_char_hp);
    let mp_max = f64::from(n_char_mana);
    let use_hp = !unlimited && hp_max > 0.0 && hp_lost_per_lair > 0.0;
    let use_mp = !unlimited && mp_max > 0.0 && mp_spent_per_lair > 0.0;

    // Serialized recovery macro-cycle.
    let mut rest_hp_secs_per_lair = 0.0;
    let mut med_mp_secs_per_lair = 0.0;
    if !is_boss && (use_hp || use_mp) {
        let th_hp = HP_REST_START_FRAC * hp_max;
        let tg_hp = HP_REST_TARGET_FRAC * hp_max;
        let th_mp = MP_REST_START_FRAC * mp_max;
        let tg_mp = MP_REST_TARGET_FRAC * mp_max;

        let mut hp_cur = hp_max;
        let mut mp_cur = mp_max;
        let mut lairs: i32 = 0;
        let mut tot_rest_hp = 0.0;
        let mut tot_med_mp = 0.0;

        for _ in 1..=MAX_LAIRS_PER_CYCLE {
            if move_secs > 0.0 {
                hp_cur += passive_hp_ps * move_secs;
                if hp_cur > hp_max {
                    hp_cur = hp_max;
                }
                mp_cur += passive_mp_ps * move_secs;
                if mp_cur > mp_max {
                    mp_cur = mp_max;
                }
            }
            if use_hp {
                hp_cur = hp_cur - hp_lost_per_lair + passive_hp_ps * attack_secs;
                if hp_cur > hp_max {
                    hp_cur = hp_max;
                }
                if hp_cur < 0.0 {
                    hp_cur = 0.0;
                }
            }
            if use_mp {
                mp_cur = mp_cur - mp_spent_per_lair + passive_mp_ps * attack_secs;
                if mp_cur > mp_max {
                    mp_cur = mp_max;
                }
                if mp_cur < 0.0 {
                    mp_cur = 0.0;
                }
            }
            lairs += 1;
            if use_hp && hp_cur <= th_hp {
                break;
            }
            if use_mp && mp_cur <= th_mp {
                break;
            }
        }
        if lairs < 1 {
            lairs = 1;
        }

        if qs.is_some() {
            // one phase: HP and mana come back together; the phase lasts as long as the slower bar
            let hp_missing = if use_hp { tg_hp - hp_cur } else { 0.0 };
            let mp_missing = if use_mp { tg_mp - mp_cur } else { 0.0 };
            let (hp_secs, mp_secs, _) =
                qs_recovery_secs(hp_missing, rest_hp_ps, mp_missing, med_mp_ps);
            tot_rest_hp += hp_secs;
            tot_med_mp += mp_secs;
        } else if use_hp && hp_cur < tg_hp && rest_hp_ps > 0.0 {
            let mut rest_secs = (tg_hp - hp_cur) / rest_hp_ps;
            if rest_secs < 0.0 {
                rest_secs = 0.0;
            }
            tot_rest_hp += rest_secs;
            mp_cur += passive_mp_ps * rest_secs;
            if mp_cur > mp_max {
                mp_cur = mp_max;
            }
        }
        if qs.is_none() && use_mp && mp_cur < tg_mp && med_mp_ps > 0.0 {
            let mut med_secs = (tg_mp - mp_cur) / med_mp_ps;
            if med_secs < 0.0 {
                med_secs = 0.0;
            }
            tot_med_mp += med_secs;
            // MME also credits passive HP here, but the value is never read afterwards.
        }

        rest_hp_secs_per_lair = tot_rest_hp / f64::from(lairs);
        med_mp_secs_per_lair = tot_med_mp / f64::from(lairs);
    }

    // Realism terms: heavy-hit rest relief, then per-kill overhead.
    let mut overhead_secs = 0.0;
    if !is_boss {
        if hp_max > 0.0 && rest_hp_secs_per_lair > 0.0 {
            let heavy_frac = hp_lost_per_lair / hp_max;
            let rest_relief = HEAVY_REST_RELIEF * ceph_b_smooth_step(0.25, 0.5, heavy_frac);
            rest_hp_secs_per_lair *= 1.0 - rest_relief;
        }
        if qs.is_some() {
            // the phase is the longer time: mana is charged only for what it adds beyond the HP time
            med_mp_secs_per_lair = (med_mp_secs_per_lair - rest_hp_secs_per_lair).max(0.0);
        }
        let rec_secs = rest_hp_secs_per_lair + med_mp_secs_per_lair;
        let active_secs = attack_secs + move_secs;
        let rec_frac = safe_div(rec_secs, active_secs + rec_secs, 0.0);
        let oh_gate = 1.0 - ceph_b_smooth_step(0.1, 0.35, rec_frac);
        overhead_secs = KILL_OVERHEAD_SEC * n_num_mobs * oh_gate;
        if overhead_secs < 0.0 {
            overhead_secs = 0.0;
        }
    }

    // Cycle seconds per lair.
    let cycle_secs;
    if is_boss {
        let mut regen_secs = n_regen_time * 3600.0;
        if regen_secs < attack_secs {
            regen_secs = attack_secs;
        }
        cycle_secs = regen_secs;
        move_secs = 0.0;
        rest_hp_secs_per_lair = 0.0;
        med_mp_secs_per_lair = 0.0;
    } else {
        cycle_secs =
            attack_secs + move_secs + rest_hp_secs_per_lair + med_mp_secs_per_lair + overhead_secs;
    }
    if cycle_secs <= 0.0 {
        return Ok(ret);
    }

    // Per-hour assembly with spawn-supply gating.
    let mut lairs_per_hour = 3600.0 / cycle_secs;
    if n_total_lairs > 0 && n_regen_time > 0.0 {
        let lairs_supply = f64::from(n_total_lairs) * (60.0 / (n_regen_time + 0.25));
        if lairs_supply > 0.0 && lairs_per_hour > lairs_supply {
            lairs_per_hour = lairs_supply;
        }
    }
    if lairs_per_hour < 0.0 {
        lairs_per_hour = 0.0;
    }

    let attack_ph = (attack_secs + overhead_secs) * lairs_per_hour;
    let move_ph = move_secs * lairs_per_hour;
    let rest_hp_ph = rest_hp_secs_per_lair * lairs_per_hour;
    let rest_mp_ph = med_mp_secs_per_lair * lairs_per_hour;
    let mut roam_ph = 3600.0 - (attack_ph + move_ph + rest_hp_ph + rest_mp_ph);
    if roam_ph < 0.0 {
        roam_ph = 0.0;
    }
    let used_ph = attack_ph + move_ph + rest_hp_ph + rest_mp_ph + roam_ph;
    if used_ph <= 0.0 {
        return Ok(ret);
    }

    ret.n_exp_per_hour = n_exp.to_f64() * lairs_per_hour;
    if !is_boss {
        ret.n_exp_per_hour *= knobs.xp;
    }

    ret.n_attack_time = attack_ph / used_ph;
    ret.n_move = move_ph / used_ph;
    ret.n_roam_time = roam_ph / used_ph;
    ret.n_hitpoint_recovery = rest_hp_ph / used_ph;
    ret.n_mana_recovery = rest_mp_ph / used_ph;
    ret.n_time_recovering = (rest_hp_ph + rest_mp_ph) / used_ph;
    ret.n_overkill = ceph_d_overkill_frac(per_mob_hp, n_char_dmg, input.n_char_first_round_dmg);
    ret.n_slowdown_time = (rtk_avg - 1.0) / rtk_avg;
    ret.n_slowdown_time = ret.n_slowdown_time.clamp(0.0, 1.0);
    ret.n_rtc = rtc;

    let pct =
        |x: f64| -> Result<String, RulesError> { Ok(format(x * 100.0, "0.0").map_err(vb)? + "%") };
    ret.s_hitpoint_recovery = pct(ret.n_hitpoint_recovery)?;
    ret.s_mana_recovery = pct(ret.n_mana_recovery)?;
    ret.s_time_recovering = pct(ret.n_time_recovering)?;
    ret.s_move_text = pct(ret.n_move)?;
    ret.s_rtc_text = format!("RTC {}", format(ret.n_rtc, "0.00").map_err(vb)?);

    Ok(ret)
}
