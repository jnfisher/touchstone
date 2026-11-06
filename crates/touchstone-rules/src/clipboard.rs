//! The texts MME's right-click menus and Char tab copy to the clipboard (FEAT-CTX, card P4-15).
//!
//! MME builds them from control state, so each builder here takes the strings the form's controls
//! would hold. Ports of frmMain `CopyChartoClip` (with the equipment block of `InvenCopytoClipboard`
//! it appends), `CopyMonsterToClipboard`, `CopyShopToClipboard` and modMain `CopyLVLinetoClipboard`.
//!
//! The columns are fixed by `String(n, " ")`, which is VB run-time error 5 for a negative `n`: a
//! name one character too long copies nothing. A builder reports that as [`Clip::error`]; its
//! `text` is what the clipboard holds afterwards (`None`: MME left it alone, which includes a blank
//! text, because `SetClipboardText` ignores one).

use touchstone_vb6::{cstr, fix, val};

use crate::game_text::{instr_ci, trim};

/// VB6 "Invalid procedure call or argument", from a negative `String()` count or `Mid()` length.
pub const ERR_INVALID_CALL: i32 = 5;
/// "Index out of bounds" of a ListView collection (a missing sub-item).
pub const ERR_INDEX_OUT_OF_BOUNDS: i32 = 35600;

/// What a builder leaves behind.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Clip {
    /// The text now on the clipboard; `None` when MME copied nothing.
    pub text: Option<String>,
    /// The VB6 error that ended the procedure (MME's handler shows a dialog), if one did.
    pub error: Option<i32>,
}

impl Clip {
    fn done(text: String) -> Self {
        Self {
            text: set_clipboard_text(text),
            error: None,
        }
    }
    fn failed(error: i32) -> Self {
        Self {
            text: None,
            error: Some(error),
        }
    }
}

type R<T> = Result<T, i32>;

/// `SetClipboardText`: a text with nothing but spaces is not copied.
fn set_clipboard_text(text: String) -> Option<String> {
    (!text.trim_matches(' ').is_empty()).then_some(text)
}

fn len(s: &str) -> i32 {
    i32::try_from(s.chars().count()).unwrap_or(i32::MAX)
}

/// `String(n, " ")`.
fn pad(n: i32) -> R<String> {
    if n < 0 {
        Err(ERR_INVALID_CALL)
    } else {
        Ok(" ".repeat(usize::try_from(n).unwrap_or(0)))
    }
}

/// `String(width - Len(s), " ")`.
fn fill(width: i32, s: &str) -> R<String> {
    pad(width - len(s))
}

/// `Left(s, n)`.
fn left(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// `InStr(1, s, find)` for a case-sensitive search: the 1-based position, 0 when absent.
fn instr(s: &str, find: &str) -> usize {
    s.find(find).map_or(0, |b| s[..b].chars().count() + 1)
}

/// `Mid(s, start, 999)` (start is 1-based and inside the string here).
fn mid_rest(s: &str, start: usize) -> String {
    s.chars().skip(start - 1).collect()
}

/// `Trim(Mid(s, InStr(s, ":") + 1, 999))` when `s` has a colon, else `s`.
fn after_colon(s: &str) -> String {
    match instr(s, ":") {
        0 => s.to_string(),
        p => trim(&mid_rest(s, p + 1)).to_string(),
    }
}

const CRLF: &str = "\r\n";

// ------------------------------------------------------------------------------------------ character

/// The design-time captions of `lblInvenStats(4..46)`, which the Stats line of the equipment block uses.
const STAT_CAPTIONS: [&str; 47] = [
    "",
    "",
    "",
    "",
    "Encumbrance:",
    "HitPoints:",
    "Mana:",
    "Crits:",
    "Dodge:",
    "SpellCast:",
    "Accuracy:",
    "Max Damage:",
    "Hit Magic:",
    "BS Accuracy:",
    "BS Min Damage:",
    "BS Max Damage:",
    "HitPoint Regen:",
    "Mana Regen:",
    "Perception:",
    "Stealth:",
    "Prot. Evil:",
    "Traps:",
    "Picklocks:",
    "Illuminate:",
    "Magic Resist:",
    "Stone Resist:",
    "Water Resist:",
    "Fire Resist:",
    "Cold Resist:",
    "Lightning Res:",
    "Min Damage:",
    "Quickness:",
    "Prot. Good:",
    "Spell Damage:",
    "DMG",
    "DMG",
    "DMG",
    "Skill",
    "Skill",
    "Skill",
    "Accy",
    "ACCY",
    "ACCY",
    "Punch",
    "Kick",
    "JmpKck",
    "Attk:",
];

/// `lblInvenCharStat(i).Visible`: the designer hides these four.
fn stat_visible(i: usize) -> bool {
    !matches!(i, 32 | 43..=45)
}

/// Port of modMain `GetEquipCaption`.
#[must_use]
pub fn equip_caption(slot: usize) -> &'static str {
    match slot {
        0 => "Head",
        1 => "Ears",
        2 => "Neck",
        3 => "Back",
        4 => "Torso",
        5 => "Arms",
        6 | 7 => "Wrist",
        8 => "Hands",
        9 | 10 => "Finger",
        11 => "Waist",
        12 => "Legs",
        13 => "Feet",
        14 | 19 => "Worn",
        15 => "Off-Hand",
        16 => "Weapon Hand",
        17 => "Eyes",
        18 => "Face",
        _ => "",
    }
}

