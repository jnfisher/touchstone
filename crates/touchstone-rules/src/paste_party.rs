//! Paste party (FAM-25, the frmPasteChar half). Card P3-41.
//!
//! MME's `ParsePasteParty` reads the pasted text and writes straight into the dialog's boxes;
//! `CalculateAverageParty` then averages the boxes. Here the boxes are a plain [`PartyForm`] (index
//! 0 is the average box, 1 to 6 the members) taken in and given back, so what MME leaves alone
//! (heals, damage, swings, the attack-last option) survives the same way.
//!
//! The two dialogs `ParsePasteParty` can raise become data: the "no matching data" `MsgBox`
//! answers its default (No) unless [`PasteEnv::no_data_continue`], each member's attack `InputBox`
//! is answered from [`PasteInput::attack_answers`], and every prompt and message comes back in
//! [`PasteOutcome`]. A VB6 error stops the procedure where it happened and the `out:` tail still
//! recounts the averages, as MME's handler does.
//!
//! Stock only. Weapons that cast are fine as long as the supplied text tables are empty, which
//! they are here: the cast text of a weapon is not part of this card.

// Slots are indexed 1 to 6 as in MME; iterator forms would hide that.
#![allow(
    clippy::needless_range_loop,
    clippy::collapsible_match,
    clippy::collapsible_if
)]

use touchstone_model::{AbilitySlot, Class, Item, Race, Spell};
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Vb6Error};

use crate::Profile;
use crate::abilities::{NO_ABILITY, class_has_ability, race_has_ability};
use crate::accuracy::{AccuracyInput, calc_dodge, calculate_accuracy};
use crate::attack::{
    AttackEnv, AttackRequest, CastText, CharacterProfile, EnergyInput, EquippedGlobals,
    calc_energy_used, calc_quick_and_deadly_bonus, calculate_attack,
};
use crate::spell_cast::{CastRequest, SpellCastEnv, SpellCasterProfile, UsableEnv, casting_stat};
use crate::spells::SpellText;
use crate::stats::{
    ManaRegenInput, calc_mana_regen, calc_resting_rate, get_class_combat, get_class_magery,
    get_class_magery_lvl,
};

type Res<T> = Result<T, Vb6Error>;

/// Members 1 to 6 are real; index 0 is the average (or a counter).
pub const SLOTS: usize = 7;

/// The boxes of the paste-party dialog. Text boxes hold text, exactly as VB6 would after
/// assigning a number to `.Text`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PartyForm {
    pub name: [String; SLOTS],
    pub ac: [String; SLOTS],
    pub dr: [String; SLOTS],
    pub mr: [String; SLOTS],
    pub dodge: [String; SLOTS],
    pub hitpoints: [String; SLOTS],
    pub rest_hp: [String; SLOTS],
    pub regen_hp: [String; SLOTS],
    pub accy: [String; SLOTS],
    pub heals: [String; SLOTS],
    pub dmg: [String; SLOTS],
    pub spell_dmg: [String; SLOTS],
    pub swings: [String; SLOTS],
    /// `chkPastePartyAM(i).Value`.
    pub anti_magic: [i16; SLOTS],
    /// `optPastyPartyAtkLast(i).Value` (1 to 6; they exclude each other).
    pub atk_last: [bool; SLOTS],
    /// `optPastyPartyAtkLast(i).Tag`.
    pub atk_last_tag: [String; SLOTS],
    pub am_total: String,
    pub party_total: String,
    /// `bHoldPartyRefresh`: while set, `CalculateAverageParty` does nothing.
    pub hold_refresh: bool,
}

impl PartyForm {
    fn set_atk_last(&mut self, i: usize) {
        for (k, v) in self.atk_last.iter_mut().enumerate() {
            *v = k == i;
        }
    }
}

/// One entry of the class or race combo box on the main window: `List(i)` and `ItemData(i)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListEntry {
    pub name: String,
    pub number: i32,
}

/// Tables, combo contents and globals the procedures read.
pub struct PasteEnv<'a> {
    pub profile: Profile,
    pub items: &'a [Item],
    pub classes: &'a [Class],
    pub races: &'a [Race],
    pub spells: &'a [Spell],
    /// `frmMain.cmbGlobalClass(0)`, in list order (the "Any" entry first).
    pub class_list: &'a [ListEntry],
    pub race_list: &'a [ListEntry],
    /// `bOnlyInGame`.
    pub only_in_game: bool,
    /// `bInvenUse2ndWrist`.
    pub use_second_wrist: bool,
    /// `nNMRVer`.
    pub nmr_ver: f64,
    /// `gAvgLevelMaxAllStats`.
    pub avg_level_max_all_stats: f64,
    /// `frmMain.lblDatVer.Caption`.
    pub dat_ver_caption: &'a str,
    /// The answer to "Continue to party screen anyway?" (MME's default button is No).
    pub no_data_continue: bool,
}

/// The pasted text and the answers to the attack prompts, in the order MME asks them.
pub struct PasteInput<'a> {
    pub text: &'a str,
    /// `None` accepts the prompt's default; past the end of the list the prompt is cancelled.
    pub attack_answers: &'a [Option<String>],
}

/// How `ParsePasteParty` ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PasteStatus {
    /// Fewer than 10 characters: the dialog is cancelled.
    #[default]
    Canceled,
    /// Nothing matched and the "continue anyway" question was answered No.
    NoDataExit,
    /// The procedure ran to its `out:` tail (possibly after an error).
    Done,
}

/// One attack prompt MME would have shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttackPrompt {
    pub member: usize,
    /// The character description the prompt names (name, level, class).
    pub who: String,
    pub default: String,
}

