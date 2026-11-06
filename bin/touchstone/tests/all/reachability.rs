//! `/api/reachability/*` and the monster pane's reach rows, pinned to fixtures/rules/reachability.json (MME's
//! ReachEnsureComputed on a slice of the stock database, and AddMonsterReachRows). The world is rebuilt from the
//! fixture header.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use touchstone::state::{Loaded, SourceInfo};
use touchstone::{AppState, app};
use touchstone_model::{
    AbilitySlot, Class, DropSlot, Info, Item, MidSpell, Monster, MonsterAttack, Race, Room, Spell,
    TbInfo, World, WorldParts,
};
use tower::ServiceExt;

fn i(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}

fn s(v: &Value) -> String {
    v.as_str().unwrap().to_string()
}

fn fixture() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/rules/reachability.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn case<'a>(fx: &'a Value, id: &str) -> &'a Value {
    fx["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no case {id}"))
}

fn world(data: &Value) -> World {
    let rooms = data["rooms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let ex: Vec<String> = s(&r[7]).split('|').map(str::to_string).collect();
            Room {
                map_number: i(&r[0]),
                room_number: i(&r[1]),
                name: Some(s(&r[2])),
                light: 0,
                shop: 0,
                npc: i(&r[3]),
                cmd: i(&r[4]),
                spell: i(&r[5]),
                lair: None,
                delay: 0,
                placed: Some(s(&r[6])),
                n: ex[0].clone(),
                s: ex[1].clone(),
                e: ex[2].clone(),
                w: ex[3].clone(),
                ne: ex[4].clone(),
                nw: ex[5].clone(),
                se: ex[6].clone(),
                sw: ex[7].clone(),
                u: ex[8].clone(),
                d: ex[9].clone(),
            }
        })
        .collect();
    let tbinfo = data["tbinfo"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| TbInfo {
            number: i(&r[0]),
            link_to: i(&r[1]),
            action: Some(s(&r[2])),
            called_from: s(&r[3]),
        })
        .collect();
    let spells = data["spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut abilities = [AbilitySlot::default(); 10];
            for (k, p) in r[4].as_array().unwrap().chunks(2).enumerate() {
                abilities[k] = AbilitySlot {
                    ability: i(&p[0]),
                    value: p.get(1).map_or(0, i),
                };
            }
            Spell {
                number: i(&r[0]),
                name: Some(String::new()),
                short: Some(String::new()),
                req_level: i(&r[1]),
                energy_cost: 0,
                mana_cost: 0,
                min_base: 0,
                max_base: 0,
                diff: 0,
                type_of_resists: 0,
                targets: 0,
                dur: i(&r[2]),
                att_type: 0,
                magery: 0,
                magery_lvl: 0,
                cap: 0,
                max_inc_lvls: 0,
                max_inc: 0,
                min_inc_lvls: 0,
                min_inc: 0,
                dur_inc_lvls: 0,
                dur_inc: 0,
                abilities,
                learnable: false,
                learned_from: None,
                casted_by: Some(s(&r[3])),
                classes: None,
            }
        })
        .collect();
    let monsters = data["monsters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Monster {
            number: i(&r[0]),
            name: Some(s(&r[1])),
            weapon: 0,
            armour_class: 0,
            damage_resist: 0,
            follow_pct: 0,
            magic_res: 0,
            bs_defense: 0,
            exp: 0.0,
            exp_multi: 1.0,
            hp: 10,
            energy: 1000,
            avg_dmg: 0.0,
            avg_dmg_phys: 0.0,
            avg_dmg_spell: 0.0,
            greet_txt: i(&r[2]),
            hp_regen: 0,
            charm_lvl: 0,
            r#type: 0,
            undead: 0,
            align: 0,
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
            attacks: std::array::from_fn(|_| MonsterAttack {
                name: String::new(),
                att_type: 0,
                acc: 0,
                pct: 0,
                true_pct: 0.0,
                min: 0,
                max: 0,
                energy: 0,
                hit_spell: 0,
            }),
            mid_spells: [MidSpell::default(); 5],
            drops: [DropSlot::default(); 10],
            abilities: [AbilitySlot::default(); 10],
            in_game: i(&r[3]) != 0,
            summoned_by: Some(s(&r[4])),
        })
        .collect();
    let items = data["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Item {
            number: i(&r[0]),
            name: String::new(),
            limit: 0,
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
            abilities: [AbilitySlot::default(); 20],
            in_game: true,
            obtained_from: Some(s(&r[1])),
            references: None,
        })
        .collect();
    World::new(WorldParts {
        info: Info {
            nmr_version: "v1.8.3".to_owned(),
            dat_file_version: String::new(),
            content_version: None,
            date: String::new(),
            time: String::new(),
            custom: String::new(),
            legit: false,
            disable_kai: false,
            update_url: String::new(),
        },
        items,
        monsters,
        spells,
        rooms,
        shops: vec![],
        // the list evaluation needs one class and one race to stand in for "any"
        classes: vec![Class {
            number: 1,
            name: "Warrior".into(),
            min_hits: 8,
            max_hits: 12,
            exp_table: 0,
            magery_type: 0,
            magery_lvl: 0,
            weapon_type: 0,
            armour_type: 0,
            combat_lvl: 0,
            abilities: [AbilitySlot::default(); 10],
        }],
        races: vec![Race {
            number: 1,
            name: "Human".into(),
            m_str: 0,
            m_int: 0,
            m_wil: 0,
            m_agl: 0,
            m_hea: 0,
            m_chm: 0,
            x_str: 0,
            x_int: 0,
            x_wil: 0,
            x_agl: 0,
            x_hea: 0,
            x_chm: 0,
            hp_per_lvl: 0,
            exp_table: 0,
            base_cp: 0,
            abilities: [AbilitySlot::default(); 10],
        }],
        lairs: vec![],
        tbinfo,
    })
    .unwrap()
}

