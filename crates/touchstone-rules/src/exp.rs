//! Exp tables and training cost (FAM-01). Card P3-01.
//!
//! Stock only: the GreaterMUD exp variants (`CalcExpNeeded_GMUD*`, `GetExpModifiers_GMUD`) are not
//! ported. `Profile` has no other variant yet, so the dispatch in MME's `CalcExpNeeded` reduces to
//! the stock procedure.
//!
//! An `Err` from a function here means MME's own error handler would have run (`HandleError`) and
//! the procedure would have returned 0.

use touchstone_model::{Class, Race};
use touchstone_vb6::{self as vb6, Currency, Vb6Error};

use crate::{Profile, RulesError};

const MAX_UINT: f64 = 4_294_967_295.0;
const BILLION: i32 = 1_000_000_000;

type R<T> = Result<T, RulesError>;

fn vb<T>(r: Result<T, Vb6Error>) -> R<T> {
    r.map_err(|e| RulesError::Vb6(e.code() as i32))
}

/// A Long value as Currency (exact).
fn lcur(n: i32) -> Currency {
    Currency::from_scaled(i64::from(n) * 10_000)
}

/// VB6 `Fix` on a Currency: truncates toward zero, stays Currency.
fn fix_cur(c: Currency) -> Currency {
    Currency::from_scaled(c.scaled() / 10_000 * 10_000)
}

/// Port of modMMudFunc `GetExpModifiers_STOCK` (Private in MME): the (multiplier, divisor) pair
/// for a level below 28; levels above 26 give (23, 20), so callers use it up to 27 only.
#[must_use]
pub fn get_exp_modifiers_stock(level: i32) -> (i32, i32) {
    match level {
        3 => (40, 20),
        4 | 5 => (44, 24),
        6 | 7 => (48, 28),
        8 | 9 => (52, 32),
        10 | 11 => (56, 36),
        12 | 13 => (60, 40),
        14 | 15 => (65, 45),
        16 | 17 => (70, 50),
        18 => (75, 55),
        _ if level <= 26 => (50, 40),
        _ => (23, 20),
    }
}

/// Port of modMMudFunc `CalcExpNeeded_STOCK`: total exp needed to reach `startlevel` on exp chart
/// `exptable` (race ExpTable + class ExpTable + 100). Levels below 1 give 0.
///
/// MME emulates the game's 32-bit exp arithmetic with a billions counter beside the running value,
/// and the loop restarts from level 1 on every call, so the result is not a closed form.
///
/// # Errors
/// `Vb6(6)` where MME's handler would catch an overflow (a chart so large that `exptable * 10`
/// overflows a Long), in which case MME returns 0.
pub fn calc_exp_needed_stock(startlevel: i32, exptable: i32) -> R<Currency> {
    let zero = Currency::from_scaled(0);
    let mut running = zero;
    let mut billions = zero;
    let mut ret = zero;

    for i in 1..=startlevel {
        if i == 1 {
            running = zero;
        } else if i == 2 {
            // Long * Integer literal: a Long product, which can overflow.
            running = lcur(vb(vb6::mul_i32(exptable, 10))?);
        } else {
            let (mult, div) = if i <= 27 {
                get_exp_modifiers_stock(i)
            } else if i <= 55 {
                (115, 100)
            } else if i <= 58 {
                (109, 100)
            } else {
                (108, 100)
            };

            let mut potential = if mult == 0 || div == 0 {
                zero
            } else {
                vb(running.checked_mul(lcur(mult)))?
            };

            let alternate = if potential.to_f64() > MAX_UINT {
                // "UINT ROLLOVER #1": shrink by 100 until the product fits 32 bits, then scale back.
                let mut num_divides = 0;
                while potential.to_f64() > MAX_UINT {
                    running = fix_cur(vb(running.checked_div(lcur(100)))?);
                    potential = vb(running.checked_mul(lcur(mult)))?;
                    num_divides += 1;
                }
                let mut alt = if num_divides > 1 {
                    let t = vb(running.checked_mul(lcur(mult)))?;
                    let t = vb(t.checked_mul(lcur(100)))?;
                    fix_cur(vb(t.checked_div(lcur(div)))?)
                } else {
                    fix_cur(vb(potential.checked_div(lcur(div)))?)
                };
                while num_divides > 0 {
                    alt = vb(alt.checked_mul(lcur(100)))?;
                    num_divides -= 1;
                }
                alt
            } else {
                fix_cur(vb(potential.checked_div(lcur(div)))?)
            };

            // `1000000 * exp_multiplier` is a Long product before it meets the Currency.
            let long_part = vb(vb6::mul_i32(1_000_000, mult))?;
            let mut j = vb(lcur(long_part).checked_mul(billions))?;
            while j.to_f64() > MAX_UINT {
                // "UINT ROLLOVER #2": Currency minus a Double is a Double, converted back on assignment.
                j = vb(Currency::from_f64(j.to_f64() - MAX_UINT - 1.0))?;
            }
            while j >= lcur(BILLION) {
                j = vb(j.checked_sub(lcur(BILLION)))?;
                billions = vb(billions.checked_add(lcur(1)))?;
            }

            let mut k = vb(j.checked_add(alternate))?;
            while k >= lcur(BILLION) {
                k = vb(k.checked_sub(lcur(BILLION)))?;
                billions = vb(billions.checked_add(lcur(1)))?;
            }
            running = k;
        }

        if i >= startlevel {
            ret = vb(running.checked_add(vb(billions.checked_mul(lcur(BILLION)))?))?;
        }
    }
    Ok(ret)
}

