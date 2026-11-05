//! Game-text inventory parser (FAM-22, stock only). Card P3-38.
//!
//! Ports the text half of MME's `modItemParse`: pasted MajorMUD inventory, key and "You notice"
//! text goes in, four item lists come out. Nothing here touches the Items table; matching parsed
//! names to item rows, shop prices and the MegaMUD stat-line parser are card P3-39.
//!
//! The lists keep MME's shape: an empty list is a single empty string (`[""]`), a name with a
//! count is `"name (N)"`, and equipped entries keep their `(Slot)` text.
//!
//! Errors: MME's `HandleError` shows a message and carries on. Here the result lists each such
//! event in [`ParseOutput::handled`] (a Long overflow from a huge count is the only one reachable).

mod vbstr;

pub use touchstone_vb6::is_numeric;
pub use vbstr::{instr, instr_ci, instrrev, lcase, left, len, mid, mid_from, right, rtrim, trim};

use touchstone_vb6::{Vb6Error, add_i32, clng, val};

const HDR_INV: &str = "You are carrying";
const HDR_KEYS: &str = "You have the following keys";
const HDR_NOKEY: &str = "You have no keys";
const HDR_NOTC: &str = "You notice";

/// MME's `ItemParseResult`: the four parsed lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemParseResult {
    /// `"Item (Slot)"`, de-duplicated case-insensitively.
    pub equipped: Vec<String>,
    /// `"name"` or `"name (N)"`, lower case.
    pub inventory: Vec<String>,
    /// Keys, singularised when counted, lower case.
    pub keys: Vec<String>,
    /// Ground items summed over rooms (max within one room), lower case.
    pub ground: Vec<String>,
}

impl Default for ItemParseResult {
    fn default() -> Self {
        let one = || vec![String::new()];
        Self {
            equipped: one(),
            inventory: one(),
            keys: one(),
            ground: one(),
        }
    }
}

/// An error MME's `HandleError` would have shown, from the procedure whose handler caught it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandledError {
    pub source: &'static str,
    pub number: u32,
}

/// What [`parse_game_text_inventory`] returns.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParseOutput {
    pub items: ItemParseResult,
    pub handled: Vec<HandledError>,
}

fn handled(source: &'static str, e: Vb6Error) -> HandledError {
    HandledError {
        source,
        number: e.code(),
    }
}

/// Port of modItemParse ParseGameTextInventory.
#[must_use]
pub fn parse_game_text_inventory(input: &str) -> ParseOutput {
    let mut out = ParseOutput::default();
    let norm = input.replace('\r', "");
    let mut lines: Vec<String> = norm.split('\n').map(String::from).collect();

    // Pass 1: scan lines and collect blobs. `i` may step back to -1 (the collector rewinds one
    // line so the main loop sees a header split off the middle of a line).
    let mut i: isize = 0;
    while i < lines.len() as isize {
        let line = trim(&lines[i as usize]).to_string();
        if starts_with_ci(&line, HDR_INV) {
            let blob = collect_inventory_blob(&mut lines, &mut i);
            parse_inventory_blob(&blob, &mut out.items);
        } else if starts_with_ci(&line, HDR_KEYS) || starts_with_ci(&line, HDR_NOKEY) {
            let blob = collect_keys_blob(&mut lines, &mut i);
            parse_keys_blob(&blob, &mut out.items);
        } else if starts_with_ci(&line, HDR_NOTC) {
            let blob = collect_notice_blob(&mut lines, &mut i);
            parse_notice_blob(&blob, &mut out.items);
        }
        i += 1;
    }

    // Pass 1b: rebuild the ground list room by room.
    if let Err(e) = consolidate_ground_by_room(&norm, &mut out.items) {
        out.handled.push(handled("ConsolidateGroundByRoom", e));
    }

    // Pass 2: consolidate; pass 3: dedupe equipped.
    for (list, is_keys) in [
        (&mut out.items.inventory, false),
        (&mut out.items.ground, false),
        (&mut out.items.keys, true),
    ] {
        if let Err(e) = consolidate_list(list, is_keys) {
            out.handled.push(handled("ConsolidateList", e));
        }
    }
    dedupe_equipped(&mut out.items.equipped);
    out
}

