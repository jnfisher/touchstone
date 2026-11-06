//! Swings, energy and weapon damage (FAM-07). Card P3-09.
//!
//! [`calculate_attack`] is the weapon/martial-arts damage calculator; the energy, Quick and Deadly,
//! backstab and true-average helpers sit beside it. Stock rules only: `Profile` has no other
//! variant, so MME's GreaterMUD branches are simply absent.
//!
//! MME reads a lot of state from globals; here it is plain data. [`CharacterProfile`] is the part
//! of `tCharacterProfile` the calculator reads, [`EquippedGlobals`] is the `nGlobalChar*` weapon
//! arrays, and [`AttackEnv`] carries the tables (`tabItems`, `tabClasses`, `tabRaces`) and the few
//! form values and globals involved. Other cards' procedures are called from their modules
//! (`stats`, `accuracy`, `abilities`, `exp`); only the spell text of a weapon's cast ability comes
//! in through [`CastText`], because the spell text builder needs the spell tables.
//!
//! Where MME's procedure has `On Error ... Resume out` the port returns what MME would have
//! returned: for `CalculateAttack` that is the all-zero result, because the function value is only
//! assigned at the very end. A run-time error that escapes (a procedure with no handler) is
//! returned as `Err`.

use std::sync::OnceLock;

use regex::{Regex, RegexBuilder};
use touchstone_model::{Class, Item, Race};
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Vb6Error};

use crate::Profile;
use crate::abilities;
use crate::accuracy::{self, AttackDefenseInput, BackstabAccuracyInput};
use crate::exp;
use crate::stats::{self, StealthInput};

type Res<T> = Result<T, Vb6Error>;

// ---------------------------------------------------------------------------------------------
// Small VB6 helpers

fn cur(v: i64) -> Currency {
    Currency::from_scaled(v * 10_000)
}

/// `Fix` of a Currency: truncation toward zero, still a Currency.
fn fix_cur(c: Currency) -> Currency {
    Currency::from_scaled(c.scaled() / 10_000 * 10_000)
}

fn div_half_even(n: i64, d: i64) -> i64 {
    let q = n / d;
    let twice = (n % d).abs() * 2;
    if twice > d.abs() || (twice == d.abs() && q % 2 != 0) {
        q + n.signum() * d.signum()
    } else {
        q
    }
}

/// `Round(c, digits)` on a Currency (digits 0 to 3): exact decimal rounding, half to even.
fn round_cur(c: Currency, digits: u32) -> Currency {
    let q = 10i64.pow(4 - digits);
    Currency::from_scaled(div_half_even(c.scaled(), q) * q)
}

/// A Currency assigned to a Long: `CLng` rounding.
fn cur_to_i32(c: Currency) -> Res<i32> {
    i32::try_from(div_half_even(c.scaled(), 10_000)).map_err(|_| Vb6Error::Overflow)
}

/// A Currency assigned to an Integer.
fn cur_to_i16(c: Currency) -> Res<i16> {
    i16::try_from(div_half_even(c.scaled(), 10_000)).map_err(|_| Vb6Error::Overflow)
}

/// A value stored into an Integer variable.
fn n16(v: i64) -> Res<i16> {
    i16::try_from(v).map_err(|_| Vb6Error::Overflow)
}

/// A value stored into a Long variable.
fn n32(v: i64) -> Res<i32> {
    i32::try_from(v).map_err(|_| Vb6Error::Overflow)
}

/// `Currency op Double` is a Double; storing it back into a Currency rounds.
fn to_cur(x: f64) -> Res<Currency> {
    Currency::from_f64(x)
}

/// `&` on a Currency: up to four decimals, trailing zeros dropped.
fn cur_str(c: Currency) -> String {
    let s = c.scaled();
    let a = s.unsigned_abs();
    let (whole, frac) = (a / 10_000, a % 10_000);
    let sign = if s < 0 { "-" } else { "" };
    if frac == 0 {
        format!("{sign}{whole}")
    } else {
        let f = format!("{frac:04}");
        format!("{sign}{whole}.{}", f.trim_end_matches('0'))
    }
}

/// Port of modSyntaxsFunc AutoAppend.
fn auto_append(base: &str, add: &str, glue: &str) -> String {
    if base.is_empty() {
        add.to_string()
    } else if add.trim_matches(' ').is_empty() {
        base.to_string()
    } else {
        format!("{base}{glue}{add}")
    }
}

