//! `/api/character/equip` and `/equip-state` over a tiny hand-made item table. The slot rules
//! themselves are pinned by fixtures/rules/equip.json in the rules crate; these check the wire.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use touchstone::state::{Loaded, SourceInfo};
use touchstone::{AppState, app};
use touchstone_model::{AbilitySlot, Class, Info, Item, World, WorldParts};
use tower::ServiceExt;

fn item(
    number: i32,
    name: &str,
    item_type: i32,
    worn: i32,
    weapon_type: i32,
    in_game: bool,
) -> Item {
    Item {
        number,
        name: name.into(),
        limit: 0,
        encum: 0,
        item_type,
        use_count: 0,
        price: 0,
        currency: 0,
        min: 0,
        max: 0,
        armour_class: 0,
        damage_resist: 0,
        weapon_type,
        armour_type: 0,
        worn,
        accy: 0,
        gettable: true,
        str_req: 0,
        speed: 0,
        not_droppable: false,
        destroy_on_death: false,
        retain_after_uses: false,
        class_rest: [0; 10],
        race_rest: [0; 10],
        negate_spell: [0; 10],
        abilities: [AbilitySlot::default(); 20],
        in_game,
        obtained_from: None,
        references: None,
    }
}

fn world() -> World {
    let mut helm = item(2, "class helm", 0, 2, 0, true);
    helm.class_rest[0] = 3;
    let class = Class {
        number: 1,
        name: "Warrior".into(),
        min_hits: 0,
        max_hits: 0,
        exp_table: 0,
        magery_type: 0,
        magery_lvl: 0,
        weapon_type: 8,
        armour_type: 9,
        combat_lvl: 0,
        abilities: [AbilitySlot::default(); 10],
    };
    World::new(WorldParts {
        info: Info {
            nmr_version: "v1.8.3".into(),
            dat_file_version: "v1.11p".into(),
            content_version: None,
            date: String::new(),
            time: String::new(),
            custom: String::new(),
            legit: false,
            disable_kai: false,
            update_url: String::new(),
        },
        items: vec![
            item(1, "iron helm", 0, 2, 0, true),
            helm,
            item(3, "ring a", 0, 4, 0, true),
            item(4, "ring b", 0, 13, 0, true),
            item(5, "wrist a", 0, 14, 0, true),
            item(6, "wrist b", 0, 14, 0, true),
            item(7, "great sword", 1, 0, 3, true),
            item(8, "tower shield", 0, 12, 0, true),
            item(9, "hidden ring", 0, 4, 0, false),
            item(10, "nowhere pants", 0, 0, 0, true),
        ],
        monsters: vec![],
        spells: vec![],
        rooms: vec![],
        shops: vec![],
        classes: vec![class],
        races: vec![],
        lairs: vec![],
        tbinfo: vec![],
    })
    .unwrap()
}

async fn call(app: &axum::Router, uri: &str, body: &Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, v)
}

fn setup() -> (TempDir, axum::Router) {
    let dir = TempDir::new().unwrap();
    let info = SourceInfo {
        kind: "mdb",
        path: "test".into(),
    };
    let loaded = Loaded::build(world(), info, None, 1).unwrap();
    let state = AppState::with_data_dir(loaded, dir.path().join("data"));
    (dir, app(state))
}

fn character(equipped: &[i32]) -> Value {
    json!({"name": "t", "class": 1, "race": 1, "level": 10, "equipped": equipped})
}

fn slots(v: &Value) -> Vec<i64> {
    v["character"]["equipped"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap())
        .collect()
}

