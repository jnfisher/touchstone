//! The Chests tab's routes over the stock database (card CH-001). The chances are what MME's chest button
//! lists (fixtures/rules/chests.json pins them for chest 1801); the sale values are touchstone's own, worked
//! by hand in the comments below from the stock prices.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tempfile::TempDir;
use touchstone::state::{Loaded, SourceInfo};
use touchstone::{AppState, app};
use tower::ServiceExt;

fn stock_sqlite() -> std::path::PathBuf {
    if let Some(p) = std::env::var_os("TOUCHSTONE_STOCK_SQLITE") {
        return p.into();
    }
    let home = std::env::var_os("HOME").expect("HOME is not set");
    std::path::PathBuf::from(home).join(
        ".cache/touchstone/mdb/eba20f06af8dc08df755eb6dcd9763b023794f12e809cbab6762236d64d7345f.sqlite",
    )
}

fn serve() -> (axum::Router, TempDir) {
    let path = stock_sqlite();
    assert!(
        path.is_file(),
        "the stock SQLite is missing at {}: run tools/mdb-fetch.sh or set TOUCHSTONE_STOCK_SQLITE",
        path.display()
    );
    let world = touchstone_data::mdb::load(&path).expect("load stock database");
    let dir = TempDir::new().unwrap();
    let info = SourceInfo {
        kind: "mdb",
        path: "stock".into(),
    };
    let loaded = Loaded::build(world, info, None, 1).unwrap();
    (
        app(AppState::with_data_dir(loaded, dir.path().join("data"))),
        dir,
    )
}

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, v)
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap()
}

#[tokio::test]
async fn the_list_holds_every_chest_item_with_its_numbers() {
    let (app, _dir) = serve();
    let (status, v) = get(&app, "/api/chests").await;
    assert_eq!(status, StatusCode::OK);
    // the default charm is 50
    assert_eq!(v["charm"], 50);
    let chests = v["chests"].as_array().unwrap();
    // the stock database has 28 items of type 8
    assert_eq!(chests.len(), 28);
    let by = |n: i64| chests.iter().find(|c| c["item"] == n).unwrap();

    // 960 "small cloth pouch" only gives from a one-line text with no chances: no lines, no average; an NPC
    // hands it out, so no monster carries it
    let pouch = by(960);
    assert_eq!(pouch["contents"], 0);
    assert_eq!(f(&pouch["average_copper"]), 0.0);
    assert_eq!(pouch["dropped_by"], 0);

    // 1801 "wooden box" (Room 17/2590 only): 4 lines; see the detail test for the average
    let wooden = by(1801);
    assert_eq!(wooden["contents"], 4);
    assert_eq!(f(&wooden["average_copper"]), 30430.0);
    assert_eq!(wooden["dropped_by"], 0);

    // 909 "iron-banded chest": Obtained From lists Monster #393, #394, #456 and #1058 (and a room)
    assert_eq!(by(909)["dropped_by"], 4);
    // 906 and 1801 share a name; the number tells them apart
    assert_eq!(by(906)["name"], by(1801)["name"]);
}

#[tokio::test]
async fn a_stock_chest_worked_by_hand() {
    let (app, _dir) = serve();
    let (status, v) = get(&app, "/api/chests/1801?charm=50").await;
    assert_eq!(status, StatusCode::OK);
    let lines: Vec<(i64, f64, bool, f64, f64)> = v["contents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["item"].as_i64().unwrap(),
                f(&c["percent"]),
                c["sellable"].as_bool().unwrap(),
                f(&c["sell_copper"]),
                f(&c["expected_copper"]),
            )
        })
        .collect();
    // Chances (fixture chests.json, case chest-1801): 409 62.5%, 1656 54.4%, 400 54.4%, 1666 23.5%. Their
    // unrounded values come from chaining the chest's 12 draws (four `random 9645`, each rolling `random 9646`
    // three times at half weight): 0.6247, 0.5442, 0.5442, 0.2352.
    // Prices (stock): 409 holy vestements 800 gold and 400 holy miter 200 gold, both in Shop #68; at charm 50 a
    // shop pays fix(50/2)+25 = 50 percent: 40,000 and 10,000 copper. 1656 morningstar ring and 1666 golden sun
    // robes have no shop in their Obtained From, so they can't be sold.
    // Average: 0.6247 * 40,000 + 0.5442 * 10,000 = 24,988 + 5,442 = 30,430.
    assert_eq!(
        lines,
        [
            (409, 62.5, true, 40_000.0, 24_988.0),
            (1656, 54.4, false, 0.0, 0.0),
            (400, 54.4, true, 10_000.0, 5_442.0),
            (1666, 23.5, false, 0.0, 0.0),
        ]
    );
    assert_eq!(v["contents"][1]["sell"], "can't be sold");
    assert_eq!(v["contents"][0]["sell_short"], "400G");
    assert_eq!(f(&v["average_copper"]), 30_430.0);
    assert_eq!(v["average_short"], "304G");
    // the only source is a room
    let d = v["droppers"].as_array().unwrap();
    assert_eq!(d.len(), 1);
    assert_eq!(d[0]["kind"], "room");
    assert_eq!(d[0]["target"]["key"], "17/2590");

    // charm 0: a shop pays the price itself (no charm modifier): 0.6247 * 80,000 + 0.5442 * 20,000
    // = 49,976 + 10,884 = 60,860
    let (_, v) = get(&app, "/api/chests/1801?charm=0").await;
    assert_eq!(f(&v["average_copper"]), 60_860.0);
}

#[tokio::test]
async fn droppers_are_sorted_by_chance_with_other_sources() {
    let (app, _dir) = serve();
    let (_, v) = get(&app, "/api/chests/909").await;
    // Obtained From (stock): Room 8/707, Monster #393(15%), #394(100%), #456(50%), #1058(50%)
    let rows: Vec<(String, String, Option<i64>)> = v["droppers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["kind"].as_str().unwrap().to_owned(),
                r["text"].as_str().unwrap().to_owned(),
                r["target"]["number"].as_i64(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("monster".to_owned(), "100%".to_owned(), Some(394)),
            ("monster".to_owned(), "50%".to_owned(), Some(456)),
            ("monster".to_owned(), "50%".to_owned(), Some(1058)),
            ("monster".to_owned(), "15%".to_owned(), Some(393)),
            ("room".to_owned(), String::new(), Some(0)),
        ]
    );
}

#[tokio::test]
async fn bad_requests_are_refused() {
    let (app, _dir) = serve();
    // item 1 is not a chest; 99999 does not exist
    assert_eq!(get(&app, "/api/chests/1").await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        get(&app, "/api/chests/99999").await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(&app, "/api/chests?charm=-1").await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get(&app, "/api/chests/909?charm=40000").await.0,
        StatusCode::BAD_REQUEST
    );
}
