//! Reference rows: the "where is this / obtained from" lists (FAM-19, stock only). Card P3-31.
//!
//! Ports modMain `GetLocations`, `AddRoomCmdLocationRow` and `ExtractRoomActions`. MME fills a
//! ListView; the port fills a [`LocationList`] with the same rows in the same order, each as the
//! ListView would hold it (text, tag, sub-items with their tags). Nested calls (a shop's rooms, a
//! chest's contents) share the one list, as MME shares one ListView.
//!
//! ListView sorting is a skipped feature (the grid sorts): [`LocationList::sorts`] records, for every
//! `SortListView` request MME makes, how many rows the list held, so a caller that wants MME's
//! order can sort the first n rows by their first cell at that moment.
//!
//! Where a VB6 run-time error would reach MME's handler the procedure ends there with the rows
//! built so far (the error number goes to [`LocationList::errors`]).

use touchstone_model::{Item, Shop, World};
use touchstone_vb6::{Currency, Vb6Error, clng, cstr, format, len, mid, right, round, val};

use crate::item_value::{get_item_shop_regen_pct, get_item_value, get_shop_markup};
use crate::lairs::LairInfo;
use crate::textblock::{RoomCmdSource, extract_map_room, split_room_cmd_sources};
use crate::{Profile, RulesError};

type Res<T> = Result<T, Vb6Error>;

/// The database rows the lookups read (MME seeks the global recordsets).
pub trait LocationData {
    /// The `Items` row with this number.
    fn item(&self, number: i32) -> Option<&Item>;
    /// The `Shops` row with this number.
    fn shop(&self, number: i32) -> Option<&Shop>;
    /// The `Name` of the monster; `None` when there is no such row.
    fn monster_name(&self, number: i32) -> Option<String>;
    /// The `Name` of the spell; `None` when there is no such row.
    fn spell_name(&self, number: i32) -> Option<String>;
    /// The room's `Name` (empty when unnamed) and `Delay`; `None` when there is no such room.
    fn room(&self, map: i32, room: i32) -> Option<(String, i32)>;
}

impl LocationData for World {
    fn item(&self, number: i32) -> Option<&Item> {
        self.get_item(number)
    }
    fn shop(&self, number: i32) -> Option<&Shop> {
        self.get_shop(number)
    }
    fn monster_name(&self, number: i32) -> Option<String> {
        self.get_monster(number)
            .map(|m| m.name.clone().unwrap_or_default())
    }
    fn spell_name(&self, number: i32) -> Option<String> {
        self.get_spell(number)
            .map(|s| s.name.clone().unwrap_or_default())
    }
    fn room(&self, map: i32, room: i32) -> Option<(String, i32)> {
        self.get_room(map, room)
            .map(|r| (r.name.clone().unwrap_or_default(), r.delay))
    }
}

/// `GetLairInfo(group_index, max_regen)`: answered by the lair cache (P3-16).
pub trait LocationLairs {
    fn get_lair_info(&mut self, group_index: &str, max_regen: i16) -> LairInfo;
}

/// What MME reads from frmMain and module globals.
#[derive(Debug, Clone)]
pub struct LocationOptions {
    /// `nNMRVer`.
    pub nmr_ver: f64,
    /// `bHideRecordNumbers`.
    pub hide_record_numbers: bool,
    /// `frmMain.chkGlobalFilter.Value = 1`.
    pub global_filter: bool,
    /// `frmMain.txtCharStats(5).Tag` (the charm), read with `Val`.
    pub charm_tag: String,
    /// `nGlobalAttackTypeMME` (0 one-shot ... 5 manual).
    pub attack_type: i32,
}

/// `GetLocations`'s arguments, in MME's order; an omitted optional one is its default.
#[derive(Debug, Clone, Default)]
pub struct LocationArgs<'a> {
    pub loc: &'a str,
    pub dont_clear: bool,
    pub header: &'a str,
    pub aux_value: i32,
    pub two_columns: bool,
    pub dont_sort: bool,
    pub percent_column: bool,
    pub footer: &'a str,
    pub limit: i16,
}

