//! The Classes/Races builder: the text MME prints when a class and a race are both selected, and
//! what the Equip and Spellbook buttons do with the two selections. Ports frmMain
//! `RefreshClassRaceBuilder` and the Equip/Spellbook branches of `cmdClassRace_Click`; the other
//! branches of that handler only show and hide controls, which is the UI's business.

use touchstone_model::{Class, Race};
use touchstone_vb6 as vb;
use touchstone_vb6::Vb6Error;

use crate::attack::{AttackEnv, AttackRequest, CharacterProfile, calculate_attack};
use crate::exp::{calc_exp_needed, round_up_to_5};
use crate::rows::monster::put_commas;
use crate::stats::{
    calc_max_hp, get_class_combat, get_class_magery, get_class_magery_lvl, get_class_max_hp,
    get_class_min_hp, round_up_to_nearest5,
};
use crate::{Profile, RulesError};

fn err(e: Vb6Error) -> RulesError {
    RulesError::Vb6(i32::try_from(e.code()).unwrap_or(0))
}

/// A Long or Double assigned to an Integer variable.
fn to_i16(v: i32) -> Result<i16, RulesError> {
    i16::try_from(v).map_err(|_| RulesError::Vb6(6))
}

/// What the builder shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    /// The text box, lines joined by CRLF. Empty when there is nothing to show.
    pub text: String,
    /// The five columns' lines as MME builds them (before empty ones are dropped).
    pub lines: Vec<String>,
    /// `nLevelSteps(0..=4)`: 1, 10, 25 and the two computed levels.
    pub levels: [i32; 5],
}

