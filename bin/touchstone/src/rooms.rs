//! `/api/rooms/...`: the Rooms tab (text half). Card P4-13.
//!
//! `GET /api/rooms/{map}/{room}` is what MME shows for a room (`MapStartMapping` title,
//! `MapGetRoomLoc` references, the `MapMapExits` tooltip), `GET /api/rooms/find-text` the room name
//! finder (`cmdMapFindText_Click`, Find then Find Next until the end or a limit),
//! `POST /api/rooms/find-exits` the rooms-with-these-exits finder (`FindRoomWithDirections`) and
//! `POST /api/rooms/cmd-rooms` the list behind a room command row (`ShowRoomCmdRoomList`).

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use touchstone_model::World;
use touchstone_rules::lairs::{LairCache, LairInfo};
use touchstone_rules::locations::LocationLairs;
use touchstone_rules::rooms::{
    DIRECTIONS, ExitView, RoomEnv, RoomHit, RoomOptions, find_exits, find_text_matches,
    room_cmd_room_list, room_exits, room_refs, room_title, room_tooltip,
};
use touchstone_vb6::clng;

use crate::calc::{ApiResult, Body, bad, json, not_found};
use crate::detail::nmr_ver;
use crate::panes::Target;
use crate::state::AppState;

/// The most hits a find-text request returns when it gives no limit; MME steps one at a time.
const DEFAULT_LIMIT: usize = 100;
const MAX_LIMIT: usize = 1000;

/// `GetLairInfo` and `GetPreCalculatedMonsterDamage` while nothing has been calculated: the lair
/// cache as loaded, and each monster's stored average damage (nothing in the arrays).
struct ServerEnv<'a> {
    world: &'a World,
    lairs: &'a LairCache,
    nmr: f64,
}

impl LocationLairs for ServerEnv<'_> {
    fn get_lair_info(&mut self, group_index: &str, max_regen: i16) -> LairInfo {
        self.lairs.get_lair_info_startup(group_index, max_regen)
    }
}

impl RoomEnv for ServerEnv<'_> {
    fn monster_damage(&self, number: i32) -> (f64, String) {
        if number < 1 {
            return (0.0, "(default)".into());
        }
        // GetMonsterAvgDmgFromDB: the stored AvgDmg from data version 1.8
        let avg = self
            .world
            .get_monster(number)
            .filter(|_| self.nmr >= 1.8)
            .and_then(|m| clng(m.avg_dmg).ok())
            .unwrap_or(0);
        (f64::from(avg), "(default)".into())
    }
}

// ---------- the reference rows ----------

/// Where a reference row leads (`GotoLocation`): the first of its prefixes the text starts with.
/// `None` for rows that open a command tree window or nothing.
fn ref_target(text: &str, tag: &str) -> Option<Target> {
    let number = || tag.trim().parse::<i32>().ok().filter(|n| *n > 0);
    let table = |table: &'static str| {
        number().map(|number| Target {
            table,
            number,
            key: None,
        })
    };
    let room = || {
        tag.contains('/').then(|| Target {
            table: "rooms",
            number: 0,
            key: Some(tag.trim().to_owned()),
        })
    };
    let starts = |p: &str| text.starts_with(p);
    // GotoLocation tests Room, Monster, Textblock, Item, Spell, Shop, Group, Command, NPC, Lair,
    // Teleport, Action, Execute, Casts, Summon, Greet in that order
    if starts("Room") || starts("Group") || starts("Lair") || starts("Teleport") || starts("Action")
    {
        room()
    } else if starts("Monster") || starts("NPC") || starts("Summon") {
        table("monsters")
    } else if starts("Item") {
        table("items")
    } else if starts("Spell") || starts("Casts") {
        table("spells")
    } else if starts("Shop") {
        table("shops")
    } else {
        None
    }
}

#[derive(Debug, Serialize)]
struct RefJson {
    text: String,
    tag: String,
    target: Option<Target>,
}

#[derive(Debug, Serialize)]
struct ExitJson {
    dir: &'static str,
    text: String,
    kind: &'static str,
    map: i32,
    room: i32,
    exit_type: String,
    /// The destination's name as `GetRoomName` shows it.
    name: Option<String>,
}

