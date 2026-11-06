//! The Rooms tab, text half (FEAT-ROOMS, stock only). Card P4-13.
//!
//! MME shows a room through three things the port returns separately: the title
//! ([`room_title`], `MapStartMapping`), the reference list ([`room_refs`], `MapGetRoomLoc`) and the
//! cell tooltip text ([`room_tooltip`], the text half of `MapMapExits`). The finders are
//! [`find_text_step`] (`cmdMapFindText_Click`), [`find_exits`] (`FindRoomWithDirections`) and
//! [`room_cmd_room_list`] (`ShowRoomCmdRoomList`). The map drawing is not ported.
//!
//! Where a VB6 run-time error reaches MME's handler the procedure ends there with what it had built;
//! the ports do the same (the reference list keeps its rows, a tooltip is `None`).

use touchstone_model::{Room, World};
use touchstone_vb6::{Currency, Vb6Error, clng, format, left, len, mid, val};

use crate::Profile;
use crate::ability_text::{
    AbilityEnv, get_class_name, get_item_name, get_monster_name, get_spell_name,
};
use crate::detail::shop::{NpcRefKind, add_room_npc_command_refs};
use crate::lairs::LairInfo;
use crate::locations::{LocationLairs, cur_str, extract_numbers, lcase, round_cur};
use crate::rows::monster::put_commas;
use crate::spells::PullArgs;
use crate::textblock::{
    auto_append, expand_room_cmd_rooms, extract_map_room, extract_value, get_room_cmd_tb,
    get_textblock_cmds,
};

type Res<T> = Result<T, Vb6Error>;

/// The direction columns of `Rooms`, in the order MME's loops walk them (bit n of an exit mask).
pub const DIRECTIONS: [&str; 10] = ["N", "S", "E", "W", "NE", "NW", "SE", "SW", "U", "D"];

/// What MME reads from the Rooms tab's controls and module globals.
#[derive(Debug, Clone)]
pub struct RoomOptions {
    /// `nNMRVer`.
    pub nmr_ver: f64,
    /// `bHideRecordNumbers`.
    pub hide_record_numbers: bool,
    /// `chkMapOptions(3)`: hide the room's NPC.
    pub hide_npcs: bool,
    /// `chkMapOptions(2)`: hide the lair monsters.
    pub hide_lairs: bool,
    /// `chkMapOptions(4)`: hide the room commands.
    pub hide_commands: bool,
    /// `chkMapOptions(6)`: show every exit's destination in the tooltip.
    pub show_all_exits: bool,
    /// `Val(lblInvenCharStat(23).Caption)`: the character's illumination.
    pub illumination: i32,
}

impl Default for RoomOptions {
    fn default() -> Self {
        Self {
            nmr_ver: 1.83,
            hide_record_numbers: false,
            hide_npcs: false,
            hide_lairs: false,
            hide_commands: false,
            show_all_exits: false,
            illumination: 0,
        }
    }
}

/// `GetLairInfo` and `GetPreCalculatedMonsterDamage`, the two callouts the room text makes.
pub trait RoomEnv: LocationLairs {
    /// `GetPreCalculatedMonsterDamage(number, label)`: the damage and its label ("vs Char",
    /// "vs Party", "(default)"); monster 0 gives only the label.
    fn monster_damage(&self, number: i32) -> (f64, String);
}

/// A reference row's Tag as the ListView holds it: a number (record references) or text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefTag {
    Num(i32),
    Text(String),
}

impl RefTag {
    /// `CStr` of the Variant, as the oracle records it.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            RefTag::Num(n) => n.to_string(),
            RefTag::Text(t) => t.clone(),
        }
    }
}

/// A row of the room's reference list (`lvMapLoc`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomRef {
    pub text: String,
    pub tag: RefTag,
}

/// What [`room_refs`] returns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoomRefs {
    /// The list's column header: `References [Name (map/room)]`.
    pub header: String,
    pub rows: Vec<RoomRef>,
    /// The message box MME shows (room not found).
    pub message: Option<String>,
    /// The VB error number that ended the procedure early, if one did.
    pub error: Option<i32>,
}

// ---- helpers ------------------------------------------------------------------------------------

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

/// `InStr(start, hay, needle)` over characters (0 when not found).
fn instr(start: i32, hay: &str, needle: &str) -> i32 {
    let h = chars(hay);
    let n = chars(needle);
    if start < 1 || start as usize > h.len() {
        return 0;
    }
    if n.is_empty() {
        return start;
    }
    if h.len() < n.len() {
        return 0;
    }
    (start as usize - 1..=h.len() - n.len())
        .find(|&i| h[i..i + n.len()] == n[..])
        .map_or(0, |i| i as i32 + 1)
}

/// `InStr(1, hay, needle, vbTextCompare)`.
fn instr_text(hay: &str, needle: &str) -> i32 {
    instr(1, &lcase(hay), &lcase(needle))
}

