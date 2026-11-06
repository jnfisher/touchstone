//! The four small calculator windows (FAM-27). Card P3-43.
//!
//! MME keeps the arithmetic of the swing, backstab, coin and exp calculators inside their forms and
//! reads the form's controls. Here each calculator is a pure function: the input struct holds what
//! the controls held (text boxes as the text, combo boxes as the selected item's number, option
//! buttons as booleans) and the output struct holds what the form shows afterwards, including the
//! text MME builds. A control the procedure did not write is an empty string (or `None`).
//!
//! A run-time error that MME's handler (or, for `CalcSwings`, the caller) leaves the procedure on
//! stops at the same point: the output is what had been written by then, and `error` is VB's error
//! number. Stock rules only; `bGreaterMUD` is false throughout.

use touchstone_model::{Class, Item, Race};
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Vb6Error};

use crate::Profile;
use crate::abilities;
use crate::attack::{self, AttackEnv, AttackRequest, CharacterProfile, EnergyInput, HandGlobals};
use crate::stats;

type Res<T> = Result<T, Vb6Error>;

fn cur(v: i64) -> Currency {
    Currency::from_scaled(v * 10_000)
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

fn rules_err(e: crate::RulesError) -> Vb6Error {
    match e {
        crate::RulesError::Vb6(11) => Vb6Error::DivisionByZero,
        _ => Vb6Error::Overflow,
    }
}

fn find_item(items: &[Item], number: i32) -> Option<&Item> {
    items.iter().find(|i| i.number == number)
}

/// `Integer` from a Long value (assignment overflows past 16 bits).
fn to_i16(x: i64) -> Res<i16> {
    i16::try_from(x).map_err(|_| Vb6Error::Overflow)
}

// ---------------------------------------------------------------------------------------------
// Swing calculator

/// The controls of frmSwingCalc that `CalcSwings` reads.
#[derive(Debug, Clone)]
pub struct SwingInput<'a> {
    /// `cmbWeapon.ItemData(ListIndex)`; `None` when nothing is selected.
    pub weapon: Option<i32>,
    /// `cmbCombat.ItemData(ListIndex)` (the combat level; a selection is required).
    pub combat: i32,
    /// `chkSlowness.Value = 1`.
    pub slowness: bool,
    /// `chkBashing.Value = 1`.
    pub bashing: bool,
    pub encum_text: &'a str,
    pub max_encum_text: &'a str,
    pub level_text: &'a str,
    pub agility_text: &'a str,
    pub strength_text: &'a str,
    /// `txtSpeed.Text`, read when the custom option is on.
    pub speed_text: &'a str,
    /// `optSpeed(0..3).Value`: speed, normal, slow, custom.
    pub speed_options: [bool; 4],
    /// `frmMain.chkGlobalFilter.Value = 1`.
    pub global_filter: bool,
    /// What `PopulateCharacterProfile` produces for the loaded character (the character glue's
    /// job); read only when `global_filter` is on, otherwise MME starts from an empty profile.
    pub profile: CharacterProfile,
}

/// What frmSwingCalc shows after `CalcSwings`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SwingOutput {
    /// `lblQND.Caption`.
    pub qnd_text: String,
    /// `lblEnergy.Caption`.
    pub energy_text: String,
    /// `lblEncum.Caption` and `.Tag`.
    pub encum_text: String,
    pub encum_tag: String,
    /// `lblRawSwing.Caption` and `.Tag`.
    pub raw_swing_text: String,
    pub raw_swing_tag: String,
    /// `txtTrueAVG(6).Text`.
    pub true_avg_6: String,
    /// `lblEU(0..9).Caption`.
    pub eu: [String; 10],
    /// `txtSwing(0..9).Text`.
    pub swings: [String; 10],
    /// `lblSwingDamage(0).Caption`: the round total of `CalculateAttack`.
    pub damage_text: String,
    /// `lblSwingDamage(1).Caption`.
    pub damage_caption: String,
    /// `txtSpeed.Text` after the call (a negative custom speed is reset to 0).
    pub speed_text: String,
    /// Energy per swing (Currency, whole) and encumbrance percent, as typed values.
    pub energy: Option<Currency>,
    pub encum_pct: Option<i16>,
    /// VB error number that left the procedure, if any.
    pub error: Option<u32>,
}