/// MME's per-member locals, which the form never stores (`tCharacter(i)` and friends).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Member {
    pub weapon_num: i32,
    pub weapon_speed: i32,
    pub weapon_str: i32,
    pub accy_worn: f64,
    pub accy_abil: i16,
    pub plus_dodge: i16,
    pub plus_regen: i16,
    pub phys_damage: i32,
    pub swings: f64,
    pub spell_damage: i32,
    pub hp: f64,
    pub hp_regen: f64,
    pub mana_regen: f64,
    pub encum_pct: i16,
    pub accuracy: f64,
    pub level: i32,
    pub class: i32,
    pub race: i32,
    pub combat: i16,
    pub str_: i16,
    pub agi: i16,
    pub cha: i16,
    pub wis: i16,
    pub int: i16,
    pub hea: i16,
    pub spellcasting: i16,
    pub max_mana: f64,
    pub crit: i16,
    pub dodge: i16,
    pub plus_max_damage: i16,
    pub plus_bs_accy: i16,
    pub plus_bs_min_dmg: i16,
    pub plus_bs_max_dmg: i16,
    pub stealth: i16,
    pub ma_skill: [i16; 3],
    pub ma_accy: [i16; 3],
    pub ma_dmg: [i16; 3],
    /// `sEquipLoc(i, 0..19)`.
    pub equip_loc: [String; 20],
    /// `sWorn(i, 0..1)`.
    pub worn: [String; 2],
}

/// Everything `ParsePasteParty` produced besides the form.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PasteOutcome {
    pub status: PasteStatus,
    /// Index 1 to 6 used.
    pub members: [Member; SLOTS],
    pub energy: i16,
    pub combat: i16,
    pub name_count: i16,
    pub find_atk_last: String,
    pub prompts: Vec<AttackPrompt>,
    /// Every `MsgBox` text, in order.
    pub messages: Vec<String>,
    /// A VB6 error reached the handler.
    pub error: bool,
}

// ---------------------------------------------------------------------------------------------
// Text helpers

/// Port of modMMudFunc TestPasteChar: the characters the paste walker keeps.
#[must_use]
pub fn test_paste_char(c: u16) -> bool {
    let Ok(b) = u8::try_from(c) else {
        return false;
    };
    let b = b.to_ascii_lowercase();
    matches!(b,
        b'a'..=b'z' | b'0'..=b'9'
        | b'(' | b')' | b'-' | b'_' | b',' | b':' | b' ' | b'\'' | b'"' | b'.' | b'`')
}

fn is_ws(c: char) -> bool {
    // The oracle (and so the fixtures) use ASCII classes; see docs/differences.md.
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c')
}

fn skip_ws(t: &[char], mut p: usize) -> usize {
    while p < t.len() && is_ws(t[p]) {
        p += 1;
    }
    p
}

fn digits(t: &[char], p: usize) -> usize {
    let mut q = p;
    while q < t.len() && t[q].is_ascii_digit() {
        q += 1;
    }
    q
}

fn keyword(t: &[char], p: usize, words: &[&str]) -> Option<(usize, String)> {
    for w in words {
        let n = w.chars().count();
        if p + n <= t.len()
            && t[p..p + n]
                .iter()
                .zip(w.chars())
                .all(|(a, b)| a.eq_ignore_ascii_case(&b))
        {
            return Some((p + n, t[p..p + n].iter().collect()));
        }
    }
    None
}

/// After `kw:`, the `\s*\*?\s*` the numeric alternatives share.
fn after_star(t: &[char], p: usize) -> usize {
    let mut p = skip_ws(t, p);
    if p < t.len() && t[p] == '*' {
        p += 1;
    }
    skip_ws(t, p)
}

const WORDS_PAIR: [&str; 4] = ["Armour Class", "Hits", "Mana", "Encumbrance"];
const WORDS_NUM: [&str; 9] = [
    "MagicRes",
    "Level",
    "Spellcasting",
    "Strength",
    "Agility",
    "Willpower",
    "Charm",
    "Intellect",
    "Health",
];
const WORDS_TEXT: [&str; 3] = ["Name", "Race", "Class"];

/// One match of the party pattern at `p`: the non-empty groups, keyword first.
fn match_at(t: &[char], p: usize) -> Option<(usize, Vec<String>)> {
    let sub = |a: usize, b: usize| -> String { t[a..b].iter().collect() };
    if let Some((q, kw)) = keyword(t, p, &WORDS_PAIR)
        && q < t.len()
        && t[q] == ':'
    {
        let mut r = after_star(t, q + 1);
        let a0 = r;
        if r < t.len() && t[r] == '-' {
            r += 1;
        }
        let d = digits(t, r);
        if d > r && d < t.len() && t[d] == '/' {
            let e = digits(t, d + 1);
            if e > d + 1 {
                return Some((e, vec![kw, sub(a0, d), sub(d + 1, e)]));
            }
        }
    }
    if let Some((q, kw)) = keyword(t, p, &WORDS_NUM)
        && q < t.len()
        && t[q] == ':'
    {
        let r = after_star(t, q + 1);
        let d = digits(t, r);
        if d > r {
            return Some((d, vec![kw, sub(r, d)]));
        }
    }
    if let Some((q, kw)) = keyword(t, p, &WORDS_TEXT)
        && q < t.len()
        && t[q] == ':'
    {
        let r = skip_ws(t, q + 1);
        let mut e = r;
        let word = |s: usize| {
            let mut x = s;
            while x < t.len() && !is_ws(t[x]) && t[x] != ':' {
                x += 1;
            }
            x
        };
        e = word(e);
        if e > r {
            // One optional extra word, unless that word is itself a label (followed by a colon).
            if e < t.len() && is_ws(t[e]) {
                let w_end = word(e + 1);
                let label = w_end < t.len() && t[w_end] == ':';
                if !label && w_end > e + 1 {
                    e = w_end;
                }
            }
            return Some((e, vec![kw, sub(r, e)]));
        }
    }
    None
}

/// `RegExpFindv2(text, pattern, MatchCase:=False, Multiline:=True)` for the party pattern.
fn find_party_matches(text: &str) -> Vec<Vec<String>> {
    let t: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut p = 0;
    while p < t.len() {
        if let Some((end, subs)) = match_at(&t, p) {
            out.push(subs);
            p = end.max(p + 1);
        } else {
            p += 1;
        }
    }
    out
}

/// String to Integer the way an assignment converts the captured digits.
fn to_i16(s: &str) -> Res<i16> {
    let v = parse_int(s)?;
    i16::try_from(v).map_err(|_| Vb6Error::Overflow)
}

fn to_i32(s: &str) -> Res<i32> {
    let v = parse_int(s)?;
    i32::try_from(v).map_err(|_| Vb6Error::Overflow)
}