fn mid2(s: &str, start: i32, length: i32) -> Res<String> {
    mid(s, start, Some(length))
}

fn mid1(s: &str, start: i32) -> Res<String> {
    mid(s, start, None)
}

fn trim(s: &str) -> &str {
    s.trim_matches(' ')
}

fn err_no(e: &Vb6Error) -> i32 {
    match e {
        Vb6Error::Overflow => 6,
        Vb6Error::InvalidProcedureCall => 5,
        Vb6Error::TypeMismatch => 13,
        _ => 0,
    }
}

fn room_name_of(r: &Room) -> String {
    r.name.clone().unwrap_or_default()
}

/// `GetRoomName(, map, room, bNoRoomNumber)`.
fn get_room_name(world: &World, map: i32, room: i32, no_number: bool) -> String {
    if map == 0 || room == 0 {
        return "?".into();
    }
    match world.get_room(map, room) {
        None => format!("{map}/{room}"),
        Some(r) => {
            let mut n = room_name_of(r);
            if n.is_empty() {
                n = "(no name)".into();
            }
            if !no_number {
                n = format!("{n} ({map}/{room})");
            }
            n
        }
    }
}

/// `GetShopName(n, bNoNumber)`.
fn get_shop_name(world: &World, n: i32, no_number: bool) -> String {
    if n == 0 {
        return "None".into();
    }
    match world.get_shop(n) {
        None => n.to_string(),
        Some(s) if no_number => s.name.clone(),
        Some(s) => format!("{}({n})", s.name),
    }
}

/// `GetRaceName(n)`.
fn get_race_name(world: &World, n: i32) -> String {
    if n == 0 {
        return "None".into();
    }
    world
        .get_race(n)
        .map_or_else(|| n.to_string(), |r| r.name.clone())
}

/// `GetMultiMonsterNames(numbers, hide)`: the names of the comma-terminated numbers.
fn get_multi_monster_names(world: &World, numbers: &str, hide: bool) -> String {
    if numbers.is_empty() {
        return "None".into();
    }
    if world.monsters.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let mut x = 0;
    loop {
        let y = instr(x + 1, numbers, ",");
        if y == 0 {
            break;
        }
        let piece = mid2(numbers, x + 1, y - x - 1).unwrap_or_default();
        if let Some(m) = world.get_monster(clng(val(&piece)).unwrap_or(0)) {
            if !out.is_empty() {
                out.push_str(", ");
            }
            out.push_str(m.name.as_deref().unwrap_or(""));
            if !hide {
                out.push_str(&format!("({})", m.number));
            }
        }
        x = y;
    }
    out
}

/// The `Select Case Left(ExitType, 5)` of MME's exit loops (0 when the text names no type).
fn exit_type_code(exit_type: &str) -> i32 {
    if len(exit_type) <= 2 {
        return 0;
    }
    match left(exit_type, 5).unwrap_or_default().as_str() {
        "(Key:" => 2,
        "(Item" => 3,
        "(Toll" => 4,
        "(Hidd" => 6,
        "(Door" => 7,
        "(Trap" => 9,
        "(Text" => 10,
        "(Gate" => 11,
        "Actio" => 12,
        "(Clas" => 13,
        "(Race" => 14,
        "(Leve" => 15,
        "(Time" => 16,
        "(Tick" => 17,
        "(Max " => 18,
        "(Bloc" => 19,
        "(Alig" => 20,
        "(Dela" => 21,
        "(Cast" => 22,
        "(Abil" => 23,
        "(Spel" => 24,
        _ => 0,
    }
}

fn exit_text(r: &Room, dir: usize) -> &str {
    match dir {
        0 => &r.n,
        1 => &r.s,
        2 => &r.e,
        3 => &r.w,
        4 => &r.ne,
        5 => &r.nw,
        6 => &r.se,
        7 => &r.sw,
        8 => &r.u,
        _ => &r.d,
    }
}

/// Rooms in `idxRooms` order (map, then room).
fn ordered_rooms(world: &World) -> Vec<&Room> {
    let mut v: Vec<&Room> = world.rooms.iter().collect();
    v.sort_by_key(|r| (r.map_number, r.room_number));
    v
}

/// The Placed column's item numbers: `Split(Placed, ",")` entries with `Val > 0`.
fn placed_numbers(placed: &str) -> Vec<i32> {
    if len(placed) <= 1 {
        return Vec::new();
    }
    placed
        .split(',')
        .filter_map(|p| {
            let v = val(p);
            (v > 0.0).then(|| clng(v).unwrap_or(0))
        })
        .collect()
}

// ---- title --------------------------------------------------------------------------------------

