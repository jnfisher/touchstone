//! `/api/calc/gear-plan` over a small invented World. The tool is touchstone's own, so the
//! expected values follow from the invented items: what the character's sheet (MME's
//! `CalcCharacterStats`) makes of them, and what their Obtained From lines say.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use touchstone::state::{Loaded, SourceInfo};
use touchstone::{AppState, app};
use touchstone_model::{Info, Item, World, WorldParts};
use tower::ServiceExt;

use crate::calc::{class, item, monster, race, weapon};

const HEAD: usize = 0;
const WRISTS: [usize; 2] = [6, 7];
const FINGERS: [usize; 2] = [9, 10];
const OFF_HAND: usize = 15;
const WEAPON: usize = 16;

const SWORD: i32 = 100;
const GREATSWORD: i32 = 101;
const CAP: i32 = 200;
const HELM: i32 = 201;
const CROWN: i32 = 202;
const ORPHAN_HELM: i32 = 203;
const HIGH_HELM: i32 = 204;
const ANVIL_HAT: i32 = 205;
const RING_A: i32 = 210;
const RING_B: i32 = 211;
const RING_C: i32 = 212;
const BRACER: i32 = 220;
const SHIELD: i32 = 230;
const WAR_PAINT: i32 = 240;
const CHEST: i32 = 300;

/// An armour row: `worn` is MME's Worn (2 head, 4 finger, 14 wrist, 12 off-hand, 19 face), AC in
/// tenths as the table stores it.
fn armour(number: i32, name: &str, worn: i32, ac: i32, from: &str) -> Item {
    let mut a = item(number, name, 0, 0);
    a.worn = worn;
    a.armour_class = ac;
    a.obtained_from = (!from.is_empty()).then(|| from.to_owned());
    a
}

fn test_app() -> axum::Router {
    let mut fighter = class(1, 0);
    fighter.weapon_type = 8;
    fighter.armour_type = 9;
    fighter.combat_lvl = 3;
    let mut high = armour(HIGH_HELM, "champion's helm", 2, 95, "Shop #3");
    high.abilities[0].ability = 135;
    high.abilities[0].value = 40;
    let mut heavy = armour(ANVIL_HAT, "anvil hat", 2, 85, "Shop #3");
    heavy.encum = 5000;
    // ability 4: max damage
    let mut paint = armour(WAR_PAINT, "war paint", 19, 0, "Shop #3");
    paint.abilities[0].ability = 4;
    paint.abilities[0].value = 6;
    let mut chest = item(CHEST, "oak chest", 0, 0);
    chest.item_type = 8;
    chest.obtained_from = Some("Monster #6(50%)".into());
    let mut great = weapon(GREATSWORD, "greatsword", (10, 20, 2000, 0), &[]);
    great.weapon_type = 3;
    let mut sword = weapon(SWORD, "short sword", (4, 10, 1000, 0), &[]);
    sword.obtained_from = Some("Shop #3".into());
    great.obtained_from = Some("Shop #3".into());
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
            sword,
            great,
            armour(CAP, "cap", 2, 10, "Shop #3"),
            armour(HELM, "helm", 2, 40, "Shop #3"),
            armour(
                CROWN,
                "crown",
                2,
                80,
                "Shop(sell) #3, Monster #5(10%), Item #300(20%), Room 1/7, NPC #6, Textblock #9(2%)",
            ),
            armour(ORPHAN_HELM, "helm of nowhere", 2, 90, ""),
            high,
            heavy,
            armour(RING_A, "iron ring", 4, 10, "Shop #3"),
            armour(RING_B, "steel ring", 4, 20, "Shop #3"),
            armour(RING_C, "gold ring", 4, 30, "Shop #3"),
            armour(BRACER, "bracer", 14, 20, "Shop #3"),
            armour(SHIELD, "shield", 12, 50, "Shop #3"),
            paint,
            chest,
        ],
        monsters: vec![
            monster(5, (0, 0, 0), &[], &[]),
            monster(6, (0, 0, 0), &[], &[]),
        ],
        spells: vec![],
        rooms: vec![],
        shops: vec![],
        classes: vec![fighter],
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

/// A level 25 fighter with the short sword and whatever `worn` puts in the other slots.
fn character(worn: &[(usize, i32)]) -> Value {
    let mut equipped = [0; 20];
    equipped[WEAPON] = SWORD;
    for (slot, n) in worn {
        equipped[*slot] = *n;
    }
    json!({
        "name": "t", "race": 1, "class": 1, "level": 25, "alignment": 0,
        "stats": [70, 50, 50, 70, 50, 50], "equipped": equipped, "quest_options": [0, 0, 0, 0],
    })
}