fn router(world: World) -> axum::Router {
    let info = SourceInfo {
        kind: "mdb",
        path: "test".into(),
    };
    let loaded = Loaded::build(world, info, None, 1).unwrap();
    app(AppState::with_data_dir(
        loaded,
        tempfile::TempDir::new().unwrap().keep(),
    ))
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<&Value>,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// The texts of the "Reachable" row and the route rows under it (up to the blank row that ends them).
fn reach_texts_fixture(rows: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut on = false;
    for r in rows.as_array().unwrap() {
        let Some(r) = r.as_array() else {
            on = false; // a bare string is the blank row
            continue;
        };
        if r[0] == "Reachable" {
            on = true;
        }
        if on {
            let c = &r[4][0];
            out.push(c.as_str().map_or_else(|| s(&c[0]), str::to_string));
        }
    }
    out
}

fn reach_texts_pane(pane: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut on = false;
    for l in pane["lines"].as_array().unwrap() {
        if l["label"] == "Reachable" {
            on = true;
        } else if l["label"] == "" && l["cells"].as_array().is_some_and(Vec::is_empty) {
            on = false;
        }
        if on {
            out.push(l["cells"][0]["text"].as_str().unwrap_or("").to_string());
        }
    }
    out
}

#[tokio::test]
async fn compute_status_and_pane_rows_follow_the_fixture() {
    let fx = fixture();
    let app = router(world(&fx["header"]["data"]));

    // before anything is computed
    let (st, v) = call(&app, "GET", "/api/reachability/status", None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["computed"], false);
    let rows_case = case(&fx, "rows_30");
    let keys = rows_case["inputs"]["steps"][0]["rows"]["keys"]
        .as_array()
        .unwrap();
    let body = json!({"global_filter": {"use_character": false, "level": 30}});
    let (st, pane) = call(
        &app,
        "POST",
        &format!("/api/detail/monsters/{}", keys[0]),
        Some(&body),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{pane}");
    let texts = reach_texts_pane(&pane);
    // AddMonsterReachRows when nothing is computed (no ReachIsComputed)
    assert_eq!(texts, vec!["(double-click to compute)".to_string()]);

    // compute: the counts are the fixture's build case
    let (st, v) = call(&app, "POST", "/api/reachability/compute", Some(&json!({}))).await;
    assert_eq!(st, StatusCode::OK);
    let build = &case(&fx, "build")["outputs"];
    assert_eq!(v["computed"], true);
    assert_eq!(v["rooms"], build["rooms"]);
    assert_eq!(v["edges"], build["edges"]);
    assert_eq!(v["random_groups"], build["groups"]);
    assert_eq!(v["level_bands"], build["bands"]);
    let (_, v2) = call(&app, "GET", "/api/reachability/status", None).await;
    assert_eq!(v2["computed"], true);

    // the pane's reach rows for every monster of the fixture's rows_30 case
    let mut checked = 0;
    for (k, mon) in keys.iter().enumerate() {
        let (st, pane) = call(
            &app,
            "POST",
            &format!("/api/detail/monsters/{mon}"),
            Some(&body),
        )
        .await;
        assert_eq!(st, StatusCode::OK, "{pane}");
        let want = reach_texts_fixture(&rows_case["outputs"]["rows"][k]);
        assert_eq!(reach_texts_pane(&pane), want, "monster {mon}");
        checked += usize::from(!want.is_empty());
    }
    assert!(checked > 5, "only {checked} monsters had reach rows");
}

#[tokio::test]
async fn reachable_only_hides_unreachable_monsters() {
    let fx = fixture();
    let app = router(world(&fx["header"]["data"]));
    // ReachIsMonsterOK at level 1 and 120 (cases ok_1 and ok_120)
    for (id, level) in [("ok_1", 1), ("ok_120", 120)] {
        let c = case(&fx, id);
        let each = c["inputs"]["steps"][0]["each"].as_array().unwrap();
        let ok = c["outputs"]["ok"].as_array().unwrap();
        let hidden: Vec<i64> = each
            .iter()
            .zip(ok)
            .filter(|(_, ok)| **ok == false)
            .map(|(a, _)| a[0].as_i64().unwrap())
            .collect();
        let mut body = json!({"global_filter": {"use_character": false, "level": level}, "options": {"monsters": {}}});
        let (st, all) = call(&app, "POST", "/api/evaluate/monsters", Some(&body)).await;
        assert_eq!(st, StatusCode::OK, "{all}");
        body["options"]["monsters"] = json!({"extras": {"enabled": true, "reachable_only": true}});
        let (st, only) = call(&app, "POST", "/api/evaluate/monsters", Some(&body)).await;
        assert_eq!(st, StatusCode::OK, "{only}");
        let numbers = |v: &Value| -> Vec<i64> {
            v["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["number"].as_i64().unwrap())
                .collect()
        };
        let (a, b) = (numbers(&all), numbers(&only));
        for n in &hidden {
            assert!(!b.contains(n), "{id}: monster {n} should be hidden");
        }
        let expect_gone: Vec<i64> = a.iter().copied().filter(|n| hidden.contains(n)).collect();
        assert_eq!(a.len() - b.len(), expect_gone.len(), "{id}");
    }
}

// ---- the route dialog (POST /api/reachability/route) ----

/// ReachRoomRoute and ReachRoute outputs of the fixture, `route.refs[i]` = [source map, source room, source name,
/// step maps, step rooms, step flags, step labels, steps, coin steps, alt steps, alt coin steps, alt source].
fn route_ref(fx: &Value, id: &str, k: usize) -> Value {
    case(fx, id)["outputs"]["route.refs"][k].clone()
}

#[tokio::test]
async fn room_route_dialog_follows_the_fixture_route() {
    let fx = fixture();
    let app = router(world(&fx["header"]["data"]));
    // room_route_30 entry 1: ReachRoomRoute(1, 1403, 30); the route is found (outputs.route[1])
    assert_eq!(case(&fx, "room_route_30")["outputs"]["route"][1], true);
    let want = route_ref(&fx, "room_route_30", 1);
    let steps = want[7].as_i64().unwrap();
    let flags = want[5].as_array().unwrap();
    let labelled = flags.iter().filter(|f| f.as_i64() != Some(0)).count();
    let body = json!({"map": 1, "room": 1403, "level": 30});
    let (st, v) = call(&app, "POST", "/api/reachability/route", Some(&body)).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert!(v["message"].is_null(), "{v}");
    let caption = v["caption"].as_str().unwrap();
    assert!(caption.starts_with("Route to "), "{caption}");
    assert!(caption.contains("(1/1403)"), "{caption}");
    assert!(
        caption.contains(&format!(": {steps} steps from {}", s(&want[2]))),
        "{caption}"
    );
    let rows = v["rows"].as_array().unwrap();
    assert_eq!(rows[0]["step"], 0);
    assert_eq!(rows[0]["map"], want[0]);
    assert_eq!(rows[0]["room"], want[1]);
    let last = rows.last().unwrap();
    assert_eq!(last["step"], steps);
    assert_eq!(last["room"], 1403);
    // a row for every action, listed at the room it is taken from
    let actions = rows
        .iter()
        .filter(|r| r["text"].as_str().unwrap().contains(" -- "))
        .count();
    assert_eq!(actions, labelled);

    // a room that cannot be routed to is MME's message box
    let body = json!({"map": 1, "room": 99999, "level": 30});
    let (_, v) = call(&app, "POST", "/api/reachability/route", Some(&body)).await;
    assert_eq!(v["message"], "No route information for this room.");
    assert_eq!(v["rows"].as_array().unwrap().len(), 0);
    // the start room: "Routes start at this room"
    let body = json!({"map": 1, "room": 297, "level": 30});
    let (_, v) = call(&app, "POST", "/api/reachability/route", Some(&body)).await;
    assert_eq!(
        v["message"],
        "Routes start at this room: Bank of Godfrey (1/297)."
    );
}

#[tokio::test]
async fn monster_route_dialog_follows_the_fixture_route() {
    let fx = fixture();
    let app = router(world(&fx["header"]["data"]));
    // route_30 entry 0: ReachRoute(45, 30), found
    let c = case(&fx, "route_30");
    assert_eq!(c["outputs"]["route"][0], true);
    let monster = c["inputs"]["steps"][0]["each"][0][0].as_i64().unwrap();
    let want = route_ref(&fx, "route_30", 0);
    let steps = want[7].as_i64().unwrap();
    let body = json!({"monster": monster, "level": 30});
    let (st, v) = call(&app, "POST", "/api/reachability/route", Some(&body)).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let caption = v["caption"].as_str().unwrap();
    assert!(caption.starts_with("Route to "), "{caption}");
    assert!(
        caption.contains(&format!(": {steps} steps from {}", s(&want[2]))),
        "{caption}"
    );
    let rows = v["rows"].as_array().unwrap();
    assert_eq!(rows.last().unwrap()["step"], steps);
    // the fixture's step labels reach the rows, listed at the room each action leaves
    assert!(
        rows.iter().any(|r| r["text"].as_str().unwrap().contains(
            "Temple Foyer (1/2316) -- go W (Alignment: Saint to Seedy) -> Temple Hall (1/520)"
        )),
        "{v}"
    );

    // neither a monster nor a room
    let (st, _) = call(
        &app,
        "POST",
        "/api/reachability/route",
        Some(&json!({"level": 1})),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    // a monster that is not in the game has no route information
    let body = json!({"monster": 424_242, "level": 30});
    let (_, v) = call(&app, "POST", "/api/reachability/route", Some(&body)).await;
    assert_eq!(v["message"], "No route information for this monster.");
}

#[test]
fn route_labels_name_their_numbers() {
    use touchstone::route::format_route_label;
    let fx = fixture();
    let w = world(&fx["header"]["data"]);
    // FormatRouteLabel: tolls with thousands separators, spell #0 is "none", class and race wording
    assert_eq!(
        format_route_label(&w, "go N (Toll: 1500)", false),
        "go N (Toll: 1,500 gold)"
    );
    assert_eq!(
        format_route_label(&w, "cast spell #0 here", false),
        "cast spell #none here"
    );
    assert_eq!(
        format_route_label(&w, "go E (Class: 1 OK, 0 NO)", false),
        "go E (Class Only: Warrior(1))"
    );
    assert_eq!(
        format_route_label(&w, "go E (Class: 1 OK, 0 NO)", true),
        "go E (Class Only: Warrior)"
    );
    assert_eq!(
        format_route_label(&w, "go E (Race: 0 OK, 1 NO)", false),
        "go E (NOT Race: Human(1))"
    );
    // a missing record is its number, and a run of ten digits is left alone
    assert_eq!(
        format_route_label(&w, "item #5000000", false),
        "item #5000000"
    );
    assert_eq!(
        format_route_label(&w, "item #12345678901", false),
        "item #12345678901"
    );
}