/// The Rooms frame caption `MapStartMapping` sets, or `None` when the room does not exist (MME
/// then shows "Room m/r was not found.").
#[must_use]
pub fn room_title(world: &World, map: i32, room: i32) -> Option<String> {
    world
        .get_room(map, room)
        .map(|r| format!("Rooms -- {} ({map}/{room})  ", room_name_of(r)))
}

// ---- references ---------------------------------------------------------------------------------

/// Port of frmMain `MapGetRoomLoc`: the references of a room (NPC, shop, placed items, room spell,
/// room command teleports and commands, the NPC's greet commands, items and spells named by exits,
/// and the lair monsters).
#[must_use]
pub fn room_refs(
    world: &World,
    opt: &RoomOptions,
    env: &dyn RoomEnv,
    map: i32,
    room: i32,
) -> RoomRefs {
    let mut out = RoomRefs::default();
    let Some(r) = world.get_room(map, room) else {
        out.message = Some(format!("Room ({map}/{room}) was not found."));
        return out;
    };
    out.header = format!("References [{} ({map}/{room})]", room_name_of(r));
    if let Err(e) = refs_inner(world, opt, env, r, &mut out.rows) {
        out.error = Some(err_no(&e));
    }
    out
}

fn push(rows: &mut Vec<RoomRef>, text: String, tag: RefTag) {
    rows.push(RoomRef { text, tag });
}

#[allow(clippy::too_many_lines)]
fn refs_inner(
    world: &World,
    opt: &RoomOptions,
    env: &dyn RoomEnv,
    r: &Room,
    rows: &mut Vec<RoomRef>,
) -> Res<()> {
    let hide = opt.hide_record_numbers;
    let map = r.map_number;
    let mut npc = 0;
    if !opt.hide_npcs && r.npc > 0 {
        npc = r.npc;
        push(
            rows,
            format!("NPC: {}", get_monster_name(world, r.npc, hide)),
            RefTag::Num(r.npc),
        );
    }
    if r.shop > 0 {
        push(
            rows,
            format!("Shop: {}", get_shop_name(world, r.shop, hide)),
            RefTag::Num(r.shop),
        );
    }
    for n in placed_numbers(r.placed.as_deref().unwrap_or("")) {
        if let Some(it) = world.get_item(n) {
            let num = if hide {
                String::new()
            } else {
                format!("({})", it.number)
            };
            push(
                rows,
                format!("Item: {}{num}", it.name),
                RefTag::Num(it.number),
            );
        }
    }
    if r.spell > 0 {
        push(
            rows,
            format!("Spell: {}", get_spell_name(world, r.spell, hide)),
            RefTag::Num(r.spell),
        );
    }
    if r.cmd > 0
        && let Some(tb) = world.get_tbinfo(r.cmd)
    {
        {
            let data = tb.action.clone().unwrap_or_default();
            teleport_rows(world, &data, map, rows)?;
            push(
                rows,
                format!("Commands: Textblock {}", r.cmd),
                RefTag::Num(r.cmd),
            );
            for c in get_textblock_cmds(world, r.cmd, 0).split(',') {
                push(rows, format!("Command: {}", trim(c)), RefTag::Num(r.cmd));
            }
        }
    }
    // the NPC's greet commands come after the room's own, so the teleport de-dupe above never sees them
    if npc > 0 {
        for n in add_room_npc_command_refs(world, npc, map) {
            let tag = match n.kind {
                NpcRefKind::Teleport => RefTag::Text(n.tag),
                NpcRefKind::Greet => RefTag::Num(n.tag.parse().unwrap_or(0)),
            };
            push(rows, n.text, tag);
        }
    }
    for dir in 0..10 {
        exit_refs(world, hide, r, dir, rows)?;
    }
    let lair = r.lair.as_deref().unwrap_or("");
    if !opt.hide_lairs && len(lair) > 1 {
        let numbers = mid1(lair, instr(1, lair, ":") + 2)?;
        let mut x = 0;
        loop {
            let y = instr(x + 1, &numbers, ",");
            if y == 0 {
                break;
            }
            let n = clng(val(&mid2(&numbers, x + 1, y - x - 1)?))?;
            if let Some(m) = world.get_monster(n) {
                let (dmg, label) = env.monster_damage(m.number);
                let dmg = clng(dmg)?;
                let mut text = format!("Monster: {}", m.name.as_deref().unwrap_or(""));
                if !hide {
                    text.push_str(&format!("({})", m.number));
                }
                if dmg > 0 {
                    text.push_str(&format!(" - {dmg} dmg/rnd {label}"));
                }
                push(rows, text, RefTag::Num(m.number));
            }
            x = y;
        }
    }
    Ok(())
}