#[tokio::test]
async fn equip_picks_the_slot_and_pairs_rings() {
    let (_d, app) = setup();
    let (st, v) = call(
        &app,
        "/api/character/equip",
        &json!({"character": character(&[]), "item": 3}),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(slots(&v)[9], 3);
    assert_eq!(v["slot"], 9);
    // a different ring takes the second finger
    let (_, v) = call(
        &app,
        "/api/character/equip",
        &json!({"character": v["character"], "item": 4}),
    )
    .await;
    assert_eq!((slots(&v)[9], slots(&v)[10]), (3, 4));
    // equipping the worn ring again changes nothing, with the toggle it comes off
    let (_, v2) = call(
        &app,
        "/api/character/equip",
        &json!({"character": v["character"], "item": 3}),
    )
    .await;
    assert_eq!(v2["changed"], false);
    assert_eq!((slots(&v2)[9], slots(&v2)[10]), (3, 4));
    let (_, v3) = call(
        &app,
        "/api/character/equip",
        &json!({"character": v["character"], "item": 3, "unequip": true}),
    )
    .await;
    assert_eq!((slots(&v3)[9], slots(&v3)[10]), (0, 4));
}

#[tokio::test]
async fn second_wrist_option_and_nowhere_message() {
    let (_d, app) = setup();
    let (_, v) = call(
        &app,
        "/api/character/equip",
        &json!({"character": character(&[]), "item": 5}),
    )
    .await;
    let (_, v) = call(
        &app,
        "/api/character/equip",
        &json!({"character": v["character"], "item": 6, "use_2nd_wrist": true}),
    )
    .await;
    assert_eq!((slots(&v)[6], slots(&v)[7]), (5, 6));
    let (_, v) = call(
        &app,
        "/api/character/equip",
        &json!({"character": character(&[]), "item": 10}),
    )
    .await;
    assert_eq!(slots(&v).iter().sum::<i64>(), 0);
    assert!(
        v["messages"][0]
            .as_str()
            .unwrap()
            .starts_with("Nowhere-Worn")
    );
}

#[tokio::test]
async fn two_handed_weapon_keeps_the_off_hand_and_flags_both() {
    let (_d, app) = setup();
    let mut eq = vec![0; 20];
    eq[15] = 8;
    let (_, v) = call(
        &app,
        "/api/character/equip",
        &json!({"character": character(&eq), "item": 7}),
    )
    .await;
    assert_eq!((slots(&v)[15], slots(&v)[16]), (8, 7));
    assert_eq!(v["flags"][15], true);
    assert_eq!(v["flags"][16], true);
}

#[tokio::test]
async fn slot_pick_checks_the_list() {
    let (_d, app) = setup();
    let (st, v) = call(
        &app,
        "/api/character/equip",
        &json!({"character": character(&[]), "item": 1, "slot": 1}),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    let (st, v) = call(
        &app,
        "/api/character/equip",
        &json!({"character": character(&[]), "item": 3, "slot": 10}),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(slots(&v)[10], 3);
    let (st, _) = call(
        &app,
        "/api/character/equip",
        &json!({"character": character(&[]), "item": 99, "slot": 0}),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, _) = call(
        &app,
        "/api/character/equip",
        &json!({"character": character(&[]), "item": 1, "slot": 20}),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn state_lists_by_slot_and_the_filter_narrows_them() {
    let (_d, app) = setup();
    let (_, v) = call(&app, "/api/character/equip-state", &character(&[])).await;
    let nums = |s: usize| -> Vec<i64> {
        v["choices"][s]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["number"].as_i64().unwrap())
            .collect()
    };
    assert_eq!(nums(0), vec![1, 2]);
    assert_eq!(nums(9), vec![3, 4]); // the hidden ring is not in game
    assert_eq!(nums(10), vec![3, 4]);
    assert_eq!(nums(16), vec![7]);
    assert_eq!(v["choices"][0][0]["label"], "iron helm (1)");
    let mut ch = character(&[]);
    ch["global_filter"] = json!(true);
    let (_, v) = call(&app, "/api/character/equip-state", &ch).await;
    // the class helm is restricted to class 3
    assert_eq!(v["choices"][0].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn unusable_item_is_flagged_without_the_filter() {
    let (_d, app) = setup();
    let mut eq = vec![0; 20];
    eq[0] = 2;
    let (_, v) = call(&app, "/api/character/equip-state", &character(&eq)).await;
    assert_eq!(v["flags"][0], true);
}
