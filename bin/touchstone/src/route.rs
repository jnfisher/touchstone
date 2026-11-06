//! `POST /api/reachability/route`: the route dialog behind the Rooms tab's "Route to Room" and the monster pane's
//! "Reachable" row (`ShowRoomRoute`, `ShowMonsterRoute`, `ShowRouteResults`). The server builds the dialog's caption
//! and rows exactly as MME prints them; the page only lists them and opens the room a row points at.

use std::sync::Arc;

use axum::extract::State;
use serde::{Deserialize, Serialize};
use touchstone_model::World;
use touchstone_rules::ability_text::{
    get_class_name, get_item_name, get_monster_name, get_spell_name,
};
use touchstone_rules::reachability::{
    Reach, ReachStatus, Route, get_room_name, route_coin_text, route_kind_text,
};
use touchstone_rules::textblock::extract_value;

use crate::calc::{ApiResult, Body, bad, json};
use crate::detail::WorldNames;
use crate::state::AppState;

/// What to route to: a monster, or a room (`map` and `room`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteReq {
    pub monster: Option<i32>,
    pub map: Option<i32>,
    pub room: Option<i32>,
    /// `bAltRoute`: the alternate route of a monster.
    #[serde(default)]
    pub alt: bool,
    /// `txtGlobalLevel(0).Text`.
    pub level: i32,
    /// Left out, the `hide_record_numbers` setting decides.
    pub hide_record_numbers: Option<bool>,
}

/// One list row: `Step`, `Location`, and the room the row points at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RouteRow {
    pub step: i32,
    pub text: String,
    pub map: i32,
    pub room: i32,
}

/// The dialog, or the message MME shows in a box instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RouteDialog {
    pub caption: String,
    pub rows: Vec<RouteRow>,
    pub message: Option<String>,
}

impl RouteDialog {
    fn message(m: &str) -> Self {
        Self {
            caption: String::new(),
            rows: Vec::new(),
            message: Some(m.to_owned()),
        }
    }
}

/// `nRouteWaypointInterval` (below 1 means 25) and `bHideRecordNumbers`.
#[derive(Debug, Clone, Copy)]
pub struct RouteOptions {
    pub waypoint_interval: i32,
    pub hide_record_numbers: bool,
}

fn steps_word(n: i32) -> &'static str {
    if n == 1 { "step" } else { "steps" }
}

/// `RouteCaptionText`.
fn caption_text(
    steps: i32,
    source: &str,
    ignore_level: bool,
    coin: i32,
    alt_steps: i32,
    alt_coin: i32,
    alt_source: &str,
) -> String {
    let mut s = format!(": {steps} {} from {source}", steps_word(steps));
    if ignore_level {
        s.push_str(" (ignoring level gates)");
    }
    if coin > 0 {
        s.push_str(&format!(", {}", route_coin_text(coin)));
    }
    if alt_steps > 0 {
        s.push_str(&format!(
            "; {}: {alt_steps} {} from {alt_source}",
            route_kind_text(alt_coin, coin).to_lowercase(),
            steps_word(alt_steps)
        ));
    }
    s
}

/// `MonsterRouteRoomText`: the name without its number, then the number.
fn room_text(world: &World, map: i32, room: i32) -> String {
    let names = |m: i32, r: i32| {
        world
            .get_room(m, r)
            .map(|x| x.name.clone().unwrap_or_default())
    };
    format!("{} ({map}/{room})", get_room_name(&names, map, room, true))
}

/// `RouteLabelNames`: every `prefix` followed by one to nine digits has the number replaced by what `name` makes of it.
fn label_names(mut s: String, prefix: &str, name: &dyn Fn(i32) -> String) -> String {
    let mut from = 0;
    while let Some(off) = s.get(from..).and_then(|t| t.find(prefix)) {
        let x = from + off;
        let digits_at = x + prefix.len();
        let digits = s[digits_at..]
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
        if (1..=9).contains(&digits) {
            let n: i32 = s[digits_at..digits_at + digits].parse().unwrap_or(0);
            let replacement = name(n);
            s.replace_range(digits_at..digits_at + digits, &replacement);
            from = digits_at + replacement.len();
        } else {
            from = digits_at;
        }
    }
    s
}