/// The teleport scan over a room command textblock. MME compares every existing row's Tag with the
/// `map/room` text; a row whose Tag is a number makes that comparison a type mismatch, which ends
/// the procedure (error 13), so a room with an NPC, shop, placed item or spell loses its commands.
fn teleport_rows(world: &World, data: &str, map_number: i32, rows: &mut Vec<RoomRef>) -> Res<()> {
    let total = len(data);
    let mut pos = 1;
    while pos < total {
        let mut x = instr(pos, data, "\n");
        if x == 0 {
            x = total + 1;
        }
        let line = mid2(data, pos, x - pos)?;
        pos = x + 1;
        let t = instr(1, &line, "teleport ");
        if t == 0 {
            continue;
        }
        let mut y = t + 9;
        let mut x = y;
        let (mut n_room, mut n_map) = (0, 0);
        let lc = chars(&line);
        while y <= len(&line) {
            let c = lc[(y - 1) as usize];
            match c {
                '0'..='9' => {}
                ' ' => {
                    if y > x && n_room == 0 {
                        n_room = clng(val(&mid2(&line, x, y - x)?))?;
                        x = y + 1;
                    } else {
                        n_map = clng(val(&mid2(&line, x, y - x)?))?;
                        break;
                    }
                }
                _ => {
                    if y > x && n_room == 0 {
                        n_room = clng(val(&mid2(&line, x, y - x)?))?;
                    } else {
                        n_map = clng(val(&mid2(&line, x, y - x)?))?;
                    }
                    break;
                }
            }
            y += 1;
        }
        if n_room != 0 {
            let colon = instr(1, &line, ":");
            let command = left(&line, colon - 1)?;
            if n_map == 0 {
                n_map = map_number;
            }
            let key = format!("{n_map}/{n_room}");
            let mut seen = false;
            for row in rows.iter() {
                match &row.tag {
                    RefTag::Num(_) => return Err(Vb6Error::TypeMismatch),
                    RefTag::Text(t) if *t == key => {
                        seen = true;
                        break;
                    }
                    RefTag::Text(_) => {}
                }
            }
            if !seen {
                let command = command.replace('*', "").replace('|', " OR ");
                push(
                    rows,
                    format!(
                        "Teleport: {command} --> {}",
                        get_room_name(world, n_map, n_room, false)
                    ),
                    RefTag::Text(key),
                );
            }
        }
    }
    Ok(())
}

/// One direction of `MapGetRoomLoc`'s exit loop.
fn exit_refs(world: &World, hide: bool, r: &Room, dir: usize, rows: &mut Vec<RoomRef>) -> Res<()> {
    let field = exit_text(r, dir);
    let item_row = |rows: &mut Vec<RoomRef>, n: i32| {
        push(
            rows,
            format!("Item: {}", get_item_name(world, n, hide)),
            RefTag::Num(n),
        );
    };
    let action_on = |rows: &mut Vec<RoomRef>, text: &str| -> bool {
        // true when the row is already listed (MME then skips the rest of this exit)
        let e = extract_map_room(text);
        if e.map > 0 {
            let s = format!("Action On: {}", get_room_name(world, e.map, e.room, false));
            if rows.iter().any(|x| x.text == s) {
                return true;
            }
            push(rows, s, RefTag::Text(format!("{}/{}", e.map, e.room)));
        }
        false
    };
    if val(field) != 0.0 {
        let e = extract_map_room(field);
        let code = exit_type_code(&e.exit_type);
        match code {
            2 | 3 | 17 => {
                let n = clng(extract_numbers(&e.exit_type))?;
                if n > 0 {
                    item_row(rows, n);
                }
            }
            22 | 24 => {
                for key in ["pre-", "post-"] {
                    let n = extract_value(&e.exit_type, key);
                    if n > 0 {
                        push(
                            rows,
                            format!("Spell: {}", get_spell_name(world, n, hide)),
                            RefTag::Num(n),
                        );
                    }
                }
            }
            10 => {
                let k = len("(Text: ");
                let text = mid2(&e.exit_type, k + 1, len(&e.exit_type) - k - 1)?;
                push(rows, format!("cmd: {text}"), RefTag::Num(0));
            }
            12 => {
                if action_on(rows, &e.exit_type) {
                    return Ok(());
                }
                if instr(1, field, "(Item: ") > 0 {
                    let n = extract_value(field, "(Item: ");
                    if n > 0 {
                        item_row(rows, n);
                    }
                }
            }
            _ => {}
        }
    } else if left(field, 6)? == "Action" {
        if action_on(rows, field) {
            return Ok(());
        }
        if instr(1, field, "(Item: ") > 0 {
            let n = extract_value(field, "(Item: ");
            if n > 0 {
                item_row(rows, n);
            }
        } else if instr(1, field, "this room]: ") > 0 {
            let at = instr(1, field, "this room]: ") + len("this room: ");
            push(rows, format!("cmd: {}", mid1(field, at)?), RefTag::Num(0));
        }
    }
    Ok(())
}

// ---- tooltip ------------------------------------------------------------------------------------

