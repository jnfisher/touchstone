//! Model A: the closed-form exp/hr model (q-elasticity HP recovery, mana-pool model,
//! density-based movement, overlap credits). Legacy, but one of the four selectable models.
//!
//! Split along MME's section banners: [`surprise`] is the opener section, [`helpers`] the two
//! `cephA_*` procedures; the rest (validation, HP, mana, movement, overlap, totals) is
//! [`model_a`] itself, in MME's order because later sections read earlier ones' locals.

// MME's explicit compare-and-assign steps stay as written: they keep the order of operations
// visible, and `clamp` would panic on a NaN bound where VB6 just compares.
#![allow(clippy::manual_clamp, clippy::needless_late_init)]

use touchstone_vb6::{Vb6Error, add_i32, div, fix, round_half_even};

use super::helpers::{
    SEC_PER_MEDI_TICK, SEC_PER_REGEN_TICK, SEC_PER_REST_TICK, SEC_PER_ROUND, max_dbl, safe_div,
};
use super::{ExpPerHourInfo, ExpPerHourInput, ExpPerHourKnobs, UNSUPPORTED_UNDER_QUICKSILVER};
use crate::stats::regen_on;
use crate::{Profile, RulesError};

mod helpers;
mod surprise;

use helpers::{HP_PASSIVE_COMBAT_EFF, calc_hp_recovery_rounds, in_combat_mp_frac};

// Public constants of the modExpPerHour declarations block that only Model A reads.
const DEFAULT_CEPHA_DMG: f64 = 1.0;
const DEFAULT_CEPHA_MANA: f64 = 1.25;
const DEFAULT_CEPHA_MOVE: f64 = 1.0;
const DEFAULT_CEPHA_MOVE_RECOVER: f64 = 0.5;
const DEFAULT_CEPHA_CLUSTER_MX: i32 = 10;
/// `STOCK_MOB_HPREGEN_ROUNDS` (GreaterMUD uses 6 and is not ported).
const STOCK_MOB_HPREGEN_ROUNDS: f64 = 18.0;
/// `MED_EFF_FACTOR`, the share of the meditate rate that counts.
const MED_EFF_FACTOR: f64 = 0.5;

/// Port of modExpPerHour `ceph_ModelA`.
///
/// MME's `On Error GoTo error ... Resume out` swallows any VB6 run-time error and returns the
/// result as assigned so far. The result is assigned at the very end (or in the boss shortcut,
/// which cannot fail afterwards), so an error gives an empty [`ExpPerHourInfo`], never `Err`.
/// Only the numeric fields except `n_rtc` are set; the sign of `n_move` flags limited movement and
/// the sign of `n_attack_time` a longer-than-normal surprise opener, as `CalcExpPerHour` expects.
///
/// # Errors
/// [`RulesError::Unsupported`] under a Quicksilver balance whose rest and meditate multiples are on.
pub fn model_a(
    input: &ExpPerHourInput,
    knobs: &ExpPerHourKnobs,
    profile: Profile,
) -> Result<ExpPerHourInfo, RulesError> {
    if regen_on(profile).is_some() {
        return Err(RulesError::Unsupported(UNSUPPORTED_UNDER_QUICKSILVER));
    }
    Ok(run(input, knobs).unwrap_or_default())
}

