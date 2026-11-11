//! `/api/calc/weapon-sim` over a small invented World. The tool is touchstone's own, so the
//! expected values are what its parts must agree on: MME's `ItemIsUsableByChar` for the list,
//! `CalculateAttack`'s damage a round for the simulated mean.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use touchstone::state::{Loaded, SourceInfo};
use touchstone::{AppState, app};
use touchstone_model::{AbilitySlot, Info, World, WorldParts};
use tower::ServiceExt;

use crate::calc::{class, item, race, weapon};

const SWORD: i32 = 100;
const LONG: i32 = 101;
const AXE: i32 = 102;
const UNHOLY: i32 = 103;
const RARE: i32 = 104;
const GONE: i32 = 105;
const THIEF: i32 = 2;

fn test_app() -> axum::Router {
    let mut rare = weapon(RARE, "rare blade", (5, 12, 1200, 0), &[]);
    rare.limit = 1;
    let mut gone = weapon(GONE, "lost club", (50, 90, 1000, 0), &[]);
    gone.in_game = false;
    let mut axe = weapon(AXE, "great axe", (14, 34, 2400, 0), &[(135, 40)]);
    axe.weapon_type = 3;
    // class 1 fights, class 2 also sneaks (ability 103); both wield anything (weapon type 8)
    let mut fighter = class(1, 0);
    let mut thief = class(THIEF, 0);
    for c in [&mut fighter, &mut thief] {
        c.weapon_type = 8;
        c.combat_lvl = 3;
    }
    thief.abilities[0] = AbilitySlot {
        ability: 103,
        value: 0,
    };
    let world = World::new(WorldParts {
        info: Info {
            nmr_version: "v1.8.3".into(),
            dat_file_version: String::new(),
            content_version: None,
            date: String::new(),
            time: String::new(),
            custom: String::new(),
            legit: true,
            disable_kai: false,
            update_url: String::new(),
        },
        items: vec![
            item(9, "iron ring", 5, 1),
            // 116: backstab accuracy, 135: minimum level, 98: evil only
            weapon(SWORD, "short sword", (4, 10, 1000, 0), &[(116, 10)]),
            weapon(LONG, "long sword", (6, 16, 1500, 0), &[(135, 20)]),
            axe,
            weapon(
                UNHOLY,
                "unholy dagger",
                (8, 20, 900, 0),
                &[(98, 0), (116, 20)],
            ),
            rare,
            gone,
        ],
        monsters: vec![],
        spells: vec![],
        rooms: vec![],
        shops: vec![],
        classes: vec![fighter, thief],
        races: vec![race(1, 0)],
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

/// A good level 25 character of `class` holding the short sword.
fn character(class: i32) -> Value {
    let mut equipped = [0; 20];
    equipped[16] = SWORD;
    json!({
        "name": "t", "race": 1, "class": class, "level": 25, "alignment": 1,
        "stats": [70, 50, 50, 70, 50, 50], "equipped": equipped, "quest_options": [0, 0, 0, 0],
    })
}

async fn post(body: Value) -> (StatusCode, Value) {
    let res = test_app()
        .oneshot(
            Request::post("/api/calc/weapon-sim")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, v)
}

async fn run(over: Value) -> Value {
    let mut body = json!({ "character": character(THIEF) });
    for (k, v) in over.as_object().unwrap() {
        body[k] = v.clone();
    }
    let (status, v) = post(body).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    v
}

fn numbers(v: &Value) -> Vec<i64> {
    let mut n: Vec<i64> = v["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["number"].as_i64().unwrap())
        .collect();
    n.sort_unstable();
    n
}

fn row(v: &Value, number: i32) -> &Value {
    v["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["number"] == number)
        .unwrap_or_else(|| panic!("no row {number} in {v}"))
}

#[tokio::test]
async fn lists_what_the_character_can_wield_and_simulates_it() {
    let v = run(json!({})).await;
    // ItemIsUsableByChar: the axe asks level 40, the dagger is evil only, the club is not in game
    assert_eq!(numbers(&v), [100, 101, 104]);
    assert_eq!(
        (v["usable"].as_i64(), v["simulated"].as_i64()),
        (Some(3), Some(3))
    );
    assert_eq!(v["equipped_weapon"], SWORD);
    assert_eq!(v["can_backstab"], true);
    let sword = row(&v, SWORD);
    assert_eq!(sword["equipped"], true);
    assert_eq!(sword["two_handed"], false);
    // only a weapon with backstab accuracy (ability 116) stabs
    assert!(sword["backstab"].is_object());
    assert!(row(&v, LONG)["backstab"].is_null());
    assert_eq!(row(&v, LONG)["level"], 20);
    // the simulated mean lands on CalculateAttack's damage a round
    for r in v["rows"].as_array().unwrap() {
        for mode in ["attack", "bash", "backstab"] {
            let m = &r[mode];
            if m.is_null() {
                continue;
            }
            let (want, got) = (
                m["expected"].as_f64().unwrap(),
                m["sim"]["mean"].as_f64().unwrap(),
            );
            assert!(want > 0.0, "{mode} of {r}");
            assert!(
                (got - want).abs() <= want * 0.06 + 1.0,
                "{mode}: {got} vs {want}"
            );
            assert_eq!(m["sim"]["rounds"], 5000);
            assert!(m["kill"].is_null(), "no hit points, no fight");
        }
    }
}

#[tokio::test]
async fn restrictions_can_be_ignored_and_say_so() {
    let v = run(json!({ "ignore_level": true })).await;
    assert_eq!(numbers(&v), [100, 101, 102, 104]);
    let axe = row(&v, AXE);
    assert_eq!(
        (&axe["needs_level"], &axe["wrong_alignment"]),
        (&json!(true), &json!(false))
    );
    assert_eq!(axe["two_handed"], true);
    assert_eq!(row(&v, LONG)["needs_level"], false);

    let v = run(json!({ "ignore_alignment": true })).await;
    assert_eq!(numbers(&v), [100, 101, 103, 104]);
    let dagger = row(&v, UNHOLY);
    assert_eq!(
        (&dagger["needs_level"], &dagger["wrong_alignment"]),
        (&json!(false), &json!(true))
    );

    let v = run(json!({ "include_limited": false })).await;
    assert_eq!(numbers(&v), [100, 101]);
}

#[tokio::test]
async fn the_shortlist_keeps_the_best_the_weapon_in_hand_and_the_pinned() {
    let all = run(json!({})).await;
    let best = |mode: &str| {
        all["rows"]
            .as_array()
            .unwrap()
            .iter()
            .max_by_key(|r| r[mode]["expected"].as_i64().unwrap_or(0))
            .unwrap()["number"]
            .as_i64()
            .unwrap()
    };
    let mut want = vec![
        best("attack"),
        best("bash"),
        best("backstab"),
        i64::from(SWORD),
    ];
    want.sort_unstable();
    want.dedup();
    let v = run(json!({ "candidates": 1 })).await;
    assert_eq!(numbers(&v), want);
    assert_eq!(v["usable"], 3);

    let v = run(json!({ "candidates": 1, "pinned": [RARE, LONG] })).await;
    assert_eq!(numbers(&v), [100, 101, 104]);
    assert_eq!(row(&v, RARE)["pinned"], true);
}

#[tokio::test]
async fn a_seed_repeats_and_a_target_changes_the_numbers() {
    let strip = |mut v: Value| {
        v.as_object_mut().unwrap().remove("elapsed_ms");
        v
    };
    let a = strip(run(json!({ "seed": 5 })).await);
    assert_eq!(a, strip(run(json!({ "seed": 5 })).await));
    assert_ne!(a, strip(run(json!({ "seed": 6 })).await));

    let soft = run(json!({ "target": { "hp": 200 } })).await;
    let hard = run(json!({ "target": { "hp": 200, "ac": 60, "dr": 3 } })).await;
    let dmg = |v: &Value| row(v, LONG)["attack"]["sim"]["mean"].as_f64().unwrap();
    assert!(dmg(&hard) < dmg(&soft));
    let kill = |v: &Value, mode: &str| row(v, SWORD)[mode]["kill"]["mean"].as_f64().unwrap();
    assert!(kill(&soft, "attack") > 1.0 && kill(&hard, "attack") > kill(&soft, "attack"));
    // a fight opened with the stab is over no later than one without
    assert!(kill(&soft, "backstab") <= kill(&soft, "attack") + 0.5);
    assert_eq!(row(&soft, SWORD)["attack"]["kill"]["estimated"], false);
    // too long a fight is not rolled: hit points over the damage a round
    let wall = run(json!({ "target": { "hp": 1_000_000 } })).await;
    let k = &row(&wall, SWORD)["attack"]["kill"];
    assert_eq!(k["estimated"], true);
    let per_round = row(&wall, SWORD)["attack"]["sim"]["mean"].as_f64().unwrap();
    assert!((k["mean"].as_f64().unwrap() - 1_000_000.0 / per_round).abs() < 1e-6);
}

#[tokio::test]
async fn a_class_without_stealth_has_no_backstab() {
    let (status, v) = post(json!({ "character": character(1) })).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["can_backstab"], false);
    assert!(
        v["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["backstab"].is_null())
    );
}

#[tokio::test]
async fn requests_outside_the_limits_are_refused() {
    for over in [
        json!({ "rounds": 0 }),
        json!({ "rounds": 50_001 }),
        json!({ "candidates": 0 }),
        json!({ "candidates": 61 }),
        json!({ "target": { "hp": -1 } }),
        json!({ "pinned": (0..21).collect::<Vec<i32>>() }),
    ] {
        let mut body = json!({ "character": character(THIEF) });
        for (k, v) in over.as_object().unwrap() {
            body[k] = v.clone();
        }
        let (status, v) = post(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{over}: {v}");
    }
    // a character without a class is the evaluate endpoint's 422
    let mut nobody = character(THIEF);
    nobody["class"] = Value::Null;
    let (status, _) = post(json!({ "character": nobody })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
