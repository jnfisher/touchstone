//! Reachability for the loaded database (`Options > Calculate Reachability`, `ReachRecompute`).
//!
//! MME computes once per database and keeps the result until another loads; here the result lives in the
//! [`Loaded`](crate::state::Loaded) it was computed from, so a reload drops it. `POST /api/reachability/compute`
//! (re)computes, `GET /api/reachability/status` says whether there is a result. The Monsters list's
//! "Reachable only" filter computes on demand (as `ReachIsMonsterOK` does); the monster pane shows the
//! "double-click to compute" row until the user asks.

use std::sync::{Arc, Mutex};

use axum::extract::State;
use serde::Serialize;
use touchstone_rules::reachability::{Reach, ReachStats, compute_world};

use crate::calc::{ApiResult, json};
use crate::state::AppState;

/// The computed reachability of one load.
#[derive(Default)]
pub struct ReachSlot(Mutex<Option<Arc<Reach>>>);

impl ReachSlot {
    /// The result, if one was computed.
    pub fn get(&self) -> Option<Arc<Reach>> {
        self.0.lock().ok()?.clone()
    }

    /// The result, computing it first when there is none.
    pub fn get_or_compute(&self, world: &touchstone_model::World) -> Arc<Reach> {
        if let Some(r) = self.get() {
            return r;
        }
        self.recompute(world)
    }

    /// `ReachRecompute`: drop the result and compute again.
    pub fn recompute(&self, world: &touchstone_model::World) -> Arc<Reach> {
        let r = Arc::new(compute_world(world));
        if let Ok(mut g) = self.0.lock() {
            *g = Some(r.clone());
        }
        r
    }
}

#[derive(Serialize)]
struct StatusJson {
    computed: bool,
    rooms: usize,
    edges: usize,
    random_groups: usize,
    level_bands: usize,
    monsters: usize,
    located: usize,
}

fn status_of(stats: Option<ReachStats>) -> StatusJson {
    match stats {
        Some(s) => StatusJson {
            computed: true,
            rooms: s.rooms,
            edges: s.edges,
            random_groups: s.random_groups,
            level_bands: s.level_bands,
            monsters: s.monsters,
            located: s.located,
        },
        None => StatusJson {
            computed: false,
            rooms: 0,
            edges: 0,
            random_groups: 0,
            level_bands: 0,
            monsters: 0,
            located: 0,
        },
    }
}

/// `GET /api/reachability/status`.
pub async fn status_route(State(state): State<Arc<AppState>>) -> ApiResult {
    let loaded = state.current();
    json(&status_of(loaded.reach.get().map(|r| r.stats())))
}

/// `POST /api/reachability/compute`: computes (again) for the loaded database.
pub async fn compute_route(State(state): State<Arc<AppState>>) -> ApiResult {
    let loaded = state.current();
    let stats = tokio::task::spawn_blocking(move || loaded.reach.recompute(&loaded.world).stats())
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    json(&status_of(Some(stats)))
}
