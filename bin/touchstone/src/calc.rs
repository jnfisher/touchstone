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
use touchstone_rules::ability_text::AbilityEnv;
use touchstone_rules::attack::CharacterProfile;
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

// ---------- calculator windows: POST /api/calc/{exp,coin,swing,backstab} ----------

/// A JSON body whose faults answer 400 like every other bad request here.
pub struct Body<T>(pub T);

impl<S: Send + Sync, T: serde::de::DeserializeOwned> axum::extract::FromRequest<S> for Body<T> {
    type Rejection = (StatusCode, String);

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(v)) => Ok(Body(v)),
            Err(e) => Err(bad(e.body_text())),
        }
    }
}

/// A text box's content. MME reads these with `Val`, so the client may send a string or a number
/// (null is an empty box).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Text(pub String);

impl<'de> Deserialize<'de> for Text {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        match serde_json::Value::deserialize(d)? {
            serde_json::Value::String(s) => Ok(Text(s)),
            serde_json::Value::Number(n) => Ok(Text(n.to_string())),
            serde_json::Value::Null => Ok(Text(String::new())),
            other => Err(D::Error::custom(format!(
                "expected text or number, got {other}"
            ))),
        }
    }
}

fn text(s: &str) -> Text {
    Text(s.to_owned())
}

/// The VB error text MME's handler would name, for the page to show next to the partial result.
pub(crate) fn vb_error_text(code: u32) -> String {
    match code {
        6 => "Overflow (run-time error 6)".to_owned(),
        11 => "Division by zero (run-time error 11)".to_owned(),
        n => format!("Run-time error {n}"),
    }
}

// --- exp ---

/// MME's level box limits in `cmdCalcExp_Click`.
const EXP_START: (f64, f64) = (2.0, 500.0);
const EXP_END: (f64, f64) = (10.0, 500.0);

#[derive(Deserialize)]
pub struct ExpCalcReq {
    /// Class and race record numbers; 0 or absent is the combo's "custom" entry.
    class: Option<i32>,
    race: Option<i32>,
    /// The exp table box when edited by hand; absent takes the number class and race give.
    table: Option<i32>,
    #[serde(default = "start_level")]
    from: Text,
    #[serde(default = "end_level")]
    to: Text,
}

fn start_level() -> Text {
    text("2")
}
fn end_level() -> Text {
    text("255")
}

#[derive(Serialize)]
struct ExpCalcRow {
    level: i32,
    exp: f64,
    exp_text: String,
    needed: f64,
    needed_text: String,
}

#[derive(Serialize)]
struct ExpCalcJson {
    /// `txtCalcEXPTable.Text`; null when MME's Integer sum overflowed.
    table: Option<i32>,
    from: i32,
    to: i32,
    rows: Vec<ExpCalcRow>,
    error: Option<u32>,
    error_text: Option<String>,
}

fn clamp_level(t: &Text, (lo, hi): (f64, f64)) -> Result<i32, (StatusCode, String)> {
    let v = touchstone_vb6::val(&t.0).clamp(lo, hi);
    touchstone_vb6::clng(v).map_err(|e| bad(e.to_string()))
}

pub async fn calc_exp(
    State(state): State<Arc<AppState>>,
    Body(req): Body<ExpCalcReq>,
) -> ApiResult {
    use touchstone_rules::calculators::{ExpCalcInput, calc_exp as core};
    let loaded = state.current();
    let world = &loaded.world;
    let known_id = |n: Option<i32>, what: &str, found: bool| match n {
        Some(n) if n > 0 && !found => Err(not_found(format!("no {what} {n}"))),
        _ => Ok(()),
    };
    known_id(
        req.class,
        "class",
        req.class.is_some_and(|n| world.get_class(n).is_some()),
    )?;
    known_id(
        req.race,
        "race",
        req.race.is_some_and(|n| world.get_race(n).is_some()),
    )?;
    let (from, to) = (
        clamp_level(&req.from, EXP_START)?,
        clamp_level(&req.to, EXP_END)?,
    );
    let calc = core(
        &ExpCalcInput {
            class: req.class.filter(|n| *n > 0),
            race: req.race.filter(|n| *n > 0),
        },
        &world.classes,
        &world.races,
    );
    let table = match req.table {
        Some(t) => Some(t),
        None => calc.table_text.as_deref().and_then(|t| t.parse().ok()),
    };
    let mut out = ExpCalcJson {
        table,
        from,
        to,
        rows: Vec::new(),
        error: calc.error,
        error_text: calc.error.map(vb_error_text),
    };
    let Some(table) = table else {
        return json(&out);
    };
    let mut last = 0.0;
    for level in from..=to {
        // MME's loop ends at the first run-time error, keeping the rows already listed.
        let exp = match touchstone_rules::exp::calc_exp_needed(Profile::Stock, level, table) {
            Ok(v) => v,
            Err(e) => {
                let code = match e {
                    touchstone_rules::RulesError::Vb6(c) => c as u32,
                    touchstone_rules::RulesError::Unsupported(_) => 5,
                };
                out.error = Some(code);
                out.error_text = Some(vb_error_text(code));
                break;
            }
        };
        let needed = exp - last;
        out.rows.push(ExpCalcRow {
            level,
            exp,
            exp_text: format(exp, "#,##0").map_err(fail)?,
            needed,
            needed_text: format(needed, "#,##0").map_err(fail)?,
        });
        last = exp;
    }
    json(&out)
}

