//! Model A's own helpers: the HP-recovery estimate and the in-combat mana fraction.

use touchstone_vb6::{Vb6Error, cint, div};

use super::super::helpers::{SEC_PER_REGEN_TICK, SEC_PER_REST_TICK, SEC_PER_ROUND, clamp_dbl};

/// `DEFAULT_CEPHA_HP_PASSIVE_COMBAT_EFF`.
pub(super) const HP_PASSIVE_COMBAT_EFF: f64 = 0.3;

/// Port of modExpPerHour `cephA_CalcHPRecoveryRounds`: rest rounds needed after a lair, with the
/// q-elasticity damping.
///
/// `n_mobs` is an `Optional Integer` in MME, so a Double argument is rounded half to even and
/// overflows beyond 32767; that error belongs to the caller. A VB6 error inside the body is
/// swallowed by the procedure's own handler, which leaves the result at 0.
///
/// # Errors
/// [`Vb6Error::Overflow`] when `n_mobs` does not fit an Integer.
pub(super) fn calc_hp_recovery_rounds(
    n_dmg_in: f64,
    n_dmg_out: f64,
    n_mob_hp: f64,
    n_rest_hp: f64,
    n_mobs: f64,
    n_rtc: f64,
) -> Result<f64, Vb6Error> {
    let mobs = cint(n_mobs)?;
    Ok(body(n_dmg_in, n_dmg_out, n_mob_hp, n_rest_hp, mobs, n_rtc).unwrap_or(0.0))
}

fn body(
    n_dmg_in: f64,
    n_dmg_out: f64,
    n_mob_hp: f64,
    mut n_rest_hp: f64,
    mut n_mobs: i16,
    n_rtc: f64,
) -> Result<f64, Vb6Error> {
    if n_dmg_in <= 0.0 {
        return Ok(0.0);
    }
    if n_mobs < 1 {
        n_mobs = 1;
    }
    if n_rest_hp < 1.0 {
        n_rest_hp = 1.0;
    }

    // Attack rounds, derived when the caller supplied none.
    let mut r;
    if n_rtc == 0.0 && n_dmg_out > 0.0 {
        if n_dmg_out >= n_mob_hp {
            r = 1.0;
        } else {
            r = n_mob_hp / n_dmg_out;
        }
        if n_mobs > 1 {
            r *= f64::from(n_mobs);
        }
    } else {
        r = n_rtc;
    }
    if r < 1.0 {
        r = 1.0;
    }

    let combat_secs = r * SEC_PER_ROUND;
    let dmg_in_total = r * n_dmg_in;

    // Passive ticks run in every state.
    let passive_per_tick = n_rest_hp / 3.0;
    let passive_heal_combat =
        HP_PASSIVE_COMBAT_EFF * (combat_secs / SEC_PER_REGEN_TICK) * passive_per_tick;
    let mut net_dmg = dmg_in_total - passive_heal_combat;
    if net_dmg < 0.0 {
        net_dmg = 0.0;
    }

    // While resting: a rest tick every 20 s plus the passive tick.
    let rest_heal_per_sec =
        (n_rest_hp / SEC_PER_REST_TICK) + (passive_per_tick / SEC_PER_REGEN_TICK);

    let mut rest_rounds = if rest_heal_per_sec > 0.0 {
        div(net_dmg, rest_heal_per_sec * SEC_PER_ROUND)?
    } else {
        0.0
    };
    if rest_rounds < 0.0 {
        rest_rounds = 0.0;
    }

    // Elastic correction on q, incoming damage per round over rest heal per round.
    let rest_heal_per_round = rest_heal_per_sec * SEC_PER_ROUND;
    let q = if rest_heal_per_round > 0.0 {
        n_dmg_in / rest_heal_per_round
    } else {
        0.0
    };
    let g = if q <= 0.0 {
        1.0
    } else if q < 1.0 {
        0.6 + 0.4 * q
    } else if q <= 4.0 {
        let mut g = 1.0 - 0.4 * (q - 1.0) / 3.0;
        if g < 0.6 {
            g = 0.6;
        }
        g
    } else {
        0.6
    };

    rest_rounds *= g;
    if rest_rounds < 0.0 {
        rest_rounds = 0.0;
    }
    Ok(rest_rounds)
}

/// Port of modExpPerHour `cephA_InCombatMPFrac`: share of the per-round spell overhead that is
/// charged as mana when there is no direct spell cost.
#[must_use]
pub(super) fn in_combat_mp_frac(n_meditate_rate: i32, n_total_lairs: i32, n_avg_walk: f64) -> f64 {
    if n_meditate_rate > 0 {
        let mut f = 0.26;
        if n_total_lairs >= 28 && n_avg_walk <= 3.5 {
            f += 0.02;
        }
        clamp_dbl(f, 0.1, 0.4)
    } else {
        clamp_dbl(0.26, 0.1, 0.35)
    }
}
