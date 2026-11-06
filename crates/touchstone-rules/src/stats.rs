//! Stats, vitals, creation points and movement (FAM-02). Card P3-02.
//!
//! Functions take plain inputs (numbers, `Race`/`Class` rows) instead of MME's globals and form
//! controls. A recordset lookup (`tabRaces.Seek`) becomes an `Option<&Race>`: pass the row
//! `World::get_race(n)` finds, or `None` where MME's seek fails (or `nNum` is 0). All of MME's
//! lookups give the same answer for "0" and "no match", so one `None` covers both.
//!
//! Where MME's procedure has an `On Error` handler the port returns what the handler leaves
//! behind (usually the default result, sometimes a partly computed one), not an error. Where an
//! error escapes MME (no handler) the port returns `RulesError::Vb6`.

use touchstone_model::{Class, Magery, Race};
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Vb6Error};

use crate::{Profile, RulesError};

fn vb6(e: Vb6Error) -> RulesError {
    RulesError::Vb6(i32::try_from(e.code()).unwrap_or(0))
}

fn cur_long(v: i32) -> Currency {
    Currency::from_scaled(i64::from(v) * 10_000)
}

/// `Fix` of a Currency value: truncation toward zero, still a Currency.
fn fix_cur(c: Currency) -> Currency {
    Currency::from_scaled(c.scaled() / 10_000 * 10_000)
}

// ---------------------------------------------------------------------------------------------
// Races and Classes row getters (modMMudDatabase)

/// Assigning a Long (a table field) to an Integer result: out of range is error 6, which the
/// getter's handler turns into `fallback`.
fn integer_or(v: i64, fallback: i32) -> i32 {
    if i16::try_from(v).is_ok() {
        v as i32
    } else {
        fallback
    }
}

/// Port of modMMudDatabase GetRaceHPBonus: the race's `HPPerLVL`, 0 for no race.
#[must_use]
pub fn get_race_hp_bonus(race: Option<&Race>) -> i32 {
    race.map_or(0, |r| integer_or(i64::from(r.hp_per_lvl), 0))
}

/// Port of modMMudDatabase GetClassMaxHP: `MinHits + MaxHits`, 0 for no class.
#[must_use]
pub fn get_class_max_hp(class: Option<&Class>) -> i32 {
    class.map_or(0, |c| {
        integer_or(i64::from(c.min_hits) + i64::from(c.max_hits), 0)
    })
}

/// Port of modMMudDatabase GetClassMinHP: `MinHits`, 0 for no class.
#[must_use]
pub fn get_class_min_hp(class: Option<&Class>) -> i32 {
    class.map_or(0, |c| integer_or(i64::from(c.min_hits), 0))
}

/// Port of modMMudDatabase GetClassMageryLVL: `MageryLVL`, 0 for no class.
#[must_use]
pub fn get_class_magery_lvl(class: Option<&Class>) -> i32 {
    class.map_or(0, |c| integer_or(i64::from(c.magery_lvl), 0))
}

/// Port of modMMudDatabase GetClassMagery: `MageryType` as a magic kind; codes above 5 and a
/// missing class are `Magery::None`.
#[must_use]
pub fn get_class_magery(class: Option<&Class>) -> Magery {
    class
        .and_then(|c| Magery::from_code(c.magery_type))
        .unwrap_or(Magery::None)
}

/// Port of modMMudDatabase GetClassArmourType: `ArmourType`, 0 for no class.
#[must_use]
pub fn get_class_armour_type(class: Option<&Class>) -> i32 {
    class.map_or(0, |c| integer_or(i64::from(c.armour_type), 0))
}

/// Port of modMMudDatabase GetClassCombat: `CombatLVL - 2`, 1 for no class.
#[must_use]
pub fn get_class_combat(class: Option<&Class>) -> i32 {
    class.map_or(1, |c| integer_or(i64::from(c.combat_lvl) - 2, 1))
}

/// Port of modMMudDatabase GetRaceCP: the race's `BaseCP`, 100 for no race.
#[must_use]
pub fn get_race_cp(race: Option<&Race>) -> i32 {
    race.map_or(100, |r| integer_or(i64::from(r.base_cp), 100))
}