/// Port of frmSwingCalc CalcSwings: energy per swing, swings in each of ten rounds and the average
/// damage of a round for one weapon. `env` carries `tabItems` and what `CalculateAttack` reads.
/// MME has no error handler here, so an error (division by zero from a negative agility, overflow
/// from huge numbers) is returned in `error` with the labels written so far.
#[must_use]
pub fn calc_swings(profile: Profile, input: &SwingInput, env: &AttackEnv) -> SwingOutput {
    let mut out = SwingOutput {
        speed_text: input.speed_text.to_string(),
        ..SwingOutput::default()
    };
    if let Err(e) = swing_steps(profile, input, env, &mut out) {
        out.error = Some(e.code());
    }
    out
}

#[allow(clippy::too_many_lines)]
fn swing_steps(
    profile: Profile,
    input: &SwingInput,
    env: &AttackEnv,
    out: &mut SwingOutput,
) -> Res<()> {
    if env.items.is_empty() {
        return Ok(());
    }
    let Some(weapon) = input.weapon else {
        return Ok(());
    };
    let Some(item) = find_item(env.items, weapon) else {
        return Ok(());
    };

    let mut weapon_speed = cur(i64::from(item.speed));
    if input.slowness {
        weapon_speed = stats::adjust_speed_for_slowness(weapon_speed).map_err(rules_err)?;
    }
    let encum_cur = vb::Currency::from_f64(vb::val(input.encum_text))?;
    let max_cur = vb::Currency::from_f64(vb::val(input.max_encum_text))?;
    let encum_pct = stats::calc_encumbrance_percent(encum_cur, max_cur).map_err(rules_err)?;

    let speed_adj: i16 = if input.speed_options[0] {
        85
    } else if input.speed_options[1] {
        0
    } else if input.speed_options[2] {
        125
    } else if input.speed_options[3] {
        let n = vb::cint(vb::val(input.speed_text))?;
        if n < 0 {
            out.speed_text = "0".into();
            0
        } else {
            n
        }
    } else {
        0
    };

    let agility = Currency::from_f64(vb::val(input.agility_text))?;
    let strength = Currency::from_f64(vb::val(input.strength_text))?;
    let level = Currency::from_f64(vb::val(input.level_text))?;
    let mut energy = attack::calc_energy_used(&EnergyInput {
        combat: cur(i64::from(input.combat)),
        level,
        attack_speed: weapon_speed,
        agility,
        strength,
        encum: encum_pct,
        item_strength: cur(i64::from(item.str_req)),
        speed_adj: cur(i64::from(speed_adj)),
        is_backstab: false,
    })?;
    if input.bashing {
        energy = energy.checked_mul(cur(2))?;
    }

    let mut qnd = cur(0);
    if vb::val(input.strength_text) >= f64::from(item.str_req) {
        qnd = attack::calc_quick_and_deadly_bonus(agility, energy, encum_pct);
    }
    out.qnd_text = if qnd > cur(0) {
        format!("QND Crits: {}", cur_str(qnd))
    } else {
        "QND Crits: None".into()
    };
    out.energy = Some(energy);
    out.encum_pct = Some(encum_pct);

    out.energy_text = format!("Energy per swing: {}", cur_str(energy));
    out.encum_text = format!("Encumbrance: {encum_pct}%");
    out.encum_tag = encum_pct.to_string();

    if energy == cur(0) {
        out.raw_swing_text = "Raw swing: 0".into();
        out.raw_swing_tag = "0".into();
        out.true_avg_6 = "0".into();
        for x in 0..10 {
            out.eu[x] = "0".into();
            out.swings[x] = "0".into();
        }
        return Ok(());
    }

    let raw = vb::round(cur(1000).checked_div(energy)?.to_f64(), 4)?;
    out.raw_swing_text = format!("Raw swing: {}", vb::cstr(raw));
    out.raw_swing_tag = vb::cstr(raw);
    out.true_avg_6 = vb::cstr(raw);

    let mut temp: i16 = 1000;
    if energy <= cur(0) {
        energy = cur(1);
    }
    for x in 0..10 {
        let mut i = vb::idiv(f64::from(temp), energy.to_f64())?;
        temp = to_i16(i64::from(vb::vmod(f64::from(temp), energy.to_f64())?) + 1000)?;
        if f64::from(i) > attack::MAX_SWINGS {
            i = attack::MAX_SWINGS as i32;
        }
        out.eu[x] = (i32::from(temp) - 1000).to_string();
        out.swings[x] = i.to_string();
    }

    // Starts from an empty profile unless the global filter pulls in the loaded character.
    let mut ch = if input.global_filter {
        input.profile.clone()
    } else {
        CharacterProfile::default()
    };
    ch.level = vb::clng(vb::val(input.level_text))?;
    ch.combat = to_i16(i64::from(input.combat))?;
    ch.agility = vb::cint(vb::val(input.agility_text))?;
    ch.strength = vb::cint(vb::val(input.strength_text))?;
    ch.encum_pct = encum_pct;

    let attack_type = if input.bashing { 6 } else { 5 };
    let adj = if speed_adj == 0 { 100 } else { speed_adj };
    let req = AttackRequest {
        attack_type,
        weapon_number: weapon,
        abil68_slow: input.slowness,
        speed_adj: adj,
        ..AttackRequest::default()
    };
    let mut casts = String::new();
    let dmg = attack::calculate_attack(profile, env, &ch, &req, &mut casts);
    out.damage_text = dmg.round_total.to_string();
    out.damage_caption = format!(
        "Avg damage vs 0 ac/dr\r\n{}",
        if input.global_filter {
            "(w/Char's Other Stats)"
        } else {
            "(global filter off)"
        }
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Backstab calculator

/// The controls of frmBSCalc that `CalcBS` reads, plus the equipped-weapon globals.
#[derive(Debug, Clone)]
pub struct BsInput<'a> {
    /// `cmbWeapon.ItemData(ListIndex)`; `None` when nothing is selected.
    pub weapon: Option<i32>,
    pub bs_min_text: &'a str,
    pub bs_max_text: &'a str,
    pub level_text: &'a str,
    pub stealth_text: &'a str,
    /// `txtMaxDMG`.
    pub max_dmg_text: &'a str,
    pub strength_text: &'a str,
    /// `chkClassStealth.Value = 1`.
    pub class_stealth: bool,
    /// `frmMain.chkGlobalFilter.Value = 1`.
    pub global_filter: bool,
    /// `nGlobalCharWeapon*(0)` and `(1)`: the equipped weapon and off-hand. Only `number`,
    /// `max_dmg`, `bs_min_dmg`, `bs_max_dmg` and `stealth` are read.
    pub equipped: [HandGlobals; 2],
}

/// What frmBSCalc shows after `CalcBS`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BsOutput {
    /// `lblDMG.Caption`: `"No BS"` or `"min - max (AVG: n)"`.
    pub damage_text: String,
    pub min_dmg: i32,
    pub max_dmg: i32,
    /// `lblMaxAdj`, `lblBSMinAdj`, `lblBSMaxAdj`, `lblStealthAdj` captions.
    pub max_adj: String,
    pub bs_min_adj: String,
    pub bs_max_adj: String,
    pub stealth_adj: String,
    /// The input boxes after MME clamps them to 0..1000 (it writes the clamp back).
    pub bs_min_text: String,
    pub bs_max_text: String,
    pub level_text: String,
    pub stealth_text: String,
    pub max_dmg_text: String,
    /// VB error number that MME's handler caught, if any; the state so far is returned.
    pub error: Option<u32>,
}

