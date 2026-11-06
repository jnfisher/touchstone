//! `/api/shops/{n}/prices` and `/api/exp` over a small invented World.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use touchstone::state::{Loaded, SourceInfo};
use touchstone::{AppState, app};
use touchstone_model::{AbilitySlot, Class, Info, Item, Race, Shop, ShopSlot, World, WorldParts};
use tower::ServiceExt;

fn item(number: i32, name: &str, price: i32, currency: i32) -> Item {
    Item {
        number,
        name: name.to_owned(),
        limit: 0,
        encum: 0,
        item_type: 0,
        use_count: 0,
        price,
        currency,
        min: 0,
        max: 0,
        armour_class: 0,
        damage_resist: 0,
        weapon_type: 0,
        armour_type: 0,
        worn: 0,
        accy: 0,
        gettable: false,
        str_req: 0,
        speed: 0,
        not_droppable: false,
        destroy_on_death: false,
        retain_after_uses: false,
        class_rest: [0; 10],
        race_rest: [0; 10],
        negate_spell: [0; 10],
        abilities: [AbilitySlot::default(); 20],
        in_game: true,
        obtained_from: None,
        references: None,
    }
}

/// A weapon row with the stock database's values.
fn weapon(number: i32, name: &str, stats: (i32, i32, i32, i32), abils: &[(i32, i32)]) -> Item {
    let mut w = item(number, name, 0, 0);
    (w.min, w.max, w.speed, w.str_req) = stats;
    w.item_type = 1;
    for (slot, (ability, value)) in w.abilities.iter_mut().zip(abils) {
        *slot = AbilitySlot {
            ability: *ability,
            value: *value,
        };
    }
    w
}

fn shop(number: i32, shop_type: i32, markup: i32, lvls: (i32, i32), stock: &[ShopSlot]) -> Shop {
    let mut slots = [ShopSlot::default(); 20];
    slots[..stock.len()].copy_from_slice(stock);
    Shop {
        number,
        name: format!("shop {number}"),
        shop_type,
        min_lvl: lvls.0,
        max_lvl: lvls.1,
        markup_pct: markup,
        class_rest: 0,
        stock: slots,
        in_game: true,
        assigned_to: None,
    }
}

fn slot(item: i32, max: i32, time: i32, amount: i32, pct: i32) -> ShopSlot {
    ShopSlot {
        item,
        max,
        time,
        amount,
        pct,
    }
}

fn race(number: i32, exp_table: i32) -> Race {
    Race {
        number,
        name: format!("race {number}"),
        m_int: 0,
        m_wil: 0,
        m_str: 0,
        m_hea: 0,
        m_agl: 0,
        m_chm: 0,
        x_int: 0,
        x_wil: 0,
        x_str: 0,
        x_hea: 0,
        x_agl: 0,
        x_chm: 0,
        hp_per_lvl: 0,
        exp_table,
        base_cp: 0,
        abilities: [AbilitySlot::default(); 10],
    }
}

fn class(number: i32, exp_table: i32) -> Class {
    Class {
        number,
        name: format!("class {number}"),
        min_hits: 0,
        max_hits: 0,
        exp_table,
        magery_type: 0,
        magery_lvl: 0,
        weapon_type: 0,
        armour_type: 0,
        combat_lvl: 0,
        abilities: [AbilitySlot::default(); 10],
    }
}