/// Port of modMMudDatabase GetRaceStealth: whether the race has ability 102.
#[must_use]
pub fn get_race_stealth(race: Option<&Race>) -> bool {
    race.is_some_and(|r| r.abilities.iter().any(|a| a.ability == 102))
}

/// Port of modMMudDatabase GetClassStealth: whether the class has ability 103.
#[must_use]
pub fn get_class_stealth(class: Option<&Class>) -> bool {
    class.is_some_and(|c| c.abilities.iter().any(|a| a.ability == 103))
}

/// A global filter combo box (`cmbGlobalRace(0)` / `cmbGlobalClass(0)`): its `ItemData` values
/// and the selected `ListIndex`.
#[derive(Debug, Clone, Copy)]
pub struct ComboSelection<'a> {
    /// `frmMain.chkGlobalFilter.Value = 1`.
    pub global_filter: bool,
    pub item_data: &'a [i32],
    pub list_index: i32,
}

impl ComboSelection<'_> {
    /// `ItemData(ListIndex)`; an index outside the list reads as 0 (MME raises error 381 there,
    /// and its handler ends in the same "no match" answer).
    fn selected(&self) -> i32 {
        usize::try_from(self.list_index)
            .ok()
            .and_then(|i| self.item_data.get(i))
            .copied()
            .unwrap_or(0)
    }

    fn active(&self) -> bool {
        self.global_filter && !self.item_data.is_empty()
    }
}

/// The record number GetRaceStealth looks up: `n`, or when `n` is 0 the race selected in the
/// global filter combo. 0 means MME exits with False.
#[must_use]
pub fn stealth_race_number(n: i32, combo: &ComboSelection) -> i32 {
    let n = if n == 0 && combo.active() {
        combo.selected()
    } else {
        n
    };
    // GetRaceStealth: `If nNum <= 0 Then Exit Function` after the combo lookup (only when 0
    // was passed); an explicit negative number simply fails the seek.
    n.max(0)
}

/// The record number GetClassStealth looks up (see [`stealth_race_number`]).
#[must_use]
pub fn stealth_class_number(n: i32, combo: &ComboSelection) -> i32 {
    if n == 0 && combo.active() {
        combo.selected()
    } else {
        n
    }
}

/// What ComputeAvgLevelMaxStats leaves in `gAvgLevelMaxAllStats` / `gAvgLevelMaxSingleStat`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AvgLevelMaxStats {
    pub all_stats: f64,
    pub single_stat: f64,
}

/// Port of modMMudDatabase CP_CostForRaise: creation points to raise a stat by `delta`; each
/// block of ten points costs one more per point, up to a per-point cap.
///
/// # Errors
/// `Vb6(6)` where the Long arithmetic overflows (a delta above about 214 million).
pub fn cp_cost_for_raise(profile: Profile, delta: i32) -> Result<i32, RulesError> {
    if delta <= 0 {
        return Ok(0);
    }
    let cap: i32 = match profile {
        Profile::Stock | Profile::Quicksilver(_) => 10,
    };
    let tens = delta / 10;
    let rem_pts = delta % 10;
    let cost = if tens >= cap {
        // 10 * ((cap - 1) * cap \ 2), then the points beyond (cap-1)*10 at `cap` each.
        let first = 10 * ((cap - 1) * cap / 2);
        let beyond =
            vb::mul_i32(vb::sub_i32(delta, 10 * (cap - 1)).map_err(vb6)?, cap).map_err(vb6)?;
        vb::add_i32(first, beyond).map_err(vb6)?
    } else {
        let full = vb::mul_i32(10, vb::mul_i32(tens, tens + 1).map_err(vb6)? / 2).map_err(vb6)?;
        vb::add_i32(full, vb::mul_i32(rem_pts, tens + 1).map_err(vb6)?).map_err(vb6)?
    };
    Ok(cost)
}

/// Port of modMMudDatabase LevelNeededForCP: the first level at which `base_cp` plus the
/// per-level creation points reaches `need_cp` (capped at 2000).
///
/// # Errors
/// `Vb6(6)` if the running total overflows a Long.
pub fn level_needed_for_cp(base_cp: i32, need_cp: i32) -> Result<i32, RulesError> {
    let mut total = base_cp;
    let mut level: i32 = 1;
    while total < need_cp && level < 2000 {
        let gain = vb::add_i32(10, 5 * (level / 10)).map_err(vb6)?;
        total = vb::add_i32(total, gain).map_err(vb6)?;
        level += 1;
    }
    Ok(level)
}

