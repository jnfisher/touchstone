//! Computed endpoints: shop prices and exp tables. The numbers come from `touchstone-rules`; this
//! module looks the rows up, shapes the JSON, and formats the text the way MME's list builders do.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::CACHE_CONTROL;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use touchstone_model::World;
use touchstone_rules::Profile;
use touchstone_rules::exp::{calc_exp_needed_by_race_class, calc_money_required_to_train};
use touchstone_rules::item_value::get_item_value;
use touchstone_vb6::{Currency, cstr, format, round};

use crate::state::AppState;

/// Shop type 8 is MME's "Training" shop.
const TRAINING: i32 = 8;
/// Longest level range one `/api/exp` call returns; MME's levels stop near 255.
const MAX_LEVELS: i32 = 1000;

pub(crate) type ApiResult = Result<Response, (StatusCode, String)>;

pub(crate) fn bad(msg: impl Into<String>) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, msg.into())
}

pub(crate) fn not_found(msg: impl Into<String>) -> (StatusCode, String) {
    (StatusCode::NOT_FOUND, msg.into())
}

fn fail(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

pub(crate) fn json<T: Serialize>(v: &T) -> ApiResult {
    Ok(([(CACHE_CONTROL, "no-cache")], Json(v)).into_response())
}

/// MME's `Format(n, "##,##0.00")` with a trailing `.00` removed.
fn trim_zero_cents(n: f64) -> Result<String, (StatusCode, String)> {
    let s = format(n, "##,##0.00").map_err(fail)?;
    Ok(s.strip_suffix(".00").map_or(s.clone(), str::to_owned))
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Cost {
    pub copper: f64,
    pub text: String,
}

/// The cost cell of a training row (`PullShopDetail`, shop type 8): copper to train to
/// `level`, shown with the coin it reduces to.
pub fn training_cost(level: i32, markup: i32) -> Result<Cost, (StatusCode, String)> {
    // MME's handler returns 0 when the Currency maths overflows.
    let cost = calc_money_required_to_train(
        Currency::from_scaled(i64::from(level) * 10_000),
        Currency::from_scaled(i64::from(markup) * 10_000),
    )
    .unwrap_or(Currency::from_scaled(0));
    let cost = if cost.scaled() < 0 {
        Currency::from_scaled(0)
    } else {
        cost
    };
    let copper = cost.to_f64();
    let ten = |n: i64| Currency::from_scaled(n * 10_000);
    let (div, coin) = if copper >= 10_000_000.0 {
        (Some(ten(1_000_000)), "Runic")
    } else if copper >= 100_000.0 {
        (Some(ten(10_000)), "Platinum")
    } else if copper >= 1000.0 {
        (Some(ten(100)), "Gold")
    } else if copper >= 100.0 {
        (Some(ten(10)), "Silver")
    } else {
        (None, "Copper")
    };
    let reduced = match div {
        Some(d) => cost.checked_div(d).map_err(fail)?.to_f64(),
        None => 0.0,
    };
    let reduced = if reduced > 0.0 {
        round(reduced, 2).map_err(fail)?
    } else {
        reduced
    };
    let text = if reduced == 0.0 {
        if copper <= 0.0 {
            "Free".to_owned()
        } else {
            format!("{} Copper", trim_zero_cents(copper)?)
        }
    } else {
        format!(
            "{} copper ({} {})",
            format_hash_comma(copper)?,
            trim_zero_cents(reduced)?,
            coin
        )
    };
    Ok(Cost { copper, text })
}

fn format_hash_comma(n: f64) -> Result<String, (StatusCode, String)> {
    format(n, "#,#").map_err(fail)
}

/// MME's `PutCommas(n, True)`: thousands separators, or trillions with three decimals.
pub fn put_commas_short(n: f64) -> Result<String, (StatusCode, String)> {
    if n.abs() >= 1e12 {
        let t = format(n.abs() / 1e12, "#,##0.000").map_err(fail)?;
        return Ok(format!("{}{t}T", if n < 0.0 { "-" } else { "" }));
    }
    let s = cstr(n);
    let (sign, rest) = s.strip_prefix('-').map_or(("", s.as_str()), |r| ("-", r));
    let (int, frac) = rest.split_once('.').map_or((rest, ""), |(i, f)| (i, f));
    let mut out = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    let dot = if frac.is_empty() { "" } else { "." };
    Ok(format!("{sign}{out}{dot}{frac}"))
}

// ---------- shop prices ----------

#[derive(Deserialize)]
pub struct PriceQuery {
    charm: Option<i32>,
}

#[derive(Serialize)]
struct PriceRow {
    slot: usize,
    item: i32,
    name: String,
    max: i32,
    regen: String,
    copper_buy: f64,
    buy: String,
    buy_short: String,
    copper_sell: f64,
    sell: String,
    sell_short: String,
}

#[derive(Serialize)]
struct PricesJson {
    shop: i32,
    name: String,
    shop_type: i32,
    min_lvl: i32,
    max_lvl: i32,
    markup: i32,
    class_rest: i32,
    charm: i32,
    /// MME's `lblCharmMod` text for buying and for selling; empty where MME shows none.
    buy_note: String,
    sell_note: String,
    /// True for a training shop, whose rows are `levels` rather than `items`.
    training: bool,
    items: Vec<PriceRow>,
    levels: Vec<TrainRow>,
}

#[derive(Serialize)]
struct TrainRow {
    level: i32,
    copper: f64,
    cost: String,
}

/// The "Regen" cell: "50% for 2 per 1h30m", built as `PullShopDetail` does.
fn regen_text(time: i32, amount: i32, pct: i32) -> String {
    if time <= 0 || pct <= 0 || amount <= 0 {
        return "no regen".to_owned();
    }
    let mut t = time;
    let mut s = String::new();
    if t >= 60 * 24 {
        s.push_str(&format!("{}d", t / (60 * 24)));
        t -= t / (60 * 24) * (60 * 24);
    }
    if t >= 60 {
        s.push_str(&format!("{}h", t / 60));
        t -= t / 60 * 60;
    }
    if t > 0 {
        s.push_str(&format!("{t}m"));
    }
    format!("{pct}% for {amount} per {s}")
}

/// The charm label above the list (`PullShopDetail`'s `sCharmMod`).
fn charm_notes(charm: i32, training: bool) -> (String, String) {
    if training {
        return (String::new(), String::new());
    }
    // Fix(charm / 5) - 10 is the percent the price moves; Fix on a non-negative is a floor.
    let step = charm / 5 - 10;
    let buy = if charm <= 0 {
        String::new()
    } else if step < 0 {
        format!("{}% Markup", -step)
    } else if step > 0 {
        format!("{step}% Discount")
    } else {
        "Retail Value".to_owned()
    };
    let sell = format!("{}% cost pre-markup", charm / 2 + 25);
    (buy, sell)
}

pub async fn shop_prices(
    State(state): State<Arc<AppState>>,
    Path(number): Path<i32>,
    Query(q): Query<PriceQuery>,
) -> ApiResult {
    let charm = q.charm.unwrap_or(0);
    // The game's charm is an Integer (`txtShopCharm` takes three digits).
    if !(0..=i32::from(i16::MAX)).contains(&charm) {
        return Err(bad("charm must be between 0 and 32767"));
    }
    let loaded = state.current();
    let world = &loaded.world;
    let shop = world
        .get_shop(number)
        .ok_or_else(|| not_found(format!("no shop {number}")))?;
    let training = shop.shop_type == TRAINING;
    let (buy_note, sell_note) = charm_notes(charm, training);
    let mut out = PricesJson {
        shop: shop.number,
        name: shop.name.clone(),
        shop_type: shop.shop_type,
        min_lvl: shop.min_lvl,
        max_lvl: shop.max_lvl,
        markup: shop.markup_pct,
        class_rest: shop.class_rest,
        charm,
        buy_note,
        sell_note,
        training,
        items: Vec::new(),
        levels: Vec::new(),
    };
    if training {
        // Level x costs CalcMoneyRequiredToTrain(x - 1, markup).
        let hi = shop.max_lvl.min(shop.min_lvl.saturating_add(MAX_LEVELS));
        for level in shop.min_lvl..=hi {
            let c = training_cost(level - 1, shop.markup_pct)?;
            out.levels.push(TrainRow {
                level,
                copper: c.copper,
                cost: c.text,
            });
        }
        return json(&out);
    }
    // MME hands the shop's markup straight to GetItemValue as an Integer.
    let markup = i16::try_from(shop.markup_pct).map_err(|_| fail("shop markup out of range"))?;
    for (slot, s) in shop.stock.iter().enumerate().filter(|(_, s)| s.item != 0) {
        let v = get_item_value(
            Profile::Stock,
            s.item,
            world.get_item(s.item),
            charm as i16,
            markup,
            0,
            None,
            false,
        )
        .map_err(fail)?;
        out.items.push(PriceRow {
            slot,
            item: s.item,
            name: world
                .get_item(s.item)
                .map_or_else(|| s.item.to_string(), |i| i.name.clone()),
            max: s.max,
            regen: regen_text(s.time, s.amount, s.pct),
            copper_buy: v.copper_buy,
            buy: v.friendly_buy,
            buy_short: v.friendly_buy_short,
            copper_sell: v.copper_sell,
            sell: v.friendly_sell,
            sell_short: v.friendly_sell_short,
        });
    }
    json(&out)
}

// ---------- exp table ----------

#[derive(Deserialize)]
pub struct ExpQuery {
    race: Option<i32>,
    class: Option<i32>,
    from: Option<i32>,
    to: Option<i32>,
    /// Trainer markup percent for the training cost; MME's trainers each have their own.
    markup: Option<i32>,
}

#[derive(Serialize)]
struct ExpRow {
    level: i32,
    exp: f64,
    exp_text: String,
    train_copper: f64,
    train_cost: String,
}

#[derive(Serialize)]
struct ExpJson {
    race: i32,
    class: i32,
    markup: i32,
    rows: Vec<ExpRow>,
}

fn known(world: &World, race: i32, class: i32) -> Result<(), (StatusCode, String)> {
    if race > 0 && world.get_race(race).is_none() {
        return Err(not_found(format!("no race {race}")));
    }
    if class > 0 && world.get_class(class).is_none() {
        return Err(not_found(format!("no class {class}")));
    }
    Ok(())
}

pub async fn exp_table(State(state): State<Arc<AppState>>, Query(q): Query<ExpQuery>) -> ApiResult {
    let (Some(race), Some(class)) = (q.race, q.class) else {
        return Err(bad("race and class are required (0 for none)"));
    };
    let (from, to) = (q.from.unwrap_or(1), q.to.unwrap_or(255));
    let markup = q.markup.unwrap_or(0);
    if from < 1 || to < from || to - from >= MAX_LEVELS {
        return Err(bad(format!(
            "from..to must be a range of at most {MAX_LEVELS} levels starting at 1 or more"
        )));
    }
    if !(0..=i32::from(i16::MAX)).contains(&markup) {
        return Err(bad("markup must be between 0 and 32767"));
    }
    let loaded = state.current();
    let world = &loaded.world;
    known(world, race, class)?;
    let mut rows = Vec::new();
    for level in from..=to {
        // MME's handler returns 0 where its Integer or Currency maths overflows.
        let exp = calc_exp_needed_by_race_class(
            Profile::Stock,
            level,
            class,
            race,
            &world.classes,
            &world.races,
        )
        .unwrap_or(0.0);
        let train = training_cost(level - 1, markup)?;
        rows.push(ExpRow {
            level,
            exp,
            exp_text: put_commas_short(exp)?,
            train_copper: train.copper,
            train_cost: train.text,
        });
    }
    json(&ExpJson {
        race,
        class,
        markup,
        rows,
    })
}
