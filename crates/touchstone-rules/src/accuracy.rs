//! Accuracy, hit chance and dodge (FAM-06). Card P3-08.
//!
//! Functions take plain inputs instead of MME's globals. A Classes lookup (`tabClasses.Seek`)
//! becomes an `Option<&Class>`: pass the row MME's seek lands on, `None` where it fails.
//!
//! Where MME's procedure has an `On Error` handler the port returns what the handler leaves
//! behind, not an error: most of these functions assign their result only at the very end, so a
//! VB6 run-time error (an Integer or Long overflow) gives 0 (or `[0, 0]`), and
//! [`calculate_accuracy`] also keeps the text lines it had already appended.
//!
//! MME's `bGreaterMUD` branches are not ported, nor the arguments only they read (`eEvil`,
//! intellect, charm, the attack type). One-line `If a Then x: If b Then y` in the original
//! runs the second test only when the first holds, so several of its clamps never fire (for
//! example a negative dodge is not clamped); the port keeps that.

use touchstone_model::Class;
use touchstone_vb6 as vb;
use touchstone_vb6::Vb6Error;

use crate::Profile;
use crate::quicksilver::Balance;
use crate::stats;

type Res<T> = Result<T, Vb6Error>;

/// Port of modSyntaxsFunc AutoAppend with a CRLF glue.
fn append_line(text: &mut String, add: &str) {
    if text.is_empty() {
        text.push_str(add);
    } else if !add.trim_matches(' ').is_empty() {
        text.push_str("\r\n");
        text.push_str(add);
    }
}

fn finite(x: f64) -> Res<f64> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(Vb6Error::Overflow)
    }
}

// ---------------------------------------------------------------------------------------------
// Caps and minimums

/// Port of modMMudFunc GetHitMin: the lowest hit chance, whatever the class.
#[must_use]
pub fn get_hit_min(profile: Profile, _class: i32) -> i32 {
    match profile {
        Profile::Stock | Profile::Quicksilver(_) => 8,
    }
}

/// Port of modMMudFunc GetHitCap: the highest hit chance.
#[must_use]
pub fn get_hit_cap(profile: Profile) -> i32 {
    match profile {
        Profile::Stock | Profile::Quicksilver(_) => 99,
    }
}

/// Port of modMMudFunc GetSpellHitCap: the highest spell hit chance.
#[must_use]
pub fn get_spell_hit_cap(profile: Profile) -> i32 {
    match profile {
        Profile::Stock | Profile::Quicksilver(_) => 98,
    }
}

/// Port of modMMudFunc GetDodgeCap: the highest dodge chance. Stock has one cap, so the soft-cap
/// flag and the class make no difference.
#[must_use]
pub fn get_dodge_cap(profile: Profile, _class: i32, _soft_cap: bool) -> i32 {
    match profile {
        Profile::Stock | Profile::Quicksilver(_) => 95,
    }
}

/// Port of modMMudFunc GMUD_IsLightArmourClass: classes whose armour type is 1, 2 or 6. Only
/// GreaterMUD's caps read it; the body itself has no profile branch.
#[must_use]
pub fn gmud_is_light_armour_class(class_number: i32, class: Option<&Class>) -> bool {
    class_number >= 1 && matches!(stats::get_class_armour_type(class), 1 | 2 | 6)
}

/// Port of modMMudFunc GMUD_DiminishingReturns: `scale` times the triangular root of
/// `value / scale`, odd in `value`. Returns 0 where MME's handler catches an overflow.
#[must_use]
pub fn gmud_diminishing_returns(value: f64, scale: f64) -> f64 {
    if scale <= 0.0 {
        return value;
    }
    let steps = || -> Res<f64> {
        let is_neg = value < 0.0;
        let v = if is_neg { -value } else { value };
        let mult = vb::div(v, scale)?;
        let tri = (vb::sqr(finite(8.0 * mult + 1.0)?)? - 1.0) / 2.0;
        let r = finite(tri * scale)?;
        Ok(if is_neg { -r } else { r })
    };
    steps().unwrap_or(0.0)
}

// ---------------------------------------------------------------------------------------------
// Dodge

