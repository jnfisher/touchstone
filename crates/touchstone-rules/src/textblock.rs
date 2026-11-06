//! Textblock and room-command parsing (FAM-19, stock only). Card P3-06.
//!
//! Every function here is a pure port of an MME procedure over the `Action`, `LinkTo` and `CMD`
//! columns. MME reads them through DAO recordsets; the ports read them through
//! [`TextblockData`] instead. Positions are 1-based characters like VB6's `InStr`/`Mid`.
//!
//! Where a VB6 run-time error would have been swallowed by MME's `HandleError` handler, the
//! port returns what the procedure had assigned by then (usually the empty default), as MME does.
//! `Integer`-typed positions in MME overflow at 32767 characters; no stock `Action` is near that,
//! so the ports use `i32` throughout.

use touchstone_model::{TbInfo, World};
use touchstone_vb6::{Currency, Vb6Error, clng, left, len, mid, right, val};

/// The database rows the textblock procedures read.
pub trait TextblockData {
    /// The `TBInfo` row with this `Number` (MME seeks `pkTBInfo`).
    fn tbinfo(&self, number: i32) -> Option<&TbInfo>;
    /// The `CMD` column of the room (MME seeks `idxRooms`).
    fn room_cmd(&self, map: i32, room: i32) -> Option<i32>;
}

impl TextblockData for World {
    fn tbinfo(&self, number: i32) -> Option<&TbInfo> {
        self.get_tbinfo(number)
    }
    fn room_cmd(&self, map: i32, room: i32) -> Option<i32> {
        self.get_room(map, room).map(|r| r.cmd)
    }
}

fn action(row: &TbInfo) -> &str {
    row.action.as_deref().unwrap_or("")
}

// ---- VB6 string helpers -------------------------------------------------------------------

/// `InStr(start, hay, needle)`: 0 when `start` is past the end, `start` for an empty needle.
fn instr(start: i32, hay: &str, needle: &str) -> i32 {
    let h: Vec<char> = hay.chars().collect();
    instr_chars(start, &h, needle)
}

fn instr_chars(start: i32, h: &[char], needle: &str) -> i32 {
    if start < 1 || start as usize > h.len() {
        return 0;
    }
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() {
        return start;
    }
    let from = (start - 1) as usize;
    if h.len() < n.len() {
        return 0;
    }
    (from..=h.len() - n.len())
        .find(|&i| h[i..i + n.len()] == n[..])
        .map_or(0, |i| i as i32 + 1)
}

/// `InStr(1, hay, needle, vbTextCompare)`; case folding is ASCII only.
fn instr_text(hay: &str, needle: &str) -> i32 {
    instr(1, &hay.to_ascii_lowercase(), &needle.to_ascii_lowercase())
}

fn mid2(s: &str, start: i32, length: i32) -> Result<String, Vb6Error> {
    mid(s, start, Some(length))
}

fn mid1(s: &str, start: i32) -> Result<String, Vb6Error> {
    mid(s, start, None)
}

fn trim(s: &str) -> &str {
    s.trim_matches(' ')
}

fn replace_cmd(s: &str) -> String {
    s.replace('*', "").replace('|', " OR ")
}

/// `AutoAppend` of modSyntaxsFunc.
fn auto_append(base: &str, add: &str, glue: &str) -> String {
    if !base.is_empty() {
        if !trim(add).is_empty() {
            format!("{base}{glue}{add}")
        } else {
            base.to_string()
        }
    } else {
        add.to_string()
    }
}

/// `ExtractValueFromString` of modSyntaxsFunc.
pub(crate) fn extract_value(whole: &str, search: &str) -> i32 {
    extract_value_inner(whole, search).unwrap_or(0)
}

fn extract_value_inner(whole: &str, search: &str) -> Result<i32, Vb6Error> {
    let chars: Vec<char> = whole.chars().collect();
    let mut x = instr_text(whole, search);
    if x > 0 {
        x += len(search);
        let mut y = x;
        while y <= chars.len() as i32 {
            match chars[(y - 1) as usize] {
                '0'..='9' => {}
                ' ' | '*' => {
                    if y > x {
                        break;
                    }
                    x += 1;
                }
                _ => break,
            }
            y += 1;
        }
        if y > x {
            return clng(val(&mid2(whole, x, y - x)?));
        }
    }
    Ok(0)
}

