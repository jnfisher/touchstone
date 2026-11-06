//! `/api/character/find-best` replaying cases of fixtures/rules/find_best.json (MME's own InvenFindBest run by
//! the oracle) through the wire. The rules are pinned in the rules crate; this checks the request and response.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use touchstone::state::{Loaded, SourceInfo};
use touchstone::{AppState, app};
use touchstone_model::{AbilitySlot, Info, Item, World, WorldParts};
use tower::ServiceExt;

fn int(v: &Value) -> i32 {
    i32::try_from(v.as_i64().unwrap()).unwrap()
}

fn world(data: &Value) -> World {
    let items = data["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut abilities = [AbilitySlot::default(); 20];
            for (k, p) in r[8].as_array().unwrap().iter().enumerate() {
                abilities[k] = AbilitySlot {
                    ability: int(&p[0]),
                    value: int(&p[1]),
                };
            }
            Item {
                number: int(&r[0]),
                name: format!("item {}", r[0]),
                limit: int(&r[7]),
                encum: int(&r[3]),
                item_type: int(&r[1]),
                use_count: 0,
                price: 0,
                currency: 0,
                min: 0,
                max: 0,
                armour_class: int(&r[4]),
                damage_resist: int(&r[5]),
                weapon_type: int(&r[2]),
                armour_type: 0,
                worn: int(&r[9]),
                accy: int(&r[6]),
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
            dat_file_version: "v1.11p".into(),
            content_version: None,
            date: String::new(),
            time: String::new(),
            custom: String::new(),
            legit: false,
            disable_kai: false,
            update_url: String::new(),
        },
        items,
        monsters: vec![],
        spells: vec![],
        rooms: vec![],
        shops: vec![],
        classes: vec![],
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

fn menu_code(call: &str) -> i32 {
    match call {
        "mnuFindBestArmour_Click" => 0,
        "mnuFindBestAttack_Click" => 1,
        "mnuFindBestResist_Click" => 2,
        "mnuFindBestStats_Click" => 3,
        "mnuFindBestMystics_Click" => 4,
        "mnuFindBestAttrib_Click" => 5,
        other => panic!("unknown handler {other}"),
    }
}

/// Replays one fixture case: pre-equips through `/equip`, then every Find Best and Next Best step through
/// `/find-best`, carrying the exclusion list and the last menu entry as the web client does.
async fn replay(app: &axum::Router, case: &Value) {
    let out = &case["outputs"];
    let frm = &case["inputs"]["forms"]["frmMain"];
    let lists: Vec<Value> = (0..20)
        .map(|s| {
            Value::Array(
                out["lists"][s.to_string()]["data"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .skip(1)
                    .cloned()
                    .collect(),
            )
        })
        .collect();
    let hold: Vec<bool> = (0..20)
        .map(|k| frm["chkEquipHold"]["$array"][k.to_string()]["Value"].as_i64() == Some(1))
        .collect();
    let second = case["inputs"]["globals"]["bInvenUse2ndWrist"].as_bool() == Some(true);
    let no_limited = frm["chkInvenNoLimited"]["Value"].as_i64() == Some(1);
    let mut character =
        json!({"name": "t", "class": 1, "race": 1, "level": 50, "equipped": vec![0; 20]});
    let mut excluded = Value::Null;
    let mut last = json!({"index": 0, "criterion": 0});
    for st in case["inputs"]["steps"].as_array().unwrap() {
        if st["select"].is_string() {
            if out[st["as"].as_str().unwrap()].as_bool() == Some(true) {
                character["equipped"][st["index"].as_u64().unwrap() as usize] = st["item"].clone();
            }
            continue;
        }
        let name = st["call"].as_str().unwrap();
        let mut body = json!({"character": character, "slot_lists": lists, "hold": hold, "no_limited": no_limited,
            "use_2nd_wrist": second, "excluded": excluded, "last": last});
        match name {
            "InvenEquipItem" => {
                let (_, v) = call(
                    app,
                    "/api/character/equip",
                    &json!({"character": character, "item": st["args"][0], "use_2nd_wrist": second}),
                )
                .await;
                character = v["character"].clone();
                continue;
            }
            "cmdEquipButtons_Click" if st["args"][0] == 12 => body["next"] = json!(true),
            n if n.starts_with("mnuFindBest") => {
                body["criterion"] = json!(menu_code(n));
                body["index"] = st["args"][0].clone();
            }
            _ => continue,
        }
        let (status, v) = call(app, "/api/character/find-best", &body).await;
        assert_eq!(status, StatusCode::OK, "{v}");
        // fixtures/rules/find_best.json: the capture MME's step recorded
        let cap = &out[format!("{}.cap", st["as"].as_str().unwrap())];
        assert_eq!(v["character"]["equipped"], cap["eq"], "{}", case["id"]);
        assert_eq!(
            json!([v["last"]["index"], v["last"]["criterion"]]),
            cap["last"]
        );
        character = v["character"].clone();
        excluded = v["excluded"].clone();
        last = v["last"].clone();
    }
}

#[tokio::test]
async fn fixture_cases_agree_through_the_endpoint() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/rules/find_best.json");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let w = world(&doc["header"]["data"]);
    let dir = TempDir::new().unwrap();
    let info = SourceInfo {
        kind: "mdb",
        path: "test".into(),
    };
    let loaded = Loaded::build(w, info, None, 1).unwrap();
    let app = app(AppState::with_data_dir(loaded, dir.path().join("data")));
    let wanted = [
        "unique-nolimit-0",
        "unique-nolimit-1",
        "2h-beats-offhand",
        "offhand-beats-2h",
        "rings-dupe",
        "rings-slot10-held",
        "wrists-dupe-w1",
        "wrists-chain-w2",
        "tie-first-listed",
        "tie-chain",
        "next-no-find-first",
        "bail-keeps-last-index",
        "chain-exhausts",
        "find-resets-exclusions",
    ];
    for id in wanted {
        let case = doc["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .unwrap_or_else(|| panic!("no case {id}"));
        replay(&app, case).await;
    }
}

#[tokio::test]
async fn bad_menu_is_refused() {
    let dir = TempDir::new().unwrap();
    let doc = json!({"items": []});
    let loaded = Loaded::build(
        world(&doc),
        SourceInfo {
            kind: "mdb",
            path: "test".into(),
        },
        None,
        1,
    )
    .unwrap();
    let app = app(AppState::with_data_dir(loaded, dir.path().join("data")));
    let (st, _) = call(
        &app,
        "/api/character/find-best",
        &json!({"character": {"equipped": []}, "criterion": 9, "index": 0}),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
}