/// The equipment slots (`cmbEquip(0..19)`).
pub const EQUIP_SLOTS: usize = 20;

/// What `CopyChartoClip` reads: the Char tab's controls as strings.
#[derive(Debug, Clone, Default)]
pub struct CharClip {
    /// `txtCharName.Text`.
    pub name: String,
    /// `lblStatCalc.Tag`: creation points left.
    pub cp_tag: String,
    /// `cmbGlobalRace(0).Text` and `cmbGlobalClass(0).Text`.
    pub race: String,
    pub class: String,
    /// `txtGlobalLevel(0).Text`.
    pub level: String,
    /// `CalcExpNeededByRaceClass` for the level, class and race.
    pub exp: f64,
    /// `lblCharMaxHP.Tag`.
    pub max_hp_tag: String,
    /// `lblInvenCharStat(2).Tag` and `(3).Tag`: AC and DR.
    pub ac_tag: String,
    pub dr_tag: String,
    /// `fraChar(2).Enabled` with `lblCharMaxMana.Tag` and `lblCharSC.Tag`.
    pub mana: Option<(String, String)>,
    /// `fraChar(3).Enabled` with `lblCharPicklocks.Tag`.
    pub picklocks: Option<String>,
    /// `txtCharStats(0..5).Tag`: strength, intellect, willpower, agility, health, charm with bless and gear.
    pub stat_tags: [String; 6],
    /// `Val(lblLabelArray(..).Tag)` per stat in the same order: the bonus that marks the stat with a star.
    pub stat_bonus: [f64; 6],
    /// `lblInvenCharStat(0..46).Caption`.
    pub stats: Vec<String>,
    /// `lblEncumLevel(1).Tag`.
    pub enc_tag: String,
    /// `cmbEquip(0..19).Text` where `ListIndex > 0`.
    pub equip: [Option<String>; EQUIP_SLOTS],
}

impl CharClip {
    fn stat(&self, i: usize) -> &str {
        self.stats.get(i).map_or("", String::as_str)
    }
}

fn star(bonus: f64) -> &'static str {
    if bonus == 0.0 { " " } else { "*" }
}

/// Port of frmMain `CopyChartoClip`. `stats_only` is its `bStatsOnly`; otherwise the equipment block of
/// `InvenCopytoClipboard(True, True)` follows. If that block fails, the character lines stay on the
/// clipboard, as in MME.
#[must_use]
pub fn char_to_clip(c: &CharClip, stats_only: bool) -> Clip {
    let head = match char_lines(c) {
        Ok(s) => s,
        Err(e) => return Clip::failed(e),
    };
    let first = set_clipboard_text(head.clone());
    if stats_only {
        return Clip {
            text: first,
            error: None,
        };
    }
    match equipment_block(c) {
        Ok(block) => {
            if block.is_empty() {
                return Clip {
                    text: first,
                    error: None,
                };
            }
            let on_clipboard = first.unwrap_or_default();
            Clip::done(format!("{on_clipboard}{CRLF}{CRLF}{block}"))
        }
        Err(e) => Clip {
            text: first,
            error: Some(e),
        },
    }
}