const CRLF: &str = "\r\n";

/// The text half of frmMain `MapMapExits`: the tooltip of the room's map cell (name, who is also
/// here, light, shop, placed items, room spell, exits, remote actions, commands, regen and lair
/// totals). `None` where a VB error ends MME's procedure before it sets the tooltip. A room that
/// does not exist gives `Map m Room r`.
///
/// `GetLairInfo` is called afresh for every room; MME caches the last answer, which gives the same
/// text because the cache is keyed by group index and regen.
#[must_use]
pub fn room_tooltip(
    world: &World,
    opt: &RoomOptions,
    env: &mut dyn RoomEnv,
    map: i32,
    room: i32,
) -> Option<String> {
    let Some(r) = world.get_room(map, room) else {
        return Some(format!("Map {map} Room {room}"));
    };
    tooltip_inner(world, opt, env, r).ok()
}

#[allow(clippy::too_many_lines)]
fn tooltip_inner(world: &World, opt: &RoomOptions, env: &mut dyn RoomEnv, r: &Room) -> Res<String> {
    let hide = opt.hide_record_numbers;
    let (map, room) = (r.map_number, r.room_number);
    let name = format!("{} ({map}/{room})", room_name_of(r));
    let (mut light_detail, mut light_desc) = (String::new(), String::new());
    if opt.nmr_ver >= 1.82 && r.light != 0 {
        let y = opt.illumination;
        light_detail = format!(
            "Room Light: {}{}",
            if r.light > 0 { "+" } else { "" },
            r.light
        );
        if y + r.light < -150 {
            light_detail.push_str(&format!(
                " ({} more illu needed to see)",
                (150 + y + r.light).abs()
            ));
        } else {
            light_detail.push_str(&format!(" ({} illu over req to see)", 150 + y + r.light));
        }
        let t = y + r.light;
        light_desc = if t < -200 {
            "The room is pitch black"
        } else if t < -150 {
            "The room is very dark - you can't see anything"
        } else if t < -100 {
            "The room is barely visible"
        } else if t < 0 {
            "The room is dimly lit"
        } else {
            ""
        }
        .to_string();
    }
    let room_cmds = if !opt.hide_commands && r.cmd > 0 {
        format!("Room commands: {}", get_textblock_cmds(world, r.cmd, 0))
    } else {
        String::new()
    };
    let npc = if !opt.hide_npcs && r.npc > 0 {
        format!("{} (NPC)", get_monster_name(world, r.npc, hide))
    } else {
        String::new()
    };
    let mut placed = String::new();
    for n in placed_numbers(r.placed.as_deref().unwrap_or("")) {
        if !placed.is_empty() {
            placed.push_str(", ");
        }
        placed.push_str(&get_item_name(world, n, hide));
    }
    if !placed.is_empty() {
        placed = format!("Placed Items: {placed}");
    }

    let lair = r.lair.as_deref().unwrap_or("");
    let (mut also_here, mut lair_info, mut regen_time) =
        (String::new(), String::new(), String::new());
    let mut max_regen: i32 = 0;
    if !opt.hide_lairs && len(lair) > 1 {
        let mut group = String::new();
        if opt.nmr_ver >= 1.83 {
            group = lair.split(',').next_back().unwrap_or("").to_string();
            if len(&group) >= 9 {
                let inner = mid2(&group, 2, len(&group) - 2)?;
                let parts: Vec<&str> = inner.split('-').collect();
                if parts.len() == 4 {
                    max_regen = i32::from(touchstone_vb6::cint(val(parts[3]))?);
                    group = format!("{}-{}-{}", parts[0], parts[1], parts[2]);
                }
            }
        }
        let info: LairInfo = if group.is_empty() {
            LairInfo::default()
        } else {
            env.get_lair_info(&group, max_regen as i16)
        };
        if info.mobs.scaled() > 0 {
            also_here = format!(
                "Also Here ({}): {npc}{}{}",
                cur_str(info.max_regen),
                if npc.is_empty() { "" } else { ", " },
                get_multi_monster_names(world, &format!("{},", info.mob_list), hide)
            );
            max_regen = i32::from(touchstone_vb6::cint(info.max_regen.to_f64())?);
            regen_time = format!("@ {} mins", r.delay);
            let exp = info.avg_exp.checked_mul(info.max_regen)?;
            let hp = Currency::from_f64(f64::from(info.avg_hp))?.checked_mul(info.max_regen)?;
            lair_info = format!(
                "Lair Exp: {}, HP: {}",
                put_commas(&cur_str(exp)),
                put_commas(&cur_str(hp))
            );
            let (_, label) = env.monster_damage(0);
            if info.avg_dmg_clear_round.scaled() != 0 {
                lair_info.push_str(&format!(
                    "{CRLF}Dmg {label}: {}/round",
                    put_commas(&cur_str(round_cur(info.avg_dmg_clear_round, 0)))
                ));
                if info.avg_dmg_clear.scaled() > 0 {
                    lair_info.push_str(&format!(
                        ", {}/clear",
                        put_commas(&cur_str(round_cur(info.avg_dmg_clear, 0)))
                    ));
                }
            }
        } else {
            let rest = mid1(lair, instr(1, lair, ":") + 2)?;
            also_here = format!(
                "Also Here: {npc}{}{}",
                if npc.is_empty() { "" } else { ", " },
                get_multi_monster_names(world, &rest, hide)
            );
            let t1 = instr_text(lair, "Max ");
            let t2 = instr(1, lair, ":");
            if t1 > 0 && t2 > 0 {
                max_regen = i32::from(touchstone_vb6::cint(val(&mid2(
                    lair,
                    t1 + 4,
                    t2 - t1 - 5,
                )?))?);
            }
        }
    }
    let shop = if r.shop > 2 {
        format!("Shop: {}", get_shop_name(world, r.shop, hide))
    } else {
        String::new()
    };
    let room_spell = if r.spell > 0 {
        format!("Room Spell: {}", get_spell_name(world, r.spell, hide))
    } else {
        String::new()
    };

    // exits
    let mut ability = AbilityEnv::for_world(world, Profile::Stock);
    ability.hide_record_numbers = hide;
    let (mut exit_text_all, mut remote) = (String::new(), String::new());
    for (dir, label) in DIRECTIONS.iter().enumerate() {
        let field = exit_text(r, dir);
        if left(field, 6)? == "Action" {
            remote = auto_append(&remote, field, CRLF);
        } else if val(field) != 0.0 {
            let e = extract_map_room(field);
            let mut code = exit_type_code(&e.exit_type);
            if e.map != map {
                code = 8;
            }
            let mut text = String::new();
            match code {
                2 | 3 => {
                    let (key, head) = if code == 2 {
                        ("Key: ", " (Key: ")
                    } else {
                        ("Item: ", " (Item): ")
                    };
                    let y = extract_value(&e.exit_type, key);
                    let after = instr(1, &e.exit_type, &y.to_string()) + len(&y.to_string()) + 1;
                    text = format!(
                        "{CRLF}{label}{head}{} {}",
                        get_item_name(world, y, hide),
                        mid1(&e.exit_type, after)?
                    );
                }
                12 => remote = auto_append(&remote, field, CRLF),
                13 | 14 => {
                    let (word, who): (&str, &dyn Fn(i32) -> String) = if code == 13 {
                        ("Class", &|n| get_class_name(world, n))
                    } else {
                        ("Race", &|n| get_race_name(world, n))
                    };
                    let y = extract_value(&e.exit_type, &format!("{word}: "));
                    let z = extract_value(&e.exit_type, &format!("{word}: {y} OK, "));
                    let num = |n: i32| {
                        if hide {
                            String::new()
                        } else {
                            format!("({n})")
                        }
                    };
                    text = if y > 0 && z == 0 {
                        format!("{CRLF}{label} ({word} Only: {}{}", who(y), num(y))
                    } else if y > 0 && z > 0 {
                        format!(
                            "{CRLF}{label} ({word} OK: {}{}, {word} NO: {}{}",
                            who(y),
                            num(y),
                            who(z),
                            num(z)
                        )
                    } else if z > 0 {
                        format!("{CRLF}{label} (NOT {word}: {}{}", who(z), num(z))
                    } else {
                        format!("{CRLF}{label} ({word}?")
                    };
                    text.push(')');
                }
                22 => {
                    let y = extract_value(&e.exit_type, "pre-");
                    let z = extract_value(&e.exit_type, "post-");
                    text = format!("{CRLF}{label} (Cast ");
                    if y > 0 || z > 0 {
                        let eq = |n: i32| {
                            ability.pull_spell_eq(&PullArgs {
                                spell: n,
                                ..PullArgs::default()
                            })
                        };
                        if y > 0 {
                            text.push_str(&format!(
                                "Pre: {}: {}",
                                get_spell_name(world, y, hide),
                                eq(y)
                            ));
                        }
                        if z > 0 {
                            text.push_str(&format!(
                                "{}Post: {}: {}",
                                if y > 0 { ", " } else { "" },
                                get_spell_name(world, z, hide),
                                eq(z)
                            ));
                        }
                    } else {
                        text.push('?');
                    }
                    text.push(')');
                }
                24 => {
                    let y = extract_value(&e.exit_type, "Spell Trap: ");
                    text = format!("{CRLF}{label} (Spell Trap: ");
                    if y > 0 {
                        text.push_str(&format!(
                            "{}: {}",
                            get_spell_name(world, y, hide),
                            ability.pull_spell_eq(&PullArgs {
                                spell: y,
                                ..PullArgs::default()
                            })
                        ));
                    } else {
                        text.push('?');
                    }
                    text.push(')');
                }
                4 => {
                    let mut et = e.exit_type.clone();
                    if left(&et, 7)? == "(Toll: " {
                        let gold = clng(val(&mid1(&et, 8)?))?;
                        let (mut reduced, mut coin) = (f64::from(gold), "gold");
                        if gold >= 10000 {
                            reduced = f64::from(gold) / 10000.0;
                            coin = "runic";
                        } else if gold >= 100 {
                            reduced = f64::from(gold) / 100.0;
                            coin = "platinum";
                        }
                        et = format!("(Toll: {}", format(reduced, "0.##")?);
                        if et.ends_with('.') {
                            et.pop();
                        }
                        et = format!("{et} {coin})");
                    }
                    text = format!("{CRLF}{label}: {et}");
                }
                c if c > 0 && c != 8 => {
                    text = format!("{CRLF}{label}: {}", e.exit_type);
                }
                _ => {}
            }
            exit_text_all.push_str(&text);
            if opt.show_all_exits {
                match code {
                    8 => exit_text_all.push_str(&format!(
                        "{CRLF}{label} > {}",
                        get_room_name(world, e.map, e.room, hide)
                    )),
                    c if c > 0 => {
                        if c != 12 {
                            exit_text_all.push_str(&format!(
                                " > {}",
                                get_room_name(world, map, e.room, hide)
                            ));
                        }
                    }
                    _ => exit_text_all.push_str(&format!(
                        "{CRLF}{label} > {}",
                        get_room_name(world, map, e.room, hide)
                    )),
                }
            }
        }
    }

    // assemble
    let mut s = name;
    if also_here.is_empty() && !npc.is_empty() {
        also_here = format!("Also Here: {npc}");
    }
    s = auto_append(&s, &also_here, CRLF);
    s = auto_append(&s, &light_desc, CRLF);
    if !(shop.clone() + &placed + &room_spell).is_empty() {
        s.push_str(CRLF);
    }
    s = auto_append(&s, &shop, CRLF);
    s = auto_append(&s, &placed, CRLF);
    s = auto_append(&s, &room_spell, CRLF);
    s.push_str(CRLF);
    if !exit_text_all.is_empty() {
        s.push_str(&exit_text_all);
        s.push_str(CRLF);
    }
    s = auto_append(&s, &remote, CRLF);
    s = auto_append(&s, &room_cmds, CRLF);
    if !remote.is_empty() || !room_cmds.is_empty() {
        s.push_str(CRLF);
    }
    s = auto_append(&s, &light_detail, CRLF);
    if max_regen > 0 {
        s = auto_append(&s, &format!("Max Regen: {max_regen}"), CRLF);
        s = auto_append(&s, &regen_time, " ");
    }
    s = auto_append(&s, &lair_info, CRLF);
    if s.ends_with(CRLF) {
        s.truncate(s.len() - 2);
    }
    Ok(s)
}