// --- coin ---

#[derive(Deserialize)]
pub struct CoinReq {
    /// Which box was typed in (0 top, 1 bottom); the other is the answer.
    source: usize,
    /// The box that gets the answer; absent is the other box. A coin button converts its own
    /// row, so it names the same box as the source.
    destination: Option<usize>,
    /// What the typed box holds.
    #[serde(default)]
    text: Text,
    /// The coin selected in each row: 0 copper, 1 silver, 2 gold, 3 platinum, 4 runic.
    #[serde(default = "gold_coin")]
    top: usize,
    #[serde(default)]
    bottom: usize,
    /// A coin button was clicked: the coin the source box was last in, as copper value
    /// (1, 10, 100, 10000 or 1000000), so the amount is converted to the new coin.
    force_source: Option<i32>,
    /// The loaded character's Charm (`txtCharStats(5).Tag`) while the global filter is on; absent
    /// is no character, which leaves the charm button without an effect.
    charm: Option<f64>,
    /// The charm button was clicked (`cmdCharm_Click`): the bottom box is rescaled.
    #[serde(default)]
    apply_charm: bool,
}

fn gold_coin() -> usize {
    2
}

#[derive(Serialize)]
struct CoinJson {
    destination: usize,
    destination_text: String,
    /// Copper value of the destination's coin (`eCurrentCoin`).
    current_coin: i32,
    /// "Weight: n" under each box, empty while the amount is not above 0.
    weights: [String; 2],
    charm: CharmJson,
}

/// What `RefreshCharmButton` leaves on the button.
#[derive(Serialize)]
struct CharmJson {
    enabled: bool,
    /// "No Charm Effect" or "Apply" and the markup or discount on a second line (CRLF).
    caption: String,
    /// `cmdCharm.Tag`: the percent of the amount the button leaves.
    percent: Option<i32>,
}

/// Port of frmCoinConvert `RefreshCharmButton`. Charm 0 and 50 have no effect; the modifier is
/// `1 - (Fix(charm / 5) - 10) / 100`, kept in whole percents.
fn charm_button(charm: Option<f64>) -> CharmJson {
    let n = charm.map_or(0.0, f64::trunc);
    let none = || CharmJson {
        enabled: false,
        caption: "No Charm Effect".to_owned(),
        percent: None,
    };
    if n == 0.0 || n == 50.0 || n.abs() > 1.0e6 {
        return none();
    }
    let pct = 110 - (n / 5.0).trunc() as i32;
    let caption = match pct.cmp(&100) {
        std::cmp::Ordering::Greater => format!("{}% Markup", pct - 100),
        std::cmp::Ordering::Less => format!("{}% Discount", 100 - pct),
        std::cmp::Ordering::Equal => return none(),
    };
    CharmJson {
        enabled: true,
        caption: format!("Apply\r\n{caption}"),
        percent: Some(pct),
    }
}

/// Port of `cmdCharm_Click`: the bottom amount through the charm percent, kept within 1..999999999.
fn apply_charm(text: &str, coin: usize, percent: i32) -> Option<String> {
    use touchstone_rules::calculators::convert_coin;
    const VALUES: [i32; 5] = [1, 10, 100, 10_000, 1_000_000];
    let amount = touchstone_vb6::val(text);
    if amount < 1.0 {
        return None;
    }
    let cur = VALUES[coin];
    let copper = convert_coin(amount, cur, 1);
    let copper = round(copper * (f64::from(percent) / 100.0), 8).unwrap_or(copper);
    let shown = touchstone_vb6::val(&cstr(convert_coin(copper, 1, cur)));
    Some(if shown < 1.0 {
        "1".to_owned()
    } else if shown > 999_999_999.0 {
        "999999999".to_owned()
    } else {
        cstr(shown)
    })
}