fn exit_json(world: &World, e: ExitView) -> ExitJson {
    let name = (e.room > 0).then(|| {
        world.get_room(e.map, e.room).map_or_else(
            || format!("{}/{}", e.map, e.room),
            |r| {
                let n = r.name.clone().unwrap_or_default();
                if n.is_empty() { "(no name)".into() } else { n }
            },
        )
    });
    ExitJson {
        dir: e.dir,
        text: e.text,
        kind: e.kind,
        map: e.map,
        room: e.room,
        exit_type: e.exit_type,
        name,
    }
}

#[derive(Debug, Serialize)]
struct RoomJson {
    found: bool,
    map: i32,
    room: i32,
    name: String,
    /// The Rooms frame caption.
    title: String,
    light: i32,
    /// The reference list's column header.
    refs_header: String,
    refs: Vec<RefJson>,
    exits: Vec<ExitJson>,
    /// The cell tooltip text: name, who is also here, light, shop, items, exits, commands, lair totals.
    tooltip: Option<String>,
    /// The message box MME shows (`Room m/r was not found.`).
    message: Option<String>,
    errors: Vec<i32>,
}

/// The display boxes of the Rooms tab (`chkMapOptions`, `bHideRecordNumbers`) and the character's
/// illumination, as query parameters. All optional.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RoomQuery {
    /// Left out, the `hide_record_numbers` setting decides.
    pub hide_numbers: Option<bool>,
    pub hide_npc: bool,
    pub hide_lairs: bool,
    pub hide_cmds: bool,
    pub all_exits: bool,
    pub illu: i32,
}

fn options(world: &World, q: &RoomQuery, hide_default: bool) -> RoomOptions {
    RoomOptions {
        nmr_ver: nmr_ver(&world.info.nmr_version),
        hide_record_numbers: q.hide_numbers.unwrap_or(hide_default),
        hide_npcs: q.hide_npc,
        hide_lairs: q.hide_lairs,
        hide_commands: q.hide_cmds,
        show_all_exits: q.all_exits,
        illumination: q.illu,
    }
}

pub async fn room_route(
    State(state): State<Arc<AppState>>,
    Path((map, room)): Path<(i32, i32)>,
    Query(q): Query<RoomQuery>,
) -> ApiResult {
    let loaded = state.current();
    let hide = state.hide_record_numbers();
    let out = tokio::task::spawn_blocking(move || {
        let world = &loaded.world;
        let opt = options(world, &q, hide);
        let mut env = ServerEnv {
            world,
            lairs: loaded.lair_cache(),
            nmr: opt.nmr_ver,
        };
        let Some(title) = room_title(world, map, room) else {
            return RoomJson {
                found: false,
                map,
                room,
                name: String::new(),
                title: String::new(),
                light: 0,
                refs_header: String::new(),
                refs: Vec::new(),
                exits: Vec::new(),
                tooltip: None,
                message: Some(format!("Room {map}/{room} was not found.")),
                errors: Vec::new(),
            };
        };
        let r = world.get_room(map, room).expect("title implies the room");
        let refs = room_refs(world, &opt, &env, map, room);
        let tooltip = room_tooltip(world, &opt, &mut env, map, room);
        RoomJson {
            found: true,
            map,
            room,
            name: r.name.clone().unwrap_or_default(),
            title,
            light: r.light,
            refs_header: refs.header,
            refs: refs
                .rows
                .into_iter()
                .map(|x| {
                    let tag = x.tag.text();
                    RefJson {
                        target: ref_target(&x.text, &tag),
                        text: x.text,
                        tag,
                    }
                })
                .collect(),
            exits: room_exits(r)
                .into_iter()
                .map(|e| exit_json(world, e))
                .collect(),
            tooltip,
            message: None,
            errors: refs.error.into_iter().collect(),
        }
    })
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    json(&out)
}

// ---------- the finders ----------

#[derive(Debug, Serialize)]
struct HitJson {
    map: i32,
    room: i32,
    name: String,
}

