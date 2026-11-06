//! `/api/info` and the table endpoints.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_NONE_MATCH};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde_json::json;

use crate::state::AppState;
use crate::{
    calc, character, class_race_builder, clipboard, combat_setup, compare, detail, equip, evaluate,
    item_manager, monster_damage, monster_sim, panes, paste, reach, rooms, route, settings,
    spell_tools,
};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/info", get(info))
        .route("/api/settings", get(settings::get).put(settings::put))
        .route("/api/exp", get(calc::exp_table))
        .route("/api/calc/exp", post(calc::calc_exp))
        .route("/api/calc/coin", post(calc::calc_coin))
        .route("/api/calc/swing", post(calc::calc_swing))
        .route("/api/calc/mega-stat", post(calc::calc_mega_stat))
        .route("/api/calc/backstab", post(calc::calc_backstab))
        .route("/api/calc/hit", post(calc::calc_hit))
        .route("/api/calc/monster-sim", post(monster_sim::sim_route))
        .route(
            "/api/calc/monster-sim/defender",
            post(monster_sim::defender_route),
        )
        .route(
            "/api/detail/{kind}/{number}",
            get(panes::get_route).post(panes::post_route),
        )
        .route(
            "/api/class-race-builder",
            get(class_race_builder::build_route),
        )
        .route(
            "/api/class-race-builder/equip",
            post(class_race_builder::equip_route),
        )
        .route(
            "/api/class-race-builder/spellbook",
            post(class_race_builder::spellbook_route),
        )
        .route(
            "/api/monster-damage/calculate",
            post(monster_damage::calculate_route),
        )
        .route(
            "/api/monster-damage/clear",
            post(monster_damage::clear_route),
        )
        .route(
            "/api/monster-damage/dismiss",
            post(monster_damage::dismiss_route),
        )
        .route(
            "/api/monster-damage/status",
            get(monster_damage::status_route),
        )
        .route("/api/shops/{number}/prices", get(calc::shop_prices))
        .route("/api/abilities", get(detail::ability_list))
        .route("/api/abilities/{table}/{number}", get(detail::abilities))
        .route("/api/spells/{number}/effect", get(detail::spell_effect))
        .route("/api/spells/{number}/casts", get(spell_tools::casts_route))
        .route("/api/spells/counters", post(spell_tools::counters_route))
        .route("/api/items/{number}/chest", get(spell_tools::chest_route))
        .route("/api/spell-book/list", post(spell_tools::book_route))
        .route("/api/spell-book/class", post(spell_tools::class_route))
        .route("/api/spell-book/copy-global", post(spell_tools::copy_route))
        .route(
            "/api/monsters/{number}/max-damage",
            get(detail::monster_max_damage),
        )
        .route("/api/character/evaluate", post(character::evaluate_route))
        .route(
            "/api/character/bless-spells",
            get(character::bless_spells_route),
        )
        .route(
            "/api/character/combat-options",
            post(combat_setup::options_route),
        )
        .route(
            "/api/character/combat-preview",
            post(combat_setup::preview_route),
        )
        .route("/api/character/equip", post(equip::equip_route))
        .route("/api/character/equip-state", post(equip::equip_state_route))
        .route("/api/character/find-best", post(equip::find_best_route))
        .route("/api/item-manager/rows", post(item_manager::rows_route))
        .route("/api/item-manager/action", post(item_manager::action_route))
        .route("/api/item-manager/delete", post(item_manager::delete_route))
        .route("/api/item-manager/copy", post(item_manager::copy_route))
        .route("/api/item-manager/add", post(item_manager::add_route))
        .route("/api/reachability/status", get(reach::status_route))
        .route("/api/reachability/compute", post(reach::compute_route))
        .route("/api/reachability/route", post(route::route_route))
        .route("/api/paste/character", post(paste::character_route))
        .route("/api/paste/party", post(paste::party_route))
        .route(
            "/api/paste/party/continue",
            post(paste::party_continue_route),
        )
        .route("/api/rooms/find-text", get(rooms::find_text_route))
        .route("/api/rooms/find-exits", post(rooms::find_exits_route))
        .route("/api/rooms/cmd-rooms", post(rooms::cmd_rooms_route))
        .route("/api/rooms/{map}/{room}", get(rooms::room_route))
        .route("/api/clipboard/character", post(clipboard::character_route))
        .route("/api/clipboard/monster", post(clipboard::monster_route))
        .route("/api/clipboard/shop", post(clipboard::shop_route))
        .route("/api/clipboard/lines", post(clipboard::lines_route))
        .route("/api/compare/classify", post(compare::classify_route))
        .route("/api/evaluate/{tab}", post(evaluate::evaluate_route))
        .route("/api/characters", get(character::list))
        .route("/api/characters/import", post(character::import))
        .route("/api/characters/{name}/export.mmec", get(character::export))
        .route(
            "/api/characters/{name}",
            get(character::get)
                .put(character::put)
                .delete(character::delete),
        )
        .route("/api/{table}", get(table))
        .with_state(state)
}

async fn info(State(state): State<Arc<AppState>>) -> Response {
    let loaded = state.current();
    let mut v = serde_json::to_value(&loaded.info).unwrap_or_else(|_| json!({}));
    // Counts read better as an object than as pairs.
    v["counts"] = loaded
        .info
        .counts
        .iter()
        .map(|(k, n)| (k.clone(), json!(n)))
        .collect::<serde_json::Map<_, _>>()
        .into();
    ([(CACHE_CONTROL, "no-cache")], axum::Json(v)).into_response()
}

async fn table(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Response {
    let loaded = state.current();
    let Some(t) = loaded.table(&name) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // `no-cache` means "revalidate", so the browser sends the tag and gets a 304 until the
    // data changes.
    let common = [
        (ETAG, t.etag.clone()),
        (CACHE_CONTROL, "no-cache".to_owned()),
    ];
    if matches(&headers, &t.etag) {
        return (StatusCode::NOT_MODIFIED, common).into_response();
    }
    (common, [(CONTENT_TYPE, "application/json")], t.body.clone()).into_response()
}

fn matches(headers: &HeaderMap, etag: &str) -> bool {
    let Some(v) = headers.get(IF_NONE_MATCH).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    v.split(',')
        .map(|s| s.trim().trim_start_matches("W/"))
        .any(|s| s == "*" || s == etag)
}