/// `FormatRouteLabel`: the step label with its spell, item, toll and class/race numbers turned into names.
#[must_use]
pub fn format_route_label(world: &World, label: &str, hide: bool) -> String {
    let names = WorldNames(world);
    let item = |n: i32| get_item_name(&names, n, hide);
    let spell = |n: i32| {
        if n == 0 {
            "none".to_owned()
        } else {
            get_spell_name(&names, n, hide)
        }
    };
    let toll = |n: i32| format!("{} gold", group_thousands(n));
    let mut s = label.to_owned();
    s = label_names(s, "spell #", &spell);
    s = label_names(s, "item #", &item);
    s = label_names(s, "(Key: ", &item);
    s = label_names(s, "(Item: ", &item);
    s = label_names(s, "(Ticket/Item: ", &item);
    s = label_names(s, "(Spell Trap: ", &spell);
    s = label_names(s, "(Toll: ", &toll);
    if s.contains("(Cast: ") {
        s = label_names(s, "pre-", &spell);
        s = label_names(s, "post-", &spell);
    }
    // (Class: a OK, b NO) / (Race: a OK, b NO) -> the map's exit-text wording
    let class_race = |kind: usize, n: i32| {
        let mut name = if kind == 0 {
            get_class_name(&names, n)
        } else if n == 0 {
            "None".to_owned()
        } else {
            world
                .get_race(n)
                .map_or_else(|| n.to_string(), |r| r.name.clone())
        };
        if !hide {
            name.push_str(&format!("({n})"));
        }
        name
    };
    for (kind, word) in ["Class", "Race"].into_iter().enumerate() {
        let open = format!("({word}: ");
        let Some(x) = s.find(&open) else { continue };
        let a = extract_value(&s, &open);
        let b = extract_value(&s, &format!("({word}: {a} OK, "));
        let mut new = if a > 0 && b == 0 {
            format!("({word} Only: {}", class_race(kind, a))
        } else if a > 0 && b > 0 {
            format!(
                "({word} OK: {}, {word} NO: {}",
                class_race(kind, a),
                class_race(kind, b)
            )
        } else if b > 0 {
            format!("(NOT {word}: {}", class_race(kind, b))
        } else {
            format!("({word}?")
        };
        new.push(')');
        // replace through the closing paren of the original text
        let end = s[x..].find(')').map_or(s.len(), |p| x + p + 1);
        s.replace_range(x..end, &new);
    }
    s
}

/// `Format$(n, "#,##0")`.
fn group_thousands(n: i32) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// `ShowRouteResults`: the `Step` / `Location` rows of a route, with a row wherever an action happens and one every
/// `waypoint_interval` steps in between.
fn route_rows(world: &World, route: &Route, opts: RouteOptions) -> Vec<RouteRow> {
    let interval = if opts.waypoint_interval < 1 {
        25
    } else {
        opts.waypoint_interval
    };
    let n = route.n_steps;
    let mut rows: Vec<RouteRow> = Vec::new();
    let mut push = |step: i32, map: i32, room: i32, text: String| {
        // a row equal to the one before it is left out
        if rows
            .last()
            .is_some_and(|r| r.step == step && r.text == text)
        {
            return;
        }
        rows.push(RouteRow {
            step,
            text,
            map,
            room,
        });
    };
    let at = |i: i32| &route.steps[usize::try_from(i).unwrap_or(0)];
    push(
        0,
        route.source_map,
        route.source_room,
        format!(
            "Room: {}",
            room_text(world, route.source_map, route.source_room)
        ),
    );
    let mut last_shown = 0;
    let mut last_row_room = 0;
    for i in 1..=n {
        let (cur, prev) = (at(i), at(i - 1));
        if cur.flags != 0 {
            // listed at the room you act FROM
            push(
                i,
                prev.map,
                prev.room,
                format!(
                    "Room: {} -- {} -> {}",
                    room_text(world, prev.map, prev.room),
                    format_route_label(world, &cur.label, opts.hide_record_numbers),
                    room_text(world, cur.map, cur.room)
                ),
            );
            last_shown = i;
            last_row_room = i - 1;
        } else if i - last_shown >= interval {
            push(
                i,
                cur.map,
                cur.room,
                format!("Room: {}", room_text(world, cur.map, cur.room)),
            );
            last_shown = i;
            last_row_room = i;
        }
    }
    if last_row_room != n {
        let end = at(n);
        push(
            n,
            end.map,
            end.room,
            format!("Room: {}", room_text(world, end.map, end.room)),
        );
    }
    rows
}