fn contains_ci(hay: &str, needle: &str) -> bool {
    hay.to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

/// `Split(s, delim, , vbTextCompare)`.
fn split_ci(s: &str, delim: &str) -> Vec<String> {
    let (ls, ld) = (s.to_ascii_lowercase(), delim.to_ascii_lowercase());
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = ls[at..].find(&ld) {
        out.push(s[at..at + i].to_string());
        at += i + ld.len();
    }
    out.push(s[at..].to_string());
    out
}

// ---------------------------------------------------------------------------------------------
// Energy and Quick and Deadly

/// Arguments of [`calc_energy_used`]. `Default` is MME's "argument left out": no strength, no
/// encumbrance step (`-1`), no speed adjustment.
#[derive(Debug, Clone, Copy)]
pub struct EnergyInput {
    pub combat: Currency,
    pub level: Currency,
    pub attack_speed: Currency,
    pub agility: Currency,
    pub strength: Currency,
    /// Encumbrance percent; negative skips the encumbrance step.
    pub encum: i16,
    pub item_strength: Currency,
    pub speed_adj: Currency,
    pub is_backstab: bool,
}

impl Default for EnergyInput {
    fn default() -> Self {
        let zero = Currency::from_scaled(0);
        Self {
            combat: zero,
            level: zero,
            attack_speed: zero,
            agility: zero,
            strength: zero,
            encum: -1,
            item_strength: zero,
            speed_adj: zero,
            is_backstab: false,
        }
    }
}

/// Port of modMMudFunc CalcEnergyUsed: energy spent per swing. Arithmetic is Currency, except the
/// encumbrance step, where the `Fix(encum / 2) + 75` factor is a Double.
///
/// # Errors
/// `DivisionByZero` when the level/combat/agility term truncates to 0 (MME has no handler there).
pub fn calc_energy_used(i: &EnergyInput) -> Res<Currency> {
    let a = i
        .level
        .checked_mul(i.combat.checked_add(cur(2))?)?
        .checked_add(cur(45))?
        .checked_mul(i.agility.checked_add(cur(150))?)?;
    let divisor = fix_cur(a.checked_div(cur(6))?);
    let mut e = fix_cur(
        i.attack_speed
            .checked_mul(cur(1000))?
            .checked_div(divisor)?,
    );

    if i.strength > cur(0) && i.strength < i.item_strength {
        let over = i.item_strength.checked_sub(i.strength)?;
        let scaled = over.checked_mul(cur(3))?.checked_add(cur(200))?;
        e = fix_cur(scaled.checked_mul(e)?.checked_div(cur(200))?);
    }
    if i.speed_adj > cur(0) && i.speed_adj != cur(100) && !i.is_backstab {
        e = fix_cur(e.checked_mul(i.speed_adj)?.checked_div(cur(100))?);
    }
    if i.encum >= 0 {
        let factor = vb::fix(f64::from(i.encum) / 2.0) + 75.0;
        e = to_cur(vb::fix(e.to_f64() * factor / 100.0))?;
    }
    Ok(e)
}

/// Port of modMMudFunc CalcEnergyUsedWithEncum: [`calc_energy_used`] without its own encumbrance
/// step, then the encumbrance factor in Currency.
///
/// # Errors
/// As [`calc_energy_used`], plus Currency overflow.
pub fn calc_energy_used_with_encum(
    combat: Currency,
    level: Currency,
    speed: Currency,
    agility: Currency,
    strength: Currency,
    encum: Currency,
    item_strength: Currency,
) -> Res<Currency> {
    let e = calc_energy_used(&EnergyInput {
        combat,
        level,
        attack_speed: speed,
        agility,
        strength,
        item_strength,
        ..EnergyInput::default()
    })?;
    adjust_energy_used_with_encum(e, encum)
}

/// Port of modMMudFunc AdjustEnergyUsedWithSpeed.
///
/// # Errors
/// Currency overflow.
pub fn adjust_energy_used_with_speed(energy: Currency, speed: Currency) -> Res<Currency> {
    Ok(fix_cur(energy.checked_mul(speed)?.checked_div(cur(100))?))
}

/// Port of modMMudFunc AdjustEnergyUsedWithEncum.
///
/// # Errors
/// Currency overflow.
pub fn adjust_energy_used_with_encum(energy: Currency, encum: Currency) -> Res<Currency> {
    let factor = fix_cur(encum.checked_div(cur(2))?).checked_add(cur(75))?;
    Ok(fix_cur(energy.checked_mul(factor)?.checked_div(cur(100))?))
}

/// Port of modMMudFunc CalcQuickAndDeadlyBonus (stock): the crit-chance bonus for a weapon that
/// costs little energy. 0 at 200 energy or more, or above 66% encumbrance.
#[must_use]
pub fn calc_quick_and_deadly_bonus(agility: Currency, energy: Currency, encum: i16) -> Currency {
    if energy >= cur(200) || encum > 66 {
        return cur(0);
    }
    // Every step is Currency; none can overflow for inputs of this size, and MME resumes at the
    // end on an error, so a failure would return what was computed so far.
    let mut bonus = cur(0);
    let _ = (|| -> Res<()> {
        let from_agi = fix_cur(agility.checked_sub(cur(50))?.checked_div(cur(10))?);
        bonus = cur(200).checked_sub(energy)?.checked_add(from_agi)?;
        if bonus > cur(20) {
            bonus = cur(20);
        }
        if encum >= 33 {
            bonus = fix_cur(bonus.checked_div(cur(2))?);
        }
        Ok(())
    })();
    bonus
}

/// The form values `GetQuickAndDeadlyBonus` reads (the global filter, class box and character
/// panel), as the text MME reads them from.
#[derive(Debug, Clone, Copy)]
pub struct QndContext<'a> {
    /// `chkGlobalFilter.Value`.
    pub global_filter: i32,
    /// `cmbGlobalClass(0).ItemData(ListIndex)`, `None` when nothing is selected.
    pub class_number: Option<i32>,
    /// `txtCharStats(0).Tag` (strength).
    pub strength_tag: &'a str,
    /// `txtCharStats(3).Tag` (agility).
    pub agility_tag: &'a str,
    /// `lblInvenCharStat(0).Caption` (current encumbrance).
    pub encum_current_caption: &'a str,
    /// `lblInvenCharStat(1).Caption` (maximum encumbrance).
    pub encum_max_caption: &'a str,
    /// `txtGlobalLevel(0).Text`.
    pub level_text: &'a str,
}

fn find_item(items: &[Item], number: i32) -> Option<&Item> {
    items.iter().find(|i| i.number == number)
}

/// Port of modMMudFunc GetQuickAndDeadlyBonus: the bonus for item `item_number` with the
/// character on the main window, 0 where MME exits early or hits an error.
#[must_use]
pub fn get_quick_and_deadly_bonus(
    items: &[Item],
    classes: &[Class],
    ctx: &QndContext,
    item_number: i32,
) -> i16 {
    if item_number == 0 || items.is_empty() || ctx.global_filter == 0 {
        return 0;
    }
    let Some(class_number) = ctx.class_number.filter(|c| *c >= 1) else {
        return 0;
    };
    let Some(item) = find_item(items, item_number) else {
        return 0;
    };
    if f64::from(item.str_req) > vb::val(ctx.strength_tag) {
        return 0;
    }
    let steps = || -> Res<i16> {
        let class = classes.iter().find(|c| c.number == class_number);
        let combat = n16(i64::from(stats::get_class_combat(class)))?;
        let encum_pct = stats::calc_encumbrance_percent(
            to_cur(vb::val(ctx.encum_current_caption))?,
            to_cur(vb::val(ctx.encum_max_caption))?,
        )
        .map_err(|_| Vb6Error::Overflow)?;
        let encum = cur(i64::from(encum_pct));
        let energy = calc_energy_used_with_encum(
            cur(i64::from(combat)),
            to_cur(vb::val(ctx.level_text))?,
            cur(i64::from(item.speed)),
            to_cur(vb::val(ctx.agility_tag))?,
            to_cur(vb::val(ctx.strength_tag))?,
            encum,
            cur(i64::from(item.str_req)),
        )?;
        let agi = to_cur(vb::val(ctx.agility_tag))?;
        cur_to_i16(calc_quick_and_deadly_bonus(agi, energy, cur_to_i16(encum)?))
    };
    steps().unwrap_or(0)
}

// ---------------------------------------------------------------------------------------------
// Backstab damage and true average

/// Port of modMMudFunc CalcBSDamage (stock): backstab damage for a level, stealth, weapon damage
/// and bonus. Without class stealth the damage is cut to 75%. An error leaves the value MME had
/// reached when it hit it.
#[must_use]
pub fn calc_bs_damage(
    level: i16,
    stealth: i16,
    damage: i16,
    bs_dmg_mod: i16,
    class_stealth: bool,
) -> i32 {
    let mut r: i32 = 0;
    let _ = (|| -> Res<()> {
        let base = f64::from(vb::mul_i16(level, 2)?)
            + vb::fix(f64::from(stealth) / 10.0)
            + f64::from(vb::mul_i16(damage, 2)?)
            + f64::from(bs_dmg_mod);
        r = vb::clng(base)?;
        if !class_stealth {
            r = vb::clng(vb::fix(f64::from(vb::mul_i32(r, 75)?) / 100.0))?;
        }
        let lv = vb::add_i16(level, 100)?;
        r = vb::clng(vb::fix(f64::from(vb::mul_i32(i32::from(lv), r)?) / 100.0))?;
        Ok(())
    })();
    r
}

/// MME's `MAX_SWINGS`.
pub const MAX_SWINGS: f64 = 5.0;

/// Port of modMMudFunc CalcTrueAverage (stock): expected damage per round from swings, hit,
/// crit and extra-hit chances (in percent) and amounts. -1 for no swings; 0 where MME's handler
/// fires.
#[must_use]
pub fn calc_true_average(
    swings: f64,
    hit_pct: f64,
    hit_amount: i32,
    crit_pct: f64,
    crit_amount: i32,
    extra_pct: f64,
    extra_amount: i32,
) -> f64 {
    if swings <= 0.0 {
        return -1.0;
    }
    let swings = swings.min(MAX_SWINGS);
    let (hit_p, crit_p, extra_p) = (hit_pct / 100.0, crit_pct / 100.0, extra_pct / 100.0);
    let total = ((hit_p * f64::from(hit_amount)) + (crit_p * f64::from(crit_amount)))
        + ((hit_p + crit_p) * extra_p * f64::from(extra_amount));
    vb::round(total * swings, 2).unwrap_or(0.0)
}