async fn post(body: Value) -> (StatusCode, Value) {
    let res = test_app()
        .oneshot(
            Request::post("/api/calc/gear-plan")
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

async fn run(worn: &[(usize, i32)], over: Value) -> Value {
    let mut body = json!({ "character": character(worn), "goal": "armour" });
    for (k, v) in over.as_object().unwrap() {
        body[k] = v.clone();
    }
    let (status, v) = post(body).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    v
}

fn suggested(v: &Value, slot: usize) -> i64 {
    v["slots"][slot]["suggested"]["number"]
        .as_i64()
        .unwrap_or(0)
}

#[tokio::test]
async fn armour_fills_every_slot_with_the_most_armour_class() {
    let v = run(&[(HEAD, CAP)], json!({})).await;
    // the anvil hat is the best head the character may wear today: the champion's helm asks
    // level 40 and nothing yields the helm of nowhere
    assert_eq!(suggested(&v, HEAD), i64::from(ANVIL_HAT));
    assert_eq!(v["slots"][HEAD]["current"]["number"], CAP);
    assert_eq!(v["slots"][HEAD]["changed"], true);
    // AC is stored in tenths: the cap's 1 gives way to the hat's 8.5
    assert_eq!(v["slots"][HEAD]["gain"], 7.5);
    // the two fingers share a list and take the two best rings, one each
    let mut rings = [suggested(&v, FINGERS[0]), suggested(&v, FINGERS[1])];
    rings.sort_unstable();
    assert_eq!(rings, [i64::from(RING_B), i64::from(RING_C)]);
    // one wrist unless the second is asked for
    assert_eq!(suggested(&v, WRISTS[0]), i64::from(BRACER));
    assert_eq!(v["slots"][WRISTS[1]]["closed"], "second_wrist");
    assert_eq!(suggested(&v, OFF_HAND), i64::from(SHIELD));
    // the weapon is the Weapon Simulator's business
    assert_eq!(v["slots"][WEAPON]["closed"], "weapon");
    assert_eq!(suggested(&v, WEAPON), i64::from(SWORD));
    assert_eq!(v["metric"], "Armour class");
    assert_eq!(
        (v["before"].as_f64(), v["after"].as_f64()),
        (Some(1.0), Some(20.5))
    );
    let ac = &v["stats"][0];
    assert_eq!(
        (&ac["label"], &ac["before"], &ac["after"]),
        (&json!("Armour class"), &json!(1.0), &json!(20.5))
    );
    // runners-up in the final set, with what they lose
    let alt = &v["slots"][HEAD]["alternatives"][0];
    assert_eq!(
        (&alt["item"]["number"], &alt["delta"]),
        (&json!(CROWN), &json!(-0.5))
    );
}

#[tokio::test]
async fn switches_locks_and_the_weapon_decide_what_is_offered() {
    let v = run(&[(HEAD, CAP)], json!({ "ignore_level": true })).await;
    assert_eq!(suggested(&v, HEAD), i64::from(HIGH_HELM));
    assert_eq!(v["slots"][HEAD]["suggested"]["needs_level"], true);
    assert_eq!(v["slots"][HEAD]["suggested"]["level"], 40);

    let v = run(&[(HEAD, CAP)], json!({ "only_obtainable": false })).await;
    assert_eq!(suggested(&v, HEAD), i64::from(ORPHAN_HELM));
    assert_eq!(v["slots"][HEAD]["suggested"]["unobtainable"], true);
    assert!(
        v["slots"][HEAD]["acquisition"]["sources"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let v = run(
        &[(HEAD, CAP)],
        json!({ "locked": [HEAD], "second_wrist": true }),
    )
    .await;
    assert_eq!(suggested(&v, HEAD), i64::from(CAP));
    assert_eq!(v["slots"][HEAD]["closed"], "locked");
    assert_eq!(v["slots"][HEAD]["changed"], false);
    // one bracer exists, so the second wrist stays empty
    assert_eq!(
        suggested(&v, WRISTS[0]) + suggested(&v, WRISTS[1]),
        i64::from(BRACER)
    );

    // a two-handed weapon closes the off-hand
    let v = run(&[(WEAPON, GREATSWORD)], json!({})).await;
    assert_eq!(v["slots"][OFF_HAND]["closed"], "two_handed");
    assert_eq!(suggested(&v, OFF_HAND), 0);
}

#[tokio::test]
async fn skipping_an_item_walks_down_the_slots_list() {
    // each "next best" drops the pick and plans again: anvil hat, crown, helm, then the cap stays
    let mut skipped = vec![];
    for want in [ANVIL_HAT, CROWN, HELM, CAP] {
        let v = run(&[(HEAD, CAP)], json!({ "skipped": skipped })).await;
        assert_eq!(
            suggested(&v, HEAD),
            i64::from(want),
            "after skipping {skipped:?}"
        );
        let names: Vec<i64> = v["skipped"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["number"].as_i64().unwrap())
            .collect();
        assert_eq!(
            names,
            skipped.iter().map(|n| i64::from(*n)).collect::<Vec<_>>()
        );
        skipped.push(want);
    }
    // the item worn is kept even when skipped, and a skipped ring leaves both fingers
    let v = run(&[(HEAD, CAP)], json!({ "skipped": [CAP, RING_C, 99_999] })).await;
    assert_eq!(v["slots"][HEAD]["current"]["number"], CAP);
    let mut rings = [suggested(&v, FINGERS[0]), suggested(&v, FINGERS[1])];
    rings.sort_unstable();
    assert_eq!(rings, [i64::from(RING_A), i64::from(RING_B)]);
    // an item that does not exist is not named back
    assert_eq!(v["skipped"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn only_an_item_that_does_more_for_the_goal_is_swapped_in() {
    // armour does nothing for damage: the melee plan leaves it all alone and takes the war paint
    // (max damage), the one item that hits harder
    let v = run(
        &[(HEAD, CAP), (FINGERS[0], RING_A)],
        json!({ "goal": "melee" }),
    )
    .await;
    let changed: Vec<usize> = v["slots"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["changed"] == true)
        .map(|s| s["slot"].as_u64().unwrap() as usize)
        .collect();
    assert_eq!(changed, [18]);
    assert_eq!(suggested(&v, 18), i64::from(WAR_PAINT));
    assert!(v["after"].as_f64() > v["before"].as_f64());
    assert!(v["slots"][18]["gain"].as_f64().unwrap() > 0.0);
    assert_eq!(v["metric"], "Damage a round");
}

#[tokio::test]
async fn limits_rule_sets_out_and_say_when_nothing_meets_them() {
    // the anvil hat weighs more than the character can carry: under a weight limit the crown,
    // next in armour class, takes the head
    let v = run(&[], json!({ "max_encum": 50 })).await;
    assert!(v["unmet"].as_array().unwrap().is_empty(), "{}", v["unmet"]);
    assert_eq!(suggested(&v, HEAD), i64::from(CROWN));
    let pct = v["stats"].as_array().unwrap().last().unwrap();
    assert_eq!(pct["label"], "Encumbrance %");
    assert!(pct["after"].as_f64().unwrap() <= 50.0);

    // a limit nothing meets: the plan gets as close as it can and says so
    let v = run(&[], json!({ "goal": "melee", "min_ac": 500 })).await;
    let unmet = v["unmet"].as_array().unwrap();
    assert_eq!(unmet.len(), 1);
    assert!(
        unmet[0].as_str().unwrap().starts_with("armour class"),
        "{unmet:?}"
    );
    assert_eq!(suggested(&v, HEAD), i64::from(ANVIL_HAT));
    assert_eq!(suggested(&v, OFF_HAND), i64::from(SHIELD));
}

#[tokio::test]
async fn a_suggested_item_says_where_it_comes_from() {
    let v = run(&[(HEAD, CAP)], json!({ "max_encum": 50 })).await;
    let a = &v["slots"][HEAD]["acquisition"];
    assert_eq!((&a["item"], &a["name"]), (&json!(CROWN), &json!("crown")));
    let kinds: Vec<&str> = a["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["kind"].as_str().unwrap())
        .collect();
    // a shop that only buys the crown is no source; the surest ways come first
    assert_eq!(kinds, ["room", "npc", "monster", "chest", "script"]);
    let src = |k: usize| &a["sources"][k];
    assert_eq!((&src(0)["map"], &src(0)["number"]), (&json!(1), &json!(7)));
    assert_eq!(src(1)["name"], "monster 6");
    // a straight drop: 10%, ten kills
    assert_eq!(
        (&src(2)["number"], &src(2)["pct"], &src(2)["kills"]),
        (&json!(5), &json!(10.0), &json!(10.0))
    );
    // the chest drops half the time and holds the crown one time in five
    assert_eq!(
        (&src(3)["number"], &src(3)["pct"], &src(3)["kills"]),
        (&json!(6), &json!(10.0), &json!(10.0))
    );
    assert_eq!(
        (
            &src(3)["chest"],
            &src(3)["chest_name"],
            &src(3)["chest_pct"]
        ),
        (&json!(CHEST), &json!("oak chest"), &json!(20.0))
    );
    assert_eq!(
        (&src(4)["number"], &src(4)["pct"]),
        (&json!(9), &json!(2.0))
    );
    // an unchanged slot lists nothing
    assert!(v["slots"][WEAPON]["acquisition"].is_null());
}

#[tokio::test]
async fn bad_requests_are_refused() {
    for over in [
        json!({ "locked": [20] }),
        json!({ "min_ac": -1 }),
        json!({ "max_encum": 1e9 }),
        json!({ "target": { "ac": -5 } }),
        json!({ "skipped": (0..201).collect::<Vec<i32>>() }),
    ] {
        let mut body = json!({ "character": character(&[]), "goal": "armour" });
        for (k, v) in over.as_object().unwrap() {
            body[k] = v.clone();
        }
        let (status, v) = post(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{over}: {v}");
    }
    let (status, _) = post(json!({ "character": character(&[]), "goal": "luck" })).await;
    assert!(status.is_client_error());
    let mut nobody = character(&[]);
    nobody["class"] = Value::Null;
    let (status, _) = post(json!({ "character": nobody, "goal": "armour" })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