fn coin_row(n: usize) -> Result<[bool; 5], (StatusCode, String)> {
    if n > 4 {
        return Err(bad("coin must be 0 (copper) to 4 (runic)"));
    }
    let mut row = [false; 5];
    row[n] = true;
    Ok(row)
}

fn weight_label(box_text: &str) -> String {
    let c = touchstone_vb6::val(box_text);
    if c > 0.0 {
        // Fix on a positive is a floor.
        format!("Weight: {}", (c / 3.0).floor())
    } else {
        String::new()
    }
}

pub async fn calc_coin(Body(req): Body<CoinReq>) -> ApiResult {
    use touchstone_rules::calculators::{CoinInput, calc_coin as core};
    if req.source > 1 {
        return Err(bad("source must be 0 (top box) or 1 (bottom box)"));
    }
    let charm = charm_button(req.charm);
    if req.apply_charm {
        let Some(percent) = charm.percent else {
            return Err(bad("the character's Charm has no effect on prices"));
        };
        let coin = req.bottom;
        coin_row(coin)?;
        let text = apply_charm(&req.text.0, coin, percent).unwrap_or_else(|| req.text.0.clone());
        return json(&CoinJson {
            destination: 1,
            weights: [String::new(), weight_label(&text)],
            destination_text: text,
            current_coin: [1, 10, 100, 10_000, 1_000_000][coin],
            charm,
        });
    }
    let force = req.force_source.unwrap_or(0);
    if !matches!(force, 0 | 1 | 10 | 100 | 10_000 | 1_000_000) {
        return Err(bad("force_source must be a coin's copper value"));
    }
    let destination = req.destination.unwrap_or(1 - req.source);
    if destination > 1 {
        return Err(bad("destination must be 0 (top box) or 1 (bottom box)"));
    }
    let out = core(&CoinInput {
        source_index: req.source,
        destination_index: destination,
        top: coin_row(req.top)?,
        bottom: coin_row(req.bottom)?,
        source_text: &req.text.0,
        force_source: force,
    });
    let mut weights = [String::new(), String::new()];
    // CalcCoin does not rewrite the source box; MME's change handler labels each box on its own.
    weights[req.source] = weight_label(&req.text.0);
    weights[destination] = weight_label(&out.destination_text);
    json(&CoinJson {
        destination,
        destination_text: out.destination_text,
        current_coin: out.current_coin,
        weights,
        charm,
    })
}

// --- swing and backstab ---

fn find_weapon(world: &World, n: Option<i32>) -> Result<(), (StatusCode, String)> {
    match n {
        Some(n) if world.get_item(n).is_none() => Err(bad(format!("no item {n}"))),
        _ => Ok(()),
    }
}

#[derive(Deserialize)]
pub struct SwingReq {
    weapon: Option<i32>,
    /// Combat level 1 (Poor) to 5 (Excellent).
    #[serde(default = "average_combat")]
    combat: i32,
    #[serde(default)]
    slowness: bool,
    #[serde(default)]
    bashing: bool,
    #[serde(default = "swing_encum")]
    encum: Text,
    #[serde(default = "swing_max_encum")]
    max_encum: Text,
    /// Take "Max Enc." from strength, as MME's strength box does when it changes.
    #[serde(default)]
    max_encum_auto: bool,
    #[serde(default = "swing_level")]
    level: Text,
    #[serde(default = "hundred")]
    agility: Text,
    #[serde(default = "hundred")]
    strength: Text,
    #[serde(default = "hundred")]
    speed: Text,
    /// 0 fast (85), 1 normal, 2 slow (125, which also ticks Slowness), 3 custom.
    #[serde(default = "normal_speed")]
    speed_option: usize,
    /// `txtTrueAVG(0..5)`: the MegaMUD Hit, Extra and Crit percentages and averages.
    #[serde(default)]
    hit_pct: Text,
    #[serde(default)]
    hit_avg: Text,
    #[serde(default)]
    extra_pct: Text,
    #[serde(default)]
    extra_avg: Text,
    #[serde(default)]
    crit_pct: Text,
    #[serde(default)]
    crit_avg: Text,
}

fn average_combat() -> i32 {
    3
}
fn swing_encum() -> Text {
    text("1000")
}
fn swing_max_encum() -> Text {
    text("4800")
}
fn swing_level() -> Text {
    text("60")
}
fn hundred() -> Text {
    text("100")
}
fn normal_speed() -> usize {
    1
}

