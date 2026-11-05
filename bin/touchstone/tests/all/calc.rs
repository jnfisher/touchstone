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
        items: vec![item(9, "iron ring", 5, 1), item(23, "silver amulet", 2, 2)],
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
