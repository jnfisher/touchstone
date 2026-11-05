//! Model A's surprise-opener section: one expected opener replaces the normal first swing, and
//! either removes rounds from the lair or adds them.

use touchstone_vb6::{Vb6Error, div, exp, pow, sqr};

/// What the surprise section changes for the rest of the model.
pub(super) struct Surprise {
    pub n_rtc_eff: f64,
    pub n_mob_dmg_use: f64,
    /// The opener makes the fight longer (flips the sign of `nAttackTime` on output).
    pub b_surprise_less: bool,
}

/// Inputs of the section, after Model A's validation.
pub(super) struct SurpriseArgs {
    pub n_surprise_dmg: f64,
    pub n_num_mobs: f64,
    pub n_mob_hp: f64,
    pub n_char_dmg: f64,
    pub n_rtk: f64,
    pub n_rtc: f64,
    pub n_mob_dmg: f64,
    pub n_mob_hp_regen: f64,
    pub n_mob_hp_regen_rounds: f64,
}

/// The `If nSurpriseDMG > 0#` block of `ceph_ModelA`. The caller only enters it for a positive
/// surprise damage; otherwise MME's defaults (`nRTC`, `nMobDmg`, no flag) stand.
///
/// # Errors
/// A VB6 run-time error, which `ceph_ModelA`'s handler turns into an empty result: `IIf` evaluates
/// both branches, so the regen ratio divides by a zero `nCharDMG` even when it is not used.
pub(super) fn opener(a: &SurpriseArgs) -> Result<Surprise, Vb6Error> {
    let mut hp1 = if a.n_num_mobs > 1.0 {
        a.n_mob_hp / a.n_num_mobs
    } else {
        a.n_mob_hp
    };
    if hp1 < 1.0 {
        hp1 = 1.0;
    }

    // Logistic smoothing around one-shot thresholds (same sharpness for both openers).
    let a_sharp = 6.0;

    // Normal opener expectation.
    let rtk_n_no_smooth;
    let p_kill_n;
    if a.n_char_dmg > 0.0 {
        rtk_n_no_smooth = -(-(hp1 / a.n_char_dmg) * 2.0).floor() / 2.0;
        let ratio_n = a.n_char_dmg / hp1;
        p_kill_n = 1.0 / (1.0 + exp(-a_sharp * (ratio_n - 1.0))?);
    } else {
        rtk_n_no_smooth = a.n_rtk;
        p_kill_n = 0.0;
    }
    let rtk_n_no_smooth = if rtk_n_no_smooth < 1.0 {
        1.0
    } else {
        rtk_n_no_smooth
    };
    let p_kill_n = clamp01(p_kill_n);
    let rtk_n_eff = (p_kill_n * 1.0) + ((1.0 - p_kill_n) * rtk_n_no_smooth);

    // Surprise opener expectation.
    let mut rmn = hp1 - a.n_surprise_dmg;
    if rmn < 0.0 {
        rmn = 0.0;
    }
    let dmg_or_one = if a.n_char_dmg > 0.0 {
        a.n_char_dmg
    } else {
        1.0
    };
    let mut rtk_s_no_smooth = 1.0 + (-(-((rmn / dmg_or_one) * 2.0)).floor() / 2.0);
    if rtk_s_no_smooth < 1.0 {
        rtk_s_no_smooth = 1.0;
    }
    let ratio_s = a.n_surprise_dmg / hp1;
    let p_kill_s = clamp01(1.0 / (1.0 + exp(-a_sharp * (ratio_s - 1.0))?));
    let rtk_s_eff = (p_kill_s * 1.0) + ((1.0 - p_kill_s) * rtk_s_no_smooth);

    let delta_first = rtk_s_eff - rtk_n_eff;
    let mut n_rtc_eff = a.n_rtc;
    let mut b_surprise_less = false;
    if delta_first < 0.0 {
        // Surprise is better: only the savings are attenuated.
        let mut saved_first = -delta_first;

        let regen_per_round = a.n_mob_hp_regen / a.n_mob_hp_regen_rounds;
        let quotient = div(regen_per_round, a.n_char_dmg)?;
        let regen_ratio = if a.n_char_dmg > 0.0 { quotient } else { 0.0 };
        // Up to ~45% attenuation when regen is large against damage per round.
        let sq = pow((regen_ratio - 0.0) / (0.6 - 0.0), 2.0)?
            * (3.0 - 2.0 * ((regen_ratio - 0.0) / (0.6 - 0.0)));
        let smooth = if regen_ratio <= 0.0 { 0.0 } else { sq };
        let mut regen_atten = 1.0 - 0.45 * smooth;
        if regen_atten < 0.55 {
            regen_atten = 0.55;
        }

        let pack_fade = 1.0
            / sqr(if a.n_num_mobs > 1.0 {
                a.n_num_mobs
            } else {
                1.0
            })?;
        let mut fade_gate = 0.0;
        if a.n_num_mobs > 3.0 {
            let mut tpf = (a.n_num_mobs - 3.0) / (8.0 - 3.0);
            tpf = clamp01(tpf);
            fade_gate = tpf * tpf * (3.0 - 2.0 * tpf);
        }

        saved_first = saved_first * (1.0 - (1.0 - pack_fade) * fade_gate) * regen_atten;
        n_rtc_eff = a.n_rtc - saved_first;
        if n_rtc_eff < a.n_num_mobs {
            n_rtc_eff = a.n_num_mobs;
        }
    } else if delta_first > 0.0 && a.n_char_dmg < (a.n_mob_hp / a.n_num_mobs) {
        // Surprise is worse: the penalty is not attenuated.
        n_rtc_eff = a.n_rtc + delta_first;
        b_surprise_less = true;
    }

    // Incoming damage drops only if the surprise opener deletes the mob earlier than normal.
    let mut n_mob_dmg_use = a.n_mob_dmg;
    let p_delete_delta = p_kill_s - p_kill_n;
    if p_delete_delta > 0.0 {
        n_mob_dmg_use = a.n_mob_dmg * (1.0 - (p_delete_delta / a.n_num_mobs));
        if n_mob_dmg_use < 0.0 {
            n_mob_dmg_use = 0.0;
        }
    }

    Ok(Surprise {
        n_rtc_eff,
        n_mob_dmg_use,
        b_surprise_less,
    })
}

fn clamp01(x: f64) -> f64 {
    if x < 0.0 {
        0.0
    } else if x > 1.0 {
        1.0
    } else {
        x
    }
}
