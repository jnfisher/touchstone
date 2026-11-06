//! `/api/clipboard/*` over the paste_char fixture's world. The texts themselves are pinned by
//! fixtures/rules/clipboard.json in the rules crate; these tests check the request shaping and the
//! values the server gathers (shop list row, character tags).

use axum::http::StatusCode;
use serde_json::json;

use crate::paste::{app_for, char_world, fixture, post};

fn app() -> (tempfile::TempDir, axum::Router) {
    let fx = fixture("paste_char");
    app_for(char_world(&fx["header"]["data"]))
}

#[tokio::test]
async fn shop_text_starts_with_the_list_row_and_the_assigned_rooms() {
    let (_d, app) = app();
    let (status, got) = post(&app, "/api/clipboard/shop", &json!({ "number": 5 })).await;
    assert_eq!(status, StatusCode::OK, "{got}");
    let text = got["text"].as_str().unwrap();
    // CopyShopToClipboard: "#: n, Name: x", the type, " -- ", the Assigned To list
    assert!(
        text.starts_with("#: 5, Name: Sword Shop\r\nType: "),
        "{text:?}"
    );
    assert!(
        text.contains("\r\nAssigned To: Room: Helfgrim's Blades (1/355)\r\n\r\n"),
        "{text:?}"
    );
    let (_, name) = post(
        &app,
        "/api/clipboard/shop",
        &json!({ "number": 5, "name_only": true }),
    )
    .await;
    assert_eq!(name["text"], "Sword Shop");
}

#[tokio::test]
async fn an_unknown_shop_is_rejected() {
    let (_d, app) = app();
    let (status, _) = post(&app, "/api/clipboard/shop", &json!({ "number": 999_999 })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn monster_lines_pad_labels_and_name_only_cuts_the_number() {
    let (_d, app) = app();
    let lines = json!([
        { "text": "Name", "subs": ["Giant Rat (12)"] },
        { "text": "Level", "subs": ["3"] }
    ]);
    let (_, all) = post(&app, "/api/clipboard/monster", &json!({ "lines": lines })).await;
    assert_eq!(
        all["text"],
        "Name             Giant Rat (12)\r\nLevel            3\r\n"
    );
    let (_, name) = post(
        &app,
        "/api/clipboard/monster",
        &json!({ "lines": lines, "name_only": true }),
    )
    .await;
    assert_eq!(name["text"], "Giant Rat");
    // a line with no cell is VB error 35600 and copies nothing
    let (status, bad) = post(
        &app,
        "/api/clipboard/monster",
        &json!({ "lines": [{ "text": "x" }] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(bad["text"].is_null());
    assert!(bad["error"].as_str().unwrap().contains("35600"), "{bad}");
}

#[tokio::test]
async fn list_lines_follow_the_columns_and_the_selection() {
    let (_d, app) = app();
    let req = json!({
        "list": "lvWeapons",
        "columns": [{ "text": "Name", "width": 1000 }, { "text": "Min", "width": 500 }],
        "rows": [
            { "text": "dagger", "subs": ["3"], "selected": true, "detail": "Sharp." },
            { "text": "club", "subs": ["1"] }
        ]
    });
    let (_, got) = post(&app, "/api/clipboard/lines", &req).await;
    assert_eq!(got["text"], "Name: dagger, Min: 3\r\n>> Sharp.");
    let mut names = req.clone();
    names["name_only"] = json!(true);
    let (_, got) = post(&app, "/api/clipboard/lines", &names).await;
    assert_eq!(got["text"], "dagger");
}

#[tokio::test]
async fn character_text_has_the_stat_block_and_the_worn_items() {
    let (_d, app) = app();
    let req = json!({ "character": {
        "name": "Syntax", "level": 10, "class": 1, "race": 2,
        "stats": [60, 40, 40, 50, 50, 40], "equipped": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 325, 0, 0, 0]
    }});
    let (status, got) = post(&app, "/api/clipboard/character", &req).await;
    assert_eq!(status, StatusCode::OK, "{got}");
    let text = got["text"].as_str().unwrap();
    assert!(text.starts_with("Name: Syntax"), "{text:?}");
    assert!(text.contains("Class: Warrior"), "{text:?}");
    assert!(
        text.contains("They are equipped with:\r\n\r\nhellblade"),
        "{text:?}"
    );
    assert!(text.contains("(Weapon Hand)"), "{text:?}");
    let mut stats = req.clone();
    stats["stats_only"] = json!(true);
    let (_, got) = post(&app, "/api/clipboard/character", &stats).await;
    assert!(!got["text"].as_str().unwrap().contains("equipped"));
}