fn char_lines(c: &CharClip) -> R<String> {
    let mut s = String::new();
    // line 1
    s += &format!("Name: {}{}", c.name, fill(33, &c.name)?);
    s += &format!("Lives/CP:{}9/{}", fill(7, &c.cp_tag)?, c.cp_tag);
    // line 2
    let exp = cstr(c.exp);
    s += &format!("{CRLF}Race: {}{}", c.race, fill(12, &c.race)?);
    s += &format!("Exp: {exp}{}Perception:      0", fill(16, &exp)?);
    // line 3
    s += &format!("{CRLF}Class: {}{}", c.class, fill(11, &c.class)?);
    s += &format!(
        "Level: {}{}Stealth:         0",
        c.level,
        fill(14, &c.level)?
    );
    // line 4
    let ac_dr = format!("{}/{}", c.ac_tag, c.dr_tag);
    s += &format!("{CRLF}Hits: {}{}", c.max_hp_tag, fill(12, &c.max_hp_tag)?);
    s += &format!(
        "Armour Class: {ac_dr}{}Thievery:        0",
        fill(7, &ac_dr)?
    );
    // line 5
    if let Some((mana, sc)) = &c.mana {
        let both = format!("{mana}-{mana}");
        s += &format!("{CRLF}Mana: {both}{}", fill(12, &both)?);
        s += &format!("Spellcasting: {sc}{}Traps:           0", fill(7, sc)?);
    } else {
        s += &format!("{CRLF}{}Traps:           0", pad(39)?);
    }
    // line 6
    if let Some(picks) = &c.picklocks {
        s += &format!("{CRLF}{}Picklocks:{}{picks}", pad(39)?, fill(8, picks)?);
    } else {
        s += &format!("{CRLF}{}Picklocks:       0", pad(39)?);
    }
    // lines 7 to 9: stat order is strength, intellect, willpower, agility, health, charm
    let t = &c.stat_tags;
    let b = &c.stat_bonus;
    s += &format!("{CRLF}Strength: {}{}{}", star(b[0]), t[0], fill(7, &t[0])?);
    s += &format!("Agility:{}{}{}", star(b[3]), t[3], fill(12, &t[3])?);
    s += "Tracking:        0";
    s += &format!("{CRLF}Intellect:{}{}{}", star(b[1]), t[1], fill(7, &t[1])?);
    s += &format!("Health: {}{}{}", star(b[4]), t[4], fill(12, &t[4])?);
    s += "Martial Arts:    0";
    s += &format!("{CRLF}Willpower:{}{}{}", star(b[2]), t[2], fill(7, &t[2])?);
    s += &format!("Charm:  {}{}{}", star(b[5]), t[5], fill(12, &t[5])?);
    s += "MagicRes:        0";
    Ok(s)
}

/// The text `InvenCopytoClipboard(bAppend, bNoCharStats)` adds: armour class, encumbrance, the worn items
/// and the stat list. Empty when there would be nothing, but the encumbrance line is always there.
fn equipment_block(c: &CharClip) -> R<String> {
    let mut s = String::new();
    if val(c.stat(2)) != 0.0 || val(c.stat(3)) != 0.0 {
        s += &format!("Armour Class: {}/{}{CRLF}", c.stat(2), c.stat(3));
    }
    s += &format!(
        "Encumberance: {}/{} {}{CRLF}{CRLF}They are equipped with:{CRLF}{CRLF}",
        c.stat(0),
        c.stat(1),
        c.enc_tag
    );
    // the off-hand and weapon slots come last
    let order = (0..EQUIP_SLOTS)
        .filter(|x| !matches!(x, 15 | 16))
        .chain([15, 16]);
    for x in order {
        let Some(text) = &c.equip[x] else { continue };
        // Mid(text, 1, InStr(text, "(") - 2): the item's name without " (number)"
        let at = i32::try_from(instr(text, "(")).unwrap_or(0) - 2;
        if at < 0 {
            return Err(ERR_INVALID_CALL);
        }
        let item = left(text, usize::try_from(at).unwrap_or(0));
        s += &format!("{item}{}({}){CRLF}", fill(31, &item)?, equip_caption(x));
    }
    let mut stats = String::new();
    for x in 4..=46usize {
        if val(c.stat(x)) == 0.0 || !stat_visible(x) {
            continue;
        }
        if !stats.is_empty() {
            stats += ", ";
        }
        if (34..=42).contains(&x) {
            let kind = ["Punch", "Kick", "Jumpkick"][(x - 34) % 3];
            let what = ["DMG", "Skill", "Accy"][(x - 34) / 3];
            stats += &format!("{kind} {what} {}", c.stat(x));
        } else {
            let cap = STAT_CAPTIONS[x];
            match instr(cap, ":") {
                0 => stats += &format!("{cap} {}", c.stat(x)),
                p => stats += &format!("{} {}", left(cap, p - 1), c.stat(x)),
            }
        }
    }
    if !stats.is_empty() {
        s += &format!("{CRLF}Stats: {stats}");
    }
    Ok(s)
}

/// The text for a character's `Val(...)` AC/DR tags: `Fix` of the caption, as `CalcCharacterStats` stores it.
#[must_use]
pub fn fix_tag(caption: &str) -> String {
    cstr(fix(val(caption)))
}

