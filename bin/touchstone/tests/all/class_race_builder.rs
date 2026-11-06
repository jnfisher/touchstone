//! `/api/class-race-builder` replaying cases of fixtures/rules/class_race_builder.json (MME's own
//! RefreshClassRaceBuilder run by the oracle) through the wire. The rules are pinned in the rules crate;
//! this checks the request and response.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use touchstone::state::{Loaded, SourceInfo};
use touchstone::{AppState, app};
use touchstone_model::{AbilitySlot, Class, Info, Race, World, WorldParts};
use tower::ServiceExt;

fn int(v: &Value) -> i32 {
    i32::try_from(v.as_i64().unwrap()).unwrap()
}

fn slots(v: &Value) -> [AbilitySlot; 10] {
    let mut out = [AbilitySlot::default(); 10];
    for (k, p) in v.as_array().unwrap().iter().enumerate() {
        out[k] = AbilitySlot {
            ability: int(&p[0]),
            value: int(&p[1]),
        };
    }
    out
}

/// The fixture's rows: all thirteen stock races, so `gAvgLevelMaxSingleStat` is the stock one.
fn world(data: &Value) -> World {
    let classes = data["classes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Class {
            number: int(&r["n"]),
            name: r["name"].as_str().unwrap().into(),
            min_hits: int(&r["min"]),
            max_hits: int(&r["max"]),
            exp_table: int(&r["exp"]),
            magery_type: int(&r["mt"]),
            magery_lvl: int(&r["ml"]),
            weapon_type: int(&r["wt"]),
            armour_type: int(&r["at"]),
            combat_lvl: int(&r["combat"]),
            abilities: slots(&r["ab"]),
        })
        .collect();
    let races = data["races"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Race {
            number: int(&r["n"]),
            name: r["name"].as_str().unwrap().into(),
            m_int: int(&r["stats"][0]),
            m_wil: int(&r["stats"][1]),
            m_str: int(&r["stats"][2]),
            m_hea: int(&r["stats"][3]),
            m_agl: int(&r["stats"][4]),
            m_chm: int(&r["stats"][5]),
            x_int: int(&r["stats"][6]),
            x_wil: int(&r["stats"][7]),
            x_str: int(&r["stats"][8]),
            x_hea: int(&r["stats"][9]),
            x_agl: int(&r["stats"][10]),
            x_chm: int(&r["stats"][11]),
            hp_per_lvl: int(&r["hp"]),
            exp_table: int(&r["exp"]),
            base_cp: int(&r["cp"]),
            abilities: slots(&r["ab"]),
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
        items: vec![],
        monsters: vec![],
        spells: vec![],
        rooms: vec![],
        shops: vec![],
        classes,
        races,
        lairs: vec![],
        tbinfo: vec![],
    })
    .unwrap()
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<&Value>,
) -> (StatusCode, Value) {
    let req = Request::builder().method(method).uri(uri);
    let req = match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(b.to_string())),
        None => req.body(Body::empty()),
    }
    .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, v)
}

fn app_for(doc: &Value) -> (axum::Router, TempDir) {
    let dir = TempDir::new().unwrap();
    let info = SourceInfo {
        kind: "mdb",
        path: "test".into(),
    };
    let loaded = Loaded::build(world(&doc["header"]["data"]), info, None, 1).unwrap();
    let router = app(AppState::with_data_dir(loaded, dir.path().join("data")));
    (router, dir)
}

fn fixture() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/rules/class_race_builder.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[tokio::test]
async fn builder_text_agrees_through_the_endpoint() {
    let doc = fixture();
    let (app, _dir) = app_for(&doc);
    // cases that ran ComputeAvgLevelMaxStats first, so the server's own average applies
    let wanted = [
        "class-01",
        "class-12",
        "class-09",
        "race-05-class-12",
        "race-13-class-09",
        "class-only",
        "race-only",
        "bad-class",
        "none",
    ];
    for id in wanted {
        let case = doc["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .unwrap_or_else(|| panic!("no fixture case {id}"));
        let tags = &case["inputs"]["forms"]["frmMain"]["txtClassRaceDetail"]["$array"];
        let sel = |k: &str| {
            tags[k]["Tag"]
                .as_str()
                .unwrap_or("")
                .parse::<i32>()
                .unwrap_or(0)
        };
        let uri = format!(
            "/api/class-race-builder?class={}&race={}",
            sel("0"),
            sel("1")
        );
        let (status, v) = call(&app, "GET", &uri, None).await;
        assert_eq!(status, StatusCode::OK, "{id}: {v}");
        // fixtures/rules/class_race_builder.json: the text MME put in txtClassRaceDetail(2)
        assert_eq!(v["text"], case["outputs"]["text"], "{id}");
        assert_eq!(v["error"], Value::Null, "{id}");
    }
}

#[tokio::test]
async fn unknown_numbers_count_as_no_selection() {
    let doc = fixture();
    let (app, _dir) = app_for(&doc);
    let (status, v) = call(
        &app,
        "GET",
        "/api/class-race-builder?class=99&race=99",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (v["class"].as_i64(), v["race"].as_i64()),
        (Some(0), Some(0))
    );
    // fixtures/rules/class_race_builder.json case bad-both
    assert_eq!(v["text"], "");
    let (_, v) = call(&app, "GET", "/api/class-race-builder", None).await;
    assert_eq!(v["text"], "");
}

#[tokio::test]
async fn buttons_follow_the_selection() {
    let doc = fixture();
    let (app, _dir) = app_for(&doc);
    let (_, v) = call(
        &app,
        "POST",
        "/api/class-race-builder/equip",
        Some(&json!({"class": 3, "race": 99})),
    )
    .await;
    assert_eq!(v, json!({"class": 3, "race": null}));
    // fixtures/rules/class_race_builder.json spellbook-09: Bard, magery 4, level 2
    let (_, v) = call(
        &app,
        "POST",
        "/api/class-race-builder/spellbook",
        Some(&json!({"class": 9})),
    )
    .await;
    assert_eq!(
        v,
        json!({"action": "open", "class": 9, "level": 999, "magery": 4, "magery_level": 2})
    );
    // spellbook-01: Warrior has no magery
    let (_, v) = call(
        &app,
        "POST",
        "/api/class-race-builder/spellbook",
        Some(&json!({"class": 1})),
    )
    .await;
    assert_eq!(v["action"], "no_magic");
    let (_, v) = call(
        &app,
        "POST",
        "/api/class-race-builder/spellbook",
        Some(&json!({"class": 0})),
    )
    .await;
    assert_eq!(v["action"], "nothing");
}
