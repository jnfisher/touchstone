//! Paste character (FAM-25, character half). Card P3-40.
//!
//! MME's `PasteCharacter` reads a pasted "stat" / "inventory" / "spells" game output and writes
//! straight into Char-tab controls. Here the controls and globals it touches are a [`PasteState`]
//! that the function changes in place, and everything MME would show (message boxes, input boxes,
//! character saves) is a [`PasteEvent`] in order; a [`Prompter`] supplies the answers.
//!
//! Besides `PasteCharacter` this ports `PasteInventoryManager`, `PasteSpells`, `ApplyPastedStat`,
//! `PastedStatIsBuffed`, `TestPasteChar` and `TestAlphaChar`. `PasteParty` only opens the dialog and
//! has no logic; `ParsePasteParty` belongs to the party card.
//!
//! Control events MME relies on are applied where they change state and VB6 raises them (a combo's
//! Click and a text box's Change fire only when the value really changes): `cmbEquip_Click` records
//! the equipped item, `cmbGlobalRace_Click` raises stats below the race minimum and then ends the
//! no-refresh mode, which makes a later class change ask "Reset learned spells?".
//!
//! Positions are characters (1-based like `InStr`). Only the stock engine is ported: the
//! GreaterMUD item stat-bonus block is skipped, so the bonus handed to `ApplyPastedStat` is 0.

use std::collections::VecDeque;

use touchstone_model::{Item, Race, World};
use touchstone_vb6 as vb;

use crate::character::Character;
use crate::game_text::{
    Event, ItemParseResult, ListRow, MatchEnv, instr_ci, item_name_in_parse_result, lcase, len,
    mid, parse_game_text_inventory, populate_item_manager_from_parsed, trim,
};
use crate::gear::{self, EQUIP_SLOTS};
use crate::spell_cast::UsableEnv;
use crate::textblock::extract_value;
use crate::{Profile, RulesError};

type R<T> = Result<T, RulesError>;

fn v6<T>(r: Result<T, vb::Vb6Error>) -> R<T> {
    r.map_err(|e| RulesError::Vb6(e.code() as i32))
}

fn err_number(e: &RulesError) -> u32 {
    match e {
        RulesError::Vb6(n) => *n as u32,
        RulesError::Unsupported(_) => 0,
    }
}

// ---------------------------------------------------------------------------------------------
// Character tests

/// Port of modMMudFunc `TestPasteChar`: letters, digits and the marks `( ) - _ , : ' " . `` and a space.
#[must_use]
pub fn test_paste_char(c: char) -> bool {
    test_alpha_char(c)
        || matches!(
            c,
            '(' | ')' | '-' | '_' | ',' | ':' | ' ' | '\'' | '"' | '.' | '`' | '0'..='9'
        )
}

/// Port of modMMudFunc `TestAlphaChar`: a letter a-z in either case (compared after `LCase`).
#[must_use]
pub fn test_alpha_char(c: char) -> bool {
    let l = lcase(&c.to_string());
    let mut it = l.chars();
    matches!((it.next(), it.next()), (Some('a'..='z'), None))
}

// ---------------------------------------------------------------------------------------------
// Types

/// `vbYes`, `vbNo`, `vbCancel`.
pub const VB_YES: i32 = 6;
pub const VB_NO: i32 = 7;
pub const VB_CANCEL: i32 = 2;

/// Answers to the boxes MME shows. `msgbox` is called for every box, information boxes too
/// (their answer is ignored), in the order MME shows them.
pub trait Prompter {
    /// `MsgBox`: `default` is the button the box defaults to (`vbYes`, `vbNo`, `vbCancel`, 1 for OK).
    fn msgbox(&mut self, text: &str, default: i32) -> i32;
    /// `InputBox`: `default` is the text it opens with; an empty string is Cancel.
    fn inputbox(&mut self, prompt: &str, default: &str) -> String;
}

/// Scripted answers in order; an exhausted list answers with the default (OK pressed).
#[derive(Debug, Clone, Default)]
pub struct Answers {
    pub msgbox: VecDeque<i32>,
    pub inputbox: VecDeque<String>,
}

impl Prompter for Answers {
    fn msgbox(&mut self, _text: &str, default: i32) -> i32 {
        self.msgbox.pop_front().unwrap_or(default)
    }
    fn inputbox(&mut self, _prompt: &str, default: &str) -> String {
        self.inputbox
            .pop_front()
            .unwrap_or_else(|| default.to_string())
    }
}