// ---------------------------------------------------------------------------------------------
// Inputs and outputs of CalculateAttack

/// The part of MME's `tCharacterProfile` that `CalculateAttack` reads. MME's Integer fields are
/// `i16`, Long fields `i32`. The caller builds this from the character (card P3-11).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CharacterProfile {
    /// A loaded character (as opposed to a made-up profile): gear stats are then subtracted and
    /// added when the weapon differs from the equipped one, and Quick and Deadly applies.
    pub is_loaded_character: bool,
    /// 0 means "the best a character can be": level, strength, agility and stealth are maxed.
    pub level: i32,
    pub class: i32,
    pub race: i32,
    pub combat: i16,
    pub strength: i16,
    pub agility: i16,
    pub intellect: i16,
    pub charm: i16,
    /// Crit chance percent, including the equipped weapon's and Quick and Deadly.
    pub crit: i16,
    pub plus_max_damage: i16,
    pub plus_min_damage: i16,
    pub plus_bs_accuracy: i16,
    pub plus_bs_min_damage: i16,
    pub plus_bs_max_damage: i16,
    /// Punch, kick, jumpkick (MME indexes 1 to 3).
    pub ma_plus_skill: [i16; 3],
    pub ma_plus_accuracy: [i16; 3],
    pub ma_plus_damage: [i16; 3],
    pub stealth: i16,
    pub class_stealth: bool,
    pub race_stealth: bool,
    pub accuracy: f64,
    pub encum_pct: i16,
    pub encum_current: i32,
    pub encum_max: i32,
    pub spell_dmg_bonus: i16,
}

/// One equipped hand's contribution: the `nGlobalChar*(0 or 1)` arrays (0 = weapon, 1 = off-hand).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HandGlobals {
    pub number: i32,
    pub encum: i32,
    pub accy: i32,
    pub crit: i32,
    pub strength: i32,
    pub agility: i32,
    pub max_dmg: i32,
    pub bs_accy: i32,
    pub bs_min_dmg: i32,
    pub bs_max_dmg: i32,
    pub punch_skill: i32,
    pub punch_accy: i32,
    pub punch_dmg: i32,
    pub kick_skill: i32,
    pub kick_accy: i32,
    pub kick_dmg: i32,
    pub jk_skill: i32,
    pub jk_accy: i32,
    pub jk_dmg: i32,
    pub stealth: i32,
}

/// The weapon-related `nGlobalChar*` globals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EquippedGlobals {
    /// `nGlobalCharQnDbonus`: the Quick and Deadly bonus already inside the profile's crit.
    pub qnd_bonus: i32,
    pub accy_abils: i32,
    pub accy_other: i32,
    pub accy_items: i32,
    pub hands: [HandGlobals; 2],
}

/// The spell text of a weapon's cast ability; both come from other cards' procedures.
pub trait CastText {
    /// modMMudDatabase GetSpellName(number, bHideRecordNumbers).
    fn spell_name(&self, number: i32) -> String;
    /// modMMudDatabase PullSpellEQ(True, 0, number, , , , True, , , , , spell_bonus): the effect
    /// text with the level calculated and the percent column on.
    fn spell_eq(&self, number: i32, spell_bonus: i16) -> String;
}

/// Tables, globals and form values `CalculateAttack` reads besides the profile.
pub struct AttackEnv<'a> {
    pub items: &'a [Item],
    pub classes: &'a [Class],
    pub races: &'a [Race],
    pub equipped: &'a EquippedGlobals,
    pub cast_text: &'a dyn CastText,
    /// `frmMain.lblDatVer.Caption`, which `CalcEncum` looks at.
    pub dat_ver_caption: &'a str,
    /// `gAvgLevelMaxAllStats`, for `GetMaxLevel`.
    pub avg_level_max_all_stats: f64,
    /// `frmMain.lblInvenCharStat(10).Tag`: the accuracy used when the profile has none.
    pub accuracy_tag: &'a str,
}

/// The arguments of `CalculateAttack` after the profile.
#[derive(Debug, Clone, Copy)]
pub struct AttackRequest {
    /// `eAttackTypeMUD`: 1 punch, 2 kick, 3 jumpkick, 4 surprise (backstab), 5 normal, 6 bash,
    /// 7 smash. 0 or less means normal.
    pub attack_type: i32,
    /// Item number; 0 for no weapon; -2 to -5 for the 10/20/40/80 damage proxy weapons.
    pub weapon_number: i32,
    pub abil68_slow: bool,
    /// Speed adjustment percent (100 = none).
    pub speed_adj: i16,
    pub vs_ac: i32,
    pub vs_dr: i32,
    pub vs_dodge: i32,
    pub force_calc: bool,
    /// Manual damage; negative means not given.
    pub specify_damage: f64,
    /// Manual accuracy; negative means not given.
    pub specify_accy: f64,
    pub bs_defense: i32,
}

impl Default for AttackRequest {
    fn default() -> Self {
        Self {
            attack_type: 0,
            weapon_number: 0,
            abil68_slow: false,
            speed_adj: 100,
            vs_ac: 0,
            vs_dr: 0,
            vs_dodge: 0,
            force_calc: false,
            specify_damage: -1.0,
            specify_accy: -1.0,
            bs_defense: 0,
        }
    }
}

/// MME's `tAttackDamage`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AttackDamage {
    pub min_dmg: i32,
    pub max_dmg: i32,
    pub hit_chance: i16,
    pub dodge_chance: i16,
    pub crit_chance: i16,
    pub qnd_bonus: i16,
    pub accy: i16,
    pub avg_hit: i32,
    pub avg_crit: i32,
    pub max_crit: i32,
    pub avg_extra_hit: i32,
    pub avg_extra_swing: i32,
    pub swings: f64,
    pub round_physical: i32,
    pub round_total: i32,
    pub first_round_damage: i32,
    pub min_round_damage: i32,
    pub attack_desc: String,
    pub attack_detail: String,
    pub attack_speed: i16,
}

// ---------------------------------------------------------------------------------------------
// Damage of spells cast by a weapon

fn damage_term() -> String {
    let text = r"[^\[\{\}\]]+";
    let name = r"(Damage(?:\(-MR\))?|DrainLife)";
    // ASCII digits, as VBScript's `\d`; the regex crate's `\d` is every Unicode digit.
    let num = r"(-?[0-9]+)";
    format!("{text}, {name} {num} to {num}")
}

/// The pattern for the three layouts a weapon's spell text can have: alternatives (`{a} OR {b}`),
/// a chain of `EndCast` spells, or one spell.
///
/// These are the three patterns `CalculateAttack` hands to `RegExpFindv2` (VBScript.RegExp, global,
/// ignore-case). They use only literals, classes, groups, `|`, `?`, `+`, `*` and `\d`, so the
/// `regex` crate's leftmost-first matching picks the same match and captures as VBScript's
/// backtracking; there are no backreferences or lookaround. MME's `{` is a literal there (no
/// digits follow it) and is written `\{` here because the crate rejects a bare one.
fn cast_regex(casts: &str) -> &'static Regex {
    static LAYOUTS: OnceLock<[Regex; 3]> = OnceLock::new();
    let layouts = LAYOUTS.get_or_init(|| {
        [
            compile_cast(&alternatives_pattern()),
            compile_cast(&end_cast_pattern()),
            compile_cast(&single_pattern()),
        ]
    });
    if contains_ci(casts, "} or {") {
        &layouts[0]
    } else if contains_ci(casts, ", EndCast [") {
        &layouts[1]
    } else {
        &layouts[2]
    }
}

