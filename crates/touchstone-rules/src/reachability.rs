//! Monster and room reachability, and routes (FEAT-REACH, stock only). Card P3-45.
//!
//! Ports modReachability: [`compute`] is `ReachEnsureComputed` (load the tables, build the exits, room
//! commands, room spells, NPC conversations and item-gated teleports into one edge list, resolve every
//! monster's rooms, then mark which monsters a character can reach in each level band). The queries are
//! `ReachIsMonsterOK`, `ReachMonsterInfo`, `ReachRoomStatus`, `ReachRoute` and `ReachRoomRoute`. The
//! detail pane's reach rows (modMain `AddMonsterReachRows`, `AddRouteRows`, `AddRouteAltRow`) are
//! [`monster_reach_rows`].
//!
//! Indexes are 1-based as in MME (index 0 means "none") and edges keep MME's order, which decides the
//! "first found" route and the order a breadth-first search meets rooms. VB `Dictionary` objects that
//! MME iterates are insertion ordered ([`OrderedMap`]). MME's progress bar, its error handlers (a failed
//! build leaves the result uncomputed) and `ReachWriteDebugCSV` are not ported; [`Reach::bands_text`] is the
//! per-monster `BandsText` of that CSV.
//!
//! Results never change after [`compute`], so the lazy structures MME builds on first use (reverse and
//! random-group adjacency, the last `VisitAtLevel` answer) are built eagerly or recomputed per query.

// The ports keep MME's argument lists and 1-based loops, so a few style lints are off for this module.
#![allow(
    clippy::too_many_arguments,
    clippy::wrong_self_convention,
    clippy::needless_range_loop,
    clippy::collapsible_if,
    clippy::if_same_then_else
)]

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashMap, HashSet};

use touchstone_model::World;

use crate::detail::monster::{MonsterCell, MonsterRow};

const START_MAP: i32 = 1;
const START_ROOM: i32 = 297;
const LEVEL_CAP: i32 = 255;
const NO_MAX: i32 = 9999;
const MAX_CHAIN_DEPTH: i32 = 10;
const MAX_RANDOM_DESTS: usize = 20;
const MAX_RESOLVE_DEPTH: i32 = 8;
const MAX_ABILS: usize = 20;

const ABIL_TELEPORT_ROOM: i32 = 140;
const ABIL_TELEPORT_MAP: i32 = 141;
const ABIL_TEXTBLOCK: i32 = 148;
const ABIL_ENDCAST: i32 = 151;

const MON_NOT_IN_GAME: u8 = 1;
const MON_NO_LOCATION: u8 = 2;
const MON_LOCATED: u8 = 3;

/// `ReachStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReachStatus {
    /// Not computed, unknown monster, or the monster is not located / not in the game (callers show nothing).
    Unknown = 0,
    Yes = 1,
    /// Unreachable; the gate fields are filled in.
    No = 2,
    /// Unreachable and no level gate lies on any path (no known path).
    NoGate = 3,
}

/// Step kind flags (`REACH_STEP_*`).
pub const REACH_STEP_ACTION: i32 = 1;
pub const REACH_STEP_LEVEL: i32 = 2;
pub const REACH_STEP_RESTRICT: i32 = 4;
pub const REACH_STEP_MAPCHANGE: i32 = 8;
pub const REACH_STEP_RANDOM: i32 = 16;
pub const REACH_STEP_COIN: i32 = 32;

const EK_EXIT: u8 = 1;
const EK_COMMAND: u8 = 2;
const EK_ROOMSPELL: u8 = 3;
const EK_NPC: u8 = 4;
const EK_ITEMSPELL: u8 = 5;
const NO_DIR: u8 = 255;

const NUM_POIS: usize = 6;

const RCAT_ROOM: u8 = 1;
const RCAT_LAIR: u8 = 2;
const RCAT_RESOLVED: u8 = 3;
const RCAT_GROUP: u8 = 4;

const ROUTE_COIN_COST: i64 = 1_000_000;
const ROUTE_INF: i64 = 2_147_483_647;
const ROUTE_DETOUR_MAX: i32 = 3;
const ROUTE_COIN_STEP_COPPER: i32 = 100_000;

const DIRS: [&str; 10] = ["N", "S", "E", "W", "NE", "NW", "SE", "SW", "U", "D"];

// ---- the tables the computation reads --------------------------------------------------------

/// The `Rooms` columns modReachability loads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReachRoom {
    pub map: i32,
    pub room: i32,
    pub name: String,
    pub npc: i32,
    pub cmd: i32,
    pub spell: i32,
    pub placed: String,
    /// The N, S, E, W, NE, NW, SE, SW, U and D columns, raw.
    pub exits: [String; 10],
}

/// The `TBInfo` columns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReachTb {
    pub number: i32,
    pub link_to: i32,
    pub action: String,
    pub called_from: String,
}

/// The `Spells` columns (`Abil-n` / `AbilVal-n` for n = 0 to 9; MME reads up to 20 where the table has them).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReachSpell {
    pub number: i32,
    pub req_level: i32,
    pub dur: i32,
    pub casted_by: String,
    pub abilities: Vec<(i32, i32)>,
}

/// The `Monsters` columns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReachMonster {
    pub number: i32,
    pub name: String,
    pub greet: i32,
    pub in_game: bool,
    pub summoned_by: String,
}

/// The `Items` columns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReachItem {
    pub number: i32,
    pub obtained_from: String,
}

/// Every table row the computation reads, in table order (MME reads `Monsters` ordered by `Number`).
#[derive(Debug, Clone, Default)]
pub struct ReachInput {
    pub rooms: Vec<ReachRoom>,
    pub tbinfo: Vec<ReachTb>,
    pub spells: Vec<ReachSpell>,
    pub monsters: Vec<ReachMonster>,
    pub items: Vec<ReachItem>,
}

impl ReachInput {
    /// The rows of a loaded database.
    #[must_use]
    pub fn from_world(world: &World) -> Self {
        let mut monsters: Vec<ReachMonster> = world
            .monsters
            .iter()
            .map(|m| ReachMonster {
                number: m.number,
                name: m.name.clone().unwrap_or_default(),
                greet: m.greet_txt,
                in_game: m.in_game,
                summoned_by: m.summoned_by.clone().unwrap_or_default(),
            })
            .collect();
        monsters.sort_by_key(|m| m.number);
        Self {
            rooms: world
                .rooms
                .iter()
                .map(|r| ReachRoom {
                    map: r.map_number,
                    room: r.room_number,
                    name: r.name.clone().unwrap_or_default(),
                    npc: r.npc,
                    cmd: r.cmd,
                    spell: r.spell,
                    placed: r.placed.clone().unwrap_or_default(),
                    exits: [
                        r.n.clone(),
                        r.s.clone(),
                        r.e.clone(),
                        r.w.clone(),
                        r.ne.clone(),
                        r.nw.clone(),
                        r.se.clone(),
                        r.sw.clone(),
                        r.u.clone(),
                        r.d.clone(),
                    ],
                })
                .collect(),
            tbinfo: world
                .tbinfo
                .iter()
                .map(|t| ReachTb {
                    number: t.number,
                    link_to: t.link_to,
                    action: t.action.clone().unwrap_or_default(),
                    called_from: t.called_from.clone(),
                })
                .collect(),
            spells: world
                .spells
                .iter()
                .map(|s| ReachSpell {
                    number: s.number,
                    req_level: s.req_level,
                    dur: s.dur,
                    casted_by: s.casted_by.clone().unwrap_or_default(),
                    abilities: s.abilities.iter().map(|a| (a.ability, a.value)).collect(),
                })
                .collect(),
            monsters,
            items: world
                .items
                .iter()
                .map(|i| ReachItem {
                    number: i.number,
                    obtained_from: i.obtained_from.clone().unwrap_or_default(),
                })
                .collect(),
        }
    }
}

// ---- VB6 text helpers ------------------------------------------------------------------------

/// `TrimWS`: only space, tab, CR and LF.
fn trim_ws(s: &str) -> &str {
    s.trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
}

/// `IsDigits`: 1 to 9 ASCII digits.
fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.len() <= 9 && s.bytes().all(|b| b.is_ascii_digit())
}

/// `IsInt`: an optional sign, then `IsDigits`.
fn is_int(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() > 1 && (b[0] == b'-' || b[0] == b'+') {
        return is_digits(&s[1..]);
    }
    is_digits(s)
}

/// `CLng` of a string `IsInt` accepted.
fn to_long(s: &str) -> i32 {
    s.parse::<i32>().unwrap_or(0)
}

/// `Split(s, delim)`: a zero-length string gives no elements.
fn vb_split<'a>(s: &'a str, delim: &str) -> Vec<&'a str> {
    if s.is_empty() {
        Vec::new()
    } else {
        s.split(delim).collect()
    }
}

/// `SplitWords`: tabs to spaces, trimmed, runs of spaces collapsed.
fn split_words(s: &str) -> Vec<String> {
    let mut t = trim_ws(&s.replace('\t', " ")).to_string();
    while t.contains("  ") {
        t = t.replace("  ", " ");
    }
    vb_split(&t, " ").into_iter().map(str::to_string).collect()
}

/// 1-based `InStr(start, hay, needle)` over bytes; 0 when absent.
fn instr(start: usize, hay: &str, needle: &str) -> usize {
    let h = hay.as_bytes();
    let n = needle.as_bytes();
    if start < 1 || start > h.len() || h.len() < n.len() {
        return 0;
    }
    (start - 1..=h.len() - n.len())
        .find(|&i| &h[i..i + n.len()] == n)
        .map_or(0, |i| i + 1)
}

/// `NumbersIn`: every run of 1 to 9 digits.
fn numbers_in(s: &str) -> Vec<String> {
    let mut all = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            cur.push(c);
        } else if !cur.is_empty() {
            if cur.len() <= 9 {
                all.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
        }
    }
    if !cur.is_empty() && cur.len() <= 9 {
        all.push(cur);
    }
    all
}

/// `NextMapRoom`: the next `<prefix><map>/<room>` at or after `pos` (1-based); an entry followed by `+`
/// was cut off at 250 characters and is skipped.
fn next_map_room(s: &str, prefix: &str, pos: &mut usize) -> Option<(i32, i32)> {
    let b = s.as_bytes();
    loop {
        let x = instr(*pos, s, prefix);
        if x == 0 {
            return None;
        }
        *pos = x + prefix.len();
        let mut i = *pos;
        let mut sm = String::new();
        while i <= b.len() && b[i - 1].is_ascii_digit() {
            sm.push(b[i - 1] as char);
            i += 1;
        }
        if !sm.is_empty() && i <= b.len() && b[i - 1] == b'/' {
            i += 1;
            let mut sr = String::new();
            while i <= b.len() && b[i - 1].is_ascii_digit() {
                sr.push(b[i - 1] as char);
                i += 1;
            }
            let next_plus = i <= b.len() && b[i - 1] == b'+';
            if !sr.is_empty() && sm.len() <= 9 && sr.len() <= 9 && !next_plus {
                *pos = i;
                return Some((to_long(&sm), to_long(&sr)));
            }
        }
    }
}

/// `Format$(n, "#,##0")`.
fn thousands(n: i32) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// `AddCapped`.
fn add_capped(a: i32, b: i32) -> i32 {
    if a > 999_999_999 - b {
        999_999_999
    } else {
        a + b
    }
}

