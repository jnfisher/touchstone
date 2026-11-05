//! The touchstone server: JSON API plus the web UI.

pub mod config;
mod web;

use axum::routing::get;
use axum::{Json, Router};
use serde_json::{Value, json};

async fn health() -> Json<Value> {
    Json(json!({ "ok": true, "version": env!("CARGO_PKG_VERSION") }))
}

pub fn app() -> Router {
    Router::new()
        .route("/api/health", get(health))
        .merge(web::router())
}