/// `ShowMonsterRoute`.
#[must_use]
pub fn monster_route(
    world: &World,
    reach: &Reach,
    monster: i32,
    level: i32,
    alt: bool,
    opts: RouteOptions,
) -> RouteDialog {
    let ignore_level = match reach.monster_info(monster, level).status {
        ReachStatus::Yes => false,
        // the full route past the blocking level gate
        ReachStatus::No => true,
        _ => return RouteDialog::message("No route information for this monster."),
    };
    let Some(r) = reach.route(monster, level, false, ignore_level, alt) else {
        return RouteDialog::message(if alt {
            "No alternate route found."
        } else {
            "No route found."
        });
    };
    // the alternate is named by what sets it apart; the default mentions its alternate
    let kind = if alt {
        route_kind_text(r.coin_steps, r.alt_coin_steps)
    } else {
        "Route"
    };
    let names = WorldNames(world);
    let caption = format!(
        "{kind} to {}{}",
        get_monster_name(&names, monster, true),
        caption_text(
            r.n_steps,
            &r.source_name,
            ignore_level,
            r.coin_steps,
            if alt { 0 } else { r.alt_steps },
            r.alt_coin_steps,
            &r.alt_source_name
        )
    );
    RouteDialog {
        caption,
        rows: route_rows(world, &r, opts),
        message: None,
    }
}

/// `ShowRoomRoute`, for a computed result.
#[must_use]
pub fn room_route(
    world: &World,
    reach: &Reach,
    map: i32,
    room: i32,
    level: i32,
    opts: RouteOptions,
) -> RouteDialog {
    if map < 1 || room < 1 {
        return RouteDialog::message("");
    }
    let ignore_level = match reach.room_status(map, room, level) {
        ReachStatus::Yes => false,
        ReachStatus::No => true,
        ReachStatus::NoGate => return RouteDialog::message("No known route leads to this room."),
        ReachStatus::Unknown => {
            return RouteDialog::message("No route information for this room.");
        }
    };
    let Some(r) = reach.room_route(map, room, level, ignore_level, false) else {
        // every path past the gate needs a random teleport too scattered to be usable
        return RouteDialog::message("No known route leads to this room.");
    };
    if r.n_steps == 0 {
        return RouteDialog::message(&format!(
            "Routes start at this room: {}.",
            room_text(world, map, room)
        ));
    }
    let caption = format!(
        "Route to {}{}",
        room_text(world, map, room),
        caption_text(
            r.n_steps,
            &r.source_name,
            ignore_level,
            r.coin_steps,
            r.alt_steps,
            r.alt_coin_steps,
            &r.alt_source_name
        )
    );
    RouteDialog {
        caption,
        rows: route_rows(world, &r, opts),
        message: None,
    }
}

/// `POST /api/reachability/route`.
pub async fn route_route(
    State(state): State<Arc<AppState>>,
    Body(req): Body<RouteReq>,
) -> ApiResult {
    let target = match (req.monster, req.map, req.room) {
        (Some(m), None, None) => (Some(m), 0, 0),
        (None, Some(map), Some(room)) => (None, map, room),
        _ => return Err(bad("give a monster, or a map and a room")),
    };
    let opts = RouteOptions {
        waypoint_interval: state.settings().map_or(25, |s| s.route_waypoint_interval),
        hide_record_numbers: req
            .hide_record_numbers
            .unwrap_or_else(|| state.hide_record_numbers()),
    };
    let loaded = state.current();
    let out = tokio::task::spawn_blocking(move || {
        // `ReachRecompute` when nothing is computed yet, as the room route does
        let reach = loaded.reach.get_or_compute(&loaded.world);
        match target {
            (Some(m), _, _) => monster_route(&loaded.world, &reach, m, req.level, req.alt, opts),
            (None, map, room) => room_route(&loaded.world, &reach, map, room, req.level, opts),
        }
    })
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    json(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands() {
        assert_eq!(group_thousands(5), "5");
        assert_eq!(group_thousands(1234), "1,234");
        assert_eq!(group_thousands(1_000_000), "1,000,000");
    }

    #[test]
    fn caption_wording() {
        assert_eq!(
            caption_text(1, "Bank", false, 0, 0, 0, ""),
            ": 1 step from Bank"
        );
        assert_eq!(
            caption_text(12, "Bank", true, 2, 14, 0, "Inn"),
            ": 12 steps from Bank (ignoring level gates), 2 tolls/fees; coin-free route: 14 steps from Inn"
        );
    }
}