fn alternatives_pattern() -> String {
    let term = damage_term();
    let tail_of_term = r"[^\]\}]*";
    let one = r"(?:\{TERMTAIL(?:\} OR )?)?"
        .replace("TERM", &term)
        .replace("TAIL", tail_of_term);
    r"\[[^\[\{\}\]]+\[\{TERMTAIL(?:\} OR )MORE\}\], ([0-9]+)%\]"
        .replace("TERM", &term)
        .replace("TAIL", tail_of_term)
        .replace("MORE", &one.repeat(5))
}

fn end_cast_pattern() -> String {
    let term = damage_term();
    let first = format!(r"(?:{term}(?:\]|, EndCast ))");
    let more = format!(r"(?:\[{term}(?:\]|, EndCast ))?").repeat(5);
    format!(r"\[{first}{more}\]+, ([0-9]+)%\]")
}

fn single_pattern() -> String {
    let term = damage_term();
    format!(r"\[(?:{term}[^\]]*), ([0-9]+)%\]")
}

fn compile_cast(pattern: &str) -> Regex {
    // `RegExpFindv2(sCasts, pattern, MatchCase:=False)`. The crate folds case with Unicode rules
    // where VBScript folds ASCII only; the spell text is ASCII, so the two agree on it.
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .build()
        .expect("the cast patterns are fixed and valid")
}

/// One `RegExpFindv2` match: the whole text and each group (`None` when it did not take part).
struct CastMatch {
    text: String,
    groups: Vec<Option<String>>,
}

fn find_all_casts(casts: &str) -> Vec<CastMatch> {
    cast_regex(casts)
        .captures_iter(casts)
        .map(|c| CastMatch {
            text: c[0].to_string(),
            groups: c
                .iter()
                .skip(1)
                .map(|g| g.map(|m| m.as_str().to_string()))
                .collect(),
        })
        .collect()
}

/// The extra damage a weapon's cast spells add per hit and per swing, from the `[spell, ...]`
/// text: only the last matching group's average survives, as in MME.
fn cast_extra_damage(casts: &str, spell_bonus: i16, swings: f64) -> Res<(Currency, Currency)> {
    let matches = find_all_casts(casts);
    let mut extra_avg_hit = cur(0);
    let mut extra_avg_swing = cur(0);
    if matches.is_empty() {
        return Ok((extra_avg_hit, extra_avg_swing));
    }
    let mut ability = String::new();
    for m in &matches {
        let subs: Vec<&str> = m
            .groups
            .iter()
            .flatten()
            .map(String::as_str)
            .filter(|g| !g.is_empty())
            .collect();
        if subs.len() < 4 {
            continue;
        }
        let parts = if contains_ci(&m.text, "} or {") {
            split_ci(&m.text, "} or {")
        } else if contains_ci(&m.text, ", EndCast ") {
            split_ci(&m.text, ", EndCast ")
        } else {
            vec![m.text.clone()]
        };
        let mut extra_tmp = cur(0);
        let mut count: i64 = 0;
        let mut dur_damage = cur(0);
        let mut dur_count: i64 = 0;
        let last = subs.len() - 1;
        let mut x = 0;
        while x < last {
            let part = x / 3;
            if x == part * 3 {
                ability = subs[x].to_string();
            } else {
                if part < parts.len()
                    && contains_ci(&parts[part], " for ")
                    && contains_ci(&parts[part], "rounds")
                {
                    dur_damage = to_cur(dur_damage.to_f64() + vb::abs(vb::val(subs[x])))?;
                    dur_count += 1;
                    count += 1;
                    x += 1;
                    if last > x {
                        dur_damage = to_cur(dur_damage.to_f64() + vb::abs(vb::val(subs[x])))?;
                        dur_count += 1;
                        count += 1;
                    }
                    x += 1;
                    continue;
                }
                let amount = vb::abs(vb::val(subs[x]));
                if spell_bonus > 0 && (ability == "Damage" || ability == "Damage(-MR)") {
                    let scaled = vb::add_i16(100, spell_bonus)?;
                    let part_amount = vb::idiv(amount * f64::from(scaled), 100.0)?;
                    extra_tmp = extra_tmp.checked_add(cur(i64::from(part_amount)))?;
                } else {
                    extra_tmp = to_cur(extra_tmp.to_f64() + amount)?;
                }
                count += 1;
            }
            x += 1;
        }

        if count > 0 {
            extra_avg_hit = round_cur(extra_tmp.checked_div(cur(count))?, 2);
        }
        if parts.len() > 1 && count > 1 && !contains_ci(&m.text, "} OR {") {
            extra_avg_hit = to_cur(extra_avg_hit.to_f64() * (count as f64 / 2.0))?;
        }
        let extra_pct = vb::round(vb::val(subs[last]) / 100.0, 2)?;
        if dur_count > 0 {
            let per_tick = dur_damage.checked_div(cur(dur_count))?.to_f64();
            let once = vb::round(vb::div(per_tick, swings)?, 0)?;
            extra_avg_hit = to_cur(extra_avg_hit.to_f64() + once)?;
        }
        extra_avg_swing = to_cur(vb::round(extra_avg_hit.to_f64() * extra_pct, 0)?)?;
    }
    if matches.len() > 1 {
        extra_avg_hit = round_cur(extra_avg_hit.checked_div(cur(matches.len() as i64))?, 0);
    }
    extra_avg_swing = round_cur(extra_avg_swing, 0);
    Ok((extra_avg_hit, extra_avg_swing))
}

// ---------------------------------------------------------------------------------------------
// CalculateAttack

/// Port of modMMudFunc CalculateAttack: damage per hit, per crit, per round and the hit chance for
/// one attack type with one weapon (or none) against a defender of the given AC, DR and dodge.
///
/// `casts` is MME's ByRef `sCasts`: the weapon's cast-spell text. Empty on entry, it is filled in
/// for a weapon with cast abilities; a caller may also pass text of its own. Any run-time error
/// inside the calculation gives the all-zero result, as MME's handler does.
#[must_use]
pub fn calculate_attack(
    profile: Profile,
    env: &AttackEnv,
    ch: &CharacterProfile,
    req: &AttackRequest,
    casts: &mut String,
) -> AttackDamage {
    let (Profile::Stock | Profile::Quicksilver(_)) = profile;
    attack_steps(env, ch, req, casts).unwrap_or_default()
}

/// Where the weapon's numbers come from.
enum Source<'a> {
    Item(&'a Item),
    Proxy { damage: i32, speed: i16 },
    Bare,
}

