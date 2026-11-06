//! Gear aggregation II (FAM-04): the second half of frmMain `CalcCharacterStats`, from the
//! `'stealth` banner on. Card P3-18.
//!
//! It continues on the [`CharStats`] the first half leaves (`encum_pct`, the stealth flags and the
//! stat tags) and is stock-engine only, like the first half. What follows the tag pass in MME
//! (`RefreshMagic`, `InvenColorCodeStats`, `GetDamageOutput` for stat 46, the panel refreshes and
//! the tooltip text) belongs to later cards or is display: stat 46 stays "0" here.

use touchstone_vb6 as vb;
use touchstone_vb6::Vb6Error;

use super::{CharStats, GearInput, Res, STAT_COUNT, calc_gear_stats, to_integer, to_long};
use crate::accuracy::{AccuracyInput, calc_dodge, calculate_accuracy};
use crate::attack::{QndContext, get_quick_and_deadly_bonus};
use crate::stats::{StealthInput, calc_movement_speed, calc_mr, calculate_stealth};
use crate::{Profile, RulesError};

/// `eAttackTypeMUD`-style values of `nGlobalAttackTypeMME` the second half tests.
const A6_PHYS_BASH: i32 = 6;
const A7_PHYS_SMASH: i32 = 7;

/// What the second half reads beyond [`GearInput`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SecondHalfInput {
    /// `chkCharQuests(0..11).Value = 1`. On the stock engine only 0..5 count.
    pub quests: [bool; 12],
    /// `cmbCharQuestOpts(0..3).ListIndex` (-1 or 0 for nothing chosen).
    pub quest_options: [i32; 4],
    /// `chkGlobalFilter.Value`: Quick and Deadly only counts with the global filter on.
    pub global_filter: i32,
    /// `nGlobalAttackTypeMME`.
    pub attack_type: i32,
}

/// Port of frmMain `CalcCharacterStats`, both halves: [`super::calc_gear_stats`] then stealth, the
/// strength bonuses, quest bonuses, accuracy, crits, magic resistance, dodge, regen text boxes,
/// movement speed and the tag pass. A VB6 error ends the calculation where MME's handler would
/// and shows in [`CharStats::error`]; if the first half failed, the second does not run.
///
/// # Errors
/// Never in practice (the signature matches [`super::calc_gear_stats`]).
pub fn calc_character_stats(
    input: &GearInput,
    second: &SecondHalfInput,
) -> Result<CharStats, RulesError> {
    let mut s = calc_gear_stats(input)?;
    if s.error.is_none()
        && let Err(e) = run(input, second, &mut s)
    {
        s.error = Some(i32::try_from(e.code()).unwrap_or(0));
    }
    Ok(s)
}

fn cint_f(x: f64) -> Res<i16> {
    vb::cint(x)
}

