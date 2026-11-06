//! `/api/map/...`: the graphical map's layout engine (FEAT-MAP). Card M-001.
//!
//! `GET /api/map` lays rooms out around a start room (`MapStartMapping`), `GET /api/map/go` is a step in a
//! direction (`MapGoDirection`, frmMap `cmdMove_Click`), `GET /api/map/leads-here` is "what leads here"
//! (`cmdMapLeadsHere_Click`), `GET /api/map/presets` the preset buttons (`LoadPresets`) and
//! `GET /api/map/legend` the colour key. The page that draws it is a later card.

use std::sync::Arc;

use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};
use touchstone_rules::map::{
    AlsoMark, LeadsHereOptions, MapError, MapOptions, MapRequest, MapSurface, PRESET_COUNT,
    RoomKey, cmd_move, go_direction, layout, leads_here, legend, presets,
};
use touchstone_rules::rooms::{DIRECTIONS, room_title};

use crate::calc::{ApiResult, bad, json, not_found};
use crate::state::AppState;

/// The checkboxes as a comma list: `follow`, `no_hidden`, `no_lairs`, `no_npcs`, `no_cmds`, `dupes`,
/// `overwrite`, `no_restricted`, and one of `mark_shops` / `mark_spells`.
fn parse_options(list: &str) -> Result<MapOptions, String> {
    let mut o = MapOptions::default();
    for name in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        match name {
            "follow" => o.follow_map_changes = true,
            "no_hidden" => o.no_hidden = true,
            "no_lairs" => o.no_lairs = true,
            "no_npcs" => o.no_npcs = true,
            "no_cmds" => o.no_commands = true,
            "dupes" => o.allow_dupes = true,
            "overwrite" => o.allow_overwrite = true,
            "no_restricted" => o.no_restricted = true,
            "mark_shops" => o.also_mark = AlsoMark::Shops,
            "mark_spells" => o.also_mark = AlsoMark::Spells,
            other => return Err(format!("unknown map option {other:?}")),
        }
    }
    Ok(o)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapQuery {
    pub map: i32,
    pub room: i32,
    /// `main` (the default), `20x20`, `30x30`, `30x50`, `50x50`, `zoom21` or `zoom28`.
    pub size: Option<String>,
    pub options: Option<String>,
    /// MME's cell number to put the start room on.
    pub center: Option<i32>,
}

#[derive(Debug, Serialize)]
struct MapJson {
    title: String,
    #[serde(flatten)]
    grid: touchstone_rules::map::MapGrid,
}

pub async fn map_route(State(state): State<Arc<AppState>>, Query(q): Query<MapQuery>) -> ApiResult {
    let surface = match q.size.as_deref() {
        None => MapSurface::Main,
        Some(s) => MapSurface::parse(s).ok_or_else(|| bad(format!("unknown map size {s:?}")))?,
    };
    let options = parse_options(q.options.as_deref().unwrap_or("")).map_err(bad)?;
    let loaded = state.current();
    let out = tokio::task::spawn_blocking(move || {
        let req = MapRequest {
            surface,
            options,
            center: q.center,
        };
        let start = RoomKey {
            map: q.map,
            room: q.room,
        };
        layout(&loaded.world, start, &req).map(|grid| MapJson {
            title: room_title(&loaded.world, q.map, q.room).unwrap_or_default(),
            grid,
        })
    })
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match out {
        Ok(v) => json(&v),
        Err(e @ MapError::RoomNotFound(..)) => Err(not_found(e.to_string())),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoQuery {
    pub map: i32,
    pub room: i32,
    /// N, S, E, W, NE, NW, SE, SW, U or D.
    pub dir: String,
    /// Use the external window's `cmdMove_Click` rules instead of frmMain's `MapGoDirection`.
    #[serde(default)]
    pub window: bool,
}

pub async fn go_route(State(state): State<Arc<AppState>>, Query(q): Query<GoQuery>) -> ApiResult {
    let loaded = state.current();
    let walk = if q.window {
        let i = DIRECTIONS
            .iter()
            .position(|d| d.eq_ignore_ascii_case(&q.dir))
            .ok_or_else(|| bad(format!("unknown direction {:?}", q.dir)))?;
        cmd_move(&loaded.world, q.map, q.room, i)
    } else {
        go_direction(&loaded.world, q.map, q.room, &q.dir)
    };
    json(&walk)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeadsQuery {
    pub map: i32,
    pub room: i32,
    /// `bOnlyInGame`: skip spells and monsters that are not in the game (the default, as elsewhere).
    pub only_in_game: Option<bool>,
}

pub async fn leads_here_route(
    State(state): State<Arc<AppState>>,
    Query(q): Query<LeadsQuery>,
) -> ApiResult {
    if q.map < 1 || q.room < 1 {
        return Err(bad("map and room must be 1 or more"));
    }
    let loaded = state.current();
    let out = tokio::task::spawn_blocking(move || {
        leads_here(
            &loaded.world,
            q.map,
            q.room,
            &LeadsHereOptions {
                only_in_game: q.only_in_game.unwrap_or(true),
                avg_level_max_all_stats: 0.0,
            },
        )
    })
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    json(&out)
}

#[derive(Debug, Serialize)]
struct PresetsJson {
    /// Presets to a page of buttons (`cmdMapPresetSelect_Click`).
    page_size: usize,
    count: usize,
    presets: Vec<touchstone_rules::map::Preset>,
}

pub async fn presets_route() -> ApiResult {
    json(&PresetsJson {
        page_size: 10,
        count: PRESET_COUNT,
        presets: presets(),
    })
}

pub async fn legend_route() -> ApiResult {
    json(&legend())
}