/// Port of modItemParse ItemNameInParseResult. A Long overflow from a huge count is caught by
/// MME's handler, which leaves the result False.
#[must_use]
pub fn item_name_in_parse_result(
    items: &ItemParseResult,
    name: &str,
    check_equipped: bool,
    check_inventory: bool,
    check_keys: bool,
) -> bool {
    let name = trim(name);
    if name.is_empty() {
        return false;
    }
    let same = |a: &str| lcase(a) == lcase(name);
    if check_equipped
        && safe_array_has_data(&items.equipped)
        && items
            .equipped
            .iter()
            .any(|e| same(&normalize_equipped_name(e)))
    {
        return true;
    }
    for (on, list) in [
        (check_inventory, &items.inventory),
        (check_keys, &items.keys),
    ] {
        if on && safe_array_has_data(list) {
            for e in list {
                match parse_name_and_qty(e) {
                    Ok((base, _)) => {
                        if same(&base) {
                            return true;
                        }
                    }
                    Err(_) => return false,
                }
            }
        }
    }
    false
}

/// Port of modItemParse ParseNameAndQty: strips trailing `(digits)` groups from `raw` and
/// returns the base name and the quantity (the group stripped last, if it is above 0, else 1).
///
/// # Errors
/// Overflow (6) when a group's number does not fit a Long.
pub fn parse_name_and_qty(raw: &str) -> Result<(String, i32), Vb6Error> {
    let mut s = trim(raw).to_string();
    let mut last_trailing = 0;
    loop {
        let p_close = instrrev(&s, ")", None);
        if p_close == 0 {
            break;
        }
        let p_open = instrrev(&s, "(", Some(p_close));
        if p_open == 0 || p_open >= p_close || p_close != len(&s) {
            break;
        }
        let inside = mid(&s, p_open + 1, p_close - p_open - 1);
        if is_numeric(trim(&inside)) {
            last_trailing = clng(val(&inside))?;
            s = rtrim(&left(&s, p_open - 1)).to_string();
        } else {
            break;
        }
        s = rtrim(&s).to_string();
    }
    let qty = if last_trailing > 0 { last_trailing } else { 1 };
    Ok((s, qty))
}

/// Port of modItemParse StartsWithCI.
#[must_use]
pub fn starts_with_ci(s: &str, prefix: &str) -> bool {
    lcase(&left(s, len(prefix))) == lcase(prefix)
}

fn is_movement_command(line: &str) -> bool {
    let mut s = lcase(trim(line));
    let p = instr(&s, "]:");
    if p > 0 {
        s = mid_from(&s, p + 2);
    }
    matches!(
        trim(&s),
        "n" | "s"
            | "e"
            | "w"
            | "nw"
            | "ne"
            | "sw"
            | "se"
            | "north"
            | "south"
            | "east"
            | "west"
            | "northwest"
            | "northeast"
            | "southwest"
            | "southeast"
    )
}

fn is_section_boundary(line: &str) -> bool {
    let sl = lcase(trim(line));
    if sl.is_empty() {
        return true;
    }
    let prefixes = [
        HDR_INV,
        HDR_KEYS,
        HDR_NOKEY,
        HDR_NOTC,
        "wealth:",
        "encumbrance:",
        "name:",
        "race:",
        "class:",
        "also here:",
        "you have no keys",
    ];
    prefixes.iter().any(|p| starts_with_ci(&sl, &lcase(p)))
        || left(&sl, 1) == "["
        || instr_ci(&sl, "obvious exits:") > 0
        || instr_ci(&sl, "*combat") > 0
}

fn should_stop_section(line: &str, past_first: bool) -> bool {
    past_first && is_section_boundary(line)
}

/// Position (1-based) of the earliest section header inside `line`, ignoring one at the very
/// start; 0 when none.
fn find_inline_boundary_pos(line: &str) -> usize {
    let sl = lcase(line);
    let mut best = 0;
    for h in [
        HDR_KEYS,
        HDR_NOKEY,
        HDR_INV,
        HDR_NOTC,
        "wealth:",
        "encumbrance:",
        "you have no keys",
    ] {
        let p = instr(&sl, &lcase(h));
        if p > 1 && (best == 0 || p < best) {
            best = p;
        }
    }
    best
}

/// If `line` has a header in its middle, appends the text before it to `s`, rewrites the line to
/// start at the header and reports true (the collector then stops).
fn split_inline(lines: &mut [String], j: usize, line: &str, s: &mut String) -> bool {
    let pos = find_inline_boundary_pos(line);
    if pos <= 1 {
        return false;
    }
    let left_part = trim(&left(line, pos - 1)).to_string();
    if !left_part.is_empty() {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(&left_part);
    }
    lines[j] = mid_from(line, pos);
    true
}

