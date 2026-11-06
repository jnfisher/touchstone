//! `/api/map/...`, pinned to fixtures/rules/map.json (frmMain/frmMap MapStartMapping, MapGoDirection,
//! cmdMove_Click, cmdMapLeadsHere_Click); the world is rebuilt from the fixture header.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use touchstone::state::{Loaded, SourceInfo};
use touchstone::{AppState, app};
use touchstone_model::{
    AbilitySlot, DropSlot, Info, MidSpell, Monster, MonsterAttack, Room, Spell, TbInfo, World,
    WorldParts,
};
use tower::ServiceExt;

fn i(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

fn fixture() -> Value {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rules/map.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn table(v: &Value) -> Vec<serde_json::Map<String, Value>> {
    let fields: Vec<&str> = v["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap())
        .collect();
    v["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            fields
                .iter()
                .zip(r.as_array().unwrap())
                .map(|(f, x)| ((*f).to_string(), x.clone()))
                .collect()
        })
        .collect()
}

fn world(data: &Value) -> World {
    let monsters = table(&data["monsters"])
        .iter()
        .map(|r| Monster {
            number: i(&r["Number"]),
            name: Some(s(&r["Name"])),
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
            greet_txt: i(&r["GreetTXT"]),
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
            in_game: i(&r["In Game"]) != 0,
            summoned_by: Some(s(&r["Summoned By"])),
        })
        .collect();
    let spells = table(&data["spells"])
        .iter()
        .map(|r| Spell {
            number: i(&r["Number"]),
            name: Some(s(&r["Name"])),
            short: None,
            req_level: i(&r["ReqLevel"]),
            energy_cost: i(&r["EnergyCost"]),
            mana_cost: i(&r["ManaCost"]),
            min_base: i(&r["MinBase"]),
            max_base: i(&r["MaxBase"]),
            diff: i(&r["Diff"]),
            type_of_resists: i(&r["TypeOfResists"]),
            targets: i(&r["Targets"]),
            dur: i(&r["Dur"]),
            att_type: i(&r["AttType"]),
            magery: i(&r["Magery"]),
            magery_lvl: i(&r["MageryLVL"]),
            cap: i(&r["Cap"]),
            max_inc_lvls: i(&r["MaxIncLVLs"]),
            max_inc: i(&r["MaxInc"]),
            min_inc_lvls: i(&r["MinIncLVLs"]),
            min_inc: i(&r["MinInc"]),
            dur_inc_lvls: i(&r["DurIncLVLs"]),
            dur_inc: i(&r["DurInc"]),
            abilities: pairs(&r["Abilities"]),
            learnable: i(&r["Learnable"]) != 0,
            learned_from: Some(s(&r["Learned From"])),
            casted_by: Some(s(&r["Casted By"])),
            classes: Some(s(&r["Classes"])),
        })
        .collect();
    let tbinfo = table(&data["tbinfo"])
        .iter()
        .map(|r| TbInfo {
            number: i(&r["Number"]),
            link_to: i(&r["LinkTo"]),
            action: Some(s(&r["Action"])),
            called_from: String::new(),
        })
        .collect();
    let rooms = table(&data["rooms"])
        .iter()
        .map(|r| Room {
            map_number: i(&r["Map Number"]),
            room_number: i(&r["Room Number"]),
            name: Some(s(&r["Name"])),
            light: 0,
            shop: i(&r["Shop"]),
            npc: i(&r["NPC"]),
            cmd: i(&r["CMD"]),
            spell: i(&r["Spell"]),
            lair: Some(s(&r["Lair"])),
            delay: 0,
            placed: None,
            n: s(&r["N"]),
            s: s(&r["S"]),
            e: s(&r["E"]),
            w: s(&r["W"]),
            ne: s(&r["NE"]),
            nw: s(&r["NW"]),
            se: s(&r["SE"]),
            sw: s(&r["SW"]),
            u: s(&r["U"]),
            d: s(&r["D"]),
        })
        .collect();
    World::new(WorldParts {
        info: Info {
            nmr_version: "v1.83".to_owned(),
            dat_file_version: "v1.11p".to_owned(),
            content_version: None,
            date: String::new(),
            time: String::new(),
            custom: String::new(),
            legit: false,
            disable_kai: false,
            update_url: String::new(),
        },
        items: vec![],
        monsters,
        spells,
        rooms,
        shops: vec![],
        classes: vec![],
        races: vec![],
        lairs: vec![],
        tbinfo,
    })
    .unwrap()
}

fn pairs<const N: usize>(v: &Value) -> [AbilitySlot; N] {
    let mut out = [AbilitySlot::default(); N];
    for (k, p) in v.as_array().unwrap().iter().enumerate() {
        out[k] = AbilitySlot {
            ability: i(&p[0]),
            value: i(&p[1]),
        };
    }
    out
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

async fn call(app: &axum::Router, uri: &str, body: Option<&Value>) -> (StatusCode, Value) {
    let req = match body {
        Some(b) => Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(b.to_string()))
            .unwrap(),
        None => Request::get(uri).body(Body::empty()).unwrap(),
    };
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, v)
}

fn query(case: &Value) -> String {
    let frm = &case["inputs"]["forms"]["frmMain"];
    let chk = |n: &str| frm["chkMapOptions"]["$array"][n]["Value"].as_i64() == Some(1);
    let mut opts = Vec::new();
    for (n, name) in [
        ("0", "follow"),
        ("1", "no_hidden"),
        ("2", "no_lairs"),
        ("3", "no_npcs"),
        ("4", "no_cmds"),
        ("9", "dupes"),
        ("10", "overwrite"),
        ("12", "no_restricted"),
    ] {
        if chk(n) {
            opts.push(name);
        }
    }
    if frm["optAlsoMark"]["$array"]["1"]["Value"] == json!(true) {
        opts.push("mark_shops");
    }
    if frm["optAlsoMark"]["$array"]["2"]["Value"] == json!(true) {
        opts.push("mark_spells");
    }
    let args = case["inputs"]["steps"][0]["args"].as_array().unwrap();
    let mut q = format!(
        "map={}&room={}&size={}&options={}",
        args[0],
        args[1],
        case["inputs"]["surface"].as_str().unwrap(),
        opts.join(",")
    );
    if let Some(c) = args.get(2) {
        q.push_str(&format!("&center={c}"));
    }
    q
}

fn case<'a>(fx: &'a Value, id: &str) -> &'a Value {
    fx["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no case {id}"))
}