fn app_with_world() -> axum::Router {
    let world = World::new(WorldParts {
        info: Info {
            nmr_version: "v1.8.3".into(),
            dat_file_version: String::new(),
            date: String::new(),
            time: String::new(),
            custom: String::new(),
            legit: true,
            disable_kai: false,
            update_url: String::new(),
        },
        items: vec![
            item(9, "iron ring", 5, 1),
            item(23, "silver amulet", 2, 2),
            // Items 48, 217 and 875 as the stock database has them.
            weapon(48, "fist", (0, 0, 0, 0), &[(0, -5), (0, 35)]),
            weapon(
                217,
                "elven war-spear",
                (6, 17, 1550, 50),
                &[(86, 2), (116, -20), (135, 10), (28, 1)],
            ),
            weapon(
                875,
                "starsteel mace",
                (11, 26, 2300, 60),
                &[(116, -10), (28, 3), (86, 3), (135, 30)],
            ),
        ],
        monsters: vec![],
        spells: vec![],
        rooms: vec![],
        shops: vec![
            shop(
                3,
                1,
                0,
                (0, 0),
                &[
                    slot(23, 5, 90, 2, 50),
                    slot(9, 3, 0, 0, 0),
                    slot(999, 1, 0, 0, 0),
                ],
            ),
            shop(5, 1, 10, (0, 0), &[slot(9, 1, 0, 0, 0)]),
            shop(4, 8, 0, (10, 11), &[]),
            shop(6, 8, 25, (101, 101), &[]),
        ],
        classes: vec![class(1, 0)],
        races: vec![race(1, 0), race(2, 20)],
        lairs: vec![],
        tbinfo: vec![],
    })
    .unwrap();
    let info = SourceInfo {
        kind: "test",
        path: String::new(),
    };
    app(AppState::new(Loaded::build(world, info, None, 1).unwrap()))
}

async fn get(uri: &str) -> (StatusCode, Value) {
    let res = app_with_world()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&body)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&body).into_owned()));
    (status, v)
}

async fn post(uri: &str, body: Value) -> (StatusCode, Value) {
    let res = app_with_world()
        .oneshot(
            Request::post(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&body)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&body).into_owned()));
    (status, v)
}

mod prices {
    use super::*;

    #[tokio::test]
    async fn charm_50_matches_the_item_value_fixture() {
        // fixtures/rules/item_value.json val_item_887: price 2 in currency 2, charm 50, no markup.
        let (st, v) = get("/api/shops/3/prices?charm=50").await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["training"], false);
        assert_eq!(v["buy_note"], "Retail Value");
        assert_eq!(v["sell_note"], "50% cost pre-markup");
        let first = &v["items"][0];
        assert_eq!(first["item"], 23);
        assert_eq!(first["name"], "silver amulet");
        assert_eq!(first["buy"], "200 Copper (20 Silver)");
        assert_eq!(first["sell"], "100 Copper (10 Silver)");
        assert_eq!(first["buy_short"], "20S");
        assert_eq!(first["copper_buy"], 200.0);
        assert_eq!(first["regen"], "50% for 2 per 1h30m");
        // fixtures/rules/item_value.json val_item_884: price 5 in currency 1, charm 50.
        let second = &v["items"][1];
        assert_eq!(second["buy"], "50 Copper");
        assert_eq!(second["sell"], "25 Copper");
        assert_eq!(second["regen"], "no regen");
        // A stocked item the table lacks still lists, as MME does (its name is the number).
        assert_eq!(v["items"][2]["name"], "999");
        assert_eq!(v["items"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn charm_defaults_to_zero_and_the_shop_markup_applies() {
        // Derived from GetItemValue: price 5 x 10 copper, +10% markup, no charm modifier.
        let (st, v) = get("/api/shops/5/prices").await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["charm"], 0);
        assert_eq!(v["buy_note"], "");
        assert_eq!(v["items"][0]["buy"], "55 Copper");
        assert_eq!(v["items"][0]["sell"], "50 Copper");
    }

    #[tokio::test]
    async fn high_charm_discounts_and_low_charm_marks_up() {
        let (_, v) = get("/api/shops/3/prices?charm=100").await;
        assert_eq!(v["buy_note"], "10% Discount");
        assert_eq!(v["sell_note"], "75% cost pre-markup");
        let (_, v) = get("/api/shops/3/prices?charm=20").await;
        assert_eq!(v["buy_note"], "6% Markup");
    }

    #[tokio::test]
    async fn training_shop_lists_levels() {
        // fixtures/rules/exp.json train-0117: CalcMoneyRequiredToTrain(9, 0) is 450 copper.
        let (st, v) = get("/api/shops/4/prices").await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["training"], true);
        assert_eq!(v["items"].as_array().unwrap().len(), 0);
        assert_eq!(v["levels"][0]["level"], 10);
        assert_eq!(v["levels"][0]["copper"], 450.0);
        assert_eq!(v["levels"][0]["cost"], "450 copper (45 Silver)");
        assert_eq!(v["levels"].as_array().unwrap().len(), 2);
        // fixtures/rules/exp.json train-1304: CalcMoneyRequiredToTrain(100, 25) is 6250 copper.
        let (_, v) = get("/api/shops/6/prices").await;
        assert_eq!(v["levels"][0]["cost"], "6,250 copper (62.50 Gold)");
    }