fn append_line(s: &mut String, line: &str) {
    if !s.is_empty() {
        s.push(' ');
    }
    s.push_str(line);
}

fn collect_inventory_blob(lines: &mut [String], i: &mut isize) -> String {
    let mut s = String::new();
    let start = *i as usize;
    let mut j = start;
    while j < lines.len() {
        let line = trim(&lines[j]).to_string();
        if should_stop_section(&line, j != start) {
            break;
        }
        if split_inline(lines, j, &line, &mut s) {
            break;
        }
        append_line(&mut s, &line);
        j += 1;
    }
    *i = j as isize - 1;
    s
}

fn collect_keys_blob(lines: &mut [String], i: &mut isize) -> String {
    let mut s = String::new();
    let start = *i as usize;
    let mut j = start;
    while j < lines.len() {
        let line = trim(&lines[j]).to_string();
        if j == start && starts_with_ci(&line, "you have no keys") {
            s = line;
            j += 1;
            break;
        }
        if should_stop_section(&line, j != start) {
            break;
        }
        if split_inline(lines, j, &line, &mut s) {
            break;
        }
        append_line(&mut s, &line);
        if right(&line, 1) == "." {
            j += 1;
            break;
        }
        j += 1;
    }
    *i = j as isize - 1;
    s
}

fn collect_notice_blob(lines: &mut [String], i: &mut isize) -> String {
    let mut s = String::new();
    let start = *i as usize;
    let mut j = start;
    while j < lines.len() {
        let line = trim(&lines[j]).to_string();
        if is_bracket_line(&line) {
            j += 1;
            continue;
        }
        if should_stop_section(&line, j != start) {
            break;
        }
        if split_inline(lines, j, &line, &mut s) {
            break;
        }
        append_line(&mut s, &line);
        if lcase(&right(&line, 5)) == "here." {
            j += 1;
            break;
        }
        j += 1;
    }
    *i = j as isize - 1;
    s
}

fn parse_inventory_blob(blob: &str, out: &mut ItemParseResult) {
    let mut s = blob.to_string();
    if starts_with_ci(&s, HDR_INV) {
        s = trim(&mid_from(&s, len(HDR_INV) + 1)).to_string();
    }
    let s = squeeze_spaces(&s.replace('\n', " "));
    for tok in split_to_array(&s) {
        let it = trim(&tok);
        if it.is_empty() || is_bracket_line(it) || is_cash_item(it) {
            continue;
        }
        if is_equipped_item(it) {
            add_string(&mut out.equipped, it);
        } else {
            add_string(&mut out.inventory, it);
        }
    }
}

fn parse_keys_blob(blob: &str, out: &mut ItemParseResult) {
    let mut s = blob.to_string();
    if starts_with_ci(&s, HDR_NOKEY) {
        return;
    } else if starts_with_ci(&s, HDR_KEYS) {
        s = trim(&mid_from(&s, len(HDR_KEYS) + 1)).to_string();
        if left(&s, 1) == ":" {
            s = trim(&mid_from(&s, 2)).to_string();
        }
    }
    if instr(&lcase(&s), "no keys") > 0 {
        return;
    }
    let mut s = rtrim(&s).to_string();
    if right(&s, 1) == "." {
        s = left(&s, len(&s) - 1);
    }
    let s = squeeze_spaces(&s.replace('\n', " "));
    for tok in split_to_array(&s) {
        let it = trim(&tok);
        if !it.is_empty() {
            add_string(&mut out.keys, it);
        }
    }
}

fn parse_notice_blob(blob: &str, out: &mut ItemParseResult) {
    let mut s = blob.to_string();
    if starts_with_ci(&s, HDR_NOTC) {
        s = trim(&mid_from(&s, len(HDR_NOTC) + 1)).to_string();
    }
    if lcase(&right(&s, 5)) == "here." {
        s = left(&s, len(&s) - 5);
    }
    let s = squeeze_spaces(&strip_bracketed_chunks(&s).replace('\n', " "));
    for tok in split_to_array(&s) {
        let it = trim(&tok);
        if it.is_empty() || is_bracket_line(it) || is_cash_item(it) {
            continue;
        }
        add_string(&mut out.ground, it);
    }
}