#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
fn attack_steps(
    env: &AttackEnv,
    ch: &CharacterProfile,
    req: &AttackRequest,
    casts: &mut String,
) -> Res<AttackDamage> {
    let g = env.equipped;
    let weapon = req.weapon_number;
    let mut attack_type = req.attack_type;

    let mut desc = String::new();
    let mut dmg_min: i32;
    let mut dmg_max: i32;
    let mut swings: f64;
    let mut attack_speed: i16 = 0;
    let mut crit_chance: i16 = 0;
    let mut qnd_bonus = cur(0);
    let mut attack_accuracy = cur(0);
    let mut plus_min: i16 = 0;
    let mut plus_max: i16 = 0;
    let n_percent: f64 = 0.0;
    // Index 1..=3 as in MME; slot 0 is never used.
    let mut ma_skill = [0i16; 4];
    let mut ma_accy = [0i32; 4];
    let mut ma_dmg = [0i32; 4];
    let mut str_req: i16 = 0;

    if req.specify_damage >= 0.0 {
        // Manual damage: no weapon, no gear, one swing.
        desc = "Manual".to_string();
        dmg_min = vb::clng(req.specify_damage)?;
        dmg_max = dmg_min;
        if dmg_min < 0 {
            dmg_min = 0;
        }
        if dmg_max > 9_999_999 {
            dmg_max = 9_999_999;
        }
        if dmg_min > dmg_max {
            dmg_min = dmg_max;
        }
        swings = 1.0;
    } else {
        if attack_type <= 0 {
            attack_type = 5;
        }
        if weapon == 0 && attack_type > 5 {
            return Ok(AttackDamage::default());
        }

        let level: i16;
        let combat: Currency;
        let mut strength: i16;
        let mut agility: i16;
        let mut stealth: i16;
        let mut plus_bs_accy: i16;
        let mut plus_bs_min: i16;
        let mut plus_bs_max: i16;
        let mut encum_pct: i16;
        let mut encum_current: i32;
        let mut encum_max: i32;
        let class_stealth: bool;

        if ch.level == 0 {
            level = n16(i64::from(
                exp::get_max_level(env.avg_level_max_all_stats).map_err(|_| Vb6Error::Overflow)?,
            ))?;
            combat = cur(3);
            strength = 255;
            agility = 255;
            stealth = 255;
            if (1..=3).contains(&attack_type) {
                ma_skill[attack_type as usize] = 1;
            }
            attack_accuracy = cur(999);
            plus_bs_accy = 999;
            plus_bs_min = 0;
            plus_bs_max = 0;
            encum_pct = 0;
            encum_current = 0;
            encum_max = 0;
            class_stealth = true;
        } else {
            level = n16(i64::from(ch.level))?;
            let mut combat_v = cur(i64::from(ch.combat));
            strength = ch.strength;
            agility = ch.agility;
            plus_min = ch.plus_min_damage;
            plus_max = ch.plus_max_damage;
            stealth = ch.stealth;
            crit_chance = ch.crit;
            if ch.is_loaded_character {
                crit_chance = n16(i64::from(crit_chance) - i64::from(g.qnd_bonus))?;
            }
            for k in 0..3 {
                ma_skill[k + 1] = ch.ma_plus_skill[k];
                ma_accy[k + 1] = i32::from(ch.ma_plus_accuracy[k]);
                ma_dmg[k + 1] = i32::from(ch.ma_plus_damage[k]);
            }
            attack_accuracy = to_cur(ch.accuracy)?;
            plus_bs_accy = ch.plus_bs_accuracy;
            plus_bs_min = ch.plus_bs_min_damage;
            plus_bs_max = ch.plus_bs_max_damage;
            encum_pct = ch.encum_pct;
            encum_current = ch.encum_current;
            encum_max = ch.encum_max;

            if combat_v == cur(0) && ch.class > 0 {
                let class = env.classes.iter().find(|c| c.number == ch.class);
                combat_v = cur(i64::from(stats::get_class_combat(class)));
            }
            combat = combat_v;

            let mut cls_st = ch.class_stealth;
            let mut race_st = ch.race_stealth;
            if !cls_st && ch.class > 0 {
                let class = env.classes.iter().find(|c| c.number == ch.class);
                cls_st = stats::get_class_stealth(class);
            }
            if !race_st && ch.race > 0 {
                let race = env.races.iter().find(|r| r.number == ch.race);
                race_st = stats::get_race_stealth(race);
            }
            class_stealth = cls_st;
            let stealth_input = |class_st: bool, race_st: bool, plus: i16| StealthInput {
                level,
                agility,
                intellect: ch.intellect,
                charm: ch.charm,
                class_stealth: class_st,
                race_stealth: race_st,
                plus_stealth: plus,
                encum_pct: 0,
            };
            if !class_stealth && req.force_calc {
                stealth = stats::calculate_stealth(
                    Profile::Stock,
                    &stealth_input(false, true, stealth),
                    "",
                )
                .value;
            } else if stealth == 0 && (class_stealth || race_st) {
                stealth = stats::calculate_stealth(
                    Profile::Stock,
                    &stealth_input(class_stealth, race_st, 0),
                    "",
                )
                .value;
            }

            if req.force_calc && (1..=3).contains(&attack_type) {
                let k = attack_type as usize;
                if ma_skill[k] < 1 {
                    ma_skill[k] = 1;
                }
            }
        }
        if encum_max < 48 {
            encum_max = 48;
        }
        let start_strength = i32::from(strength);

        let source = if weapon == 0 {
            Source::Bare
        } else if (-5..=-2).contains(&weapon) {
            Source::Proxy {
                damage: 10 * (1 << (-weapon - 2)),
                speed: (1000 * -weapon) as i16,
            }
        } else {
            let Some(item) = find_item(env.items, weapon) else {
                return Ok(AttackDamage::default());
            };
            if ch.is_loaded_character && weapon > 0 && weapon != g.hands[0].number {
                let mut recalc_encum = false;
                let hand = &g.hands[0];
                if hand.number > 0 {
                    // The equipped weapon's contribution comes off before this one's goes on.
                    let sub = |stat: i16, by: i32| n16(i64::from(stat) - i64::from(by));
                    attack_accuracy = attack_accuracy.checked_sub(cur(i64::from(hand.accy)))?;
                    crit_chance = sub(crit_chance, hand.crit)?;
                    plus_max = sub(plus_max, hand.max_dmg)?;
                    plus_bs_accy = sub(plus_bs_accy, hand.bs_accy)?;
                    plus_bs_min = sub(plus_bs_min, hand.bs_min_dmg)?;
                    plus_bs_max = sub(plus_bs_max, hand.bs_max_dmg)?;
                    stealth = sub(stealth, hand.stealth)?;
                    strength = sub(strength, hand.strength)?;
                    agility = sub(agility, hand.agility)?;

                    if (hand.strength != 0 || hand.encum != item.encum)
                        && encum_current > 0
                        && encum_max > 0
                    {
                        encum_current = n32(i64::from(encum_current) - i64::from(hand.encum))?;
                        recalc_encum = true;
                    }
                    subtract_martial_arts(
                        attack_type,
                        hand,
                        &mut ma_skill,
                        &mut ma_accy,
                        &mut ma_dmg,
                    )?;

                    if item.weapon_type == 1 || item.weapon_type == 3 {
                        let off = &g.hands[1];
                        if off.number > 0 {
                            attack_accuracy =
                                attack_accuracy.checked_sub(cur(i64::from(off.accy)))?;
                            crit_chance = sub(crit_chance, off.crit)?;
                            plus_max = sub(plus_max, off.max_dmg)?;
                            plus_bs_accy = sub(plus_bs_accy, off.bs_accy)?;
                            plus_bs_min = sub(plus_bs_min, off.bs_min_dmg)?;
                            plus_bs_max = sub(plus_bs_max, off.bs_max_dmg)?;
                            stealth = sub(stealth, off.stealth)?;
                            strength = sub(strength, off.strength)?;
                            agility = sub(agility, off.agility)?;
                            if recalc_encum {
                                encum_current =
                                    n32(i64::from(encum_current) - i64::from(off.encum))?;
                            }
                            subtract_martial_arts(
                                attack_type,
                                off,
                                &mut ma_skill,
                                &mut ma_accy,
                                &mut ma_dmg,
                            )?;
                        }
                    }
                } else {
                    recalc_encum = true;
                }

                if attack_type > 3 {
                    attack_accuracy = attack_accuracy.checked_add(cur(i64::from(item.accy)))?;
                }
                for slot in &item.abilities {
                    if slot.ability > 0 && slot.value != 0 {
                        let ability = n16(i64::from(slot.ability))?;
                        n16(i64::from(slot.value))?;
                        let equip = abilities::get_ability_stat_slot(ability);
                        let v = i64::from(slot.value);
                        if equip > 0 {
                            match equip {
                                7 => crit_chance = n16(i64::from(crit_chance) + v)?,
                                11 => plus_max = n16(i64::from(plus_max) + v)?,
                                13 => plus_bs_accy = n16(i64::from(plus_bs_accy) + v)?,
                                14 => plus_bs_min = n16(i64::from(plus_bs_min) + v)?,
                                15 => plus_bs_max = n16(i64::from(plus_bs_max) + v)?,
                                37 if attack_type == 1 => {
                                    ma_skill[1] = n16(i64::from(ma_skill[1]) + v)?
                                }
                                40 if attack_type == 1 => {
                                    ma_accy[1] = n32(i64::from(ma_accy[1]) + v)?
                                }
                                34 if attack_type == 1 => {
                                    ma_dmg[1] = n32(i64::from(ma_dmg[1]) + v)?
                                }
                                38 if attack_type == 2 => {
                                    ma_skill[2] = n16(i64::from(ma_skill[2]) + v)?
                                }
                                41 if attack_type == 2 => {
                                    ma_accy[2] = n32(i64::from(ma_accy[2]) + v)?
                                }
                                35 if attack_type == 2 => {
                                    ma_dmg[2] = n32(i64::from(ma_dmg[2]) + v)?
                                }
                                39 if attack_type == 3 => {
                                    ma_skill[3] = n16(i64::from(ma_skill[3]) + v)?
                                }
                                42 if attack_type == 3 => {
                                    ma_accy[3] = n32(i64::from(ma_accy[3]) + v)?
                                }
                                36 if attack_type == 3 => {
                                    ma_dmg[3] = n32(i64::from(ma_dmg[3]) + v)?
                                }
                                19 => stealth = n16(i64::from(stealth) + v)?,
                                101 => strength = n16(i64::from(strength) + v)?,
                                102 => agility = n16(i64::from(agility) + v)?,
                                _ => {}
                            }
                        }
                    }
                }

                if recalc_encum {
                    encum_current = n32(i64::from(encum_current) + i64::from(item.encum))?;
                    if start_strength != i32::from(strength) {
                        let diff = start_strength - i32::from(strength);
                        let enc_diff =
                            stats::calc_encum(n16(i64::from(diff.abs()))?, 0, env.dat_ver_caption);
                        if diff > 0 {
                            encum_max = n32(i64::from(encum_max) - i64::from(enc_diff))?;
                        } else {
                            encum_max = n32(i64::from(encum_max) + i64::from(enc_diff))?;
                        }
                    }
                    encum_pct = stats::calc_encumbrance_percent(
                        cur(i64::from(encum_current)),
                        cur(i64::from(encum_max)),
                    )
                    .map_err(|_| Vb6Error::Overflow)?;
                }
            }
            if attack_type <= 3 {
                Source::Bare
            } else {
                Source::Item(item)
            }
        };

        match source {
            Source::Item(item) => {
                desc.clone_from(&item.name);
                str_req = n16(i64::from(item.str_req))?;
                dmg_min = item.min;
                dmg_max = item.max;
                attack_speed = n16(i64::from(item.speed))?;
                if req.abil68_slow {
                    attack_speed =
                        vb::cint(vb::fix(f64::from(vb::mul_i16(attack_speed, 3)?) / 2.0))?;
                }
            }
            Source::Proxy { damage, speed } => {
                str_req = 0;
                dmg_min = damage;
                dmg_max = damage;
                attack_speed = speed;
            }
            Source::Bare => {
                if attack_type <= 3 && ma_skill[attack_type as usize] <= 0 {
                    return Ok(AttackDamage::default());
                }
                desc = "Punch".to_string();
                attack_speed = match (attack_type, req.abil68_slow) {
                    (1, false) => 1150,
                    (1, true) => 1750,
                    (2, false) => 1400,
                    (2, true) => 2000,
                    (3, false) => 1900,
                    (3, true) => 2650,
                    (4 | 5, false) => 1200,
                    (4 | 5, true) => 1800,
                    _ => return Ok(AttackDamage::default()),
                };
                if attack_type <= 3 {
                    let skill = i64::from(ma_skill[attack_type as usize]);
                    let lv = i64::from(level.min(20));
                    let mut lo = skill * lv;
                    if lo < 0 {
                        lo += 7;
                    }
                    dmg_min = n32(vb::fix(lo as f64 / 8.0) as i64 + 2)?;
                    dmg_max = match attack_type {
                        1 => {
                            let mut hi = skill * (lv + 3);
                            if hi < 0 {
                                hi += 3;
                            }
                            n32(vb::fix(hi as f64 / 4.0) as i64 + 6)?
                        }
                        2 => n32(vb::fix((skill * lv) as f64 / 6.0) as i64 + 7)?,
                        _ => n32(vb::fix((skill * lv) as f64 / 6.0) as i64 + 8)?,
                    };
                } else {
                    dmg_min = 1;
                    dmg_max = 4;
                }
            }
        }

        // Energy per swing, Quick and Deadly, crit diminishing returns, swings.
        let mut energy: i32 = if attack_type == 4 || attack_type == 7 {
            1000
        } else {
            cur_to_i32(calc_energy_used(&EnergyInput {
                combat,
                level: cur(i64::from(level)),
                attack_speed: cur(i64::from(attack_speed)),
                agility: cur(i64::from(agility)),
                strength: cur(i64::from(strength)),
                encum: encum_pct,
                item_strength: cur(i64::from(str_req)),
                speed_adj: cur(i64::from(req.speed_adj)),
                is_backstab: false,
            })?)?
        };

        if ch.is_loaded_character
            && strength >= str_req
            && attack_type != 4
            && attack_type != 6
            && attack_type != 7
        {
            qnd_bonus = calc_quick_and_deadly_bonus(
                cur(i64::from(agility)),
                cur(i64::from(energy)),
                encum_pct,
            );
            crit_chance = cur_to_i16(cur(i64::from(crit_chance)).checked_add(qnd_bonus)?)?;
        }
        if crit_chance > 40 {
            let over = vb::fix(f64::from(crit_chance - 40) / 3.0);
            crit_chance = vb::cint(40.0 + over)?;
            if crit_chance > 99 {
                crit_chance = 99;
            }
        }

        if attack_type == 6 {
            energy = n32(i64::from(energy) * 2)?;
        }
        if energy < 1 {
            energy = 1;
        }
        swings = vb::round(1000.0 / f64::from(energy), 4)?;
        if swings > MAX_SWINGS {
            swings = MAX_SWINGS;
        }

        dmg_min = n32(i64::from(dmg_min) + i64::from(plus_min))?;
        dmg_max = n32(i64::from(dmg_max) + i64::from(plus_max))?;
        if dmg_min > dmg_max {
            dmg_min = dmg_max;
        }
        if dmg_min < 0 {
            dmg_min = 0;
        }
        if dmg_max < 0 {
            dmg_max = 0;
        }

        let mut pre_roll = 1.0_f64;
        let mut multiplier = 1.0_f64;
        if attack_type <= 3 {
            let k = attack_type as usize;
            attack_accuracy = attack_accuracy.checked_add(cur(i64::from(ma_accy[k])))?;
            dmg_min = n32(i64::from(dmg_min) + i64::from(ma_dmg[k]))?;
            dmg_max = n32(i64::from(dmg_max) + i64::from(ma_dmg[k]))?;
            if attack_type == 2 {
                pre_roll = 1.33;
                desc = "Kick".to_string();
            } else if attack_type == 3 {
                pre_roll = 1.66;
                desc = "JumpKick".to_string();
            }
        } else if attack_type == 4 {
            desc = if desc == "Punch" {
                "surprise punch".to_string()
            } else {
                format!("backstab with {desc}")
            };
            crit_chance = 0;
            qnd_bonus = cur(0);
            let t = vb::fix(f64::from(stealth) / 10.0) + f64::from(vb::mul_i16(level, 2)?);
            let t = vb::clng(t)?;
            dmg_min =
                n32(i64::from(vb::mul_i32(dmg_min, 2)?) + i64::from(t) + i64::from(plus_bs_min))?;
            dmg_max =
                n32(i64::from(vb::mul_i32(dmg_max, 2)?) + i64::from(t) + i64::from(plus_bs_max))?;
            if !class_stealth {
                dmg_min = vb::clng(vb::fix(f64::from(vb::mul_i32(dmg_min, 75)?) / 100.0))?;
                dmg_max = vb::clng(vb::fix(f64::from(vb::mul_i32(dmg_max, 75)?) / 100.0))?;
            }
            let lv100 = i32::from(vb::add_i16(level, 100)?);
            dmg_min = vb::clng(vb::fix(f64::from(vb::mul_i32(lv100, dmg_min)?) / 100.0))?;
            dmg_max = vb::clng(vb::fix(f64::from(vb::mul_i32(lv100, dmg_max)?) / 100.0))?;

            let plus_normal = if ch.is_loaded_character {
                let sum = vb::add_i32(g.accy_abils, g.accy_other)?;
                // IIf evaluates both arms, so the items term is added even though stock ignores it.
                vb::add_i32(sum, g.accy_items)?;
                sum
            } else {
                0
            };
            let accy = accuracy::calculate_backstab_accuracy(
                Profile::Stock,
                &BackstabAccuracyInput {
                    stealth,
                    agility,
                    plus_bs_accuracy: plus_bs_accy,
                    class_stealth,
                    plus_normal_accuracy: n16(i64::from(plus_normal))?,
                },
            );
            attack_accuracy = cur(i64::from(accy));
        } else if attack_type == 6 {
            crit_chance = 0;
            qnd_bonus = cur(0);
            pre_roll = 1.1;
            multiplier = 3.0;
            attack_accuracy = attack_accuracy.checked_sub(cur(15))?;
            desc = format!("bash with {desc}");
        } else if attack_type == 7 {
            crit_chance = 0;
            qnd_bonus = cur(0);
            pre_roll = 1.2;
            multiplier = 5.0;
            attack_accuracy = attack_accuracy.checked_sub(cur(25))?;
            desc = format!("smash with {desc}");
        }
        return finish(
            env,
            ch,
            req,
            casts,
            Tail {
                attack_type,
                desc,
                dmg_min,
                dmg_max,
                swings,
                attack_speed,
                crit_chance,
                qnd_bonus,
                attack_accuracy,
                pre_roll,
                multiplier,
                n_percent,
            },
        );
    }

    finish(
        env,
        ch,
        req,
        casts,
        Tail {
            attack_type,
            desc,
            dmg_min,
            dmg_max,
            swings,
            attack_speed,
            crit_chance,
            qnd_bonus,
            attack_accuracy,
            pre_roll: 1.0,
            multiplier: 1.0,
            n_percent,
        },
    )
}