    #[tokio::test]
    async fn bad_requests() {
        assert_eq!(get("/api/shops/77/prices").await.0, StatusCode::NOT_FOUND);
        assert_eq!(
            get("/api/shops/3/prices?charm=-1").await.0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            get("/api/shops/3/prices?charm=40000").await.0,
            StatusCode::BAD_REQUEST
        );
    }
}

mod exp {
    use super::*;

    #[tokio::test]
    async fn levels_match_the_exp_fixtures() {
        let (st, v) = get("/api/exp?race=1&class=1&from=1&to=28").await;
        assert_eq!(st, StatusCode::OK);
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 28);
        assert_eq!(rows[0]["level"], 1);
        assert_eq!(rows[0]["exp"], 0.0);
        assert_eq!(rows[0]["train_cost"], "Free");
        // fixtures/rules/exp.json rc_10_1_1 and rc_28_1_1 (CalcExpNeededByRaceClass).
        assert_eq!(rows[9]["exp"], 81123.0);
        assert_eq!(rows[9]["exp_text"], "81,123");
        assert_eq!(rows[27]["exp"], 12_480_730.0);
        assert_eq!(rows[27]["exp_text"], "12,480,730");
        // fixtures/rules/exp.json train-0117: training to level 10 costs 450 copper.
        assert_eq!(rows[9]["train_copper"], 450.0);
        assert_eq!(rows[9]["train_cost"], "450 copper (45 Silver)");
    }

    #[tokio::test]
    async fn a_race_with_more_exp_needs_more() {
        let (_, a) = get("/api/exp?race=1&class=1&from=10&to=10").await;
        let (_, b) = get("/api/exp?race=2&class=1&from=10&to=10").await;
        let (ea, eb) = (
            a["rows"][0]["exp"].as_f64().unwrap(),
            b["rows"][0]["exp"].as_f64().unwrap(),
        );
        assert!(eb > ea, "{eb} should exceed {ea}");
    }

    #[tokio::test]
    async fn markup_changes_only_the_training_cost() {
        // fixtures/rules/exp.json train-1304: CalcMoneyRequiredToTrain(100, 25) is 6250 copper.
        let (_, v) = get("/api/exp?race=1&class=1&from=101&to=101&markup=25").await;
        assert_eq!(v["rows"][0]["train_copper"], 6250.0);
        assert_eq!(v["rows"][0]["train_cost"], "6,250 copper (62.50 Gold)");
    }

    #[tokio::test]
    async fn bad_requests() {
        assert_eq!(get("/api/exp?class=1").await.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            get("/api/exp?race=1&class=1&from=0").await.0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            get("/api/exp?race=1&class=1&from=5&to=4").await.0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            get("/api/exp?race=1&class=1&to=5000").await.0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            get("/api/exp?race=9&class=1").await.0,
            StatusCode::NOT_FOUND
        );
    }
}

#[test]
fn put_commas_short_groups_and_shortens() {
    // modSyntaxsFunc PutCommas(s, True).
    let s = |n| touchstone::calc::put_commas_short(n).unwrap();
    assert_eq!(s(0.0), "0");
    assert_eq!(s(999.0), "999");
    assert_eq!(s(1000.0), "1,000");
    assert_eq!(s(1_234_567.0), "1,234,567");
    assert_eq!(s(1_234_567_890_123.0), "1.235T");
}