/// One room's notice items: name, highest count seen.
struct RoomAgg {
    key: String,
    names: Vec<String>,
    counts: Vec<i32>,
}

fn get_or_create_room(rooms: &mut Vec<RoomAgg>, key: &str) -> usize {
    if let Some(i) = rooms.iter().position(|r| r.key == key) {
        return i;
    }
    rooms.push(RoomAgg {
        key: key.to_string(),
        names: Vec::new(),
        counts: Vec::new(),
    });
    rooms.len() - 1
}

fn add_notice_to_room(rooms: &mut Vec<RoomAgg>, key: &str, span: &str) -> Result<(), Vb6Error> {
    let idx = get_or_create_room(rooms, key);
    let mut span = span.to_string();
    if starts_with_ci(&span, HDR_NOTC) {
        span = trim(&mid_from(&span, len(HDR_NOTC) + 1)).to_string();
    }
    if lcase(&right(&span, 5)) == "here." {
        span = left(&span, len(&span) - 5);
    }
    let span = squeeze_spaces(&strip_bracketed_chunks(&span).replace('\n', " "));
    for tok in split_to_array(&span) {
        let t = trim(&tok);
        if t.is_empty() || is_cash_item(t) {
            continue;
        }
        let (nm, mut qty) = parse_name_and_qty(t)?;
        let nm = lcase(&nm);
        if qty < 1 {
            qty = 1;
        }
        let room = &mut rooms[idx];
        match room.names.iter().position(|n| *n == nm) {
            Some(k) => {
                if qty > room.counts[k] {
                    room.counts[k] = qty;
                }
            }
            None => {
                room.names.push(nm);
                room.counts.push(qty);
            }
        }
    }
    Ok(())
}

fn room_key(name: &str, exits: &str) -> String {
    lcase(&format!("{}||{}", trim(name), trim(exits)))
}

/// Where a pending notice goes when the room ends without an exits line.
fn flush_key(cur_room: &str, cur_exits: &str) -> String {
    if cur_exits.is_empty() {
        "__unknown||__unknown".to_string()
    } else {
        room_key(cur_room, cur_exits)
    }
}

fn consolidate_ground_by_room(all: &str, out: &mut ItemParseResult) -> Result<(), Vb6Error> {
    let v: Vec<String> = all
        .replace('\r', "")
        .split('\n')
        .map(String::from)
        .collect();
    let mut rooms: Vec<RoomAgg> = Vec::new();
    let (mut cur_room, mut cur_exits, mut pending) = (String::new(), String::new(), String::new());

    let mut i = 0usize;
    while i < v.len() {
        let line = trim(&v[i]).to_string();
        if line.is_empty() {
            i += 1;
            continue;
        }
        if is_movement_command(&line) {
            if !pending.is_empty() {
                add_notice_to_room(&mut rooms, &flush_key(&cur_room, &cur_exits), &pending)?;
                pending.clear();
            }
            cur_room.clear();
            cur_exits.clear();
            i += 1;
            continue;
        }
        if !starts_with_ci(&line, HDR_NOTC)
            && !starts_with_ci(&line, "also here:")
            && instr_ci(&line, "obvious exits:") == 0
            && left(&line, 1) != "["
            && !starts_with_ci(&line, "name:")
            && !starts_with_ci(&line, "race:")
            && !starts_with_ci(&line, "class:")
            && !starts_with_ci(&line, HDR_INV)
            && !starts_with_ci(&line, HDR_KEYS)
            && !starts_with_ci(&line, HDR_NOKEY)
        {
            cur_room.clone_from(&line);
        }
        if starts_with_ci(&line, HDR_NOTC) {
            // the full "You notice ... here." span, which may wrap
            let mut span = line;
            while i + 1 < v.len() {
                if lcase(&right(trim(&v[i]), 5)) == "here." {
                    break;
                }
                i += 1;
                span.push(' ');
                span.push_str(trim(&v[i]));
            }
            pending = squeeze_spaces(&strip_bracketed_chunks(&span));
            i += 1;
            continue;
        }
        if instr_ci(&line, "obvious exits:") > 0 {
            cur_exits = line;
            if !pending.is_empty() {
                add_notice_to_room(&mut rooms, &room_key(&cur_room, &cur_exits), &pending)?;
                pending.clear();
            }
        }
        i += 1;
    }
    if !pending.is_empty() {
        add_notice_to_room(&mut rooms, &flush_key(&cur_room, &cur_exits), &pending)?;
    }

    // MAX within a room, then SUM across rooms
    let mut tot_n: Vec<String> = Vec::new();
    let mut tot_c: Vec<i32> = Vec::new();
    for r in &rooms {
        for (nm, &c) in r.names.iter().zip(&r.counts) {
            match tot_n.iter().position(|n| n == nm) {
                Some(k) => tot_c[k] = add_i32(tot_c[k], c)?,
                None => {
                    tot_n.push(nm.clone());
                    tot_c.push(c);
                }
            }
        }
    }
    out.ground = if tot_n.is_empty() {
        vec![String::new()]
    } else {
        tot_n
            .iter()
            .zip(&tot_c)
            .map(|(n, &c)| {
                if c > 1 {
                    format!("{n} ({c})")
                } else {
                    n.clone()
                }
            })
            .collect()
    };
    Ok(())
}