/// Port of modMMudDatabase ComputeAvgLevelMaxStats: the average level at which a character
/// could max every stat of a race, and the average for a single stat, over all races.
///
/// `races` must be in table order (by `Number`). MME declares its running cost inside the loop,
/// so it is never reset between races and the "all stats" figure accumulates across them; that
/// is reproduced. An overflow anywhere gives MME's handler result, zero for both.
#[must_use]
pub fn compute_avg_level_max_stats(profile: Profile, races: &[Race]) -> AvgLevelMaxStats {
    run_avg_level_max_stats(profile, races).unwrap_or(AvgLevelMaxStats {
        all_stats: 0.0,
        single_stat: 0.0,
    })
}

fn run_avg_level_max_stats(
    profile: Profile,
    races: &[Race],
) -> Result<AvgLevelMaxStats, RulesError> {
    if races.is_empty() {
        return Ok(AvgLevelMaxStats {
            all_stats: 0.0,
            single_stat: 0.0,
        });
    }
    let mut sum_lvl_all = 0.0_f64;
    let mut sum_lvl_single = 0.0_f64;
    let mut race_count: i32 = 0;
    let mut single_count: i32 = 0;
    // `Dim totalCost As Long` sits inside the Do loop in MME: one variable for the whole run.
    let mut total_cost: i32 = 0;
    for r in races {
        let delta = |m: i32, x: i32| -> Result<i32, RulesError> {
            if x > m {
                vb::sub_i32(x, m).map_err(vb6)
            } else {
                Ok(0)
            }
        };
        let deltas = [
            delta(r.m_str, r.x_str)?,
            delta(r.m_int, r.x_int)?,
            delta(r.m_wil, r.x_wil)?,
            delta(r.m_agl, r.x_agl)?,
            delta(r.m_hea, r.x_hea)?,
            delta(r.m_chm, r.x_chm)?,
        ];
        // GetRaceCP_Safe: the race's own BaseCP, 0 when its number is not positive.
        let base_cp = if r.number > 0 {
            get_race_cp(Some(r))
        } else {
            0
        };
        for d in deltas {
            total_cost = vb::add_i32(total_cost, cp_cost_for_raise(profile, d)?).map_err(vb6)?;
        }
        sum_lvl_all += f64::from(level_needed_for_cp(base_cp, total_cost)?);
        race_count += 1;
        for d in deltas {
            let cost_one = cp_cost_for_raise(profile, d)?;
            sum_lvl_single += f64::from(level_needed_for_cp(base_cp, cost_one)?);
            single_count += 1;
        }
    }
    Ok(AvgLevelMaxStats {
        all_stats: sum_lvl_all / f64::from(race_count),
        single_stat: sum_lvl_single / f64::from(single_count),
    })
}

// ---------------------------------------------------------------------------------------------
// modMMudFunc

/// Port of modMMudFunc CalcEncum: carrying capacity from strength.
///
/// `dat_ver_caption` is `frmMain.lblDatVer.Caption`; a caption ending in `v1.11i` (any case)
/// uses the flat 48 per point. MME multiplies two Integers there, so strength above 682
/// overflows and its handler leaves 0.
#[must_use]
pub fn calc_encum(strength: i16, encum_bonus: i16, dat_ver_caption: &str) -> i32 {
    if strength < 0 {
        return 0;
    }
    let tail = vb::right(dat_ver_caption, 6).unwrap_or_default();
    let mut r: i32 = if tail.to_lowercase() == "v1.11i" || strength < 101 {
        match vb::mul_i16(strength, 48) {
            Ok(v) => i32::from(v),
            Err(_) => return 0,
        }
    } else {
        4800 + (i32::from(strength) - 100) * 84
    };
    if encum_bonus > 0 {
        let base = f64::from(r);
        match vb::clng(base + (base * (f64::from(encum_bonus) / 100.0))) {
            Ok(v) => r = v,
            Err(_) => return r,
        }
    }
    r
}

