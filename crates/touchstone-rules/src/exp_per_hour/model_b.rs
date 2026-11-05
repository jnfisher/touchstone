//! Model B: the smoothed, band-aware closed form (`ceph_ModelB`).
//!
//! MME's own notes call it overfit. The patches MB1..MB5 and the zone-specific constants are
//! behaviour and are kept as they are, in MME's order of operations.
#![allow(
    clippy::manual_clamp,
    clippy::manual_range_contains,
    clippy::needless_late_init,
    clippy::similar_names,
    clippy::float_cmp,
    unused_assignments
)] // MME's guarded assignments and exact comparisons, kept in its order.

mod travel;

use touchstone_vb6::{Currency, round, round_half_even};

use super::helpers::{
    SEC_PER_REST_TICK, SEC_PER_ROUND, ceph_b_smooth_step, clamp_dbl, max_dbl, min_dbl, safe_div,
};
use super::{ExpPerHourInfo, ExpPerHourInput, ExpPerHourKnobs, vb};
use crate::{Profile, RulesError};
use travel::{
    SlackParams, SlackState, apply_slack_window, band_weight, calc_density, calc_overkill,
    calc_travel_loop_secs, lerp, mul_blend,
};

const DEFAULT_DMG: f64 = 1.0;
const DEFAULT_MANA: f64 = 0.95;
const DEFAULT_MOVE: f64 = 0.9;
const DEFAULT_XP: f64 = 1.0;
const MIN_LOOP: f64 = 22.0;
/// `STOCK_MOB_HPREGEN_ROUNDS` (modMMudFunc): rounds between mob HP-regen ticks.
const STOCK_MOB_HPREGEN_ROUNDS: f64 = 18.0;

/// Port of modSyntaxsFunc `RoundUp` (ceiling via Int).
fn round_up(n: f64) -> f64 {
    if 0.0 < n - n.floor() {
        n.floor() + 1.0
    } else {
        n.floor()
    }
}

/// `IIf(cond, 1#, 0#)`.
fn flag(cond: bool) -> f64 {
    if cond { 1.0 } else { 0.0 }
}

/// Port of modExpPerHour `ceph_ModelB` (stock path).
///
/// Model B reads only the first 21 `CalcExpPerHour` arguments; the surprise-minimum, surprise
/// chance, first-round and minimum-round fields of the input are ignored. It leaves every text
/// field empty (the dispatcher formats the text). As in MME, an early exit or an error that the
/// procedure's own handler swallows returns an empty record, and the boss shortcut returns only
/// the exp/hr. `nCharMana` is an `Integer` here, so a value outside its range is error 6.
///
/// # Errors
/// [`RulesError::Vb6`] 6 when `n_char_mana` does not fit an `Integer`.
#[allow(clippy::too_many_lines)]
pub fn model_b(
    input: &ExpPerHourInput,
    knobs: &ExpPerHourKnobs,
    profile: Profile,
) -> Result<ExpPerHourInfo, RulesError> {
    match profile {
        Profile::Stock => {}
    }
    let n_char_mana = i16::try_from(input.n_char_mana).map_err(|_| RulesError::Vb6(6))?;
    let _ = vb; // formatting errors cannot happen: Model B builds no text.
    Ok(run(input, knobs, n_char_mana).unwrap_or_default())
}