/// Clamps a box to 0..=1000 the way CalcBS does (writing the limit back as text).
fn clamp_box(text: &mut String) {
    if vb::val(text) > 1000.0 {
        *text = "1000".into();
    }
    if vb::val(text) < 0.0 {
        *text = "0".into();
    }
}

/// `val(caption) + n` stored back as a caption (a Double through `CStr`).
fn add_to_caption(caption: &mut String, n: i32) {
    *caption = vb::cstr(vb::val(caption) + f64::from(n));
}

/// `caption & IIf(n < 0, "+", "-") & Abs(n)`.
fn subtract_note(caption: &mut String, n: i32) -> Res<()> {
    let abs = n.checked_abs().ok_or(Vb6Error::Overflow)?;
    caption.push_str(if n < 0 { "+" } else { "-" });
    caption.push_str(&abs.to_string());
    Ok(())
}

/// Port of frmBSCalc CalcBS: backstab damage range for a weapon, with the gear the character
/// already wears swapped out when the global filter is on. Errors are caught by MME's handler,
/// which leaves the procedure; `error` holds the number and the output what had been written.
#[must_use]
pub fn calc_bs(profile: Profile, input: &BsInput, items: &[Item]) -> BsOutput {
    let mut out = BsOutput {
        bs_min_text: input.bs_min_text.to_string(),
        bs_max_text: input.bs_max_text.to_string(),
        level_text: input.level_text.to_string(),
        stealth_text: input.stealth_text.to_string(),
        max_dmg_text: input.max_dmg_text.to_string(),
        ..BsOutput::default()
    };
    if let Err(e) = bs_steps(profile, input, items, &mut out) {
        out.error = Some(e.code());
    }
    out
}

