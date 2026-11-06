//! The monster damage actions (`/api/monster-damage/*`) and what the Monsters list and pane do with the stored arrays.
//! Card P4-30.
//!
//! Expected values: fixtures/rules/monster_damage_calc.json, produced by the oracle running MME's own
//! `CalculateMonsterDamageVsCharALL` and `ClearMonsterDamageVsCharALL` (the progress-bar form shimmed) for a character and
//! for a party over a small melee-only Monsters table. The World is the pipeline fixture's rows plus the scenario's
//! `table` monsters.

use axum::http::StatusCode;
use serde_json::{Value, json};
use touchstone::AppState;
use touchstone::monster_damage::Snapshot;
use touchstone::state::{Loaded, SourceInfo};
use touchstone_model::{
    AbilitySlot, DropSlot, MidSpell, Monster, MonsterAttack, World, WorldParts,
};

use crate::monsters::{base, blank_attack, call, case, character, f, fixture, i, info, s};

use std::sync::Arc;

fn world(fx: &Value) -> World {
    let b = base();
    let table = &fx["header"]["data"]["table"];
    let monsters = table["monsters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut attacks: [MonsterAttack; 5] = std::array::from_fn(|_| blank_attack());
            for (k, a) in r[4].as_array().unwrap().iter().enumerate() {
                attacks[k] = MonsterAttack {
                    name: s(&a[0]),
                    att_type: i(&a[1]),
                    acc: i(&a[2]),
                    pct: i(&a[3]),
                    true_pct: f(&a[4]),
                    min: i(&a[5]),
                    max: i(&a[6]),
                    energy: i(&a[7]),
                    hit_spell: i(&a[8]),
                };
            }
            Monster {
                number: i(&r[0]),
                name: Some(s(&r[10])),
                weapon: i(&r[3]),
                armour_class: i(&r[7]),
                damage_resist: i(&r[8]),
                follow_pct: 0,
                magic_res: i(&r[9]),
                bs_defense: 0,
                exp: 100.0,
                exp_multi: 0.0,
                hp: i(&r[6]),
                energy: i(&r[2]),
                avg_dmg: f(&r[5]),
                avg_dmg_phys: 0.0,
                avg_dmg_spell: 0.0,
                greet_txt: 0,
                hp_regen: 0,
                charm_lvl: 0,
                r#type: 0,
                undead: 0,
                align: i(&r[1]),
                regen_time: 0,
                game_limit: 0,
                avg_lair_exp: 0.0,
                script_value: 0.0,
                r: 0,
                p: 0,
                g: 0,
                s: 0,
                c: 0,
                death_spell: 0,
                create_spell: 0,
                attacks,
                mid_spells: <[MidSpell; 5]>::default(),
                drops: <[DropSlot; 10]>::default(),
                abilities: [AbilitySlot::default(); 10],
                in_game: true,
                summoned_by: None,
            }
        })
        .collect();
    World::new(WorldParts {
        info: info(),
        items: b.items,
        monsters,
        spells: b.spells,
        rooms: vec![],
        shops: vec![],
        classes: b.classes,
        races: b.races,
        lairs: vec![],
        tbinfo: vec![],
    })
    .unwrap()
}

/// The app over `world`, with `rounds` as the settings' simulation rounds (`nGlobalMonsterSimRounds`).
fn app(world: World, rounds: i64) -> (axum::Router, Arc<AppState>) {
    let source = SourceInfo {
        kind: "mdb",
        path: "test".into(),
    };
    let loaded = Loaded::build(world, source, None, 1).unwrap();
    let dir = tempfile::TempDir::new().unwrap().keep();
    std::fs::write(
        dir.join("settings.json"),
        json!({"monster_sim_rounds": rounds}).to_string(),
    )
    .unwrap();
    let state = AppState::with_data_dir(loaded, dir);
    (touchstone::app(state.clone()), state)
}

fn ctl<'a>(fx: &'a Value, c: &'a Value, name: &str) -> &'a Value {
    let v = &c["inputs"]["forms"]["frmMain"][name];
    if v.is_null() {
        &fx["header"]["setup"]["forms"]["frmMain"][name]
    } else {
        v
    }
}

