//! The Chests tab: `GET /api/chests` and `GET /api/chests/{item}` (card CH-001).
//!
//! The rules are `touchstone_rules::chests`; this module reads the tables, prices at the requested charm
//! (50 by default, as the tab's box starts) and shapes the JSON. Sale values are a touchstone addition,
//! not MME numbers; docs/differences.md says how they are worked out.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use touchstone_rules::chests::{ChestContent, chest_detail, chest_summary};
use touchstone_rules::item_value::friendly_copper;
use touchstone_rules::locations::LocationOptions;

use crate::calc::{ApiResult, bad, json, not_found};
use crate::detail::nmr_ver;
use crate::panes::{RefJson, StoredLairs, references};
use crate::state::AppState;

/// The charm the average sale is worked out at unless the request says otherwise.
const DEFAULT_CHARM: i32 = 50;

#[derive(Debug, Deserialize, Default)]
pub struct ChestQuery {
    pub charm: Option<i32>,
}

fn charm_of(q: &ChestQuery) -> Result<i16, (StatusCode, String)> {
    // The game's charm is an Integer, as on the shop prices route.
    let charm = q.charm.unwrap_or(DEFAULT_CHARM);
    i16::try_from(charm)
        .ok()
        .filter(|c| *c >= 0)
        .ok_or_else(|| bad("charm must be between 0 and 32767"))
}

fn fail(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

#[derive(Debug, Serialize)]
pub struct SummaryJson {
    pub item: i32,
    pub name: String,
    pub contents: usize,
    pub average_copper: f64,
    pub average: String,
    pub average_short: String,
    pub dropped_by: usize,
}

#[derive(Debug, Serialize)]
pub struct ListJson {
    pub charm: i16,
    pub chests: Vec<SummaryJson>,
}

pub async fn list_route(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ChestQuery>,
) -> ApiResult {
    let charm = charm_of(&q)?;
    let loaded = state.current();
    let rows = chest_summary(&loaded.world, loaded.profile, charm).map_err(fail)?;
    let mut chests = Vec::with_capacity(rows.len());
    for r in rows {
        let (average, average_short) = friendly_copper(r.average_copper).map_err(fail)?;
        chests.push(SummaryJson {
            item: r.item,
            name: r.name,
            contents: r.contents,
            average_copper: r.average_copper,
            average,
            average_short,
            dropped_by: r.dropped_by,
        });
    }
    json(&ListJson { charm, chests })
}

#[derive(Debug, Serialize)]
pub struct ContentJson {
    pub item: i32,
    pub name: String,
    /// The chance in percent as MME shows it, and its text.
    pub percent: f64,
    pub percent_text: String,
    pub sellable: bool,
    pub sell_copper: f64,
    pub sell: String,
    pub sell_short: String,
    pub expected_copper: f64,
    pub expected: String,
    pub expected_short: String,
}

fn content_json(c: ChestContent) -> Result<ContentJson, (StatusCode, String)> {
    let (expected, expected_short) = friendly_copper(c.expected_copper).map_err(fail)?;
    Ok(ContentJson {
        item: c.item,
        name: c.name,
        percent: c.percent.scaled() as f64 / 10_000.0,
        percent_text: c.percent_text,
        sellable: c.sale.sellable,
        sell_copper: c.sale.copper,
        sell: c.sale.text,
        sell_short: c.sale.short,
        expected_copper: c.expected_copper,
        expected,
        expected_short,
    })
}

#[derive(Debug, Serialize)]
pub struct DetailJson {
    pub item: i32,
    pub name: String,
    pub charm: i16,
    pub contents: Vec<ContentJson>,
    pub average_copper: f64,
    pub average: String,
    pub average_short: String,
    /// The Items pane's reference rows for the chest: monsters first among equals by chance, with rooms,
    /// textblocks and NPCs as other sources.
    pub droppers: Vec<RefJson>,
}

pub async fn detail_route(
    State(state): State<Arc<AppState>>,
    Path(number): Path<i32>,
    Query(q): Query<ChestQuery>,
) -> ApiResult {
    let charm = charm_of(&q)?;
    let loaded = state.current();
    let world = &loaded.world;
    let opt = LocationOptions {
        nmr_ver: nmr_ver(&world.info.nmr_version),
        hide_record_numbers: state.hide_record_numbers(),
        global_filter: false,
        charm_tag: String::new(),
        attack_type: 0,
    };
    let mut lairs = StoredLairs(loaded.lair_cache());
    let d = chest_detail(world, loaded.profile, &mut lairs, &opt, number, charm)
        .map_err(fail)?
        .ok_or_else(|| not_found(format!("item {number} is not a chest")))?;
    let (average, average_short) = friendly_copper(d.average_copper).map_err(fail)?;
    let (droppers, _) = references(d.droppers, Some(false));
    json(&DetailJson {
        item: d.item,
        name: d.name,
        charm,
        contents: d
            .contents
            .into_iter()
            .map(content_json)
            .collect::<Result<_, _>>()?,
        average_copper: d.average_copper,
        average,
        average_short,
        droppers,
    })
}