#[allow(clippy::too_many_lines)]
fn bs_steps(profile: Profile, input: &BsInput, items: &[Item], out: &mut BsOutput) -> Res<()> {
    let Some(weapon) = input.weapon else {
        return Ok(());
    };
    let Some(item) = find_item(items, weapon) else {
        return Ok(());
    };
    // Weapons without the backstab-accuracy ability (116) have no backstab.
    if !item.abilities.iter().any(|a| a.ability == 116) {
        out.damage_text = "No BS".into();
        return Ok(());
    }

    clamp_box(&mut out.bs_min_text);
    let mut plus_bs_min = vb::cint(vb::val(&out.bs_min_text))?;
    clamp_box(&mut out.bs_max_text);
    let mut plus_bs_max = vb::cint(vb::val(&out.bs_max_text))?;
    clamp_box(&mut out.level_text);
    clamp_box(&mut out.stealth_text);
    let mut stealth = vb::cint(vb::val(&out.stealth_text))?;
    clamp_box(&mut out.max_dmg_text);
    let mut plus_max = vb::cint(vb::val(&out.max_dmg_text))?;

    let mut min_str_bonus = vb::cint(vb::fix((vb::val(input.strength_text) - 100.0) / 10.0))?;
    min_str_bonus = vb::mul_i16(min_str_bonus, 2)?;
    if min_str_bonus < 0 {
        min_str_bonus = 0;
    }

    let [hand0, hand1] = input.equipped;
    if hand0.number != weapon || !input.global_filter {
        for slot in &item.abilities {
            if slot.ability > 0 && slot.value != 0 {
                let n = slot.value;
                let n16 = to_i16(i64::from(n))?;
                let stat = abilities::get_ability_stat_slot(to_i16(i64::from(slot.ability))?);
                if stat > 0 {
                    match stat {
                        11 => {
                            plus_max = vb::cint(f64::from(plus_max) + f64::from(n16))?;
                            add_to_caption(&mut out.max_adj, n);
                        }
                        14 => {
                            plus_bs_min = vb::cint(f64::from(plus_bs_min) + f64::from(n16))?;
                            add_to_caption(&mut out.bs_min_adj, n);
                        }
                        15 => {
                            plus_bs_max = vb::cint(f64::from(plus_bs_max) + f64::from(n16))?;
                            add_to_caption(&mut out.bs_max_adj, n);
                        }
                        19 => {
                            stealth = vb::cint(f64::from(stealth) + f64::from(n16))?;
                            add_to_caption(&mut out.stealth_adj, n);
                        }
                        _ => {}
                    }
                }
            }
        }
        for c in [
            &mut out.max_adj,
            &mut out.bs_min_adj,
            &mut out.bs_max_adj,
            &mut out.stealth_adj,
        ] {
            if vb::val(c) > 0.0 {
                *c = format!("+{c}");
            }
        }
    }

    if hand0.number != weapon && input.global_filter {
        let mut take = |h: &HandGlobals, out: &mut BsOutput| -> Res<()> {
            if h.max_dmg != 0 {
                plus_max = vb::cint(f64::from(plus_max) - f64::from(h.max_dmg))?;
                subtract_note(&mut out.max_adj, h.max_dmg)?;
            }
            if h.bs_min_dmg != 0 {
                plus_bs_min = vb::cint(f64::from(plus_bs_min) - f64::from(h.bs_min_dmg))?;
                subtract_note(&mut out.bs_min_adj, h.bs_min_dmg)?;
            }
            if h.bs_max_dmg != 0 {
                plus_bs_max = vb::cint(f64::from(plus_bs_max) - f64::from(h.bs_max_dmg))?;
                subtract_note(&mut out.bs_max_adj, h.bs_max_dmg)?;
            }
            if h.stealth != 0 {
                stealth = vb::cint(f64::from(stealth) - f64::from(h.stealth))?;
                subtract_note(&mut out.stealth_adj, h.stealth)?;
            }
            Ok(())
        };
        take(&hand0, out)?;
        // A two-handed weapon also frees the off-hand.
        if (item.weapon_type == 1 || item.weapon_type == 3) && hand1.number > 0 {
            take(&hand1, out)?;
        }
    }

    let min_base = vb::add_i16(to_i16(i64::from(item.min))?, min_str_bonus)?;
    let mut min_dmg = i32::from(min_base);
    let max_base = vb::add_i16(to_i16(i64::from(item.max))?, plus_max)?;
    let mut max_dmg = i32::from(max_base);
    if max_dmg < min_dmg {
        max_dmg = min_dmg;
    }

    let level = vb::cint(vb::val(&out.level_text))?;
    min_dmg = attack::calc_bs_damage(
        profile,
        level,
        stealth,
        to_i16(i64::from(min_dmg))?,
        plus_bs_min,
        input.class_stealth,
    );
    max_dmg = attack::calc_bs_damage(
        profile,
        level,
        stealth,
        to_i16(i64::from(max_dmg))?,
        plus_bs_max,
        input.class_stealth,
    );
    out.min_dmg = min_dmg;
    out.max_dmg = max_dmg;
    let sum = vb::add_i32(max_dmg, min_dmg)?;
    let avg = vb::round(f64::from(sum) / 2.0, 0)?;
    out.damage_text = format!("{min_dmg} - {max_dmg} (AVG: {})", vb::cstr(avg));
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Coin converter

/// MME's `eCoins`: the value of one coin in copper.
pub const COPPER: i32 = 1;
pub const SILVER: i32 = 10;
pub const GOLD: i32 = 100;
pub const PLATINUM: i32 = 10_000;
pub const RUNIC: i32 = 1_000_000;

/// The controls of frmCoinConvert that `CalcCoin` reads.
#[derive(Debug, Clone)]
pub struct CoinInput<'a> {
    /// 0 for the top row, 1 for the bottom row (the only values the form passes).
    pub source_index: usize,
    pub destination_index: usize,
    /// `optCoinTop(0..4).Value`: copper, silver, gold, platinum, runic.
    pub top: [bool; 5],
    pub bottom: [bool; 5],
    /// `txtCoin(source_index).Text`.
    pub source_text: &'a str,
    /// The optional `eForceSource`; 0 (or less) when left out.
    pub force_source: i32,
}