// ------------------------------------------------------------------------------------------ list rows

/// A ListView row: `Text` and the sub-items (`ListSubItems`, 1-based in MME).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LvRow {
    pub text: String,
    pub subs: Vec<String>,
    pub selected: bool,
    /// `CopyLVLinetoClipboard` clicks each row before it reads the detail box and reference list: what those hold
    /// for this row. `None`: the call's shared ones.
    pub detail: Option<String>,
    pub refs: Option<Vec<LvRow>>,
}

impl LvRow {
    #[must_use]
    pub fn new(text: &str, subs: &[&str]) -> Self {
        Self {
            text: text.to_string(),
            subs: subs.iter().map(ToString::to_string).collect(),
            ..Self::default()
        }
    }

    /// `ListItem.SubItems(i)`: VB error 35600 where there is no such sub-item.
    fn sub(&self, i: usize) -> R<&str> {
        i.checked_sub(1)
            .and_then(|k| self.subs.get(k))
            .map(String::as_str)
            .ok_or(ERR_INDEX_OUT_OF_BOUNDS)
    }
}

/// A ColumnHeader. MME hides a column by giving it a width of 2 or less.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LvColumn {
    pub text: String,
    pub width: i32,
}

// ------------------------------------------------------------------------------------------ monster

/// Port of frmMain `CopyMonsterToClipboard` over the monster pane's lines (`lvMonsterDetail`): each line's
/// label and sub-items.
#[must_use]
pub fn monster_to_clip(lines: &[LvRow], name_only: bool) -> Clip {
    match monster_text(lines, name_only) {
        Ok(s) => Clip::done(s),
        Err(e) => Clip::failed(e),
    }
}

fn monster_text(lines: &[LvRow], name_only: bool) -> R<String> {
    let mut s = String::new();
    for row in lines {
        if name_only {
            if instr_ci(&row.text, "Name") > 0 {
                s = row.sub(1)?.to_string();
                if instr(&s, "(") > 0 {
                    s = trim(&left(&s, instr(&s, "(") - 1)).to_string();
                }
                break;
            }
        } else {
            s += &row.text;
            if len(&row.text) <= 16 {
                s += &fill(17, &row.text)?;
            }
            s += row.sub(1)?;
            s += CRLF;
        }
    }
    Ok(s)
}

// ------------------------------------------------------------------------------------------ shop

/// What `CopyShopToClipboard` reads.
#[derive(Debug, Clone, Default)]
pub struct ShopClip {
    /// `lvShops.SelectedItem`; `None` copies nothing.
    pub selected: Option<LvRow>,
    /// `lvShops.ColumnHeaders` texts ("#", "Name", "Type").
    pub shop_columns: Vec<String>,
    /// `txtShopDetail.Text`.
    pub detail: String,
    /// `lvShopLoc.ColumnHeaders(1).Text` and the texts of its rows.
    pub loc_header: String,
    pub locs: Vec<String>,
    /// `lvShopDetail`: two columns for a trainer, five for a store.
    pub detail_columns: Vec<String>,
    pub detail_rows: Vec<LvRow>,
}

/// `AutoAppend(s, add, vbTab)`.
fn auto_append_tab(s: String, add: &str) -> String {
    if s.is_empty() {
        add.to_string()
    } else if !trim(add).is_empty() {
        format!("{s}\t{add}")
    } else {
        s
    }
}

/// Port of frmMain `CopyShopToClipboard`.
#[must_use]
pub fn shop_to_clip(c: &ShopClip, name_only: bool) -> Clip {
    match shop_text(c, name_only) {
        Ok(Some(s)) => Clip::done(s),
        Ok(None) => Clip::default(),
        Err(e) => Clip::failed(e),
    }
}

