//! `/api/settings`: MME's save rules, persistence in the data directory, a damaged file, and the
//! per-character exp/hr block.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use touchstone::state::{Loaded, SourceInfo};
use touchstone::{AppState, app};
use touchstone_model::{Info, World, WorldParts};
use tower::ServiceExt;

fn router(data_dir: &Path) -> axum::Router {
    let world = World::new(WorldParts {
        info: Info {
            nmr_version: "v1.8.3".to_owned(),
            dat_file_version: "v1.11p".to_owned(),
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
        classes: vec![],
        races: vec![],
        lairs: vec![],
        tbinfo: vec![],
    })
    .unwrap();
    let info = SourceInfo {
        kind: "mdb",
        path: "test".into(),
    };
    let loaded = Loaded::build(world, info, None, 1).unwrap();
    app(AppState::with_data_dir(loaded, data_dir.to_owned()))
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<&Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    let b = match body {
        Some(v) => {
            req = req.header("content-type", "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    let res = app.clone().oneshot(req.body(b).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, v)
}

#[tokio::test]
async fn defaults_are_what_mme_runs_with() {
    let dir = TempDir::new().unwrap();
    let app = router(dir.path());
    let (st, v) = call(&app, "GET", "/api/settings", None).await;
    assert_eq!(st, StatusCode::OK);
    // frmMain LoadExpPerHourKnobs reads missing cephModel keys as 1, so all four models are on.
    let g = &v["exp_per_hour"]["global"];
    for m in ["model_a", "model_b", "model_c", "model_d"] {
        assert_eq!(g[m], true, "{m}");
    }
    assert_eq!(g["recovery_only"], false);
    assert_eq!(g["show_all"], false);
    assert_eq!(g["knob_xp"], 1.0);
    assert_eq!(v["exp_per_hour"]["by_character"], false);
    // frmMain Form_Load sets 500 rounds; the waypoint interval's default in LoadSettings is 25.
    assert_eq!(v["monster_sim_rounds"], 500);
    assert_eq!(v["route_waypoint_interval"], 25);
    assert_eq!(v["auto_load_last_char"], true);
    // frmMain Form_Load: bHideRecordNumbers is true only for a HideRecordNumbers key of "1"
    assert_eq!(v["hide_record_numbers"], false);
}

#[tokio::test]
async fn put_applies_cmd_save_click_rules() {
    let dir = TempDir::new().unwrap();
    let app = router(dir.path());
    let body = json!({
        "exp_per_hour": {
            "by_character": true,
            "global": {
                "model_a": false, "model_b": false, "model_c": false, "model_d": false,
                "recovery_only": true, "show_all": true,
                "knob_xp": 12.0, "knob_dmg": 0.004, "knob_mana": 1.234, "knob_move": 2.5
            }
        },
        "monster_sim_rounds": 20,
        "route_waypoint_interval": 0,
        "auto_load_last_char": false
    });
    let (st, v) = call(&app, "PUT", "/api/settings", Some(&body)).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let g = &v["exp_per_hour"]["global"];
    // cmdSave_Click: with no model ticked, A, B and C come on and D stays off.
    assert_eq!(
        [
            g["model_a"].clone(),
            g["model_b"].clone(),
            g["model_c"].clone(),
            g["model_d"].clone()
        ],
        [json!(true), json!(true), json!(true), json!(false)]
    );
    // Knobs are rounded to two places and kept between 0.01 and 10.
    assert_eq!(g["knob_xp"], 10.0);
    assert_eq!(g["knob_dmg"], 0.01);
    assert_eq!(g["knob_mana"], 1.23);
    assert_eq!(g["knob_move"], 2.5);
    assert_eq!(g["recovery_only"], true);
    assert_eq!(v["monster_sim_rounds"], 100);
    // Blank or 0 means the default 25.
    assert_eq!(v["route_waypoint_interval"], 25);
    assert_eq!(v["auto_load_last_char"], false);

    let big = json!({"monster_sim_rounds": 99999, "route_waypoint_interval": 5000});
    let (_, v) = call(&app, "PUT", "/api/settings", Some(&big)).await;
    assert_eq!(v["monster_sim_rounds"], 10_000);
    assert_eq!(v["route_waypoint_interval"], 1000);
    // A PUT replaces everything: fields left out return to their defaults.
    assert_eq!(v["exp_per_hour"]["global"]["model_d"], true);
}

#[tokio::test]
async fn bad_bodies_are_400() {
    let dir = TempDir::new().unwrap();
    let app = router(dir.path());
    for body in [
        json!({"nope": 1}),
        json!({"monster_sim_rounds": "many"}),
        json!({"exp_per_hour": {"global": {"knob_xp": "x"}}}),
        json!({"exp_per_hour": {"global": {"surprise": true}}}),
    ] {
        let (st, _) = call(&app, "PUT", "/api/settings", Some(&body)).await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "{body}");
    }
    // Nothing was written.
    assert!(!dir.path().join("settings.json").exists());
}

#[tokio::test]
async fn settings_survive_a_restart() {
    let dir = TempDir::new().unwrap();
    let app = router(dir.path());
    let body = json!({"monster_sim_rounds": 750, "exp_per_hour": {"global": {"knob_dmg": 1.5}}});
    let (st, _) = call(&app, "PUT", "/api/settings", Some(&body)).await;
    assert_eq!(st, StatusCode::OK);
    // No temp files are left beside the result.
    let names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["settings.json"]);

    let again = router(dir.path());
    let (_, v) = call(&again, "GET", "/api/settings", None).await;
    assert_eq!(v["monster_sim_rounds"], 750);
    assert_eq!(v["exp_per_hour"]["global"]["knob_dmg"], 1.5);
}

#[tokio::test]
async fn damaged_file_is_reported_and_kept_until_a_save() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("settings.json"), "{ not json").unwrap();
    let app = router(dir.path());
    let (st, v) = call(&app, "GET", "/api/settings", None).await;
    assert_eq!(st, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(v.as_str().unwrap().contains("settings.json"), "{v}");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("settings.json")).unwrap(),
        "{ not json"
    );
    // An explicit save replaces it, keeping the old text aside.
    let (st, _) = call(&app, "PUT", "/api/settings", Some(&json!({}))).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("settings.json.damaged")).unwrap(),
        "{ not json"
    );
    let (st, _) = call(&app, "GET", "/api/settings", None).await;
    assert_eq!(st, StatusCode::OK);
}