/// What MME shows or does besides changing controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasteEvent {
    MsgBox(String),
    InputBox {
        prompt: String,
        default: String,
    },
    /// `SaveCharacter(bPromptForFile, file)`; success is assumed.
    Save {
        prompt: bool,
        file: String,
    },
    /// "Unload current character file first?" answered Yes: the host resets the Char tab
    /// (`LoadCharacter` with clear-selections) and the damage caches.
    ClearCharacter,
    /// An error MME's `HandleError` caught in `source` (`PasteCharacter` abandons the paste).
    HandledError {
        source: &'static str,
        number: u32,
    },
}

/// The globals and options the paste reads.
#[derive(Clone, Copy)]
pub struct PasteEnv<'a> {
    pub world: &'a World,
    /// `bOnlyInGame`.
    pub only_in_game: bool,
    /// `bInvenUse2ndWrist`.
    pub second_wrist: bool,
    /// `bHideRecordNumbers`.
    pub hide_record_numbers: bool,
    /// `nNMRVer`.
    pub nmr_ver: f64,
    /// `bDisableKaiAutolearn`.
    pub disable_kai_autolearn: bool,
    /// `Val(txtGlobalMinLVL.Text)`.
    pub min_item_level: i32,
    /// `cmbGlobalAlignment.ListIndex`.
    pub alignment: i32,
    /// `FormIsLoaded("frmMonsterAttackSim")`.
    pub attack_sim_loaded: bool,
    /// `bAutoSave`.
    pub auto_save: bool,
    /// `ReadINI(..., "LastCharFile")`.
    pub last_char_file: &'a str,
    /// `FileExists`.
    pub file_exists: &'a dyn Fn(&str) -> bool,
}

/// What the Char tab holds, changed in place by a paste.
#[derive(Debug, Clone, PartialEq)]
pub struct PasteState {
    /// Name, race, class, level, filter, equipped items (`nEquippedItem`), add-weight and learned
    /// spells. `stats` is refreshed from `stat_text` at the end.
    pub character: Character,
    /// `txtCharStats(0..5).Text`.
    pub stat_text: [String; 6],
    /// `txtCharStats(5).Tag`, the charm the shop prices use.
    pub charm_tag: String,
    /// `Val(lblLabelArray(3).Tag)`: what the charm tag adds to the charm box.
    pub charm_bonus: f64,
    /// Item numbers each `cmbEquip(x)` holds after its "None" entry.
    pub combos: Vec<Vec<i32>>,
    /// `cmbEquip(x).ListIndex` (0 is None).
    pub sel: [usize; EQUIP_SLOTS],
    /// `chkEquipHold(x)`.
    pub hold: [bool; EQUIP_SLOTS],
    /// `chkUnequipMissing`.
    pub unequip_missing: bool,
    /// `lvItemManager`.
    pub rows: Vec<ListRow>,
    /// `bCharLoaded`.
    pub char_loaded: bool,
    /// `bPromptSave`.
    pub prompt_save: bool,
    /// `sSessionLastCharFile`.
    pub session_char_file: String,
    /// `nGlobalRefreshDelay <> 0`: an equipment change outside no-refresh mode starts MME's refresh
    /// timer, and while it runs the stat boxes' Change handler does nothing (the charm tag stays).
    pub refresh_delay: bool,
}

/// What a paste leaves besides the state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PasteOutcome {
    pub events: Vec<PasteEvent>,
    /// `frmMonsterAttackSim.txtUserAC` / `txtUserDR` (set only when `attack_sim_loaded`).
    pub sim_ac: Option<i32>,
    pub sim_dr: Option<i32>,
    /// The Item Manager was empty before an inventory paste: the host sorts it by the Value tag,
    /// descending, and recolours it (presentation, not ported).
    pub sort_after: bool,
}

// ---------------------------------------------------------------------------------------------
// Text helpers

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

/// `InStr(start, s, find)`, binary compare, 1-based; 0 when `start` is past the end.
fn instr_from(s: &str, start: usize, find: &str) -> usize {
    let c = chars(s);
    let f = chars(find);
    if start == 0 || start > c.len() || f.is_empty() || f.len() > c.len() {
        return 0;
    }
    (start..=c.len() + 1 - f.len())
        .find(|&p| c[p - 1..p - 1 + f.len()] == f[..])
        .unwrap_or(0)
}

fn remove_spaces(s: &str) -> String {
    s.chars().filter(|&c| c != ' ').collect()
}

fn auto_append(base: &str, add: &str) -> String {
    if !base.is_empty() {
        if !trim(add).is_empty() {
            format!("{base}, {add}")
        } else {
            base.to_string()
        }
    } else {
        add.to_string()
    }
}

/// Port of frmMain `PastedStatIsBuffed`: is the first `label` followed (after spaces) by `*`?
#[must_use]
pub fn pasted_stat_is_buffed(search: &str, label: &str) -> bool {
    let x = instr_ci(search, label);
    if x == 0 {
        return false;
    }
    let c = chars(search);
    let mut x = x + len(label);
    while x <= c.len() {
        match c[x - 1] {
            ' ' => x += 1,
            '*' => return true,
            _ => return false,
        }
    }
    false
}