mod calc_exp {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn table_number_is_class_plus_100_plus_race() {
        // fixtures/rules/calculators.json exp-*: the class's ExpTable + 100 + the race's.
        // Race 2 has ExpTable 20, class 1 has 0.
        let (st, v) = post("/api/calc/exp", json!({"class": 1, "race": 2})).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["table"], 120);
        // Custom entries (0) and absent ones give 0, as the fixture's exp-None-None.
        let (_, v) = post("/api/calc/exp", json!({"class": 0, "race": 0})).await;
        assert_eq!(v["table"], 0);
    }

    #[tokio::test]
    async fn levels_follow_the_list_button() {
        // fixtures/rules/exp.json rc_10_1_1: level 10 needs 81,123 at table 100 (class 1, race 1).
        let (_, v) = post(
            "/api/calc/exp",
            json!({"class": 1, "race": 1, "from": "9", "to": 10}),
        )
        .await;
        assert_eq!(v["table"], 100);
        // MME clamps the end box up to 10, so 9..10 lists two rows.
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1]["level"], 10);
        assert_eq!(rows[1]["exp"], 81123.0);
        assert_eq!(rows[1]["exp_text"], "81,123");
        // "Needed" is the difference from the row above; the first row counts from 0.
        assert_eq!(rows[0]["needed"], rows[0]["exp"]);
        let gap = rows[1]["exp"].as_f64().unwrap() - rows[0]["exp"].as_f64().unwrap();
        assert_eq!(rows[1]["needed"], gap);
    }

    #[tokio::test]
    async fn level_boxes_are_clamped_and_a_typed_table_wins() {
        let (_, v) = post(
            "/api/calc/exp",
            json!({"table": 100, "from": "0", "to": "9999"}),
        )
        .await;
        assert_eq!((v["from"].clone(), v["to"].clone()), (json!(2), json!(500)));
        assert_eq!(v["rows"].as_array().unwrap().len(), 499);
        // fixtures/rules/exp.json rc_28_1_1 (table 100): level 28 needs 12,480,730.
        assert_eq!(v["rows"][26]["level"], 28);
        assert_eq!(v["rows"][26]["exp_text"], "12,480,730");
    }

    #[tokio::test]
    async fn bad_requests() {
        let (st, _) = post("/api/calc/exp", json!({"class": 9})).await;
        assert_eq!(st, StatusCode::NOT_FOUND);
        let (st, _) = post("/api/calc/exp", json!({"class": "x"})).await;
        assert_eq!(st, StatusCode::BAD_REQUEST);
        let (st, _) = post("/api/calc/exp", json!({"from": [1]})).await;
        assert_eq!(st, StatusCode::BAD_REQUEST);
    }
}

mod calc_coin {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn converts_the_other_box() {
        // fixtures/rules/calculators.json coin01-0000: 250 silver (top) to runic (bottom) is 0.0025.
        let (st, v) = post(
            "/api/calc/coin",
            json!({"source": 0, "text": "250", "top": 1, "bottom": 4}),
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["destination"], 1);
        assert_eq!(v["destination_text"], "0.0025");
        assert_eq!(v["current_coin"], 1_000_000);
        // txtCoin_Change: "Weight: " & Fix(n / 3) while n is above 0, so a tiny amount reads 0.
        assert_eq!(v["weights"][0], "Weight: 83");
        assert_eq!(v["weights"][1], "Weight: 0");
        let (_, v) = post("/api/calc/coin", json!({"source": 0, "text": "0"})).await;
        assert_eq!(v["weights"], json!(["", ""]));
    }

    #[tokio::test]
    async fn opening_state_is_one_gold_in_copper() {
        // frmCoinConvert Form_Load: 1 gold on top, copper below, which reads 100.
        let (_, v) = post(
            "/api/calc/coin",
            json!({"source": 0, "text": 1, "top": 2, "bottom": 0}),
        )
        .await;
        assert_eq!(v["destination_text"], "100");
        assert_eq!(v["weights"][1], "Weight: 33");
    }