fn subtract_martial_arts(
    attack_type: i32,
    hand: &HandGlobals,
    skill: &mut [i16; 4],
    accy: &mut [i32; 4],
    dmg: &mut [i32; 4],
) -> Res<()> {
    let (s, a, d) = match attack_type {
        1 => (hand.punch_skill, hand.punch_accy, hand.punch_dmg),
        2 => (hand.kick_skill, hand.kick_accy, hand.kick_dmg),
        3 => (hand.jk_skill, hand.jk_accy, hand.jk_dmg),
        _ => return Ok(()),
    };
    let k = attack_type as usize;
    skill[k] = n16(i64::from(skill[k]) - i64::from(s))?;
    accy[k] = n32(i64::from(accy[k]) - i64::from(a))?;
    dmg[k] = n32(i64::from(dmg[k]) - i64::from(d))?;
    Ok(())
}

/// State handed from the set-up half of `CalculateAttack` to the damage half (`calc_damage:`).
struct Tail {
    attack_type: i32,
    desc: String,
    dmg_min: i32,
    dmg_max: i32,
    swings: f64,
    attack_speed: i16,
    crit_chance: i16,
    qnd_bonus: Currency,
    attack_accuracy: Currency,
    pre_roll: f64,
    multiplier: f64,
    n_percent: f64,
}