// ---- map/room text --------------------------------------------------------------------------

/// MME's `RoomExitType`: the parts of a room-exit text such as `12/73 (Cast: pre-681, post-0)`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoomExit {
    pub map: i32,
    pub room: i32,
    pub exit_type: String,
}

/// Port of modMMudFunc `ExtractMapRoom`. A VB6 error inside (a text with no digit before the
/// slash gives `Mid(s, 0, ..)`) leaves the fields assigned so far.
#[must_use]
pub fn extract_map_room(exit: &str) -> RoomExit {
    // MME starts with `.ExitType = 0`, which a String field stores as "0"
    let mut out = RoomExit {
        exit_type: "0".into(),
        ..RoomExit::default()
    };
    let _ = extract_map_room_inner(exit, &mut out);
    out
}

fn extract_map_room_inner(s: &str, out: &mut RoomExit) -> Result<(), Vb6Error> {
    let chars: Vec<char> = s.chars().collect();
    let mut x = instr(1, s, "/");
    let mut i = 0;
    while x - 1 > 0 {
        if chars[(x - 2) as usize].is_ascii_digit() {
            i = x - 1;
        } else {
            break;
        }
        x -= 1;
    }
    let x = instr(1, s, "/");
    if x == 0 || x == len(s) {
        return Ok(());
    }
    out.map = clng(val(&mid2(s, i, x - 1)?))?;
    let y = instr(x, s, " ");
    if y == 0 {
        out.room = clng(val(&mid1(s, x + 1)?))?;
    } else {
        out.room = clng(val(&mid2(s, x + 1, y - 1)?))?;
        out.exit_type = mid1(s, y + 1)?;
    }
    Ok(())
}

/// Port of modMMudFunc `ExtractTextCommand`: the first comma-separated command of an exit text.
#[must_use]
pub fn extract_text_command(whole: &str) -> String {
    let chars: Vec<char> = whole.chars().collect();
    let mut x = instr(1, whole, " ") + 1;
    if x == 1 {
        return whole.to_string();
    }
    let mut command = String::new();
    while x < chars.len() as i32 {
        let c = chars[(x - 1) as usize];
        if c == ',' && !command.is_empty() {
            break;
        }
        command.push(c);
        x += 1;
    }
    if command.is_empty() {
        whole.to_string()
    } else {
        command
    }
}

/// Port of modMMudDatabase `GetRoomCMDTB`: the textblock a room's command runs. Pass `""` as
/// `map_room` to use `map` and `room`.
#[must_use]
pub fn get_room_cmd_tb(db: &dyn TextblockData, map_room: &str, map: i32, room: i32) -> i32 {
    let (m, r) = if map_room.is_empty() {
        (map, room)
    } else {
        let e = extract_map_room(map_room);
        (e.map, e.room)
    };
    if m == 0 || r == 0 {
        return 0;
    }
    db.room_cmd(m, r).unwrap_or(0)
}

// ---- textblock columns ----------------------------------------------------------------------

/// Port of `GetTextblockAction`: the `Action` text, `"none"` for 0 or an unknown number.
#[must_use]
pub fn get_textblock_action(db: &dyn TextblockData, n: i32) -> String {
    if n == 0 {
        return "none".into();
    }
    db.tbinfo(n)
        .map_or_else(|| "none".into(), |r| action(r).to_string())
}

/// Port of `GetTextblockLinkTo`: 0 for 0 or an unknown number.
#[must_use]
pub fn get_textblock_link_to(db: &dyn TextblockData, n: i32) -> i32 {
    if n == 0 {
        return 0;
    }
    db.tbinfo(n).map_or(0, |r| r.link_to)
}

/// Port of `GetTextblockCMDS`: the command words of every `command:action` line, joined with
/// `", "`. `max_length` of 0 means no limit.
#[must_use]
pub fn get_textblock_cmds(db: &dyn TextblockData, n: i32, max_length: i16) -> String {
    if n == 0 {
        return "none".into();
    }
    let Some(row) = db.tbinfo(n) else {
        return format!("Textblock {n} not found.");
    };
    cmds_of(action(row), max_length).unwrap_or_else(|_| "none".into())
}