#[tokio::test]
async fn effective_block_follows_by_character() {
    let dir = TempDir::new().unwrap();
    let app = router(dir.path());
    let own = json!({"model_d": false, "knob_move": 3.0, "knob_xp": 99.0});
    let (st, v) = call(
        &app,
        "PUT",
        "/api/characters/Bob",
        Some(&json!({"name": "Bob", "exp_hr": own})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    // The character's block is clamped like the global one.
    assert_eq!(v["exp_hr"]["knob_xp"], 10.0);

    let (_, v) = call(&app, "GET", "/api/settings?character=Bob", None).await;
    assert_eq!(
        v["effective"]["model_d"], true,
        "by_character is off: global"
    );
    assert_eq!(v["effective"]["knob_move"], 1.0);

    let on = json!({"exp_per_hour": {"by_character": true}});
    call(&app, "PUT", "/api/settings", Some(&on)).await;
    let (_, v) = call(&app, "GET", "/api/settings?character=Bob", None).await;
    assert_eq!(v["effective"]["model_d"], false);
    assert_eq!(v["effective"]["knob_move"], 3.0);

    // MME reads a missing per-character key as the default (not the global value); with no saved
    // character open the global block applies.
    let global = json!({"exp_per_hour": {"by_character": true, "global": {"knob_move": 2.0}}});
    call(&app, "PUT", "/api/settings", Some(&global)).await;
    call(
        &app,
        "PUT",
        "/api/characters/Amy",
        Some(&json!({"name": "Amy"})),
    )
    .await;
    let (_, v) = call(&app, "GET", "/api/settings?character=Amy", None).await;
    assert_eq!(v["effective"]["knob_move"], 1.0);
    let (_, v) = call(&app, "GET", "/api/settings?character=Nobody", None).await;
    assert_eq!(v["effective"]["knob_move"], 2.0);
    let (st, _) = call(&app, "GET", "/api/settings?character=a/b", None).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    // Without the query there is no effective block.
    let (_, v) = call(&app, "GET", "/api/settings", None).await;
    assert!(v.get("effective").is_none());
}

// Expected: fixtures/rules/ability_text.json cases list-0 and list-1 (GetAbilityList with bHideRecordNumbers off and
// on); the setting stands in for the global.
#[tokio::test]
async fn ability_list_follows_hide_record_numbers() {
    let doc: Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rules/ability_text.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let want = |id: &str| -> Vec<(i64, String)> {
        let case = doc["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .unwrap();
        case["outputs"]["result"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .skip(1)
            .map(|(n, v)| (i64::try_from(n).unwrap(), v.as_str().unwrap().to_owned()))
            .collect()
    };
    let dir = TempDir::new().unwrap();
    let app = router(dir.path());
    for (hide, id) in [(false, "list-0"), (true, "list-1")] {
        let (st, _) = call(
            &app,
            "PUT",
            "/api/settings",
            Some(&json!({"hide_record_numbers": hide})),
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        let (st, v) = call(&app, "GET", "/api/abilities", None).await;
        assert_eq!(st, StatusCode::OK);
        let got: Vec<(i64, String)> = v["abilities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                (
                    a["number"].as_i64().unwrap(),
                    a["name"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        assert_eq!(got, want(id), "{id}");
    }
}

#[tokio::test]
async fn hide_record_numbers_is_stored() {
    let dir = TempDir::new().unwrap();
    let app = router(dir.path());
    let (st, v) = call(
        &app,
        "PUT",
        "/api/settings",
        Some(&json!({"hide_record_numbers": true})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["hide_record_numbers"], true);
    // a restart reads it back from settings.json
    let again = router(dir.path());
    let (_, v) = call(&again, "GET", "/api/settings", None).await;
    assert_eq!(v["hide_record_numbers"], true);
}