/// What `CalcCoin` leaves behind.
#[derive(Debug, Clone, PartialEq)]
pub struct CoinOutput {
    /// `txtCoin(destination_index).Text`.
    pub destination_text: String,
    /// `eCurrentCoin(destination_index)`.
    pub current_coin: i32,
}

/// The coin picked by a row of option buttons: the highest one set, copper when none is.
fn picked_coin(row: &[bool; 5]) -> i32 {
    if row[4] {
        RUNIC
    } else if row[3] {
        PLATINUM
    } else if row[2] {
        GOLD
    } else if row[1] {
        SILVER
    } else {
        COPPER
    }
}

/// Port of frmCoinConvert ConvertCoin: `coins` of one kind as another, rounded to 8 decimals.
/// The copper total is capped at 9,999,999,999. MME's handler swallows errors (an unset coin of 0
/// divides by zero), leaving the function value at 0.
#[must_use]
pub fn convert_coin(coins: f64, from: i32, to: i32) -> f64 {
    let run = || -> Res<f64> {
        let mut copper = coins * f64::from(from);
        if copper > 9_999_999_999.0 {
            copper = 9_999_999_999.0;
        }
        vb::round(vb::div(copper, f64::from(to))?, 8)
    };
    run().unwrap_or(0.0)
}