fn consolidate_list(a: &mut Vec<String>, is_keys_list: bool) -> Result<(), Vb6Error> {
    if !safe_array_has_data(a) {
        return Ok(());
    }
    let mut keys: Vec<String> = Vec::new();
    let mut counts: Vec<i32> = Vec::new();
    for item in a.iter().filter(|s| !s.is_empty()) {
        let (nm, cnt) = parse_count_and_name(item, is_keys_list)?;
        match keys.iter().position(|k| *k == nm) {
            Some(k) => counts[k] = add_i32(counts[k], cnt)?,
            None => {
                keys.push(nm);
                counts.push(cnt);
            }
        }
    }
    *a = if keys.is_empty() {
        vec![String::new()]
    } else {
        keys.iter()
            .zip(&counts)
            .map(|(k, &c)| {
                if c > 1 {
                    format!("{k} ({c})")
                } else {
                    k.clone()
                }
            })
            .collect()
    };
    Ok(())
}

/// Port of modItemParse ParseCountAndName: splits a list entry into a lower-case name and a
/// count, from a leading number (`3 flasks`) or a trailing one (`flasks (3)`). With
/// `is_keys_list` it also trims plural endings (`keys`, `idols`) and singularises counted names.
///
/// # Errors
/// Overflow (6) when a number does not fit a Long.
pub fn parse_count_and_name(token: &str, is_keys_list: bool) -> Result<(String, i32), Vb6Error> {
    let mut s = trim(token).to_string();
    while !s.is_empty() && matches!(right(&s, 1).as_str(), "." | ";") {
        s = left(&s, len(&s) - 1);
        s = rtrim(&s).to_string();
    }
    let mut last_trailing = 0;
    loop {
        let p_close = instrrev(&s, ")", None);
        if p_close == 0 {
            break;
        }
        let p_open = instrrev(&s, "(", Some(p_close));
        if p_open == 0 || p_open >= p_close {
            break;
        }
        let inside = mid(&s, p_open + 1, p_close - p_open - 1);
        if is_numeric(trim(&inside)) {
            last_trailing = clng(val(&inside))?;
            s = rtrim(&left(&s, p_open - 1)).to_string();
        } else {
            break;
        }
        s = rtrim(&s).to_string();
    }

    let (mut count, mut name);
    let p = instr(&s, " ");
    if p > 0 && is_numeric(&left(&s, p - 1)) {
        count = clng(val(&left(&s, p - 1)))?;
        name = trim(&mid_from(&s, p + 1)).to_string();
    } else {
        count = 1;
        name = s;
    }
    name = squeeze_spaces(trim(&name));
    if count == 1 && last_trailing > 1 {
        count = last_trailing;
    }
    if is_keys_list {
        if lcase(&right(&name, 5)) == " keys" {
            name = left(&name, len(&name) - 1);
        }
        if lcase(&right(&name, 6)) == " idols" {
            name = left(&name, len(&name) - 1);
        }
        if count > 1 {
            name = singularize_simple(&name);
        }
    }
    Ok((lcase(&name), count))
}