/// A `Scripting.Dictionary` of Long keys: insertion order, and setting an existing key keeps its place.
#[derive(Debug, Clone, Default)]
struct OrderedMap {
    items: Vec<(usize, u8)>,
    pos: HashMap<usize, usize>,
}

impl OrderedMap {
    fn get(&self, k: usize) -> Option<u8> {
        self.pos.get(&k).map(|&i| self.items[i].1)
    }
    fn set(&mut self, k: usize, v: u8) {
        if let Some(&i) = self.pos.get(&k) {
            self.items[i].1 = v;
        } else {
            self.pos.insert(k, self.items.len());
            self.items.push((k, v));
        }
    }
}

/// An insertion-ordered set of room indexes.
#[derive(Debug, Clone, Default)]
struct OrderedSet {
    items: Vec<usize>,
    seen: HashSet<usize>,
}

impl OrderedSet {
    fn add(&mut self, k: usize) {
        if self.seen.insert(k) {
            self.items.push(k);
        }
    }
    fn contains(&self, k: usize) -> bool {
        self.seen.contains(&k)
    }
}

// ---- textblock ops ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    None,
    Teleport,
    Text,
    Random,
    Cast,
    MinLevel,
    MaxLevel,
    RoomItem,
    Other,
}

/// `OpKind`: one textblock op's kind, with its numbers.
fn op_kind(op: &str) -> (Op, i32, i32) {
    let w = split_words(op);
    if w.is_empty() {
        return (Op::None, 0, 0);
    }
    let n = w.len() - 1;
    let k = w[0].to_ascii_lowercase();
    if k.is_empty() {
        return (Op::None, 0, 0);
    }
    let mut v1 = 0;
    let mut v2 = 0;
    if n == 0 && is_digits(&k) {
        return (Op::Text, to_long(&k), 0);
    }
    if n >= 1 {
        if is_int(&w[1]) {
            v1 = to_long(&w[1]);
            match k.as_str() {
                "teleport" => {
                    if n >= 2 && is_int(&w[2]) {
                        v2 = to_long(&w[2]);
                    }
                    return (Op::Teleport, v1, v2);
                }
                "text" => return (Op::Text, v1, v2),
                "random" => return (Op::Random, v1, v2),
                "cast" => return (Op::Cast, v1, v2),
                "minlevel" => return (Op::MinLevel, v1, v2),
                "maxlevel" => return (Op::MaxLevel, v1, v2),
                "roomitem" => return (Op::RoomItem, v1, v2),
                "price" | "checkitem" | "takeitem" | "class" | "race" | "checkability"
                | "testability" => return (Op::Other, v1, v2),
                _ => {}
            }
            v1 = 0;
        }
        if k == "testskill" && n >= 2 {
            return (Op::Other, v1, v2);
        }
    }
    match k.as_str() {
        "goodaligned" | "evilaligned" | "nomonsters" | "needmonster" | "failroomitem"
        | "failability" => (Op::Other, v1, v2),
        _ => (Op::None, v1, v2),
    }
}

/// `SplitLine`: a command (unless the line is a called block that starts with an op) and its ops.
fn split_line(line: &str, called: bool) -> (String, bool, Vec<String>) {
    let parts = vb_split(trim_ws(line), ":");
    let mut ops = Vec::new();
    let mut cmd = trim_ws(parts.first().copied().unwrap_or("")).to_string();
    let mut has_cmd = true;
    if called && !cmd.is_empty() {
        let (k, _, _) = op_kind(&cmd);
        if k != Op::None && !is_digits(&cmd) {
            ops.push(cmd.clone());
            cmd.clear();
            has_cmd = false;
        }
    }
    for p in parts.iter().skip(1) {
        let s = trim_ws(p);
        if !s.is_empty() {
            ops.push(s.to_string());
        }
    }
    (cmd, has_cmd, ops)
}

/// `IsCommandLine`.
fn is_command_line(cmd: &str) -> bool {
    if is_digits(cmd) {
        return false;
    }
    vb_split(&cmd.replace('*', ""), "|")
        .iter()
        .any(|n| !trim_ws(n).is_empty())
}

/// `CmdText`: the first typed form of a command.
fn cmd_text(cmd: &str) -> String {
    for n in vb_split(&cmd.replace('*', ""), "|") {
        if !trim_ws(n).is_empty() {
            return trim_ws(n).to_string();
        }
    }
    trim_ws(cmd).to_string()
}

// ---- the build -------------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct TeleOut {
    room: i32,
    map: i32,
    min_lvl: i32,
    max_lvl: i32,
    is_random: bool,
    room_items: String,
    line_no: i32,
    cmd: String,
    price: i32,
    wait: i32,
}

#[derive(Debug, Clone, Default)]
struct Sp {
    num: i32,
    ab: [i32; MAX_ABILS],
    val: [i32; MAX_ABILS],
    req_level: i32,
    casted_by: String,
    dur: i32,
    spell_only: bool,
}

#[derive(Debug, Clone, Default)]
struct Mon {
    num: i32,
    name: String,
    summoned_by: String,
    greet: i32,
    status: u8,
    rooms: OrderedMap,
    base: OrderedMap,
    base_raw: i32,
}

#[derive(Default)]
struct Edges {
    from: Vec<i32>,
    to: Vec<i32>,
    min: Vec<i32>,
    max: Vec<i32>,
    group: Vec<i32>,
    kind: Vec<u8>,
    dir: Vec<u8>,
    label: Vec<String>,
    coin: Vec<i32>,
    wait: Vec<i32>,
}

impl Edges {
    fn new() -> Self {
        // index 0 is unused: edges are 1-based
        let mut e = Self::default();
        e.from.push(0);
        e.to.push(0);
        e.min.push(0);
        e.max.push(0);
        e.group.push(0);
        e.kind.push(0);
        e.dir.push(0);
        e.label.push(String::new());
        e.coin.push(0);
        e.wait.push(0);
        e
    }
    fn count(&self) -> usize {
        self.from.len() - 1
    }
}

struct Build {
    n_rooms: usize,
    r_map: Vec<i32>,
    r_room: Vec<i32>,
    r_cmd: Vec<i32>,
    r_spell: Vec<i32>,
    r_npc: Vec<i32>,
    r_exit: Vec<[String; 10]>,
    r_placed: Vec<String>,
    room_idx: HashMap<(i32, i32), usize>,
    tb_idx: HashMap<i32, usize>,
    tb_action: Vec<String>,
    tb_link: Vec<i32>,
    tb_called: Vec<String>,
    sp_idx: HashMap<i32, usize>,
    sp: Vec<Sp>,
    item_rooms: HashMap<i32, Vec<usize>>,
    mons: Vec<Mon>,
    mon_idx: HashMap<i32, usize>,
    edges: Edges,
    n_groups: i32,
    dropped_random: i32,
    out: Vec<TeleOut>,
    pool: Vec<TeleOut>,
    chain_cache: HashMap<String, (usize, usize)>,
    poi_idx: [usize; NUM_POIS],
}

impl Build {
    fn room_index(&self, map: i32, room: i32) -> usize {
        self.room_idx.get(&(map, room)).copied().unwrap_or(0)
    }

    fn tb_index(&self, n: i32) -> usize {
        self.tb_idx.get(&n).copied().unwrap_or(0)
    }

    fn spell_index(&self, n: i32) -> usize {
        self.sp_idx.get(&n).copied().unwrap_or(0)
    }

    fn load(input: &ReachInput) -> Self {
        let mut b = Self {
            n_rooms: 0,
            r_map: vec![0],
            r_room: vec![0],
            r_cmd: vec![0],
            r_spell: vec![0],
            r_npc: vec![0],
            r_exit: vec![Default::default()],
            r_placed: vec![String::new()],
            room_idx: HashMap::new(),
            tb_idx: HashMap::new(),
            tb_action: vec![String::new()],
            tb_link: vec![0],
            tb_called: vec![String::new()],
            sp_idx: HashMap::new(),
            sp: vec![Sp::default()],
            item_rooms: HashMap::new(),
            mons: vec![Mon::default()],
            mon_idx: HashMap::new(),
            edges: Edges::new(),
            n_groups: 0,
            dropped_random: 0,
            out: Vec::new(),
            pool: Vec::new(),
            chain_cache: HashMap::new(),
            poi_idx: [0; NUM_POIS],
        };
        // LoadTextblocks
        for t in &input.tbinfo {
            b.tb_action.push(t.action.replace('\r', ""));
            b.tb_link.push(t.link_to);
            b.tb_called.push(t.called_from.clone());
            let n = b.tb_action.len() - 1;
            b.tb_idx.entry(t.number).or_insert(n);
        }
        // LoadSpells
        for s in &input.spells {
            let mut sp = Sp {
                num: s.number,
                req_level: s.req_level,
                casted_by: s.casted_by.clone(),
                dur: s.dur.max(0),
                ..Sp::default()
            };
            sp.spell_only = spell_only_casters(&sp.casted_by);
            for (j, (a, v)) in s.abilities.iter().take(MAX_ABILS).enumerate() {
                sp.ab[j] = *a;
                sp.val[j] = *v;
            }
            b.sp.push(sp);
            let n = b.sp.len() - 1;
            b.sp_idx.entry(s.number).or_insert(n);
        }
        // LoadRooms
        for r in &input.rooms {
            b.n_rooms += 1;
            b.r_map.push(r.map);
            b.r_room.push(r.room);
            b.r_cmd.push(r.cmd);
            b.r_spell.push(r.spell);
            b.r_npc.push(r.npc);
            b.r_placed.push(r.placed.clone());
            b.r_exit.push(r.exits.clone());
            let n = b.n_rooms;
            b.room_idx.entry((r.map, r.room)).or_insert(n);
        }
        // ResolvePOIs
        let pois = [
            (START_MAP, START_ROOM),
            (17, 2435),
            (6, 1334),
            (16, 426),
            (14, 1),
            (14, 10),
        ];
        for (i, (m, r)) in pois.iter().enumerate() {
            b.poi_idx[i] = b.room_index(*m, *r);
        }
        // LoadItemRooms
        for i in 1..=b.n_rooms {
            if !b.r_placed[i].is_empty() {
                for n in numbers_in(&b.r_placed[i].clone()) {
                    b.add_item_room(to_long(&n), i);
                }
            }
        }
        for it in &input.items {
            let mut pos = 1;
            while let Some((m, r)) = next_map_room(&it.obtained_from, "Room ", &mut pos) {
                let ri = b.room_index(m, r);
                b.add_item_room(it.number, ri);
            }
        }
        // LoadMonsters (ordered by Number)
        let prefixes = [
            ("Group: ", RCAT_GROUP),
            ("Group(lair): ", RCAT_LAIR),
            ("Room ", RCAT_ROOM),
        ];
        for m in &input.monsters {
            let mut mon = Mon {
                num: m.number,
                name: m.name.clone(),
                summoned_by: m.summoned_by.clone(),
                greet: m.greet,
                status: if m.in_game {
                    MON_NO_LOCATION
                } else {
                    MON_NOT_IN_GAME
                },
                ..Mon::default()
            };
            for (prefix, cat) in prefixes {
                let mut pos = 1;
                while let Some((mm, rr)) = next_map_room(&m.summoned_by, prefix, &mut pos) {
                    mon.base_raw += 1;
                    let idx = b.room_index(mm, rr);
                    if idx > 0 {
                        match mon.base.get(idx) {
                            None => mon.base.set(idx, cat),
                            Some(cur) if cur > cat => mon.base.set(idx, cat),
                            _ => {}
                        }
                    }
                }
            }
            b.mons.push(mon);
            let n = b.mons.len() - 1;
            b.mon_idx.entry(m.number).or_insert(n);
        }
        b
    }

