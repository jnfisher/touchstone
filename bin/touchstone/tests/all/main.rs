mod api;
mod calc;
mod character;
mod combat_setup;
mod common;
mod detail;
mod equip;
mod evaluate;
mod find_best;
mod item_manager;
mod monster_sim;
mod monsters;
mod panes;
mod paste;
mod rooms;
mod settings;

mod health {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use crate::common::Fixture;

    #[tokio::test]
    async fn health_answers() {
        let fx = Fixture::new();
        let res = fx
            .app()
            .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["ok"], true);
        assert!(v["version"].is_string());
    }

    #[tokio::test]
    async fn unknown_api_path_is_404() {
        let fx = Fixture::new();
        let res = fx
            .app()
            .oneshot(Request::get("/api/nope").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
}