fn parse_int(s: &str) -> Res<i128> {
    let s = s.trim_matches(' ');
    let (neg, body) = match s.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, s),
    };
    if body.is_empty() || !body.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Vb6Error::TypeMismatch);
    }
    let body = body.trim_start_matches('0');
    if body.len() > 20 {
        return Err(Vb6Error::Overflow);
    }
    let v: i128 = if body.is_empty() {
        0
    } else {
        body.parse().map_err(|_| Vb6Error::Overflow)?
    };
    Ok(if neg { -v } else { v })
}

fn fits_i16(v: i64) -> Res<i16> {
    i16::try_from(v).map_err(|_| Vb6Error::Overflow)
}

fn trim(s: &str) -> &str {
    s.trim_matches(' ')
}

/// A slot counter with its values, as MME keeps them in element 0 and 1 to 6 of one array.
#[derive(Debug, Clone, Default)]
struct Slots<T: Default + Clone> {
    n: usize,
    v: [T; SLOTS],
}

impl<T: Default + Clone> Slots<T> {
    fn full(&self) -> bool {
        self.n >= 6
    }
    fn push(&mut self, x: T) {
        self.v[self.n + 1] = x;
        self.n += 1;
    }
    fn sync(&mut self, to: usize) {
        if self.n < to {
            self.n = to;
        }
    }
}

struct NoText;

impl CastText for NoText {
    fn spell_name(&self, _: i32) -> String {
        String::new()
    }
    fn spell_eq(&self, _: i32, _: i16) -> String {
        String::new()
    }
}

impl SpellText for NoText {
    fn ability_stats(&self, _: i16, _: i16, _: bool, _: bool) -> String {
        String::new()
    }
    fn spell_name(&self, _: i32) -> String {
        String::new()
    }
    fn monster_name(&self, _: i32) -> String {
        String::new()
    }
}

fn slots_of<'a>(
    rows: impl Iterator<Item = (i32, &'a [AbilitySlot])>,
    number: i32,
) -> Option<&'a [AbilitySlot]> {
    let mut found = None;
    for (n, s) in rows {
        if n == number {
            found = Some(s);
            break;
        }
    }
    found
}

fn class_ability(env: &PasteEnv, class: i32, ability: i16) -> i32 {
    let s = slots_of(
        env.classes.iter().map(|c| (c.number, &c.abilities[..])),
        class,
    );
    class_has_ability(env.profile, s, class, ability)
}

fn race_ability(env: &PasteEnv, race: i32, ability: i16) -> i32 {
    let s = slots_of(env.races.iter().map(|r| (r.number, &r.abilities[..])), race);
    race_has_ability(env.profile, s, race, ability)
}

fn cur(v: i64) -> Currency {
    Currency::from_scaled(v * 10_000)
}

fn text_of(x: f64) -> String {
    vb::cstr(x)
}

// ---------------------------------------------------------------------------------------------
// ParsePasteParty

/// Port of frmPasteChar `ParsePasteParty`: reads `input.text`, fills `form` and returns the
/// per-member values MME keeps in locals.
#[must_use]
pub fn parse_paste_party(env: &PasteEnv, input: &PasteInput, form: &mut PartyForm) -> PasteOutcome {
    let mut out = PasteOutcome::default();
    let units: Vec<u16> = input.text.encode_utf16().collect();
    if units.len() < 10 {
        out.status = PasteStatus::Canceled;
        return out;
    }
    let matches = find_party_matches(input.text);
    if matches.is_empty() {
        if vb::val(&form.party_total) == 0.0 {
            out.messages
                .push("No matching data pasted. Continue to party screen anyway?".to_string());
            if !env.no_data_continue {
                out.status = PasteStatus::NoDataExit;
                return out;
            }
        } else {
            out.messages
                .push("No data, returning to previous screen.".to_string());
        }
        out.status = PasteStatus::Done;
        finish(form);
        return out;
    }
    out.status = PasteStatus::Done;
    let mut st = State::default();
    if run(env, input, &units, &matches, form, &mut out, &mut st).is_err() {
        out.error = true;
    }
    out.energy = st.energy;
    out.combat = st.combat;
    out.name_count = st.name_count;
    out.find_atk_last = st.find_atk_last.clone();
    finish(form);
    out
}

/// The `out:` tail: release the hold and recount the averages.
fn finish(form: &mut PartyForm) {
    form.hold_refresh = false;
    calculate_average_party(form, 0);
}

#[derive(Default)]
struct State {
    energy: i16,
    combat: i16,
    name_count: i16,
    find_atk_last: String,
}

#[derive(Default)]
struct Parsed {
    name: Slots<String>,
    class_name: Slots<String>,
    race_name: Slots<String>,
    mr: Slots<i16>,
    level: Slots<i16>,
    agility: Slots<i16>,
    strength: Slots<i16>,
    intellect: Slots<i16>,
    health: Slots<i16>,
    charm: Slots<i16>,
    willpower: Slots<i16>,
    spellcasting: Slots<i16>,
    hit_points: Slots<i32>,
    max_mana: Slots<i16>,
    ac: Slots<i16>,
    dr: Slots<i16>,
    cur_enc: Slots<i32>,
    max_enc: Slots<i32>,
}

impl Parsed {
    fn sync_all(&mut self, to: usize) {
        self.class_name.sync(to);
        self.race_name.sync(to);
        self.mr.sync(to);
        self.level.sync(to);
        self.agility.sync(to);
        self.strength.sync(to);
        self.intellect.sync(to);
        self.health.sync(to);
        self.charm.sync(to);
        self.willpower.sync(to);
        self.spellcasting.sync(to);
        self.hit_points.sync(to);
        self.max_mana.sync(to);
        self.ac.sync(to);
        self.dr.sync(to);
        self.cur_enc.sync(to);
        self.max_enc.sync(to);
    }