    fn n_mons(&self) -> usize {
        self.mons.len() - 1
    }

    fn add_item_room(&mut self, item: i32, room_idx: usize) {
        if room_idx == 0 {
            return;
        }
        self.item_rooms.entry(item).or_default().push(room_idx);
    }

    // -- textblock chains --

    fn add_out(
        &mut self,
        room: i32,
        map: i32,
        min: i32,
        max: i32,
        items: &str,
        price: i32,
        wait: i32,
        line: i32,
    ) {
        self.out.push(TeleOut {
            room,
            map,
            min_lvl: min,
            max_lvl: max,
            is_random: false,
            room_items: items.to_string(),
            line_no: line,
            cmd: String::new(),
            price,
            wait,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn from_ops(
        &mut self,
        ops: &[String],
        mut min: i32,
        mut max: i32,
        mut items: String,
        mut price: i32,
        wait: i32,
        depth: i32,
        seen: &str,
        cmd: &str,
        has_cmd: bool,
        line: i32,
    ) {
        let first = self.out.len();
        for op in ops {
            let (k, v1, v2) = op_kind(op);
            match k {
                Op::MinLevel => {
                    if v1 > min {
                        min = v1;
                    }
                }
                Op::MaxLevel => {
                    if v1 > 0 && v1 < max {
                        max = v1;
                    }
                }
                Op::RoomItem => items = format!("{items}{v1},"),
                Op::Teleport => {
                    if v1 != 0 {
                        self.add_out(v1, v2, min, max, &items, price, wait, line);
                    }
                }
                Op::Text => self.from_tb(
                    v1,
                    min,
                    max,
                    &items,
                    price,
                    wait,
                    depth + 1,
                    seen,
                    cmd,
                    has_cmd,
                    line,
                ),
                Op::Cast => {
                    self.from_spell(v1, min, max, &items, price, wait, depth + 1, seen, line)
                }
                Op::Other => {
                    if v1 > 0 {
                        let w = split_words(op);
                        if w[0].eq_ignore_ascii_case("price") {
                            for o in &mut self.out[first..] {
                                o.price = add_capped(o.price, v1);
                            }
                            price = add_capped(price, v1);
                        }
                    }
                }
                Op::Random => {
                    let start = self.out.len();
                    self.from_tb(
                        v1,
                        min,
                        max,
                        &items,
                        price,
                        wait,
                        depth + 1,
                        seen,
                        "",
                        false,
                        line,
                    );
                    let mut distinct = 0;
                    for a in start..self.out.len() {
                        let dup = (start..a).any(|b| {
                            self.out[b].room == self.out[a].room
                                && self.out[b].map == self.out[a].map
                        });
                        if !dup {
                            distinct += 1;
                        }
                    }
                    if distinct > MAX_RANDOM_DESTS {
                        self.out.truncate(start);
                        self.dropped_random += 1;
                    } else if distinct > 1 {
                        for o in &mut self.out[start..] {
                            o.is_random = true;
                        }
                    }
                }
                Op::None => {}
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn from_tb(
        &mut self,
        n: i32,
        min: i32,
        max: i32,
        items: &str,
        price: i32,
        wait: i32,
        depth: i32,
        seen: &str,
        cmd: &str,
        has_cmd: bool,
        line: i32,
    ) {
        if depth > MAX_CHAIN_DEPTH || seen.contains(&format!(",t{n},")) {
            return;
        }
        let seen = format!("{seen}t{n},");
        let idx = self.tb_index(n);
        if idx == 0 {
            return;
        }
        let action = self.tb_action[idx].clone();
        let lines = vb_split(&action, "\n");
        let mut filter = false;
        if has_cmd {
            // A 'text N' jump runs the called block for the same command when it has one.
            for l in &lines {
                if !trim_ws(l).is_empty() {
                    let (lc, lh, _) = split_line(l, true);
                    if lh && lc == cmd {
                        filter = true;
                        break;
                    }
                }
            }
        }
        for l in &lines {
            if !trim_ws(l).is_empty() {
                let (lc, lh, ops) = split_line(l, true);
                if !filter || (lh && lc == cmd) {
                    self.from_ops(
                        &ops,
                        min,
                        max,
                        items.to_string(),
                        price,
                        wait,
                        depth,
                        &seen,
                        cmd,
                        has_cmd,
                        line,
                    );
                }
            }
        }
        let link = self.tb_link[idx];
        if link != 0 {
            self.from_tb(
                link,
                min,
                max,
                items,
                price,
                wait,
                depth + 1,
                &seen,
                cmd,
                has_cmd,
                line,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn from_spell(
        &mut self,
        s: i32,
        min: i32,
        max: i32,
        items: &str,
        price: i32,
        wait: i32,
        depth: i32,
        seen: &str,
        line: i32,
    ) {
        if depth > MAX_CHAIN_DEPTH || seen.contains(&format!(",s{s},")) {
            return;
        }
        let seen = format!("{seen}s{s},");
        let idx = self.spell_index(s);
        if idx == 0 {
            return;
        }
        let (ab, val, dur) = (self.sp[idx].ab, self.sp[idx].val, self.sp[idx].dur);
        let (mut room, mut map) = (0, 0);
        let (mut got_room, mut got_map) = (false, false);
        for j in 0..MAX_ABILS {
            if ab[j] == ABIL_TELEPORT_ROOM && !got_room {
                room = val[j];
                got_room = true;
            }
            if ab[j] == ABIL_TELEPORT_MAP && !got_map {
                map = val[j];
                got_map = true;
            }
        }
        if room != 0 {
            self.add_out(room, map, min, max, items, price, wait, line);
        }
        for j in 0..MAX_ABILS {
            if val[j] != 0 {
                if ab[j] == ABIL_ENDCAST {
                    self.from_spell(
                        val[j],
                        min,
                        max,
                        items,
                        price,
                        add_capped(wait, dur),
                        depth + 1,
                        &seen,
                        line,
                    );
                } else if ab[j] == ABIL_TEXTBLOCK {
                    self.from_tb(
                        val[j],
                        min,
                        max,
                        items,
                        price,
                        wait,
                        depth + 1,
                        &seen,
                        "",
                        false,
                        line,
                    );
                }
            }
        }
    }

    /// `ChainOutputs`: the outputs for a root, computed once; (start, count) into the pool.
    fn chain_outputs(&mut self, kind: char, n: i32) -> (usize, usize) {
        let key = format!("{kind}{n}");
        if let Some(&r) = self.chain_cache.get(&key) {
            return r;
        }
        self.out.clear();
        match kind {
            'S' => self.from_spell(n, 0, NO_MAX, ",", 0, 0, 0, ",", 0),
            _ => {
                let idx = self.tb_index(n);
                if idx > 0 {
                    let action = self.tb_action[idx].clone();
                    let mut line_no = 0;
                    for l in vb_split(&action, "\n") {
                        if trim_ws(l).is_empty() {
                            continue;
                        }
                        line_no += 1;
                        let (cmd, _, ops) = split_line(l, false);
                        if is_command_line(&cmd) {
                            let first = self.out.len();
                            if kind == 'C' {
                                self.from_ops(
                                    &ops,
                                    0,
                                    NO_MAX,
                                    ",".into(),
                                    0,
                                    0,
                                    0,
                                    ",",
                                    &cmd,
                                    true,
                                    line_no,
                                );
                            } else {
                                self.from_ops(
                                    &ops,
                                    0,
                                    NO_MAX,
                                    ",".into(),
                                    0,
                                    0,
                                    0,
                                    &format!(",t{n},"),
                                    "",
                                    false,
                                    line_no,
                                );
                            }
                            for o in &mut self.out[first..] {
                                o.cmd.clone_from(&cmd);
                            }
                        }
                    }
                }
            }
        }
        let start = self.pool.len();
        let count = self.out.len();
        self.pool.append(&mut self.out);
        self.chain_cache.insert(key, (start, count));
        (start, count)
    }

    // -- edges --

    #[allow(clippy::too_many_arguments)]
    fn add_edge(
        &mut self,
        from: usize,
        to: usize,
        min: i32,
        max: i32,
        group: i32,
        kind: u8,
        dir: u8,
        label: String,
        coin: i32,
        wait: i32,
    ) {
        if from == 0 || to == 0 {
            return;
        }
        let e = &mut self.edges;
        e.from.push(from as i32);
        e.to.push(to as i32);
        e.min.push(min);
        e.max.push(max);
        e.group.push(group);
        e.kind.push(kind);
        e.dir.push(dir);
        e.label.push(label);
        e.coin.push(coin);
        e.wait.push(wait);
    }

    fn add_chain_edges(
        &mut self,
        from: usize,
        start: usize,
        count: usize,
        skip_self: bool,
        kind: u8,
        prefix: &str,
    ) {
        let mut last_line = -1;
        for i in start..start + count {
            let t = self.pool[i].clone();
            let mut map = t.map;
            if map == 0 {
                map = self.r_map[from];
            }
            let to = self.room_index(map, t.room);
            if to > 0 && !(skip_self && to == from) {
                let mut group = 0;
                if t.is_random {
                    if t.line_no != last_line {
                        self.n_groups += 1;
                        last_line = t.line_no;
                    }
                    group = self.n_groups;
                }
                let label = match kind {
                    EK_COMMAND => t.cmd.clone(),
                    EK_NPC => format!("{prefix}|{}", t.cmd),
                    _ => prefix.to_string(),
                };
                self.add_edge(
                    from, to, t.min_lvl, t.max_lvl, group, kind, NO_DIR, label, t.price, t.wait,
                );
            }
        }
    }

    fn build_edges(&mut self) {
        for i in 1..=self.n_rooms {
            for j in 0..10 {
                if let Some((map, room, rest)) = parse_exit(&self.r_exit[i][j]) {
                    let (min, max) = exit_level(&rest);
                    let to = self.room_index(map, room);
                    self.add_edge(
                        i,
                        to,
                        min,
                        max,
                        0,
                        EK_EXIT,
                        j as u8,
                        exit_restriction(&rest),
                        exit_toll(&rest),
                        0,
                    );
                }
            }
            if self.r_spell[i] != 0 {
                let (s, c) = self.chain_outputs('S', self.r_spell[i]);
                self.add_chain_edges(i, s, c, true, EK_ROOMSPELL, &self.r_spell[i].to_string());
            }
            if self.r_cmd[i] != 0 {
                let (s, c) = self.chain_outputs('C', self.r_cmd[i]);
                self.add_chain_edges(i, s, c, false, EK_COMMAND, "");
            }
        }

        // NPC conversations: 'ask <npc> <keyword>' from every room the NPC is in
        let mut npc_rooms: HashMap<i32, Vec<usize>> = HashMap::new();
        for i in 1..=self.n_rooms {
            if self.r_npc[i] != 0 {
                npc_rooms.entry(self.r_npc[i]).or_default().push(i);
            }
        }
        for m in 1..=self.n_mons() {
            if self.mons[m].greet == 0 {
                continue;
            }
            let mut where_ = OrderedSet::default();
            for &(k, _) in &self.mons[m].base.items {
                where_.add(k);
            }
            if let Some(v) = npc_rooms.get(&self.mons[m].num) {
                for &r in v {
                    where_.add(r);
                }
            }
            if !where_.items.is_empty() {
                let (s, c) = self.chain_outputs('G', self.mons[m].greet);
                if c > 0 {
                    let name = self.mons[m].name.clone();
                    for &r in &where_.items {
                        self.add_chain_edges(r, s, c, false, EK_NPC, &name);
                    }
                }
            }
        }

        // Teleport spells that need a room item only work where that item is.
        for s in 1..self.sp.len() {
            let (mut start, mut count) = (0, 0);
            if !self.sp[s].spell_only {
                (start, count) = self.chain_outputs('S', self.sp[s].num);
            }
            for k in start..start + count {
                let t = self.pool[k].clone();
                if t.map != 0 {
                    let to = self.room_index(t.map, t.room);
                    if to > 0 {
                        if t.room_items != "," {
                            let min = t.min_lvl.max(self.sp[s].req_level);
                            let max = t.max_lvl;
                            let need = numbers_in(&t.room_items);
                            let mut hold: Option<OrderedSet> = None;
                            let mut item_list = String::new();
                            for (j, n) in need.iter().enumerate() {
                                if j > 0 {
                                    item_list.push(',');
                                }
                                item_list.push_str(n);
                                let mut here = OrderedSet::default();
                                if let Some(c) = self.item_rooms.get(&to_long(n)) {
                                    for &r in c {
                                        match &hold {
                                            None => here.add(r),
                                            Some(h) if h.contains(r) => here.add(r),
                                            _ => {}
                                        }
                                    }
                                }
                                hold = Some(here);
                            }
                            if let Some(h) = hold {
                                for r in h.items {
                                    let label = format!("{}|{item_list}", self.sp[s].num);
                                    self.add_edge(
                                        r,
                                        to,
                                        min,
                                        max,
                                        0,
                                        EK_ITEMSPELL,
                                        NO_DIR,
                                        label,
                                        t.price,
                                        t.wait,
                                    );
                                }
                            }
                        }
                        break; // first teleport with an explicit map
                    }
                }
            }
        }
    }

    // -- monster rooms --

    fn parse_refs(&self, s: &str) -> Vec<String> {
        let mut c = Vec::new();
        let mut pos = 1;
        while let Some((m, r)) = next_map_room(s, "Room ", &mut pos) {
            let idx = self.room_index(m, r);
            if idx > 0 {
                c.push(format!("R:{idx}"));
            }
        }
        let pre = [
            ("Monster #", "M:"),
            ("Textblock #", "T:"),
            ("Textblock(rndm) #", "T:"),
            ("Spell #", "S:"),
            ("Item #", "I:"),
        ];
        for (p, tag) in pre {
            c.extend(refs_after(s, p, tag));
        }
        c
    }

    fn summon_refs(&self, m: usize) -> Vec<String> {
        let s = &self.mons[m].summoned_by;
        let mut c = refs_after(s, "Textblock #", "T:");
        c.extend(refs_after(s, "Spell #", "S:"));
        c
    }

    fn resolve(&self, r: &str, depth: i32, seen: &str, out: &mut OrderedSet) {
        if depth > MAX_RESOLVE_DEPTH || seen.contains(&format!("|{r}|")) {
            return;
        }
        let seen = format!("{seen}{r}|");
        let kind = &r[..1];
        let n = to_long(&r[2..]);
        let refs: Vec<String> = match kind {
            "R" => {
                out.add(n as usize);
                return;
            }
            "M" => match self.mon_idx.get(&n) {
                Some(&i) => {
                    if self.mons[i].base_raw > 0 {
                        for &(k, _) in &self.mons[i].base.items {
                            out.add(k);
                        }
                        return;
                    }
                    self.summon_refs(i)
                }
                None => return,
            },
            "T" => {
                let i = self.tb_index(n);
                if i == 0 {
                    return;
                }
                self.parse_refs(&self.tb_called[i])
            }
            "S" => {
                let i = self.spell_index(n);
                if i == 0 {
                    return;
                }
                self.parse_refs(&self.sp[i].casted_by)
            }
            _ => return,
        };
        for v in refs {
            self.resolve(&v, depth + 1, &seen, out);
        }
    }

    fn resolve_monster_rooms(&mut self) {
        for m in 1..=self.n_mons() {
            let mut rooms = OrderedMap::default();
            for &(k, c) in &self.mons[m].base.items {
                rooms.set(k, c);
            }
            let mut found = OrderedSet::default();
            for v in self.summon_refs(m) {
                let mut dr = OrderedSet::default();
                self.resolve(&v, 0, "|", &mut dr);
                let mut tb_idx = 0;
                if let Some(rest) = v.strip_prefix("T:") {
                    tb_idx = self.tb_index(to_long(rest));
                }
                if tb_idx > 0 {
                    if let Some((room, map)) =
                        summon_after_teleport(&self.tb_action[tb_idx], self.mons[m].num)
                    {
                        if dr.items.is_empty() {
                            if map != 0 {
                                let to = self.room_index(map, room);
                                if to > 0 {
                                    found.add(to);
                                }
                            }
                        } else {
                            for &k in &dr.items {
                                let mut caller_map = map;
                                if caller_map == 0 {
                                    caller_map = self.r_map[k];
                                }
                                let to = self.room_index(caller_map, room);
                                if to > 0 {
                                    found.add(to);
                                }
                            }
                        }
                        continue;
                    }
                }
                for &k in &dr.items {
                    found.add(k);
                }
            }
            for &k in &found.items {
                match rooms.get(k) {
                    None => rooms.set(k, RCAT_RESOLVED),
                    Some(c) if c > RCAT_RESOLVED => rooms.set(k, RCAT_RESOLVED),
                    _ => {}
                }
            }
            let mon = &mut self.mons[m];
            mon.rooms = rooms;
            if mon.status != MON_NOT_IN_GAME {
                mon.status = if !mon.rooms.items.is_empty() || mon.base_raw > 0 {
                    MON_LOCATED
                } else {
                    MON_NO_LOCATION
                };
            }
        }
    }
}

/// Every `<prefix><digits>` in `s` as `<tag><digits>` (a number cut off by `+` is skipped).
fn refs_after(s: &str, prefix: &str, tag: &str) -> Vec<String> {
    let b = s.as_bytes();
    let mut c = Vec::new();
    let mut x = instr(1, s, prefix);
    while x > 0 {
        let mut i = x + prefix.len();
        let mut num = String::new();
        while i <= b.len() && b[i - 1].is_ascii_digit() {
            num.push(b[i - 1] as char);
            i += 1;
        }
        let plus = i <= b.len() && b[i - 1] == b'+';
        if !num.is_empty() && num.len() <= 9 && !plus {
            c.push(format!("{tag}{num}"));
        }
        x = instr(i, s, prefix);
    }
    c
}

/// `SpellOnlyCasters`: nothing but other spells cast it.
fn spell_only_casters(s: &str) -> bool {
    let s = trim_ws(&s.replace('\0', "")).to_string();
    if s.is_empty() || s.ends_with('+') {
        return false;
    }
    let mut n = 0;
    for ent in vb_split(&s, ",") {
        let e = trim_ws(ent);
        if !e.is_empty() {
            if !e.starts_with("Spell #") {
                return false;
            }
            n += 1;
        }
    }
    n > 0
}

/// `SummonAfterTeleport`: (room, map) of a line that teleports before 'summon <monster>'.
fn summon_after_teleport(action: &str, mon: i32) -> Option<(i32, i32)> {
    let (mut room, mut map) = (0, 0);
    for line in vb_split(action, "\n") {
        let mut tp = false;
        for op in vb_split(line, ":") {
            let w = split_words(op);
            if w.len() >= 2 {
                if w[0].eq_ignore_ascii_case("teleport") && is_digits(&w[1]) {
                    room = to_long(&w[1]);
                    map = 0;
                    tp = true;
                    if w.len() >= 3 && is_digits(&w[2]) {
                        map = to_long(&w[2]);
                    }
                } else if w[0].eq_ignore_ascii_case("summon") && w[1] == mon.to_string() && tp {
                    return Some((room, map));
                }
            }
        }
    }
    None
}

/// `ParseExit`: map, room and the rest of the text.
fn parse_exit(s: &str) -> Option<(i32, i32, String)> {
    let s = trim_ws(s);
    if s.is_empty() || s == "0" || s.starts_with("Action") {
        return None;
    }
    let b = s.as_bytes();
    let mut i = 0;
    let mut sm = String::new();
    while i < b.len() && b[i].is_ascii_digit() {
        sm.push(b[i] as char);
        i += 1;
    }
    while i < b.len() && b[i] == b' ' {
        i += 1;
    }
    if sm.is_empty() || i >= b.len() || b[i] != b'/' {
        return None;
    }
    i += 1;
    while i < b.len() && b[i] == b' ' {
        i += 1;
    }
    let mut sr = String::new();
    while i < b.len() && b[i].is_ascii_digit() {
        sr.push(b[i] as char);
        i += 1;
    }
    if sr.is_empty() || sm.len() > 9 || sr.len() > 9 {
        return None;
    }
    Some((to_long(&sm), to_long(&sr), trim_ws(&s[i..]).to_string()))
}

/// The "(...)" groups of an exit's restriction text, as (start, end) byte positions of the parentheses.
fn paren_groups(s: &str) -> Vec<(usize, usize)> {
    let b = s.as_bytes();
    let mut v = Vec::new();
    let mut from = 0;
    while let Some(x) = b[from..].iter().position(|&c| c == b'(').map(|p| p + from) {
        let Some(y) = b[x + 1..]
            .iter()
            .position(|&c| c == b')')
            .map(|p| p + x + 1)
        else {
            break;
        };
        v.push((x, y));
        from = y + 1;
    }
    v
}

/// `ExitToll`: the largest "(Toll: N)".
fn exit_toll(rest: &str) -> i32 {
    let mut toll = 0;
    for (x, y) in paren_groups(rest) {
        let body = trim_ws(&rest[x + 1..y]);
        if let Some(after) = body.strip_prefix("Toll: ") {
            let after = trim_ws(after);
            let num: String = after.chars().take_while(char::is_ascii_digit).collect();
            if !num.is_empty() && num.len() <= 9 && to_long(&num) > toll {
                toll = to_long(&num);
            }
        }
    }
    toll
}

/// `ExitLevel`: the "(Level: a to b)" range (b = 0 or b < a means no upper limit).
fn exit_level(rest: &str) -> (i32, i32) {
    let (mut min, mut max) = (0, NO_MAX);
    for (x, y) in paren_groups(rest) {
        let body = trim_ws(&rest[x + 1..y]);
        if let Some(after) = body.strip_prefix("Level: ") {
            let w = vb_split(after, " to ");
            if w.len() == 2 && is_digits(w[0]) && is_digits(w[1]) {
                let lo = to_long(w[0]);
                let mut hi = to_long(w[1]);
                if hi == 0 || hi < lo {
                    hi = NO_MAX;
                }
                if lo > min {
                    min = lo;
                }
                if hi < max {
                    max = hi;
                }
            }
        }
    }
    (min, max)
}

/// `ExitRestriction`: the non-level "(...)" groups, joined by spaces.
fn exit_restriction(rest: &str) -> String {
    if rest.is_empty() {
        return String::new();
    }
    if !rest.contains('(') {
        return rest.to_string();
    }
    let mut s = String::new();
    for (x, y) in paren_groups(rest) {
        let body = trim_ws(&rest[x + 1..y]);
        if !body.starts_with("Level: ") {
            if !s.is_empty() {
                s.push(' ');
            }
            s.push_str(&rest[x..=y]);
        }
    }
    s
}

// ---- the result ------------------------------------------------------------------------------

/// Where a route search starts: Bank of Godfrey and the other points of interest.
const POI_NAMES: [&str; NUM_POIS] = [
    "Bank of Godfrey",
    "Bank of Arlysia",
    "Bank of Khazarad",
    "The Lost City",
    "Bank of Albion",
    "Port Blackwater",
];

/// The computed reachability of one database (`ReachEnsureComputed`'s results).
#[derive(Debug)]
pub struct Reach {
    n_rooms: usize,
    r_map: Vec<i32>,
    r_room: Vec<i32>,
    room_idx: HashMap<(i32, i32), usize>,
    n_edges: usize,
    e_from: Vec<i32>,
    e_to: Vec<i32>,
    e_min: Vec<i32>,
    e_max: Vec<i32>,
    e_group: Vec<i32>,
    e_kind: Vec<u8>,
    e_dir: Vec<u8>,
    e_label: Vec<String>,
    e_coin: Vec<i32>,
    e_wait: Vec<i32>,
    n_groups: usize,
    n_dropped_random: i32,
    /// CSR over non-random edges, keyed on `eFrom`.
    adj_start: Vec<usize>,
    adj_edge: Vec<usize>,
    /// CSR over random-group edges, keyed on `eFrom`.
    gadj_start: Vec<usize>,
    gadj_edge: Vec<usize>,
    /// CSR over all edges, keyed on `eTo`.
    radj_start: Vec<usize>,
    radj_edge: Vec<usize>,
    /// Edges that belong to a random group, in edge order.
    group_edges: Vec<usize>,
    grp_ok: Vec<bool>,
    grp_src: Vec<usize>,
    n_start: usize,
    poi_idx: [usize; NUM_POIS],
    n_bp: usize,
    bp_level: Vec<i32>,
    mon_idx: HashMap<i32, usize>,
    m_num: Vec<i32>,
    m_status: Vec<u8>,
    m_rooms: Vec<Vec<(usize, u8)>>,
    m_hit: Vec<Vec<u8>>,
}

/// Counts for a status line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReachStats {
    pub rooms: usize,
    pub edges: usize,
    pub random_groups: usize,
    pub scatter_dropped: i32,
    pub level_bands: usize,
    pub monsters: usize,
    pub located: usize,
}

/// Everything `ReachMonsterInfo` hands back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonsterReach {
    pub status: ReachStatus,
    pub gate_map: i32,
    pub gate_room: i32,
    pub dest_map: i32,
    pub dest_room: i32,
    pub min_lvl: i32,
    pub max_lvl: i32,
    pub random_group: bool,
    /// The lowest level above the asked one at which the monster is reachable (0 = none).
    pub reachable_at: i32,
}

/// One step of a route: the room reached, its flags and the player-facing action leading into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteStep {
    pub map: i32,
    pub room: i32,
    pub flags: i32,
    pub label: String,
}

/// What `ReachRoute` / `ReachRoomRoute` hand back. `steps[0]` is the source room; `steps[i]` the room reached
/// after action `i`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Route {
    pub source_map: i32,
    pub source_room: i32,
    pub source_name: String,
    pub steps: Vec<RouteStep>,
    pub n_steps: i32,
    pub coin_steps: i32,
    pub alt_steps: i32,
    pub alt_coin_steps: i32,
    pub alt_source_name: String,
}

/// `Reach::compute` for a loaded database.
#[must_use]
pub fn compute_world(world: &World) -> Reach {
    compute(&ReachInput::from_world(world))
}

/// `ReachEnsureComputed`: build the graph from the tables and mark every monster's reachable level bands.
#[must_use]
pub fn compute(input: &ReachInput) -> Reach {
    let mut b = Build::load(input);
    b.build_edges();
    b.resolve_monster_rooms();
    Reach::finish(b)
}

impl Reach {
    /// `ComputeReachability`.
    fn finish(b: Build) -> Self {
        let n_rooms = b.n_rooms;
        let n_start = b.room_index(START_MAP, START_ROOM);
        let ne = b.edges.count();
        let Edges {
            from,
            to,
            min,
            max,
            group,
            kind,
            dir,
            label,
            coin,
            wait,
        } = b.edges;
        let csr = |keys: &dyn Fn(usize) -> usize, pick: &dyn Fn(usize) -> bool| {
            let mut start = vec![0usize; n_rooms + 2];
            for e in 1..=ne {
                if pick(e) {
                    start[keys(e)] += 1;
                }
            }
            let mut j = 1;
            let mut pos = vec![0usize; n_rooms + 2];
            for i in 1..=n_rooms {
                let tmp = start[i];
                start[i] = j;
                pos[i] = j;
                j += tmp;
            }
            start[n_rooms + 1] = j;
            let mut edge = vec![0usize; j.max(2)];
            for e in 1..=ne {
                if pick(e) {
                    let k = keys(e);
                    edge[pos[k]] = e;
                    pos[k] += 1;
                }
            }
            (start, edge)
        };
        let (adj_start, adj_edge) = csr(&|e| from[e] as usize, &|e| group[e] == 0);
        let (gadj_start, gadj_edge) = csr(&|e| from[e] as usize, &|e| group[e] > 0);
        let (radj_start, radj_edge) = csr(&|e| to[e] as usize, &|_| true);

        // random groups: usable only if every destination is in one strongly connected area
        let comp = scc_ids(n_rooms, &adj_start, &adj_edge, &to);
        let n_groups = b.n_groups as usize;
        let mut grp_ok = vec![true; n_groups + 1];
        let mut grp_comp = vec![-1i64; n_groups + 1];
        let mut grp_src = vec![0usize; n_groups + 1];
        let mut group_edges = Vec::new();
        for e in 1..=ne {
            let g = group[e] as usize;
            if g > 0 {
                group_edges.push(e);
                grp_src[g] = from[e] as usize;
                let c = i64::from(comp[to[e] as usize]);
                if grp_comp[g] == -1 {
                    grp_comp[g] = c;
                } else if grp_comp[g] != c {
                    grp_ok[g] = false;
                }
            }
        }

        // level breakpoints (where the set of usable edges can change)
        let mut bp: BTreeSet<i32> = BTreeSet::new();
        bp.insert(1);
        for e in 1..=ne {
            if min[e] >= 1 && min[e] < LEVEL_CAP {
                bp.insert(min[e]);
            }
            if max[e] >= 0 && max[e] < LEVEL_CAP {
                bp.insert(max[e] + 1);
            }
        }
        let bp_level: Vec<i32> = bp
            .into_iter()
            .filter(|&v| (1..=LEVEL_CAP).contains(&v))
            .collect();
        let n_bp = bp_level.len();

        let n_mons = b.mons.len() - 1;
        let mut m_num = vec![0];
        let mut m_status = vec![0];
        let mut m_rooms: Vec<Vec<(usize, u8)>> = vec![Vec::new()];
        for m in &b.mons[1..] {
            m_num.push(m.num);
            m_status.push(m.status);
            m_rooms.push(m.rooms.items.clone());
        }
        let mut reach = Self {
            n_rooms,
            r_map: b.r_map,
            r_room: b.r_room,
            room_idx: b.room_idx,
            n_edges: ne,
            e_from: from,
            e_to: to,
            e_min: min,
            e_max: max,
            e_group: group,
            e_kind: kind,
            e_dir: dir,
            e_label: label,
            e_coin: coin,
            e_wait: wait,
            n_groups,
            n_dropped_random: b.dropped_random,
            adj_start,
            adj_edge,
            gadj_start,
            gadj_edge,
            radj_start,
            radj_edge,
            group_edges,
            grp_ok,
            grp_src,
            n_start,
            poi_idx: b.poi_idx,
            n_bp,
            bp_level,
            mon_idx: b.mon_idx,
            m_num,
            m_status,
            m_rooms,
            m_hit: vec![vec![0u8; n_bp]; n_mons + 1],
        };
        if n_start == 0 {
            // No Bank of Godfrey in this database: don't hide anything.
            for s in &mut reach.m_status {
                if *s == MON_LOCATED {
                    *s = MON_NO_LOCATION;
                }
            }
            return reach;
        }
        for bnd in 0..n_bp {
            let visited = reach.visit_at_level(reach.bp_level[bnd]);
            for m in 1..=n_mons {
                if reach.m_status[m] == MON_LOCATED
                    && reach.m_rooms[m].iter().any(|&(r, _)| visited[r] == 1)
                {
                    reach.m_hit[m][bnd] = 1;
                }
            }
        }
        reach
    }

    #[must_use]
    pub fn stats(&self) -> ReachStats {
        ReachStats {
            rooms: self.n_rooms,
            edges: self.n_edges,
            random_groups: self.n_groups,
            scatter_dropped: self.n_dropped_random,
            level_bands: self.n_bp,
            monsters: self.m_num.len() - 1,
            located: self.m_status.iter().filter(|&&s| s == MON_LOCATED).count(),
        }
    }

    /// The level breakpoints (`bpLevel`).
    #[must_use]
    pub fn breakpoints(&self) -> &[i32] {
        &self.bp_level
    }

    /// A `ReachEnsureComputed` result is per database; the room count identifies nothing, so callers keep
    /// the result next to the load it came from.
    #[must_use]
    pub fn room_count(&self) -> usize {
        self.n_rooms
    }

    fn find_room(&self, map: i32, room: i32) -> usize {
        self.room_idx.get(&(map, room)).copied().unwrap_or(0)
    }

    fn band_of(&self, level: i32) -> usize {
        let level = level.max(1);
        let mut i = 0;
        while i + 1 < self.n_bp {
            if self.bp_level[i + 1] > level {
                break;
            }
            i += 1;
        }
        i
    }

    /// `BandsText`: "1-5,50+" style bands for the monster with this record number ("" when unknown).
    #[must_use]
    pub fn bands_text(&self, monster: i32) -> String {
        let Some(&m) = self.mon_idx.get(&monster) else {
            return String::new();
        };
        let hit = &self.m_hit[m];
        let mut s = String::new();
        let mut lo = 0;
        for b in 0..self.n_bp {
            if hit[b] == 1 {
                if b == 0 || hit[b - 1] == 0 {
                    lo = self.bp_level[b];
                }
                if b == self.n_bp - 1 {
                    if !s.is_empty() {
                        s.push(',');
                    }
                    s.push_str(&format!("{lo}+"));
                } else if hit[b + 1] == 0 {
                    if !s.is_empty() {
                        s.push(',');
                    }
                    s.push_str(&format!("{lo}-{}", self.bp_level[b + 1] - 1));
                }
            }
        }
        s
    }

    /// Status of a monster as `ReachDebugCSV` writes it: 1 not in game, 2 no location, 3 located (0 unknown).
    #[must_use]
    pub fn monster_status_code(&self, monster: i32) -> u8 {
        self.mon_idx.get(&monster).map_or(0, |&m| self.m_status[m])
    }

    /// `ReachIsMonsterOK`: show the monster at this level? True when it is reachable, or its location is
    /// unknown (never hide what cannot be placed), or it is not in the game.
    #[must_use]
    pub fn is_monster_ok(&self, monster: i32, level: i32) -> bool {
        let Some(&m) = self.mon_idx.get(&monster) else {
            return true;
        };
        if self.m_status[m] != MON_LOCATED {
            return true;
        }
        self.m_hit[m][self.band_of(level)] == 1
    }

    // -- traversal --

    /// `VisitAtLevel`.
    fn visit_at_level(&self, level: i32) -> Vec<u8> {
        if self.n_start == 0 {
            return vec![0; self.n_rooms + 1];
        }
        self.visit_from_seeds(&[self.n_start], level).0
    }

    /// `VisitFromSeeds`: (visited, parentEdge, dist), all 1-based.
    fn visit_from_seeds(&self, seeds: &[usize], level: i32) -> (Vec<u8>, Vec<usize>, Vec<i32>) {
        let n = self.n_rooms;
        let mut visited = vec![0u8; n + 1];
        let mut parent = vec![0usize; n + 1];
        let mut dist = vec![0i32; n + 1];
        if seeds.is_empty() {
            return (visited, parent, dist);
        }
        let mut lvl_ok = self.grp_ok.clone();
        for &e in &self.group_edges {
            if level < self.e_min[e] || level > self.e_max[e] {
                lvl_ok[self.e_group[e] as usize] = false;
            }
        }
        let mut queue: Vec<usize> = Vec::with_capacity(n);
        for &u in seeds {
            if u >= 1 && u <= n && visited[u] == 0 {
                visited[u] = 1;
                queue.push(u);
            }
        }
        let mut head = 0;
        loop {
            while head < queue.len() {
                let u = queue[head];
                head += 1;
                for j in self.adj_start[u]..self.adj_start[u + 1] {
                    let e = self.adj_edge[j];
                    let t = self.e_to[e] as usize;
                    if visited[t] == 0 && level >= self.e_min[e] && level <= self.e_max[e] {
                        visited[t] = 1;
                        parent[t] = e;
                        dist[t] = dist[u] + 1;
                        queue.push(t);
                    }
                }
            }
            let mut changed = false;
            for &e in &self.group_edges {
                let g = self.e_group[e] as usize;
                if lvl_ok[g] {
                    let src = self.grp_src[g];
                    let t = self.e_to[e] as usize;
                    if visited[src] == 1 && visited[t] == 0 {
                        visited[t] = 1;
                        parent[t] = e;
                        dist[t] = dist[src] + 1;
                        queue.push(t);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        (visited, parent, dist)
    }

    fn coin_weight(&self, e: usize) -> i64 {
        let w = if self.e_kind[e] == EK_EXIT {
            self.e_coin[e] / (ROUTE_COIN_STEP_COPPER / 100)
        } else {
            self.e_coin[e] / ROUTE_COIN_STEP_COPPER
        };
        i64::from(w.min(99_999))
    }

    /// `RouteSearch`: (visited, parentEdge, dist, cost).
    fn route_search(
        &self,
        seeds: &[usize],
        level: i32,
        ignore_level: bool,
        avoid_coin: bool,
    ) -> (Vec<u8>, Vec<usize>, Vec<i32>, Vec<i64>) {
        let n = self.n_rooms;
        let mut visited = vec![0u8; n + 1];
        let mut parent = vec![0usize; n + 1];
        let mut dist = vec![0i32; n + 1];
        let mut cost = vec![ROUTE_INF; n + 1];
        if seeds.is_empty() {
            return (visited, parent, dist, cost);
        }
        let mut lvl_ok = self.grp_ok.clone();
        if !ignore_level {
            for &e in &self.group_edges {
                if level < self.e_min[e] || level > self.e_max[e] {
                    lvl_ok[self.e_group[e] as usize] = false;
                }
            }
        }
        // (key, push order) pops in the order MME's binary heap does: equal keys leave in push order
        let mut heap: BinaryHeap<Reverse<(i64, u64, usize)>> = BinaryHeap::new();
        let mut seq = 0u64;
        for &u in seeds {
            if u >= 1 && u <= n && cost[u] > 0 {
                cost[u] = 0;
                seq += 1;
                heap.push(Reverse((0, seq, u)));
            }
        }
        while let Some(Reverse((key, _, u))) = heap.pop() {
            if visited[u] != 0 || key != cost[u] {
                continue;
            }
            visited[u] = 1;
            for pass in 1..=2 {
                let (lo, hi) = if pass == 1 {
                    (self.adj_start[u], self.adj_start[u + 1])
                } else {
                    (self.gadj_start[u], self.gadj_start[u + 1])
                };
                for i in lo..hi {
                    let e = if pass == 1 {
                        self.adj_edge[i]
                    } else {
                        self.gadj_edge[i]
                    };
                    let v = self.e_to[e] as usize;
                    if visited[v] != 0 {
                        continue;
                    }
                    if pass == 1 {
                        if !ignore_level && (level < self.e_min[e] || level > self.e_max[e]) {
                            continue;
                        }
                    } else if !lvl_ok[self.e_group[e] as usize] {
                        continue;
                    }
                    let mut new = key + 1;
                    let mut add = i64::from(self.e_wait[e]).min(99_999);
                    if self.e_coin[e] > 0 {
                        add += self.coin_weight(e);
                        if avoid_coin {
                            add += ROUTE_COIN_COST;
                        }
                    }
                    if add > 0 && new < ROUTE_INF - add {
                        new += add;
                    }
                    if new < cost[v] {
                        cost[v] = new;
                        parent[v] = e;
                        dist[v] = dist[u] + 1;
                        seq += 1;
                        heap.push(Reverse((new, seq, v)));
                    }
                }
            }
        }
        (visited, parent, dist, cost)
    }

    /// `FindGateEdge`: the level gate blocking a monster (or a room, when `room_idx` > 0), given the rooms
    /// reachable at some level. 0 = none.
    fn find_gate_edge(&self, mon: usize, visited: &[u8], room_idx: usize) -> usize {
        let n = self.n_rooms;
        let mut seen = vec![0u8; n + 1];
        let mut queue: Vec<usize> = Vec::new();
        if room_idx > 0 {
            seen[room_idx] = 1;
            queue.push(room_idx);
        } else {
            for &(r, _) in &self.m_rooms[mon] {
                if r >= 1 && r <= n && seen[r] == 0 {
                    seen[r] = 1;
                    queue.push(r);
                }
            }
        }
        let mut head = 0;
        while head < queue.len() {
            let u = queue[head];
            head += 1;
            for j in self.radj_start[u]..self.radj_start[u + 1] {
                let e = self.radj_edge[j];
                let f = self.e_from[e] as usize;
                if visited[f] == 1 {
                    return e;
                } else if seen[f] == 0 {
                    seen[f] = 1;
                    queue.push(f);
                }
            }
        }
        0
    }

    /// `ReachMonsterInfo` for a computed result.
    #[must_use]
    pub fn monster_info(&self, monster: i32, level: i32) -> MonsterReach {
        let mut r = MonsterReach {
            status: ReachStatus::Unknown,
            gate_map: 0,
            gate_room: 0,
            dest_map: 0,
            dest_room: 0,
            min_lvl: 0,
            max_lvl: 0,
            random_group: false,
            reachable_at: 0,
        };
        let Some(&m) = self.mon_idx.get(&monster) else {
            return r;
        };
        if self.m_status[m] != MON_LOCATED || self.n_start == 0 || self.n_rooms < 1 {
            return r;
        }
        let level = level.max(1);
        let band = self.band_of(level);
        if self.m_hit[m][band] == 1 {
            r.status = ReachStatus::Yes;
            return r;
        }
        // first higher band that reaches it
        for b in band + 1..self.n_bp {
            if self.m_hit[m][b] == 1 {
                r.reachable_at = self.bp_level[b];
                break;
            }
        }
        let visited = self.visit_at_level(level);
        for &(room, _) in &self.m_rooms[m] {
            if room >= 1 && room <= self.n_rooms && visited[room] == 1 {
                // shouldn't happen (mHit says unreachable); trust the BFS
                r.status = ReachStatus::Yes;
                r.reachable_at = 0;
                return r;
            }
        }
        let gate = self.find_gate_edge(m, &visited, 0);
        if gate == 0 {
            r.status = ReachStatus::NoGate;
            return r;
        }
        let (gf, gt) = (self.e_from[gate] as usize, self.e_to[gate] as usize);
        r.gate_map = self.r_map[gf];
        r.gate_room = self.r_room[gf];
        r.dest_map = self.r_map[gt];
        r.dest_room = self.r_room[gt];
        let mut lo = self.e_min[gate];
        let mut hi = self.e_max[gate];
        let g = self.e_group[gate];
        if g > 0 {
            r.random_group = !self.grp_ok[g as usize];
            // a usable group is level-gated as a whole: it needs a level every one of its edges allows
            for e in 1..=self.n_edges {
                if self.e_group[e] == g {
                    lo = lo.max(self.e_min[e]);
                    hi = hi.min(self.e_max[e]);
                }
            }
            if lo > hi {
                r.random_group = true;
            }
        }
        r.min_lvl = lo.max(1);
        r.max_lvl = if hi >= NO_MAX || hi >= LEVEL_CAP {
            0
        } else {
            hi
        };
        r.status = ReachStatus::No;
        r
    }

    /// `ReachRoomStatus`: can room map/room be reached from Bank of Godfrey at this level?
    #[must_use]
    pub fn room_status(&self, map: i32, room: i32, level: i32) -> ReachStatus {
        if self.n_start == 0 || self.n_rooms < 1 {
            return ReachStatus::Unknown;
        }
        let r = self.find_room(map, room);
        if r == 0 {
            return ReachStatus::Unknown;
        }
        let visited = self.visit_at_level(level.max(1));
        if visited[r] == 1 {
            ReachStatus::Yes
        } else if self.find_gate_edge(0, &visited, r) > 0 {
            ReachStatus::No
        } else {
            ReachStatus::NoGate
        }
    }

    // -- routes --

    /// `RouteSources`: Bank of Godfrey first, then (unless `godfrey_only`) every other point of interest
    /// reachable from it at the character's level.
    fn route_sources(&self, vis_g: &[u8], godfrey_only: bool) -> Vec<usize> {
        let mut seeds = vec![self.n_start];
        if godfrey_only {
            return seeds;
        }
        for p in 1..NUM_POIS {
            let r = self.poi_idx[p];
            if r >= 1 && r <= self.n_rooms && vis_g[r] == 1 {
                seeds.push(r);
            }
        }
        seeds
    }

    fn poi_name(&self, r: usize) -> String {
        for p in 0..NUM_POIS {
            if self.poi_idx[p] == r {
                return POI_NAMES[p].to_string();
            }
        }
        String::new()
    }

    /// `RouteDest`: the lowest category, then the lowest cost, then the lowest room index.
    fn route_dest(&self, mon: usize, visited: &[u8], cost: &[i64]) -> usize {
        let mut dest = 0usize;
        let mut dest_cat = 0u8;
        for &(r, cat) in &self.m_rooms[mon] {
            if r >= 1 && r <= self.n_rooms && visited[r] == 1 {
                if dest == 0 {
                    dest = r;
                    dest_cat = cat;
                } else if cat < dest_cat {
                    dest = r;
                    dest_cat = cat;
                } else if cat == dest_cat
                    && (cost[r] < cost[dest] || (cost[r] == cost[dest] && r < dest))
                {
                    dest = r;
                    dest_cat = cat;
                }
            }
        }
        dest
    }

    /// `RoutePathInfo`: (steps, coin steps, source room) of the search path to `dest`; steps -1 if the links
    /// do not lead back to a seed.
    fn route_path_info(&self, dest: usize, parent: &[usize]) -> (i32, i32, usize) {
        let mut n = 0;
        let mut coin = 0;
        let mut r = dest;
        while parent[r] != 0 {
            n += 1;
            if n as usize > self.n_rooms {
                return (-1, 0, 0);
            }
            let e = parent[r];
            if self.e_coin[e] > 0 {
                coin += 1;
            }
            r = self.e_from[e] as usize;
        }
        (n, coin, r)
    }

    fn edge_has_level(&self, e: usize) -> bool {
        self.e_min[e] > 1 || self.e_max[e] < LEVEL_CAP
    }

    /// `StepFlags`.
    fn step_flags(&self, e: usize) -> i32 {
        let mut f = 0;
        if self.e_kind[e] != EK_EXIT {
            f |= REACH_STEP_ACTION;
        }
        if self.edge_has_level(e) {
            f |= REACH_STEP_LEVEL;
        }
        if self.e_kind[e] == EK_EXIT && !self.e_label[e].is_empty() {
            f |= REACH_STEP_RESTRICT;
        }
        if self.r_map[self.e_from[e] as usize] != self.r_map[self.e_to[e] as usize] {
            f |= REACH_STEP_MAPCHANGE;
        }
        if self.e_group[e] > 0 {
            f |= REACH_STEP_RANDOM;
        }
        if self.e_coin[e] > 0 {
            f |= REACH_STEP_COIN;
        }
        f
    }

    /// `StepLabel`.
    fn step_label(&self, e: usize) -> String {
        let label = &self.e_label[e];
        let mut s = match self.e_kind[e] {
            EK_EXIT => {
                let mut s = if self.e_dir[e] <= 9 {
                    format!("go {}", DIRS[self.e_dir[e] as usize])
                } else {
                    "go ?".to_string()
                };
                if !label.is_empty() {
                    s = format!("{s} {label}");
                }
                s
            }
            EK_COMMAND => format!("type \"{}\"", cmd_text(label)),
            EK_ROOMSPELL => format!("room spell #{label}"),
            EK_NPC => match label.find('|') {
                Some(x) => format!("ask {} {}", &label[..x], cmd_text(&label[x + 1..])),
                None => format!("ask {label}"),
            },
            EK_ITEMSPELL => match label.find('|') {
                Some(x) => {
                    let items: Vec<String> = label[x + 1..]
                        .split(',')
                        .map(|i| format!("item #{i}"))
                        .collect();
                    format!(
                        "cast spell #{} here (needs {})",
                        &label[..x],
                        items.join(", ")
                    )
                }
                None => format!("cast spell #{label} here"),
            },
            _ => String::new(),
        };
        if self.e_kind[e] != EK_EXIT && self.e_coin[e] > 0 {
            s.push_str(&format!(" (price {})", thousands(self.e_coin[e])));
        }
        if self.e_wait[e] > 0 {
            s.push_str(&format!(" (after {} rounds)", thousands(self.e_wait[e])));
        }
        if self.edge_has_level(e) {
            let lo = self.e_min[e].max(1);
            if self.e_max[e] >= LEVEL_CAP {
                s.push_str(&format!(" (Level {lo}+)"));
            } else {
                s.push_str(&format!(" (Level {lo} to {})", self.e_max[e]));
            }
        }
        if self.e_group[e] > 0 {
            s.push_str(" (random destination)");
        }
        s
    }

    /// `RouteSteps`.
    fn route_steps(&self, dest: usize, n: i32, parent: &[usize]) -> (i32, i32, Vec<RouteStep>) {
        let mut steps = vec![
            RouteStep {
                map: 0,
                room: 0,
                flags: 0,
                label: String::new()
            };
            n as usize + 1
        ];
        let mut r = dest;
        for i in (1..=n as usize).rev() {
            let e = parent[r];
            let t = self.e_to[e] as usize;
            steps[i] = RouteStep {
                map: self.r_map[t],
                room: self.r_room[t],
                flags: self.step_flags(e),
                label: self.step_label(e),
            };
            r = self.e_from[e] as usize;
        }
        steps[0].map = self.r_map[r];
        steps[0].room = self.r_room[r];
        (self.r_map[r], self.r_room[r], steps)
    }

    /// `RouteBuild`: the two searches and the route choice. `dest` > 0 routes to that room; else to the
    /// monster `mon`'s best room.
    fn route_build(
        &self,
        seeds: &[usize],
        level: i32,
        ignore_level: bool,
        alt_route: bool,
        mon: usize,
        dest: usize,
    ) -> Option<Route> {
        // F = fewest coin-costing steps, then fewest steps; S = fewest steps (both reach the same rooms)
        let (vis_f, par_f, _, cost_f) = self.route_search(seeds, level, ignore_level, true);
        let (vis_s, par_s, _, cost_s) = self.route_search(seeds, level, ignore_level, false);
        let (dest_f, dest_s) = if dest > 0 {
            (dest, dest)
        } else {
            (
                self.route_dest(mon, &vis_f, &cost_f),
                self.route_dest(mon, &vis_s, &cost_s),
            )
        };
        if dest_f == 0 || dest_s == 0 || vis_f[dest_f] == 0 || vis_s[dest_s] == 0 {
            return None;
        }
        let (n_f, coin_f, src_f) = self.route_path_info(dest_f, &par_f);
        let (n_s, coin_s, src_s) = self.route_path_info(dest_s, &par_s);
        if n_f < 0 || n_s < 0 {
            return None;
        }
        // the coin-avoiding route, unless a shorter coin-costing one exists and it's more than ROUTE_DETOUR_MAX times longer
        let has_alt = coin_s > coin_f && n_s < n_f;
        let mut use_short = false;
        if has_alt {
            use_short = n_f > ROUTE_DETOUR_MAX * n_s;
        }
        if alt_route {
            if !has_alt {
                return None;
            }
            use_short = !use_short;
        }
        let mut route = Route::default();
        if use_short {
            let (sm, sr, steps) = self.route_steps(dest_s, n_s, &par_s);
            route.source_map = sm;
            route.source_room = sr;
            route.steps = steps;
            route.source_name = self.poi_name(src_s);
            route.n_steps = n_s;
            route.coin_steps = coin_s;
            if has_alt {
                route.alt_steps = n_f;
                route.alt_coin_steps = coin_f;
                route.alt_source_name = self.poi_name(src_f);
            }
        } else {
            let (sm, sr, steps) = self.route_steps(dest_f, n_f, &par_f);
            route.source_map = sm;
            route.source_room = sr;
            route.steps = steps;
            route.source_name = self.poi_name(src_f);
            route.n_steps = n_f;
            route.coin_steps = coin_f;
            if has_alt {
                route.alt_steps = n_s;
                route.alt_coin_steps = coin_s;
                route.alt_source_name = self.poi_name(src_s);
            }
        }
        Some(route)
    }

    /// `ReachRoute`: the route from the nearest usable source to the monster's best spawn room, or (with
    /// `to_gate`) to the reachable room holding the gate that blocks it. `alt_route` asks for the alternate.
    #[must_use]
    pub fn route(
        &self,
        monster: i32,
        level: i32,
        to_gate: bool,
        ignore_level: bool,
        alt_route: bool,
    ) -> Option<Route> {
        let &m = self.mon_idx.get(&monster)?;
        if self.m_status[m] != MON_LOCATED || self.n_start == 0 || self.n_rooms < 1 {
            return None;
        }
        let level = level.max(1);
        // sources are chosen at the character's level even when the searches ignore it
        let vis_g = self.visit_at_level(level);
        let seeds = self.route_sources(&vis_g, false);
        let mut dest = 0;
        if to_gate && !ignore_level {
            for &(r, _) in &self.m_rooms[m] {
                if r >= 1 && r <= self.n_rooms && vis_g[r] == 1 {
                    return None; // reachable at this level: no gate
                }
            }
            let e = self.find_gate_edge(m, &vis_g, 0);
            if e == 0 {
                return None;
            }
            dest = self.e_from[e] as usize;
        }
        self.route_build(&seeds, level, ignore_level, alt_route, m, dest)
    }

    /// `ReachRoomRoute`: the same sources and searches with one room as the destination. A room that is itself
    /// a point of interest is routed from Bank of Godfrey alone.
    #[must_use]
    pub fn room_route(
        &self,
        map: i32,
        room: i32,
        level: i32,
        ignore_level: bool,
        alt_route: bool,
    ) -> Option<Route> {
        if self.n_start == 0 || self.n_rooms < 1 {
            return None;
        }
        let r = self.find_room(map, room);
        if r == 0 {
            return None;
        }
        let level = level.max(1);
        let is_poi = self.poi_idx.contains(&r);
        let vis_g = self.visit_at_level(level);
        let seeds = self.route_sources(&vis_g, is_poi);
        self.route_build(&seeds, level, ignore_level, alt_route, 0, r)
    }

    /// Edge columns for the oracle's digests: 1-based, one list per column.
    #[must_use]
    pub fn edge_columns(&self) -> EdgeColumns<'_> {
        EdgeColumns {
            from: &self.e_from[1..],
            to: &self.e_to[1..],
            min: &self.e_min[1..],
            max: &self.e_max[1..],
            group: &self.e_group[1..],
            kind: &self.e_kind[1..],
            dir: &self.e_dir[1..],
            label: &self.e_label[1..],
            coin: &self.e_coin[1..],
            wait: &self.e_wait[1..],
        }
    }

    /// `Rooms` visited at a level, 1-based room index to reached (the rooms `VisitAtLevel` marks).
    #[must_use]
    pub fn visited_at_level(&self, level: i32) -> Vec<bool> {
        self.visit_at_level(level.max(1))[1..]
            .iter()
            .map(|&v| v == 1)
            .collect()
    }
}

/// The edge arrays, 0-based here (edge `i` of MME is element `i - 1`).
#[derive(Debug, Clone, Copy)]
pub struct EdgeColumns<'a> {
    pub from: &'a [i32],
    pub to: &'a [i32],
    pub min: &'a [i32],
    pub max: &'a [i32],
    pub group: &'a [i32],
    pub kind: &'a [u8],
    pub dir: &'a [u8],
    pub label: &'a [String],
    pub coin: &'a [i32],
    pub wait: &'a [i32],
}

/// `SCCIds`: strongly connected component id per room (iterative Tarjan), ignoring level gates.
fn scc_ids(n: usize, adj_start: &[usize], adj_edge: &[usize], e_to: &[i32]) -> Vec<i32> {
    let mut idx = vec![0usize; n + 1];
    let mut low = vec![0usize; n + 1];
    let mut on_stk = vec![false; n + 1];
    let mut comp = vec![0i32; n + 1];
    let mut stk: Vec<usize> = Vec::new();
    let mut work: Vec<(usize, usize)> = Vec::new();
    let mut counter = 1;
    let mut n_comp = 0;
    for root in 1..=n {
        if idx[root] != 0 {
            continue;
        }
        idx[root] = counter;
        low[root] = counter;
        counter += 1;
        stk.push(root);
        on_stk[root] = true;
        work.push((root, adj_start[root]));
        while let Some(&(u, mut p)) = work.last() {
            let mut advanced = false;
            while p < adj_start[u + 1] {
                let v = e_to[adj_edge[p]] as usize;
                p += 1;
                if idx[v] == 0 {
                    if let Some(w) = work.last_mut() {
                        w.1 = p;
                    }
                    idx[v] = counter;
                    low[v] = counter;
                    counter += 1;
                    stk.push(v);
                    on_stk[v] = true;
                    work.push((v, adj_start[v]));
                    advanced = true;
                    break;
                } else if on_stk[v] && idx[v] < low[u] {
                    low[u] = idx[v];
                }
            }
            if !advanced {
                work.pop();
                if let Some(&(parent, _)) = work.last() {
                    if low[u] < low[parent] {
                        low[parent] = low[u];
                    }
                }
                if low[u] == idx[u] {
                    loop {
                        let w = stk.pop().unwrap_or(u);
                        on_stk[w] = false;
                        comp[w] = n_comp;
                        if w == u {
                            break;
                        }
                    }
                    n_comp += 1;
                }
            }
        }
    }
    comp
}

// ---- the detail pane's rows ------------------------------------------------------------------

/// `&H8000&`, `&HC0&` and `&H80000015`: green, dark red and the system grey (`TColor` is the identity).
const GREEN: i32 = 0x8000;
const DARK_RED: i32 = 0xC0;
const SYS_GREY: i32 = -2_147_483_627;

/// What `AddMonsterReachRows` reads from frmMain and module globals.
#[derive(Debug, Clone, Copy)]
pub struct ReachRowOptions {
    /// `val(frmMain.txtGlobalLevel(0).Text)`.
    pub level: i32,
    /// `bHideRecordNumbers`.
    pub hide_record_numbers: bool,
    /// `nRouteWaypointInterval` (below 1 means 25).
    pub waypoint_interval: i32,
}

/// `GetRoomName(, map, room, bHideRecordNumbers)` over a lookup of room names (`None`: no such room).
#[must_use]
pub fn get_room_name(
    names: &dyn Fn(i32, i32) -> Option<String>,
    map: i32,
    room: i32,
    hide: bool,
) -> String {
    if map == 0 || room == 0 {
        return "?".into();
    }
    match names(map, room) {
        None => format!("{map}/{room}"),
        Some(n) => {
            let mut name = if n.is_empty() {
                "(no name)".to_string()
            } else {
                n
            };
            if !hide {
                name = format!("{name} ({map}/{room})");
            }
            name
        }
    }
}

/// `RouteCoinText`.
#[must_use]
pub fn route_coin_text(coin_steps: i32) -> String {
    format!(
        "{coin_steps} {}",
        if coin_steps == 1 {
            "toll/fee"
        } else {
            "tolls/fees"
        }
    )
}

/// `RouteKindText`.
#[must_use]
pub fn route_kind_text(coin_steps: i32, other_coin_steps: i32) -> &'static str {
    if coin_steps == 0 {
        "Coin-free route"
    } else if coin_steps > other_coin_steps {
        "Shorter route"
    } else {
        "Route with fewer tolls/fees"
    }
}

fn row(
    label: &str,
    tag: Option<&str>,
    color: Option<i32>,
    text: String,
    text_color: Option<i32>,
) -> MonsterRow {
    MonsterRow {
        label: label.to_string(),
        tag: tag.map(str::to_string),
        fore_color: color,
        bold: false,
        cells: vec![MonsterCell {
            text,
            tag: None,
            fore_color: text_color,
            bold: false,
        }],
    }
}

/// `AddRouteRows`.
fn add_route_rows(
    rows: &mut Vec<MonsterRow>,
    names: &dyn Fn(i32, i32) -> Option<String>,
    opts: &ReachRowOptions,
    route: &Route,
    dbl_click_word: &str,
) {
    let interval = if opts.waypoint_interval < 1 {
        25
    } else {
        opts.waypoint_interval
    };
    let hide = opts.hide_record_numbers;
    let room_row = |text: String, map: i32, room: i32| {
        let mut r = row("", Some("room"), None, text, None);
        r.cells[0].tag = Some(format!("{map}/{room}"));
        r
    };
    let src = format!(
        "start: {}{}",
        route.source_name,
        if hide {
            String::new()
        } else {
            format!(" ({}/{})", route.source_map, route.source_room)
        }
    );
    rows.push(room_row(src, route.source_map, route.source_room));
    let n_steps = route.n_steps;
    let mut shown = 0;
    let mut more = 0;
    let mut i = interval;
    while i < n_steps {
        if shown < 10 {
            let s = &route.steps[i as usize];
            rows.push(room_row(
                format!("{i}: {}", get_room_name(names, s.map, s.room, hide)),
                s.map,
                s.room,
            ));
            shown += 1;
        } else {
            more += 1;
        }
        i += interval;
    }
    if more > 0 {
        let mut text = format!(
            "... plus {more} more waypoint{}",
            if more == 1 { "" } else { "s" }
        );
        if !dbl_click_word.is_empty() {
            text.push_str(&format!(
                " (double-click {dbl_click_word} for the full route)"
            ));
        }
        rows.push(row("", None, None, text, None));
    }
    if n_steps > 0 {
        let s = &route.steps[n_steps as usize];
        rows.push(room_row(
            format!("{n_steps}: {}", get_room_name(names, s.map, s.room, hide)),
            s.map,
            s.room,
        ));
    }
}

/// `AddRouteAltRow`.
fn add_route_alt_row(
    rows: &mut Vec<MonsterRow>,
    where_: &str,
    alt_steps: i32,
    alt_coin: i32,
    alt_src: &str,
    coin: i32,
) {
    let mut text = format!(
        "{}{where_}: {alt_steps} step{} from {alt_src}",
        route_kind_text(alt_coin, coin),
        if alt_steps == 1 { "" } else { "s" }
    );
    if alt_coin > 0 {
        text.push_str(&format!(", {}", route_coin_text(alt_coin)));
    }
    text.push_str(" (double-click to view)");
    rows.push(row(
        "",
        Some("reach_alt_placeholder"),
        None,
        text,
        Some(SYS_GREY),
    ));
    if let Some(r) = rows.last_mut() {
        r.tag = Some("reach_route_alt".into());
    }
}

/// `AddMonsterReachRows` when nothing has been computed: the "double-click to compute" row.
#[must_use]
pub fn reach_not_computed_rows() -> Vec<MonsterRow> {
    vec![
        row(
            "Reachable",
            Some("reach_compute"),
            Some(SYS_GREY),
            "(double-click to compute)".into(),
            Some(SYS_GREY),
        ),
        // a blank row has no sub-item in MME (only its Text is assigned)
        MonsterRow {
            label: String::new(),
            ..MonsterRow::default()
        },
    ]
}

/// `AddMonsterReachRows` for a computed result: the monster pane's "Reachable" row, the route rows and the
/// alternate-route row. `names` answers `GetRoomName`'s lookup of a room's `Name` (`None`: no such room).
#[must_use]
pub fn monster_reach_rows(
    reach: &Reach,
    monster: i32,
    opts: &ReachRowOptions,
    names: &dyn Fn(i32, i32) -> Option<String>,
) -> Vec<MonsterRow> {
    let mut rows = Vec::new();
    let level = opts.level;
    let info = reach.monster_info(monster, level);
    let help = " - Double-Click for Detail";
    let mut added = false;
    match info.status {
        ReachStatus::Yes => {
            let route = reach.route(monster, level, false, false, false);
            let (tag, text) = match &route {
                Some(r) => (
                    Some("reach_route"),
                    format!(
                        "Yes ({} step{} from {}{}){help}",
                        r.n_steps,
                        if r.n_steps == 1 { "" } else { "s" },
                        r.source_name,
                        if r.coin_steps > 0 {
                            format!(", {}", route_coin_text(r.coin_steps))
                        } else {
                            String::new()
                        }
                    ),
                ),
                None => (None, format!("Yes{help}")),
            };
            rows.push(row("Reachable", tag, Some(GREEN), text, Some(GREEN)));
            added = true;
            if let Some(r) = &route {
                add_route_rows(&mut rows, names, opts, r, "Yes");
                if r.alt_steps > 0 {
                    add_route_alt_row(
                        &mut rows,
                        "",
                        r.alt_steps,
                        r.alt_coin_steps,
                        &r.alt_source_name,
                        r.coin_steps,
                    );
                }
            }
        }
        ReachStatus::NoGate => {
            rows.push(row(
                "Reachable",
                None,
                Some(DARK_RED),
                "No, no known path from Bank of Godfrey".into(),
                Some(DARK_RED),
            ));
            added = true;
        }
        ReachStatus::No => {
            let mut reason = if info.random_group {
                "random teleport (unusable)".to_string()
            } else if level < info.min_lvl {
                let mut s = format!(
                    "needs Level {}{}",
                    info.min_lvl,
                    if info.max_lvl > 0 {
                        format!(" to {}", info.max_lvl)
                    } else {
                        "+".into()
                    }
                );
                if info.reachable_at == 0 {
                    s.push_str(" (still blocked at higher levels)");
                }
                s
            } else {
                format!(
                    "level too high (Level {} to {})",
                    info.min_lvl, info.max_lvl
                )
            };
            // the gate shown is the nearest one, not the lowest
            if info.reachable_at > 0 && (info.random_group || info.reachable_at != info.min_lvl) {
                reason.push_str(&format!(" (reachable from Level {})", info.reachable_at));
            }
            // the full route past the gate is what double-clicking the No row opens
            let full = reach.route(monster, level, false, true, false);
            let text = format!(
                "No - blocked by {}, {reason}",
                get_room_name(
                    names,
                    info.gate_map,
                    info.gate_room,
                    opts.hide_record_numbers
                )
            );
            rows.push(row(
                "Reachable",
                if full.is_some() {
                    Some("reach_route")
                } else {
                    None
                },
                Some(DARK_RED),
                text,
                Some(DARK_RED),
            ));
            added = true;
            // route to the room holding the blocking gate
            if let Some(r) = reach.route(monster, level, true, false, false) {
                add_route_rows(
                    &mut rows,
                    names,
                    opts,
                    &r,
                    if full.is_some() { "No" } else { "" },
                );
            }
            if let Some(f) = &full {
                if f.alt_steps > 0 {
                    add_route_alt_row(
                        &mut rows,
                        " past the level gate",
                        f.alt_steps,
                        f.alt_coin_steps,
                        &f.alt_source_name,
                        f.coin_steps,
                    );
                }
            }
        }
        ReachStatus::Unknown => {}
    }
    if added {
        rows.push(row("", None, None, String::new(), None));
        // a blank row has no sub-item in MME (only its Text is assigned)
        if let Some(last) = rows.last_mut() {
            last.cells.clear();
        }
    }
    rows
}

impl crate::filters::Reachability for Reach {
    fn is_monster_ok(&self, number: i32, level: i32) -> bool {
        Reach::is_monster_ok(self, number, level)
    }
}

/// The monster pane's reach rows (`AddMonsterReachRows`): the computed result when there is one, else the
/// "double-click to compute" row.
pub struct PaneReach<'a> {
    pub reach: Option<&'a Reach>,
    pub opts: ReachRowOptions,
    /// `GetRoomName`'s lookup of a room's `Name`.
    pub names: &'a dyn Fn(i32, i32) -> Option<String>,
}

impl crate::detail::monster::ReachRows for PaneReach<'_> {
    fn rows(&self, monster: i32) -> Vec<MonsterRow> {
        match self.reach {
            Some(r) => monster_reach_rows(r, monster, &self.opts, self.names),
            None => reach_not_computed_rows(),
        }
    }
}