/// Port of modMMudFunc CalcDodgeVSAccuracy: the dodge percent of `raw_dodge` against an
/// attacker's accuracy. Stock: `raw * 10 \ (accuracy \ 8)` for accuracy above 8, else 0, capped
/// at 95. A negative `raw_dodge` gives 0 straight away.
#[must_use]
pub fn calc_dodge_vs_accuracy(profile: Profile, raw_dodge: i32, accy: i32, _class: i32) -> i32 {
    if raw_dodge < 0 {
        return 0;
    }
    let raw = raw_dodge.min(9999);
    let accy = accy.clamp(0, 9999);
    let pct = match profile {
        Profile::Stock | Profile::Quicksilver(_) => {
            if accy > 8 {
                let temp = (accy / 8).max(1);
                (raw * 10 / temp).min(get_dodge_cap(profile, 0, false))
            } else {
                0
            }
        }
    };
    pct.max(0)
}

/// Port of modMMudFunc CalcDodge: raw dodge from level, agility, charm and the cumulative bonus,
/// plus the low-encumbrance bonus when `max_encum` is above 0. MME's defaults are
/// `current_encum = 0` and `max_encum = -1`. Integer overflow gives 0 (MME's handler).
///
/// Quicksilver (R13) counts a third of the agility above `dodge_agi_knee` in the `(agility - 50) / 3`
/// term.
#[must_use]
pub fn calc_dodge(
    profile: Profile,
    level: i16,
    agility: i16,
    charm: i16,
    plus_dodge: f64,
    current_encum: f64,
    max_encum: f64,
) -> i32 {
    let agility = match profile {
        // The knee only lowers the value, so it still fits an Integer.
        Profile::Quicksilver(b) => Balance::knee_third(i64::from(agility), b.dodge_agi_knee) as i16,
        Profile::Stock => agility,
    };
    let steps = || -> Res<i16> {
        let mut dodge = vb::cint(vb::fix(f64::from(level) / 5.0))?;
        dodge = vb::cint(f64::from(dodge) + vb::fix(f64::from(vb::sub_i16(charm, 50)?) / 5.0))?;
        dodge = vb::cint(f64::from(dodge) + vb::fix(f64::from(vb::sub_i16(agility, 50)?) / 3.0))?;
        // Integer + Double is a Double sum, rounded half to even on the way back (2.5 -> 2).
        dodge = vb::cint(f64::from(dodge) + plus_dodge)?;
        if max_encum > 0.0 {
            let pct = vb::cint(vb::fix(finite(vb::div(current_encum, max_encum)? * 100.0)?))?;
            if pct < 33 {
                let base = vb::add_i16(dodge, 10)?;
                dodge = vb::cint(f64::from(base) - vb::fix(f64::from(pct) / 10.0))?;
            }
        }
        Ok(dodge)
    };
    steps().map_or(0, i32::from)
}