fn party_boxes(fx: &Value, c: &Value) -> Value {
    let t = |k: usize| ctl(fx, c, "txtMonsterLairFilter")["$array"][k.to_string()]["Text"].clone();
    let mut p = json!({});
    for (k, name) in [
        "size",
        "ac",
        "dr",
        "mr",
        "dodge",
        "hp",
        "anti_magic",
        "rest_hp",
        "accuracy",
        "swings",
    ]
    .iter()
    .enumerate()
    {
        if !t(k).is_null() {
            p[name] = t(k);
        }
    }
    p
}

fn rounds(fx: &Value, c: &Value) -> i64 {
    let v = &c["inputs"]["globals"]["nGlobalMonsterSimRounds"];
    let v = if v.is_null() {
        &fx["header"]["setup"]["globals"]["nGlobalMonsterSimRounds"]
    } else {
        v
    };
    v.as_i64().unwrap()
}

fn scaled(v: &Value) -> i64 {
    v["cur"]
        .as_i64()
        .unwrap_or_else(|| panic!("not a currency: {v}"))
}

fn array(v: &Value) -> Vec<i64> {
    v.as_array().unwrap().iter().map(scaled).collect()
}

fn mine(a: &[touchstone_vb6::Currency]) -> Vec<i64> {
    a.iter().map(|c| c.scaled()).collect()
}

fn snapshot(state: &AppState) -> Snapshot {
    let loaded = state.current();
    state
        .monster_damage()
        .snapshot(loaded.info.load_count, &loaded.world)
}

/// One oracle case, replayed through the endpoints. The array a calculation filled first is pinned to the oracle's
/// (every later calculation continues MME's one generator, where the server starts each one from the initial state, so
/// those are only checked to be filled in); the config text and the flags are pinned after every sequence.
async fn replay(id: &str) {
    let fx = fixture("monster_damage_calc");
    let c = case(&fx, id);
    let (router, state) = app(world(&fx), rounds(&fx, c));
    let wire = character(&fx, c);
    let mut first: Option<&str> = None;
    for step in c["inputs"]["steps"].as_array().unwrap() {
        let party = step["args"][0] == true;
        let mode = if party { "party" } else { "char" };
        match step["call"].as_str().unwrap() {
            "CalculateMonsterDamageVsCharALL" => {
                let body = json!({"character": wire, "mode": mode, "party": party_boxes(&fx, c)});
                let (st, got) = call(&router, "/api/monster-damage/calculate", Some(&body)).await;
                assert_eq!(st, StatusCode::OK, "{id}: {got}");
                first.get_or_insert(mode);
            }
            "ClearMonsterDamageVsCharALL" => {
                let (st, got) = call(
                    &router,
                    "/api/monster-damage/clear",
                    Some(&json!({"mode": mode})),
                )
                .await;
                assert_eq!(st, StatusCode::OK, "{id}: {got}");
            }
            _ => {}
        }
    }
    let got = snapshot(&state);
    let o = &c["outputs"];
    for (name, mine_, want, which) in [
        ("char", mine(&got.vs_char), array(&o["chr"]), "char"),
        ("party", mine(&got.vs_party), array(&o["party"]), "party"),
    ] {
        assert_eq!(mine_.len(), want.len(), "{id}: {name} array length");
        if want.iter().all(|v| *v == -10_000) {
            assert!(
                mine_.iter().all(|v| *v == -10_000),
                "{id}: {name} should be cleared"
            );
        } else if first == Some(which) {
            assert_eq!(mine_, want, "{id}: {name} damage");
        } else {
            assert!(
                mine_.iter().any(|v| *v >= 0),
                "{id}: {name} should be calculated"
            );
        }
    }
    assert_eq!(
        got.char_config,
        s(&o["config"]),
        "{id}: sMonsterDamageVsCharDefenseConfig"
    );
    assert_eq!(
        got.char_asked.is_some(),
        o["dont_char"] == true,
        "{id}: bDontPromptCalcCharMonsterDamage"
    );
    assert_eq!(
        got.party_calculated,
        o["party_calc"] == true,
        "{id}: bMonsterDamageVsPartyCalculated"
    );
    assert_eq!(
        got.party_asked.is_some(),
        o["dont_party"] == true,
        "{id}: bDontPromptCalcPartyMonsterDamage"
    );
}