/// Port of modItemParse SingularizeSimple. Case-sensitive. Its `xes` test compares four
/// characters to three, so it only fires on a three-character string (`boxes` becomes `boxe`).
#[must_use]
pub fn singularize_simple(s: &str) -> String {
    let s = trim(s);
    let l = len(s);
    if l == 0 {
        return s.to_string();
    }
    if l >= 3 && right(s, 3) == "ies" {
        return format!("{}y", left(s, l - 3));
    }
    if l >= 2 && right(s, 2) == "es" {
        let r4 = right(s, 4);
        if r4 == "sses" || r4 == "ches" || r4 == "shes" || r4 == "xes" {
            return left(s, l - 2);
        }
    }
    if l >= 2 && right(s, 1) == "s" && right(s, 2) != "ss" {
        left(s, l - 1)
    } else {
        s.to_string()
    }
}

fn dedupe_equipped(a: &mut Vec<String>) {
    if !safe_array_has_data(a) {
        return;
    }
    let mut seen: Vec<String> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    for e in a.iter() {
        let key = lcase(trim(e));
        if key.is_empty() || seen.contains(&key) {
            continue;
        }
        seen.push(key);
        out.push(trim(e).to_string());
    }
    *a = if out.is_empty() {
        vec![String::new()]
    } else {
        out
    };
}

/// Port of modItemParse NormalizeEquippedName: drops a trailing `(Slot)`.
#[must_use]
pub fn normalize_equipped_name(raw: &str) -> String {
    let s = trim(raw);
    let p_close = instrrev(s, ")", None);
    if p_close > 0 {
        let p_open = instrrev(s, "(", Some(p_close));
        if p_open > 0 && p_close == len(s) {
            return trim(&left(s, p_open - 1)).to_string();
        }
    }
    s.to_string()
}

/// Port of modItemParse SplitToArray: splits on commas, trims each piece, drops empty ones.
#[must_use]
pub fn split_to_array(s: &str) -> Vec<String> {
    s.split(',')
        .map(trim)
        .filter(|t| !t.is_empty())
        .map(String::from)
        .collect()
}

/// Port of modItemParse SqueezeSpaces: runs of spaces become one.
#[must_use]
pub fn squeeze_spaces(s: &str) -> String {
    let mut s = s.to_string();
    while s.contains("  ") {
        s = s.replace("  ", " ");
    }
    s
}

/// Port of modItemParse SafeArrayHasData: some entry is not blank.
#[must_use]
pub fn safe_array_has_data(a: &[String]) -> bool {
    a.iter().any(|s| !trim(s).is_empty())
}

/// Port of modItemParse ArrayCount.
#[must_use]
pub fn array_count(a: &[String]) -> usize {
    if safe_array_has_data(a) { a.len() } else { 0 }
}

/// Port of modItemParse AddString: appends, reusing the single empty slot of a fresh list.
pub fn add_string(a: &mut Vec<String>, s: &str) {
    if a.len() == 1 && a[0].is_empty() {
        a[0] = s.to_string();
    } else {
        a.push(s.to_string());
    }
}

/// Port of modItemParse StripBracketedChunks: removes `[...]` including nested ones; an
/// unclosed `[` swallows the rest, a stray `]` is dropped.
#[must_use]
pub fn strip_bracketed_chunks(s: &str) -> String {
    let mut res = String::new();
    let mut depth = 0u32;
    for ch in s.chars() {
        match ch {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            c if depth == 0 => res.push(c),
            _ => {}
        }
    }
    res
}

/// Port of modItemParse IsCashItem: a coin word as a whole word.
#[must_use]
pub fn is_cash_item(item: &str) -> bool {
    let sl = format!(" {} ", lcase(trim(item)));
    [
        " gold crown ",
        " gold crowns ",
        " silver noble ",
        " silver nobles ",
        " copper farthing ",
        " copper farthings ",
        " platinum piece ",
        " platinum pieces ",
        " runic coin ",
        " runic coins ",
    ]
    .iter()
    .any(|c| sl.contains(c))
}

/// Port of modItemParse IsEquippedItem: ends in `)` with a ` (` before it.
#[must_use]
pub fn is_equipped_item(item: &str) -> bool {
    if right(item, 1) != ")" {
        return false;
    }
    let p = instrrev(item, " (", None);
    p > 0 && p < len(item)
}

/// Port of modItemParse IsBracketLine: starts with `[` and ends with `]`.
#[must_use]
pub fn is_bracket_line(line: &str) -> bool {
    let t = trim(line);
    len(t) >= 2 && left(t, 1) == "[" && right(t, 1) == "]"
}