/// Port of modMMudFunc `CalcExpNeeded`: exp needed as a Double. Stock rules only.
///
/// # Errors
/// As [`calc_exp_needed_stock`].
pub fn calc_exp_needed(profile: Profile, startlevel: i32, exptable: i32) -> R<f64> {
    match profile {
        Profile::Stock => Ok(calc_exp_needed_stock(startlevel, exptable)?.to_f64()),
    }
}

/// Port of modMMudDatabase `CalcExpNeededByRaceClass`. `classes` and `races` stand for the Classes
/// and Races tables; a number of 0 or less, or one with no row, adds nothing (a missing class does
/// not get the +100 either).
///
/// # Errors
/// `Vb6(6)` where an Integer sum would overflow in MME.
pub fn calc_exp_needed_by_race_class(
    profile: Profile,
    level: i32,
    class_number: i32,
    race_number: i32,
    classes: &[Class],
    races: &[Race],
) -> R<f64> {
    // nClassExp and nRaceExp are Integers, so the sums below are Integer arithmetic.
    let mut class_exp: i16 = 0;
    let mut race_exp: i16 = 0;
    if class_number > 0
        && let Some(c) = classes.iter().find(|c| c.number == class_number)
    {
        class_exp = vb(vb6::add_i16(vb(vb6::cint(f64::from(c.exp_table)))?, 100))?;
    }
    if race_number > 0
        && let Some(r) = races.iter().find(|r| r.number == race_number)
    {
        race_exp = vb(vb6::cint(f64::from(r.exp_table)))?;
    }
    let chart = vb(vb6::add_i16(class_exp, race_exp))?;
    calc_exp_needed(profile, level, i32::from(chart))
}

/// Port of modMMudDatabase `GetMaxLevel`: 255, or the average level at which a race maxes all six
/// stats rounded up to a multiple of 5 when that is above 255. `avg_level_max_all_stats` is the
/// global `gAvgLevelMaxAllStats`.
///
/// # Errors
/// `Vb6(6)` when the average does not fit a Long (MME has no handler here).
pub fn get_max_level(avg_level_max_all_stats: f64) -> R<i32> {
    let mut max = 255;
    if avg_level_max_all_stats > 0.0 {
        let n = vb(vb6::clng(avg_level_max_all_stats))?; // ByVal Long argument: rounds half to even
        let dyn_level = round_up_to_5(n)?;
        if dyn_level > 255 {
            max = dyn_level;
        }
    }
    Ok(max)
}

/// Port of modSyntaxsFunc `RoundUpTo5`.
pub fn round_up_to_5(n: i32) -> R<i32> {
    let r = n % 5;
    if r == 0 {
        Ok(n)
    } else if n >= 0 {
        vb(vb6::add_i32(n, 5 - r))
    } else {
        vb(vb6::sub_i32(n, r))
    }
}

/// Port of modMMudFunc `CalcMoneyRequiredToTrain`: copper farthings to train to `level` at a shop
/// `markup` percent. Both are Currency in MME and the result is truncated to a whole number before
/// the times-ten.
///
/// # Errors
/// `Vb6(6)` on Currency overflow (MME's handler returns 0).
pub fn calc_money_required_to_train(level: Currency, markup: Currency) -> R<Currency> {
    let a = vb(level.checked_mul(lcur(5)))?;
    let b = vb(markup.checked_add(lcur(100)))?;
    let c = vb(a.checked_mul(b))?;
    let d = fix_cur(vb(c.checked_div(lcur(100)))?);
    vb(d.checked_mul(lcur(10)))
}