// ---- text find ----------------------------------------------------------------------------------

/// The state of the room name finder (`sMapSearch`, `nMapLastFind`).
#[derive(Debug, Clone, Default)]
pub struct FindTextState {
    pub search: String,
    pub last: Option<(i32, i32)>,
}

/// What a press of the finder did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindTextEvent {
    /// The room MME jumps to (`MapStartMapping`).
    Hit(i32, i32),
    /// A message box.
    Message(String),
}

/// Port of frmMain `cmdMapFindText_Click` for Find (`index` 0) and Find Next (1). `answer` is what
/// the "Enter text to search for." box returns when MME asks (an empty answer cancels); Find Next
/// asks as well when there is no previous hit.
#[must_use]
pub fn find_text_step(
    world: &World,
    st: &mut FindTextState,
    index: u8,
    answer: &str,
) -> Vec<FindTextEvent> {
    if world.rooms.is_empty() {
        return Vec::new();
    }
    let rooms = ordered_rooms(world);
    let mut start = 0usize;
    match (index, st.last) {
        (1, Some((m, r))) => {
            match rooms
                .iter()
                .position(|x| (x.map_number, x.room_number) == (m, r))
            {
                None => {
                    return vec![FindTextEvent::Message(format!("Room {m}/{r} not found."))];
                }
                Some(i) => start = i + 1,
            }
        }
        _ => {
            if answer.is_empty() {
                return Vec::new();
            }
            st.search = answer.to_string();
        }
    }
    let needle = lcase(&st.search);
    for x in rooms.iter().skip(start) {
        if instr(1, &lcase(&room_name_of(x)), &needle) > 0 {
            st.last = Some((x.map_number, x.room_number));
            return vec![FindTextEvent::Hit(x.map_number, x.room_number)];
        }
    }
    st.last = None;
    vec![FindTextEvent::Message("Name not found.".into())]
}