    #[tokio::test]
    async fn a_coin_button_converts_the_amount_it_held() {
        // optCoinTop_Click: CalcCoin(0, 0, eCurrentCoin(0)); 100 copper re-read as silver is 10.
        let (_, v) = post(
            "/api/calc/coin",
            json!({"source": 0, "text": "100", "top": 1, "bottom": 0, "force_source": 1}),
        )
        .await;
        assert_eq!(v["destination_text"], "100");
        let (_, v) = post(
            "/api/calc/coin",
            json!({"source": 0, "text": "100", "top": 2, "bottom": 0, "force_source": 10}),
        )
        .await;
        // 100 silver is 1000 copper, which the bottom (copper) box shows.
        assert_eq!(v["destination_text"], "1000");
    }

    #[tokio::test]
    async fn the_copper_total_is_capped() {
        // ConvertCoin caps the copper total at 9,999,999,999.
        let (_, v) = post(
            "/api/calc/coin",
            json!({"source": 0, "text": "99999999999", "top": 0, "bottom": 0}),
        )
        .await;
        assert_eq!(v["destination_text"], "9999999999");
    }

    #[tokio::test]
    async fn bad_requests() {
        for body in [
            json!({"source": 2, "text": "1"}),
            json!({"source": 0, "text": "1", "top": 5}),
            json!({"source": 0, "text": "1", "bottom": 9}),
            json!({"source": 0, "text": "1", "force_source": 7}),
            json!({"text": "1"}),
        ] {
            assert_eq!(
                post("/api/calc/coin", body.clone()).await.0,
                StatusCode::BAD_REQUEST,
                "{body}"
            );
        }
    }
}

mod calc_swing {
    use super::*;
    use serde_json::json;

    fn strings(v: &Value) -> Vec<&str> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect()
    }

    #[tokio::test]
    async fn custom_speed_matches_swing_0004() {
        // fixtures/rules/calculators.json swing-0004.
        let (st, v) = post(
            "/api/calc/swing",
            json!({"weapon": 217, "combat": 1, "agility": "90", "encum": "0", "level": "150",
                   "max_encum": "4800", "speed": "99.5", "strength": "180", "speed_option": 3}),
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["energy"], "Energy per swing: 58");
        assert_eq!(v["raw_swing"], "Raw swing: 17.2414");
        assert_eq!(v["qnd"], "QND Crits: 20");
        assert_eq!(v["encum"], "Encumbrance: 0%");
        assert_eq!(v["damage"], "60");
        assert_eq!(
            v["damage_caption"],
            "Avg damage vs 0 ac/dr\r\n(global filter off)"
        );
        assert_eq!(
            strings(&v["eu"]),
            ["14", "28", "42", "56", "12", "26", "40", "54", "10", "24"]
        );
        assert_eq!(strings(&v["swings"]), ["5"; 10]);
        assert_eq!(v["speed"], "99.5");
        assert!(v["error"].is_null());
    }

    #[tokio::test]
    async fn encumbrance_and_fast_speed_match_swing_0147() {
        // fixtures/rules/calculators.json swing-0147 (the Fast option).
        let (_, v) = post(
            "/api/calc/swing",
            json!({"weapon": 217, "combat": 4, "agility": 150, "encum": 2600, "level": 1,
                   "max_encum": 2600, "strength": 20, "speed_option": 0}),
        )
        .await;
        assert_eq!(v["energy"], "Energy per swing: 935");
        assert_eq!(v["encum"], "Encumbrance: 100%");
        assert_eq!(v["encum_pct"], 100);
        assert_eq!(v["damage"], "13");
    }

    #[tokio::test]
    async fn zero_energy_zeroes_the_table() {
        // fixtures/rules/calculators.json swing-0022: a negative strength gives 0 energy.
        let (_, v) = post(
            "/api/calc/swing",
            json!({"weapon": 217, "combat": 4, "agility": 150, "encum": 0, "level": 9999,
                   "max_encum": 1000, "strength": -10, "speed": "99.5", "speed_option": 2}),
        )
        .await;
        assert_eq!(v["energy"], "Energy per swing: 0");
        assert_eq!(v["raw_swing"], "Raw swing: 0");
        assert_eq!(strings(&v["swings"]), ["0"; 10]);
        assert_eq!(v["damage"], "");
    }

    #[tokio::test]
    async fn max_encum_can_follow_strength() {
        // modMMudFunc CalcEncum: 48 per point up to 100, then 84 a point.
        let (_, v) = post(
            "/api/calc/swing",
            json!({"weapon": 217, "strength": 120, "max_encum_auto": true, "max_encum": "1"}),
        )
        .await;
        assert_eq!(v["max_encum"], "6480");
        let (_, v) = post("/api/calc/swing", json!({"weapon": 217, "strength": 100})).await;
        assert_eq!(v["max_encum"], "4800");
    }

    #[tokio::test]
    async fn no_weapon_leaves_the_results_blank() {
        let (st, v) = post("/api/calc/swing", json!({})).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["energy"], "");
    }

    #[tokio::test]
    async fn bad_requests() {
        for body in [
            json!({"weapon": 217, "combat": 0}),
            json!({"weapon": 217, "combat": 6}),
            json!({"weapon": 217, "speed_option": 4}),
            json!({"weapon": 999999}),
            json!({"weapon": "x"}),
            json!({"level": true}),
        ] {
            assert_eq!(
                post("/api/calc/swing", body.clone()).await.0,
                StatusCode::BAD_REQUEST,
                "{body}"
            );
        }
    }
}