fn run(input: &GearInput, q: &SecondHalfInput, s: &mut CharStats) -> Res<()> {
    let Profile::Stock = input.profile;
    let world = input.world;
    let who = &input.character;
    let hide = input.hide_class_race;
    let level = who.level;
    let t = s.stat_with_bonus; // Val(txtCharStats(i).Tag)

    // stealth
    if !hide && (s.race_stealth || s.class_stealth) {
        let si = StealthInput {
            level: to_integer(level)?,
            agility: cint_f(t[3])?,
            intellect: cint_f(t[1])?,
            charm: cint_f(t[5])?,
            class_stealth: s.class_stealth,
            race_stealth: s.race_stealth,
            plus_stealth: cint_f(s.stat[19])?,
            encum_pct: i16::try_from(s.encum_pct).map_err(|_| Vb6Error::Overflow)?,
        };
        let r = calculate_stealth(input.profile, &si, "");
        s.put(19, f64::from(r.value));
    }

    // strength: max damage, then spell damage (twice as fast on the stock engine)
    if t[0] != 0.0 && !hide {
        let bonus = to_long(vb::fix((t[0] - 50.0) / 10.0))?;
        if bonus != 0 {
            s.add(11, f64::from(bonus));
        }
        let mut bonus = to_long(vb::fix((t[0] - 100.0) / 10.0))?;
        bonus = vb::mul_i32(bonus, 2)?;
        if bonus < 0 {
            bonus = 0;
        }
        if bonus > 0 {
            s.add(30, f64::from(bonus));
        }
    }

    if !hide {
        quest_bonuses(q, s)?;
    }

    // accuracy
    let accy_pair = vb::add_i32(s.accy_other, s.accy_abils)?;
    let class16 = to_integer(who.class)?;
    let accy = if hide {
        let ai = AccuracyInput {
            accuracy_worn: to_integer(s.accy_items)?,
            plus_accuracy: to_integer(accy_pair)?,
            encum_pct: i16::try_from(s.encum_pct).map_err(|_| Vb6Error::Overflow)?,
            ..AccuracyInput::default()
        };
        calculate_accuracy(input.profile, &ai, "")
    } else {
        let ai = AccuracyInput {
            class: class16,
            class_row: world.get_class(who.class),
            level: to_integer(level)?,
            strength: cint_f(t[0])?,
            agility: cint_f(t[3])?,
            accuracy_worn: {
                // intellect and charm are converted too, though the stock formula ignores them
                cint_f(t[1])?;
                cint_f(t[5])?;
                to_integer(s.accy_items)?
            },
            plus_accuracy: to_integer(accy_pair)?,
            encum_pct: i16::try_from(s.encum_pct).map_err(|_| Vb6Error::Overflow)?,
        };
        calculate_accuracy(input.profile, &ai, "")
    };
    s.put(10, f64::from(accy.value));
    s.attack_accy_adj = 0;
    if s.stat[10] > 0.0 {
        if q.attack_type == A6_PHYS_BASH {
            s.attack_accy_adj = -15;
        } else if q.attack_type == A7_PHYS_SMASH {
            s.attack_accy_adj = -25;
        }
    }

    // critical hit
    if (level > 0 || t[1] > 0.0 || t[3] > 0.0) && !hide {
        let mut crit = 0.0_f64;
        crit += vb::fix(f64::from(level) / 10.0);
        crit += vb::fix((t[3] - 50.0) / 20.0);
        crit += vb::fix((t[1] - 50.0) / 10.0);
        crit += vb::fix((t[5] - 50.0) / 30.0);
        let mut crit = to_long(crit)?;
        crit = crit.clamp(1, 75);
        let weapon = input.equipped[16];
        if weapon > 0 {
            let strength = vb::cstr(t[0]);
            let agility = vb::cstr(t[3]);
            let cur = vb::cstr(s.stat[0]);
            let max = vb::cstr(s.stat[1]);
            let level_text = level.to_string();
            let ctx = QndContext {
                global_filter: q.global_filter,
                class_number: Some(who.class),
                strength_tag: &strength,
                agility_tag: &agility,
                encum_current_caption: &cur,
                encum_max_caption: &max,
                level_text: &level_text,
            };
            let bonus = get_quick_and_deadly_bonus(&world.items, &world.classes, &ctx, weapon);
            if bonus > 0 {
                s.qnd_bonus = i32::from(bonus);
                crit = vb::add_i32(crit, i32::from(bonus))?;
            }
        }
        let mut total = to_long(f64::from(crit) + s.stat[7])?;
        if total < 0 {
            total = 0;
        }
        s.put(7, f64::from(total));
    }

    // magic resistance
    if hide {
        s.put(24, f64::from(s.plus_mr));
    } else {
        let mr = calc_mr(to_long(t[1])?, to_long(t[2])?, s.plus_mr);
        s.put(24, f64::from(mr));
    }

    // dodge
    if (level > 0 || t[5] > 0.0 || t[3] > 0.0) && !hide {
        let d = calc_dodge(
            to_integer(level)?,
            cint_f(t[3])?,
            cint_f(t[5])?,
            f64::from(s.plus_dodge),
            s.stat[0],
            s.stat[1],
        );
        s.put(8, f64::from(d));
    }
    s.hp_regen = s.stat[16];
    s.mana_regen = s.stat[17];

    // movement speed
    let quickness = to_long(s.stat[31])?;
    let ms = calc_movement_speed(input.profile, s.encum_pct, quickness, 0);
    s.walk_speed = vb::round(f64::from(ms) / 1000.0, 0)?;

    // tag pass: AC and DR tags are whole numbers, the rest copy the caption
    s.stat_tag[2] = Some(vb::fix(s.stat[2]));
    s.stat_tag[3] = Some(vb::fix(s.stat[3]));
    for x in 0..STAT_COUNT {
        if s.stat_tag[x].is_none() {
            s.stat_tag[x] = Some(s.stat[x]);
        }
    }
    Ok(())
}

/// The `'quest bonuses` loop. Quests 6 to 11 are GreaterMUD only and skipped on the stock engine.
fn quest_bonuses(q: &SecondHalfInput, s: &mut CharStats) -> Res<()> {
    for x in 0..6usize {
        if !q.quests[x] {
            continue;
        }
        match x {
            0 => s.add(2, 1.0),
            1 => s.add(9, 1.0),
            2 => {
                s.add(7, 1.0);
                s.add(9, 2.0);
            }
            3 => {
                // the bishop's +3 replaces a smaller ability accuracy, it does not stack
                if s.accy_abils < 3 {
                    s.accy_abils = 3;
                }
            }
            4 => s.plus_dodge = vb::add_i32(s.plus_dodge, 1)?,
            _ => second_alignment(q.quest_options[0], s)?,
        }
    }
    Ok(())
}

/// `cmbCharQuestOpts(0)` choices of the second alignment quest (stock branches).
fn second_alignment(option: i32, s: &mut CharStats) -> Res<()> {
    match option {
        1 => s.add(11, 1.0),
        2 => {
            s.add(2, 1.0);
            s.add(6, 6.0);
        }
        3 => {
            s.add(9, 1.0);
            s.add(6, 10.0);
        }
        4 => {
            s.add(6, 4.0);
            s.add(14, 6.0);
            s.add(15, 6.0);
            s.add(19, 1.0);
        }
        5 => {
            s.add(14, 10.0);
            s.add(15, 10.0);
            s.add(19, 2.0);
        }
        6 => {
            s.add(11, 1.0);
            s.accy_abils = vb::add_i32(s.accy_abils, 5)?;
            s.add(2, 1.0);
        }
        7 => {
            s.add(11, 1.0);
            s.accy_abils = vb::add_i32(s.accy_abils, 10)?;
        }
        _ => {}
    }
    Ok(())
}