// Expected: fixtures/rules/monster_damage_calc.json cases char-* (CalculateMonsterDamageVsCharALL for a warrior, a mage and a
// mystic, with 100, 250 and 30 simulation rounds and an evil alignment): the damage array, the stored defense description
// and the flags.
#[tokio::test]
async fn calculate_for_the_character_matches_the_oracle() {
    for id in [
        "char-warrior",
        "char-mage",
        "char-mystic",
        "char-warrior-r250",
        "char-warrior-r30",
        "char-warrior-evil",
    ] {
        replay(id).await;
    }
}

// Expected: fixtures/rules/monster_damage_calc.json cases party-* (CalculateMonsterDamageVsCharALL(True): a solo party, plain,
// mixed, all anti-magic and one anti-magic member, and a mage's party with other boxes).
#[tokio::test]
async fn calculate_for_the_party_matches_the_oracle() {
    for id in [
        "party-solo",
        "party-plain",
        "party-mixed",
        "party-allanti",
        "party-one-anti",
        "party-ac",
    ] {
        replay(id).await;
    }
}

// Expected: fixtures/rules/monster_damage_calc.json cases both-*, clear-*: the two calculations in either order, and
// ClearMonsterDamageVsCharALL for both arrays, one of them, or when nothing was calculated.
#[tokio::test]
async fn calculating_twice_and_clearing_follow_the_oracle() {
    for id in [
        "both-char-party",
        "both-party-char",
        "clear-both",
        "clear-char-only",
        "clear-party-only",
        "clear-fresh",
    ] {
        replay(id).await;
    }
}

