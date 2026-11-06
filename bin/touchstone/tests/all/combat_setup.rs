//! The combat setup dialog's endpoints, on the character of fixtures/rules/pipeline.json cases.
//! The expected captions and fall-backs come from MME's frmPopUpOptions RefreshCombatStats and
//! frmMain PopUpChooseCombatGUI; the character comes from the fixture case named in each test.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::character::{call, env, request};

const CASE: &str = "attack-1-heal-4";

fn attack(over: Value) -> Value {
    let mut a = json!({
        "attack_type": 0, "backstab": false, "backstab_weapon": 0, "martial_art": 0,
        "spell_num": 0, "spell_lvl": 0, "manual_p": 0, "manual_m": 0, "use_meditate": false,
        "heal_type": 0, "heal_spell_num": 0, "heal_spell_lvl": 5, "heal_rounds": 30,
        "heal_manual": 0,
    });
    for (k, v) in over.as_object().unwrap() {
        a[k] = v.clone();
    }
    a
}

async fn preview(draft: Value) -> Value {
    let e = env();
    let (mut req, _) = request(&e.fx, CASE);
    req["attack"] = attack(json!({}));
    let body = json!({ "character": req, "draft": draft });
    let (status, got) = call(&e.app, "POST", "/api/character/combat-preview", Some(&body)).await;
    assert_eq!(status, StatusCode::OK, "{got}");
    got
}

#[tokio::test]
async fn one_shot_and_infinite_heal() {
    // RefreshCombatStats: one-shot is 999999 damage, an infinite heal is 99999
    let got = preview(attack(json!({}))).await;
    assert_eq!(got["attack"]["caption"], "999999 dmg/rnd");
    assert_eq!(got["heal"]["caption"], "99999 heal/rnd");
    assert_eq!(got["heal"]["mana_caption"], "0 mana/rnd");
    assert_eq!(got["attack_name"], "one-shot");
}

#[tokio::test]
async fn manual_attack_and_manual_heal() {
    // RefreshCombatStats: manual damage is physical + magic, a manual heal is the box (to 99999)
    let got = preview(attack(
        json!({"attack_type": 5, "manual_p": 150, "manual_m": 15, "heal_type": 4, "heal_manual": 123456}),
    ))
    .await;
    assert_eq!(got["attack"]["caption"], "165 dmg/rnd");
    assert_eq!(got["heal"]["caption"], "99999 heal/rnd");
    // GetCurrentAttackName: both kinds of damage
    assert_eq!(got["attack_name"], "man dmg");
    assert_eq!(got["applied"]["heal_manual"], 123456);
}

#[tokio::test]
async fn spell_attack_and_heal_use_the_spell_numbers() {
    let got = preview(attack(json!({
        "attack_type": 3, "spell_num": 17, "spell_lvl": 20,
        "heal_type": 3, "heal_spell_num": 123, "heal_spell_lvl": 10, "heal_rounds": 4
    })))
    .await;
    let a = &got["attack"];
    // RefreshCombatStats: the caption is damage plus the spell round, in one number
    let total = a["damage"].as_i64().unwrap() + a["spell_value"].as_i64().unwrap();
    assert_eq!(
        a["caption"].as_str().unwrap().split(' ').next().unwrap(),
        total.to_string()
    );
    assert!(a["cast_level"].is_i64());
    // the heal rounds box is never below one and the cost is per round
    assert!(got["heal"]["rounds"].as_i64().unwrap() >= 1);
    assert!(got["error_text"].is_null(), "{got}");
}

#[tokio::test]
async fn choice_without_its_spell_art_or_damage_is_one_shot() {
    // PopUpChooseCombatGUI: each of these sets the attack to one-shot (0)
    for over in [
        json!({"attack_type": 2}),
        json!({"attack_type": 3, "spell_lvl": 9}),
        json!({"attack_type": 4, "martial_art": 0}),
        json!({"attack_type": 5}),
    ] {
        let got = preview(attack(over.clone())).await;
        assert_eq!(got["applied"]["attack_type"], 0, "{over}");
    }
    // a one-shot does not write the backstab flag; the saved one stays
    let got = preview(attack(json!({"attack_type": 0, "backstab": true}))).await;
    assert_eq!(got["applied"]["backstab"], false);
}

