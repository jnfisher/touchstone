//! Item value and shop price (FAM-18). Card P3-05.
//!
//! The callers look rows up (MME seeks `tabItems` / `tabShops` inside each procedure); these
//! functions take the row found, or `None` where MME's seek finds nothing. The GreaterMUD
//! gang-house markup is not ported; `Profile` has no other variant yet, so the dispatch reduces
//! to the stock branch.

use touchstone_model::{Item, Shop};
use touchstone_vb6::{Currency, cstr, fix, format, left, right, round};

use crate::{Profile, RulesError};

type R<T> = Result<T, RulesError>;

fn vb<T>(r: Result<T, touchstone_vb6::Vb6Error>) -> R<T> {
    r.map_err(|e| RulesError::Vb6(e.code() as i32))
}

/// MME's `tItemValue` (stock fields; the GreaterMUD gang-house fields are not ported).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ItemValue {
    pub base_cost: i32,
    pub base_coin: i32,
    pub copper_buy: f64,
    pub copper_sell: f64,
    pub friendly_buy: String,
    pub friendly_buy_short: String,
    pub friendly_sell: String,
    pub friendly_sell_short: String,
    pub markup: i16,
    pub shop_number: i32,
}

/// Port of modMMudDatabase `GetShopMarkup`. `shop` is the row `Seek` finds for `num`.
///
/// Zero is a Type mismatch in MME (it assigns "None" to an Integer), which its error handler
/// swallows, so the result is 0. A number outside Integer range overflows the same way. A shop
/// that is not found returns the number itself.
#[must_use]
pub fn get_shop_markup(num: i32, shop: Option<&Shop>) -> i16 {
    if num == 0 {
        return 0;
    }
    let Ok(n) = i16::try_from(num) else {
        return 0;
    };
    match shop {
        None => n,
        Some(s) => i16::try_from(s.markup_pct).unwrap_or(0),
    }
}

/// Port of modMMudDatabase `GetItemShopRegenPCT`: the chance (percent, at most 99, two
/// decimals) that a shop has the item in stock. `shop` is the row for `shop_num`.
///
/// # Errors
/// [`RulesError::Unsupported`] for a profile other than Stock.
pub fn get_item_shop_regen_pct(
    profile: Profile,
    shop_num: i32,
    item_num: i32,
    shop: Option<&Shop>,
) -> R<Currency> {
    match profile {
        Profile::Stock | Profile::Quicksilver(_) => {}
    }
    let zero = Currency::from_scaled(0);
    if item_num < 1 || shop_num < 1 {
        return Ok(zero);
    }
    let Some(shop) = shop else {
        return Ok(zero);
    };
    // Shop type 8 is not a real shop (no stock regeneration).
    if shop.shop_type == 8 {
        return Ok(zero);
    }
    let mut acc = zero;
    // An error inside the loop jumps to MME's `out:` label, which still clamps and rounds.
    let _ = (|| -> Result<(), touchstone_vb6::Vb6Error> {
        for slot in &shop.stock {
            if slot.item == item_num && slot.max > 0 {
                if slot.time > 0 && slot.pct > 0 && slot.amount > 0 {
                    let mult = Currency::from_f64(1440.0 / f64::from(slot.time))?;
                    let per_day = Currency::from_scaled(
                        mult.scaled()
                            .checked_mul(i64::from(slot.amount))
                            .ok_or(touchstone_vb6::Vb6Error::Overflow)?,
                    );
                    let term = per_day.to_f64() * (f64::from(slot.pct) / 100.0);
                    acc = Currency::from_f64(acc.to_f64() + term)?;
                } else {
                    // Stock only: a slot without regeneration data counts as a 1% chance.
                    acc = acc.checked_add(Currency::from_scaled(10_000))?;
                }
            }
        }
        Ok(())
    })();
    if acc > Currency::from_scaled(99 * 10_000) {
        acc = Currency::from_scaled(99 * 10_000);
    }
    Ok(round_currency_2(acc))
}

