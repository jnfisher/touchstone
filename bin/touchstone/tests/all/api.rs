use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::header::{ACCEPT_ENCODING, CONTENT_ENCODING, ETAG, IF_NONE_MATCH};
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use touchstone::config::Config;
use touchstone::source;
use tower::ServiceExt;

use crate::common::{Fixture, add_item};

async fn get(fx: &Fixture, uri: &str) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let res = fx
        .app()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (parts, body) = res.into_parts();
    (
        parts.status,
        parts.headers,
        body.collect().await.unwrap().to_bytes().to_vec(),
    )
}

#[tokio::test]
async fn info_reports_counts_and_source() {
    let fx = Fixture::new();
    let (status, _, body) = get(&fx, "/api/info").await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["source"]["kind"], "silvermere");
    assert_eq!(v["counts"]["items"], 1);
    assert_eq!(v["counts"]["rooms"], 0);
    assert_eq!(v["load_count"], 1);
    assert!(v["loaded_at"].as_str().unwrap().ends_with('Z'));
    assert!(v["data_version"].is_i64());
}

#[tokio::test]
async fn every_table_is_served_as_a_json_array() {
    let fx = Fixture::new();
    for t in [
        "items", "monsters", "spells", "rooms", "shops", "classes", "races", "lairs",
    ] {
        let (status, headers, body) = get(&fx, &format!("/api/{t}")).await;
        assert_eq!(status, StatusCode::OK, "{t}");
        assert_eq!(headers["content-type"], "application/json");
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert!(v.is_array(), "{t}");
    }
    let (_, _, body) = get(&fx, "/api/items").await;
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v[0]["number"], 7);
    assert_eq!(v[0]["name"], "test blade");
}

#[tokio::test]
async fn etag_revalidates_with_304() {
    let fx = Fixture::new();
    let (_, headers, _) = get(&fx, "/api/items").await;
    let etag = headers[ETAG].to_str().unwrap().to_owned();
    let res = fx
        .app()
        .oneshot(
            Request::get("/api/items")
                .header(IF_NONE_MATCH, &etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_MODIFIED);
    let res = fx
        .app()
        .oneshot(
            Request::get("/api/items")
                .header(IF_NONE_MATCH, "\"other\"")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn responses_are_gzipped_on_request() {
    let fx = Fixture::new();
    for n in 100..160 {
        add_item(&fx.writer, n, "a long repeated item name for compression");
    }
    assert!(fx.live.poll(&fx.state).unwrap());
    let res = fx
        .app()
        .oneshot(
            Request::get("/api/items")
                .header(ACCEPT_ENCODING, "gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.headers()[CONTENT_ENCODING], "gzip");
}

#[tokio::test]
async fn poll_picks_up_changes_and_changes_the_etag() {
    let fx = Fixture::new();
    assert!(!fx.live.poll(&fx.state).unwrap(), "nothing changed yet");
    let (_, h1, _) = get(&fx, "/api/items").await;
    add_item(&fx.writer, 8, "new charm");
    assert!(fx.live.poll(&fx.state).unwrap());
    assert!(!fx.live.poll(&fx.state).unwrap());
    let (_, h2, body) = get(&fx, "/api/items").await;
    assert_ne!(h1[ETAG], h2[ETAG]);
    assert_eq!(
        serde_json::from_slice::<Value>(&body)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let (_, _, info) = get(&fx, "/api/info").await;
    let v: Value = serde_json::from_slice(&info).unwrap();
    assert_eq!(v["counts"]["items"], 2);
    assert_eq!(v["load_count"], 2);
}

#[tokio::test]
async fn background_task_reloads() {
    let fx = Fixture::new();
    source::spawn_reload(fx.live.clone(), fx.state.clone(), Duration::from_millis(20));
    add_item(&fx.writer, 8, "new charm");
    let deadline = Instant::now() + Duration::from_secs(10);
    while fx.state.current().info.counts[0].1 != 2 {
        assert!(Instant::now() < deadline, "no reload within 10 s");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn failed_rebuild_keeps_the_old_world() {
    let fx = Fixture::new();
    // Losing a table the loader reads makes the rebuild fail.
    fx.writer.execute_batch("DROP TABLE races").unwrap();
    let before = fx.state.current().info.load_count;
    assert!(fx.live.poll(&fx.state).is_err());
    assert_eq!(fx.state.current().info.load_count, before);
    let (status, _, body) = get(&fx, "/api/items").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()[0]["number"],
        7
    );
}

#[test]
fn start_needs_a_source_and_explains_mdb_limits() {
    let err = source::start(&Config::default()).err().unwrap().to_string();
    assert!(err.contains("[source]"), "{err}");
    let fx = Fixture::new();
    let mut cfg = Config::default();
    let mut s = crate::common::config_for(&fx.path);
    s.kind = touchstone::config::SourceKind::Mdb;
    s.path = fx.dir.path().join("x.txt");
    cfg.source = Some(s);
    let err = source::start(&cfg).err().unwrap().to_string();
    assert!(err.contains(".mdb") && err.contains(".sqlite"), "{err}");
}