fn packed(v: &Value) -> Vec<i64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap())
        .collect()
}

#[tokio::test]
async fn map_endpoint_draws_what_the_oracle_recorded() {
    let fx = fixture();
    let app = router(world(&fx["header"]["data"]));
    // expected values: fixtures/rules/map.json, the cases named here
    for id in [
        "m-hub-follow",
        "m-hub-both",
        "w20x20-hub",
        "wzoom28-hub",
        "m-corridor",
        "w30x50-diagonal",
    ] {
        let c = case(&fx, id);
        let (status, v) = call(&app, &format!("/api/map?{}", query(c)), None).await;
        assert_eq!(status, StatusCode::OK, "{id}: {v}");
        let want = packed(&c["outputs"]["m"]["draws"]);
        let got: Vec<i64> = v["draws"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| {
                let shape = [
                    "Square",
                    "Star",
                    "OpenCircle",
                    "Up",
                    "Down",
                    "Circle",
                    "LineN",
                    "LineS",
                    "LineE",
                    "LineW",
                    "LineNE",
                    "LineNW",
                    "LineSE",
                    "LineSW",
                ]
                .iter()
                .position(|n| *n == d["shape"].as_str().unwrap())
                .unwrap() as i64;
                (d["cell"].as_i64().unwrap() << 12)
                    | (shape << 8)
                    | (d["size"].as_i64().unwrap() << 4)
                    | d["color"].as_i64().unwrap()
            })
            .collect();
        assert_eq!(got, want, "{id}");
        assert_eq!(v["center_cell"], c["outputs"]["m"]["center"], "{id}");
        let charted = c["outputs"]["m"]["cells"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r[1].as_i64().unwrap() != 0)
            .count();
        assert_eq!(v["cells"].as_array().unwrap().len(), charted, "{id}");
        assert!(
            v["title"].as_str().unwrap().starts_with("Rooms -- "),
            "{id}"
        );
    }
}

#[tokio::test]
async fn map_endpoint_refuses_what_mme_refuses() {
    let fx = fixture();
    let app = router(world(&fx["header"]["data"]));
    // expected value: case m-missing, where MME shows "Room 9999/1 was not found."
    let (status, _) = call(&app, "/api/map?map=9999&room=1", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&app, "/api/map?map=9001&room=1&size=99x99", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(&app, "/api/map?map=9001&room=1&options=nonsense", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn go_endpoint_follows_an_exit() {
    let fx = fixture();
    let app = router(world(&fx["header"]["data"]));
    // expected value: case go-main step g9001_1_N (the hub's north exit goes to 9001/2)
    let (status, v) = call(&app, "/api/map/go?map=9001&room=1&dir=N", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["Go"], json!({"map": 9001, "room": 2}));
    let (_, v) = call(&app, "/api/map/go?map=9999&room=1&dir=N", None).await;
    assert_eq!(v["Message"], "Source room (9999/1) not found.");
    let (_, v) = call(&app, "/api/map/go?map=9001&room=1&dir=NE&window=true", None).await;
    assert_eq!(v["Go"], json!({"map": 9001, "room": 6}));
}

#[tokio::test]
async fn leads_here_endpoint_lists_the_rows() {
    let fx = fixture();
    let app = router(world(&fx["header"]["data"]));
    let c = case(&fx, "leads-9001-1");
    let want: Vec<String> = c["outputs"]["l.cap"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| s(&r[0]))
        .collect();
    let (status, v) = call(
        &app,
        "/api/map/leads-here?map=9001&room=1&only_in_game=false",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let got: Vec<String> = v["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| s(&r["text"]))
        .collect();
    assert_eq!(got, want);
    assert_eq!(v["caption"], c["outputs"]["l.cap"]["caption"]);
    let (status, _) = call(&app, "/api/map/leads-here?map=0&room=1", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn presets_and_legend() {
    let fx = fixture();
    let app = router(world(&fx["header"]["data"]));
    let (_, v) = call(&app, "/api/map/presets", None).await;
    assert_eq!(v["count"], 50);
    assert_eq!(v["presets"].as_array().unwrap().len(), 50);
    // expected value: data.presets, read from LoadPresets
    let first = &fx["header"]["data"]["presets"][0];
    assert_eq!(v["presets"][0]["name"], first[0]);
    assert_eq!(v["presets"][0]["map"], first[1]);
    let (_, v) = call(&app, "/api/map/legend", None).await;
    assert!(v.as_array().unwrap().len() >= 9);
}