/// `Round(x, 2)` on a Currency value: exact, half to even on the hundredths.
fn round_currency_2(c: Currency) -> Currency {
    let s = c.scaled();
    let q = s.div_euclid(100);
    let r = s.rem_euclid(100);
    let q = match r.cmp(&50) {
        std::cmp::Ordering::Less => q,
        std::cmp::Ordering::Greater => q + 1,
        std::cmp::Ordering::Equal => q + (q & 1),
    };
    Currency::from_scaled(q * 100)
}

/// Copper per coin type 0..=4 (copper, silver, gold, platinum, runic); other codes leave the
/// accumulator at 0 in `GetItemValue` and at the base price in `GetItemCopperCost`.
fn coin_factor(currency: i32) -> Option<f64> {
    match currency {
        0 => Some(1.0),
        1 => Some(10.0),
        2 => Some(100.0),
        3 => Some(10_000.0),
        4 => Some(1_000_000.0),
        _ => None,
    }
}

/// Port of modMMudDatabase `GetItemCopperCost`: the item's price in copper. `item` is the row
/// for `item_number`; 0 when it is not found.
#[must_use]
pub fn get_item_copper_cost(item_number: i32, item: Option<&Item>) -> f64 {
    if item_number < 1 {
        return 0.0;
    }
    let Some(item) = item else {
        return 0.0;
    };
    let cost = f64::from(item.price);
    let copper = coin_factor(item.currency).map_or(cost, |f| cost * f);
    // Round() of a Double with no digits is banker's rounding.
    round(copper, 0).unwrap_or(0.0)
}

/// Which coin a copper amount is shown in, and the amount in that coin (not yet rounded).
fn reduce_coin(copper: f64) -> (f64, &'static str) {
    if copper >= 10_000_000.0 {
        (copper / 1_000_000.0, "Runic")
    } else if copper >= 100_000.0 {
        (copper / 10_000.0, "Platinum")
    } else if copper >= 1000.0 {
        (copper / 100.0, "Gold")
    } else {
        (copper / 10.0, "Silver")
    }
}

/// The "N Copper (M Coin)" text and the short form, from the copper amount and its reduced
/// coin amount (`GetItemValue` builds both for buy and sell the same way).
fn friendly(copper: f64, reduced: f64, coin: &str) -> R<(String, String)> {
    if reduced == 0.0 {
        return Ok(("(no value)".to_string(), "0".to_string()));
    }
    let mut s = format!("{} Copper", vb(format(copper, "#,#"))?);
    if reduced != copper {
        s.push_str(" (");
        s.push_str(&vb(format(reduced, "##,##0.00"))?);
        if vb(right(&s, 3))? == ".00" {
            let n = s.len() - 3;
            s = vb(left(&s, n as i32))?;
        }
        s.push(' ');
        s.push_str(coin);
        s.push(')');
    }
    let short = format!("{}{}", cstr(vb(round(reduced, 0))?), vb(left(coin, 1))?);
    Ok((s, short))
}

/// The wording `GetItemValue` gives a sell price, for any copper amount: `(text, short)`, as in
/// [`ItemValue::friendly_sell`] and [`ItemValue::friendly_sell_short`]. The amount is rounded to whole
/// copper first (half to even), so an average that is not a whole number reads like a price. Added for
/// the Chests tab; MME has no such procedure.
///
/// # Errors
/// [`RulesError::Vb6`] if a conversion fails.
pub fn friendly_copper(copper: f64) -> R<(String, String)> {
    let copper = vb(round(copper.max(0.0), 0))?;
    let (reduced, coin) = if copper >= 100.0 {
        let (r, c) = reduce_coin(copper);
        (if r > 0.0 { vb(round(r, 2))? } else { r }, c)
    } else {
        (copper, "Copper")
    };
    friendly(copper, reduced, coin)
}

