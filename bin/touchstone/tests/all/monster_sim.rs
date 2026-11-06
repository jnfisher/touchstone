//! `/api/calc/monster-sim` over the monsters of fixtures/scenarios/monster_glue_char.json.
//!
//! `cmdRunSim_Click` and `CalculateMonsterDamageVsChar` set the simulator up the same way for a
//! solo character (caps from the class, a box above 0 replaces the default, populate, run), so the
//! fixture's `monster_glue_char` average damage and generator state are the endpoint's.

use std::path::PathBuf;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use touchstone::state::{Loaded, SourceInfo};
use touchstone::{AppState, app};
use touchstone_model::{
    AbilitySlot, Class, Info, Item, Monster, MonsterAttack, Spell, World, WorldParts,
};
use tower::ServiceExt;

fn i(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}

fn pairs(flat: &Value) -> Vec<AbilitySlot> {
    flat.as_array()
        .unwrap()
        .chunks(2)
        .map(|p| AbilitySlot {
            ability: i(&p[0]),
            value: p.get(1).map_or(0, i),
        })
        .collect()
}

fn class(number: i32) -> Class {
    Class {
        number,
        name: format!("class {number}"),
        min_hits: 0,
        max_hits: 0,
        exp_table: 0,
        magery_type: 0,
        magery_lvl: 0,
        weapon_type: 0,
        armour_type: 0,
        combat_lvl: 0,
        abilities: [AbilitySlot::default(); 10],
    }
}

fn read(path: &str) -> Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(path);
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// The World of the scenario's "table" key: monsters, the spells they cast and the items they use.
fn world() -> World {
    let doc = read("scenarios/monster_glue_char.json");
    let t = &doc["table"];
    let n = |r: &Value, k: usize| r.get(k).map_or(0, i);
    let monsters = t["monsters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut attacks: [MonsterAttack; 5] = std::array::from_fn(|_| MonsterAttack {
                name: String::new(),
                att_type: 0,
                acc: 0,
                pct: 0,
                true_pct: 0.0,
                min: 0,
                max: 0,
                energy: 0,
                hit_spell: 0,
            });
            for (k, a) in r[4].as_array().unwrap().iter().enumerate() {
                attacks[k] = MonsterAttack {
                    name: a.get(0).and_then(Value::as_str).unwrap_or("").to_string(),
                    att_type: n(a, 1),
                    acc: n(a, 2),
                    pct: n(a, 3),
                    true_pct: a.get(4).and_then(Value::as_f64).unwrap_or(0.0),
                    min: n(a, 5),
                    max: n(a, 6),
                    energy: n(a, 7),
                    hit_spell: n(a, 8),
                };
            }
            let mut mid = [Default::default(); 5];
            for (k, a) in r[5].as_array().unwrap().iter().enumerate() {
                mid[k] = touchstone_model::MidSpell {
                    spell: n(a, 0),
                    pct: n(a, 1),
                    lvl: n(a, 2),
                };
            }
            let mut drops = [Default::default(); 10];
            for (k, d) in r[6].as_array().unwrap().iter().enumerate() {
                drops[k] = touchstone_model::DropSlot {
                    item: i(&d[0]),
                    pct: i(&d[1]),
                };
            }
            Monster {
                number: i(&r[0]),
                name: Some(format!("monster {}", i(&r[0]))),
                weapon: i(&r[3]),
                armour_class: 0,
                damage_resist: 0,
                follow_pct: 0,
                magic_res: 0,
                bs_defense: 0,
                exp: 0.0,
                exp_multi: 0.0,
                hp: 0,
                energy: i(&r[2]),
                avg_dmg: 0.0,
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
                death_spell: i(&r[7]),
                create_spell: 0,
                attacks,
                mid_spells: mid,
                drops,
                abilities: [AbilitySlot::default(); 10],
                in_game: true,
                summoned_by: None,
            }
        })
        .collect();
    let spells = t["spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut abilities = [AbilitySlot::default(); 10];
            for (k, a) in pairs(&r[17]).into_iter().enumerate() {
                abilities[k] = a;
            }
            Spell {
                number: i(&r[0]),
                name: Some(r[1].as_str().unwrap().to_string()),
                short: None,
                req_level: i(&r[14]),
                energy_cost: i(&r[16]),
                mana_cost: 0,
                min_base: i(&r[8]),
                max_base: i(&r[9]),
                diff: 0,
                type_of_resists: i(&r[3]),
                targets: i(&r[4]),
                dur: i(&r[5]),
                att_type: i(&r[2]),
                magery: 0,
                magery_lvl: 0,
                cap: i(&r[15]),
                max_inc_lvls: i(&r[13]),
                max_inc: i(&r[12]),
                min_inc_lvls: i(&r[11]),
                min_inc: i(&r[10]),
                dur_inc_lvls: i(&r[7]),
                dur_inc: i(&r[6]),
                abilities,
                learnable: false,
                learned_from: None,
                casted_by: None,
                classes: None,
            }
        })
        .collect();
    let items = t["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut abilities = [AbilitySlot::default(); 20];
            for (k, a) in pairs(&r[2]).into_iter().enumerate() {
                abilities[k] = a;
            }
            Item {
                number: i(&r[0]),
                name: String::new(),
                limit: i(&r[1]),
                encum: 0,
                item_type: 0,
                use_count: 0,
                price: 0,
                currency: 0,
                min: 0,
                max: 0,
                armour_class: 0,
                damage_resist: 0,
                weapon_type: 0,
                armour_type: 0,
                worn: 0,
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
                abilities,
                in_game: true,
                obtained_from: None,
                references: None,
            }
        })
        .collect();
    World::new(WorldParts {
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
        items,
        monsters,
        spells,
        rooms: vec![],
        shops: vec![],
        classes: (1..=14).map(class).collect(),
        races: vec![],
        lairs: vec![],
        tbinfo: vec![],
    })
    .unwrap()
}