fn cmds_of(s: &str, max_length: i16) -> Result<String, Vb6Error> {
    if s.is_empty() {
        return Ok("none".into());
    }
    let mut x1 = instr(1, s, ":");
    if x1 == 0 {
        return Ok("none".into());
    }
    let mut res = mid2(s, 1, x1 - 1)?;
    x1 += 1;
    let total = len(s);
    while x1 < total {
        x1 = instr(x1, s, "\n") + 1;
        if x1 == 1 {
            break;
        }
        let x2 = instr(x1, s, ":");
        if x2 == 0 {
            break;
        }
        res.push_str(", ");
        res.push_str(&mid2(s, x1, x2 - x1)?);
        x1 = x2 + 1;
    }
    let mut res = replace_cmd(&res);
    let max = i32::from(max_length);
    if max > 0 && len(&res) > max {
        res = format!("{}+", left(&res, max - 1)?);
    }
    Ok(res)
}

/// Port of `GetTextblockTrigger`: how textblock `n` is reached from `value`.
#[must_use]
pub fn get_textblock_trigger(db: &dyn TextblockData, n: i32, value: i32) -> String {
    if n == 0 {
        return "none".into();
    }
    let Some(row) = db.tbinfo(n) else {
        return format!("Textblock {n} not found.");
    };
    if row.link_to == value {
        return "[dialog link]".into();
    }
    let act = action(row);
    if instr(1, act, &format!("random {value}")) > 0 {
        return format!("[random {value}]");
    }
    if instr(1, act, &format!("text {value}")) > 0 {
        return format!("[text {value}]");
    }
    let key = format!(":{value}");
    let pos = instr(1, act, &key);
    if pos > 0 {
        return trigger_command(act, pos).unwrap_or_else(|_| "none".into());
    }
    "none".into()
}

fn trigger_command(act: &str, pos: i32) -> Result<String, Vb6Error> {
    let command = left(act, pos - 1)?;
    let mut x1 = 1;
    let mut z = 0;
    while x1 < len(&command) {
        z = instr(x1, &command, "\n");
        if z == 0 {
            z = x1;
            break;
        }
        x1 = z + 1;
    }
    let command = right(&command, len(&command) - z + 1)?;
    Ok(format!("[ask monster {command}]"))
}

/// Port of `GetTextblockCMDLine`: the action text after `command:` up to the line end. Pass the
/// textblock text as `data`, or `""` and a textblock number `n`.
#[must_use]
pub fn get_textblock_cmd_line(db: &dyn TextblockData, command: &str, data: &str, n: i32) -> String {
    if n == 0 && data.is_empty() {
        return "unknown".into();
    }
    let owned;
    let data = if data.is_empty() {
        let Some(row) = db.tbinfo(n) else {
            return "unknown".into();
        };
        owned = action(row).to_string();
        owned.as_str()
    } else {
        data
    };
    cmd_line_of(command, data).unwrap_or_default()
}

fn cmd_line_of(command: &str, data: &str) -> Result<String, Vb6Error> {
    let command = if right(command, 1)? == ":" {
        command.to_string()
    } else {
        format!("{command}:")
    };
    let mut x1 = instr(1, data, &command);
    if x1 == 0 {
        return Ok("none".into());
    }
    x1 += len(&command);
    let mut y = instr(x1, data, "\n");
    if y == 0 {
        y = len(data);
    }
    Ok(replace_cmd(&mid2(data, x1, y - x1)?))
}

/// Port of `GetTextblockCMDText`: the command words of the line holding `command`.
#[must_use]
pub fn get_textblock_cmd_text(db: &dyn TextblockData, command: &str, data: &str, n: i32) -> String {
    if n == 0 && data.is_empty() {
        return String::new();
    }
    let owned;
    let data = if data.is_empty() {
        let Some(row) = db.tbinfo(n) else {
            return String::new();
        };
        owned = action(row).to_string();
        owned.as_str()
    } else {
        data
    };
    cmd_text_of(command, data).unwrap_or_default()
}