/// Port of modMMudFunc GetMageryEnum: the magic kind's name, with `-level` unless it is None.
#[must_use]
pub fn get_magery_enum(kind: i16, level: i16) -> String {
    let mut s = match kind {
        0 => "None".to_string(),
        1 => "Mage".to_string(),
        2 => "Priest".to_string(),
        3 => "Druid".to_string(),
        4 => "Bard".to_string(),
        5 => "Kai".to_string(),
        n => format!("Unknown ({n})"),
    };
    if kind != 0 {
        s.push('-');
        s.push_str(&level.to_string());
    }
    s
}

/// Port of modMMudFunc CalcMovementSpeed: milliseconds per step.
#[must_use]
pub fn calc_movement_speed(profile: Profile, encum_pct: i32, quickness: i32, slowness: i32) -> i32 {
    let mut speed = match profile {
        Profile::Stock | Profile::Quicksilver(_) => {
            let mut s = if encum_pct > 66 { 2000 } else { 1000 };
            if slowness > 0 {
                s *= 2;
            }
            if quickness > 0 {
                s /= 2;
            }
            s
        }
    };
    if speed < 1000 {
        speed = 1000;
    }
    speed
}

/// Port of modMMudFunc CalcRestingRate: HP regenerated per tick. MME's resting-rate form: the
/// caller multiplies by 3 for resting. Returns -1 where MME's Long arithmetic overflows.
#[must_use]
pub fn calc_resting_rate(
    profile: Profile,
    level: i32,
    health: i32,
    hp_regen_percent: i32,
    resting: bool,
) -> i32 {
    let run = || -> Result<i32, Vb6Error> {
        let mut regen = match profile {
            Profile::Stock | Profile::Quicksilver(_) => {
                let prod = vb::mul_i32(vb::add_i32(level, 20)?, health)?;
                vb::clng(vb::fix(f64::from(prod) / 750.0))?
            }
        };
        if regen < 1 {
            regen = 1;
        }
        if resting {
            regen = vb::mul_i32(regen, 3)?;
        }
        let num = vb::mul_i32(vb::add_i32(hp_regen_percent, 100)?, regen)?;
        vb::clng(vb::fix(f64::from(num) / 100.0))
    };
    run().unwrap_or(-1)
}

/// Inputs of [`calc_mana_regen`].
#[derive(Debug, Clone, Copy)]
pub struct ManaRegenInput {
    pub level: i32,
    pub intellect: i32,
    pub willpower: i32,
    pub charm: i32,
    pub magic_level: i32,
    /// `enmMagicEnum`: 0 none, 1 mage, 2 priest, 3 druid, 4 bard, 5 kai.
    pub magic_type: i32,
    /// Extra mana regeneration percent (`nMPRegen`).
    pub mp_regen: i32,
    pub meditating: bool,
}

/// Port of modMMudFunc CalcManaRegen: mana regenerated per tick (a Currency in MME, always a
/// whole number). On an overflow MME's handler returns whatever was assigned so far.
#[must_use]
pub fn calc_mana_regen(profile: Profile, input: &ManaRegenInput) -> Currency {
    let mut r = Currency::from_scaled(0);
    let _ = mana_regen_steps(profile, input, &mut r);
    r
}

fn mana_regen_steps(
    profile: Profile,
    i: &ManaRegenInput,
    r: &mut Currency,
) -> Result<(), Vb6Error> {
    match i.magic_type {
        1 => *r = cur_long(i.intellect),
        2 => *r = cur_long(i.willpower),
        3 => {
            let sum = vb::add_i32(i.intellect, i.willpower)?;
            *r = Currency::from_f64(vb::fix(f64::from(sum) / 2.0))?;
        }
        4 => *r = cur_long(i.charm),
        5 => {
            let num = vb::add_i32(i.mp_regen, 100)?;
            *r = Currency::from_f64(vb::fix(f64::from(num) / 100.0))?;
            return Ok(());
        }
        _ => return Ok(()),
    }
    let lvl = cur_long(vb::add_i32(i.level, 20)?);
    let scaled = lvl.checked_mul(*r)?;
    let mult = cur_long(vb::add_i32(i.magic_level, 2)?);
    let q = scaled.checked_mul(mult)?.checked_div(cur_long(1650))?;
    *r = fix_cur(q);
    if i.meditating {
        return Ok(());
    }
    match profile {
        Profile::Stock | Profile::Quicksilver(_) => {
            let pct = cur_long(vb::add_i32(i.mp_regen, 100)?);
            let q = pct.checked_mul(*r)?.checked_div(cur_long(100))?;
            *r = fix_cur(q);
        }
    }
    Ok(())
}

