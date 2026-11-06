//! `/api/item-manager/*` over the paste_char fixture's world (items 298 scroll, 325 hellblade
//! weight 125, 330 hauberk weight 400, 337 claw glyph not gettable), the question loop, and the
//! saved lists following the rows. The list actions themselves are pinned by
//! fixtures/rules/item_manager.json in the rules crate.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::paste::{app_for, char_world, fixture, post};

fn app() -> (tempfile::TempDir, axum::Router) {
    let fx = fixture("paste_char");
    app_for(char_world(&fx["header"]["data"]))
}

fn row(item: &str, name: &str, flag: &str, qty: &str, source: &str, enc: &str) -> Value {
    let cells: Vec<Value> = [name, flag, qty, source, enc]
        .iter()
        .map(|t| json!({ "text": t }))
        .chain((0..5).map(|_| json!({ "text": "" })))
        .collect();
    json!({ "text": item, "subs": cells })
}

#[tokio::test]
async fn rows_are_rebuilt_from_the_saved_lists() {
    let (_d, app) = app();
    let req = json!({ "character": {
        "level": 1, "carried": [{ "item": 325, "qty": 2 }], "stash": [{ "item": 298, "qty": 1 }] } });
    let (status, got) = post(&app, "/api/item-manager/rows", &req).await;
    assert_eq!(status, StatusCode::OK, "{got}");
    let rows = got["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "{got}");
    // LoadCharacter adds the carried rows first; the flag carries the quantity
    assert_eq!(rows[0]["text"], "325");
    assert_eq!(rows[0]["subs"][1]["text"], "CARRIED x2");
    assert_eq!(rows[1]["subs"][1]["text"], "STASH");
    assert_eq!(
        got["character"]["carried"],
        json!([{ "item": 325, "qty": 2 }])
    );
}

#[tokio::test]
async fn carrying_asks_about_the_weight_and_saves_the_list() {
    let (_d, app) = app();
    let mut req = json!({
        "character": { "level": 1, "add_weight": 300 },
        "rows": [row("325", "hellblade", "", "1", "Inventory", "125")],
        "selected": [0], "action": 12 });
    // LV_SetActionCell asks before touching the additional weight
    let (status, got) = post(&app, "/api/item-manager/action", &req).await;
    assert_eq!(status, StatusCode::OK, "{got}");
    assert_eq!(got["needs_answer"]["kind"], "yes_no");
    assert!(
        got["needs_answer"]["text"]
            .as_str()
            .unwrap()
            .starts_with("Subtract hellblade's encumbrance")
    );
    assert_eq!(got["rows"][0]["subs"][1]["text"], "");
    req["answers"] = json!([6]);
    let (_, yes) = post(&app, "/api/item-manager/action", &req).await;
    assert_eq!(yes["rows"][0]["subs"][1]["text"], "CARRIED");
    assert_eq!(yes["character"]["add_weight"], 175);
    assert_eq!(
        yes["character"]["carried"],
        json!([{ "item": 325, "qty": 1 }])
    );
    assert_eq!(yes["carried_changed"], true);
    req["answers"] = json!([7]);
    let (_, no) = post(&app, "/api/item-manager/action", &req).await;
    assert_eq!(no["character"]["add_weight"], 300);
    assert_eq!(no["rows"][0]["subs"][1]["text"], "CARRIED");
}

#[tokio::test]
async fn plus_minus_and_stash_follow_the_buttons() {
    let (_d, app) = app();
    let rows = json!([row("298", "scroll", "STASH x2", "2", "Ground", "10")]);
    let go = |action: i32| {
        let rows = rows.clone();
        let app = app.clone();
        async move {
            post(
                &app,
                "/api/item-manager/action",
                &json!({ "character": { "level": 1 }, "rows": rows, "selected": [0], "action": action }),
            )
            .await
            .1
        }
    };
    let plus = go(16).await;
    assert_eq!(plus["rows"][0]["subs"][1]["text"], "STASH x3");
    assert_eq!(
        plus["character"]["stash"],
        json!([{ "item": 298, "qty": 3 }])
    );
    assert_eq!(plus["stash_changed"], true);
    let cleared = go(18).await;
    assert_eq!(cleared["rows"][0]["subs"][1]["text"], "");
    assert_eq!(cleared["character"]["stash"], json!([]));
}

#[tokio::test]
async fn deleting_several_rows_asks_first() {
    let (_d, app) = app();
    let mut req = json!({
        "character": { "level": 1 },
        "rows": [
            row("325", "hellblade", "CARRIED", "1", "Saved", "125"),
            row("298", "scroll", "DROP", "1", "Ground", "10"),
            row("330", "hauberk", "", "1", "Ground", "400")],
        "selected": [0, 1] });
    let (_, ask) = post(&app, "/api/item-manager/delete", &req).await;
    assert_eq!(
        ask["needs_answer"]["text"],
        "Are you sure you want to delete the selected rows?"
    );
    assert_eq!(ask["rows"].as_array().unwrap().len(), 3);
    req["answers"] = json!([7]);
    let (_, no) = post(&app, "/api/item-manager/delete", &req).await;
    assert_eq!(no["rows"].as_array().unwrap().len(), 3);
    req["answers"] = json!([6]);
    let (_, yes) = post(&app, "/api/item-manager/delete", &req).await;
    assert_eq!(yes["rows"].as_array().unwrap().len(), 1);
    assert_eq!(yes["rows"][0]["text"], "330");
    // the row after the highest selected one is selected next
    assert_eq!(yes["selected"], json!([0]));
    assert_eq!(yes["carried_changed"], true);
    assert_eq!(yes["character"]["carried"], json!([]));
    // one row needs no question
    let one = json!({ "character": { "level": 1 }, "rows": req["rows"], "selected": [2] });
    let (_, got) = post(&app, "/api/item-manager/delete", &one).await;
    assert!(got["needs_answer"].is_null());
    assert_eq!(got["rows"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn copy_builds_the_game_commands() {
    let (_d, app) = app();
    let req = json!({
        "rows": [
            row("325", "hellblade", "DROP x2", "1", "Inventory", "125"),
            row("298", "scroll", "USE", "1", "Ground", "10"),
            row("330", "hauberk", "CARRIED", "1", "Inventory", "400")],
        "selected": [0, 1, 2] });
    let (status, got) = post(&app, "/api/item-manager/copy", &req).await;
    assert_eq!(status, StatusCode::OK, "{got}");
    // LV_CopySelectedActionsToClipboard: a scroll (type 9) is read; CARRIED is ignored
    assert_eq!(
        got["text"],
        "drop hellblade\r\ndrop hellblade\r\nread scroll"
    );
    let none = json!({ "rows": [row("330", "hauberk", "", "1", "x", "0")], "selected": [0] });
    let (_, got) = post(&app, "/api/item-manager/copy", &none).await;
    assert!(got["text"].is_null());
}

#[tokio::test]
async fn adding_by_hand_checks_getable() {
    let (_d, app) = app();
    let (status, got) = post(
        &app,
        "/api/item-manager/add",
        &json!({ "character": { "level": 1 }, "rows": [], "item": 325 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{got}");
    assert_eq!(got["rows"][0]["subs"][1]["text"], "MANUAL");
    assert_eq!(got["rows"][0]["subs"][3]["text"], "Manual");
    let (_, got) = post(
        &app,
        "/api/item-manager/add",
        &json!({ "character": { "level": 1 }, "rows": [], "item": 337 }),
    )
    .await;
    assert_eq!(got["rows"], json!([]));
    assert_eq!(got["messages"][0], "Item 337 not \"getable\"");
}

#[tokio::test]
async fn bad_requests_are_rejected() {
    let (_d, app) = app();
    let (status, _) = post(
        &app,
        "/api/item-manager/action",
        &json!({ "character": { "level": 1 }, "rows": [], "action": 5 }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = post(&app, "/api/item-manager/copy", &json!({ "rows": "x" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// P4-26 follow-up: a paste's rows land in the saved lists.
#[tokio::test]
async fn a_paste_saves_its_carried_and_stash_rows() {
    let fx = fixture("paste_char");
    let (_d, app) = app_for(char_world(&fx["header"]["data"]));
    let req = json!({
        "character": { "level": 1, "stats": [0,0,0,0,0,0] },
        "text": "stat",
        "rows": [row("325", "hellblade", "CARRIED x2", "2", "Saved", "125"),
                 row("298", "scroll", "STASH", "1", "Saved", "10")],
    });
    let (status, got) = post(&app, "/api/paste/character", &req).await;
    assert_eq!(status, StatusCode::OK, "{got}");
    assert_eq!(
        got["character"]["carried"],
        json!([{ "item": 325, "qty": 2 }])
    );
    assert_eq!(
        got["character"]["stash"],
        json!([{ "item": 298, "qty": 1 }])
    );
}

/// `PUT /api/settings` with `hide_record_numbers` switched.
async fn put_hide(app: &axum::Router, hide: bool) {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;
    let get = Request::builder()
        .uri("/api/settings")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(get).await.unwrap();
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    let mut s: Value = serde_json::from_slice(&bytes).unwrap();
    s["hide_record_numbers"] = json!(hide);
    let put = Request::builder()
        .method("PUT")
        .uri("/api/settings")
        .header("content-type", "application/json")
        .body(Body::from(s.to_string()))
        .unwrap();
    let res = app.clone().oneshot(put).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn rows_follow_the_hide_record_numbers_setting() {
    let (_d, app) = app();
    let req = json!({ "character": { "level": 1, "carried": [{ "item": 325, "qty": 1 }] } });
    let (_, shown) = post(&app, "/api/item-manager/rows", &req).await;
    put_hide(&app, true).await;
    let (_, hidden) = post(&app, "/api/item-manager/rows", &req).await;
    // the shop cell is GetShopRoomNames(shop, 0, bHideRecordNumbers)
    let shop = |v: &Value| v["rows"][0]["subs"][9]["text"].as_str().unwrap().to_owned();
    assert_eq!(
        shop(&shown),
        "Junkyard, Entrance (1/168), Portal Room (1/2337)"
    );
    assert_eq!(shop(&hidden), "Junkyard, Entrance, Portal Room");
    // a request that says so overrides the setting
    let mut over = req.clone();
    over["hide_record_numbers"] = json!(false);
    let (_, back) = post(&app, "/api/item-manager/rows", &over).await;
    assert_eq!(shop(&back), shop(&shown));
    // adding an item follows the setting too
    let add = json!({ "character": { "level": 1 }, "rows": [], "item": 325 });
    let (_, added) = post(&app, "/api/item-manager/add", &add).await;
    assert_eq!(shop(&added), "Junkyard, Entrance, Portal Room");
}

#[tokio::test]
async fn paste_rows_follow_the_hide_record_numbers_setting() {
    let (_d, app) = app();
    // a paste with no text still rebuilds the Item Manager from the saved lists
    let req =
        json!({ "character": { "level": 1, "carried": [{ "item": 325, "qty": 1 }] }, "text": "" });
    let shop = |v: &Value| v["rows"][0]["subs"][9]["text"].as_str().unwrap().to_owned();
    let (st, shown) = post(&app, "/api/paste/character", &req).await;
    assert_eq!(st, StatusCode::OK, "{shown}");
    put_hide(&app, true).await;
    let (_, hidden) = post(&app, "/api/paste/character", &req).await;
    assert_eq!(
        shop(&shown),
        "Junkyard, Entrance (1/168), Portal Room (1/2337)"
    );
    assert_eq!(shop(&hidden), "Junkyard, Entrance, Portal Room");
}