#[derive(Serialize)]
struct SwingJson {
    qnd: String,
    energy: String,
    encum: String,
    encum_pct: Option<i16>,
    raw_swing: String,
    true_avg: String,
    /// `txtTrueAVG(7)`: `CalcTrueAverage` over the swings and the six MegaMUD boxes.
    true_average: Option<String>,
    /// The error `txtTrueAVG_Change` caught (a box that does not fit a Long).
    true_average_error: Option<String>,
    eu: Vec<String>,
    swings: Vec<String>,
    damage: String,
    damage_caption: String,
    speed: String,
    /// The Max Enc. the calculation used.
    max_encum: String,
    error: Option<u32>,
    error_text: Option<String>,
}

pub async fn calc_swing(
    State(state): State<Arc<AppState>>,
    Body(req): Body<SwingReq>,
) -> ApiResult {
    use touchstone_rules::attack::{AttackEnv, EquippedGlobals};
    use touchstone_rules::calculators::{SwingInput, calc_swings};
    use touchstone_rules::stats::{calc_encum, compute_avg_level_max_stats};
    if !(1..=5).contains(&req.combat) {
        return Err(bad("combat must be 1 (Poor) to 5 (Excellent)"));
    }
    if req.speed_option > 3 {
        return Err(bad("speed_option must be 0 to 3"));
    }
    let loaded = state.current();
    let world = &loaded.world;
    find_weapon(world, req.weapon)?;
    let caption = &world.info.dat_file_version;
    let max_encum = if req.max_encum_auto {
        // txtStrength_Change: CalcEncum(Val(strength)); an Integer argument that overflows ends
        // MME's handler with 0.
        let strength = touchstone_vb6::cint(touchstone_vb6::val(&req.strength.0)).unwrap_or(0);
        calc_encum(strength, 0, caption).to_string()
    } else {
        req.max_encum.0.clone()
    };
    let abilities = AbilityEnv::for_world(world, Profile::Stock);
    let equipped = EquippedGlobals::default();
    let env = AttackEnv {
        items: &world.items,
        classes: &world.classes,
        races: &world.races,
        equipped: &equipped,
        cast_text: &abilities,
        dat_ver_caption: caption,
        avg_level_max_all_stats: compute_avg_level_max_stats(Profile::Stock, &world.races)
            .all_stats,
        accuracy_tag: "",
    };
    let mut speed_options = [false; 4];
    speed_options[req.speed_option] = true;
    let out = calc_swings(
        Profile::Stock,
        &SwingInput {
            weapon: req.weapon,
            combat: req.combat,
            slowness: req.slowness,
            bashing: req.bashing,
            encum_text: &req.encum.0,
            max_encum_text: &max_encum,
            level_text: &req.level.0,
            agility_text: &req.agility.0,
            strength_text: &req.strength.0,
            speed_text: &req.speed.0,
            speed_options,
            global_filter: false,
            profile: CharacterProfile::default(),
        },
        &env,
    );
    // txtTrueAVG_Change: the Long arguments are the boxes rounded; one that does not fit is an
    // Overflow, which the handler reports and leaves the box as it was.
    let swings = touchstone_vb6::val(&out.true_avg_6);
    let amount = |t: &Text| touchstone_vb6::clng(touchstone_vb6::val(&t.0));
    let pct = |t: &Text| touchstone_vb6::val(&t.0);
    let (true_average, true_average_error) = match (
        amount(&req.hit_avg),
        amount(&req.crit_avg),
        amount(&req.extra_avg),
    ) {
        (Ok(h), Ok(c), Ok(e)) => (
            Some(cstr(touchstone_rules::attack::calc_true_average(
                swings,
                pct(&req.hit_pct),
                h,
                pct(&req.crit_pct),
                c,
                pct(&req.extra_pct),
                e,
            ))),
            None,
        ),
        _ => (None, Some(vb_error_text(6))),
    };
    json(&SwingJson {
        true_average,
        true_average_error,
        qnd: out.qnd_text,
        energy: out.energy_text,
        encum: out.encum_text,
        encum_pct: out.encum_pct,
        raw_swing: out.raw_swing_text,
        true_avg: out.true_avg_6,
        eu: out.eu.to_vec(),
        swings: out.swings.to_vec(),
        damage: out.damage_text,
        damage_caption: out.damage_caption,
        speed: out.speed_text,
        max_encum,
        error: out.error,
        error_text: out.error.map(vb_error_text),
    })
}

// --- MegaMUD stat-line paste ---

/// frmSwingCalc's message box when the clipboard lacks the `Hit:` or `Crit:` line.
const MEGA_INCOMPLETE: &str = "Incomplete/Missing MegaMUD Statistics in Clipboard";

#[derive(Deserialize)]
pub struct MegaReq {
    /// What MME reads from the clipboard.
    #[serde(default)]
    text: Text,
}