/// Port of modMMudFunc CalcMR: magic resistance from intellect and willpower; 0 on overflow.
#[must_use]
pub fn calc_mr(intellect: i32, willpower: i32, modifiers: i32) -> i32 {
    let run = || -> Result<i32, Vb6Error> {
        let sum = vb::add_i32(intellect, vb::mul_i32(willpower, 3)?)?;
        vb::clng(vb::fix(f64::from(sum) / 4.0) + f64::from(modifiers))
    };
    run().unwrap_or(0)
}

/// Port of modMMudFunc CalcMaxHP: maximum HP; 0 on overflow.
///
/// `random` is the class HP roll total (see MME's comment: the range times level for the
/// maximum, the range alone for the minimum).
#[must_use]
pub fn calc_max_hp(random: i32, level: i32, health: i32, min_hp_per_level: i32) -> i32 {
    let run = || -> Result<i32, Vb6Error> {
        let per_level = vb::mul_i32(level, min_hp_per_level)?;
        let skew = vb::mul_i32(vb::sub_i32(health, 50)?, level)?;
        let total = (vb::fix(f64::from(health) / 2.0) + f64::from(per_level))
            + vb::fix(f64::from(skew) / 16.0)
            + f64::from(random);
        vb::clng(total)
    };
    run().unwrap_or(0)
}

/// Port of modMMudFunc CalcMaxMana: `magic_level * level * 2 + 6`; 0 on overflow.
#[must_use]
pub fn calc_max_mana(level: i32, magic_level: i32) -> i32 {
    let run = || -> Result<i32, Vb6Error> {
        let a = vb::mul_i32(magic_level, level)?;
        vb::add_i32(vb::mul_i32(a, 2)?, 6)
    };
    run().unwrap_or(0)
}

/// Port of modMMudFunc CalcSpellCasting: spellcasting skill; 0 for no magic or on overflow.
#[must_use]
pub fn calc_spell_casting(
    level: i32,
    intellect: i32,
    willpower: i32,
    charm: i32,
    magic_level: i32,
    magic_type: i32,
) -> i32 {
    let run = || -> Result<i32, Vb6Error> {
        let rest = |base: f64| -> Result<i32, Vb6Error> {
            let a = vb::mul_i32(level, 2)?;
            let b = vb::mul_i32(magic_level, 5)?;
            vb::clng(base + f64::from(a) + f64::from(b))
        };
        match magic_type {
            1 => rest(vb::fix(
                f64::from(vb::add_i32(vb::mul_i32(intellect, 3)?, willpower)?) / 6.0,
            )),
            2 => rest(vb::fix(
                f64::from(vb::add_i32(vb::mul_i32(willpower, 3)?, intellect)?) / 6.0,
            )),
            3 => rest(vb::fix(f64::from(vb::add_i32(willpower, intellect)?) / 3.0)),
            4 => rest(vb::fix(
                f64::from(vb::add_i32(vb::mul_i32(charm, 3)?, willpower)?) / 6.0,
            )),
            5 => {
                let a = vb::add_i32(500, vb::mul_i32(level, 2)?)?;
                vb::add_i32(a, vb::mul_i32(magic_level, 5)?)
            }
            _ => Ok(0),
        }
    };
    run().unwrap_or(0)
}