/// Port of modMMudFunc DodgeMaxAccForPercent: the highest accuracy at which `raw_dodge` still
/// dodges at least `target_pct` percent, or -1 when none does (or an argument is not above 0).
#[must_use]
pub fn dodge_max_acc_for_percent(
    profile: Profile,
    raw_dodge: i32,
    target_pct: i32,
    class: i32,
) -> i32 {
    if target_pct <= 0 || raw_dodge <= 0 {
        return -1;
    }
    let raw = raw_dodge.min(9999);
    match profile {
        Profile::Stock | Profile::Quicksilver(_) => {
            let cap = get_dodge_cap(profile, class, false);
            let target = target_pct.min(cap);
            // Best case is accuracy 9..15, where the percent is raw * 10 capped.
            if (raw * 10).min(cap) < target {
                return -1;
            }
            // Exact inverse of pct = (raw * 10) \ (accuracy \ 8): the largest accuracy for
            // k = accuracy \ 8 is 8k + 7, then nudge both ways in case an edge was missed.
            let k = (raw * 10 / target).max(1);
            let mut cand = (8 * k + 7).clamp(9, 1000);
            while cand < 1000 && calc_dodge_vs_accuracy(profile, raw, cand + 1, class) >= target {
                cand += 1;
            }
            while cand >= 9 {
                if calc_dodge_vs_accuracy(profile, raw, cand, class) >= target {
                    break;
                }
                cand -= 1;
            }
            if cand < 9 { -1 } else { cand }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Accuracy

/// Inputs of [`calculate_accuracy`] (MME's Integer arguments). The class row is the one
/// `GetClassCombat(class)` finds; GreaterMUD-only arguments are left out.
#[derive(Debug, Clone, Copy, Default)]
pub struct AccuracyInput<'a> {
    pub class: i16,
    pub class_row: Option<&'a Class>,
    pub level: i16,
    pub strength: i16,
    pub agility: i16,
    /// Accuracy from worn gear (`nAccyWorn`); 0 becomes MME's "pity" 1, negative becomes 0.
    pub accuracy_worn: i16,
    pub plus_accuracy: i16,
    pub encum_pct: i16,
}

/// The accuracy and the breakdown MME builds in `sReturnText`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accuracy {
    pub value: i32,
    pub text: String,
}

/// Port of modMMudFunc CalculateAccuracy. `text_in` is the incoming `sReturnText` (usually
/// empty); the breakdown lines are appended to it. An Integer overflow gives 0 and the text
/// MME had built by then.
#[must_use]
pub fn calculate_accuracy(profile: Profile, input: &AccuracyInput, text_in: &str) -> Accuracy {
    let mut text = text_in.to_string();
    let value = accuracy_steps(profile, input, &mut text).map_or(0, i32::from);
    Accuracy { value, text }
}

fn accuracy_steps(profile: Profile, i: &AccuracyInput, text: &mut String) -> Res<i16> {
    let (Profile::Stock | Profile::Quicksilver(_)) = profile;
    let mut worn = i.accuracy_worn;
    let mut encum_pct = i.encum_pct;
    if worn == 0 {
        worn = 1;
        append_line(text, "Pity Accy (1)");
    }
    if encum_pct == 0 {
        encum_pct = 1;
    }
    if worn < 0 {
        worn = 0;
    }

    let mut calc: i16 = 0;
    if encum_pct < 33 {
        let bonus = 15.0 - vb::fix(f64::from(encum_pct) / 10.0);
        append_line(text, &format!("Encum ({})", vb::cstr(bonus)));
        calc = vb::cint(f64::from(calc) + bonus)?;
    }

    // "how it is in the dll": an odd running total loses one.
    let before = f64::from(calc);
    calc = vb::cint(vb::fix(f64::from(calc) / 2.0) * 2.0)?;
    if before != f64::from(calc) {
        let diff = vb::round(f64::from(calc) - before, 0)?;
        append_line(text, &format!("odd number penalty ({})", vb::cstr(diff)));
    }

    let mut base = 0.0_f64;
    if i.level > 0 {
        base = vb::fix(vb::sqr(f64::from(i.level))?);
        while (base + 1.0) * (base + 1.0) <= f64::from(i.level) {
            base += 1.0;
        }
    }
    if i.class > 0 {
        let combat = vb::add_i16(stats::get_class_combat(i.class_row) as i16, 2)?;
        base *= f64::from(vb::sub_i16(combat, 1)?);
        base = (base + f64::from(vb::mul_i16(combat, 2)?) + vb::fix(f64::from(i.level) / 2.0)
            - 2.0)
            * 2.0;
    }
    if base > 0.0 {
        append_line(text, &format!("Combat+Level ({})", vb::cstr(base)));
    }
    calc = vb::cint(f64::from(calc) + base)?;

    if i.strength > 0 {
        let t = vb::fix(f64::from(vb::sub_i16(i.strength, 50)?) / 3.0);
        if t != 0.0 {
            append_line(text, &format!("Strength ({})", vb::cstr(t)));
            calc = vb::cint(f64::from(calc) + t)?;
        }
    }
    if i.agility > 0 {
        let t = vb::fix(f64::from(vb::sub_i16(i.agility, 50)?) / 6.0);
        if t != 0.0 {
            append_line(text, &format!("Agility ({})", vb::cstr(t)));
            calc = vb::cint(f64::from(calc) + t)?;
        }
    }

    vb::add_i16(vb::add_i16(calc, worn)?, i.plus_accuracy)
}

/// Inputs of [`calculate_backstab_accuracy`]; level, strength and the strength requirement are
/// GreaterMUD only and left out.
#[derive(Debug, Clone, Copy, Default)]
pub struct BackstabAccuracyInput {
    pub stealth: i16,
    pub agility: i16,
    pub plus_bs_accuracy: i16,
    /// True when the class (not just the race) has stealth.
    pub class_stealth: bool,
    pub plus_normal_accuracy: i16,
}

/// Port of modMMudFunc CalculateBackstabAccuracy. An Integer overflow in `stealth + agility`
/// gives 0 (MME's handler).
#[must_use]
pub fn calculate_backstab_accuracy(profile: Profile, input: &BackstabAccuracyInput) -> i32 {
    let (Profile::Stock | Profile::Quicksilver(_)) = profile;
    let Ok(sum) = vb::add_i16(input.stealth, input.agility) else {
        return 0;
    };
    let mut accy = vb::fix(f64::from(sum) / 2.0) as i32
        + vb::fix(f64::from(input.plus_bs_accuracy) / 2.0) as i32;
    accy += if input.class_stealth { 5 } else { -15 };
    accy + i32::from(input.plus_normal_accuracy)
}

// ---------------------------------------------------------------------------------------------
// Hit chance and dodge chance of an attack

/// Inputs of [`calculate_attack_defense`] (MME's Long arguments). `eEvil` is left out: only the
/// GreaterMUD vile-ward rules read it.
#[derive(Debug, Clone, Copy, Default)]
pub struct AttackDefenseInput {
    pub accy: i32,
    pub ac: i32,
    pub dodge: i32,
    pub bs_defense: i32,
    pub prot_evil: i32,
    pub prot_good: i32,
    pub perception: i32,
    pub vile_ward: i32,
    pub shadow: bool,
    pub see_hidden: bool,
    pub backstab: bool,
    pub vs_player: bool,
    pub class: i32,
}

/// Hit and dodge percentages, MME's `nReturn(0)` and `nReturn(1)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackDefense {
    pub hit_chance: i32,
    pub dodge_chance: i32,
}