/// Subtracts 4294967295 until the value fits an unsigned 32 bit total (MME's "overflow bug").
fn wrap_u32(mut x: f64) -> f64 {
    while x > 4_294_967_295.0 {
        x -= 4_294_967_295.0;
    }
    x
}

/// Port of modMMudDatabase `GetItemValue` (stock). `item` is the row for `item_number`;
/// `shop` the row for `shop_number` (only its markup is used, through [`get_shop_markup`]).
/// With no item the result is MME's empty value whose buy text is "unknown".
///
/// # Errors
/// [`RulesError::Unsupported`] for a profile other than Stock; [`RulesError::Vb6`] if a
/// conversion fails, where MME's handler would return the partly filled result.
#[allow(clippy::too_many_arguments)]
pub fn get_item_value(
    profile: Profile,
    item_number: i32,
    item: Option<&Item>,
    charm: i16,
    markup: i16,
    shop_number: i32,
    shop: Option<&Shop>,
    no_buy: bool,
) -> R<ItemValue> {
    match profile {
        Profile::Stock | Profile::Quicksilver(_) => {}
    }
    let mut out = ItemValue {
        friendly_buy: "unknown".to_string(),
        ..ItemValue::default()
    };
    let Some(item) = item.filter(|_| item_number >= 1) else {
        return Ok(out);
    };
    if item.price == 0 {
        out.friendly_sell = "(no value)".to_string();
        out.friendly_buy = "Free".to_string();
        return Ok(out);
    }
    let base_cost = f64::from(item.price);
    let mut markup = markup;
    let mut copper_buy = coin_factor(item.currency).map_or(0.0, |f| base_cost * f);
    let mut copper_sell = copper_buy;

    if !no_buy {
        if shop_number > 0 {
            markup = get_shop_markup(shop_number, shop);
        }
        if markup > 0 {
            copper_buy += fix(copper_buy * (f64::from(markup) / 100.0));
        }
    }

    if charm > 0 {
        let charm_mod_sell = fix(f64::from(charm) / 2.0) + 25.0;
        copper_sell = wrap_u32(charm_mod_sell * copper_sell);
        copper_sell = fix(copper_sell / 100.0);

        let charm_mod_buy = 1.0 - ((fix(f64::from(charm) / 5.0) - 10.0) / 100.0);
        if !no_buy {
            copper_buy = wrap_u32(charm_mod_buy * copper_buy);
        }
    }

    if copper_buy <= 0.0 {
        copper_buy = 0.0;
    }
    if copper_sell <= 0.0 {
        copper_sell = 0.0;
    }

    let (reduced_buy, coin_buy) = if copper_buy >= 100.0 {
        let (r, c) = reduce_coin(copper_buy);
        (if r > 0.0 { vb(round(r, 2))? } else { r }, c)
    } else {
        (vb(round(copper_buy, 0))?, "Copper")
    };
    let (reduced_sell, coin_sell) = if copper_sell >= 100.0 {
        let (r, c) = reduce_coin(copper_sell);
        (if r > 0.0 { vb(round(r, 2))? } else { r }, c)
    } else {
        (vb(round(copper_sell, 0))?, "Copper")
    };

    let (s, short) = friendly(copper_sell, reduced_sell, coin_sell)?;
    out.friendly_sell = s;
    out.friendly_sell_short = short;
    let mut friendly_buy = String::new();
    let mut friendly_buy_short = String::new();
    if !no_buy {
        let (s, short) = friendly(copper_buy, reduced_buy, coin_buy)?;
        friendly_buy = s;
        friendly_buy_short = short;
    }

    out.base_cost = item.price;
    out.base_coin = item.currency;
    out.copper_buy = vb(round(copper_buy, 0))?;
    out.copper_sell = vb(round(copper_sell, 0))?;
    out.friendly_buy = friendly_buy;
    out.friendly_buy_short = friendly_buy_short;
    out.markup = markup;
    out.shop_number = shop_number;
    Ok(out)
}