/// Port of frmCoinConvert CalcCoin: converts the source box into the destination box using the
/// coin kinds selected for each row. A source or destination that is neither 0 nor 1 would leave a
/// coin unset (0); the form never does that.
#[must_use]
pub fn calc_coin(input: &CoinInput) -> CoinOutput {
    let mut from = 0;
    let mut to = 0;
    let (s, d) = (input.source_index, input.destination_index);
    if s == 0 || d == 0 {
        let coin = picked_coin(&input.top);
        if s == 0 {
            from = coin;
        }
        if d == 0 {
            to = coin;
        }
    }
    if s == 1 || d == 1 {
        let coin = picked_coin(&input.bottom);
        if s == 1 {
            from = coin;
        }
        if d == 1 {
            to = coin;
        }
    }
    if input.force_source > 0 {
        from = input.force_source;
    }
    CoinOutput {
        destination_text: vb::cstr(convert_coin(vb::val(input.source_text), from, to)),
        current_coin: to,
    }
}

// ---------------------------------------------------------------------------------------------
// Exp calculator

/// The combo boxes of frmExpCalc.
#[derive(Debug, Clone, Copy)]
pub struct ExpCalcInput {
    /// `cmbClass.ItemData(ListIndex)` when `ListIndex > 0`, else `None`.
    pub class: Option<i32>,
    /// `cmbRace.ItemData(ListIndex)` when `ListIndex > 0`, else `None`.
    pub race: Option<i32>,
}

/// What `CalcExp` writes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExpCalcOutput {
    /// `txtCalcEXPTable.Text`; `None` when MME left it alone (its Integer sum overflowed).
    pub table_text: Option<String>,
    pub error: Option<u32>,
}

/// Port of frmExpCalc CalcExp: the exp table number for a class and race, the class's `ExpTable`
/// plus 100 plus the race's `ExpTable`. A row that is not found counts as 0. Both are Integers, so
/// an overflow ends the procedure before the box is written.
#[must_use]
pub fn calc_exp(input: &ExpCalcInput, classes: &[Class], races: &[Race]) -> ExpCalcOutput {
    let run = || -> Res<i16> {
        let mut class_exp: i16 = 0;
        let mut race_exp: i16 = 0;
        if let Some(n) = input.class
            && let Some(c) = classes.iter().find(|c| c.number == n)
        {
            class_exp = vb::add_i16(to_i16(i64::from(c.exp_table))?, 100)?;
        }
        if let Some(n) = input.race
            && let Some(r) = races.iter().find(|r| r.number == n)
        {
            race_exp = to_i16(i64::from(r.exp_table))?;
        }
        vb::add_i16(class_exp, race_exp)
    };
    match run() {
        Ok(v) => ExpCalcOutput {
            table_text: Some(v.to_string()),
            error: None,
        },
        Err(e) => ExpCalcOutput {
            table_text: None,
            error: Some(e.code()),
        },
    }
}