fn cmd_text_of(command: &str, data: &str) -> Result<String, Vb6Error> {
    let x1 = instr(1, data, command);
    if x1 == 0 {
        return Ok(String::new());
    }
    let mut line = mid2(data, 1, x1)?;
    while instr(1, &line, "\n") > 0 {
        line = mid1(&line, instr(1, &line, "\n") + 1)?;
    }
    if instr(1, &line, ":") > 0 {
        line = left(&line, instr(1, &line, ":") - 1)?;
    }
    Ok(replace_cmd(&line))
}

// ---- teleports --------------------------------------------------------------------------------

/// The `teleport <room> [map]` scan shared by `TextBlockHasTeleport` and `GetTextblockTeleport`.
/// `x` is the position of "teleport " in the line. Returns (room, map).
fn teleport_args(line: &str, x: i32) -> Result<(i32, i32), Vb6Error> {
    let chars: Vec<char> = line.chars().collect();
    let total = chars.len() as i32;
    let mut y = x + 9;
    let mut x = y;
    let mut room = 0;
    let mut map = 0;
    while y <= total {
        match chars[(y - 1) as usize] {
            '0'..='9' => {}
            ' ' => {
                if y > x && room == 0 {
                    room = clng(val(&mid2(line, x, y - x)?))?;
                    x = y + 1;
                } else {
                    map = clng(val(&mid2(line, x, y - x)?))?;
                    break;
                }
            }
            _ => {
                if y > x && room == 0 {
                    room = clng(val(&mid2(line, x, y - x)?))?;
                } else {
                    map = clng(val(&mid2(line, x, y - x)?))?;
                }
                break;
            }
        }
        y += 1;
    }
    if y > total && y > x && room > 0 && map == 0 {
        map = clng(val(&mid2(line, x, y - x)?))?;
    }
    Ok((room, map))
}

/// Port of `TextBlockHasTeleport`: does textblock `tb` (or a textblock it rolls with `random`)
/// teleport to `find_room`? `strict` demands `find_map` to match; otherwise a named map only
/// has to agree when both are given. `nest` is the recursion depth, 0 from outside.
#[must_use]
pub fn textblock_has_teleport(
    db: &dyn TextblockData,
    tb: i32,
    find_room: i32,
    find_map: i32,
    strict: bool,
    nest: i16,
) -> bool {
    has_teleport(db, tb, find_room, find_map, strict, nest).unwrap_or(false)
}