/// The body of `ceph_ModelB`. `None` is a run-time error inside the procedure (its handler
/// resumes at the exit and the function value stays empty).
#[allow(clippy::too_many_lines)]
fn run(
    input: &ExpPerHourInput,
    knobs: &ExpPerHourKnobs,
    n_char_mana: i16,
) -> Option<ExpPerHourInfo> {
    let mut r = ExpPerHourInfo::default();
    let mut b_backstab_less = false;

    let n_exp = input.n_exp;
    let n_regen_time = input.n_regen_time;
    let mut n_num_mobs = input.n_num_mobs;
    let mut n_total_lairs = input.n_total_lairs;
    let n_poss_spawns = input.n_poss_spawns;
    let mut n_rtk = input.n_rtk;
    let n_char_dmg = input.n_char_dmg;
    let n_char_hp = input.n_char_hp;
    let n_char_hp_regen = input.n_char_hp_regen;
    let n_mob_dmg = input.n_mob_dmg;
    let n_mob_hp = input.n_mob_hp;
    let n_mob_hp_regen = input.n_mob_hp_regen;
    let mut n_damage_threshold = input.n_damage_threshold;
    let n_spell_cost = input.n_spell_cost;
    let n_spell_overhead = input.n_spell_overhead;
    let n_char_mp_regen = input.n_char_mp_regen;
    let n_meditate_rate = input.n_meditate_rate;
    let n_avg_walk = input.n_avg_walk;
    let n_walk_speed = input.n_walk_speed;
    let n_surprise_dmg = input.n_surprise_dmg;

    if n_exp.scaled() == 0 {
        return Some(r);
    }

    let mut b_basic_damage = false;
    if n_num_mobs <= 0.0 {
        n_num_mobs = 1.0;
    }
    if n_damage_threshold == -1 {
        b_basic_damage = true;
        n_damage_threshold = 2_000_000_000;
    }

    let n_mob_hp_regen_rounds = STOCK_MOB_HPREGEN_ROUNDS;

    // NPC / boss shortcut
    if n_total_lairs <= 0 && n_regen_time > 0.0 {
        r.n_exp_per_hour = round_half_even(n_exp.to_f64() * (1.0 / n_regen_time));
        return Some(ExpPerHourInfo {
            n_exp_per_hour: r.n_exp_per_hour,
            ..ExpPerHourInfo::default()
        });
    }
    if n_total_lairs == 0 && n_regen_time == 0.0 {
        return Some(ExpPerHourInfo::default());
    }
    let b_instant = n_total_lairs < 0;
    if n_total_lairs <= 0 {
        n_total_lairs = 1;
    }
    let tl = f64::from(n_total_lairs);

    let dens_guess = calc_density(n_total_lairs, n_poss_spawns, n_avg_walk);

    // patch 2025.08.24
    if n_rtk == 0.0 && n_mob_hp > 0 && n_char_dmg > 0.0 {
        if n_num_mobs > 1.0 {
            n_rtk = f64::from(n_mob_hp) / n_num_mobs / n_char_dmg;
            n_rtk = -((-(n_rtk * 2.0)).floor()) / 2.0; // round up to the nearest 0.5
            n_rtk *= n_num_mobs;
        } else {
            n_rtk = round(f64::from(n_mob_hp) / n_char_dmg, 1).ok()?;
        }
    }
    if n_rtk <= 0.0 {
        n_rtk = 1.0;
    }

    // ----- effective RTK -----
    let eff_rtk;
    if n_spell_cost > 0 {
        eff_rtk = max_dbl(min_dbl(n_rtk * 0.78, round_up(n_rtk)), 0.74);
    } else if n_rtk < 2.0 {
        let t_melee = ceph_b_smooth_step(1.2, 1.6, n_rtk);
        let e = n_rtk * (1.0 - 0.1 * t_melee);
        eff_rtk = max_dbl(e, 1.0);
    } else {
        eff_rtk = round_up(n_rtk);
    }

    // ----- effective RTC with Surprise opener (one target per lair) -----
    let rtc_base = eff_rtk * n_num_mobs;
    let mut rtc_adj = rtc_base;

    // The surprise opener replaces the first normal round.
    if n_surprise_dmg > 0.0 && n_num_mobs >= 1.0 {
        let hp_per_mob = safe_div(f64::from(n_mob_hp), max_dbl(1.0, n_num_mobs), 0.0);
        let s_ratio = safe_div(n_surprise_dmg, max_dbl(1.0, hp_per_mob), 0.0);

        // Surprise in units of normal rounds of damage
        let rounds_per_surp = safe_div(n_surprise_dmg, max_dbl(1.0, n_char_dmg), 0.0);

        // Regen reduces opener effectiveness symmetrically (saves less, hurts more)
        let regen_per_round = f64::from(n_mob_hp_regen) / n_mob_hp_regen_rounds;
        let regen_ratio = safe_div(regen_per_round, max_dbl(1.0, n_char_dmg), 0.0);
        let regen_atten = 1.0 - 0.45 * ceph_b_smooth_step(0.0, 0.6, regen_ratio);

        // (roundsPerSurp - 1) is signed: positive saves rounds, negative adds rounds.
        let mut pos_saved = max_dbl(0.0, rounds_per_surp - 1.0) * regen_atten;
        let neg_penalty = max_dbl(0.0, 1.0 - rounds_per_surp) * regen_atten;
        if neg_penalty > 0.0 && n_char_dmg < (f64::from(n_mob_hp) / n_num_mobs) {
            b_backstab_less = true;
        }

        // One-shot gating only helps the savings side
        let p_one_shot = ceph_b_smooth_step(0.85, 1.15, s_ratio);
        pos_saved = min_dbl(eff_rtk, lerp(pos_saved, eff_rtk, p_one_shot));

        // Pack fade (both directions): opener only touches the first target in chaos pulls
        let pack_fade = lerp(
            1.0,
            1.0 / max_dbl(1.0, n_num_mobs).sqrt(),
            ceph_b_smooth_step(3.0, 8.0, n_num_mobs),
        );

        // Signed delta in rounds (positive: faster; negative: slower)
        let mut delta_rounds = (pos_saved - neg_penalty) * pack_fade;

        // Never save or add more than a full effRTK on the first target
        delta_rounds = clamp_dbl(delta_rounds, -eff_rtk, eff_rtk);

        // Apply once per lair
        rtc_adj = eff_rtk * n_num_mobs - delta_rounds;

        // Discrete-round floor: never drop below one normal round for the lair.
        let rtc_floor = eff_rtk * max_dbl(0.0, n_num_mobs - 1.0) + 1.0;
        rtc_adj = max_dbl(rtc_adj, rtc_floor);
    }

    r.n_rtc = rtc_adj;
    let mut kill_secs_per_lair = rtc_adj * SEC_PER_ROUND;

    // -- Over-kill time inflation --
    let mut overkill_factor = calc_overkill(n_char_dmg, n_mob_hp, n_spell_cost > 0);

    // One-shotty blend: when effRTK is ~1, dial down the melee overkill cap
    let mut ok_cap = if n_spell_cost > 0 { 1.06 } else { 1.18 };

    // tOne=1 at effRTK<=1.05, fades to 0 by ~1.20
    let t_one = 1.0 - ceph_b_smooth_step(1.05, 1.2, eff_rtk);

    // Near-one-shot cap to blend toward
    let cap_near_one = if n_spell_cost > 0 { 1.02 } else { 1.06 };

    ok_cap = lerp(ok_cap, cap_near_one, t_one);

    // Overkill cap taper for low-RTK, multi-mob pulls (melee / no-med)
    let t_low_rtk = ceph_b_smooth_step(1.2, 1.6, eff_rtk);
    let ok_cap_mid = lerp(1.12, 1.1, t_low_rtk);
    // Larger pulls waste less per kill; up to ~6% ease by 5+ mobs
    let group_ease = 1.0 - 0.06 * ceph_b_smooth_step(3.0, 5.0, n_num_mobs);

    // Apply only to melee (spells already have a low cap)
    if n_spell_cost == 0 {
        ok_cap = min_dbl(ok_cap, ok_cap_mid) * group_ease;
    }

    // Final clamp and apply
    overkill_factor = min_dbl(overkill_factor, ok_cap);
    kill_secs_per_lair *= overkill_factor;

    // Near-one-shot melee: slightly wider window
    if n_spell_cost == 0 {
        let t_near1 = 1.0 - ceph_b_smooth_step(1.0, 1.4, n_rtk);
        let near_one_cut = lerp(1.0, 0.955, t_near1);
        kill_secs_per_lair *= near_one_cut;
    }

    // Smoothed global chain cut
    let w_chain_l = ceph_b_smooth_step(32.0, 44.0, tl);
    let w_chain_w = 1.0 - ceph_b_smooth_step(2.5, 3.1, n_avg_walk);
    let w_chain = w_chain_l * w_chain_w;
    kill_secs_per_lair = mul_blend(kill_secs_per_lair, 0.97, w_chain);

    let caster_or_overhead = n_spell_cost > 0 || n_spell_overhead > 0.0;
    let no_med_caster_gate = flag(caster_or_overhead && n_meditate_rate == 0);

    // Targeted mid-band spell/no-meditate trim with blend
    let w_mbl = band_weight(tl, 28.0, 40.0, 4.0);
    let w_mbw =
        ceph_b_smooth_step(2.4, 2.6, n_avg_walk) * (1.0 - ceph_b_smooth_step(3.3, 3.7, n_avg_walk));
    let w_mbd =
        ceph_b_smooth_step(1.7, 2.0, dens_guess) * (1.0 - ceph_b_smooth_step(4.0, 4.6, dens_guess));
    let w_mb = w_mbl * w_mbw * w_mbd * no_med_caster_gate;
    // PATCH 2025-08-30: soften the no-med caster kill trim.
    kill_secs_per_lair = mul_blend(kill_secs_per_lair, 0.99, w_mb);

    // === PATCH MB4 (micro kill trim; no-med casters, mid-band only) ===
    let w_rtk_micro_mb4 = 1.0 - ceph_b_smooth_step(1.0, 1.1, eff_rtk);
    let w_mobs_mb4 = 1.0 - ceph_b_smooth_step(1.2, 1.6, n_num_mobs);
    let w_micro_mb4 = w_rtk_micro_mb4 * w_mobs_mb4 * w_mb * no_med_caster_gate;

    // Up to ~22% faster kills at the micro peak; 0% outside the band/gates
    let k_kill_mb4 = lerp(0.78, 1.0, 1.0 - w_micro_mb4);
    kill_secs_per_lair *= k_kill_mb4;

    r.n_overkill = overkill_factor - 1.0;

    let mut walk_loop_secs =
        calc_travel_loop_secs(n_avg_walk, n_total_lairs, n_poss_spawns, n_walk_speed);

    // Use pre-trim travel seconds for (partial) mana-regen credit
    let walk_regen_secs = walk_loop_secs;

    // === PATCH MB1 (micro-route taper; no-med casters, mid-band only) ===
    let is_no_med_caster_mb1 = n_spell_cost > 0 && n_meditate_rate == 0;
    let w_band_mb1 = band_weight(tl, 28.0, 40.0, 4.0)
        * ceph_b_smooth_step(2.4, 2.6, n_avg_walk)
        * (1.0 - ceph_b_smooth_step(3.3, 3.7, n_avg_walk))
        * ceph_b_smooth_step(1.7, 2.0, dens_guess)
        * (1.0 - ceph_b_smooth_step(4.0, 4.6, dens_guess));

    // Micro weight: strong when effRTK~1 and mobs~1; fades if >1 round or >1.2 mobs
    let w_rtk_micro_mb1 = 1.0 - ceph_b_smooth_step(1.0, 1.1, eff_rtk);
    let w_mobs_mb1 = 1.0 - ceph_b_smooth_step(1.2, 1.6, n_num_mobs);
    let w_micro_mb1 = w_rtk_micro_mb1 * w_mobs_mb1 * w_band_mb1;

    if is_no_med_caster_mb1 && w_micro_mb1 > 0.0 {
        // Up to ~12% shrink at hard micro; 0% by 1 full round or >1.2 mobs
        let k_route_mb1 = lerp(0.78, 1.0, 1.0 - w_micro_mb1);
        walk_loop_secs *= k_route_mb1;
    }

    // Ease off the global travel cut when the big-chain ~3-walk band is active
    let w_lx = band_weight(tl, 30.0, 38.0, 3.0);
    let w_wx =
        ceph_b_smooth_step(2.6, 3.2, n_avg_walk) * (1.0 - ceph_b_smooth_step(3.6, 3.9, n_avg_walk));
    let w_cx = w_lx * w_wx;

    let cut_factor = lerp(0.96, 1.0, w_cx);
    walk_loop_secs *= cut_factor;

    // MOVEMENT KNOB
    walk_loop_secs = walk_loop_secs * DEFAULT_MOVE * knobs.mv;

    // High-density, short-walk micro inflation: raise Move% a bit in dense loops
    if dens_guess >= 60.0 && n_avg_walk >= 1.8 && n_avg_walk <= 2.6 {
        let k_dense = lerp(1.0, 1.12, ceph_b_smooth_step(60.0, 90.0, dens_guess));
        walk_loop_secs *= k_dense;
    }

    let regen_window = n_regen_time * 60.0;

    // Actual loop time before recovery. The regen envelope is not floored: a floor would inflate
    // passive HP on short loops.
    let loop_secs_raw = kill_secs_per_lair * tl + walk_loop_secs;
    let regen_envelope = loop_secs_raw;

    // Locals that the recovery sections set; zero when they are skipped, as VB6 locals are.
    let mut rest_secs = 0.0;
    let mut rest_secs_disp = 0.0;
    let mut med_secs = 0.0;
    let mut med_secs_disp = 0.0;
    let mut mana_cost_loop = 0.0;
    let mut mana_gain = 0.0;
    let mut med_needed = 0.0;
    let mut pool_credit = 0.0;
    let mut h_per_mob = 0.0;

    // ===== basic damage only, no recovery =====
    if !b_basic_damage {
        // ===== HP / Rest =====
        let hp_regen = f64::from(n_char_hp_regen);

        // Use the effective lair rounds (post-surprise), not raw nRTK/nNumMobs
        let lair_rounds_eff = max_dbl(1.0, r.n_rtc);
        let dmg_per_round_core = safe_div(n_mob_dmg, lair_rounds_eff, 0.0);

        let mut hp_loss_per_round =
            max_dbl(0.0, dmg_per_round_core - f64::from(n_damage_threshold));

        // HP KNOB
        hp_loss_per_round = hp_loss_per_round * DEFAULT_DMG * knobs.dmg;

        // Per-mob intensity only for gating/smoothing
        let h_lair = hp_loss_per_round;
        h_per_mob = safe_div(h_lair, max_dbl(1.0, n_num_mobs), 0.0);

        // Use only partial surprise credit when estimating *incoming* damage.
        let rtc_for_damage = lerp(rtc_base, r.n_rtc, 0.35);
        let mut hp_loss_per_loop = hp_loss_per_round * rtc_for_damage * tl;

        // Crowd/light-hit exposure: several mobs with low per-mob hits swing more at once.
        let w_crowd = ceph_b_smooth_step(2.0, 4.5, n_num_mobs);
        let w_light = 1.0 - ceph_b_smooth_step(4.0, 12.0, h_per_mob);
        let hp_lift = lerp(1.0, 3.2, w_crowd * w_light);
        hp_loss_per_loop *= hp_lift;

        let w_long_walk = ceph_b_smooth_step(8.0, 12.0, n_avg_walk);

        // Passive regen that is always ticking (trimmed more for tiny hPerMob)
        let passive_coef = 0.02;
        // MB-HPR3: if per-mob hits are light, reduce passive credit slightly (up to -25%)
        let light_hit_trim = 1.0 - 0.25 * (1.0 - ceph_b_smooth_step(8.0, 14.0, h_per_mob));
        let passive_hp =
            (hp_regen * passive_coef * light_hit_trim) * safe_div(regen_envelope, 30.0, 0.0);

        let h_gate_buf = ceph_b_smooth_step(24.0, 36.0, h_per_mob);
        let w_tiny_long =
            band_weight(tl, 5.0, 9.0, 1.0) * ceph_b_smooth_step(8.0, 12.0, n_avg_walk);
        // MB-HPR1: reduce buffer ~50% so more deficit becomes explicit rest
        let hp_buffer = f64::from(n_char_hp)
            * (0.01 + 0.006 * h_gate_buf + 0.006 * w_long_walk + 0.004 * w_tiny_long);

        // First pass: figure out if we *need* to rest
        let deficit = hp_loss_per_loop - passive_hp - hp_buffer;

        let mut rest_tick_hp = 0.0;
        let dmg_per_round = max_dbl(0.0, dmg_per_round_core - f64::from(n_damage_threshold));

        // Continuous minBoost based on damage bands
        let h = hp_loss_per_round;
        let mut min_boost = 1.05
            + 0.1 * ceph_b_smooth_step(1.0, 4.0, h)
            + 0.1 * ceph_b_smooth_step(10.0, 15.0, h)
            + 0.1 * ceph_b_smooth_step(25.0, 35.0, h);

        // Heavy-rest loop blend (tiny chain + long walk + big hits)
        let w_heavy = band_weight(tl, 8.0, 16.0, 4.0)
            * ceph_b_smooth_step(5.0, 7.0, n_avg_walk)
            * ceph_b_smooth_step(10.0, 16.0, h);
        min_boost = lerp(min_boost, max_dbl(min_boost, 1.3), w_heavy);

        let mut tick_boost;
        if n_char_hp_regen == 0 {
            tick_boost = 1.0;
        } else {
            tick_boost = clamp_dbl(
                safe_div(dmg_per_round, max_dbl(1.0, hp_regen), 0.0),
                1.0,
                3.0,
            );
            if tick_boost < min_boost {
                tick_boost = min_boost;
            }
        }

        // Boost actual resting rate for high incoming damage, gated by per-mob intensity.
        let h_gate = ceph_b_smooth_step(12.0, 24.0, h_per_mob);
        let h_gate_lair = ceph_b_smooth_step(18.0, 28.0, h);
        let mut rest_rate_boost = 1.0 + 0.62 * (tick_boost - 1.0) * max_dbl(h_gate, h_gate_lair);
        // tiny allowance for truly heavy per-mob hits
        if h_per_mob >= 30.0 {
            rest_rate_boost *= 1.004;
        }
        // Extra help if per-lair damage is moderate/high but per-mob is modest
        let h_pack_gate = ceph_b_smooth_step(32.0, 60.0, h);
        rest_rate_boost += 0.22
            * (tick_boost - 1.0)
            * h_pack_gate
            * (1.0 - ceph_b_smooth_step(12.0, 24.0, h_per_mob));

        // Short-walk, big-hit bruiser: micro lift
        if n_spell_cost == 0 {
            let w_bruiser = ceph_b_smooth_step(28.0, 36.0, h_per_mob)
                * (1.0 - ceph_b_smooth_step(2.2, 3.0, n_avg_walk));
            rest_rate_boost *= 1.0 + 0.008 * w_bruiser; // up to ~+0.8%
        }

        // Cap a hair lower so we don't erase rest in edge cases
        rest_rate_boost = clamp_dbl(rest_rate_boost, 1.0, 1.85);

        if deficit > 0.0 {
            let mut rest_pulse_k = 0.28;
            if n_spell_cost > 0 {
                let k_chain = ceph_b_smooth_step(20.0, 36.0, tl);
                let k_short = 1.0 - ceph_b_smooth_step(1.6, 2.2, n_avg_walk);
                rest_pulse_k += 0.06 * max_dbl(k_chain, k_short); // casters: up to 0.34
            } else {
                // melee bruiser pulse (short-walk + high per-mob hits)
                let h_gate_hi = ceph_b_smooth_step(18.0, 32.0, h_per_mob);
                let w_short_walk = 1.0 - ceph_b_smooth_step(2.2, 3.3, n_avg_walk);
                rest_pulse_k += 0.03 * (h_gate_hi * w_short_walk); // melee: up to ~0.31
            }
            rest_tick_hp = hp_regen * rest_pulse_k * max_dbl(0.0, tick_boost - 1.0);
        }

        // Final regen total and rest calculation
        let regen_hp = passive_hp + rest_tick_hp;
        let rest_needed = max_dbl(0.0, hp_loss_per_loop - regen_hp - hp_buffer);

        if n_char_hp_regen > 0 && rest_rate_boost > 0.0 {
            rest_secs = rest_needed * SEC_PER_REST_TICK / (hp_regen * rest_rate_boost);
        }

        // ===== Mana / Meditate =====
        rest_secs_disp = rest_secs;
        med_secs_disp = 0.0;

        if caster_or_overhead {
            // Fewer combat rounds after surprise -> fewer casts
            let total_rounds = r.n_rtc * tl;

            // === PATCH MB5 (micro cost bump; no-med casters, mid-band only) ===
            let w_rtk_micro_mb5 = 1.0 - ceph_b_smooth_step(1.0, 1.1, eff_rtk);
            let w_mobs_mb5 = 1.0 - ceph_b_smooth_step(1.2, 1.6, n_num_mobs);
            let w_micro_mb5 = w_rtk_micro_mb5
                * w_mobs_mb5
                * band_weight(tl, 28.0, 40.0, 4.0)
                * ceph_b_smooth_step(2.4, 2.6, n_avg_walk)
                * (1.0 - ceph_b_smooth_step(3.3, 3.7, n_avg_walk))
                * ceph_b_smooth_step(1.7, 2.0, dens_guess)
                * (1.0 - ceph_b_smooth_step(4.0, 4.6, dens_guess))
                * no_med_caster_gate;

            // Up to +10% effective cost at the micro peak; 0% outside gates
            let k_cost_mb5 = lerp(1.1, 1.0, 1.0 - w_micro_mb5);

            let kill_secs_all = kill_secs_per_lair * tl;

            let mut in_combat_mp_frac;
            if n_meditate_rate > 0 {
                in_combat_mp_frac = 0.26;
                if n_total_lairs >= 28 && n_avg_walk <= 3.5 {
                    in_combat_mp_frac += 0.02;
                }
                in_combat_mp_frac = clamp_dbl(in_combat_mp_frac, 0.1, 0.4);
            } else {
                in_combat_mp_frac = 0.31 - 0.035 * n_avg_walk;
                in_combat_mp_frac = in_combat_mp_frac
                    + 0.04 * (1.0 - ceph_b_smooth_step(2.0, 2.6, n_avg_walk))
                    + 0.015 * (1.0 - ceph_b_smooth_step(1.6, 1.9, n_avg_walk))
                    + 0.01 * ceph_b_smooth_step(30.0, 50.0, dens_guess)
                    + 0.01
                        * ceph_b_smooth_step(70.0, 90.0, dens_guess)
                        * (1.0 - ceph_b_smooth_step(1.4, 1.8, n_avg_walk))
                    + 0.01 * flag(n_spell_cost > 0)
                    + 0.01
                        * flag(n_spell_cost > 0 && n_meditate_rate == 0)
                        * ceph_b_smooth_step(2.4, 2.6, n_avg_walk)
                        * (1.0 - ceph_b_smooth_step(3.5, 3.9, n_avg_walk))
                        * ceph_b_smooth_step(1.7, 2.0, dens_guess)
                        * (1.0 - ceph_b_smooth_step(4.0, 4.6, dens_guess));

                let mut mp_frac_hi = 0.34;
                if n_spell_cost > 0 && n_meditate_rate == 0 {
                    mp_frac_hi = 0.36;
                    if dens_guess >= 60.0 && n_avg_walk <= 1.6 {
                        mp_frac_hi = 0.38;
                    }
                }

                // Extra micro-bump + cap lift for ultra-dense, very short walk
                if dens_guess >= 80.0 && n_avg_walk <= 1.4 {
                    in_combat_mp_frac += 0.005;
                    if n_spell_cost > 0 && n_meditate_rate == 0 {
                        mp_frac_hi = max_dbl(mp_frac_hi, 0.385);
                    }
                }

                // Mid-band weight (same shape as travel band)
                let w_mbn = band_weight(tl, 28.0, 40.0, 4.0)
                    * ceph_b_smooth_step(2.4, 2.6, n_avg_walk)
                    * (1.0 - ceph_b_smooth_step(3.3, 3.7, n_avg_walk))
                    * ceph_b_smooth_step(1.7, 2.0, dens_guess)
                    * (1.0 - ceph_b_smooth_step(4.0, 4.6, dens_guess));

                // Tiny in-combat regen bump in the band to offset travel trims
                in_combat_mp_frac += 0.005 * w_mbn;

                // Allow a hair more headroom in the same band
                if w_mbn > 0.0 {
                    mp_frac_hi = max_dbl(mp_frac_hi, 0.37);
                }

                in_combat_mp_frac = clamp_dbl(in_combat_mp_frac, 0.1, mp_frac_hi);

                if n_meditate_rate == 0 {
                    let w_no_med_band = band_weight(tl, 28.0, 40.0, 4.0)
                        * ceph_b_smooth_step(2.4, 3.2, n_avg_walk)
                        * (1.0 - ceph_b_smooth_step(3.4, 3.9, n_avg_walk));

                    in_combat_mp_frac = max_dbl(0.1, in_combat_mp_frac - 0.035 * w_no_med_band);

                    // === PATCH MB2 (micro in-combat MP damp; no-med casters) ===
                    let w_rtk_micro_mb2 = 1.0 - ceph_b_smooth_step(1.0, 1.1, eff_rtk);
                    let w_mobs_mb2 = 1.0 - ceph_b_smooth_step(1.2, 1.6, n_num_mobs);
                    let w_micro_mb2 = w_rtk_micro_mb2 * w_mobs_mb2 * w_no_med_band;

                    if w_micro_mb2 > 0.0 {
                        // Up to ~12% reduction at hard micro
                        let damp_mb2 = lerp(0.88, 1.0, 1.0 - w_micro_mb2);
                        in_combat_mp_frac *= damp_mb2;
                    }
                }
            }

            // Direct mana attacks spend mana every combat round, so overhead is charged in full;
            // without a direct cost, overhead stays fractional.
            let mp_use_frac_b = if n_spell_cost > 0 {
                1.0
            } else {
                in_combat_mp_frac
            };

            mana_cost_loop = (f64::from(n_spell_cost) * total_rounds)
                + (n_spell_overhead * total_rounds * mp_use_frac_b);
            mana_cost_loop *= k_cost_mb5;

            // MANA KNOB
            mana_cost_loop = mana_cost_loop * DEFAULT_MANA * knobs.mana;

            // Passive MP ticks through most of combat for direct mana attacks; overhead-only
            // upkeep keeps the modeled in-combat fraction.
            let mut combat_regen_frac_b;
            if n_spell_cost > 0 {
                combat_regen_frac_b = 0.5 + 0.35 * ceph_b_smooth_step(3.0, 28.0, r.n_rtc);
                if combat_regen_frac_b < in_combat_mp_frac {
                    combat_regen_frac_b = in_combat_mp_frac;
                }
                if combat_regen_frac_b > 1.0 {
                    combat_regen_frac_b = 1.0;
                }
            } else {
                combat_regen_frac_b = in_combat_mp_frac;
            }
            let combat_regen_secs = combat_regen_frac_b * kill_secs_all;

            // Credit some of the shaved route time back to mana regen
            let walk_for_mana =
                walk_loop_secs + 0.5 * max_dbl(0.0, walk_regen_secs - walk_loop_secs);

            // (MME also derives an HP passive-only equivalence of the walk here; nothing reads it.)

            let mana_regen_secs = walk_for_mana + rest_secs + combat_regen_secs;

            let mp_regen = f64::from(n_char_mp_regen);
            mana_gain = mp_regen * safe_div(mana_regen_secs, 30.0, 0.0);

            pool_credit = f64::from(n_char_mana) * 0.1;
            if n_spell_cost > 0 && n_meditate_rate == 0 {
                if dens_guess >= 60.0 && n_avg_walk <= 1.6 {
                    pool_credit = f64::from(n_char_mana) * 0.16; // ultra-dense micro-loops
                } else if n_avg_walk >= 2.5
                    && n_avg_walk <= 3.5
                    && dens_guess >= 2.0
                    && dens_guess <= 4.0
                {
                    let w_mbn_pc = band_weight(tl, 28.0, 40.0, 4.0)
                        * ceph_b_smooth_step(2.4, 2.6, n_avg_walk)
                        * (1.0 - ceph_b_smooth_step(3.3, 3.7, n_avg_walk))
                        * ceph_b_smooth_step(1.7, 2.0, dens_guess)
                        * (1.0 - ceph_b_smooth_step(4.0, 4.6, dens_guess));

                    // PATCH 2025-08-30: no-meditate mid-band gets a smaller pool credit.
                    pool_credit = f64::from(n_char_mana) * lerp(0.06, 0.1, w_mbn_pc);
                    // === PATCH MB3 (micro pool-credit damp; no-med casters) ===
                    let w_rtk_micro_mb3 = 1.0 - ceph_b_smooth_step(1.0, 1.1, eff_rtk);
                    let w_mobs_mb3 = 1.0 - ceph_b_smooth_step(1.2, 1.6, n_num_mobs);
                    let w_micro_mb3 = w_rtk_micro_mb3
                        * w_mobs_mb3
                        * band_weight(tl, 28.0, 40.0, 4.0)
                        * ceph_b_smooth_step(2.4, 2.6, n_avg_walk)
                        * (1.0 - ceph_b_smooth_step(3.3, 3.7, n_avg_walk))
                        * ceph_b_smooth_step(1.7, 2.0, dens_guess)
                        * (1.0 - ceph_b_smooth_step(4.0, 4.6, dens_guess));

                    if w_micro_mb3 > 0.0 {
                        // Up to ~15% less pool at hard micro
                        let k_pool_mb3 = lerp(0.85, 1.0, 1.0 - w_micro_mb3);
                        pool_credit *= k_pool_mb3;
                    }
                }
            }

            // medNeeded is the authoritative loop-level mana deficit.
            med_needed = max_dbl(0.0, mana_cost_loop - mana_gain - pool_credit);

            if n_meditate_rate > 0 && med_needed >= f64::from(n_meditate_rate) / 2.0 {
                med_secs = (med_needed / f64::from(n_meditate_rate)) * 10.0;
            } else if n_meditate_rate == 0 && n_char_mp_regen > 0 {
                med_secs = (med_needed / mp_regen) * 30.0;
            } else {
                med_secs = 0.0;
            }

            med_secs_disp = med_secs;

            // PATCH 2025-08-30: re-label HP Rest -> Mana when no-meditate and resting is
            // masking an MP deficit.
            if med_secs == 0.0 && n_meditate_rate == 0 {
                let mut relabel_cap_pct = 0.0;

                if n_avg_walk >= 8.0 && dens_guess >= 12.0 {
                    relabel_cap_pct = 0.55;
                } else if hp_loss_per_round <= 8.0 {
                    relabel_cap_pct = 0.35;
                }

                if relabel_cap_pct > 0.0 && rest_secs_disp > 0.0 {
                    let mana_regen_secs_no_rest =
                        walk_loop_secs + in_combat_mp_frac * kill_secs_all;
                    let mana_gain_no_rest = mp_regen * safe_div(mana_regen_secs_no_rest, 30.0, 0.0);
                    let med_needed_no_rest =
                        max_dbl(0.0, mana_cost_loop - mana_gain_no_rest - pool_credit);

                    if med_needed_no_rest > 0.0 {
                        let mut relabel = (med_needed_no_rest / max_dbl(1.0, mp_regen)) * 30.0;
                        relabel = min_dbl(relabel, rest_secs_disp * relabel_cap_pct);

                        med_secs_disp += relabel;
                        rest_secs_disp -= relabel;
                    }
                }
            }
        } else {
            // No mana consumer -> skip mana model entirely
            med_secs = 0.0;
            med_secs_disp = 0.0;
        }
    }

    // ===== Final loop time (with instant-respawn micro-floor) =====
    let final_raw = loop_secs_raw + rest_secs + med_secs;
    let mut slack_secs = 0.0;
    let mut loop_secs;

    let slack_params = SlackParams {
        spell_cost: n_spell_cost,
        spell_overhead: n_spell_overhead,
        char_mp_regen: n_char_mp_regen,
        meditate_rate: n_meditate_rate,
        mana_cost_loop,
        pool_credit,
    };
    let mut slack = SlackState {
        walk_loop_secs,
        mana_gain,
        med_secs,
        med_secs_disp,
        med_needed,
    };

    if b_instant {
        if final_raw < MIN_LOOP {
            slack_secs = MIN_LOOP - final_raw;
            apply_slack_window(slack_secs, &mut slack, &slack_params);
            loop_secs = MIN_LOOP;
        } else {
            loop_secs = final_raw;
        }
    } else {
        loop_secs = final_raw;
    }

    // --- Respawn gating (hard cap): loop cannot be shorter than per-chain respawn window ---
    if n_regen_time > 0.0 && n_total_lairs > 0 {
        let spawn_interval = safe_div(n_regen_time * 60.0, tl, 0.0);
        if loop_secs < spawn_interval {
            let gate_slack = spawn_interval - loop_secs;
            loop_secs = spawn_interval;
            slack_secs += gate_slack;
            apply_slack_window(gate_slack, &mut slack, &slack_params);
        }
    }
    let _ = slack_secs;
    let walk_loop_secs = slack.walk_loop_secs;
    let med_secs_disp = slack.med_secs_disp;

    // xpPerCycle is Currency * Long, a Currency: overflow is error 6, which MME's handler swallows.
    let xp_per_cycle = n_exp
        .checked_mul(Currency::from_scaled(i64::from(n_total_lairs) * 10_000))
        .ok()?
        .to_f64();

    let cycles_per_hour = safe_div(3600.0, loop_secs, 0.0);

    // PATCH 2025-08-30: HP/MP overlap during meditation - reduce displayed HP rest when medding.
    // Display-only: adjust restSecsDisp/medSecsDisp, not loopSecs or medSecs.
    let mut med_secs_disp = med_secs_disp;
    if n_meditate_rate > 0 && med_secs_disp > 0.0 && rest_secs_disp > 0.0 {
        // More overlap when per-mob hits are light; fades out for heavier hits.
        let overlap_coef = 0.6 * (1.0 - ceph_b_smooth_step(8.0, 16.0, h_per_mob));

        let overlap = min_dbl(rest_secs_disp, med_secs_disp * overlap_coef);

        if overlap > 0.0 {
            med_secs_disp += overlap;
            rest_secs_disp -= overlap;
        }

        rest_secs_disp = max_dbl(0.0, rest_secs_disp);
        med_secs_disp = max_dbl(0.0, med_secs_disp);
    }

    // ===== Pack =====
    r.n_exp_per_hour = xp_per_cycle * cycles_per_hour;

    // EXP KNOB
    r.n_exp_per_hour = r.n_exp_per_hour * DEFAULT_XP * knobs.xp;

    r.n_hitpoint_recovery = safe_div(rest_secs_disp, loop_secs, 0.0);
    r.n_mana_recovery = safe_div(med_secs_disp, loop_secs, 0.0);
    r.n_time_recovering = r.n_hitpoint_recovery + r.n_mana_recovery;
    r.n_move = safe_div(walk_loop_secs, loop_secs, 0.0);
    if r.n_move > 1.0 {
        r.n_move = 1.0;
    }
    r.n_overkill = max_dbl(0.0, overkill_factor - 1.0);

    // Fractions for text (mirrors ModelA semantics)
    let attack_frac = safe_div(kill_secs_per_lair * tl, loop_secs, 0.0);
    let slowdown_frac = if n_rtk > 0.0 {
        max_dbl(0.0, safe_div(eff_rtk, n_rtk, 0.0) - 1.0)
    } else {
        0.0
    };

    // Insufficient-lairs (respawn gating) display metric
    let roam_share = if regen_window > 0.0 && loop_secs < regen_window {
        clamp_dbl((regen_window - loop_secs) / regen_window, 0.0, 1.0)
    } else {
        0.0
    };

    r.n_slowdown_time = slowdown_frac;
    r.n_attack_time = attack_frac;
    r.n_roam_time = roam_share;
    if b_backstab_less {
        r.n_attack_time *= -1.0;
    }
    Some(r)
}