mod calc_backstab {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn range_matches_bs_0002() {
        // fixtures/rules/calculators.json bs-0002: values are clamped to 0..1000 and written back.
        let (st, v) = post(
            "/api/calc/backstab",
            json!({"weapon": 875, "bs_min": "12", "bs_max": "1000", "level": "100",
                   "stealth": "0", "max_dmg": "abc", "strength": "200", "class_stealth": true}),
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["damage"], "548 - 2524 (AVG: 1536)");
        assert_eq!(
            (v["min_dmg"].clone(), v["max_dmg"].clone()),
            (548.into(), 2524.into())
        );
        assert_eq!(v["bs_min"], "12");
        assert_eq!(v["bs_max"], "1000");
        assert_eq!(v["max_dmg_box"], "abc");
        assert_eq!(v["max_adj"], "");
    }

    #[tokio::test]
    async fn boxes_over_the_limit_are_written_back() {
        // CalcBS: the stealth box is limited to 1000 and the limit shown.
        let (_, v) = post(
            "/api/calc/backstab",
            json!({"weapon": 875, "stealth": 5000, "level": -4}),
        )
        .await;
        assert_eq!(v["stealth"], "1000");
        assert_eq!(v["level"], "0");
    }

    #[tokio::test]
    async fn weapons_without_bs_accuracy_cannot_backstab() {
        // fixtures/rules/calculators.json bs-0018: item 48 has no ability 116.
        let (_, v) = post(
            "/api/calc/backstab",
            json!({"weapon": 48, "bs_min": 999, "bs_max": 999, "max_dmg": 1001,
                   "stealth": 50, "strength": 255, "class_stealth": true}),
        )
        .await;
        assert_eq!(v["damage"], "No BS");
    }

    #[tokio::test]
    async fn bad_requests() {
        for body in [
            json!({"weapon": 999999}),
            json!({"weapon": 875, "class_stealth": "yes"}),
            json!({"weapon": 875, "stealth": {}}),
        ] {
            assert_eq!(
                post("/api/calc/backstab", body.clone()).await.0,
                StatusCode::BAD_REQUEST,
                "{body}"
            );
        }
        let req = Request::post("/api/calc/backstab")
            .header("content-type", "application/json")
            .body(Body::from("{not json"))
            .unwrap();
        let res = app_with_world().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }
}
