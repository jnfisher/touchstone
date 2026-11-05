//! Model B's private helpers: band weights, overkill inflation, the travel loop and the slack
//! window (`cephB_*` in modExpPerHour). Debug logging is not ported (it has no effect on results).
#![allow(
    clippy::manual_clamp,
    clippy::manual_range_contains,
    unused_assignments
)] // MME's guarded assignments, kept in its order.

use touchstone_vb6::{exp, log};

use crate::exp_per_hour::helpers::{
    SEC_PER_MEDI_TICK, SEC_PER_REGEN_TICK, ceph_b_saturate, ceph_b_smooth_step, clamp_dbl, max_dbl,
    min_dbl, safe_div,
};

const LOGISTIC_CAP: f64 = 700.0;
const LOGISTIC_DENOM: f64 = 0.5;
const TF_LOG_COEF: f64 = 0.15;
const TF_SMALL_BUMP: f64 = 0.7;
const TF_SCARCITY_COEF: f64 = 0.15;
const LAIR_OVERHEAD_R: f64 = 1.0;

/// Port of modExpPerHour `cephB_Lerp`.
#[must_use]
pub(super) fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Port of modExpPerHour `cephB_MulBlend`.
#[must_use]
pub(super) fn mul_blend(cur: f64, factor: f64, t: f64) -> f64 {
    cur * lerp(1.0, factor, ceph_b_saturate(t))
}

/// Port of modExpPerHour `cephB_BandWeight` (a plateau between `lo` and `hi` with smooth edges
/// `fade` wide; the VB6 default for `fade` is 2).
#[must_use]
pub(super) fn band_weight(x: f64, lo: f64, hi: f64, fade: f64) -> f64 {
    let w_in = ceph_b_smooth_step(lo - fade, lo, x);
    let w_out = 1.0 - ceph_b_smooth_step(hi, hi + fade, x);
    ceph_b_saturate(w_in * w_out)
}

/// Port of modExpPerHour `cephB_CalcDensity`: rooms per lair, or the average walk when the lair
/// data is missing.
#[must_use]
pub(super) fn calc_density(total_lairs: i32, poss_spawns: i32, avg_walk: f64) -> f64 {
    if total_lairs > 0 && poss_spawns > 0 {
        safe_div(f64::from(poss_spawns), f64::from(total_lairs), avg_walk)
    } else {
        avg_walk
    }
}

/// Port of modExpPerHour `cephB_CalcOverkill`. MME's handler swallows a failing `Exp` (overflow
/// for a very negative damage) and the function then returns its unset value, 0.
#[must_use]
pub(super) fn calc_overkill(dmg: f64, mob_hp: i32, is_spell: bool) -> f64 {
    if mob_hp <= 0 {
        return 1.0;
    }
    let mut raw = (f64::from(mob_hp) - dmg) / (LOGISTIC_DENOM * f64::from(mob_hp));
    raw = clamp_dbl(raw, -LOGISTIC_CAP, LOGISTIC_CAP);

    let mult = if is_spell { 1.35 } else { 1.0 };
    let Ok(e) = exp(raw * mult) else {
        return 0.0;
    };
    let ok = 1.0 + 1.0 / (1.0 + e);
    if is_spell {
        clamp_dbl(ok, 1.0, 1.06)
    } else {
        clamp_dbl(ok, 1.0, 1.18)
    }
}