/// Port of modMMudFunc GetEncumPercents: the tooltip of encumbrance thresholds for a maximum
/// carrying capacity; empty for 0.
///
/// The 10% to 90% lines come from a Double loop (`For x = 0.1 To 0.9 Step 0.1`) whose counter
/// drifts (the 80% line uses 0.7999999999999999), which changes some rounded values; that is
/// reproduced.
#[must_use]
pub fn get_encum_percents(total_encum: i32) -> String {
    if total_encum == 0 {
        return String::new();
    }
    let t = f64::from(total_encum);
    let line = |name: &str, factor: f64| -> String {
        format!(
            "{name} @ {}/{total_encum}",
            vb::cstr(vb::fix(t * factor) + 1.0)
        )
    };
    let mut s = format!(
        "{}\r\n{}\r\n{}",
        line("Light", 0.17),
        line("Medium", 0.34),
        line("Heavy", 0.67)
    );
    s.push_str("\r\n");
    let mut x = 0.1_f64;
    while x <= 0.9 {
        s.push_str("\r\n");
        s.push_str(&vb::cstr(x * 100.0));
        s.push_str("% @ ");
        s.push_str(&vb::cstr(vb::fix(t * x) + 1.0));
        x += 0.1;
    }
    s
}

/// Port of modMMudFunc CalcPicklocks.
///
/// # Errors
/// `Vb6(6)` where the Long arithmetic overflows (MME has no handler there).
pub fn calc_picklocks(
    profile: Profile,
    level: i32,
    agility: i32,
    intellect: i32,
) -> Result<i32, RulesError> {
    match profile {
        Profile::Stock | Profile::Quicksilver(_) => {
            let base = if level <= 15 {
                vb::mul_i32(level, 2).map_err(vb6)?
            } else {
                let half = vb::fix(f64::from(vb::sub_i32(level, 15).map_err(vb6)?) / 2.0);
                vb::clng((half + 15.0) * 2.0).map_err(vb6)?
            };
            let stats = vb::add_i32(agility, intellect).map_err(vb6)?;
            let num = vb::mul_i32(
                vb::add_i32(vb::mul_i32(base, 5).map_err(vb6)?, stats).map_err(vb6)?,
                2,
            )
            .map_err(vb6)?;
            vb::clng(vb::fix(f64::from(num) / 7.0)).map_err(vb6)
        }
    }
}

/// Port of modMMudFunc CalcCPLevel: creation points earned by reaching `level`.
///
/// # Errors
/// `Vb6(6)` if the total overflows a Long.
pub fn calc_cp_level(level: i32) -> Result<i32, RulesError> {
    let last = vb::sub_i32(level, 1).map_err(vb6)?;
    let mut total: i32 = 0;
    for i in 1..=last {
        let step = 5 * (i / 10) + 10;
        total = vb::add_i32(total, step).map_err(vb6)?;
    }
    Ok(total)
}

/// Port of modMMudFunc CalcEncumbrancePercent: current over maximum encumbrance as a whole
/// percent. The inputs are clamped (max to 1..=999999, current to 0..=max) and MME divides in
/// Currency, which rounds to four decimals before the truncation.
///
/// # Errors
/// Only a Currency overflow, which the clamps rule out.
pub fn calc_encumbrance_percent(current: Currency, max: Currency) -> Result<i16, RulesError> {
    let mut max = max;
    let mut current = current;
    if max < cur_long(1) {
        max = cur_long(1);
    }
    if max > cur_long(999_999) {
        max = cur_long(999_999);
    }
    if current < cur_long(0) {
        current = cur_long(0);
    }
    if current > max {
        current = max;
    }
    let pct = fix_cur(
        current
            .checked_mul(cur_long(100))
            .and_then(|p| p.checked_div(max))
            .map_err(vb6)?,
    );
    vb::cint(pct.to_f64()).map_err(vb6)
}

/// Port of modMMudFunc AdjustSpeedForSlowness: weapon speed times 1.5, truncated.
///
/// # Errors
/// `Vb6(6)` on Currency overflow.
pub fn adjust_speed_for_slowness(speed: Currency) -> Result<Currency, RulesError> {
    let q = speed
        .checked_mul(cur_long(3))
        .and_then(|p| p.checked_div(cur_long(2)))
        .map_err(vb6)?;
    Ok(fix_cur(q))
}

/// Inputs of [`calculate_stealth`].
#[derive(Debug, Clone, Copy)]
pub struct StealthInput {
    pub level: i16,
    pub agility: i16,
    pub intellect: i16,
    pub charm: i16,
    pub class_stealth: bool,
    pub race_stealth: bool,
    /// Flat stealth from gear and spells (`nPlusStealth`).
    pub plus_stealth: i16,
    /// Encumbrance percent; only the GreaterMUD formula reads it.
    pub encum_pct: i16,
}