#[derive(Serialize)]
struct MegaJson {
    ok: bool,
    /// `txtTrueAVG(0..5)`: Hit %, Hit avg, Extra %, Extra avg, Crit %, Crit avg. Empty when the
    /// paste was rejected, because MME changes nothing then.
    fields: Option<[String; 6]>,
    /// The message box MME would show.
    message: Option<String>,
    error: Option<u32>,
}

/// Port of frmSwingCalc `cmdPasteMega_Click` over the text the page sends in place of the clipboard.
pub async fn calc_mega_stat(Body(req): Body<MegaReq>) -> ApiResult {
    use touchstone_rules::mega_stat::paste_mega_stat_line;
    let (fields, message, error) = match paste_mega_stat_line(&req.text.0) {
        Ok(Some(f)) => (Some(f), None, None),
        Ok(None) => (None, Some(MEGA_INCOMPLETE.to_owned()), None),
        // HandleError("cmdPasteMega_Click") reports it and the boxes stay as they were.
        Err(e) => (None, Some(vb_error_text(e.code())), Some(e.code())),
    };
    json(&MegaJson {
        ok: fields.is_some(),
        fields,
        message,
        error,
    })
}

#[derive(Deserialize)]
pub struct BsReq {
    weapon: Option<i32>,
    #[serde(default)]
    bs_min: Text,
    #[serde(default)]
    bs_max: Text,
    #[serde(default = "bs_level")]
    level: Text,
    #[serde(default = "hundred")]
    stealth: Text,
    #[serde(default)]
    max_dmg: Text,
    #[serde(default = "hundred")]
    strength: Text,
    #[serde(default)]
    class_stealth: bool,
}

fn bs_level() -> Text {
    text("50")
}

#[derive(Serialize)]
struct BsJson {
    /// "No BS" or "min - max (AVG: n)".
    damage: String,
    min_dmg: i32,
    max_dmg: i32,
    max_adj: String,
    bs_min_adj: String,
    bs_max_adj: String,
    stealth_adj: String,
    /// The boxes after MME clamps them to 0..1000.
    bs_min: String,
    bs_max: String,
    level: String,
    stealth: String,
    max_dmg_box: String,
    error: Option<u32>,
    error_text: Option<String>,
}

pub async fn calc_backstab(
    State(state): State<Arc<AppState>>,
    Body(req): Body<BsReq>,
) -> ApiResult {
    use touchstone_rules::calculators::{BsInput, calc_bs};
    let loaded = state.current();
    let world = &loaded.world;
    find_weapon(world, req.weapon)?;
    let out = calc_bs(
        &BsInput {
            weapon: req.weapon,
            bs_min_text: &req.bs_min.0,
            bs_max_text: &req.bs_max.0,
            level_text: &req.level.0,
            stealth_text: &req.stealth.0,
            max_dmg_text: &req.max_dmg.0,
            strength_text: &req.strength.0,
            class_stealth: req.class_stealth,
            global_filter: false,
            equipped: Default::default(),
        },
        &world.items,
    );
    json(&BsJson {
        damage: out.damage_text,
        min_dmg: out.min_dmg,
        max_dmg: out.max_dmg,
        max_adj: out.max_adj,
        bs_min_adj: out.bs_min_adj,
        bs_max_adj: out.bs_max_adj,
        stealth_adj: out.stealth_adj,
        bs_min: out.bs_min_text,
        bs_max: out.bs_max_text,
        level: out.level_text,
        stealth: out.stealth_text,
        max_dmg_box: out.max_dmg_text,
        error: out.error,
        error_text: out.error.map(vb_error_text),
    })
}

// --- hit calculator ---

mod hit {
    use serde::{Deserialize, Serialize};
    use touchstone_rules::character::AttackType;
    use touchstone_rules::hit_calc::{
        Attacker, Defender, HitCalcCharacter, HitCalcForm, HitCalcResult, HitCalcState,
        HitCalcType, Refresh, SetValsInput, do_hit_calc, get_monster_data, modify_hit_calc_values,
        set_hit_calc_vals,
    };

    use super::Text;