#[derive(Debug, Serialize)]
struct FindTextJson {
    matches: Vec<HitJson>,
    /// True when more rooms match than `limit` lists.
    truncated: bool,
    /// "Name not found." when nothing matches.
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FindTextQuery {
    q: String,
    /// `map/room`: list the matches after this room (Find Next).
    after: Option<String>,
    limit: Option<usize>,
}

/// Find, then Find Next from each hit, as MME's two buttons do, until the end of the rooms or `limit`.
pub async fn find_text_route(
    State(state): State<Arc<AppState>>,
    Query(q): Query<FindTextQuery>,
) -> ApiResult {
    let limit = q.limit.unwrap_or(DEFAULT_LIMIT);
    if limit == 0 || limit > MAX_LIMIT {
        return Err(bad(format!("limit must be between 1 and {MAX_LIMIT}")));
    }
    if q.q.is_empty() {
        return Err(bad("q is empty"));
    }
    let after = match q.after.as_deref() {
        None => None,
        Some(a) => {
            let e = touchstone_rules::textblock::extract_map_room(a);
            if e.map == 0 || e.room == 0 {
                return Err(bad("after must be map/room"));
            }
            Some((e.map, e.room))
        }
    };
    let loaded = state.current();
    let out = tokio::task::spawn_blocking(move || {
        let world = &loaded.world;
        let (hits, truncated) = find_text_matches(world, &q.q, after, limit);
        let message = hits.is_empty().then(|| "Name not found.".to_owned());
        let matches = hits
            .into_iter()
            .map(|(m, r)| HitJson {
                map: m,
                room: r,
                name: world
                    .get_room(m, r)
                    .and_then(|x| x.name.clone())
                    .unwrap_or_default(),
            })
            .collect();
        FindTextJson {
            matches,
            truncated,
            message,
        }
    })
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    json(&out)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FindExitsBody {
    /// The text box of the "Find Room with Exits" window.
    name: String,
    /// `optRoomFindMatch(1)`: the name must equal the text instead of containing it.
    #[serde(default)]
    exact: bool,
    /// The direction buttons that are on: `N S E W NE NW SE SW U D`.
    exits: Vec<String>,
}

#[derive(Debug, Serialize)]
struct RowsJson {
    caption: String,
    rows: Vec<RowJson>,
    /// The message boxes MME shows.
    messages: Vec<String>,
}

#[derive(Debug, Serialize)]
struct RowJson {
    text: String,
    tag: String,
    target: Option<Target>,
}

fn row_json(h: RoomHit) -> RowJson {
    RowJson {
        target: ref_target(&h.text, &h.tag),
        text: h.text,
        tag: h.tag,
    }
}

pub async fn find_exits_route(
    State(state): State<Arc<AppState>>,
    Body(b): Body<FindExitsBody>,
) -> ApiResult {
    let mut mask = 0;
    for e in &b.exits {
        let Some(i) = DIRECTIONS.iter().position(|d| d.eq_ignore_ascii_case(e)) else {
            return Err(bad(format!("unknown exit {e:?}")));
        };
        mask |= 1 << i;
    }
    let loaded = state.current();
    let out = tokio::task::spawn_blocking(move || {
        let r = find_exits(&loaded.world, &b.name, b.exact, mask);
        RowsJson {
            caption: r.caption,
            rows: r.rows.into_iter().map(row_json).collect(),
            messages: r.messages,
        }
    })
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    json(&out)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CmdRoomsBody {
    /// The row's tag: `17/278|17/2073-2096`.
    list: String,
    /// The row's text, for the caption.
    #[serde(default)]
    row_text: String,
    /// The percent cell of the row.
    #[serde(default)]
    percent: String,
    #[serde(default)]
    hide_numbers: Option<bool>,
}

pub async fn cmd_rooms_route(
    State(state): State<Arc<AppState>>,
    Body(b): Body<CmdRoomsBody>,
) -> ApiResult {
    let loaded = state.current();
    let hide = b
        .hide_numbers
        .unwrap_or_else(|| state.hide_record_numbers());
    let out = tokio::task::spawn_blocking(move || {
        room_cmd_room_list(&loaded.world, hide, &b.list, &b.row_text, &b.percent)
    })
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if out.rows.is_empty() {
        return Err(not_found("no rooms in that list"));
    }
    json(&RowsJson {
        caption: out.caption,
        rows: out.rows.into_iter().map(row_json).collect(),
        messages: Vec::new(),
    })
}
