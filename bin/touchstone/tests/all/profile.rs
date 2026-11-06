//! Which rules a load follows: `/api/info`, the `[rules]` config, a reload that meets an unknown
//! content version, and a real Quicksilver database when the cache has one.
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use touchstone::config::Config;
use touchstone::profile::{ProfileSetting, RulesConfig};
use touchstone::source;
use touchstone_rules::Profile;
use touchstone_rules::quicksilver::Balance;
use tower::ServiceExt;

use crate::common::{Fixture, config_for};

async fn info(state: &std::sync::Arc<touchstone::AppState>) -> Value {
    let res = touchstone::app(state.clone())
        .oneshot(Request::get("/api/info").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

/// Makes the fixture database a Quicksilver one (migration 9001) with this content version.
fn set_content_version(fx: &Fixture, version: &str) {
    let has_column = fx
        .writer
        .prepare("SELECT 1 FROM pragma_table_info('world_import') WHERE name = 'content_version'")
        .unwrap()
        .exists([])
        .unwrap();
    if !has_column {
        fx.writer
            .execute_batch(
                "ALTER TABLE world_import ADD COLUMN content_version TEXT; \
                 INSERT INTO _sqlx_migrations (version, description, success) VALUES (9001,'m',1); \
                 INSERT INTO world_import (id, started_at, finished_at) \
                 VALUES (1, '2026-10-14T09:00:00Z', '2026-10-14T09:30:15Z');",
            )
            .unwrap();
    }
    fx.writer
        .execute("UPDATE world_import SET content_version = ?", [version])
        .unwrap();
}

#[tokio::test]
async fn a_plain_silvermere_database_is_stock() {
    let fx = Fixture::new();
    let v = info(&fx.state).await;
    assert_eq!(v["profile"]["name"], "stock");
    assert_eq!(v["profile"]["setting"], "auto");
    assert!(v["profile"]["content_version"].is_null());
    assert_eq!(v["profile"]["balance_overrides"], serde_json::json!([]));
    assert_eq!(fx.state.current().profile, Profile::Stock);
}

#[tokio::test]
async fn a_reload_picks_the_profile_and_an_unknown_version_keeps_the_world() {
    let fx = Fixture::new();
    set_content_version(&fx, "1.20.0");
    assert!(fx.live.poll(&fx.state).unwrap());
    assert_eq!(fx.state.current().profile, Profile::quicksilver_v120());
    let v = info(&fx.state).await;
    assert_eq!(v["profile"]["name"], "quicksilver");
    assert_eq!(v["profile"]["content_version"], "1.20.0");
    assert_eq!(v["version"]["dat_file_version"], "1.20.0");
    assert_eq!(v["load_count"], 2);

    // 1.20.1 reports 1.20.0, but any 1.20.N is the same rules.
    set_content_version(&fx, "1.20.1");
    assert!(fx.live.poll(&fx.state).unwrap());
    assert_eq!(fx.state.current().profile, Profile::quicksilver_v120());

    // A version without rules: the old world stays, the reason comes back once, and the next poll
    // does not repeat it.
    set_content_version(&fx, "1.21.0");
    let err = format!("{:#}", fx.live.poll(&fx.state).unwrap_err());
    assert!(err.contains("1.21.0") && err.contains("1.20.N"), "{err}");
    assert_eq!(fx.state.current().info.load_count, 3);
    assert_eq!(
        info(&fx.state).await["profile"]["content_version"],
        "1.20.1"
    );
    assert!(!fx.live.poll(&fx.state).unwrap());
}

#[test]
fn startup_refuses_an_unknown_content_version() {
    let fx = Fixture::new();
    set_content_version(&fx, "2.0.0");
    let cfg = Config {
        source: Some(config_for(&fx.path)),
        ..Config::default()
    };
    let err = format!("{:#}", source::start(&cfg).err().unwrap());
    assert!(err.contains("2.0.0"), "{err}");
    // `stock` ignores Quicksilver's rules, so it does not need a known version.
    let cfg = Config {
        rules: RulesConfig {
            profile: ProfileSetting::Stock,
            ..RulesConfig::default()
        },
        ..cfg
    };
    let started = source::start(&cfg).unwrap();
    assert_eq!(started.state.current().profile, Profile::Stock);
}

#[tokio::test]
async fn the_rules_section_forces_a_profile_and_reports_an_override() {
    let fx = Fixture::new();
    let cfg = Config {
        source: Some(config_for(&fx.path)),
        rules: toml::from_str(
            "profile = \"quicksilver\"\n[quicksilver_balance]\ncrit_knee = 55\nweave_pct = 10\n",
        )
        .unwrap(),
        ..Config::default()
    };
    let started = source::start(&cfg).unwrap();
    let loaded = started.state.current();
    let b = loaded.profile.balance().expect("quicksilver");
    assert_eq!((b.crit_knee, b.weave_pct), (55, 10));
    assert_eq!(b.scalecap, Balance::V120.scalecap);
    let v = info(&started.state).await;
    assert_eq!(v["profile"]["name"], "quicksilver");
    assert_eq!(v["profile"]["setting"], "quicksilver");
    assert_eq!(
        v["profile"]["balance_overrides"],
        serde_json::json!(["crit_knee", "weave_pct"])
    );
    // The same override survives a reload without leaking a new copy.
    let live = started.live.unwrap();
    fx.writer
        .execute_batch("UPDATE items SET name = 'x'")
        .unwrap();
    assert!(live.poll(&started.state).unwrap());
    assert!(std::ptr::eq(
        b,
        started.state.current().profile.balance().unwrap()
    ));
}

#[test]
fn a_bad_balance_override_fails_startup() {
    let fx = Fixture::new();
    let cfg = Config {
        source: Some(config_for(&fx.path)),
        rules: toml::from_str("[quicksilver_balance]\nbackstab_stealth_div = 0\n").unwrap(),
        ..Config::default()
    };
    let err = format!("{:#}", source::start(&cfg).err().unwrap());
    assert!(err.contains("backstab_stealth_div"), "{err}");
}

#[tokio::test]
async fn the_real_quicksilver_database_runs_the_1_20_rules() {
    // Built by tools/qs-fetch.sh and never committed; without it the test says so and passes.
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let path = std::path::PathBuf::from(home).join(".cache/touchstone/quicksilver-1.20.sqlite");
    if !path.is_file() {
        eprintln!("skipped: no Quicksilver database at {}", path.display());
        return;
    }
    let cfg = Config {
        source: Some(config_for(&path)),
        ..Config::default()
    };
    let started = source::start(&cfg).unwrap();
    assert_eq!(started.state.current().profile, Profile::quicksilver_v120());
    let v = info(&started.state).await;
    assert_eq!(v["profile"]["name"], "quicksilver");
    assert_eq!(v["profile"]["content_version"], "1.20.0");

    // `stock` on the same database, for comparisons.
    let cfg = Config {
        rules: RulesConfig {
            profile: ProfileSetting::Stock,
            ..RulesConfig::default()
        },
        ..cfg
    };
    let started = source::start(&cfg).unwrap();
    assert_eq!(started.state.current().profile, Profile::Stock);
}