#[allow(clippy::too_many_lines)]
fn finish(
    env: &AttackEnv,
    ch: &CharacterProfile,
    req: &AttackRequest,
    casts: &mut String,
    t: Tail,
) -> Res<AttackDamage> {
    let Tail {
        attack_type,
        desc,
        mut dmg_min,
        mut dmg_max,
        swings,
        attack_speed,
        crit_chance,
        qnd_bonus,
        mut attack_accuracy,
        pre_roll,
        multiplier,
        mut n_percent,
    } = t;
    let vs_dr = req.vs_dr;

    if attack_accuracy == cur(0) && req.specify_accy < 0.0 && ch.is_loaded_character {
        attack_accuracy = to_cur(vb::val(env.accuracy_tag))?;
    } else if req.specify_accy >= 0.0 {
        attack_accuracy = to_cur(req.specify_accy)?;
    }
    if attack_accuracy < cur(8) {
        attack_accuracy = cur(8);
    }

    let mut hit_chance = cur(100);
    let mut dodge_chance: i16 = 0;
    if req.vs_ac > 0 || req.vs_dodge > 0 {
        let defense = accuracy::calculate_attack_defense(
            Profile::Stock,
            &AttackDefenseInput {
                accy: cur_to_i32(attack_accuracy)?,
                ac: req.vs_ac,
                dodge: req.vs_dodge,
                bs_defense: req.bs_defense,
                backstab: attack_type == 4,
                ..AttackDefenseInput::default()
            },
        );
        hit_chance = cur(i64::from(defense.hit_chance));
        if defense.dodge_chance > 0 {
            dodge_chance = n16(i64::from(defense.dodge_chance))?;
            let keep = 1.0 - (f64::from(defense.dodge_chance) / 100.0);
            hit_chance = to_cur(hit_chance.to_f64() * keep)?;
        }
    }

    if req.vs_dodge < 0 && req.vs_ac > 0 {
        // Negative dodge: a chance the defender's AC is ignored for a 99% hit.
        n_percent = f64::from(req.vs_dodge + 100) / 100.0;
        let n_percent2 = 1.0 - n_percent;
        hit_chance = to_cur((99.0 * n_percent) + (hit_chance.to_f64() * n_percent2))?;
        if hit_chance < cur(8) {
            hit_chance = cur(8);
        }
    }
    hit_chance = hit_chance.checked_div(cur(100))?;

    if pre_roll > 1.0 {
        dmg_min = vb::clng(vb::fix(f64::from(dmg_min) * pre_roll))?;
        dmg_max = vb::clng(vb::fix(f64::from(dmg_max) * pre_roll))?;
    }

    let (mut avg_crit, mut min_crit, mut max_crit) = (0i32, 0i32, 0i32);
    if crit_chance > 0 {
        min_crit = vb::mul_i32(dmg_max, 2)?;
        max_crit = vb::mul_i32(dmg_max, 4)?;
        if min_crit > max_crit {
            max_crit = min_crit;
        }
        let mid = vb::round(f64::from(vb::add_i32(min_crit, max_crit)?) / 2.0, 0)?;
        avg_crit = vb::clng(mid - f64::from(vs_dr))?;
        min_crit = vb::sub_i32(min_crit, vs_dr)?;
        max_crit = vb::sub_i32(max_crit, vs_dr)?;
        avg_crit = avg_crit.max(0);
        min_crit = min_crit.max(0);
        max_crit = max_crit.max(0);
    }

    dmg_min = vb::clng(vb::fix(
        f64::from(vb::sub_i32(dmg_min, vs_dr)?) * multiplier,
    ))?;
    dmg_max = vb::clng(vb::fix(
        f64::from(vb::sub_i32(dmg_max, vs_dr)?) * multiplier,
    ))?;
    if dmg_min < 0 {
        dmg_min = 0;
    }
    if dmg_max < 0 {
        dmg_max = 0;
    }
    let avg_hit = round_cur(
        to_cur(vb::round(
            f64::from(vb::add_i32(dmg_min, dmg_max)?) / 2.0,
            0,
        )?)?,
        0,
    );

    // The weapon's cast-spell text and the damage it adds.
    let weapon = req.weapon_number;
    if casts.is_empty()
        && weapon > 0
        && attack_type > 3
        && let Some(item) = find_item(env.items, weapon)
    {
        for slot in &item.abilities {
            match slot.ability {
                43 => {
                    let name = env.cast_text.spell_name(slot.value);
                    let eq = env.cast_text.spell_eq(slot.value, ch.spell_dmg_bonus);
                    *casts = auto_append(casts, &format!("[{name}, {eq}"), "|");
                    if n_percent == 0.0 {
                        casts.push(']');
                    } else {
                        casts.push_str(&format!(", {}%]", vb::cstr(n_percent)));
                    }
                }
                114 => n_percent = f64::from(slot.value),
                _ => {}
            }
        }
    }
    let (extra_avg_hit, extra_avg_swing) = if !casts.is_empty() && weapon > 0 && attack_type > 3 {
        cast_extra_damage(casts, ch.spell_dmg_bonus, swings)?
    } else {
        (cur(0), cur(0))
    };

    let mut r = AttackDamage {
        min_dmg: dmg_min,
        max_dmg: dmg_max,
        avg_hit: cur_to_i32(avg_hit)?,
        avg_crit,
        max_crit,
        avg_extra_hit: cur_to_i32(extra_avg_hit)?,
        avg_extra_swing: cur_to_i32(extra_avg_swing)?,
        crit_chance,
        qnd_bonus: cur_to_i16(qnd_bonus)?,
        swings,
        accy: cur_to_i16(attack_accuracy)?,
        attack_speed,
        dodge_chance,
        ..AttackDamage::default()
    };

    n_percent = f64::from(crit_chance) / 100.0;
    let hit = hit_chance.to_f64();
    let per_swing = ((1.0 - n_percent) * avg_hit.to_f64()) + (n_percent * f64::from(avg_crit));
    r.round_physical = vb::clng(vb::round(per_swing * swings * hit, 0)?)?;
    r.round_total = vb::clng(
        f64::from(r.round_physical) + vb::round(extra_avg_swing.to_f64() * swings * hit, 0)?,
    )?;
    let whole = vb::fix(swings);
    r.first_round_damage = vb::clng(
        vb::round(per_swing * whole * hit, 0)?
            + vb::round(extra_avg_swing.to_f64() * whole * hit, 0)?,
    )?;
    let min_swing = ((1.0 - n_percent) * f64::from(dmg_min)) + (n_percent * f64::from(min_crit));
    r.min_round_damage = vb::clng(vb::round(min_swing * whole * hit, 0)?)?;
    r.hit_chance = cur_to_i16(round_cur(hit_chance.checked_mul(cur(100))?, 0))?;

    let mut detail = String::new();
    if swings > 0.0 && avg_hit.checked_add(cur(i64::from(avg_crit)))? > cur(0) {
        if attack_type == 4 {
            detail = format!("Backstab: {} avg @ {}% hit ", r.round_total, r.hit_chance);
            if cur(i64::from(dmg_min)) < avg_hit
                || cur(i64::from(dmg_max)) > avg_hit
                || extra_avg_hit != extra_avg_swing
            {
                let mut lo = dmg_min;
                let mut hi = dmg_max;
                if extra_avg_hit > cur(0) {
                    if extra_avg_hit == extra_avg_swing {
                        lo = cur_to_i32(cur(i64::from(lo)).checked_add(extra_avg_hit)?)?;
                    }
                    hi = cur_to_i32(cur(i64::from(hi)).checked_add(extra_avg_hit)?)?;
                }
                let both = vb::add_i32(dmg_min, dmg_max)?;
                let mid = cur(i64::from(both))
                    .checked_add(extra_avg_swing)?
                    .checked_add(extra_avg_swing)?;
                let mid = vb::idiv(mid.to_f64(), 2.0)?;
                detail.push_str(&format!("(Min/Avg/Max: {lo}/{mid}/{hi})"));
            }
        } else {
            let shown = vb::fix(swings * 10.0) / 10.0;
            detail = format!("Swings: {}, Avg Hit: {}", vb::cstr(shown), cur_str(avg_hit));
            if avg_crit > 0 {
                detail = auto_append(
                    &detail,
                    &format!("Avg/Max Crit: {avg_crit}/{max_crit}"),
                    ", ",
                );
                if crit_chance > 0 {
                    detail.push_str(&format!(" ({crit_chance}%)"));
                }
            }
            if r.hit_chance > 0 {
                detail = auto_append(&detail, &format!("Hit: {}%", r.hit_chance), ", ");
            }
        }
    }
    r.attack_desc = desc;
    r.attack_detail = detail;
    Ok(r)
}