    /// The handler MME would run for what the person just did.
    #[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum Event {
        /// A box, Shadow or See Hidden changed.
        #[default]
        Calc,
        /// An option button, or the refresh-set button.
        Refresh,
        /// The monster combo picked a monster.
        Monster,
        /// One of the twelve arrow buttons (`index`).
        Modify,
    }

    #[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum CalcType {
        #[default]
        Normal,
        Backstab,
    }

    #[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum Att {
        #[default]
        Character,
        Monster,
        Manual,
    }

    #[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum Def {
        #[default]
        Character,
        Monster,
        Player,
        Manual,
    }

    /// The character's attack as `nGlobalAttackTypeMME` and `nGlobalAttackMA` hold it.
    #[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum Strike {
        #[default]
        Normal,
        Bash,
        Smash,
        Kick,
        Jumpkick,
    }

    /// Stands in for the loaded character until the Character tab can supply one. Numbers are text
    /// boxes read with `Val`, like the `Tag`s MME reads.
    #[derive(Debug, Deserialize, Default)]
    #[serde(default)]
    pub struct CharReq {
        pub accuracy: Text,
        pub ac: Text,
        pub dodge: Text,
        pub prot_evil: Text,
        pub stealth: Text,
        pub bs_accuracy: Text,
        pub agility: Text,
        /// `txtCharStats(0).Tag` and `txtGlobalLevel(0).Text`, which the backstab accuracy reads.
        pub strength: Text,
        pub level: Text,
        pub shadow_ac: i32,
        pub class: Option<i32>,
        /// The backstab weapon (`nGlobalAttackBackstabWeapon`); absent is none chosen.
        pub bs_weapon: Option<i32>,
        pub strike: Strike,
        /// The accuracy the bash, smash or kick adds (`nGlobalAttackAccyAdj`).
        pub accy_adj: i16,
    }

    #[derive(Debug, Deserialize, Serialize, Clone, Copy, Default)]
    #[serde(default)]
    pub struct Enabled {
        pub prot_evil: bool,
        pub vile_ward: bool,
        pub percep: bool,
        pub shadow: bool,
        pub see_hidden: bool,
        pub monster_list: bool,
    }

    #[derive(Debug, Deserialize, Serialize, Clone, Default)]
    #[serde(default)]
    pub struct StateJson {
        pub monster_accy: i32,
        pub monster_ac: i32,
        pub monster_dodge: i32,
        pub monster_bs_defense: i32,
        pub monster_evil: bool,
        pub monster_see_hidden: bool,
        pub defender_ac: i32,
        pub defender_dodge: i32,
        pub defender_perception: i32,
        pub defender_prot_evil: i32,
        pub defender_vile_ward: i32,
        pub defender_evilness: i32,
        pub defender_shadow: bool,
        pub char_perception: i32,
    }

    impl From<StateJson> for HitCalcState {
        fn from(s: StateJson) -> Self {
            HitCalcState {
                monster_accy: s.monster_accy,
                monster_ac: s.monster_ac,
                monster_dodge: s.monster_dodge,
                monster_bs_defense: s.monster_bs_defense,
                monster_evil: s.monster_evil,
                monster_see_hidden: s.monster_see_hidden,
                defender_ac: s.defender_ac,
                defender_dodge: s.defender_dodge,
                defender_perception: s.defender_perception,
                defender_prot_evil: s.defender_prot_evil,
                defender_vile_ward: s.defender_vile_ward,
                defender_evilness: s.defender_evilness,
                defender_shadow: s.defender_shadow,
                char_perception: s.char_perception,
            }
        }
    }

    impl From<&HitCalcState> for StateJson {
        fn from(s: &HitCalcState) -> Self {
            StateJson {
                monster_accy: s.monster_accy,
                monster_ac: s.monster_ac,
                monster_dodge: s.monster_dodge,
                monster_bs_defense: s.monster_bs_defense,
                monster_evil: s.monster_evil,
                monster_see_hidden: s.monster_see_hidden,
                defender_ac: s.defender_ac,
                defender_dodge: s.defender_dodge,
                defender_perception: s.defender_perception,
                defender_prot_evil: s.defender_prot_evil,
                defender_vile_ward: s.defender_vile_ward,
                defender_evilness: s.defender_evilness,
                defender_shadow: s.defender_shadow,
                char_perception: s.char_perception,
            }
        }
    }

    #[derive(Deserialize)]
    pub struct HitReq {
        #[serde(default)]
        pub event: Event,
        /// The arrow button for `modify`: accuracy -/+, AC -/+, dodge -/+, protection -/+, vile
        /// ward -/+, perception -/+.
        pub index: Option<usize>,
        #[serde(default)]
        pub calc_type: CalcType,
        #[serde(default)]
        pub attacker: Att,
        #[serde(default)]
        pub defender: Def,
        /// The monster combo's selection.
        pub monster: Option<i32>,
        /// `txtHitCalc(0..5)`.
        #[serde(default = "default_text")]
        pub text: [Text; 6],
        #[serde(default)]
        pub shadow: bool,
        #[serde(default)]
        pub see_hidden: bool,
        #[serde(default)]
        pub evil_index: i32,
        pub enabled: Option<Enabled>,
        #[serde(default)]
        pub state: StateJson,
        #[serde(default)]
        pub character: CharReq,
    }

    /// Form_Load's design-time boxes.
    fn default_text() -> [Text; 6] {
        ["100", "0", "10", "0", "0", "0"].map(super::text)
    }

    pub fn type_of(t: CalcType) -> HitCalcType {
        match t {
            CalcType::Normal => HitCalcType::Normal,
            CalcType::Backstab => HitCalcType::Backstab,
        }
    }
    pub fn attacker_of(a: Att) -> Attacker {
        match a {
            Att::Character => Attacker::Character,
            Att::Monster => Attacker::Monster,
            Att::Manual => Attacker::Manual,
        }
    }
    pub fn defender_of(d: Def) -> Defender {
        match d {
            Def::Character => Defender::Character,
            Def::Monster => Defender::Monster,
            Def::Player => Defender::Player,
            Def::Manual => Defender::Manual,
        }
    }

    /// `(attack type, martial art)` for a strike.
    pub fn strike_of(s: Strike) -> (AttackType, i16) {
        match s {
            Strike::Normal => (AttackType::PhysAttack, 0),
            Strike::Bash => (AttackType::PhysBash, 0),
            Strike::Smash => (AttackType::PhysSmash, 0),
            Strike::Kick => (AttackType::MartialArts, 2),
            Strike::Jumpkick => (AttackType::MartialArts, 3),
        }
    }

    pub fn run(
        form: &HitCalcForm,
        state: &HitCalcState,
        ch: &HitCalcCharacter,
        world: &touchstone_model::World,
        event: Event,
        index: Option<usize>,
    ) -> HitCalcResult {
        let profile = touchstone_rules::Profile::Stock;
        let set_vals = |refresh| {
            set_hit_calc_vals(
                profile,
                &SetValsInput {
                    form,
                    state,
                    character: ch,
                    items: &world.items,
                    classes: &world.classes,
                    refresh,
                },
            )
        };
        match event {
            Event::Calc => do_hit_calc(profile, form, state, ch.class),
            Event::Refresh => set_vals(Refresh::default()),
            Event::Monster => set_vals(Refresh {
                character_only: false,
                monster_only: true,
            }),
            Event::Modify => {
                let mut f = form.clone();
                modify_hit_calc_values(&mut f, index.unwrap_or(usize::MAX));
                do_hit_calc(profile, &f, state, ch.class)
            }
        }
    }

    pub fn load_monster(
        world: &touchstone_model::World,
        number: i32,
        nmr_ver: f64,
        state: &mut HitCalcState,
    ) -> bool {
        get_monster_data(&world.monsters, number, nmr_ver, state)
    }
}