/// What a row stands for (derived from the code that adds it; MME has no such field).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationKind {
    Room,
    Monster,
    Textblock,
    Item,
    Spell,
    Lair,
    Group,
    Npc,
    RoomCmd,
    /// The "... plus more." row of a cut text.
    More,
    /// The "... plus n more. Double-click to see all." row (tag `nolimit`).
    NoLimit,
}

/// A ListView sub-item.
#[derive(Debug, Clone, PartialEq)]
pub struct LocationCell {
    pub text: String,
    /// `None` when MME never assigned the sub-item's Tag.
    pub tag: Option<String>,
}

/// A ListView row: `Text`, `Tag` (as `CStr` of what MME assigned) and `ListSubItems`.
#[derive(Debug, Clone, PartialEq)]
pub struct LocationRow {
    pub kind: LocationKind,
    pub text: String,
    pub tag: Option<String>,
    pub cells: Vec<LocationCell>,
}

/// The ListView being filled.
#[derive(Debug, Clone, Default)]
pub struct LocationList {
    pub rows: Vec<LocationRow>,
    /// Row count at each `SortListView(lv, 1, string, ascending)` request.
    pub sorts: Vec<usize>,
    /// VB error numbers that reached `GetLocations`' or `AddRoomCmdLocationRow`'s handler.
    pub errors: Vec<i32>,
}

// ---- VB6 helpers ---------------------------------------------------------------------------------

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

/// `InStr(start, hay, needle)` over characters.
fn instr(start: i32, hay: &[char], needle: &[char]) -> i32 {
    if start < 1 || start as usize > hay.len() {
        return 0;
    }
    if needle.is_empty() {
        return start;
    }
    if hay.len() < needle.len() {
        return 0;
    }
    (start as usize - 1..=hay.len() - needle.len())
        .find(|&i| hay[i..i + needle.len()] == *needle)
        .map_or(0, |i| i as i32 + 1)
}

fn instr_s(start: i32, hay: &str, needle: &str) -> i32 {
    instr(start, &chars(hay), &chars(needle))
}

/// `Mid(chars, start, length)` where an error is impossible (start >= 1, length >= 0 checked by caller).
fn mid_c(c: &[char], start: i32, length: i32) -> String {
    if start < 1 || length < 0 {
        return String::new();
    }
    c.iter()
        .skip(start as usize - 1)
        .take(length as usize)
        .collect()
}

pub(crate) fn lcase(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii() {
                c.to_ascii_lowercase()
            } else {
                c.to_lowercase().next().unwrap_or(c)
            }
        })
        .collect()
}

fn cur(x: f64) -> Res<Currency> {
    Currency::from_f64(x)
}

fn cur_int(v: i64) -> Currency {
    Currency::from_scaled(v * 10_000)
}

/// `CStr(Currency)`.
pub(crate) fn cur_str(c: Currency) -> String {
    let s = c.scaled();
    let sign = if s < 0 { "-" } else { "" };
    let a = s.unsigned_abs();
    let (whole, frac) = (a / 10_000, a % 10_000);
    if frac == 0 {
        format!("{sign}{whole}")
    } else {
        let f = format!("{frac:04}");
        format!("{sign}{whole}.{}", f.trim_end_matches('0'))
    }
}

/// `Round(c, digits)` on a Currency (exact, half to even), digits 0 or 2.
pub(crate) fn round_cur(c: Currency, digits: u32) -> Currency {
    let q = 10i64.pow(4 - digits);
    let s = c.scaled();
    let d = s.div_euclid(q);
    let r = s.rem_euclid(q);
    let d = match (r * 2).cmp(&q) {
        std::cmp::Ordering::Less => d,
        std::cmp::Ordering::Greater => d + 1,
        std::cmp::Ordering::Equal => d + (d & 1),
    };
    Currency::from_scaled(d * q)
}

/// Port of modSyntaxsFunc `ExtractNumbersFromString`: the first number in the text.
pub(crate) fn extract_numbers(s: &str) -> f64 {
    let mut n = String::new();
    let mut ignore_decimal = false;
    for c in s.chars() {
        match c {
            '0'..='9' => n.push(c),
            '.' => {
                if !n.is_empty() && !ignore_decimal {
                    n.push(c);
                    ignore_decimal = true;
                }
            }
            '-' => {
                if n.is_empty() {
                    n.push(c);
                }
            }
            _ => {
                if n == "-" {
                    n.clear();
                } else if !n.is_empty() {
                    break;
                }
            }
        }
    }
    val(&n)
}