// ---- exits finder -------------------------------------------------------------------------------

/// A row of a results list: `Room: Name (map/room)` with the `map/room` tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomHit {
    pub text: String,
    pub tag: String,
}

/// What [`find_exits`] returns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FindExits {
    pub rows: Vec<RoomHit>,
    /// "Matching rooms:" when there are rows.
    pub caption: String,
    /// The message boxes MME shows, in order.
    pub messages: Vec<String>,
}

/// The exit mask `FindRoomWithDirections` builds for a room: bit n is set for direction n when it
/// has an exit that is not an action, hidden, text, remote-action or timed one.
#[must_use]
pub fn room_exit_mask(r: &Room) -> i32 {
    let mut mask = 0;
    for dir in 0..10 {
        let field = exit_text(r, dir);
        if left(field, 6).unwrap_or_default() == "Action" || val(field) == 0.0 {
            continue;
        }
        let e = extract_map_room(field);
        let mut code = exit_type_code(&e.exit_type);
        if e.map != r.map_number {
            code = 8;
        }
        if matches!(code, 6 | 10 | 12 | 16) {
            continue;
        }
        mask |= 1 << dir;
    }
    mask
}

/// Port of frmMain `FindRoomWithDirections` after the dialog: rooms whose name matches (`exact`:
/// the trimmed name equals the text; else the name contains it, ignoring case) and whose exit mask
/// is exactly `mask`. A name under three characters or an empty mask does nothing. MME stops with
/// a message once the list holds more than 100 rooms (so at most 101 rows).
#[must_use]
pub fn find_exits(world: &World, name: &str, exact: bool, mask: i32) -> FindExits {
    let mut out = FindExits::default();
    if world.rooms.is_empty() || len(trim(name)) < 3 || mask == 0 {
        return out;
    }
    let find = trim(name);
    let find_l = lcase(find);
    for r in ordered_rooms(world) {
        let rn = room_name_of(r);
        let ok = if exact {
            trim(&rn) == find
        } else {
            instr(1, &lcase(&rn), &find_l) > 0
        };
        if !ok || room_exit_mask(r) != mask {
            continue;
        }
        if out.rows.len() > 100 {
            out.messages
                .push("Over 100 rooms found, quitting search.".into());
            break;
        }
        out.rows.push(RoomHit {
            text: format!("Room: {rn} ({}/{})", r.map_number, r.room_number),
            tag: format!("{}/{}", r.map_number, r.room_number),
        });
    }
    if out.rows.is_empty() {
        out.messages.push("Nothing Found.".into());
    } else {
        out.caption = "Matching rooms:".into();
    }
    out
}

