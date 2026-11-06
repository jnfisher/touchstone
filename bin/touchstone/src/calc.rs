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
fn vb_error_text(code: u32) -> String {
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
    let force = req.force_source.unwrap_or(0);
    if !matches!(force, 0 | 1 | 10 | 100 | 10_000 | 1_000_000) {
        return Err(bad("force_source must be a coin's copper value"));
    }
    let destination = 1 - req.source;
    let out = core(&CoinInput {
        source_index: req.source,
        destination_index: destination,
        top: coin_row(req.top)?,
        bottom: coin_row(req.bottom)?,
        source_text: &req.text.0,
        force_source: force,
    });
    let mut weights = [String::new(), String::new()];
    weights[req.source] = weight_label(&req.text.0);
    weights[destination] = weight_label(&out.destination_text);
    json(&CoinJson {
        destination,
        destination_text: out.destination_text,
        current_coin: out.current_coin,
        weights,
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
    json(&SwingJson {
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