    fn collect(&mut self, subs: &[String]) -> Res<()> {
        let sub = |i: usize| trim(subs.get(i).map_or("", String::as_str)).to_string();
        macro_rules! num16 {
            ($f:ident) => {{
                if !self.$f.full() {
                    let v = to_i16(&sub(1))?;
                    self.$f.push(v);
                }
            }};
        }
        match subs[0].as_str() {
            "Name" => {
                if self.name.full() {
                    return Ok(());
                }
                self.name.push(sub(1));
                let sync = self.name.n - 1;
                self.sync_all(sync);
            }
            "Class" => {
                if !self.class_name.full() {
                    self.class_name.push(sub(1));
                }
            }
            "Race" => {
                if !self.race_name.full() {
                    self.race_name.push(sub(1));
                }
            }
            "MagicRes" => num16!(mr),
            "Level" => num16!(level),
            "Agility" => num16!(agility),
            "Strength" => num16!(strength),
            "Intellect" => num16!(intellect),
            "Health" => num16!(health),
            "Charm" => num16!(charm),
            "Willpower" => num16!(willpower),
            "Spellcasting" => num16!(spellcasting),
            "Hits" => {
                if !self.hit_points.full() {
                    let v = to_i32(&sub(2))?;
                    self.hit_points.push(v);
                }
            }
            "Mana" => {
                if !self.max_mana.full() {
                    let v = to_i16(&sub(2))?;
                    self.max_mana.push(v);
                }
            }
            "Armour Class" => {
                if self.ac.full() || self.dr.full() {
                    return Ok(());
                }
                let a = to_i16(&sub(1))?;
                self.ac.push(a);
                let d = to_i16(&sub(2))?;
                self.dr.push(d);
            }
            "Encumbrance" => {
                if self.cur_enc.full() || self.max_enc.full() {
                    return Ok(());
                }
                let a = to_i32(&sub(1))?;
                self.cur_enc.push(a);
                let b = to_i32(&sub(2))?;
                self.max_enc.push(b);
            }
            _ => {}
        }
        Ok(())
    }
}

fn contains_lower(s: &str, needle: &str) -> bool {
    s.to_ascii_lowercase().contains(needle)
}

/// Walk the pasted text collecting equipment per character: `sEquipLoc` and `sWorn`.
fn walk_equipment(units: &[u16], members: &mut [Member; SLOTS], st: &mut State) -> Res<()> {
    let len = units.len() as i64;
    let mut x: i16 = 1;
    let mut y: i16 = 1;
    let mut x2: i16 = -1;
    let mut i_char: i16 = 0;
    let mut text = String::new();
    loop {
        let s = vb::add_i16(x, y)?;
        if i64::from(s) > len + 1 {
            break;
        }
        let ch = units[(s - 2) as usize];
        let keep = test_paste_char(ch);
        let mut clear = false;
        if keep {
            let c = char::from(ch as u8);
            if c != ' ' {
                text.push(c);
            }
            if contains_lower(&text, "name:") {
                st.name_count = fits_i16(i64::from(st.name_count) + 1)?;
                clear = true;
            } else if contains_lower(&text, "equippedwith:") || contains_lower(&text, "arecarrying")
            {
                i_char = if st.name_count > 0 {
                    st.name_count
                } else {
                    fits_i16(i64::from(i_char) + 1)?
                };
                clear = true;
            } else {
                match c {
                    ',' => clear = true,
                    '(' => x2 = i16::try_from(text.len()).map_err(|_| Vb6Error::Overflow)?,
                    ')' => {
                        if x2 != -1 {
                            if (1..=6).contains(&i_char) {
                                let before = text[..(x2 - 1) as usize].to_string();
                                let upto = text.len() - 1;
                                let between = text
                                    .get(x2 as usize..upto)
                                    .unwrap_or("")
                                    .to_ascii_uppercase();
                                assign_slot(&mut members[i_char as usize], &between, before);
                            }
                        }
                        clear = true;
                    }
                    _ => {}
                }
            }
        }
        if clear {
            text.clear();
            x = vb::add_i16(x, y)?;
            y = 0;
            x2 = -1;
        }
        y = vb::add_i16(y, 1)?;
    }
    Ok(())
}

fn assign_slot(m: &mut Member, label: &str, name: String) {
    let loc = &mut m.equip_loc;
    match label {
        "HEAD" => loc[0] = name,
        "EARS" => loc[1] = name,
        "EYES" => loc[17] = name,
        "FACE" => loc[18] = name,
        "NECK" => loc[2] = name,
        "BACK" => loc[3] = name,
        "TORSO" => loc[4] = name,
        "ARMS" => loc[5] = name,
        "WRIST" => {
            if loc[6].is_empty() {
                loc[6] = name;
            } else if loc[7].is_empty() {
                loc[7] = name;
            }
        }
        "WAIST" => loc[11] = name,
        "FINGER" => {
            if loc[9].is_empty() {
                loc[9] = name;
            } else if loc[10].is_empty() {
                loc[10] = name;
            }
        }
        "HANDS" => loc[8] = name,
        "LEGS" => loc[12] = name,
        "FEET" => loc[13] = name,
        "WORN" => {
            if m.worn[0].is_empty() {
                m.worn[0] = name;
            } else if m.worn[1].is_empty() {
                m.worn[1] = name;
            }
        }
        "OFF-HAND" => loc[15] = name,
        "WEAPONHAND" | "TWOHANDED" => loc[16] = name,
        _ => {}
    }
}

fn add16(cur_: i16, add: i32) -> Res<i16> {
    fits_i16(i64::from(cur_) + i64::from(add))
}