/// Port of frmMain `RefreshClassRaceBuilder`. `class` and `race` are the selected record numbers
/// (0 for no selection); `env` carries the tables and `avg_level_max_single_stat` is the global
/// `gAvgLevelMaxSingleStat`.
///
/// # Errors
/// A VB6 run-time error MME's handler catches. MME then leaves the text box as it was, so the
/// caller keeps whatever it showed before.
#[allow(clippy::too_many_lines)] // one MME procedure, in its order
pub fn build(
    profile: Profile,
    env: &AttackEnv,
    avg_level_max_single_stat: f64,
    class: i32,
    race: i32,
) -> Result<Built, RulesError> {
    let avg = avg_level_max_single_stat;
    let mut n_max_level_calc = vb::clng(avg).map_err(err)?;
    if n_max_level_calc < 50 {
        n_max_level_calc = 50;
    } else if n_max_level_calc > 100 {
        // `/` is Double division; the Long assignment rounds half to even.
        n_max_level_calc = vb::clng(f64::from(n_max_level_calc) / 2.0).map_err(err)?;
    }

    let mut steps = [0_i16; 5];
    steps[0] = 1;
    steps[1] = 10;
    steps[2] = 25;
    steps[3] = to_i16(n_max_level_calc)?;
    if f64::from(steps[3]) > avg * 3.0 {
        steps[3] = vb::cint(avg * 3.0).map_err(err)?;
    }
    steps[3] = to_i16(round_up_to_5(i32::from(steps[3]))?)?;

    steps[4] = to_i16(vb::mul_i32(n_max_level_calc, 2).map_err(err)?)?;
    if f64::from(steps[4]) > avg * 6.0 {
        steps[4] = vb::cint(avg * 6.0).map_err(err)?;
    }
    steps[4] = to_i16(round_up_to_5(i32::from(steps[4]))?)?;

    let mut class_row = None;
    let mut race_row = None;
    let mut n_class = class;
    let mut n_race = race;
    let mut class_exp: i32 = 0;
    let mut race_exp: i32 = 0;
    let (mut min_hea, mut max_hea, mut min_agi, mut max_agi, mut race_bonus) = (0, 0, 0, 0, 0);

    if n_class > 0 {
        match env.classes.iter().find(|c| c.number == n_class) {
            None => n_class = 0,
            Some(c) => {
                // `Fields("ExpTable") + 100` is Variant arithmetic on an Integer column.
                class_exp = i32::from(
                    vb::add_i16(vb::cint(f64::from(c.exp_table)).map_err(err)?, 100)
                        .map_err(err)?,
                );
                class_row = Some(c);
            }
        }
    }
    if n_race > 0 {
        match env.races.iter().find(|r| r.number == n_race) {
            None => n_race = 0,
            Some(r) => {
                race_exp = r.exp_table;
                min_hea = r.m_hea;
                max_hea = r.x_hea;
                min_agi = r.m_agl;
                max_agi = r.x_agl;
                race_bonus = r.hp_per_lvl;
                race_row = Some(r);
            }
        }
    }
    let _ = race_row;

    let chart = vb::add_i32(class_exp, race_exp).map_err(err)?;
    let mut out: [String; 5] = Default::default();
    if chart > 0 {
        out[0] = format!("EXP Chart: {chart}%");
        for x in 1..=4 {
            // CalcExpNeeded has its own handler: an overflow inside it gives 0.
            let exp = calc_exp_needed(profile, i32::from(steps[x]), chart).unwrap_or(0.0);
            out[x] = format!("EXP @ LVL {}: {}", steps[x], put_commas(&vb::cstr(exp)));
        }
    }

    let mut max_len: usize = 0;
    let mut space = [0_usize; 5];
    pad(&out, &mut max_len, &mut space, 5);

    if n_class > 0 && max_hea > 0 {
        let min_roll = get_class_min_hp(class_row);
        let max_roll = get_class_max_hp(class_row);
        out[0] = format!(
            "{}{}HP Range: {min_roll}-{max_roll}",
            out[0],
            spaces(space[0])
        );
        for x in 1..=4 {
            let health: i32 = match x {
                1 => min_hea,
                2 => half_way(min_hea, max_hea, 0.5)?,
                3 => half_way(min_hea, max_hea, 0.75)?,
                _ => max_hea,
            };
            let health = i32::from(to_i16(health)?);
            let level = i32::from(steps[x]);
            let bonus = vb::mul_i32(race_bonus, level).map_err(err)?;
            let spread = vb::sub_i32(max_roll, min_roll).map_err(err)?;
            let lo =
                vb::add_i32(calc_max_hp(spread, level, health, min_roll), bonus).map_err(err)?;
            let hi_roll = vb::mul_i32(spread, level).map_err(err)?;
            let hi =
                vb::add_i32(calc_max_hp(hi_roll, level, health, min_roll), bonus).map_err(err)?;
            out[x] = format!(
                "{}{}HP @ LVL {level}/{health} HEA: {lo}-{hi}",
                out[x],
                spaces(space[x])
            );
        }
    }

    if n_class > 0 && max_agi > 0 {
        pad(&out, &mut max_len, &mut space, 4);
        let mut ch = CharacterProfile {
            class: n_class,
            race: n_race,
            combat: to_i16(get_class_combat(class_row))?,
            encum_current: 1,
            encum_max: 100,
            encum_pct: 1,
            ..CharacterProfile::default()
        };
        out[0] = format!(
            "{}{}SPEED/DMG @ LVL/AGI: Weapon Swings (DMG)",
            out[0],
            spaces(space[0])
        );
        for x in 1..=4 {
            ch.level = i32::from(steps[x]);
            let spread = f64::from(vb::sub_i32(max_agi, min_agi).map_err(err)?);
            let (tag, agi) = match x {
                1 => (" 2000/10", f64::from(min_agi)),
                2 => (" 3000/20", f64::from(min_agi) + spread * 0.5),
                3 => (" 4000/40", f64::from(min_agi) + spread * 0.75),
                _ => (" 5000/80", f64::from(max_agi)),
            };
            ch.agility = vb::cint(agi).map_err(err)?;
            let req = AttackRequest {
                attack_type: 5,
                // proxy weapons -2 to -5 stand for 10, 20, 40 and 80 damage
                weapon_number: -(i32::try_from(x).unwrap_or(0) + 1),
                // the twelfth argument of CalculateAttack: the accuracy to use
                specify_accy: 100.0,
                ..AttackRequest::default()
            };
            let mut casts = String::new();
            let attack = calculate_attack(profile, env, &ch, &req, &mut casts);
            out[x] = format!(
                "{}{}{tag} @ LVL {}/{} AGI: {} swings ({})",
                out[x],
                spaces(space[x]),
                steps[x],
                ch.agility,
                vb::cstr(truncate(attack.swings, 2)),
                attack.round_total
            );
        }
    }

    let mut text = String::new();
    for line in &out {
        text = auto_append(&text, line, "\r\n");
    }
    Ok(Built {
        text,
        lines: out.to_vec(),
        levels: steps.map(i32::from),
    })
}