// ---- room command room lists --------------------------------------------------------------------

/// What [`room_cmd_room_list`] returns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CmdRoomList {
    pub caption: String,
    pub rows: Vec<RoomHit>,
}

/// Port of frmMain `ShowRoomCmdRoomList`: the rooms behind a `Room cmd` row (`room_list` is the
/// row's tag such as `17/278|17/2073-2096`, `row_text` its text, `percent` the percent cell).
/// Command textblocks come first (tag `n@map`), then the rooms.
#[must_use]
pub fn room_cmd_room_list(
    world: &World,
    hide_record_numbers: bool,
    room_list: &str,
    row_text: &str,
    percent: &str,
) -> CmdRoomList {
    let rooms = expand_room_cmd_rooms(room_list);
    let mut out = CmdRoomList::default();
    if rooms.is_empty() {
        return out;
    }
    let mut seen: Vec<i32> = Vec::new();
    for r in &rooms {
        let cmd = get_room_cmd_tb(world, r, 0, 0);
        if cmd > 0 && !seen.contains(&cmd) {
            seen.push(cmd);
            out.rows.push(RoomHit {
                text: format!("Commands: Textblock {cmd}"),
                tag: format!("{cmd}@{}", extract_map_room(r).map),
            });
        }
    }
    for r in &rooms {
        let e = extract_map_room(r);
        out.rows.push(RoomHit {
            text: format!(
                "Room: {}",
                get_room_name(world, e.map, e.room, hide_record_numbers)
            ),
            tag: r.clone(),
        });
    }
    let mut caption = row_text.to_string();
    let colon = instr(1, row_text, ": ");
    if colon > 0 {
        caption = left(row_text, colon - 1).unwrap_or_default();
        let arrow = instr(1, row_text, " -> ");
        if arrow > 0 {
            caption.push_str(&mid1(row_text, arrow).unwrap_or_default());
        }
    }
    if !percent.is_empty() {
        caption.push_str(&format!(" ({percent})"));
    }
    caption.push_str(&format!(", {} rooms", rooms.len()));
    if rooms.len() >= 5000 {
        caption.push_str(" (first 5000 only)");
    }
    out.caption = format!("{caption}:");
    out
}