// ---------------------------------------------------------------------------------------------
// The scan of the equipment text

/// The equipment slots a paste names: `sEquipLoc(0..19)`, `sWorn(0..1)` and whether the text says
/// "are carrying".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EquipScan {
    pub loc: [String; EQUIP_SLOTS],
    pub worn: [String; 2],
    pub pasted_inven: bool,
}

/// The first half of `PasteCharacter`: spaces are dropped, characters `TestPasteChar` rejects are
/// skipped (a line break does not end an item), and `Name (Slot)` pairs are collected.
#[must_use]
pub fn scan_equipment(search: &str) -> EquipScan {
    let mut out = EquipScan::default();
    let mut text: Vec<char> = Vec::new();
    let mut x2: i64 = -1;
    for ch in search.chars() {
        if !test_paste_char(ch) {
            continue;
        }
        if ch != ' ' {
            text.push(ch);
        }
        let s: String = text.iter().collect();
        let lower = lcase(&s);
        let clear = if lower.contains("equippedwith:") {
            true
        } else if lower.contains("arecarrying") {
            out.pasted_inven = true;
            true
        } else {
            match ch {
                ',' => true,
                '(' => {
                    x2 = text.len() as i64;
                    false
                }
                ')' => {
                    if x2 != -1 {
                        let a = x2 as usize;
                        let name: String = text[..a - 1].iter().collect();
                        let slot: String = text[a..text.len() - 1].iter().collect();
                        assign_slot(&mut out, &slot.to_ascii_uppercase(), name);
                    }
                    true
                }
                _ => false,
            }
        };
        if clear {
            text.clear();
            x2 = -1;
        }
    }
    out
}

fn assign_slot(out: &mut EquipScan, slot: &str, name: String) {
    let pair = |loc: &mut [String; EQUIP_SLOTS], a: usize, b: usize, name: String| {
        if !loc[a].is_empty() {
            if loc[b].is_empty() {
                loc[b] = name;
            }
        } else {
            loc[a] = name;
        }
    };
    match slot {
        "HEAD" => out.loc[0] = name,
        "EARS" => out.loc[1] = name,
        "EYES" => out.loc[17] = name,
        "FACE" => out.loc[18] = name,
        "NECK" => out.loc[2] = name,
        "BACK" => out.loc[3] = name,
        "TORSO" => out.loc[4] = name,
        "ARMS" => out.loc[5] = name,
        "WRIST" => pair(&mut out.loc, 6, 7, name),
        "WAIST" => out.loc[11] = name,
        "FINGER" => pair(&mut out.loc, 9, 10, name),
        "HANDS" => out.loc[8] = name,
        "LEGS" => out.loc[12] = name,
        "FEET" => out.loc[13] = name,
        "WORN" => {
            if !out.worn[0].is_empty() {
                if out.worn[1].is_empty() {
                    out.worn[1] = name;
                }
            } else {
                out.worn[0] = name;
            }
        }
        "OFF-HAND" => out.loc[15] = name,
        "WEAPONHAND" | "TWOHANDED" => out.loc[16] = name,
        _ => {}
    }
}