/// Port of modMMudFunc CalculateAttackDefense: the chance an attack of `accy` lands against the
/// defences, and the chance it is dodged. A Long overflow gives 0 and 0 (MME's handler returns the
/// zeroed array it prepared first).
#[must_use]
pub fn calculate_attack_defense(profile: Profile, input: &AttackDefenseInput) -> AttackDefense {
    attack_defense_steps(profile, input).unwrap_or(AttackDefense {
        hit_chance: 0,
        dodge_chance: 0,
    })
}

fn attack_defense_steps(profile: Profile, i: &AttackDefenseInput) -> Res<AttackDefense> {
    let (Profile::Stock | Profile::Quicksilver(_)) = profile;
    // Only the upper bound is applied: the lower-bound test sits after a colon on the same
    // one-line If, so it runs only when the value was above 9999.
    let cap = |v: i32| v.min(9999);
    let accy = cap(i.accy);
    let ac = cap(i.ac);
    let dodge = cap(i.dodge);
    let prot_ev = cap(i.prot_evil);
    let perception = cap(i.perception);
    let bs_defense = cap(i.bs_defense);
    let vile_ward = cap(i.vile_ward);

    let acc_temp = (vb::mul_i32(accy, accy)? / 140).max(1);
    let shadow_pts = if i.shadow { 10 } else { 0 };

    // IIf evaluates both arms, so the sum can overflow even for a backstab.
    let secondary = vb::add_i32(
        vb::add_i32(vb::add_i32(prot_ev, i.prot_good)?, vile_ward)?,
        shadow_pts,
    )?;
    let extra = if i.backstab { 0 } else { secondary };
    let mut hit = if vb::add_i32(ac, extra)? <= 0 {
        100
    } else if i.backstab {
        let defense = if i.vs_player {
            vb::add_i32(ac, perception)? / 2
        } else if i.see_hidden {
            vb::add_i32(ac, bs_defense)?
        } else {
            vb::add_i32(ac / 4, bs_defense)?
        };
        vb::sub_i32(accy, defense.clamp(0, 9999))?
    } else {
        let defense = vb::add_i32(
            ac,
            vb::add_i32(vb::add_i32(prot_ev, i.prot_good)?, shadow_pts)?,
        )?;
        vb::sub_i32(100, vb::mul_i32(defense, defense)? / acc_temp)?
    };

    // GetHitMin / GetHitCap take an Integer, so a Long class outside Integer overflows here.
    let class = i16::try_from(i.class).map_err(|_| Vb6Error::Overflow)?;
    hit = hit.max(get_hit_min(profile, i32::from(class)));
    hit = hit.min(get_hit_cap(profile));

    let mut dodge_chance = 0;
    if dodge > 0 && accy > 8 {
        dodge_chance = calc_dodge_vs_accuracy(profile, dodge, accy, i.class);
        if i.backstab {
            dodge_chance /= 5;
        }
    }
    Ok(AttackDefense {
        hit_chance: hit,
        dodge_chance,
    })
}