fn shop_text(c: &ShopClip, name_only: bool) -> R<Option<String>> {
    let Some(sel) = &c.selected else {
        return Ok(None);
    };
    if name_only {
        return Ok(Some(sel.sub(1)?.to_string()));
    }
    let mut s = String::new();
    for (x, head) in c.shop_columns.iter().enumerate() {
        s += &format!("{head}: ");
        match x {
            0 => s += &format!("{}, ", sel.text),
            1 => s += &format!("{}{CRLF}", sel.sub(x)?),
            _ => s += sel.sub(x)?,
        }
    }
    s += &format!(" -- {}", c.detail);
    s += &format!("{CRLF}{}: ", c.loc_header);
    s += &c.locs.join(", ");
    s += &format!("{CRLF}{CRLF}");

    let wide = c.detail_columns.len() > 2;
    for (k, head) in c.detail_columns.iter().enumerate() {
        let x = k + 1;
        if x == 2 && wide {
            s += head;
            s += &fill(30, head)?;
        } else if wide {
            s += head;
            if len(head) < 7 {
                s += &fill(7, head)?;
            }
        } else if x == 1 {
            s += head;
        } else {
            s = auto_append_tab(s, head);
        }
    }
    s += CRLF;
    for row in &c.detail_rows {
        s += &row.text;
        if wide && len(&row.text) <= 6 {
            s += &fill(7, &row.text)?;
        }
        for (k, cell) in row.subs.iter().enumerate() {
            if k == 0 && wide {
                s += &left(cell, 29);
                s += &fill(30, cell)?;
            } else if wide {
                s += cell;
                if len(cell) < 7 {
                    s += &fill(7, cell)?;
                }
            } else {
                s = auto_append_tab(s, cell);
            }
        }
        s += CRLF;
    }
    Ok(Some(s))
}

// ------------------------------------------------------------------------------------------ list lines

/// What `CopyLVLinetoClipboard` reads.
#[derive(Debug, Clone, Default)]
pub struct LinesClip {
    /// `lv.name`, which decides how the name-only copy reads a reference list.
    pub list: String,
    pub columns: Vec<LvColumn>,
    pub rows: Vec<LvRow>,
    /// `DetailTB.Text` and `LocationLV` as the call gave them; a row's own override these.
    pub detail: Option<String>,
    pub locations: Option<Vec<LvRow>>,
    /// `nExcludeColumn` (-1: none).
    pub exclude_column: i32,
}

/// Port of modMain `CopyLVLinetoClipboard`: the selected rows, at most 100, as `Column: value` lists
/// (or just the names), with the row's detail text and references after each.
#[must_use]
pub fn lines_to_clip(c: &LinesClip, name_only: bool) -> Clip {
    match lines_text(c, name_only) {
        Ok(s) => Clip::done(s),
        Err(e) => Clip::failed(e),
    }
}

fn lines_text(c: &LinesClip, name_only: bool) -> R<String> {
    let mut s = String::new();
    if c.rows.is_empty() {
        return Ok(s);
    }
    let loc_list = c.list.ends_with("Loc");
    let name_lists = matches!(
        c.list.as_str(),
        "lvMapLoc" | "lvSpellLoc" | "lvShopLoc" | "lvSpellCompareLoc"
    );
    let mut count = 1;
    for row in c.rows.iter().filter(|r| r.selected) {
        if count > 100 {
            break;
        }
        if count > 1 {
            s += if name_only { ", " } else { "\r\n\r\n" };
        }
        for (x, col) in c.columns.iter().enumerate() {
            if i64::try_from(x).ok() == Some(i64::from(c.exclude_column)) {
                continue;
            }
            if name_only {
                if name_lists && x == 0 {
                    s += &if instr(&row.text, ":") > 0 {
                        after_colon(&row.text)
                    } else {
                        row.text.clone()
                    };
                } else if col.text == "Name" {
                    s += if x == 0 { &row.text } else { row.sub(x)? };
                } else if loc_list && !row.subs.is_empty() {
                    let cell = row.sub(x)?;
                    if left(cell, 9) == "Room cmd " || left(cell, 12) == "Room action:" {
                        s += cell;
                    } else {
                        s += &after_colon(cell);
                    }
                }
            } else if col.width > 2 {
                if x != 0 {
                    s += ", ";
                }
                s += &format!("{}: ", col.text);
                s += if x == 0 { &row.text } else { row.sub(x)? };
            }
        }
        if !name_only {
            let detail = row.detail.as_deref().or(c.detail.as_deref());
            if let Some(d) = detail.filter(|d| !d.is_empty()) {
                s += &format!(
                    "{CRLF}>> {}",
                    d.replace("\r\nCompared to", ">> Compared to")
                );
            }
            let refs = row.refs.as_deref().or(c.locations.as_deref());
            if let Some(refs) = refs.filter(|r| !r.is_empty()) {
                let many = refs.len() > 5;
                s += &if many {
                    format!("{CRLF}References--{CRLF}")
                } else {
                    format!("{CRLF}>> Refs: ")
                };
                for (k, r) in refs.iter().enumerate() {
                    if k > 0 {
                        s += if many { CRLF } else { ", " };
                    }
                    s += &r.text;
                    if let Some(first) = r.subs.first() {
                        if !trim(&r.text).is_empty() {
                            s += ": ";
                        }
                        s += first;
                    }
                }
            }
        }
        count += 1;
    }
    Ok(s)
}