/// Scan the item table for the pasted equipment and add up what it gives each member.
fn scan_items(env: &PasteEnv, members: &mut [Member; SLOTS]) -> Res<()> {
    'item: for it in env.items {
        if env.only_in_game && !it.in_game {
            continue;
        }
        let name: String = it.name.chars().filter(|c| *c != ' ').collect();
        if trim(&name).is_empty() {
            continue;
        }
        for i in 1..=6 {
            for x in 0..20 {
                let m = &mut members[i];
                if (x == 14 || x == 19) && (name == m.worn[0] || name == m.worn[1]) {
                    if it.worn == 1 {
                        m.equip_loc[19].clone_from(&name);
                    } else if it.worn == 16 {
                        m.equip_loc[14].clone_from(&name);
                    }
                }
                if name != m.equip_loc[x] {
                    continue;
                }
                if x == 7 && !env.use_second_wrist {
                    continue 'item;
                }
                if x == 16 {
                    m.weapon_num = it.number;
                    m.weapon_speed = it.speed;
                    m.weapon_str = it.str_req;
                }
                m.accy_worn += f64::from(it.accy);
                for slot in &it.abilities {
                    if !(slot.ability > 0 && slot.value != 0) {
                        continue;
                    }
                    let v = slot.value;
                    match slot.ability {
                        7 => m.crit = add16(m.crit, v)?,
                        11 => m.plus_max_damage = add16(m.plus_max_damage, v)?,
                        13 => m.plus_bs_accy = add16(m.plus_bs_accy, v)?,
                        14 => m.plus_bs_min_dmg = add16(m.plus_bs_min_dmg, v)?,
                        15 => m.plus_bs_max_dmg = add16(m.plus_bs_max_dmg, v)?,
                        37 => m.ma_skill[0] = add16(m.ma_skill[0], v)?,
                        40 => m.ma_accy[0] = add16(m.ma_accy[0], v)?,
                        34 => m.ma_dmg[0] = add16(m.ma_dmg[0], v)?,
                        38 => m.ma_skill[1] = add16(m.ma_skill[1], v)?,
                        41 => m.ma_accy[1] = add16(m.ma_accy[1], v)?,
                        35 => m.ma_dmg[1] = add16(m.ma_dmg[1], v)?,
                        39 => m.ma_skill[2] = add16(m.ma_skill[2], v)?,
                        42 => m.ma_accy[2] = add16(m.ma_accy[2], v)?,
                        36 => m.ma_dmg[2] = add16(m.ma_dmg[2], v)?,
                        19 => m.stealth = add16(m.stealth, v)?,
                        145 => m.mana_regen += f64::from(v),
                        // MME's second `Case 34` (dodge) can never be reached: the first one wins.
                        123 => m.plus_regen = add16(m.plus_regen, v)?,
                        22 => {
                            if i64::from(v) > i64::from(m.accy_abil) {
                                m.accy_abil = fits_i16(i64::from(v))?;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(())
}

/// Port of modMMudDatabase GetSpellByShort for a 4-letter short code.
fn spell_by_short(env: &PasteEnv, short: &str, class: i32) -> i32 {
    let short = trim(short);
    if short.is_empty() {
        return 0;
    }
    let usable = UsableEnv {
        profile: env.profile,
        spells: env.spells,
        classes: env.classes,
        nmr_ver: env.nmr_ver,
        only_in_game: env.only_in_game,
        disable_kai_autolearn: false,
    };
    for sp in env.spells {
        if trim(sp.short.as_deref().unwrap_or("")) != short {
            continue;
        }
        if class > 0 && !usable.spell_is_usable(sp.number, class, 0, 0, true) {
            continue;
        }
        return sp.number;
    }
    0
}

fn find_class<'a>(env: &PasteEnv<'a>, n: i32) -> Option<&'a Class> {
    env.classes.iter().find(|c| c.number == n)
}

fn run(
    env: &PasteEnv,
    input: &PasteInput,
    units: &[u16],
    matches: &[Vec<String>],
    form: &mut PartyForm,
    out: &mut PasteOutcome,
    st: &mut State,
) -> Res<()> {
    form.hold_refresh = true;
    for i in 0..SLOTS {
        if i > 0 {
            form.name[i].clear();
            form.anti_magic[i] = 0;
        }
        form.ac[i].clear();
        form.dr[i].clear();
        form.mr[i].clear();
        form.dodge[i].clear();
        form.hitpoints[i].clear();
        form.rest_hp[i].clear();
        form.regen_hp[i].clear();
        form.accy[i].clear();
    }
    form.am_total.clear();
    form.party_total.clear();

    let mut p = Parsed::default();
    for subs in matches {
        p.collect(subs)?;
    }
    // Everything below writes into `out.members` as MME writes its locals; a VB error leaves
    // them as they stood.
    walk_equipment(units, &mut out.members, st)?;
    let items_found = (1..=6).any(|i| {
        let m = &out.members[i];
        !m.worn[0].is_empty() || !m.worn[1].is_empty() || m.equip_loc.iter().any(|s| !s.is_empty())
    });
    if items_found {
        scan_items(env, &mut out.members)?;
    }

    for i in 1..=6 {
        if p.name.n >= i {
            form.name[i].clone_from(&p.name.v[i]);
        }
        if p.ac.n >= i {
            form.ac[i] = p.ac.v[i].to_string();
        }
        if p.dr.n >= i {
            form.dr[i] = p.dr.v[i].to_string();
        }
        if p.mr.n >= i {
            form.mr[i] = p.mr.v[i].to_string();
        }
        if p.hit_points.n >= i {
            form.hitpoints[i] = p.hit_points.v[i].to_string();
            out.members[i].hp = f64::from(p.hit_points.v[i]);
        }
        let mut n_class: i16 = 0;
        let mut n_race: i16 = 0;
        if p.class_name.n >= i {
            for e in env.class_list {
                if e.name == p.class_name.v[i] {
                    n_class = fits_i16(i64::from(e.number))?;
                    out.members[i].class = i32::from(n_class);
                }
            }
        }
        if p.race_name.n >= i {
            for e in env.race_list {
                if e.name == p.race_name.v[i] {
                    n_race = fits_i16(i64::from(e.number))?;
                    out.members[i].race = i32::from(n_race);
                }
            }
        }
        let mut plus_dodge: i16 = out.members[i].plus_dodge;
        let mut plus_regen: i16 = out.members[i].plus_regen;
        if n_class > 0 {
            let c = i32::from(n_class);
            if class_ability(env, c, 51) >= 0 {
                form.anti_magic[i] = 1;
            }
            let t = class_ability(env, c, 34);
            if t != NO_ABILITY {
                plus_dodge = add16(plus_dodge, t)?;
                out.members[i].plus_dodge = plus_dodge;
            }
            let t = class_ability(env, c, 123);
            if t != NO_ABILITY {
                plus_regen = add16(plus_regen, t)?;
                out.members[i].plus_regen = plus_regen;
            }
        }
        if n_race > 0 {
            let r = i32::from(n_race);
            if race_ability(env, r, 51) >= 0 {
                form.anti_magic[i] = 1;
            }
            let t = race_ability(env, r, 34);
            if t != NO_ABILITY {
                plus_dodge = add16(plus_dodge, t)?;
                out.members[i].plus_dodge = plus_dodge;
            }
            let t = race_ability(env, r, 123);
            if t != NO_ABILITY {
                plus_regen = add16(plus_regen, t)?;
                out.members[i].plus_regen = plus_regen;
            }
        }
        let level = p.level.v[i];
        let (agi, cha, hea) = (p.agility.v[i], p.charm.v[i], p.health.v[i]);
        let cur_enc = p.cur_enc.v[i];
        let max_enc = p.max_enc.v[i];
        if level > 0 && agi > 0 && cha > 0 {
            let d = calc_dodge(
                env.profile,
                level,
                agi,
                cha,
                f64::from(plus_dodge),
                f64::from(cur_enc),
                f64::from(max_enc),
            );
            form.dodge[i] = d.to_string();
            out.members[i].dodge = vb::cint(vb::val(&form.dodge[i]))?;
        }
        if level > 0 && hea > 0 {
            let (l, h, r) = (i32::from(level), i32::from(hea), i32::from(plus_regen));
            form.regen_hp[i] = calc_resting_rate(env.profile, l, h, r, false).to_string();
            form.rest_hp[i] = calc_resting_rate(env.profile, l, h, r, true).to_string();
            out.members[i].hp_regen = vb::val(&form.regen_hp[i]);
        }
        if max_enc > 0 {
            let pct = vb::round(vb::div(f64::from(cur_enc), f64::from(max_enc))? * 100.0, 0)?;
            out.members[i].encum_pct = vb::cint(pct)?;
        }
        if out.members[i].encum_pct > 100 {
            out.members[i].encum_pct = 100;
        }
        if n_class > 0 && level > 0 {
            let acc = accuracy(
                env,
                out.members[i].clone(),
                n_class,
                level,
                p.strength.v[i],
                agi,
                out.members[i].encum_pct,
            )?;
            form.accy[i] = acc.to_string();
            out.members[i].accuracy = vb::val(&form.accy[i]);
        }
        let m = &mut out.members[i];
        m.level = i32::from(level);
        m.str_ = p.strength.v[i];
        m.int = p.intellect.v[i];
        m.agi = agi;
        m.cha = cha;
        m.wis = p.willpower.v[i];
        m.hea = hea;
        m.spellcasting = p.spellcasting.v[i];
        m.max_mana = f64::from(p.max_mana.v[i]);
    }

    // Keep the "attack last" mark on the same person after the names changed.
    for i in 1..=6 {
        if form.atk_last[i] && !form.atk_last_tag[i].is_empty() {
            if form.atk_last_tag[i] != trim(&form.name[i]) {
                st.find_atk_last.clone_from(&form.atk_last_tag[i]);
            }
            break;
        }
    }
    if !st.find_atk_last.is_empty() {
        for i in 1..=6 {
            if st.find_atk_last == trim(&form.name[i]) {
                form.set_atk_last(i);
                break;
            }
        }
    }

    form.hold_refresh = false;
    calculate_average_party(form, 0);
    form.hold_refresh = true;

    attack_prompts(env, input, form, out, st)
}

/// `CalculateAccuracy(class, level, str, agi, int, cha, accyWorn, accyAbil, encum)`.
fn accuracy(
    env: &PasteEnv,
    m: Member,
    class: i16,
    level: i16,
    strength: i16,
    agility: i16,
    encum_pct: i16,
) -> Res<i32> {
    let input = AccuracyInput {
        class,
        class_row: find_class(env, i32::from(class)),
        level,
        strength,
        agility,
        accuracy_worn: vb::cint(m.accy_worn)?,
        plus_accuracy: m.accy_abil,
        encum_pct,
    };
    Ok(calculate_accuracy(env.profile, &input, "").value)
}

fn attack_prompts(
    env: &PasteEnv,
    input: &PasteInput,
    form: &mut PartyForm,
    out: &mut PasteOutcome,
    st: &mut State,
) -> Res<()> {
    const NONE: i32 = 0;
    const PUNCH: i32 = 1;
    const KICK: i32 = 2;
    const JUMPKICK: i32 = 3;
    const SURPRISE: i32 = 4;
    const NORMAL: i32 = 5;
    const BASH: i32 = 6;
    const SMASH: i32 = 7;

    let text_tables = NoText;
    let equipped = EquippedGlobals::default();
    let attack_env = AttackEnv {
        items: env.items,
        classes: env.classes,
        races: env.races,
        equipped: &equipped,
        cast_text: &text_tables,
        dat_ver_caption: env.dat_ver_caption,
        avg_level_max_all_stats: env.avg_level_max_all_stats,
        accuracy_tag: "",
    };
    let cast_env = SpellCastEnv {
        profile: env.profile,
        spells: env.spells,
        text: &text_tables,
        avg_level_max_all_stats: env.avg_level_max_all_stats,
    };
    let mut asked = 0usize;

    form.hold_refresh = true;
    'member: for i in 1..=6 {
        let mut attack = NONE;
        let mut spell_short = String::new();
        let mut spell_num = 0;
        if out.members[i].weapon_num > 0 || out.members[i].spellcasting > 0 {
            let mut who = trim(&form.name[i]).to_string();
            if who.is_empty() {
                who = format!("Character {i}");
            }
            if out.members[i].level > 0 {
                who = format!("{who} - {}", out.members[i].level);
            }
            if out.members[i].class > 0 {
                let name = find_class(env, out.members[i].class).map_or("", |c| c.name.as_str());
                who = format!("{who} {name}");
            }
            let default = if vb::val(&form.spell_dmg[i]) > 0.0 || vb::val(&form.dmg[i]) > 0.0 {
                ""
            } else {
                "a"
            };
            out.prompts.push(AttackPrompt {
                member: i,
                who,
                default: default.to_string(),
            });
            let answer = match input.attack_answers.get(asked) {
                Some(Some(s)) => s.clone(),
                Some(None) => default.to_string(),
                None => String::new(),
            };
            asked += 1;
            attack = match trim(&answer) {
                "" | "0" => continue 'member,
                "a" | "at" | "att" | "attack" => NORMAL,
                "aa" | "ba" | "bash" => BASH,
                "aaa" | "sm" | "smash" => SMASH,
                "p" | "pu" | "punch" => PUNCH,
                "k" | "ki" | "kick" => KICK,
                "j" | "jk" | "jumpk" | "jumpkick" => JUMPKICK,
                "bs" | "backstab" => SURPRISE,
                other => {
                    if vb::len(other) == 4 {
                        spell_short = other.to_string();
                        NONE
                    } else {
                        continue 'member;
                    }
                }
            };
        }

        let m = &out.members[i];
        if (attack == BASH || attack == SMASH) && m.weapon_num == 0 {
            out.messages.push("No weapon detected.".to_string());
            continue;
        } else if (attack == PUNCH && m.ma_skill[0] == 0)
            || (attack == KICK && m.ma_skill[1] == 0)
            || (attack == JUMPKICK && m.ma_skill[2] == 0)
        {
            out.messages
                .push("Proper MA skill not detected.".to_string());
            continue;
        }
        if vb::len(&spell_short) == 4 {
            if m.spellcasting == 0 {
                out.messages
                    .push("Spellcast rating not detected.".to_string());
            } else {
                spell_num = spell_by_short(env, &spell_short, m.class);
            }
        }

        if attack == NONE && spell_num > 0 {
            let class = out.members[i].class;
            if class > 0 {
                let c = find_class(env, class);
                let m = &out.members[i];
                let regen = calc_mana_regen(
                    env.profile,
                    &ManaRegenInput {
                        level: m.level,
                        intellect: i32::from(m.int),
                        willpower: i32::from(m.wis),
                        charm: i32::from(m.cha),
                        magic_level: get_class_magery_lvl(c),
                        magic_type: get_class_magery(c) as i32,
                        mp_regen: vb::clng(m.mana_regen)?,
                        meditating: false,
                    },
                );
                out.members[i].mana_regen = regen.to_f64();
            }
            let m = &out.members[i];
            let who = SpellCasterProfile {
                spell_dmg_bonus: 0,
                spellcasting: m.spellcasting,
                max_mana: m.max_mana,
                spell_overhead: 0.0,
                mana_regen: m.mana_regen,
                casting_stat: casting_stat(
                    get_class_magery(find_class(env, m.class)),
                    i32::from(m.int),
                    i32::from(m.wis),
                    i32::from(m.cha),
                ),
                class: m.class,
                level: m.level,
            };
            let req = CastRequest {
                spell: spell_num,
                cast_level: m.level,
                ..CastRequest::default()
            };
            out.members[i].spell_damage = cast_env.calculate_spell_cast(&who, &req).n_avg_round_dmg;
        } else if attack != NONE {
            let class = out.members[i].class;
            if class > 0 {
                st.combat = i16::try_from(get_class_combat(find_class(env, class)))
                    .map_err(|_| Vb6Error::Overflow)?;
                out.members[i].combat = st.combat;
            }
            let m = &out.members[i];
            if attack == SURPRISE || attack == SMASH {
                st.energy = 1000;
            } else {
                let e = calc_energy_used(&EnergyInput {
                    combat: cur(i64::from(st.combat)),
                    level: cur(i64::from(m.level)),
                    attack_speed: cur(i64::from(m.weapon_speed)),
                    agility: cur(i64::from(m.agi)),
                    strength: cur(i64::from(m.str_)),
                    encum: m.encum_pct,
                    item_strength: cur(i64::from(m.weapon_str)),
                    speed_adj: cur(0),
                    is_backstab: attack == SURPRISE,
                })?;
                st.energy = vb::cint(e.to_f64())?;
            }
            let bonus = calc_quick_and_deadly_bonus(
                cur(i64::from(m.agi)),
                cur(i64::from(st.energy)),
                m.encum_pct,
            );
            let crit = vb::cint(f64::from(m.crit) + bonus.to_f64())?;
            out.members[i].crit = crit;

            if attack == BASH || attack == SMASH {
                let m = out.members[i].clone();
                let class16 = fits_i16(i64::from(m.class))?;
                let acc = accuracy(
                    env,
                    m.clone(),
                    class16,
                    fits_i16(i64::from(m.level))?,
                    m.str_,
                    m.agi,
                    m.encum_pct,
                )?;
                form.accy[i] = acc.to_string();
                out.members[i].accuracy = vb::val(&form.accy[i]);
            }

            let m = &out.members[i];
            let profile = CharacterProfile {
                is_loaded_character: false,
                level: m.level,
                class: m.class,
                race: m.race,
                combat: m.combat,
                strength: m.str_,
                agility: m.agi,
                intellect: m.int,
                charm: m.cha,
                crit: m.crit,
                plus_max_damage: m.plus_max_damage,
                plus_min_damage: 0,
                plus_bs_accuracy: m.plus_bs_accy,
                plus_bs_min_damage: m.plus_bs_min_dmg,
                plus_bs_max_damage: m.plus_bs_max_dmg,
                ma_plus_skill: m.ma_skill,
                ma_plus_accuracy: m.ma_accy,
                ma_plus_damage: m.ma_dmg,
                stealth: m.stealth,
                class_stealth: false,
                race_stealth: false,
                accuracy: m.accuracy,
                encum_pct: m.encum_pct,
                encum_current: 0,
                encum_max: 0,
                spell_dmg_bonus: 0,
                energy_pct: 0,
            };
            let req = AttackRequest {
                attack_type: attack,
                weapon_number: m.weapon_num,
                ..AttackRequest::default()
            };
            let mut casts = String::new();
            let dmg = calculate_attack(env.profile, &attack_env, &profile, &req, &mut casts);
            out.members[i].phys_damage = dmg.round_total;
            out.members[i].swings = dmg.swings;
        }

        let m = &out.members[i];
        if m.phys_damage > 0 {
            form.dmg[i] = m.phys_damage.to_string();
            form.swings[i] = text_of(m.swings);
        } else if m.spell_damage > 0 {
            form.spell_dmg[i] = m.spell_damage.to_string();
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// CalculateAverageParty

/// What `CalculateAverageParty(nWhat)` recomputes (`PartyCalc`); anything else behaves as AC.
const SECTIONS: usize = 13;

/// Port of frmPasteChar `CalculateAverageParty`: refreshes the average box (index 0) of one
/// column, or all of them for `what` 0, then the party size and the attack-last tags. Does
/// nothing while `form.hold_refresh` is set. A VB6 error (a Long total overflowing) skips to the
/// party-size recount, as MME's handler does.
pub fn calculate_average_party(form: &mut PartyForm, what: i32) {
    if form.hold_refresh {
        return;
    }
    let atk_last = (1..=6).any(|x| form.atk_last[x]);
    let first = match what {
        2 => 1,
        3 => 2,
        4 => 3,
        5 => 4,
        6 => 5,
        7 => 6,
        8 => 7,
        9 => 8,
        10 => 10,
        11 => 9,
        12 => 11,
        13 => 12,
        _ => 0,
    };
    let _ = sections(form, what, first, atk_last);
    recount(form, atk_last);
}

fn avg_weighted(col: &[String; SLOTS], opt: &[bool; SLOTS], atk_last: bool) -> Res<Option<String>> {
    let (mut total, mut count) = (0i32, 0i32);
    for x in 1..=6 {
        if !trim(&col[x]).is_empty() {
            let v = vb::val(&col[x]);
            let extra = if opt[x] { v } else { 0.0 };
            total = vb::clng(f64::from(total) + v + extra)?;
            count += 1;
        }
    }
    if count > 0 {
        let d = f64::from(count + i32::from(atk_last));
        return Ok(Some(text_of(vb::round(vb::div(f64::from(total), d)?, 0)?)));
    }
    Ok(None)
}

fn avg_plain(col: &[String; SLOTS]) -> Res<Option<String>> {
    let (mut total, mut count) = (0i32, 0i32);
    for x in 1..=6 {
        if !trim(&col[x]).is_empty() {
            total = vb::clng(f64::from(total) + vb::val(&col[x]))?;
            count += 1;
        }
    }
    if count > 0 {
        return Ok(Some(text_of(vb::round(
            vb::div(f64::from(total), f64::from(count))?,
            0,
        )?)));
    }
    Ok(None)
}

fn sections(form: &mut PartyForm, what: i32, first: usize, atk_last: bool) -> Res<()> {
    let mut sec = first;
    while sec < SECTIONS {
        match sec {
            0 => {
                if let Some(t) = avg_weighted(&form.ac, &form.atk_last, atk_last)? {
                    form.ac[0] = t;
                }
            }
            1 => {
                if let Some(t) = avg_weighted(&form.dr, &form.atk_last, atk_last)? {
                    form.dr[0] = t;
                }
            }
            2 => {
                if let Some(t) = avg_weighted(&form.mr, &form.atk_last, atk_last)? {
                    form.mr[0] = t;
                }
            }
            3 => {
                if let Some(t) = avg_weighted(&form.dodge, &form.atk_last, atk_last)? {
                    form.dodge[0] = t;
                }
            }
            4 => {
                if let Some(t) = avg_plain(&form.hitpoints)? {
                    form.hitpoints[0] = t;
                }
            }
            5 => {
                if let Some(t) = avg_weighted(&form.regen_hp, &form.atk_last, atk_last)? {
                    form.regen_hp[0] = t;
                }
            }
            6 => {
                if let Some(t) = avg_plain(&form.rest_hp)? {
                    form.rest_hp[0] = t;
                }
            }
            7 => {
                let mut total = 0i32;
                for x in 1..=6 {
                    if !trim(&form.heals[x]).is_empty() {
                        total = vb::clng(f64::from(total) + vb::val(&form.heals[x]))?;
                    }
                }
                if total > 0 {
                    form.heals[0] = total.to_string();
                }
            }
            8 => {
                if let Some(t) = avg_plain(&form.dmg)? {
                    form.dmg[0] = t;
                }
                if what > 0 {
                    sec = 12;
                    continue;
                }
            }
            9 => {
                if let Some(t) = avg_plain(&form.accy)? {
                    form.accy[0] = t;
                }
            }
            10 => {
                let n = (1..=6).filter(|x| form.anti_magic[*x] == 1).count();
                form.am_total = n.to_string();
            }
            11 => {
                if let Some(t) = avg_plain(&form.spell_dmg)? {
                    form.spell_dmg[0] = t;
                }
            }
            12 => {
                let (mut total, mut count) = (0i32, 0i32);
                for x in 1..=6 {
                    let has_dmg = !trim(&form.dmg[x]).is_empty();
                    if !trim(&form.swings[x]).is_empty() && has_dmg {
                        total = vb::clng(f64::from(total) + vb::val(&form.swings[x]))?;
                        count += 1;
                    } else if has_dmg {
                        total = vb::clng(f64::from(total) + 1.0)?;
                        count += 1;
                    }
                }
                if count > 0 {
                    form.swings[0] =
                        text_of(vb::round(vb::div(f64::from(total), f64::from(count))?, 1)?);
                }
            }
            _ => {}
        }
        if what > 0 {
            break;
        }
        sec += 1;
    }
    Ok(())
}

/// The `out:` part of `CalculateAverageParty`: party size and the attack-last tags.
fn recount(form: &mut PartyForm, atk_last: bool) {
    let mut size = 0;
    for x in 1..=6 {
        let any = form.anti_magic[x] == 1
            || [
                &form.ac,
                &form.dr,
                &form.mr,
                &form.dodge,
                &form.hitpoints,
                &form.rest_hp,
                &form.regen_hp,
                &form.heals,
                &form.swings,
                &form.accy,
                &form.spell_dmg,
                &form.dmg,
            ]
            .iter()
            .any(|c| !trim(&c[x]).is_empty());
        if any {
            size += 1;
        }
    }
    form.party_total = size.to_string();
    if atk_last {
        for x in 1..=6 {
            if form.atk_last[x] && !trim(&form.name[x]).is_empty() {
                form.atk_last_tag[x] = trim(&form.name[x]).to_string();
            } else {
                form.atk_last_tag[x].clear();
            }
        }
    }
}