// Expected: the same fixture case as above. MME's "Calc. Monster Dmg vs Party" (mnuOptionsItems_Click 3 ->
// CalculateMonsterPartyDamage) is enabled by the lair filter's party size and reads no character, so it runs with none
// loaded: a request with no class and race gives the arrays the oracle pinned for the case's character.
#[tokio::test]
async fn party_damage_runs_without_a_character() {
    let fx = fixture("monster_damage_calc");
    let c = case(&fx, "party-plain");
    let want = array(&c["outputs"]["party"]);
    let (router, state) = app(world(&fx), rounds(&fx, c));
    let mut blank = character(&fx, c);
    blank["class"] = Value::Null;
    blank["race"] = Value::Null;
    let body = json!({"character": blank, "mode": "party", "party": party_boxes(&fx, c)});
    let (st, got) = call(&router, "/api/monster-damage/calculate", Some(&body)).await;
    assert_eq!(st, StatusCode::OK, "{got}");
    let snap = snapshot(&state);
    assert!(snap.party_calculated);
    let got = mine(&snap.vs_party);
    for (n, (g, w)) in got.iter().zip(&want).enumerate() {
        // the first calculation of a session is the one the oracle pinned
        assert_eq!(g, w, "monster {n}");
    }
    // the character calculation still needs a class and race
    let body = json!({"character": blank, "mode": "char"});
    let (st, _) = call(&router, "/api/monster-damage/calculate", Some(&body)).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn status_reports_what_is_calculated() {
    let fx = fixture("monster_damage_calc");
    let c = case(&fx, "char-warrior");
    let (router, _) = app(world(&fx), 100);
    let (st, got) = call(&router, "/api/monster-damage/status", None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(got["char"]["calculated"], false);
    assert_eq!(got["party"]["calculated"], false);
    assert!(got["running"].is_null());
    let body = json!({"character": character(&fx, c), "mode": "char"});
    let (st, done) = call(&router, "/api/monster-damage/calculate", Some(&body)).await;
    assert_eq!(st, StatusCode::OK, "{done}");
    assert_eq!(done["monsters"], 17);
    let (_, got) = call(&router, "/api/monster-damage/status", None).await;
    assert_eq!(got["char"]["calculated"], true);
    assert_eq!(got["party"]["calculated"], false);
    let (st, got) = call(&router, "/api/monster-damage/clear", Some(&json!({}))).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(got["char"]["calculated"], false);
}

#[tokio::test]
async fn bad_requests_are_rejected() {
    let fx = fixture("monster_damage_calc");
    let c = case(&fx, "char-warrior");
    let (router, _) = app(world(&fx), 100);
    for (uri, body) in [
        ("/api/monster-damage/calculate", json!({"mode": "char"})),
        (
            "/api/monster-damage/calculate",
            json!({"character": character(&fx, c), "mode": "everyone"}),
        ),
        ("/api/monster-damage/clear", json!({"mode": "x"})),
        ("/api/monster-damage/dismiss", json!({"mode": "char"})),
    ] {
        let (st, got) = call(&router, uri, Some(&body)).await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "{uri} {body}: {got}");
    }
}

// MME asks "Calculate Monster Damage vs Character Defenses first?" when a lair-mode list is made for a character whose
// defenses differ from the stored ones, and not again once it was answered (or the damage calculated) for them.
#[tokio::test]
async fn lair_list_asks_until_the_damage_is_calculated_or_dismissed() {
    let fx = fixture("monster_damage_calc");
    let c = case(&fx, "char-warrior");
    let (router, _) = app(world(&fx), 100);
    let wire = character(&fx, c);
    let list = json!({
        "character": wire,
        "global_filter": {"use_character": true},
        "options": {"monsters": {"lair": true, "ask_damage": true, "party": {"size": 1}}},
    });
    let (st, first) = call(&router, "/api/evaluate/monsters", Some(&list)).await;
    assert_eq!(st, StatusCode::OK, "{first}");
    assert_eq!(first["prompt"]["dialog"], "calc_char");
    assert_eq!(first["prompt"]["mode"], "char");
    assert_eq!(first["rows"].as_array().unwrap().len(), 0);
    let key = first["prompt"]["key"].clone();
    assert!(key.as_str().unwrap().contains('_'));

    // No: asked once, and the list is made from the damage as it stands
    let (st, _) = call(
        &router,
        "/api/monster-damage/dismiss",
        Some(&json!({"mode": "char", "key": key})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (_, again) = call(&router, "/api/evaluate/monsters", Some(&list)).await;
    assert!(again["prompt"].is_null(), "{again}");
    assert!(!again["rows"].as_array().unwrap().is_empty());

    // a different character (other defenses) is asked about again
    let mut other = list.clone();
    other["character"]["level"] = json!(45);
    let (_, asked) = call(&router, "/api/evaluate/monsters", Some(&other)).await;
    assert!(
        asked["prompt"]["dialog"].is_string(),
        "defenses changed: {asked}"
    );

    // Clear forgets the answer; a calculation for the character ends the questions and fills the Dmg column
    let (_, before) = call(&router, "/api/evaluate/monsters", Some(&list)).await;
    call(&router, "/api/monster-damage/clear", Some(&json!({}))).await;
    let (_, asked) = call(&router, "/api/evaluate/monsters", Some(&list)).await;
    assert_eq!(asked["prompt"]["dialog"], "calc_char");
    let body = json!({"character": wire, "mode": "char"});
    call(&router, "/api/monster-damage/calculate", Some(&body)).await;
    let (_, after) = call(&router, "/api/evaluate/monsters", Some(&list)).await;
    assert!(after["prompt"].is_null(), "{after}");
    assert_ne!(
        before["rows"], after["rows"],
        "the list reads the calculated damage"
    );

    // a recalculation after the character changed says RE-Calculate
    let (_, re) = call(&router, "/api/evaluate/monsters", Some(&other)).await;
    assert_eq!(re["prompt"]["dialog"], "recalc_char", "{re}");

    // the party asks on its own
    let mut party = list.clone();
    party["options"]["monsters"]["party"] = json!({"size": 3});
    let (_, p) = call(&router, "/api/evaluate/monsters", Some(&party)).await;
    assert_eq!(p["prompt"]["dialog"], "calc_party", "{p}");
    assert_eq!(p["prompt"]["mode"], "party");
    // without ask_damage no question is asked at all
    party["options"]["monsters"]["ask_damage"] = json!(false);
    let (_, quiet) = call(&router, "/api/evaluate/monsters", Some(&party)).await;
    assert!(quiet["prompt"].is_null());
}

// A second calculation while one runs is refused, and the status shows the progress of the running one.
#[tokio::test]
async fn a_reload_forgets_the_damage() {
    let fx = fixture("monster_damage_calc");
    let c = case(&fx, "char-warrior");
    let (router, state) = app(world(&fx), 100);
    let body = json!({"character": character(&fx, c), "mode": "char"});
    call(&router, "/api/monster-damage/calculate", Some(&body)).await;
    assert!(snapshot(&state).vs_char.iter().any(|v| v.scaled() >= 0));
    // a new load (another load number) starts from nothing calculated
    let source = SourceInfo {
        kind: "mdb",
        path: "test".into(),
    };
    state.swap(Loaded::build(world(&fx), source, None, 2).unwrap());
    let after = snapshot(&state);
    assert!(after.vs_char.iter().all(|v| v.scaled() == -10_000));
    assert!(after.char_config.is_empty());
}

#[tokio::test]
async fn cancel_stops_between_monsters_and_keeps_the_previous_arrays() {
    let fx = fixture("monster_damage_calc");
    let c = case(&fx, "char-warrior");
    // a long enough simulation per monster that the cancel always lands in the middle of the run
    let (router, state) = app(world(&fx), 2_000_000);
    // nothing running: cancel is a plain status
    let (st, got) = call(&router, "/api/monster-damage/cancel", Some(&json!({}))).await;
    assert_eq!(st, StatusCode::OK, "{got}");
    assert!(got["running"].is_null());
    let before = snapshot(&state);

    let body = json!({"character": character(&fx, c), "mode": "char"});
    // the cancel has to land while the run is going; a run on a slow machine may be over first, so try again
    let mut cancelled = false;
    for _ in 0..5 {
        let run = {
            let router = router.clone();
            let body = body.clone();
            tokio::spawn(async move {
                call(&router, "/api/monster-damage/calculate", Some(&body)).await
            })
        };
        // wait for the first monster to finish, then cancel
        for _ in 0..2000 {
            let (_, s) = call(&router, "/api/monster-damage/status", None).await;
            if s["running"]["done"].as_u64().unwrap_or(0) >= 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        let (st, got) = call(&router, "/api/monster-damage/cancel", Some(&json!({}))).await;
        assert_eq!(st, StatusCode::OK, "{got}");
        let (st, done) = run.await.unwrap();
        assert_eq!(st, StatusCode::OK, "{done}");
        if done["cancelled"] == true {
            cancelled = true;
            break;
        }
        state
            .monster_damage()
            .clear(1, &state.current().world, None);
    }
    assert!(cancelled, "no run was cancelled");

    // the arrays and the flags are as they were
    let after = snapshot(&state);
    assert_eq!(mine(&after.vs_char), mine(&before.vs_char));
    assert_eq!(after.char_config, before.char_config);
    assert_eq!(after.char_asked, before.char_asked);
    let (_, s) = call(&router, "/api/monster-damage/status", None).await;
    assert_eq!(s["char"]["calculated"], false);
    assert!(s["running"].is_null());

    // and the next run is not cancelled
    let (st, done) = call(&router, "/api/monster-damage/calculate", Some(&body)).await;
    assert_eq!(st, StatusCode::OK, "{done}");
    assert_eq!(done["cancelled"], false, "{done}");
    let (_, s) = call(&router, "/api/monster-damage/status", None).await;
    assert_eq!(s["char"]["calculated"], true);
}
