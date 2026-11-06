//! The touchstone server: JSON API plus the web UI.

mod api;
pub mod calc;
pub mod character;
pub mod class_race_builder;
pub mod clipboard;
pub mod combat_setup;
pub mod compare;
pub mod config;
mod detail;
pub mod equip;
pub mod evaluate;
pub mod item_manager;
pub mod monster_sim;
pub mod panes;
pub mod paste;
pub mod settings;
pub mod source;
pub mod state;
mod web;

use std::sync::Arc;

use axum::routing::get;
use axum::{Json, Router};
use serde_json::{Value, json};
use tower_http::compression::CompressionLayer;

pub use state::AppState;

async fn health() -> Json<Value> {
    Json(json!({ "ok": true, "version": env!("CARGO_PKG_VERSION") }))
}

pub fn app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .merge(api::router(state))
        .merge(web::router())
        // Whole tables are large and repetitive; gzip/brotli cut them by an order of magnitude.
        .layer(CompressionLayer::new())
}