/// Port of modExpPerHour `cephB_CalcTravelLoopSecs`. `Log` of a non-positive number (a walk of
/// -1 or less) is swallowed by MME's handler, which leaves the result at 0.
///
/// The band tweaks below are hand-tuned to specific zones; they are behaviour and kept verbatim.
#[must_use]
#[allow(clippy::too_many_lines)]
pub(super) fn calc_travel_loop_secs(
    avg_walk: f64,
    total_lairs: i32,
    poss_spawns: i32,
    sec_per_room: f64,
) -> f64 {
    let tl = f64::from(total_lairs);

    // Rooms per lair (fallback to avgWalk if missing inputs)
    let dens = calc_density(total_lairs, poss_spawns, avg_walk);

    // Scarcity: more time if avgWalk >> density (rooms/lair)
    let mut scarcity = if dens >= 5.0 {
        1.0 + (TF_SCARCITY_COEF - 0.03) * safe_div(avg_walk, max_dbl(1.0, dens), 0.0)
    } else {
        1.0 + TF_SCARCITY_COEF * safe_div(avg_walk, max_dbl(1.0, dens), 0.0)
    };

    // Short-walk bump + mild log growth
    let Ok(ln) = log(1.0 + avg_walk) else {
        return 0.0;
    };
    let mut tf = 1.0 + TF_LOG_COEF * ln + TF_SMALL_BUMP / (1.0 + avg_walk);
    if avg_walk <= 1.6 && dens >= 30.0 {
        tf *= 0.93;
    }

    // Base overheads
    let mut lair_overhead = LAIR_OVERHEAD_R * sec_per_room;
    let base_rooms = avg_walk * sec_per_room;

    // Scale overheads for lair count
    let mut overhead_scale = 0.6 + 0.4 * min_dbl(1.0, 20.0 / max_dbl(1.0, tl));
    let scale_up =
        0.06 * ceph_b_smooth_step(30.0, 45.0, tl) * ceph_b_smooth_step(2.4, 3.6, avg_walk);
    overhead_scale += scale_up;
    lair_overhead *= overhead_scale;

    // ---- Smoothed micro-route shaves ----
    let w_short = 1.0 - ceph_b_smooth_step(1.6, 2.2, avg_walk);
    let w_dense = ceph_b_smooth_step(50.0, 70.0, dens);
    let w_ud = w_short * w_dense;
    tf = mul_blend(tf, 0.91, w_ud);
    lair_overhead = mul_blend(lair_overhead, 0.91, w_ud);

    let w_short2 = 1.0 - ceph_b_smooth_step(1.4, 1.9, avg_walk);
    let w_dense2 = ceph_b_smooth_step(70.0, 90.0, dens);
    let w_ud2 = w_short2 * w_dense2;
    tf = mul_blend(tf, 0.97, w_ud2);
    lair_overhead = mul_blend(lair_overhead, 0.96, w_ud2);

    // Only damp LONG routes: no penalty until ~6 rooms. aw is never negative, so Pow cannot fail.
    let aw = max_dbl(0.0, avg_walk - 5.0);
    let mut damp = 1.0 / (1.0 + 0.12 * aw.powf(1.4));

    // --- Route band tweaks ---
    if total_lairs >= 12 && total_lairs <= 16 && avg_walk >= 5.0 {
        // Keep this one discrete (tiny population + explicit target)
        tf *= 0.75;
        damp *= 0.75;
        lair_overhead *= 0.8;
        if avg_walk >= 6.0 {
            tf *= 0.97;
            damp *= 0.92;
            lair_overhead *= 0.94;
        }
        if total_lairs <= 13 && avg_walk >= 6.0 {
            tf *= 0.97;
            lair_overhead *= 0.96;
        }
    } else {
        // Smoothed big-chain / short-walk regime
        let w_big = ceph_b_smooth_step(24.0, 34.0, tl);
        let w_short3 = 1.0 - ceph_b_smooth_step(3.3, 4.2, avg_walk);
        let w_low_walk = w_big * w_short3;
        // On very large chains (30-45 lairs) with modest walk, reduce trim strength
        let w_huge = ceph_b_smooth_step(30.0, 45.0, tl) * ceph_b_smooth_step(2.4, 3.6, avg_walk);

        // Up to 50% weaker trim on huge chains; trims themselves less aggressive
        let w_ll = w_low_walk * (1.0 - 0.5 * w_huge);
        tf = mul_blend(tf, 0.94, w_ll);
        lair_overhead = mul_blend(lair_overhead, 0.96, w_ll);

        // Route complexity for big-chains with ~3-room walks (SIM2/16)
        let w_lx = band_weight(tl, 30.0, 38.0, 3.0);
        let w_wx =
            ceph_b_smooth_step(2.6, 3.2, avg_walk) * (1.0 - ceph_b_smooth_step(3.6, 3.9, avg_walk));
        let w_cx = w_lx * w_wx;

        // Banded "junction" seconds per lair (raw seconds, not scaled by secPerRoom)
        let junction_sec = 2.5 * w_cx;
        lair_overhead += junction_sec;

        // A little extra pathing inefficiency on the loop multiplier
        tf = mul_blend(tf, 1.1, w_cx);

        // Scarcity easing within low-walk (guard against upward re-inflation on huge chains)
        let ratio = safe_div(avg_walk, max_dbl(1.0, dens), 0.0);
        let sc_coef_base = TF_SCARCITY_COEF;

        // Start from base and allow only the low-walk easing (no add-back on huge chains)
        let mut sc_coef = TF_SCARCITY_COEF - 0.02 * w_low_walk;
        if sc_coef > sc_coef_base {
            sc_coef = sc_coef_base;
        }
        scarcity = 1.0 + sc_coef * ratio;

        // Additional gentle easing when the chain is truly huge (60-80 lairs) with ~3 walk
        let w_huge_chain =
            band_weight(tl, 60.0, 80.0, 6.0) * ceph_b_smooth_step(2.4, 3.4, avg_walk);

        // Soften TF and per-lair overhead slightly in that band
        tf = mul_blend(tf, 0.95, w_huge_chain);
        lair_overhead = mul_blend(lair_overhead, 0.92, w_huge_chain);

        // Sparse chains inside (dens<~6) with soft blend
        let w_sparse = 1.0 - ceph_b_smooth_step(5.0, 7.0, dens);
        let w_sparse_band = band_weight(tl, 28.0, 40.0, 4.0) * w_short3 * w_sparse;
        tf = mul_blend(tf, 0.91, w_sparse_band);
        lair_overhead = mul_blend(lair_overhead, 0.94, w_sparse_band);

        // Mid-walk / mid-density band trim (2.6-3.3 walk, 2-4 dens) smoothed
        let w_walk_band =
            ceph_b_smooth_step(2.4, 2.6, avg_walk) * (1.0 - ceph_b_smooth_step(3.3, 3.7, avg_walk));
        let w_dens_band =
            ceph_b_smooth_step(1.7, 2.0, dens) * (1.0 - ceph_b_smooth_step(4.0, 4.6, dens));
        let w_band = w_big * w_walk_band * w_dens_band;
        tf = mul_blend(tf, 0.93, w_band);
        lair_overhead = mul_blend(lair_overhead, 0.88, w_band);
        let sc_ease = TF_SCARCITY_COEF - 0.12;
        scarcity = 1.0 + lerp(TF_SCARCITY_COEF, sc_ease, w_band) * ratio;
    }
    // --- end band tweaks ---

    // Very sparse & big chain smoothing
    let w_very_sparse =
        (1.0 - ceph_b_smooth_step(1.6, 2.4, dens)) * ceph_b_smooth_step(36.0, 44.0, tl);
    tf = mul_blend(tf, 0.985, w_very_sparse);
    scarcity = lerp(scarcity, scarcity * 0.97, w_very_sparse);

    (tl * (base_rooms + lair_overhead)) * tf * scarcity * damp
}