/// `RoundUpToNearest5(min + (max - min) * fraction)`: the Double argument becomes a Long.
fn half_way(min: i32, max: i32, fraction: f64) -> Result<i32, RulesError> {
    let spread = vb::sub_i32(max, min).map_err(err)?;
    let at = f64::from(min) + f64::from(spread) * fraction;
    round_up_to_nearest5(vb::clng(at).map_err(err)?)
}

/// The padding pass: the widest line so far (never reset) plus `gap`, per line.
fn pad(out: &[String; 5], max_len: &mut usize, space: &mut [usize; 5], gap: usize) {
    for line in out {
        *max_len = (*max_len).max(line.chars().count());
    }
    if *max_len > 0 {
        for (s, line) in space.iter_mut().zip(out) {
            *s = *max_len - line.chars().count() + gap;
        }
    }
}

fn spaces(n: usize) -> String {
    " ".repeat(n)
}

/// modSyntaxsFunc Truncate with two places.
fn truncate(x: f64, places: i32) -> f64 {
    let scale = 10f64.powi(places);
    vb::fix(x * scale) / scale
}

/// modSyntaxsFunc AutoAppend.
fn auto_append(base: &str, add: &str, glue: &str) -> String {
    if base.is_empty() {
        add.to_string()
    } else if add.trim_matches(' ').is_empty() {
        base.to_string()
    } else {
        format!("{base}{glue}{add}")
    }
}

// ---------------------------------------------------------------------------------------------
// Buttons

/// The Equip button: which of the two selections the EQ tab's class and race pickers take.
/// A picker is set only when it has an entry for the selected number (`ItemData` match); the
/// button then opens the EQ tab (`cmdNav_Click 5`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EquipSelection {
    pub class: Option<i32>,
    pub race: Option<i32>,
}

/// Port of the `Index = 3` branch of `cmdClassRace_Click`.
#[must_use]
pub fn equip_selection(classes: &[Class], races: &[Race], class: i32, race: i32) -> EquipSelection {
    EquipSelection {
        class: classes.iter().any(|c| c.number == class).then_some(class),
        race: races.iter().any(|r| r.number == race).then_some(race),
    }
}

/// What the Spellbook button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spellbook {
    /// No class selected: the click does nothing.
    Nothing,
    /// The class has no magery: MME shows "No Magic".
    NoMagic,
    /// Open the spell book on this class: level 999, the class's magery kind and magery level
    /// (as list positions of the book's combos) and the first alignment.
    Open {
        class: i32,
        level: i32,
        magery: i32,
        magery_level: i32,
    },
}

/// Port of the `Index = 4` branch of `cmdClassRace_Click`.
#[must_use]
pub fn spellbook(classes: &[Class], class: i32) -> Spellbook {
    if class <= 0 {
        return Spellbook::Nothing;
    }
    let row = classes.iter().find(|c| c.number == class);
    let magery = get_class_magery(row);
    if magery == touchstone_model::Magery::None {
        return Spellbook::NoMagic;
    }
    Spellbook::Open {
        class,
        level: 999,
        magery: magery as i32,
        magery_level: get_class_magery_lvl(row),
    }
}