/// Port of modMain `ExtractRoomActions`: the text after the first colon, trimmed of spaces.
#[must_use]
pub fn extract_room_actions(exit: &str) -> String {
    let p = instr_s(1, exit, ":");
    if p > 0 {
        let rest = mid(exit, p + 1, Some(999)).unwrap_or_default();
        rest.trim_matches(' ').to_string()
    } else {
        exit.to_string()
    }
}

// ---- name lookups -----------------------------------------------------------------------------------

/// `GetRoomName(sMapRoom, , , bNoRoomNumber)`.
fn room_name(data: &dyn LocationData, map_room: &str, hide: bool) -> String {
    let e = extract_map_room(map_room);
    if e.map == 0 || e.room == 0 {
        return "?".into();
    }
    match data.room(e.map, e.room) {
        None => format!("{}/{}", e.map, e.room),
        Some((name, _)) => {
            let mut n = if name.is_empty() {
                "(no name)".to_string()
            } else {
                name
            };
            if !hide {
                n = format!("{n} ({}/{})", e.map, e.room);
            }
            n
        }
    }
}

/// `GetRoomRegen(sMapRoom)` (stock): the room's `Delay`; 0 where MME's handler swallows a type mismatch.
fn room_regen(data: &dyn LocationData, nmr_ver: f64, map_room: &str) -> f64 {
    if nmr_ver < 1.83 {
        return 0.0;
    }
    let e = extract_map_room(map_room);
    if e.map == 0 || e.room == 0 {
        return 0.0;
    }
    data.room(e.map, e.room).map_or(0.0, |(_, d)| f64::from(d))
}

fn named(number: i32, name: Option<String>, hide: bool) -> String {
    if number == 0 {
        return "None".into();
    }
    match name {
        None => number.to_string(),
        Some(n) if hide => n,
        Some(n) => format!("{n}({number})"),
    }
}

fn item_name(data: &dyn LocationData, n: i32, hide: bool) -> String {
    named(n, data.item(n).map(|i| i.name.clone()), hide)
}

/// `GetShopLocation`: the shop's `Assigned To` text.
fn shop_location(data: &dyn LocationData, n: i32) -> String {
    if n == 0 {
        return "None".into();
    }
    match data.shop(n) {
        None => n.to_string(),
        Some(s) => s.assigned_to.clone().unwrap_or_default(),
    }
}

// ---- GetLocations ----------------------------------------------------------------------------------

struct Ctx<'a> {
    data: &'a dyn LocationData,
    lairs: &'a mut dyn LocationLairs,
    opt: &'a LocationOptions,
    out: &'a mut LocationList,
}

/// Port of modMain `GetLocations`: appends the rows for the reference text `args.loc` to `out`
/// (after clearing it unless `args.dont_clear`).
pub fn get_locations(
    data: &dyn LocationData,
    lairs: &mut dyn LocationLairs,
    opt: &LocationOptions,
    args: &LocationArgs<'_>,
    out: &mut LocationList,
) {
    let mut ctx = Ctx {
        data,
        lairs,
        opt,
        out,
    };
    ctx.get_locations(args);
}

const LOOKS: [&str; 12] = [
    "room ",
    "monster #",
    "textblock #",
    "textblock(rndm) #",
    "item #",
    "spell #",
    "shop #",
    "shop(sell) #",
    "shop(nogen) #",
    "group(lair): ",
    "group: ",
    "npc #",
];