async fn post(uri: &str, body: Value) -> (StatusCode, Value) {
    let info = SourceInfo {
        kind: "test",
        path: String::new(),
    };
    let router = app(AppState::new(
        Loaded::build(world(), info, None, 1).unwrap(),
    ));
    let res = router
        .oneshot(
            Request::post(uri)
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

async fn sim(body: Value) -> Value {
    let (st, v) = post("/api/calc/monster-sim", body.clone()).await;
    assert_eq!(st, StatusCode::OK, "{body}: {v}");
    v
}

/// The typed defender a monster_glue_char case's frmMain controls amount to.
fn defender_of(forms: &Value, setup: &Value) -> Option<Value> {
    let ctl = |name: &str| {
        let v = &forms[name];
        if v.is_null() { &setup[name] } else { v }
    };
    let stat = |k: usize, f: &str| ctl("lblInvenCharStat")["$array"][k.to_string()][f].clone();
    let text = |v: Value| v.as_str().map(str::to_owned);
    let class_ctl = &ctl("cmbGlobalClass")["$array"]["0"];
    let class = if class_ctl["ListIndex"].as_i64().unwrap_or(-1) > 0 {
        i(&class_ctl["ItemData"]["1"])
    } else {
        0
    };
    // An empty resist tag stops MME's setup with error 13; the typed boxes have no such case.
    let tags: Vec<String> = [28, 27, 25, 29, 26]
        .iter()
        .map(|&k| text(stat(k, "Tag")))
        .collect::<Option<_>>()?;
    if tags.iter().any(String::is_empty) {
        return None;
    }
    Some(json!({
        "class": class,
        "ac": text(stat(2, "Caption"))?,
        "dr": text(stat(3, "Caption"))?,
        "mr": text(ctl("txtCharMR")["Text"].clone())?,
        "dodge": text(ctl("lblCharDodge")["Tag"].clone())?,
        "anti_magic": ctl("chkCharAntiMagic")["Value"].as_i64() == Some(1),
        "prot_evil": text(stat(20, "Tag"))?,
        "resist_cold": tags[0],
        "resist_fire": tags[1],
        "resist_stone": tags[2],
        "resist_lightning": tags[3],
        "resist_water": tags[4],
    }))
}

// Expected values: fixtures/rules/monster_glue_char.json (MME's own CalculateMonsterDamageVsChar
// for one solo monster: its average damage and the generator state after the run).
#[tokio::test]
async fn solo_runs_match_the_glue_char_fixture() {
    let fx = read("rules/monster_glue_char.json");
    let setup = &fx["header"]["setup"];
    let setup_rounds = setup["globals"]["nGlobalMonsterSimRounds"]
        .as_i64()
        .unwrap();
    let mut checked = 0;
    for case in fx["cases"].as_array().unwrap() {
        let inp = &case["inputs"];
        let steps = inp["steps"].as_array().cloned().unwrap_or_default();
        if steps.len() != 1 || steps[0]["call"] != "CalculateMonsterDamageVsChar" {
            continue;
        }
        let args = steps[0]["args"].as_array().unwrap();
        if args.len() != 1 || args[0].as_i64().unwrap() <= 0 {
            continue;
        }
        let Some(defender) = defender_of(&inp["forms"]["frmMain"], &setup["forms"]["frmMain"])
        else {
            continue;
        };
        let rounds = inp["globals"]["nGlobalMonsterSimRounds"]
            .as_i64()
            .unwrap_or(setup_rounds)
            .clamp(100, 10_000);
        let seed = inp["rnd"].as_u64().unwrap_or(327_680);
        let key = steps[0]["as"].as_str().unwrap();
        let want = case["outputs"][key]["cur"].as_i64().unwrap() as f64 / 10_000.0;

        let v = sim(json!({
            "monster": args[0], "rounds": rounds, "seed": seed, "defender": defender
        }))
        .await;
        let id = &case["id"];
        let got = v["summary"]["average_damage"].as_f64().unwrap_or(0.0);
        assert_eq!(got, want, "{id} average damage");
        assert_eq!(
            v["rng_after"].as_i64(),
            case["outputs"]["rnd"].as_i64(),
            "{id} generator state"
        );
        checked += 1;
    }
    assert!(checked >= 25, "only {checked} cases were usable");
}

#[tokio::test]
async fn the_default_seed_is_the_unseeded_vb6_generator() {
    let body = json!({"monster": 13, "rounds": 100, "defender": {"ac": "20", "mr": "60"}});
    let a = sim(body.clone()).await;
    let mut explicit = body;
    explicit["seed"] = json!(327_680);
    let b = sim(explicit).await;
    assert_eq!(a["seed"], 327_680);
    assert_eq!(a, b);
}

#[tokio::test]
async fn a_character_fills_the_boxes_like_the_reset_button() {
    // cmdResetUserDefs_Click(1): rounded numbers, tags as text. The defender endpoint needs a
    // class and race the test world lacks, so it answers 422 like the evaluate endpoint.
    let (st, _) = post("/api/calc/monster-sim/defender", json!({"character": {}})).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn results_follow_the_form() {
    // Monster 13 hits twice a round with melee, so the table has two rows with numbers.
    let v = sim(json!({"monster": 13, "rounds": "200", "defender": {"mr": 0}})).await;
    assert_eq!(v["ran"], true);
    assert_eq!(v["rounds"], 200);
    let rows = v["attacks"].as_array().unwrap();
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0]["name"], "slashes you");
    assert!(rows[0]["stats"]["success_pct"].as_f64().unwrap() > 0.0);
    assert!(rows[2]["stats"].is_null());
    let s = &v["summary"];
    assert!(
        s["avg_caption"]
            .as_str()
            .unwrap()
            .starts_with("AVG Dmg/Rnd: ")
    );
    assert!(s["max_caption"].as_str().unwrap().starts_with("Max/Seen: "));
    // The log is trimmed and holds at most 100 rounds by default.
    let log = v["combat_log"].as_str().unwrap();
    assert!(!log.is_empty() && !log.starts_with(' ') && log.contains("\r\n"));
    let sep = "Damage for round:";
    assert_eq!(log.matches(sep).count(), 100);
}

#[tokio::test]
async fn log_rounds_and_zero_rounds() {
    let v = sim(json!({"monster": 13, "rounds": 50, "log_rounds": 3})).await;
    let sep = "Damage for round:";
    assert_eq!(v["combat_log"].as_str().unwrap().matches(sep).count(), 3);
    // Rounds at 0: the labels are set and nothing runs.
    let v = sim(json!({"monster": 13, "rounds": 0})).await;
    assert_eq!(v["ran"], false);
    assert!(v["summary"].is_null());
    assert_eq!(v["attacks"][0]["name"], "slashes you");
    assert_eq!(v["rng_after"], 327_680);
}

#[tokio::test]
async fn rounds_are_cut_at_the_forms_limit() {
    // cmdRunSim_Click: Val(rounds) above 500000 becomes 500000. Dynamic rounds settle sooner.
    let v = sim(json!({"monster": 13, "rounds": 9_000_000, "dynamic": true})).await;
    assert!(v["rounds"].as_i64().unwrap() <= 100_000);
    let v = sim(json!({"monster": 1, "rounds": 9_000_000, "log_rounds": 0})).await;
    assert_eq!(v["rounds"], 500_000);
}

#[tokio::test]
async fn bad_requests() {
    for (body, why) in [
        (json!({"monster": 99_999}), "unknown monster"),
        (json!({"monster": 0}), "monster 0"),
        (
            json!({"monster": 13, "defender": {"class": 400}}),
            "unknown class",
        ),
        (json!({"monster": 13, "log_rounds": 5000}), "log too long"),
        (
            json!({"monster": 13, "defender": {"ac": "99999999999"}}),
            "AC past a Long",
        ),
        (json!({"monster": 13, "rounds": [1]}), "rounds not text"),
        (json!({"rounds": 5}), "no monster"),
    ] {
        let (st, v) = post("/api/calc/monster-sim", body).await;
        assert!(
            st == StatusCode::BAD_REQUEST || st == StatusCode::UNPROCESSABLE_ENTITY,
            "{why}: {st} {v}"
        );
    }
}