#[derive(Serialize)]
struct HitErrorJson {
    source: &'static str,
    number: u32,
    text: String,
}

#[derive(Serialize)]
struct HitJson {
    calc_type: hit::CalcType,
    attacker: hit::Att,
    defender: hit::Def,
    /// The Monster attacker button is off while Backstab is chosen.
    attacker_monster_enabled: bool,
    text: Vec<String>,
    shadow: bool,
    see_hidden: bool,
    evil_index: i32,
    enabled: hit::Enabled,
    /// `lblBSPercep.Caption`, the label of the sixth box.
    bs_label: String,
    /// `lblMain.Caption`, `lblSubLeft.Caption`, `lblSubRight.Caption` (CRLF between lines).
    main_text: String,
    sub_left: String,
    sub_right: String,
    sub_right_bold: bool,
    /// The three percentages, absent when DoHitCalc stopped before printing them.
    hit: Option<i32>,
    dodge: Option<i32>,
    overall_hit: Option<i32>,
    state: hit::StateJson,
    errors: Vec<HitErrorJson>,
}

pub async fn calc_hit(State(app): State<Arc<AppState>>, Body(req): Body<hit::HitReq>) -> ApiResult {
    use touchstone_rules::hit_calc::{HitCalcCharacter, HitCalcForm};
    let loaded = app.current();
    let world = &loaded.world;

    if req.event == hit::Event::Modify && req.index.is_none_or(|i| i > 11) {
        return Err(bad("index must be 0 to 11 for a modify event"));
    }
    if !(0..=2).contains(&req.evil_index) {
        return Err(bad(
            "evil_index must be 0 (None), 1 (Criminal) or 2 (Fiend/Villain)",
        ));
    }
    if req.event == hit::Event::Monster && req.monster.is_none() {
        return Err(bad("a monster event needs a monster"));
    }
    let ch = &req.character;
    if let Some(c) = ch.class
        && world.get_class(c).is_none()
    {
        return Err(bad(format!("no class {c}")));
    }
    find_weapon(world, ch.bs_weapon)?;

    // optHitCalcType_Click: a backstab has no monster attacker.
    let attacker = if req.calc_type == hit::CalcType::Backstab && req.attacker == hit::Att::Monster
    {
        hit::Att::Manual
    } else {
        req.attacker
    };

    let mut state: touchstone_rules::hit_calc::HitCalcState = req.state.into();
    if let Some(n) = req.monster {
        let nmr = crate::detail::nmr_ver(&world.info.nmr_version);
        if !hit::load_monster(world, n, nmr, &mut state) {
            return Err(bad(format!("no monster {n}")));
        }
    }

    let (attack_type, martial_art) = hit::strike_of(ch.strike);
    let character = HitCalcCharacter {
        accuracy: touchstone_vb6::val(&ch.accuracy.0),
        ac: touchstone_vb6::val(&ch.ac.0),
        dodge: touchstone_vb6::val(&ch.dodge.0),
        prot_evil: touchstone_vb6::val(&ch.prot_evil.0),
        stealth: touchstone_vb6::val(&ch.stealth.0),
        bs_accuracy: touchstone_vb6::val(&ch.bs_accuracy.0),
        agility: touchstone_vb6::val(&ch.agility.0),
        strength: touchstone_vb6::val(&ch.strength.0),
        level: touchstone_vb6::val(&ch.level.0),
        class: ch.class,
        shadow_ac: ch.shadow_ac,
        attack_type,
        attack_accy_adj: ch.accy_adj,
        attack_martial_art: martial_art,
        attack_backstab: ch.bs_weapon.is_some(),
        attack_backstab_weapon: ch.bs_weapon.unwrap_or(0),
        ..HitCalcCharacter::default()
    };

    let mut form = HitCalcForm {
        calc_type: Some(hit::type_of(req.calc_type)),
        attacker: Some(hit::attacker_of(attacker)),
        defender: Some(hit::defender_of(req.defender)),
        text: req.text.map(|t| t.0),
        shadow: req.shadow,
        see_hidden: req.see_hidden,
        evil_index: req.evil_index,
        ..HitCalcForm::default()
    };
    // The enabled flags are control state from the last SetHitCalcVals; without them, derive what
    // that run would have left.
    let enabled = match req.enabled {
        Some(e) => e,
        None => {
            let d = hit::run(&form, &state, &character, world, hit::Event::Refresh, None).form;
            hit::Enabled {
                prot_evil: d.prot_evil_enabled,
                vile_ward: d.vile_ward_enabled,
                percep: d.percep_enabled,
                shadow: d.shadow_enabled,
                see_hidden: d.see_hidden_enabled,
                monster_list: d.monster_list_enabled,
            }
        }
    };
    form.prot_evil_enabled = enabled.prot_evil;
    form.vile_ward_enabled = enabled.vile_ward;
    form.percep_enabled = enabled.percep;
    form.shadow_enabled = enabled.shadow;
    form.see_hidden_enabled = enabled.see_hidden;
    form.monster_list_enabled = enabled.monster_list;

    let r = hit::run(&form, &state, &character, world, req.event, req.index);
    let f = &r.form;
    json(&HitJson {
        calc_type: req.calc_type,
        attacker,
        defender: req.defender,
        attacker_monster_enabled: req.calc_type == hit::CalcType::Normal,
        text: f.text.to_vec(),
        shadow: f.shadow,
        see_hidden: f.see_hidden,
        evil_index: f.evil_index,
        enabled: hit::Enabled {
            prot_evil: f.prot_evil_enabled,
            vile_ward: f.vile_ward_enabled,
            percep: f.percep_enabled,
            shadow: f.shadow_enabled,
            see_hidden: f.see_hidden_enabled,
            monster_list: f.monster_list_enabled,
        },
        bs_label: f.bs_label.clone(),
        main_text: f.main_text.clone(),
        sub_left: f.sub_left.clone(),
        sub_right: f.sub_right.clone(),
        sub_right_bold: f.sub_right_bold,
        hit: r.numbers.map(|n| n.hit),
        dodge: r.numbers.map(|n| n.dodge),
        overall_hit: r.numbers.map(|n| n.overall_hit),
        state: (&r.state).into(),
        errors: r
            .errors
            .iter()
            .map(|e| HitErrorJson {
                source: e.source,
                number: e.number,
                text: vb_error_text(e.number),
            })
            .collect(),
    })
}