impl Ctx<'_> {
    fn get_locations(&mut self, a: &LocationArgs<'_>) {
        if !a.dont_clear {
            self.out.rows.clear();
        }
        if let Err(e) = self.run(a) {
            self.out.errors.push(e.code() as i32);
        }
    }

    fn add_row(&mut self, kind: LocationKind, text: &str, tag: Option<String>) -> usize {
        self.out.rows.push(LocationRow {
            kind,
            text: text.to_string(),
            tag,
            cells: Vec::new(),
        });
        self.out.rows.len() - 1
    }

    fn add_cell(&mut self, row: usize, text: String, tag: Option<String>) {
        self.out.rows[row].cells.push(LocationCell { text, tag });
    }

    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
    fn run(&mut self, a: &LocationArgs<'_>) -> Res<()> {
        let hide = self.opt.hide_record_numbers;
        let nmr = self.opt.nmr_ver;
        let mut display_footer = a.footer.to_string();
        if len(a.loc) < 5 {
            return Ok(());
        }
        let mut lair_re = String::from(r"group\(lair\): ([0-9]+)/([0-9]+)");
        if nmr >= 1.82 {
            lair_re = format!(r"\[([0-9]+)\]{lair_re}");
        }
        if nmr >= 1.83 {
            lair_re = format!(r"\[([0-9\-]+)\]{lair_re}");
        }
        let lair_re = regex::Regex::new(&format!("(?i){lair_re}")).expect("lair pattern");

        let mut room_cmds: Vec<RoomCmdSource> = Vec::new();
        let mut scan = a.loc.to_string();
        if instr_s(1, a.loc, "Room(") > 0 {
            let (rest, cmds) = split_room_cmd_sources(a.loc);
            scan = rest;
            room_cmds = cmds;
        }
        let test = chars(&lcase(&scan));
        let tlen = test.len() as i32;
        let limit = i32::from(a.limit);
        let mut count: i32 = 0;
        let mut n_value: i32 = 0;

        'finish: {
            for z in 1..=12usize {
                let look = chars(LOOKS[z - 1]);
                let mut x: i32 = 1;
                loop {
                    if limit > 0 && count > limit {
                        break 'finish;
                    }
                    let mut s_percent = String::new();
                    let found = instr(x, &test, &look);
                    if found == 0 {
                        break;
                    }
                    x = found;
                    let y1 = x + look.len() as i32;
                    let mut y2: i32 = 0;
                    loop {
                        let ch = mid_c(&test, y1 + y2, 1);
                        match ch.as_str() {
                            "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "/"
                            | "."
                                if y1 + y2 - 1 != tlen =>
                            {
                                y2 += 1;
                                continue;
                            }
                            "+" => return Ok(()),
                            "(" => {
                                let x2 = instr(y1 + y2, &test, &[')']);
                                if x2 != 0 {
                                    s_percent =
                                        format!(" {}", mid_c(&test, y1 + y2, x2 - y1 - y2 + 1));
                                }
                            }
                            _ => {}
                        }
                        break;
                    }
                    if y2 == 0 {
                        x = y1;
                        continue;
                    }
                    let numtext = mid_c(&test, y1, y2);
                    if z != 1 {
                        n_value = clng(val(&numtext))?;
                    }

                    let mut n_percent = Currency::from_scaled(0);
                    if a.percent_column {
                        n_percent = cur(extract_numbers(&s_percent))?;
                        if instr_s(1, a.footer, "%)") > 0 {
                            let p = instr_s(1, a.footer, "(");
                            let n2 = cur(extract_numbers(&mid(a.footer, p, None)?))?;
                            if n2 > Currency::from_scaled(0) {
                                display_footer =
                                    a.footer.replace(&format!("({}%)", cur_str(n2)), "");
                                if n_percent > Currency::from_scaled(0) {
                                    n_percent =
                                        n_percent.checked_mul(n2)?.checked_div(cur_int(100))?;
                                } else {
                                    n_percent = n2;
                                }
                            }
                        }
                        n_percent = if n_percent > cur_int(1) {
                            round_cur(n_percent, 0)
                        } else {
                            round_cur(n_percent, 2)
                        };
                    }
                    let df = display_footer.clone();

                    match z {
                        1 => {
                            if limit > 0 {
                                count += 1;
                            }
                            if limit > 0 && count > limit {
                                x = y1;
                                continue;
                            }
                            let mut loc = String::from("Room: ");
                            if !a.header.is_empty() {
                                if instr_s(1, a.header, ":") == 0 {
                                    loc.push_str(a.header);
                                } else {
                                    loc = a.header.to_string();
                                }
                            }
                            let name = room_name(self.data, &numtext, hide);
                            let aux = a.aux_value > 0 && !a.header.is_empty();
                            let sub_tag = if aux {
                                a.aux_value.to_string()
                            } else {
                                numtext.clone()
                            };
                            let kind = LocationKind::Room;
                            if a.percent_column {
                                let t = pct_label(n_percent);
                                let r = self.add_row(kind, &t, Some(cur_str(n_percent)));
                                self.add_cell(r, format!("{loc}{name}{df}"), Some(sub_tag));
                            } else if a.two_columns {
                                let r = self.add_row(kind, &loc, Some("Room".into()));
                                self.add_cell(r, format!("{name}{s_percent}{df}"), Some(sub_tag));
                            } else {
                                let tag = if aux {
                                    a.aux_value.to_string()
                                } else {
                                    numtext.clone()
                                };
                                self.add_row(
                                    kind,
                                    &format!("{loc}{name}{s_percent}{df}"),
                                    Some(tag),
                                );
                            }
                        }
                        2 | 12 => {
                            if limit > 0 {
                                count += 1;
                            }
                            if limit > 0 && count > limit {
                                x = y1;
                                continue;
                            }
                            let (loc, kind) = if z == 2 {
                                ("Monster: ", LocationKind::Monster)
                            } else {
                                ("NPC: ", LocationKind::Npc)
                            };
                            let name = named(n_value, self.data.monster_name(n_value), hide);
                            self.value_row(
                                a, kind, n_percent, loc, &name, &s_percent, &df, n_value, "monster",
                            );
                        }
                        3 | 4 => {
                            if limit > 0 {
                                count += 1;
                            }
                            if limit > 0 && count > limit {
                                x = y1;
                                continue;
                            }
                            self.value_row(
                                a,
                                LocationKind::Textblock,
                                n_percent,
                                "Textblock ",
                                &n_value.to_string(),
                                &s_percent,
                                &df,
                                n_value,
                                "textblock",
                            );
                        }
                        5 => {
                            if limit > 0 {
                                count += 1;
                            }
                            if limit > 0 && count > limit {
                                x = y1;
                                continue;
                            }
                            let name = item_name(self.data, n_value, hide);
                            self.value_row(
                                a,
                                LocationKind::Item,
                                n_percent,
                                "Item: ",
                                &name,
                                &s_percent,
                                &df,
                                n_value,
                                "item",
                            );
                            if let Some(chest) = self
                                .data
                                .item(n_value)
                                .filter(|i| i.item_type == 8)
                                .filter(|_| a.header.is_empty() && a.footer.is_empty())
                            {
                                let loc = chest.obtained_from.clone().unwrap_or_default();
                                let footer = format!(" -> {}{}", chest.name, s_percent);
                                let sub = LocationArgs {
                                    loc: &loc,
                                    dont_clear: true,
                                    dont_sort: true,
                                    percent_column: a.percent_column,
                                    footer: &footer,
                                    limit: (limit - count) as i16,
                                    ..LocationArgs::default()
                                };
                                self.get_locations(&sub);
                            }
                        }
                        6 => {
                            if limit > 0 {
                                count += 1;
                            }
                            if limit > 0 && count > limit {
                                x = y1;
                                continue;
                            }
                            let name = named(n_value, self.data.spell_name(n_value), hide);
                            self.value_row(
                                a,
                                LocationKind::Spell,
                                n_percent,
                                "Spell: ",
                                &name,
                                &s_percent,
                                &df,
                                n_value,
                                "spell",
                            );
                        }
                        7..=9 => {
                            if limit > 0 {
                                count += 1;
                            }
                            if limit > 0 && count > limit {
                                x = y1;
                                continue;
                            }
                            self.shop_rows(a, z, n_value, count, n_percent)?;
                        }
                        10 => {
                            self.lair_row(
                                a, &test, &look, y1, y2, &s_percent, &df, n_percent, &lair_re,
                            )?;
                        }
                        11 => {
                            if limit > 0 {
                                count += 1;
                            }
                            if limit > 0 && count > limit {
                                x = y1;
                                continue;
                            }
                            let name = room_name(self.data, &numtext, hide);
                            if a.percent_column {
                                let r = self.add_row(
                                    LocationKind::Group,
                                    &pct_label(n_percent),
                                    Some(cur_str(n_percent)),
                                );
                                self.add_cell(r, format!("Group: {name}{df}"), Some(numtext));
                            } else if a.two_columns {
                                let r = self.add_row(
                                    LocationKind::Group,
                                    "Group: ",
                                    Some("Room".into()),
                                );
                                self.add_cell(r, format!("{name}{s_percent}{df}"), Some(numtext));
                            } else {
                                self.add_row(
                                    LocationKind::Group,
                                    &format!("Group: {name}{s_percent}{df}"),
                                    Some(numtext),
                                );
                            }
                        }
                        _ => {}
                    }
                    x = y1;
                }
            }

            for src in &room_cmds {
                if limit > 0 {
                    count += 1;
                }
                if limit > 0 && count > limit {
                    break 'finish;
                }
                self.add_room_cmd_row(src, a.two_columns, a.percent_column, a.footer);
            }
        }

        if self.out.rows.len() > 1 && !a.dont_sort && !a.percent_column {
            self.out.sorts.push(self.out.rows.len());
        }
        if self.out.rows.len() > 1 {
            if right(a.loc, 2)? == "+\u{0}" {
                if a.footer.is_empty() {
                    self.more_row(a, "... plus more.", LocationKind::More, "0");
                }
            } else if limit > 0 && count >= limit && a.header.is_empty() && a.footer.is_empty() {
                let t = format!("... plus {} more. Double-click to see all.", count - limit);
                self.more_row(a, &t, LocationKind::NoLimit, "nolimit");
            }
        }
        Ok(())
    }

    fn more_row(&mut self, a: &LocationArgs<'_>, text: &str, kind: LocationKind, tag: &str) {
        if a.two_columns || a.percent_column {
            let r = self.add_row(kind, "", Some(tag.to_string()));
            self.add_cell(r, text.to_string(), None);
        } else {
            self.add_row(kind, text, Some(tag.to_string()));
        }
    }

    /// Rows of the monster, NPC, textblock, item and spell keywords: all lay out the same way.
    #[allow(clippy::too_many_arguments)]
    fn value_row(
        &mut self,
        a: &LocationArgs<'_>,
        kind: LocationKind,
        percent: Currency,
        loc: &str,
        name: &str,
        s_percent: &str,
        df: &str,
        n_value: i32,
        two_tag: &str,
    ) {
        let h = a.header;
        let nv = n_value.to_string();
        if a.percent_column {
            let r = self.add_row(kind, &pct_label(percent), Some(cur_str(percent)));
            self.add_cell(r, format!("{loc}{h}{name}{df}"), Some(nv));
        } else if a.two_columns {
            let r = self.add_row(kind, &format!("{loc}{h}"), Some(two_tag.to_string()));
            self.add_cell(r, format!("{name}{s_percent}{df}"), Some(nv));
        } else {
            self.add_row(kind, &format!("{loc}{h}{name}{s_percent}{df}"), Some(nv));
        }
    }

    fn charm(&self) -> Res<i16> {
        if self.opt.global_filter {
            touchstone_vb6::cint(val(&self.opt.charm_tag))
        } else {
            Ok(0)
        }
    }

    fn shop_rows(
        &mut self,
        a: &LocationArgs<'_>,
        z: usize,
        shop: i32,
        count: i32,
        _n_percent: Currency,
    ) -> Res<()> {
        let mut shop_value = String::new();
        if shop > 0 && a.aux_value > 0 {
            let markup = get_shop_markup(shop, self.data.shop(shop));
            let charm = self.charm()?;
            let v = get_item_value(
                Profile::Stock,
                a.aux_value,
                self.data.item(a.aux_value),
                charm,
                markup,
                0,
                None,
                false,
            )
            .map_err(rules_err)?;
            if z == 8 {
                if v.copper_sell > 0.0 {
                    shop_value = format!(" - Value: {}", v.friendly_sell_short);
                }
            } else if v.base_cost > 0 {
                shop_value = format!(
                    " - Value: {}/{}",
                    v.friendly_buy_short, v.friendly_sell_short
                );
            }
        }
        let rest = (i32::from(a.limit) - count) as i16;
        let location = shop_location(self.data, shop);
        let header = match z {
            7 => "Shop: ",
            8 => "Shop (sell): ",
            _ => "Shop (nogen): ",
        };
        let mut loc = location;
        let mut percent_col = a.percent_column;
        if z == 7 {
            if a.percent_column && a.aux_value > 0 {
                let pct = get_item_shop_regen_pct(
                    Profile::Stock,
                    shop,
                    a.aux_value,
                    self.data.shop(shop),
                )
                .map_err(rules_err)?;
                if pct > Currency::from_scaled(0) {
                    let mut t = loc.replace(',', &format!("({}%),", cur_str(pct)));
                    if right(&t, 2)? != "%)" {
                        t = format!("{t}({}%)", cur_str(pct));
                    }
                    loc = t;
                }
            } else {
                percent_col = false;
            }
        }
        let sub = LocationArgs {
            loc: &loc,
            dont_clear: true,
            header,
            aux_value: shop,
            percent_column: percent_col,
            footer: &shop_value,
            limit: rest,
            ..LocationArgs::default()
        };
        self.get_locations(&sub);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn lair_row(
        &mut self,
        a: &LocationArgs<'_>,
        test: &[char],
        look: &[char],
        y1: i32,
        y2: i32,
        s_percent: &str,
        df: &str,
        n_percent: Currency,
        lair_re: &regex::Regex,
    ) -> Res<()> {
        let hide = self.opt.hide_record_numbers;
        let nmr = self.opt.nmr_ver;
        let zero = Currency::from_scaled(0);
        let mut max_regen: i16 = 0;
        let mut spawn = zero;
        let mut info = LairInfo::default(); // `GetLairInfo("")` (the reset) answers an empty type
        let llen = look.len() as i32;

        let seg = if (y1 - llen - 2) > 0 {
            let mut x2 = y1 - llen - 1;
            while x2 >= 1 {
                if mid_c(test, x2, 1) == "," {
                    break;
                }
                x2 -= 1;
            }
            mid_c(test, x2 + 1, y1 + y2 - x2 - 1)
        } else {
            mid_c(test, y1 - llen, y1 + y2 - 1)
        };
        let room_key;
        if let Some(m) = lair_re.captures(&seg) {
            let g = |i: usize| m.get(i).map_or("", |x| x.as_str());
            if nmr >= 1.83 {
                let group = g(1).to_string();
                max_regen = touchstone_vb6::cint(val(g(2)))?;
                room_key = format!("{}/{}", g(3), g(4));
                info = self.lairs.get_lair_info(&group, max_regen);
                if info.mobs > zero {
                    let inv = cur_int(1).checked_div(info.mobs)?;
                    let base = cur_int(1).checked_sub(inv)?.to_f64();
                    let p = touchstone_vb6::pow(base, f64::from(max_regen))?;
                    spawn = cur(round(1.0 - p, 2)? * 100.0)?;
                }
            } else if nmr >= 1.82 {
                max_regen = touchstone_vb6::cint(val(g(1)))?;
                room_key = format!("{}/{}", g(2), g(3));
            } else {
                room_key = format!("{}/{}", g(1), g(2));
            }
        } else {
            room_key = mid_c(test, y1, y2);
        }

        let mut loc = if spawn > zero {
            format!("Lair {max_regen} ({}%)", cur_str(spawn))
        } else if max_regen > 0 {
            format!("Lair {max_regen}")
        } else {
            "Lair".to_string()
        };
        let regen = room_regen(self.data, nmr, &room_key);
        if regen > 0.0 {
            loc = format!("{loc} - {}m", cstr(regen));
        }
        let name = room_name(self.data, &room_key, hide);
        if a.percent_column {
            let r = self.add_row(
                LocationKind::Lair,
                &pct_label(n_percent),
                Some(cur_str(n_percent)),
            );
            self.add_cell(r, format!("{loc}{name}{df}"), Some(room_key));
        } else if a.two_columns {
            let mut t = format!("{name}{s_percent}{df}");
            if info.avg_exp > zero {
                let e = info.avg_exp.checked_mul(info.max_regen)?;
                t.push_str(&format!(", Exp: {}", format(e.to_f64(), "#,##0")?));
            }
            let shows_out = self.opt.attack_type > 0 && self.opt.attack_type != 5;
            if info.avg_dmg_clear_round > zero && info.damage_out > 0 && shows_out {
                t.push_str(&format!(
                    ", Dmg In/Out: {}/{}",
                    cur_str(round_cur(info.avg_dmg_clear_round, 0)),
                    info.damage_out
                ));
            } else if info.avg_dmg_clear_round > zero {
                t.push_str(&format!(
                    ", Dmg In: {}",
                    cur_str(round_cur(info.avg_dmg_clear_round, 0))
                ));
            } else if info.damage_out > 0 && shows_out {
                t.push_str(&format!(", Dmg Out: {}", info.damage_out));
            }
            let r = self.add_row(LocationKind::Lair, &loc, Some("Room".into()));
            self.add_cell(r, t, Some(room_key));
        } else {
            self.add_row(
                LocationKind::Lair,
                &format!("{}: {name}{s_percent}{df}", loc.trim_matches(' ')),
                Some(room_key),
            );
        }
        Ok(())
    }

    /// Port of modMain `AddRoomCmdLocationRow`.
    fn add_room_cmd_row(
        &mut self,
        src: &RoomCmdSource,
        two_columns: bool,
        percent_column: bool,
        footer: &str,
    ) {
        let hide = self.opt.hide_record_numbers;
        let loc = if src.action {
            "Room action: ".to_string()
        } else {
            format!("Room cmd {}: ", src.label)
        };
        let mut room = room_name(self.data, &src.first_room, hide);
        if src.room_count > 1 {
            room = format!("{room} +{} more", src.room_count - 1);
        }
        if src.cut {
            room.push_str(" (list cut short)");
        }
        let kind = LocationKind::RoomCmd;
        let mut df = footer.to_string();
        let zero = Currency::from_scaled(0);
        let row = self.add_row(kind, "", None);
        let res: Res<()> = (|| {
            if percent_column {
                let mut p = zero;
                if src.chance > 0.0 {
                    p = cur(src.chance)?;
                }
                if instr_s(1, footer, "%)") > 0 {
                    let pos = instr_s(1, footer, "(");
                    let p2 = cur(extract_numbers(&mid(footer, pos, None)?))?;
                    if p2 > zero {
                        if src.chance < 0.0 {
                            p = p2;
                            df = footer.replace(&format!("({}%)", cur_str(p2)), "");
                        } else if p > zero {
                            p = p.checked_mul(p2)?.checked_div(cur_int(100))?;
                            df = footer.replace(&format!("({}%)", cur_str(p2)), "");
                        }
                    }
                }
                let text = if p > cur_int(1) {
                    format!("{}%", cur_str(round_cur(p, 0)))
                } else if round_cur(p, 2) > zero {
                    format!("{}%", cur_str(round_cur(p, 2)))
                } else if src.chance > 0.0 {
                    "<0.01%".to_string()
                } else if src.chance == 0.0 {
                    "?".to_string()
                } else {
                    String::new()
                };
                let r = &mut self.out.rows[row];
                r.text = text;
                r.tag = Some(cur_str(p));
                r.cells.push(LocationCell {
                    text: format!("{loc}{room}{df}"),
                    tag: Some(src.room_list.clone()),
                });
            } else {
                let pct = if src.chance > 0.0 {
                    format!(" ({}%)", cstr(src.chance))
                } else if src.chance == 0.0 {
                    " (?%)".to_string()
                } else {
                    String::new()
                };
                let r = &mut self.out.rows[row];
                if two_columns {
                    r.text = loc.clone();
                    r.cells.push(LocationCell {
                        text: format!("{room}{pct}{df}"),
                        tag: Some(src.room_list.clone()),
                    });
                    r.tag = Some("Room".into());
                } else {
                    r.text = format!("{loc}{room}{pct}{df}");
                    r.tag = Some(src.room_list.clone());
                }
            }
            Ok(())
        })();
        if let Err(e) = res {
            self.out.errors.push(e.code() as i32);
        }
    }
}

/// The percent cell of a percent-column row: blank unless positive.
fn pct_label(p: Currency) -> String {
    if p > Currency::from_scaled(0) {
        format!("{}%", cur_str(p))
    } else {
        String::new()
    }
}

fn rules_err(e: RulesError) -> Vb6Error {
    match e {
        RulesError::Vb6(5) => Vb6Error::InvalidProcedureCall,
        _ => Vb6Error::Overflow,
    }
}