fn has_teleport(
    db: &dyn TextblockData,
    tb: i32,
    find_room: i32,
    find_map: i32,
    strict: bool,
    nest: i16,
) -> Result<bool, Vb6Error> {
    if tb <= 0 {
        return Ok(false);
    }
    let Some(row) = db.tbinfo(tb) else {
        return Ok(false);
    };
    let data = action(row);
    let total = len(data);
    let mut pos = 1;
    while pos < total {
        let mut x = instr(pos, data, "\n");
        if x == 0 {
            x = total + 1;
        }
        let line = mid2(data, pos, x - pos)?;
        pos = x + 1;
        if nest < 5 {
            let mut x = instr(1, &line, "random ");
            while x > 0 {
                let rtb = extract_value(&mid1(&line, x)?, "random ");
                if rtb > 0
                    && rtb != tb
                    && has_teleport(db, rtb, find_room, find_map, strict, nest + 1)?
                {
                    return Ok(true);
                }
                x = instr(x + 1, &line, "random ");
            }
        }
        let x = instr(1, &line, "teleport ");
        if x > 0 {
            let (room, map) = teleport_args(&line, x)?;
            if room == find_room {
                if find_map > 0 && map > 0 && map != find_map {
                    continue;
                }
                if strict && map != find_map {
                    continue;
                }
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Port of `GetTextblockTeleport`: the destination of the first `teleport <room> [map]` in the
/// textblock, as `Some((room, map))`; the map is 0 when the command names none.
#[must_use]
pub fn get_textblock_teleport(db: &dyn TextblockData, tb: i32) -> Option<(i32, i32)> {
    if tb <= 0 {
        return None;
    }
    let row = db.tbinfo(tb)?;
    first_teleport(action(row)).ok().flatten()
}

fn first_teleport(data: &str) -> Result<Option<(i32, i32)>, Vb6Error> {
    let total = len(data);
    let mut pos = 1;
    while pos < total {
        let mut x = instr(pos, data, "\n");
        if x == 0 {
            x = total + 1;
        }
        let line = mid2(data, pos, x - pos)?;
        pos = x + 1;
        let x = instr(1, &line, "teleport ");
        if x > 0 {
            let (room, map) = teleport_args(&line, x)?;
            if room > 0 {
                return Ok(Some((room, map)));
            }
        }
    }
    Ok(None)
}

// ---- chests -------------------------------------------------------------------------------------

/// One column of MME's chest array: item number, chance to get it, chance of not having got it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChestEntry {
    pub item: i32,
    pub percent: Currency,
    pub fail: Currency,
}

/// MME's `nChestArray(1 To 3, 0 To n)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChestItems {
    pub entries: Vec<ChestEntry>,
}

impl Default for ChestItems {
    fn default() -> Self {
        Self::new()
    }
}

impl ChestItems {
    /// The array as the caller dimensions it, `ReDim nChestItems(1 To 3, 0)`: one empty column.
    #[must_use]
    pub fn new() -> Self {
        let zero = Currency::from_scaled(0);
        Self {
            entries: vec![ChestEntry {
                item: 0,
                percent: zero,
                fail: zero,
            }],
        }
    }
}

/// Port of `GetChestItems`: adds what chest textblock `tb` can give, with `random` textblocks
/// followed. `nest` is MME's `ByRef nNest` (0 from outside); `percent_mod` is the optional
/// share already rolled by the caller. A VB6 error inside leaves the array as it was and `nest`
/// raised, as MME's handler does.
pub fn get_chest_items(
    db: &dyn TextblockData,
    chest: &mut ChestItems,
    tb: i32,
    nest: &mut i32,
    percent_mod: Option<Currency>,
) {
    let _ = chest_items(db, chest, tb, nest, percent_mod);
}

fn chest_items(
    db: &dyn TextblockData,
    chest: &mut ChestItems,
    tb: i32,
    nest: &mut i32,
    percent_mod: Option<Currency>,
) -> Result<(), Vb6Error> {
    let Some(row) = db.tbinfo(tb) else {
        return Ok(());
    };
    let sdata = action(row).to_lowercase();
    if sdata == "\0" {
        return Ok(());
    }
    if *nest > 5 {
        return Ok(());
    }
    *nest += 1;
    let one = Currency::from_scaled(10_000);
    let zero = Currency::from_scaled(0);
    let mut pmod = percent_mod.unwrap_or(zero);
    if pmod <= zero {
        pmod = one;
    }
    let mut items: Vec<(i32, Currency)> = vec![(0, zero)];
    let total = len(&sdata);
    let mut pos = 1;
    let (mut per1, mut per2);
    per2 = 0;
    let mut percent;
    while pos < total {
        let x = instr(pos, &sdata, ":");
        if x > pos {
            per1 = clng(val(&mid2(&sdata, pos, x - pos)?))?;
            let diff = touchstone_vb6::sub_i32(per1, per2)?;
            percent = Currency::from_f64(f64::from(diff) / 100.0)?;
            per2 = per1;
            pos = x + 1;
            let mut x = instr(pos, &sdata, "\n");
            if x <= 0 {
                x = total;
            }
            let line = mid2(&sdata, pos, x - pos)?.to_lowercase();
            pos = x;

            let mut y = 1;
            loop {
                y = instr(y, &line, "giveitem ");
                if y <= 0 {
                    break;
                }
                let value = extract_value(&mid1(&line, y)?, "giveitem ");
                if let Some(e) = items.iter_mut().find(|e| e.0 == value) {
                    e.1 = e.1.checked_add(percent)?;
                } else {
                    items.push((value, percent));
                }
                y += 1;
            }
            y = 1;
            loop {
                y = instr(y, &line, "random ");
                if y <= 0 {
                    break;
                }
                let value = extract_value(&mid1(&line, y)?, "random ");
                if value > 0 {
                    get_chest_items(db, chest, value, nest, Some(percent.checked_mul(pmod)?));
                }
                y += 1;
            }
        } else {
            pos += 1;
        }
    }

    for &(item, pct) in &items {
        if item > 0 {
            let percent = pct;
            if let Some(c) = chest.entries.iter_mut().find(|c| c.item == item) {
                c.percent = c
                    .percent
                    .checked_add(c.fail.checked_mul(percent)?.checked_mul(pmod)?)?;
                c.fail = c.fail.checked_mul(one.checked_sub(percent)?)?;
            } else {
                let got = percent.checked_mul(pmod)?;
                chest.entries.push(ChestEntry {
                    item,
                    percent: got,
                    fail: one.checked_sub(got)?,
                });
            }
        }
    }
    *nest -= 1;
    Ok(())
}

// ---- Obtained From room commands ----------------------------------------------------------------

/// MME's `tRoomCmdSource`: one `Room(<command>|..) <map/room>|..[(<chance>%)]` entry.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RoomCmdSource {
    pub commands: String,
    pub label: String,
    pub room_list: String,
    pub first_room: String,
    pub room_count: i32,
    /// -1 always, 0 unknown, else the percent.
    pub chance: f64,
    pub cut: bool,
    pub action: bool,
}

fn is_room_cmd_number(s: &str) -> bool {
    let n = len(s);
    n != 0 && n <= 9 && s.chars().all(|c| c.is_ascii_digit())
}

fn is_room_cmd_action(command: &str) -> bool {
    let c = trim(command).to_lowercase();
    if c.is_empty() {
        return false;
    }
    let words: Vec<&str> = c.split(' ').collect();
    if words.len() < 2 {
        return false;
    }
    if !words[1..].iter().all(|w| is_room_cmd_number(w)) {
        return false;
    }
    matches!(
        words[0],
        "message"
            | "text"
            | "random"
            | "giveitem"
            | "takeitem"
            | "checkitem"
            | "clearitem"
            | "roomitem"
            | "failroomitem"
            | "teleport"
            | "cast"
            | "summon"
            | "delay"
            | "price"
            | "class"
            | "race"
            | "minlevel"
            | "maxlevel"
            | "checkability"
            | "testability"
            | "failability"
            | "removeability"
    )
}

/// `ParseRoomCmdToken`: `map/room` or `map/first-last` as (map, first, last).
fn parse_room_cmd_token(token: &str) -> Option<(i32, i32, i32)> {
    let token = trim(token);
    let slash = instr(1, token, "/");
    if slash < 2 {
        return None;
    }
    let s_map = left(token, slash - 1).ok()?;
    let dash = instr(slash, token, "-");
    let (first, last) = if dash > 0 {
        (
            mid2(token, slash + 1, dash - slash - 1).ok()?,
            mid1(token, dash + 1).ok()?,
        )
    } else {
        let f = mid1(token, slash + 1).ok()?;
        (f.clone(), f)
    };
    if !is_room_cmd_number(&s_map) || !is_room_cmd_number(&first) || !is_room_cmd_number(&last) {
        return None;
    }
    let n_map = val(&s_map) as i32;
    let n_first = val(&first) as i32;
    let n_last = val(&last) as i32;
    if n_map < 1 || n_first < 1 || n_last < n_first || n_last - n_first > 100_000 {
        return None;
    }
    Some((n_map, n_first, n_last))
}

/// `ParseRoomCmdEntry`.
fn parse_room_cmd_entry(entry: &str, cut_here: bool) -> Option<RoomCmdSource> {
    let mut t = RoomCmdSource {
        chance: -1.0,
        cut: cut_here,
        ..Default::default()
    };
    let mut cut_here = cut_here;
    let pos = instr(6, entry, ")");
    if pos == 0 {
        return None;
    }
    t.commands = mid2(entry, 6, pos - 6).ok()?;
    let mut rooms = trim(&mid1(entry, pos + 1).ok()?).to_string();
    let pos = instr(1, &rooms, "(");
    if pos > 0 {
        let mut chance = trim(&mid1(&rooms, pos + 1).ok()?).to_string();
        rooms = left(&rooms, pos - 1).ok()?;
        if right(&chance, 2).ok()? == "%)" {
            chance = trim(&left(&chance, len(&chance) - 2).ok()?).to_string();
        } else {
            chance = "?".into();
        }
        if chance == "?" || chance.is_empty() {
            t.chance = 0.0;
        } else {
            t.chance = val(&chance.replace(',', "."));
            if t.chance < 0.0 {
                t.chance = 0.0;
            }
        }
        cut_here = false;
        t.cut = false;
    } else if cut_here {
        t.chance = 0.0;
    }
    let tokens: Vec<&str> = rooms.split('|').collect();
    let mut last = tokens.len() as i32 - 1;
    if cut_here {
        last -= 1;
    }
    for tok in tokens.iter().take((last + 1).max(0) as usize) {
        if let Some((map, first, lastroom)) = parse_room_cmd_token(tok) {
            t.room_list = auto_append(&t.room_list, trim(tok), "|");
            if t.room_count == 0 {
                t.first_room = format!("{map}/{first}");
            }
            t.room_count = t.room_count.checked_add(lastroom - first + 1)?;
        }
    }
    if t.room_count == 0 {
        return None;
    }
    for c in t.commands.clone().split('|') {
        let plain = c.replace('*', "");
        if !trim(&plain).is_empty() && !is_room_cmd_action(&plain) {
            t.label = auto_append(&t.label, &format!("\"{}\"", trim(&plain)), " / ");
        }
    }
    t.action = t.label.is_empty();
    Some(t)
}

/// Port of `SplitRoomCmdSources`: splits the `Room(..)` entries off an item's "Obtained From"
/// text. Returns the remaining text and the entries.
#[must_use]
pub fn split_room_cmd_sources(obtained: &str) -> (String, Vec<RoomCmdSource>) {
    let mut out = Vec::new();
    let start = if left(obtained, 5).unwrap_or_default() == "Room(" {
        1
    } else {
        let s = instr(1, obtained, ", Room(");
        if s == 0 {
            return (obtained.to_string(), out);
        }
        s + 2
    };
    let mut body = obtained.to_string();
    let mut cut = false;
    if right(&body, 2).unwrap_or_default() == "+\0" {
        cut = true;
        body = left(&body, len(&body) - 2).unwrap_or_default();
    } else if right(&body, 1).unwrap_or_default() == "+" {
        cut = true;
        body = left(&body, len(&body) - 1).unwrap_or_default();
    }
    let mut rest = left(&body, start - 1).unwrap_or_default();
    if right(&rest, 2).unwrap_or_default() == ", " {
        rest = left(&rest, len(&rest) - 2).unwrap_or_default();
    }
    let tail = mid1(&body, start).unwrap_or_default();
    let parts: Vec<&str> = tail.split(", ").collect();
    for (x, part) in parts.iter().enumerate() {
        let is_last = x == parts.len() - 1;
        if left(part, 5).unwrap_or_default() == "Room(" {
            if let Some(src) = parse_room_cmd_entry(part, cut && is_last) {
                out.push(src);
            }
        } else if !(cut && is_last) {
            rest = auto_append(&rest, part, ", ");
        }
    }
    (rest, out)
}

/// Port of `ExpandRoomCmdRooms`: a room list such as `17/278|17/2073-2096` as `map/room` texts,
/// at most 5000.
#[must_use]
pub fn expand_room_cmd_rooms(room_list: &str) -> Vec<String> {
    let mut out = Vec::new();
    if room_list.is_empty() {
        return out;
    }
    for tok in room_list.split('|') {
        if let Some((map, first, last)) = parse_room_cmd_token(tok) {
            for room in first..=last {
                if out.len() >= 5000 {
                    return out;
                }
                out.push(format!("{map}/{room}"));
            }
        }
    }
    out
}

/// Port of `ObtainedFromIsGettable`: can the item be got by killing an NPC or running a
/// textblock (not only lying in a room)?
#[must_use]
pub fn obtained_from_is_gettable(obtained: &str) -> bool {
    let text = if instr(1, obtained, "Room(") > 0 {
        split_room_cmd_sources(obtained).0
    } else {
        obtained.to_string()
    };
    if instr_text(&text, "NPC #") > 0 {
        true
    } else {
        instr_text(&text, "Textblock #") > 0 && instr_text(&text, "Room ") == 0
    }
}