/// The stealth value and the breakdown MME builds in `sReturnText`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stealth {
    pub value: i16,
    pub text: String,
}

/// Port of modSyntaxsFunc AutoAppend with a CRLF glue.
fn append_line(text: &mut String, add: &str) {
    if text.is_empty() {
        text.push_str(add);
    } else if !add.trim_matches(' ').is_empty() {
        text.push_str("\r\n");
        text.push_str(add);
    }
}

/// Port of modMMudFunc CalculateStealth. `text_in` is the incoming `sReturnText` (usually
/// empty); the breakdown lines are appended to it. A character with neither race nor class
/// stealth gets 0 and the text unchanged. Integer overflow ends the calculation early and
/// returns the partial value and text, as MME's handler does.
#[must_use]
pub fn calculate_stealth(profile: Profile, input: &StealthInput, text_in: &str) -> Stealth {
    let mut value: i16 = 0;
    let mut text = text_in.to_string();
    if input.race_stealth || input.class_stealth {
        let _ = stealth_steps(profile, input, &mut value, &mut text);
    }
    Stealth { value, text }
}

fn stealth_steps(
    profile: Profile,
    i: &StealthInput,
    st: &mut i16,
    text: &mut String,
) -> Result<(), Vb6Error> {
    if i.level <= 15 {
        *st = vb::mul_i16(i.level, 2)?;
    } else {
        let doubled = vb::mul_i16(vb::sub_i16(i.level, 15)?, 2)?;
        *st = vb::cint(vb::fix(f64::from(doubled) / 2.0) + 30.0)?;
    }
    *st = vb::add_i16(*st, 20)?;
    append_line(text, &format!("Level ({})", st));

    let agl = vb::fix(f64::from(i.agility) / 4.0);
    let int = vb::fix(f64::from(i.intellect) / 8.0);
    let chm = vb::fix(f64::from(i.charm) / 6.0);
    match profile {
        Profile::Stock | Profile::Quicksilver(_) => {
            *st = vb::cint(f64::from(*st) + agl)?;
            *st = vb::cint(f64::from(*st) + int)?;
            *st = vb::cint(f64::from(*st) + chm)?;
        }
    }
    append_line(text, &format!("Agility ({})", vb::cstr(agl)));
    append_line(text, &format!("Intellect ({})", vb::cstr(int)));
    append_line(text, &format!("Charm ({})", vb::cstr(chm)));

    if i.race_stealth && i.class_stealth {
        *st = vb::add_i16(*st, 10)?;
        append_line(text, "Race+Class (10)");
    } else if i.race_stealth {
        *st = vb::sub_i16(*st, 15)?;
        append_line(text, "Race Only (-15)");
    }
    *st = vb::add_i16(*st, i.plus_stealth)?;
    Ok(())
}

/// Port of modMMudFunc RoundUpToNearest5.
///
/// # Errors
/// `Vb6(6)` for values within 4 of the Long maximum (the rounded result does not fit).
pub fn round_up_to_nearest5(n: i32) -> Result<i32, RulesError> {
    if n % 5 == 0 {
        Ok(n)
    } else if n > 0 {
        vb::mul_i32(vb::add_i32(n / 5, 1).map_err(vb6)?, 5).map_err(vb6)
    } else {
        Ok(n / 5 * 5)
    }
}

/// Port of modMain Get_Enc_Ratio: `(val1 + val2) / enc` as a percent with four decimals, or
/// the plain total when `enc` is below 1; 0 for a non-positive total.
///
/// # Errors
/// `Vb6(6)` if the total overflows a Long.
pub fn get_enc_ratio(enc: i32, val1: i32, val2: i32) -> Result<Currency, RulesError> {
    let total = vb::add_i32(val1, val2).map_err(vb6)?;
    if total <= 0 {
        return Ok(Currency::from_scaled(0));
    }
    if enc < 1 {
        return Ok(cur_long(total));
    }
    let ratio = vb::round(f64::from(total) / f64::from(enc), 4).map_err(vb6)?;
    Currency::from_f64(ratio * 100.0).map_err(vb6)
}