/// One header field (`Race: x  Exp:`): the text after `label` up to the next marker, or to the end
/// of the line when the marker is missing, too far away or on another line.
fn header_field(search: &str, label: &str, marker: &str, far: usize) -> Option<String> {
    let x = crate::game_text::instr(search, label);
    if x == 0 {
        return None;
    }
    let x = x + len(label);
    let mut y = instr_from(search, x, marker);
    if y > x + far {
        y = 0;
    }
    if y > 0 && mid(search, x, y - x).contains('\n') {
        y = 0;
    }
    if y == 0 {
        y = instr_from(search, x, "\r");
    }
    if y == 0 {
        y = instr_from(search, x, "\n");
    }
    if y > x {
        Some(trim(&mid(search, x, y - x)).to_string())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------------------------
// Controls and events

struct Ctx<'a, 'p> {
    env: &'a PasteEnv<'a>,
    st: &'a mut PasteState,
    out: &'a mut PasteOutcome,
    ask: &'p mut dyn Prompter,
    /// `bDontRefresh`.
    dont_refresh: bool,
}

impl Ctx<'_, '_> {
    fn msg(&mut self, text: &str, default: i32) -> i32 {
        self.out.events.push(PasteEvent::MsgBox(text.to_string()));
        self.ask.msgbox(text, default)
    }

    fn item_data(&self, slot: usize, sel: usize) -> i32 {
        if sel == 0 {
            0
        } else {
            self.st.combos[slot].get(sel - 1).copied().unwrap_or(0)
        }
    }

    /// `cmbEquip_Click`: remembers the selected item.
    fn equip_click(&mut self, slot: usize) {
        let n = self.item_data(slot, self.st.sel[slot]);
        if self.st.character.equipped[slot] != n {
            self.st.character.equipped[slot] = n;
            if !self.dont_refresh {
                self.st.refresh_delay = true;
            }
        }
    }

    fn set_sel(&mut self, slot: usize, sel: usize) {
        if self.st.sel[slot] != sel {
            self.st.sel[slot] = sel;
            self.equip_click(slot);
        }
    }

    /// `cmbGlobalRace_Click` for a race chosen in code.
    fn race_click(&mut self) {
        let Some(number) = self.st.character.race else {
            return;
        };
        let world = self.env.world;
        let Some(race) = world.races.iter().find(|r| r.number == number) else {
            // number 0 is the "Any" entry; an unknown number leaves the stats alone
            return;
        };
        if race.name == "(none)" {
            return;
        }
        self.dont_refresh = true;
        let mins = race_mins(race);
        for (x, min) in mins.iter().enumerate() {
            if vb::val(&self.st.stat_text[x]) < f64::from(*min) {
                set_stat(self.st, x, vb::cstr(f64::from(*min)));
            }
        }
        self.dont_refresh = false;
    }

    /// The two `cmbGlobalClass_Click` calls that setting the class in code raises: the first keeps
    /// the other combo in step, the second asks whether to reset the learned spells.
    fn class_click(&mut self) {
        let class = self.st.character.class.unwrap_or(0);
        if !self.dont_refresh
            && class > 0
            && self.st.character.learned.class > 0
            && i32::from(self.st.character.learned.class) != class
        {
            let q = "Class changed. Reset learned spells?";
            if self.msg(q, VB_NO) == VB_YES {
                self.st.character.clear_learned_spells();
            }
        }
    }
}

/// STR INT WIL AGL HEA CHM minimums of a race.
fn race_mins(r: &Race) -> [i32; 6] {
    [r.m_str, r.m_int, r.m_wil, r.m_agl, r.m_hea, r.m_chm]
}

/// The class combo: "Any" first, then the Classes rows.
fn class_names(world: &World) -> Vec<(String, i32)> {
    std::iter::once(("Any".to_string(), 0))
        .chain(world.classes.iter().map(|c| (c.name.clone(), c.number)))
        .collect()
}

fn race_names(world: &World) -> Vec<(String, i32)> {
    std::iter::once(("Any".to_string(), 0))
        .chain(world.races.iter().map(|r| (r.name.clone(), r.number)))
        .collect()
}

// ---------------------------------------------------------------------------------------------
// ApplyPastedStat

/// `txtCharStats(i).Text = text` and the Change event it raises when the text really changes: the
/// charm tag follows the charm box.
fn set_stat(st: &mut PasteState, i: usize, text: String) {
    if st.stat_text[i] != text {
        st.stat_text[i] = text;
        if i == 5 && !st.refresh_delay {
            st.charm_tag = vb::cstr(vb::val(&st.stat_text[5]) + st.charm_bonus);
        }
    }
}

/// Port of frmMain `ApplyPastedStat`: writes a pasted stat into box `index` less its item bonus.
/// A spell-modified stat (`buffed`) asks for the real value; an empty or non-positive answer skips
/// this and every later prompt (`manual` becomes `"SKIP"`).
#[allow(clippy::too_many_arguments)] // mirrors the VB parameter list
pub fn apply_pasted_stat(
    st: &mut PasteState,
    out: &mut PasteOutcome,
    ask: &mut dyn Prompter,
    index: usize,
    stat_name: &str,
    pasted: i32,
    buffed: bool,
    item_bonus: i32,
    item_sources: &str,
    manual: &mut String,
) {
    if pasted < 1 {
        return;
    }
    let base = (i64::from(pasted) - i64::from(item_bonus)).max(1) as i32;
    if !buffed {
        set_stat(st, index, base.to_string());
        return;
    }
    if manual == "SKIP" {
        return;
    }
    if vb::val(&st.stat_text[index]) == f64::from(base) {
        return;
    }
    let mut prompt =
        format!("Your pasted character has spell-modified {stat_name} ({pasted}).\r\n");
    if item_bonus != 0 && !item_sources.is_empty() {
        prompt.push_str(&format!(
            "\r\nItem bonuses already taken off: {item_sources}\r\n"
        ));
    }
    prompt.push_str(&format!(
        "\r\nEnter your actual {stat_name} or press cancel to skip this and all further stat prompts."
    ));
    let default = base.to_string();
    out.events.push(PasteEvent::InputBox {
        prompt: prompt.clone(),
        default: default.clone(),
    });
    *manual = ask.inputbox(&prompt, &default);
    let v = vb::val(manual);
    if v > 0.0 {
        set_stat(st, index, vb::cstr(v));
    } else {
        *manual = "SKIP".to_string();
    }
}

// ---------------------------------------------------------------------------------------------
// PasteSpells

fn gear_who(env: &PasteEnv, st: &PasteState) -> gear::Character {
    gear::Character {
        level: st.character.level,
        class: st.character.class.unwrap_or(0),
        race: st.character.race.unwrap_or(0),
        alignment: env.alignment,
        min_item_level: env.min_item_level,
    }
}

/// Port of frmMain `PasteSpells`: learns the spells named in a pasted `spells` or `powers` list
/// (each line `level mana short name`), for the selected class. "You have no spells" clears the list.
pub fn paste_spells(env: &PasteEnv, st: &mut PasteState, out: &mut PasteOutcome, search: &str) {
    if len(search) < 10 {
        return;
    }
    let lower = lcase(search);
    if lower.contains("you have no spells") || lower.contains("you have no power") {
        st.character.clear_learned_spells();
        return;
    }
    let c: Vec<char> = search.chars().collect();
    let mut spells: Vec<String> = vec![String::new(); 200];
    let mut cur = 0_usize;
    let mut text = String::new();
    for (i, &ch) in c.iter().enumerate() {
        let mut store = false;
        if ch == '\n' || ch == '\r' {
            store = true;
        } else {
            text.push(ch);
            let l = lcase(&text);
            if l.contains("you have the following spells:")
                || l.contains("you have the following powers:")
                || l.contains("level mana short spell name")
                || l.contains("level kai  short spell name")
            {
                text.clear();
                continue;
            }
            if i + 1 >= c.len() {
                store = true;
            }
        }
        if !store {
            continue;
        }
        if len(&text) > 17 {
            let t = trim(&text).replace("  ", " ");
            let t = t.replace("  ", " ").replace("  ", " ");
            let mut arr: Vec<String> = trim(&t).split(' ').map(String::from).collect();
            if arr.len() >= 4 && vb::val(&arr[0]) > 0.0 && (arr[1] == "0" || vb::val(&arr[1]) > 0.0)
            {
                arr[0].clear();
                arr[1].clear();
                arr[2].clear();
                spells[cur] = trim(trim(&arr.join(" "))).to_string();
                if cur < 199 {
                    cur += 1;
                }
            }
        }
        text.clear();
    }
    if spells[0].is_empty() {
        return;
    }
    let max = cur;
    let class = match st.character.class {
        Some(n) if n > 0 => n,
        _ => 0,
    };
    let usable = UsableEnv {
        profile: Profile::Stock,
        spells: &env.world.spells,
        classes: &env.world.classes,
        nmr_ver: env.nmr_ver,
        only_in_game: env.only_in_game,
        disable_kai_autolearn: env.disable_kai_autolearn,
    };
    let mut n_cur = 0_usize;
    for sp in &env.world.spells {
        let name = trim(sp.name.as_deref().unwrap_or(""));
        if name.is_empty() || trim(sp.short.as_deref().unwrap_or("")).is_empty() {
            continue;
        }
        if !usable.spell_is_usable(sp.number, class, 0, 0, true) {
            continue;
        }
        if !spells[..=max].iter().any(|s| s == name) {
            continue;
        }
        // MME guards the duplicate with `Not InStr(..)`, which is bitwise and always true
        if n_cur == 0 {
            st.character.clear_learned_spells();
        }
        if n_cur > 99 {
            out.events.push(PasteEvent::HandledError {
                source: "",
                number: 9,
            });
            return;
        }
        st.character.learned.spells[n_cur] = sp.number;
        let Some(cls) = st.character.class else {
            // ItemData(-1): nothing selected
            out.events.push(PasteEvent::HandledError {
                source: "",
                number: 381,
            });
            return;
        };
        if sp.number > 0 && cls > 0 && i32::from(st.character.learned.class) != cls {
            st.character.learned.class = cls as i16;
        }
        n_cur += 1;
    }
}

// ---------------------------------------------------------------------------------------------
// PasteInventoryManager

/// Port of frmMain `PasteInventoryManager`: parses a pasted inventory and adds its items to the
/// Item Manager. `extra_only` is the call `PasteCharacter` makes (no questions about equipped items
/// and keys, nothing cleared); otherwise `text` is what the dialog returned.
pub fn paste_inventory_manager(
    env: &PasteEnv,
    st: &mut PasteState,
    out: &mut PasteOutcome,
    ask: &mut dyn Prompter,
    text: &str,
    extra_only: bool,
) {
    let input = if extra_only {
        text.to_string()
    } else {
        trim(text).to_string()
    };
    if len(&input) < 5 {
        return;
    }
    let sort_after = st.rows.is_empty();
    let items = parse_game_text_inventory(&input).items;
    let who = gear_who(env, st);
    let menv = MatchEnv {
        world: env.world,
        only_in_game: env.only_in_game,
        hide_record_numbers: env.hide_record_numbers,
        charm_tag: &st.charm_tag,
        who: &who,
        equipped: &st.character.equipped,
    };
    let mut log: Vec<Event> = Vec::new();
    let mut ask_fn = |_q: &str, default_yes: bool| {
        ask.msgbox(_q, if default_yes { VB_YES } else { VB_NO }) == VB_YES
    };
    populate_item_manager_from_parsed(
        &menv,
        &mut st.rows,
        &items,
        extra_only,
        &mut ask_fn,
        &mut log,
    );
    for e in log {
        out.events.push(match e {
            Event::MsgBox(t) => PasteEvent::MsgBox(t),
            Event::HandledError { source, number } => PasteEvent::HandledError { source, number },
        });
    }
    if !st.rows.is_empty() && sort_after {
        out.sort_after = true;
    }
}

// ---------------------------------------------------------------------------------------------
// PasteCharacter

const PLACEHOLDER: &str = "Paste the commands below into your game to get your stats.\r\nCopy and paste the output here.\r\n\r\npowers\r\nspells\r\ninventory\r\nstat\r\n\r\nor, create a macro: sp^Mi^Mstat^M";

/// `InvenAddEquip`: the combo(s) an item is listed in.
fn inven_add_equip(st: &mut PasteState, item: &Item) {
    let x: i32 = match item.item_type {
        0 => match item.worn {
            1 => 19,
            2 => 0,
            3 => 8,
            4 => 9,
            5 => 13,
            6 => 5,
            7 => 3,
            8 => 2,
            9 => 12,
            10 => 11,
            11 => 4,
            12 => 15,
            13 => 10,
            14 => 6,
            15 => 1,
            16 => 14,
            18 => 17,
            19 => 18,
            _ => -1,
        },
        1 => 16,
        _ => -1,
    };
    match x {
        9 | 10 => {
            st.combos[9].push(item.number);
            st.combos[10].push(item.number);
        }
        6 | 7 => {
            st.combos[6].push(item.number);
            st.combos[7].push(item.number);
        }
        0.. => st.combos[x as usize].push(item.number),
        _ => {}
    }
}

fn current_index(list: &[(String, i32)], number: Option<i32>) -> Option<usize> {
    let n = number?;
    list.iter().position(|(_, num)| *num == n)
}

/// Port of frmMain `PasteCharacter`: reads a pasted stat / inventory / spells text and updates the
/// character, equipment, Item Manager and learned spells. The state is changed in place; a VB6
/// error abandons the paste at that point with what was done so far and a
/// [`PasteEvent::HandledError`].
pub fn paste_character(
    env: &PasteEnv,
    st: &mut PasteState,
    text: &str,
    ask: &mut dyn Prompter,
) -> PasteOutcome {
    let mut out = PasteOutcome::default();
    let res = {
        let mut ctx = Ctx {
            env,
            st,
            out: &mut out,
            ask,
            dont_refresh: false,
        };
        run_paste(&mut ctx, text)
    };
    if let Err(e) = res {
        out.events.push(PasteEvent::HandledError {
            source: "PasteCharacter",
            number: err_number(&e),
        });
    }
    out
}

#[allow(clippy::too_many_lines)] // one VB procedure
fn run_paste(ctx: &mut Ctx, text: &str) -> R<()> {
    let env = *ctx.env;
    let world = env.world;
    if world.items.is_empty() {
        ctx.msg("No item records", 1);
        return Ok(());
    }
    let mut search = trim(text).to_string();
    if search.is_empty() {
        search = PLACEHOLDER.to_string();
    }
    if len(&search) < 10 {
        return Ok(());
    }

    let mut scan = scan_equipment(&search);
    let pasted_inven = scan.pasted_inven;
    let race_name = header_field(&search, "Race: ", "Exp:", 20).unwrap_or_default();
    let class_name = header_field(&search, "Class: ", "Level:", 15).unwrap_or_default();
    let name = header_field(&search, "Name: ", "Lives/CP:", 35).unwrap_or_default();
    let items_found = scan.loc.iter().any(|s| !s.is_empty())
        || !scan.worn[0].is_empty()
        || !scan.worn[1].is_empty();

    let mut spell_only = false;
    if name.is_empty() && class_name.is_empty() && race_name.is_empty() && !items_found {
        if instr_ci(&search, "ou have no spells") > 0 || instr_ci(&search, "ou have no power") > 0 {
            spell_only = true;
        } else if instr_ci(&search, "ou have the following") == 0
            && instr_ci(&search, "short spell nam") == 0
        {
            ctx.msg("No data found in paste. Aborted.", 1);
            return Ok(());
        } else {
            spell_only = true;
        }
    }

    let mut equipped_anom = String::new();
    let mut missing_anom = String::new();
    if !spell_only {
        // save the loaded character first
        if ctx.st.char_loaded {
            let mut file = env.last_char_file.to_string();
            if len(&ctx.st.session_char_file) > 0 {
                file = ctx.st.session_char_file.clone();
            }
            if !(env.file_exists)(&file) {
                file.clear();
                ctx.st.session_char_file.clear();
            }
            if env.auto_save {
                if !file.is_empty() {
                    ctx.out.events.push(PasteEvent::Save {
                        prompt: false,
                        file,
                    });
                } else if ctx.st.prompt_save {
                    ctx.out.events.push(PasteEvent::Save {
                        prompt: true,
                        file: String::new(),
                    });
                }
            } else if ctx.st.prompt_save {
                match ctx.msg("Save character file first?", VB_CANCEL) {
                    VB_YES => {
                        let prompt = file.is_empty();
                        ctx.out.events.push(PasteEvent::Save { prompt, file });
                    }
                    VB_CANCEL => return Ok(()),
                    _ => {}
                }
            }
        }

        let classes = class_names(world);
        let races = race_names(world);
        let cur_class_idx = current_index(&classes, ctx.st.character.class);
        let cur_class_name = cur_class_idx
            .map(|i| classes[i].0.clone())
            .unwrap_or_default();
        if ctx.st.char_loaded
            && !class_name.is_empty()
            && class_name != cur_class_name
            && cur_class_idx.is_some_and(|i| i > 0)
        {
            let q = "It appears that you are pasting a character with a different class. Unload current character file first?";
            match ctx.msg(q, VB_CANCEL) {
                VB_YES => {
                    ctx.st.char_loaded = false;
                    ctx.dont_refresh = true;
                    ctx.out.events.push(PasteEvent::ClearCharacter);
                }
                VB_CANCEL => return Ok(()),
                _ => {}
            }
        }

        ctx.dont_refresh = true;

        if ctx.st.unequip_missing && items_found {
            for x in 0..EQUIP_SLOTS {
                if !ctx.st.hold[x] {
                    ctx.set_sel(x, 0);
                }
            }
        }

        let lvl = extract_value(&search, "Level:");
        if lvl > 0 {
            ctx.st.character.level = lvl;
            ctx.st.character.global_filter = true;
        }
        if !name.is_empty() {
            ctx.st.character.name = name.clone();
        }
        let mut encum = extract_value(&search, "Encumbrance:");

        if !race_name.is_empty() {
            for (x, (nm, num)) in races.iter().enumerate() {
                if *nm == race_name && current_index(&races, ctx.st.character.race) != Some(x) {
                    ctx.st.character.race = Some(*num);
                    ctx.race_click();
                }
            }
        }
        if !class_name.is_empty() {
            for (x, (nm, num)) in classes.iter().enumerate() {
                if *nm == class_name {
                    if current_index(&classes, ctx.st.character.class) != Some(x) {
                        ctx.st.character.class = Some(*num);
                        ctx.class_click();
                    }
                    ctx.st.character.global_filter = true;
                }
            }
        }

        // equip the pasted items
        for item in &world.items {
            if env.only_in_game && !item.in_game {
                continue;
            }
            let s_text = remove_spaces(&item.name);
            'slots: for x in 0..EQUIP_SLOTS {
                if (x == 14 || x == 19) && (s_text == scan.worn[0] || s_text == scan.worn[1]) {
                    if item.worn == 1 {
                        scan.loc[19] = s_text.clone();
                    } else if item.worn == 16 {
                        scan.loc[14] = s_text.clone();
                    }
                }
                if s_text != scan.loc[x] {
                    continue;
                }
                if (x == 14 && item.worn == 1) || (x == 19 && item.worn == 16) {
                    continue;
                }
                if x == 7 && !env.second_wrist {
                    break 'slots;
                }
                let mut tries = 0;
                loop {
                    if let Some(p) = ctx.st.combos[x].iter().position(|&n| n == item.number) {
                        if encum > 0 {
                            encum = v6(vb::sub_i32(encum, item.encum))?;
                        } else {
                            encum = 0;
                        }
                        scan.loc[x].clear();
                        if ctx.st.hold[x] {
                            break 'slots;
                        }
                        ctx.set_sel(x, p + 1);
                        break 'slots;
                    }
                    if tries > 0 {
                        break 'slots;
                    }
                    inven_add_equip(ctx.st, item);
                    tries = 1;
                }
            }
        }

        let pasted_items = if pasted_inven {
            parse_game_text_inventory(&search).items
        } else {
            ItemParseResult::default()
        };

        // stats (the stock path has no item bonus to take back off)
        let mut manual = String::new();
        for (i, (label, nm)) in [
            ("Strength:", "Strength"),
            ("Intellect:", "Intellect"),
            ("Willpower:", "Willpower"),
            ("Agility:", "Agility"),
            ("Health:", "Health"),
            ("Charm:", "Charm"),
        ]
        .iter()
        .enumerate()
        {
            let n = extract_value(&search, label);
            let buffed = pasted_stat_is_buffed(&search, label);
            apply_pasted_stat(
                ctx.st,
                ctx.out,
                ctx.ask,
                i,
                nm,
                n,
                buffed,
                0,
                "",
                &mut manual,
            );
        }

        // weight the pasted encumbrance leaves after the carried items
        if encum > 0 {
            for row in &ctx.st.rows {
                if row.subs.len() >= 5 && instr_ci(&row.subs[1].text, "CARRIED") > 0 {
                    let mut count_this = true;
                    if pasted_inven {
                        count_this = item_name_in_parse_result(
                            &pasted_items,
                            &row.subs[0].text,
                            false,
                            true,
                            true,
                        );
                    }
                    if count_this {
                        let mut y: i16 = 1;
                        if instr_ci(&row.subs[1].text, " x") > 0 {
                            let parts: Vec<&str> = row.subs[1].text.split(" x").collect();
                            if parts.len() >= 2 {
                                y = v6(vb::cint(vb::val(parts[1])))?;
                            }
                        }
                        if y < 1 {
                            y = 1;
                        }
                        encum = v6(vb::clng(
                            f64::from(encum) - vb::val(&row.subs[4].text) * f64::from(y),
                        ))?;
                    }
                }
            }
        }
        if encum > 0 {
            ctx.st.character.add_weight = encum;
            ctx.st.character.use_add_weight = true;
        }

        if pasted_inven && !ctx.st.rows.is_empty() {
            for row in &ctx.st.rows {
                if row.subs.len() >= 2 && instr_ci(&row.subs[1].text, "CARRIED") > 0 {
                    let nm = &row.subs[0].text;
                    if item_name_in_parse_result(&pasted_items, nm, false, true, true) {
                    } else if item_name_in_parse_result(&pasted_items, nm, true, false, false) {
                        equipped_anom = auto_append(&equipped_anom, nm);
                    } else {
                        missing_anom = auto_append(&missing_anom, nm);
                    }
                }
            }
        }
    }

    // spellimport:
    paste_spells(&env, ctx.st, ctx.out, &search);
    if pasted_inven {
        paste_inventory_manager(&env, ctx.st, ctx.out, ctx.ask, &search, true);
    }
    ctx.dont_refresh = false;

    if !equipped_anom.is_empty() || !missing_anom.is_empty() {
        let mut m = String::from("Discrepancy in carried/equipped items detected.\r\n");
        if !missing_anom.is_empty() {
            m.push_str(&format!(
                "\r\nFlagged CARRIED on the Item Manager but NOT in the pasted inventory (their weight was not counted toward encumbrance):\r\n{missing_anom}\r\n"
            ));
        }
        if !equipped_anom.is_empty() {
            m.push_str(&format!(
                "\r\nFlagged CARRIED on the Item Manager but EQUIPPED in the pasted inventory (items both carried and equipped are highlighted in red on the Item Manager):\r\n{equipped_anom}\r\n"
            ));
        }
        m.push_str("\r\nApart from the stat values, nothing was changed automatically -- review these items if this was unexpected.");
        ctx.msg(&m, 1);
    }

    let ac = extract_value(&search, "Armour Class:");
    if ac > 0 && env.attack_sim_loaded {
        ctx.out.sim_ac = Some(ac);
        let digits = vb::cstr(f64::from(ac));
        if len(&digits) <= 4 {
            let key = format!("Armour Class:{}{}/", " ".repeat(4 - len(&digits)), digits);
            let dr = extract_value(&search, &key);
            if dr > 0 {
                ctx.out.sim_dr = Some(dr);
            }
        }
    }

    // the stats live on in the character as whole numbers
    for i in 0..6 {
        if let Ok(v) = vb::clng(vb::val(&ctx.st.stat_text[i])) {
            ctx.st.character.stats[i] = v;
        }
    }
    Ok(())
}