/// The ByRef state `cephB_ApplySlackWindow` updates.
pub(super) struct SlackState {
    pub walk_loop_secs: f64,
    pub mana_gain: f64,
    pub med_secs: f64,
    pub med_secs_disp: f64,
    pub med_needed: f64,
}

/// The ByVal arguments of `cephB_ApplySlackWindow` that come from the caller's inputs.
pub(super) struct SlackParams {
    pub spell_cost: i16,
    pub spell_overhead: f64,
    pub char_mp_regen: i32,
    pub meditate_rate: i32,
    pub mana_cost_loop: f64,
    pub pool_credit: f64,
}

/// Port of modExpPerHour `cephB_ApplySlackWindow`: extra seconds are pushed into movement and
/// credit passive mana, and the meditate requirement is recomputed.
pub(super) fn apply_slack_window(extra_sec: f64, s: &mut SlackState, p: &SlackParams) {
    if extra_sec <= 0.0 {
        return;
    }

    // Push slack into movement so time-share math stays normalized
    s.walk_loop_secs += extra_sec;

    // Credit extra passive MP ticks that occur during this slack window
    if (p.spell_cost > 0 || p.spell_overhead > 0.0) && p.char_mp_regen > 0 {
        let regen = f64::from(p.char_mp_regen);
        let slack_mp = regen * safe_div(extra_sec, SEC_PER_REGEN_TICK, 0.0);
        s.mana_gain += slack_mp;

        // Recompute med requirement since manaGain changed
        s.med_needed = max_dbl(0.0, p.mana_cost_loop - s.mana_gain - p.pool_credit);
        let rate = f64::from(p.meditate_rate);
        if p.meditate_rate > 0 && s.med_needed >= rate / 2.0 {
            s.med_secs = (s.med_needed / rate) * SEC_PER_MEDI_TICK;
        } else if p.meditate_rate == 0 && p.char_mp_regen > 0 {
            s.med_secs = (s.med_needed / regen) * SEC_PER_REGEN_TICK;
        } else {
            s.med_secs = 0.0;
        }
        s.med_secs_disp = s.med_secs;
    }
}