#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
fn run(input: &ExpPerHourInput, knobs: &ExpPerHourKnobs) -> Result<ExpPerHourInfo, Vb6Error> {
    let mut ret = ExpPerHourInfo::default();

    let n_exp = input.n_exp;
    let mut n_regen_time = input.n_regen_time;
    let mut n_num_mobs = input.n_num_mobs;
    let n_total_lairs = input.n_total_lairs;
    let n_poss_spawns = input.n_poss_spawns;
    let mut n_rtk = input.n_rtk;
    let n_char_dmg = input.n_char_dmg;
    let mut n_char_hp = input.n_char_hp;
    let mut n_char_hp_regen = input.n_char_hp_regen;
    let n_mob_dmg = input.n_mob_dmg;
    let mut n_mob_hp = input.n_mob_hp;
    let n_mob_hp_regen = input.n_mob_hp_regen;
    let mut n_damage_threshold = input.n_damage_threshold;
    let n_spell_cost = input.n_spell_cost;
    let n_spell_overhead = input.n_spell_overhead;
    let n_char_mana = input.n_char_mana;
    let n_char_mp_regen = input.n_char_mp_regen;
    let n_meditate_rate = input.n_meditate_rate;
    let n_avg_walk = input.n_avg_walk;
    let n_walk_speed = input.n_walk_speed;
    let n_surprise_dmg = input.n_surprise_dmg;

    // Fast bail-outs.
    if n_exp.scaled() == 0 {
        return Ok(ret);
    }
    if n_total_lairs == 0 && n_regen_time == 0.0 {
        return Ok(ret);
    }

    // Exactly -1 means "basic damage only, no recovery"; other negatives are not special.
    let mut b_basic_damage = false;
    if n_damage_threshold == -1 {
        b_basic_damage = true;
        n_damage_threshold = 2_000_000_000;
    }
    let threshold = f64::from(n_damage_threshold);

    // Cluster detection (i.e. a gnoll encampment) limits movement.
    let mut b_limit_movement = false;
    if n_avg_walk > 0.0
        && n_avg_walk <= 2.0
        && n_total_lairs > 0
        && n_poss_spawns > n_total_lairs
        && f64::from(n_poss_spawns) / f64::from(n_total_lairs)
            >= f64::from(DEFAULT_CEPHA_CLUSTER_MX)
    {
        b_limit_movement = true;
    }

    let mut n_secs_per_room = n_walk_speed;
    if n_secs_per_room < 1.0 {
        n_secs_per_room = 1.0;
    }
    if n_secs_per_room > 2.0 {
        n_secs_per_room = 2.0;
    }

    let n_mob_hp_regen_rounds = STOCK_MOB_HPREGEN_ROUNDS;

    // Validation.
    if n_char_hp < 1 {
        n_char_hp = 1;
    }
    if n_mob_hp < 1 {
        n_mob_hp = 1;
    }
    if n_char_hp_regen < 1 {
        n_char_hp_regen = 1;
    }
    if n_regen_time < 0.0 {
        n_regen_time = 0.0;
    }
    if n_regen_time > 60.0 {
        n_regen_time = 60.0;
    }

    // No canonical RTK: derive it with the round-up-to-0.5 rule.
    if n_rtk == 0.0 && n_char_dmg > 0.0 && n_mob_hp > 0 {
        let hp_per_mob = if n_num_mobs > 1.0 {
            f64::from(n_mob_hp) / n_num_mobs
        } else {
            f64::from(n_mob_hp)
        };
        n_rtk = hp_per_mob / n_char_dmg;
        if n_rtk > 1.0 {
            n_rtk = -(-(n_rtk * 2.0)).floor() / 2.0;
        }
    }
    if n_rtk < 1.0 {
        n_rtk = 1.0;
    }

    if n_num_mobs < 1.0 {
        n_num_mobs = 1.0;
    }
    let n_rtc = n_rtk * n_num_mobs;
    let mob_hp = f64::from(n_mob_hp);

    // Surprise opener (either/or; can add or subtract rounds).
    let mut n_mob_dmg_use = n_mob_dmg;
    let mut n_rtc_eff = n_rtc;
    let mut b_surprise_less = false;
    if n_surprise_dmg > 0.0 {
        let s = surprise::opener(&surprise::SurpriseArgs {
            n_surprise_dmg,
            n_num_mobs,
            n_mob_hp: mob_hp,
            n_char_dmg,
            n_rtk,
            n_rtc,
            n_mob_dmg,
            n_mob_hp_regen: f64::from(n_mob_hp_regen),
            n_mob_hp_regen_rounds,
        })?;
        n_rtc_eff = s.n_rtc_eff;
        n_mob_dmg_use = s.n_mob_dmg_use;
        b_surprise_less = s.b_surprise_less;
    }

    // NPC / boss shortcut.
    if n_total_lairs <= 0 && n_regen_time > 0.0 {
        let eff_clears_per_hour = div(1.0, n_regen_time)?;
        ret.n_exp_per_hour = round_half_even(n_exp.to_f64() * eff_clears_per_hour);
        return Ok(ret);
    }

    // The lair regen happens by time of day, so a quarter minute is added on average.
    if n_regen_time > 0.0 {
        n_regen_time += 0.25;
    }

    // Attack time and over-damage.
    let kill_time_sec = n_rtc_eff * SEC_PER_ROUND;
    let mut overshoot_frac = 0.0;
    if n_char_dmg > 0.0 {
        let total_damage = n_rtk * n_char_dmg;
        let effective_mob_hp = if n_num_mobs > 1.0 {
            mob_hp / n_num_mobs
        } else {
            mob_hp
        };
        if total_damage > effective_mob_hp {
            overshoot_frac = ((total_damage - effective_mob_hp) / total_damage) * 0.8;
        } else if effective_mob_hp > 0.0 && n_rtk != fix(n_rtk) {
            overshoot_frac = ((n_rtk - fix(n_rtk)).abs() * n_char_dmg) / effective_mob_hp;
        }
    }

    // Locals that the later sections read even when an earlier one is skipped (VB6 zeroes them).
    let mut n_hitpoint_recovery_time_sec = 0.0;
    let mut n_mana_recovery_time_sec = 0.0;
    let mut rounds_hitpoints = 0.0;
    let mut recovery_demand_frac = 0.0;
    let mut recovery_demand_time = 0.0;
    let mut recovery_time_sec = 0.0;
    let mut recovery_credit_sec = 0.0;
    let mut b_hp_fully_sustained = false;
    let mut n_local_dmg_scale_factor = 0.0;
    let mut mp_per_sec_regen = 0.0;
    let mut cost_room = 0.0;
    let mut regen_room = 0.0;
    let mut t_refill = 0.0;
    let mut p_travel = 0.0;

    if !b_basic_damage {
        // HP recovery.
        let mut dmg_in_hp = n_mob_dmg_use;
        if threshold > 0.0 {
            dmg_in_hp -= threshold;
            if dmg_in_hp < 0.0 {
                dmg_in_hp = 0.0;
            }
        }
        let q_ratio = if dmg_in_hp > 0.0 {
            safe_div(f64::from(n_char_hp_regen), dmg_in_hp, 0.0)
        } else {
            9999.0
        };
        n_local_dmg_scale_factor = if q_ratio < 0.3 {
            1.2
        } else if (0.3..=0.7).contains(&q_ratio) {
            1.05 + (0.375 * (0.7 - q_ratio))
        } else {
            1.0
        };

        // When net damage is tiny against regen, HP is self-sustained: no explicit rest.
        if threshold > 0.0 && (dmg_in_hp <= 0.0001 || q_ratio >= 4.0) {
            b_hp_fully_sustained = true;
        }

        if b_hp_fully_sustained {
            rounds_hitpoints = 0.0;
        } else {
            let mut dmg_per_round = dmg_in_hp;
            if dmg_per_round < 0.05 {
                dmg_per_round = 0.0;
            }
            rounds_hitpoints = if dmg_per_round > 0.0 {
                calc_hp_recovery_rounds(
                    dmg_in_hp,
                    n_char_dmg,
                    mob_hp,
                    f64::from(n_char_hp_regen),
                    n_num_mobs,
                    n_rtc_eff,
                )?
            } else {
                0.0
            };
        }

        n_hitpoint_recovery_time_sec = rounds_hitpoints * SEC_PER_ROUND;
        if n_hitpoint_recovery_time_sec < 0.0 {
            n_hitpoint_recovery_time_sec = 0.0;
        }
        let mut n_hitpoint_recovery = 0.0;
        if (kill_time_sec + n_hitpoint_recovery_time_sec) > 0.0 {
            n_hitpoint_recovery =
                n_hitpoint_recovery_time_sec / (kill_time_sec + n_hitpoint_recovery_time_sec);
        }
        if n_hitpoint_recovery > 1.0 {
            n_hitpoint_recovery = 1.0;
        }
        if n_hitpoint_recovery < 0.0 {
            n_hitpoint_recovery = 0.0;
        }

        // Mana recovery (per-room pool model).
        mp_per_sec_regen = f64::from(n_char_mp_regen) / SEC_PER_REGEN_TICK;
        // Meditation is not fully efficient (interrupts, stutter steps).
        let mp_per_sec_meditate =
            mp_per_sec_regen + MED_EFF_FACTOR * (f64::from(n_meditate_rate) / SEC_PER_MEDI_TICK);

        let mp_use_frac = in_combat_mp_frac(n_meditate_rate, n_total_lairs, n_avg_walk);
        // A direct mana attack pays the overhead on every cast; otherwise only a fraction.
        let mp_cost_frac = if n_spell_cost > 0 { 1.0 } else { mp_use_frac };
        cost_room =
            (f64::from(n_spell_cost) * n_rtc_eff) + (n_spell_overhead * n_rtc_eff * mp_cost_frac);

        regen_room = mp_per_sec_regen * kill_time_sec;
        let mut drain_room = cost_room - regen_room;
        if drain_room < 0.0 {
            drain_room = 0.0;
        }

        let rooms_per_pool = if drain_room == 0.0 || n_char_mana == 0 {
            1e30
        } else {
            f64::from(n_char_mana) / drain_room
        };

        // Time to refill 95% of the pool.
        let refill_target = 0.95 * f64::from(n_char_mana);
        if mp_per_sec_meditate > 0.0 {
            t_refill = refill_target / mp_per_sec_meditate;
        }

        let t_rest_avg = div(t_refill, rooms_per_pool)?;
        n_mana_recovery_time_sec = t_rest_avg * DEFAULT_CEPHA_MANA * knobs.mana;
        if n_mana_recovery_time_sec < 0.0 {
            n_mana_recovery_time_sec = 0.0;
        }

        let mut n_mana_recovery = 0.0;
        if (kill_time_sec + n_mana_recovery_time_sec) > 0.0 {
            n_mana_recovery = n_mana_recovery_time_sec / (kill_time_sec + n_mana_recovery_time_sec);
        }
        if n_mana_recovery > 1.0 {
            n_mana_recovery = 1.0;
        }

        if n_mana_recovery == 1.0 {
            n_mana_recovery_time_sec = kill_time_sec * 2.0;
        } else if n_mana_recovery > 0.0 && n_mana_recovery_time_sec == 0.0 {
            n_mana_recovery_time_sec = kill_time_sec * (n_mana_recovery / (1.0 - n_mana_recovery));
        }
        if n_mana_recovery_time_sec < 0.0 {
            n_mana_recovery_time_sec = 0.0;
        }

        // Combine HP and mana demand (no movement overlap yet).
        recovery_demand_frac =
            n_hitpoint_recovery + n_mana_recovery - (n_hitpoint_recovery * n_mana_recovery);
        if recovery_demand_frac < 0.0 {
            recovery_demand_frac = 0.0;
        }
        if recovery_demand_frac > 1.0 {
            recovery_demand_frac = 1.0;
        }
        if recovery_demand_frac > 0.0 && recovery_demand_frac < 1.0 {
            recovery_demand_time =
                kill_time_sec * (recovery_demand_frac / (1.0 - recovery_demand_frac));
        } else if recovery_demand_frac >= 1.0 {
            // Pathological; clipped by the spawn gate later.
            recovery_demand_time = 3600.0;
        } else {
            recovery_demand_time = 0.0;
        }
    }

    // Movement model.
    let move_base_sec;
    if n_total_lairs > 0 && !b_limit_movement {
        let rooms_raw = add_i32(n_poss_spawns, n_total_lairs)?;
        let rooms_raw_f = f64::from(rooms_raw);
        let lairs_f = f64::from(n_total_lairs);
        let mut density_p;
        let mut effective_lairs;
        let mut rooms_scaled;
        let max_rooms;
        if n_regen_time > 0.0 {
            effective_lairs = (60.0 * n_regen_time) / 5.0;
            if effective_lairs > lairs_f {
                rooms_scaled = rooms_raw_f * (lairs_f / effective_lairs);
                effective_lairs = lairs_f;
            } else {
                rooms_scaled = rooms_raw_f;
            }
            max_rooms = effective_lairs * (60.0 / n_regen_time);
        } else {
            effective_lairs = lairs_f;
            rooms_scaled = rooms_raw_f;
            max_rooms = 720.0; // one lair every 5 seconds for an hour
        }
        if rooms_scaled > max_rooms {
            rooms_scaled = max_rooms;
        }

        // Density of lairs among walkable rooms after scaling.
        if rooms_scaled <= 0.0 {
            density_p = 1.0;
        } else {
            density_p = effective_lairs / rooms_scaled;
            if density_p < 0.01 {
                density_p = 0.01;
            }
            if density_p > 1.0 {
                density_p = 1.0;
            }
        }

        p_travel = div(lairs_f, rooms_raw_f)?;
        if p_travel < 0.0001 {
            p_travel = 0.0001;
        }
        if p_travel > 1.0 {
            p_travel = 1.0;
        }

        let n_route_bias_local = if p_travel < 0.1 {
            0.7 + (3.0 * p_travel)
        } else if p_travel < 0.18 {
            if density_p > 0.5 {
                1.08
            } else if (0.25..=0.4).contains(&density_p) {
                0.85
            } else {
                1.02
            }
        } else {
            0.98
        };

        // Tuners.
        const MOVE_TARGET_SECS: f64 = 2.2;
        const DENSITY_COEF: f64 = 0.2;
        let mut n_room_density_coef = DENSITY_COEF;
        if DEFAULT_CEPHA_MOVE > 0.0 {
            n_room_density_coef *= DEFAULT_CEPHA_MOVE;
        }
        if knobs.mv > 0.0 {
            n_room_density_coef *= knobs.mv;
        }

        // Density-aware effective seconds per room.
        let target_factor = MOVE_TARGET_SECS / n_secs_per_room;
        let scale_factor;
        if density_p != 0.0 && n_room_density_coef != 0.0 && n_room_density_coef != 1.0 {
            let mut s = 1.0
                + ((1.0 / density_p - 1.0) / (1.0 / n_room_density_coef - 1.0))
                    * (target_factor - 1.0);
            if s < 1.0 {
                s = 1.0;
            }
            scale_factor = s;
        } else if n_room_density_coef == 0.0 {
            scale_factor = 0.00001;
        } else {
            scale_factor = target_factor;
        }
        let secs_per_room_eff = n_secs_per_room * scale_factor;

        let n_move_bias = 0.75;

        // Spawn-based: patrol density (map truth), not the post-scaled density.
        let mut dens_for_spawn = p_travel;
        if dens_for_spawn < 0.0001 {
            dens_for_spawn = 0.0001;
        }
        if dens_for_spawn > 1.0 {
            dens_for_spawn = 1.0;
        }
        let move_spawn_based =
            ((1.0 - dens_for_spawn) / dens_for_spawn) * secs_per_room_eff * n_move_bias;

        // Route-based.
        let mut move_route_based = ((rooms_raw_f / lairs_f) - 1.0)
            * n_secs_per_room
            * n_route_bias_local
            * DEFAULT_CEPHA_MOVE
            * knobs.mv;

        // Soften the 0.20-0.30 pTravel uplift.
        if (0.2..=0.3).contains(&p_travel) {
            move_route_based *= 1.0 + 0.1 * (p_travel - 0.2) / 0.1;
        }
        if density_p > 0.8 && move_route_based < 2.0 * n_secs_per_room {
            move_route_based = 2.0 * n_secs_per_room;
        }
        if move_route_based < 0.0 {
            move_route_based = 0.0;
        }

        // The larger one: the walk can't undercut the loop.
        move_base_sec = if move_route_based > move_spawn_based {
            move_route_based
        } else {
            move_spawn_based
        };
    } else if b_limit_movement {
        move_base_sec = n_secs_per_room * n_avg_walk;
    } else {
        move_base_sec = n_secs_per_room;
    }

    if !b_basic_damage {
        // Walk-credit rolled back into the mana model.
        let regen_walk_raw = mp_per_sec_regen * move_base_sec;
        let mut walk_scale = 1.0;
        let mut regen_walk;
        if b_limit_movement {
            regen_walk = regen_walk_raw * 0.25;
        } else {
            // Density dampener: 0.40 to 0.90 for pTravel 0 to 0.25.
            if p_travel < 0.25 {
                walk_scale = 0.4 + (2.0 * p_travel);
                if walk_scale > 0.9 {
                    walk_scale = 0.9;
                }
            }
            regen_walk = regen_walk_raw * walk_scale;
        }
        let walk_cap = if kill_time_sec <= 6.0 {
            0.7 * kill_time_sec * mp_per_sec_regen // micro-rooms
        } else {
            0.4 * kill_time_sec * mp_per_sec_regen
        };
        if regen_walk > walk_cap {
            regen_walk = walk_cap;
        }

        regen_room += regen_walk;
        let mut drain_room = cost_room - regen_room;
        if drain_room < 0.0 {
            drain_room = 0.0;
        }
        let rooms_per_pool = if drain_room == 0.0 {
            1e30
        } else {
            let mut r = f64::from(n_char_mana) / drain_room;
            if r < 1.0 {
                r = 1.0;
            }
            r
        };
        let t_rest_avg = div(t_refill, rooms_per_pool)?;
        n_mana_recovery_time_sec = t_rest_avg * DEFAULT_CEPHA_MANA * knobs.mana;
        if n_mana_recovery_time_sec < 0.0 {
            n_mana_recovery_time_sec = 0.0;
        }

        // Overlap credits.
        let mut rest_secs_loop_a = 0.0;
        let mut rest_rounds_loop_a = 0.0;
        if n_mana_recovery_time_sec <= 0.0
            && !b_hp_fully_sustained
            && rounds_hitpoints <= 0.0
            && n_mob_dmg_use > 0.0
            && n_total_lairs > 0
        {
            // Loop-level HP fallback: sustained drip damage over a long chain of small hits.
            let combat_secs_a = max_dbl(0.0, n_rtc_eff) * SEC_PER_ROUND;
            let passive_combat_hp_a = HP_PASSIVE_COMBAT_EFF
                * (f64::from(n_char_hp_regen) / 3.0)
                * safe_div(combat_secs_a, SEC_PER_REGEN_TICK, 0.0);

            let mut dmg_per_round_hp = n_mob_dmg_use;
            if threshold > 0.0 {
                dmg_per_round_hp -= threshold;
                if dmg_per_round_hp < 0.0 {
                    dmg_per_round_hp = 0.0;
                }
            }
            let dmg_total_a = dmg_per_round_hp * max_dbl(1.0, n_rtc_eff);
            let lair_net_hp_a = max_dbl(0.0, dmg_total_a - passive_combat_hp_a);
            // A tiny buffer so it doesn't over-trigger.
            let loop_deficit_hp_a = max_dbl(
                0.0,
                lair_net_hp_a * f64::from(n_total_lairs) - (0.02 * f64::from(n_char_hp)),
            );
            let rest_hp_per_sec_a = (f64::from(n_char_hp_regen) / SEC_PER_REST_TICK)
                + ((f64::from(n_char_hp_regen) / 3.0) / SEC_PER_REGEN_TICK);
            rest_secs_loop_a = safe_div(loop_deficit_hp_a, max_dbl(0.0001, rest_hp_per_sec_a), 0.0);
            rest_rounds_loop_a = safe_div(rest_secs_loop_a, SEC_PER_ROUND, 0.0);
            if rest_rounds_loop_a > rounds_hitpoints {
                rounds_hitpoints = rest_rounds_loop_a;
            }
        }

        // Promote the loop fallback to the authoritative HP time.
        if rest_secs_loop_a > 0.0 {
            n_hitpoint_recovery_time_sec = max_dbl(n_hitpoint_recovery_time_sec, rest_secs_loop_a);
            rounds_hitpoints = max_dbl(rounds_hitpoints, rest_rounds_loop_a);
        }

        let t_hp0 = max_dbl(
            n_hitpoint_recovery_time_sec,
            rounds_hitpoints * SEC_PER_ROUND,
        );
        let t_m0 = n_mana_recovery_time_sec;

        // Walk/rest overlap: credit up to the lesser of the needed rest and the walk window.
        let t_hp = t_hp0;
        let tmp = t_m0;
        let walk_window_sec = move_base_sec;

        // Only passive regen works while walking, so each second offsets rest by this ratio.
        let passive_per_sec_hp = (f64::from(n_char_hp_regen) / 3.0) / SEC_PER_REGEN_TICK;
        let rest_per_sec_hp = (f64::from(n_char_hp_regen) / SEC_PER_REST_TICK) + passive_per_sec_hp;
        let mut hp_walk_eq = 0.0;
        if rest_per_sec_hp > 0.0 {
            hp_walk_eq = passive_per_sec_hp / rest_per_sec_hp;
        }
        let hp_walk_window = if t_hp < walk_window_sec {
            t_hp
        } else {
            walk_window_sec
        };
        let mp_walk_window = if tmp < walk_window_sec {
            tmp
        } else {
            walk_window_sec
        };
        let move_cred_hp = DEFAULT_CEPHA_MOVE_RECOVER * hp_walk_eq * hp_walk_window;

        let passive_per_sec_mp = f64::from(n_char_mp_regen) / SEC_PER_REGEN_TICK;
        let med_per_sec_mp = (f64::from(n_meditate_rate) / SEC_PER_MEDI_TICK) + passive_per_sec_mp;
        let mut mp_walk_eq = 0.0;
        if med_per_sec_mp > 0.0 {
            mp_walk_eq = passive_per_sec_mp / med_per_sec_mp;
        }
        let mut move_cred_mp = DEFAULT_CEPHA_MOVE_RECOVER * mp_walk_eq * mp_walk_window;

        // HP credit first (players rest before meditating); MP gets what is left of the walk.
        let mut walk_left = walk_window_sec - move_cred_hp;
        if walk_left < 0.0 {
            walk_left = 0.0;
        }
        if move_cred_mp > walk_left {
            move_cred_mp = walk_left;
        }

        let mut t_hp1 = t_hp - move_cred_hp;
        if t_hp1 < 0.0 {
            t_hp1 = 0.0;
        }
        let mut t_m1 = tmp - move_cred_mp;
        if t_m1 < 0.0 {
            t_m1 = 0.0;
        }
        recovery_credit_sec = move_cred_hp + move_cred_mp;

        // The HP-rest to meditate conversion is disabled in MME: its credit stays zero.
        let rest_as_mana_eq = 0.0;
        let mut rest_mana_credit = rest_as_mana_eq;
        if rest_mana_credit > t_m1 {
            rest_mana_credit = t_m1;
        }
        let mut t_m2 = t_m1 - rest_mana_credit;
        if t_m2 < 0.0 {
            t_m2 = 0.0;
        }

        // Hard gate: no HP rest when the net damage per round is nothing.
        let mut dmg_per_round_core_a = n_mob_dmg_use;
        if threshold > 0.0 {
            dmg_per_round_core_a -= threshold;
        }
        if dmg_per_round_core_a <= 0.0001 {
            t_hp1 = 0.0;
        }

        // Local and global HP slack, applied here so it reaches all downstream timing.
        let mut hp_scale = n_local_dmg_scale_factor;
        if hp_scale <= 0.0 {
            hp_scale = 1.0;
        }
        if DEFAULT_CEPHA_DMG > 0.0 {
            hp_scale *= DEFAULT_CEPHA_DMG;
        }
        if knobs.dmg > 0.0 {
            hp_scale *= knobs.dmg;
        }

        n_hitpoint_recovery_time_sec = t_hp1 * hp_scale;
        n_mana_recovery_time_sec = t_m2;

        recovery_time_sec = n_hitpoint_recovery_time_sec + n_mana_recovery_time_sec;
        recovery_demand_time = recovery_time_sec;

        if n_hitpoint_recovery_time_sec < 0.0 {
            n_hitpoint_recovery_time_sec = 0.0;
        }
        if n_mana_recovery_time_sec < 0.0 {
            n_mana_recovery_time_sec = 0.0;
        }
        if recovery_time_sec < 0.0 {
            recovery_time_sec = 0.0;
        }
    }

    // Totals before the spawn gate.
    let mut move_time_sec = move_base_sec;
    let mut time_per_clear_sec = kill_time_sec + recovery_time_sec + move_time_sec;

    // Spawn gating / filler wait.
    let mut spawn_interval = 0.0;
    if n_total_lairs > 0 && n_regen_time > 0.0 {
        spawn_interval = (n_regen_time * 60.0) / f64::from(n_total_lairs);
    }
    let time_loss;
    if time_per_clear_sec > 0.0 && spawn_interval > time_per_clear_sec {
        let filler_sec = spawn_interval - time_per_clear_sec;

        // The wait is spent moving or standing, by how sparse the lairs are (never 0 moving).
        let mut filler_to_move_frac = 0.2 + 0.8 * (1.0 - p_travel);
        if filler_to_move_frac < 0.0 {
            filler_to_move_frac = 0.0;
        }
        if filler_to_move_frac > 1.0 {
            filler_to_move_frac = 1.0;
        }
        let filler_move = filler_sec * filler_to_move_frac;
        let filler_stand = filler_sec - filler_move;

        // Standing still also recovers HP and MP.
        let extra_rest_credit = filler_stand * recovery_demand_frac;
        recovery_credit_sec += extra_rest_credit;
        if recovery_credit_sec > recovery_demand_time {
            recovery_credit_sec = recovery_demand_time;
        }
        recovery_time_sec = recovery_demand_time - recovery_credit_sec;
        if recovery_time_sec < 0.0 {
            recovery_time_sec = 0.0;
        }

        move_time_sec = move_base_sec + filler_move;
        time_per_clear_sec = spawn_interval;
        time_loss = filler_sec / spawn_interval;
    } else {
        move_time_sec = move_base_sec;
        time_loss = 0.0;
    }

    // Final exp/hr and fractions.
    let mut eff_clears_per_hour = 0.0;
    if time_per_clear_sec > 0.0 {
        eff_clears_per_hour = 3600.0 / time_per_clear_sec;
    }
    let mut attack_frac = 0.0;
    let mut recover_frac = 0.0;
    let mut hitpoint_frac = 0.0;
    let mut mana_frac = 0.0;
    let mut move_frac = 0.0;
    let mut slowdown_frac = 0.0;
    if time_per_clear_sec > 0.0 {
        attack_frac = kill_time_sec / time_per_clear_sec;
        recover_frac = recovery_time_sec / time_per_clear_sec;
        hitpoint_frac = n_hitpoint_recovery_time_sec / time_per_clear_sec;
        mana_frac = n_mana_recovery_time_sec / time_per_clear_sec;
        move_frac = move_time_sec / time_per_clear_sec;
        if n_rtc_eff > n_num_mobs {
            slowdown_frac = (kill_time_sec - (SEC_PER_ROUND * n_num_mobs)) / time_per_clear_sec;
            if slowdown_frac < 0.0 {
                slowdown_frac = 0.0;
            }
        }
    }

    ret.n_exp_per_hour = round_half_even(n_exp.to_f64() * eff_clears_per_hour) * knobs.xp;
    ret.n_hitpoint_recovery = hitpoint_frac;
    ret.n_mana_recovery = mana_frac;
    ret.n_time_recovering = recover_frac;
    ret.n_overkill = overshoot_frac;
    ret.n_move = move_frac;
    ret.n_attack_time = attack_frac;
    ret.n_slowdown_time = slowdown_frac;
    ret.n_roam_time = time_loss;

    if b_limit_movement {
        ret.n_move *= -1.0;
    }
    if b_surprise_less {
        ret.n_attack_time *= -1.0;
    }
    Ok(ret)
}