#[tokio::test]
async fn bash_smash_backstab_and_heal_clamps_are_kept() {
    // PopUpChooseCombatGUI: bash/smash stay, a real attack writes the backstab choice, heal rounds
    // clamp to 50 and the heal level to 9999 once a heal spell is picked
    let got = preview(attack(json!({
        "attack_type": 7, "backstab": true, "backstab_weapon": -1,
        "heal_type": 3, "heal_spell_num": 123, "heal_spell_lvl": 9999, "heal_rounds": 99
    })))
    .await;
    let ap = &got["applied"];
    assert_eq!(ap["attack_type"], 7);
    assert_eq!(ap["backstab"], true);
    assert_eq!(ap["backstab_weapon"], -1);
    assert_eq!(ap["heal_rounds"], 50);
    assert_eq!(ap["heal_spell_lvl"], 9999);
    // a heal spell radio with no spell is "infinite"
    let got = preview(attack(json!({"heal_type": 2}))).await;
    assert_eq!(got["applied"]["heal_type"], 0);
    assert_eq!(got["heal"]["caption"], "99999 heal/rnd");
}

#[tokio::test]
async fn bad_drafts_are_400_and_no_class_is_422() {
    let e = env();
    let (req, _) = request(&e.fx, CASE);
    let body = json!({"character": req, "draft": attack(json!({"attack_type": 9}))});
    let (s, _) = call(&e.app, "POST", "/api/character/combat-preview", Some(&body)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let mut none = req.clone();
    none["class"] = Value::Null;
    let (s, _) = call(
        &e.app,
        "POST",
        "/api/character/combat-options",
        Some(&json!({"character": none})),
    )
    .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn options_follow_the_learned_list_and_the_class() {
    let e = env();
    // the spells of the fixture world, split the way RefreshSpells does (8 is damage, 18 a heal)
    let spells = e.fx["header"]["data"]["spells"].as_array().unwrap();
    let with = |abil: &[i64]| -> Vec<i64> {
        spells
            .iter()
            .filter(|r| {
                r[20]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| abil.contains(&p[0].as_i64().unwrap()))
            })
            .map(|r| r[0].as_i64().unwrap())
            .collect()
    };
    let dmg = with(&[1, 8, 17]);
    let heal = with(&[18]);
    assert!(!dmg.is_empty() && !heal.is_empty());
    let (mut req, _) = request(&e.fx, CASE);
    req["learned_spells"] = json!([dmg[0], heal[0]]);
    let (s, got) = call(
        &e.app,
        "POST",
        "/api/character/combat-options",
        Some(&json!({"character": req})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{got}");
    let nums = |k: &str| -> Vec<i64> {
        got[k]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["number"].as_i64().unwrap())
            .collect()
    };
    assert!(nums("attack_learned").contains(&dmg[0]));
    assert!(nums("heal_learned").contains(&heal[0]));
    assert!(!nums("attack_learned").contains(&heal[0]) || dmg.contains(&heal[0]));
    for c in got["attack_any"].as_array().unwrap() {
        assert!(c["label"].as_str().unwrap().contains(" - LVL "));
    }
    assert_eq!(got["nmr_ver"], 1.83);
    assert!(got["current_weapon"].is_string());
}

#[tokio::test]
async fn the_chosen_attack_reaches_the_evaluation() {
    // the evaluate response names the attack for the Char tab's button
    let e = env();
    let (mut req, _) = request(&e.fx, CASE);
    req["attack"] = attack(json!({"attack_type": 5, "manual_p": 150}));
    let (s, got) = call(&e.app, "POST", "/api/character/evaluate", Some(&req)).await;
    assert_eq!(s, StatusCode::OK, "{got}");
    // GetCurrentAttackName: physical manual damage only
    assert_eq!(got["attack_name"], "150 phys");
    // and the choice round-trips through the character store
    let (s, _) = call(&e.app, "PUT", "/api/characters/Zed", Some(&req)).await;
    assert_eq!(s, StatusCode::OK);
    let (_, back) = call(&e.app, "GET", "/api/characters/Zed", None).await;
    assert_eq!(back["attack"], req["attack"]);
}
