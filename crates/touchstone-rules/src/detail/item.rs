//! Item detail text and compare text (FAM-20). Cards P3-35 and P3-36.
//!
//! Ports modMain `PullItemDetail` with `CompareArmor` and `CompareWeapons`: what MME writes into an
//! item's detail box (flags, uses, abilities, casts, restrictions, the "Compared to <equipped
//! item>" blocks and the weapon-damage or armour block) and into the item's reference list
//! (Obtained From, References and the rows the item's abilities add).
//!
//! # What comes back
//!
//! [`ItemDetail`] holds the exact text MME puts in the box, the same text as [`DetailRow`]s (kind,
//! label, value) for a UI that wants the pieces, and the reference rows as [`LocationList`] (the rows
//! in the order MME adds them to its ListView). ListView sorting is a skipped feature: the list's
//! `sorts` hold the row counts at each `SortListView` request of `GetLocations`, and
//! [`ItemDetail::sort`] the one `LV_RefreshSort` (or `LV_RefreshSort_ShopsFirst`) request that ends
//! the procedure.
//!
//! # State MME reads
//!
//! The frmMain controls and module globals the procedure reads are [`ItemDetailOptions`]; the tables
//! come through [`ItemDetailEnv`]. The loaded character is `Option<&CharacterState>`: weapons call
//! `PopulateCharacterProfile` with the global filter as `bForceUseChar`, and the surprise attack
//! looks at the character's class and race for stealth.
//!
//! # Rows `GetAbilityStats` and `PullSpellEQ` add
//!
//! Given a list those two procedures add rows ("Spell: x" for learn/remove/give-spell abilities,
//! "Teleport: room", "Summon: monster", "Execute: Textblock n" for the spells an ability casts).
//! The text ports (P3-04, P3-07) behave as MME does with no list, so this module walks the same
//! control flow for rows only ([`RowWalker`]).
//!
//! GreaterMUD and Paramud branches (the aura-item AC/DR line, cast-on-kill, the Ganghouse value) are
//! not ported. A VB6 run-time error ends the procedure where MME's handler does: the text stays
//! empty, the rows built so far remain and no sort is requested; the error numbers are in
//! [`ItemDetail::errors`].

// MME's `For x = 0 To 19` loops index several parallel arrays; the port keeps them as written.
#![allow(clippy::needless_range_loop)]

use std::cell::Cell;

use touchstone_model::{Item, Spell, World};
use touchstone_vb6 as vb;

use crate::Profile;
use crate::ability_text::{AbilityEnv, NameLookup, get_class_name, get_spell_name};
use crate::attack::{self, AttackEnv, AttackRequest, EquippedGlobals};
use crate::character::{AttackTypeMud, CharacterProfile};
use crate::locations::{
    LocationArgs, LocationCell, LocationData, LocationKind, LocationLairs, LocationList,
    LocationOptions, LocationRow, get_locations,
};
use crate::pipeline::CharacterState;
use crate::rows::{armour_type_text, no_character_profile, weapon_type_text, worn_text};
use crate::spells::{MinMaxRequest, PullArgs, get_current_spell_min_max};
use crate::stats;
use crate::textblock::{TextblockData, get_textblock_action};

/// A VB6 or DAO run-time error number.
type Res<T> = Result<T, i32>;

fn code(e: vb::Vb6Error) -> i32 {
    e.code() as i32
}

/// DAO 3021: "No current record" (a field read on a recordset a failed `Seek` left at EOF).
const NO_CURRENT_RECORD: i32 = 3021;
/// 381: "Invalid property array index" (`ItemData` of a combo with no selection).
const BAD_INDEX: i32 = 381;

fn to_i16(v: i32) -> Res<i16> {
    i16::try_from(v).map_err(|_| code(vb::Vb6Error::Overflow))
}

/// The frmMain controls and globals `PullItemDetail` reads.
#[derive(Debug, Clone)]
pub struct ItemDetailOptions {
    /// `frmMain.chkGlobalFilter.Value = 1`.
    pub global_filter: bool,
    /// `chkWeaponOptions(3)`: calculate combat.
    pub calc_combat: bool,
    /// `chkWeaponOptions(4)`: the 85% speed option.
    pub slow_85: bool,
    /// `cmbWeaponCombos(1).ItemData(ListIndex)`; `None` where nothing is selected (error 381).
    pub combo_mode: Option<i32>,
    /// `txtWeaponExtras(2..4).Text`: versus AC, DR and dodge.
    pub extras: [String; 3],
    /// `mnuShopsFirst.Checked`.
    pub shops_first: bool,
    /// `nEquippedItem(0 To 19)`.
    pub equipped: [i32; 20],
    /// `nNMRVer`.
    pub nmr_ver: f64,
    /// `bHideRecordNumbers`.
    pub hide_record_numbers: bool,
    /// `bQuickSpell`.
    pub quick_spell: bool,
    /// `txtGlobalLevel(0).Text` (read while the global filter is on).
    pub global_level_text: String,
    /// `gAvgLevelMaxAllStats`.
    pub avg_level_max_all_stats: f64,
    /// `frmMain.lblDatVer.Caption`.
    pub dat_ver_caption: String,
    /// `lblInvenCharStat(10).Tag`.
    pub accuracy_tag: String,
    /// `txtCharStats(5).Tag` (the charm shop prices use).
    pub charm_tag: String,
    /// `txtCharStats(0).Tag` (the strength of the strength notice).
    pub strength_tag: String,
    /// `nGlobalAttackTypeMME`.
    pub attack_type_mme: i32,
}

impl Default for ItemDetailOptions {
    fn default() -> Self {
        Self {
            global_filter: false,
            calc_combat: false,
            slow_85: false,
            combo_mode: None,
            extras: [String::new(), String::new(), String::new()],
            shops_first: false,
            equipped: [0; 20],
            nmr_ver: 1.83,
            hide_record_numbers: false,
            quick_spell: false,
            global_level_text: String::new(),
            avg_level_max_all_stats: 0.0,
            dat_ver_caption: String::new(),
            accuracy_tag: String::new(),
            charm_tag: "0".to_string(),
            strength_tag: String::new(),
            attack_type_mme: 0,
        }
    }
}

/// The tables the procedure looks things up in.
pub struct ItemDetailEnv<'a> {
    pub profile: Profile,
    /// Items, spells, classes and races.
    pub world: &'a World,
    /// `GetSpellName`, `GetMonsterName`, `GetItemName` and `GetClassName`.
    pub names: &'a dyn NameLookup,
    /// `TBInfo`, for the textblock-cast ability.
    pub textblocks: &'a dyn TextblockData,
    /// The rows `GetLocations` and the teleport rows read.
    pub locations: &'a dyn LocationData,
    pub options: &'a ItemDetailOptions,
}

/// What a piece of the detail text is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailKind {
    /// The weapon's "Min/Max Hit, Speed, ..." line (full details) or the armour line.
    Summary,
    /// The weapon-damage line ("Damage: 40/round @ ...").
    WeaponDamage,
    /// "Notice: Character Strength ... < Strength Requirement ...".
    StrengthNotice,
    /// Not Getable, Not Droppable, Destroys On Death.
    Flags,
    Uses,
    Abilities,
    Casts,
    ClassOk,
    Classes,
    Races,
    Negates,
    /// "Compared to <item>: ...".
    Compared,
}

/// One piece of the detail text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailRow {
    pub kind: DetailKind,
    /// The part before the value ("Abilities", "Casts", "Compared to Sword", ""), without its colon.
    pub label: String,
    pub value: String,
}

/// The ListView sort `PullItemDetail` ends with, as a request: the grid does the sorting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshSort {
    /// `LV_RefreshSort_ShopsFirst` (the Shops First menu is checked) instead of `LV_RefreshSort`.
    pub shops_first: bool,
    /// The rows the list held. Both procedures sort column 1 as numbers by tag, descending.
    pub rows: usize,
}

/// What `PullItemDetail` leaves behind.
#[derive(Debug, Clone, Default)]
pub struct ItemDetail {
    /// `DetailTB.Text`.
    pub text: String,
    /// The same text in pieces. Joining them as MME does gives `text` (the weapon block first).
    pub rows: Vec<DetailRow>,
    /// `LocationLV`.
    pub locations: LocationList,
    pub sort: Option<RefreshSort>,
    /// VB6/DAO error numbers that reached `PullItemDetail`'s handler.
    pub errors: Vec<i32>,
}

// ---------------------------------------------------------------------------------------------
// text helpers

/// modSyntaxsFunc AutoAppend.
fn auto_append(s: &str, add: &str, glue: &str) -> String {
    if !s.is_empty() {
        if add.trim_matches(' ').is_empty() {
            s.to_string()
        } else {
            format!("{s}{glue}{add}")
        }
    } else {
        add.to_string()
    }
}

fn aa(s: &str, add: &str) -> String {
    auto_append(s, add, ", ")
}

/// modSyntaxsFunc AutoPrepend.
fn auto_prepend(s: &str, pre: &str) -> String {
    if !s.is_empty() {
        if pre.trim_matches(' ').is_empty() {
            s.to_string()
        } else {
            format!("{pre}, {s}")
        }
    } else {
        pre.to_string()
    }
}

/// modSyntaxsFunc RoundUp.
fn round_up(x: f64) -> f64 {
    if 0.0 < x - vb::int(x) {
        vb::int(x) + 1.0
    } else {
        vb::int(x)
    }
}

/// modSyntaxsFunc Truncate.
fn truncate(x: f64, places: i32) -> f64 {
    let scale = 10f64.powi(places);
    vb::fix(x * scale) / scale
}

fn sign(x: f64) -> &'static str {
    if x > 0.0 { "+" } else { "" }
}

// ---------------------------------------------------------------------------------------------
// CompareArmor / CompareWeapons

/// Port of modMain CompareArmor: what differs between `a` (the item shown) and `b` (the equipped one).
///
/// # Errors
/// A VB6 error number where MME's handler would end the function early (its text so far is
/// `Err`'s payload in neither case: the function cannot fail on database values).
#[must_use]
pub fn compare_armour(a: &Item, b: &Item) -> String {
    let mut out = String::new();
    if a.worn != b.worn {
        out = aa(
            &out,
            &format!("{} -> {}", worn_text(b.worn), worn_text(a.worn)),
        );
    }
    if a.armour_type != b.armour_type {
        out = aa(
            &out,
            &format!(
                "{} -> {}",
                armour_type_text(b.armour_type),
                armour_type_text(a.armour_type)
            ),
        );
    }
    if a.encum != b.encum {
        let n = f64::from(a.encum) - f64::from(b.encum);
        if n != 0.0 {
            out = aa(&out, &format!("Weight: {}{}", sign(n), vb::cstr(n)));
        }
    }
    if a.accy != b.accy {
        let n = f64::from(a.accy) - f64::from(b.accy);
        if n != 0.0 {
            out = aa(&out, &format!("WepAccy: {}{}", sign(n), vb::cstr(n)));
        }
    }
    if a.armour_class != b.armour_class || a.damage_resist != b.damage_resist {
        let tenth = |x: i32| vb::round(f64::from(x) / 10.0, 1).unwrap_or(0.0);
        let n = tenth(a.armour_class) - tenth(b.armour_class);
        let n2 = tenth(a.damage_resist) - tenth(b.damage_resist);
        if n != 0.0 || n2 != 0.0 {
            out = aa(
                &out,
                &format!(
                    "AC: {}{}/{}{}",
                    sign(n),
                    vb::cstr(n),
                    sign(n2),
                    vb::cstr(n2)
                ),
            );
        }
    }
    if a.limit != b.limit {
        let n = f64::from(a.limit) - f64::from(b.limit);
        if n != 0.0 {
            out = aa(&out, &format!("Limit: {}{}", sign(n), vb::cstr(n)));
        }
    }
    out
}

/// Port of modMain CompareWeapons: what differs between `a` (the item shown) and `b` (the equipped one).
#[must_use]
pub fn compare_weapons(a: &Item, b: &Item) -> String {
    let mut out = String::new();
    if a.weapon_type != b.weapon_type {
        out = aa(
            &out,
            &format!(
                "{} -> {}",
                weapon_type_text(b.weapon_type),
                weapon_type_text(a.weapon_type)
            ),
        );
    }
    let diff = |x: i32, y: i32| f64::from(x) - f64::from(y);
    if a.min != b.min || a.max != b.max {
        let mid = |i: &Item| {
            vb::round(f64::from(i.min) + f64::from(i.max) / 2.0, 0)
                .ok()
                .and_then(|r| vb::clng(r).ok())
        };
        // nTemp is a Long: both rounds are whole numbers, so the difference is exact
        let n = match (mid(a), mid(b)) {
            (Some(x), Some(y)) => f64::from(x) - f64::from(y),
            _ => 0.0,
        };
        if n != 0.0 {
            out = aa(&out, &format!("Damage: {}{}", sign(n), vb::cstr(n)));
        }
    }
    if a.speed != b.speed {
        let n = diff(a.speed, b.speed);
        if n != 0.0 {
            out = aa(&out, &format!("Speed: {}{}", sign(n), vb::cstr(n)));
        }
    }
    if a.encum != b.encum {
        let n = diff(a.encum, b.encum);
        if n != 0.0 {
            out = aa(&out, &format!("Weight: {}{}", sign(n), vb::cstr(n)));
        }
    }
    if a.accy != b.accy {
        let n = diff(a.accy, b.accy);
        if n != 0.0 {
            out = aa(&out, &format!("WepAccy: {}{}", sign(n), vb::cstr(n)));
        }
    }
    if a.armour_class != b.armour_class || a.damage_resist != b.damage_resist {
        let n =
            round_up(f64::from(a.armour_class) / 10.0) - round_up(f64::from(b.armour_class) / 10.0);
        let n2 = f64::from(a.damage_resist) / 10.0 - f64::from(b.damage_resist) / 10.0;
        if n != 0.0 || n2 != 0.0 {
            out = aa(
                &out,
                &format!(
                    "AC: {}{}/{}{}",
                    sign(n),
                    vb::cstr(n),
                    sign(n2),
                    vb::cstr(n2)
                ),
            );
        }
    }
    if a.str_req != b.str_req {
        let n = diff(a.str_req, b.str_req);
        if n != 0.0 {
            out = aa(
                &out,
                &format!("StrReq: {}{} ({})", sign(n), vb::cstr(n), a.str_req),
            );
        }
    }
    if a.limit != b.limit {
        let n = diff(a.limit, b.limit);
        if n != 0.0 {
            out = aa(
                &out,
                &format!("Limit: {}{} ({})", sign(n), vb::cstr(n), a.limit),
            );
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// rows the ability procedures add to a list

/// Walks `GetAbilityStats` and `PullSpellEQ` for the ListView rows they add (see the module docs).
struct RowWalker<'a> {
    spells: &'a [Spell],
    names: &'a dyn NameLookup,
    data: &'a dyn LocationData,
    hide: bool,
    quick: bool,
    /// `Val(txtGlobalLevel(0).Text)` source while the global filter is on.
    level_text: Option<&'a str>,
    avg_level_max_all_stats: f64,
    textblocks: &'a dyn TextblockData,
    /// `nSpellNest`.
    nest: Cell<i32>,
}

fn find_spell(spells: &[Spell], n: i32) -> Option<&Spell> {
    spells.iter().find(|s| s.number == n)
}

impl RowWalker<'_> {
    fn spell_name(&self, n: i32) -> String {
        get_spell_name(self.names, n, self.hide)
    }

    fn monster_name(&self, n: i32) -> String {
        crate::ability_text::get_monster_name(self.names, n, self.hide)
    }

    /// `GetRoomName(, nMap, nRoom, False)`: always with the record numbers.
    fn room_name(&self, map: i32, room: i32) -> String {
        if map == 0 || room == 0 {
            return "?".into();
        }
        match self.data.room(map, room) {
            None => format!("{map}/{room}"),
            Some((name, _)) => {
                let n = if name.is_empty() {
                    "(no name)".to_string()
                } else {
                    name
                };
                format!("{n} ({map}/{room})")
            }
        }
    }

    /// One row as the abilities add it: the text and tag in the first cell (percent column) or the row.
    fn push(
        rows: &mut Vec<LocationRow>,
        kind: LocationKind,
        percent_column: bool,
        text: String,
        tag: String,
    ) {
        if percent_column {
            rows.push(LocationRow {
                kind,
                text: String::new(),
                tag: None,
                cells: vec![LocationCell {
                    text,
                    tag: Some(tag),
                }],
            });
        } else {
            rows.push(LocationRow {
                kind,
                text,
                tag: Some(tag),
                cells: Vec::new(),
            });
        }
    }

    /// The rows `GetAbilityStats(num, value, lv, calc_level, percent_column)` adds.
    fn ability_rows(
        &self,
        rows: &mut Vec<LocationRow>,
        num: i32,
        value: i32,
        calc_level: bool,
        pc: bool,
    ) -> Res<()> {
        // the textblock-cast shortcut returns before any row
        if num == 148 && value > 0 {
            let action = get_textblock_action(self.textblocks, value);
            if action.to_ascii_lowercase().contains("cast ") {
                let all_casts = action
                    .split(':')
                    .all(|part| vb::left(part, 5).is_ok_and(|l| l == "cast "));
                if all_casts {
                    return Ok(());
                }
            }
        }
        if value == 0 {
            return Ok(());
        }
        match num {
            42 | 122 | 160 => Self::push(
                rows,
                LocationKind::Spell,
                pc,
                format!("Spell: {}", self.spell_name(value)),
                value.to_string(),
            ),
            43 | 153 | 151 => {
                self.spell_rows(rows, calc_level, 0, value, pc)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// The rows `PullSpellEQ(calc_level, level, spell, lv, , , pc)` adds.
    fn spell_rows(
        &self,
        rows: &mut Vec<LocationRow>,
        calc_level: bool,
        level: i16,
        spell: i32,
        pc: bool,
    ) -> Res<()> {
        self.nest.set(self.nest.get() + 1);
        let r = self.spell_inner(rows, calc_level, level, spell, pc);
        self.nest.set(self.nest.get() - 1);
        r
    }

    #[allow(clippy::too_many_lines)] // one MME procedure, kept in its order
    fn spell_inner(
        &self,
        rows: &mut Vec<LocationRow>,
        calc_level: bool,
        level: i16,
        spell: i32,
        pc: bool,
    ) -> Res<()> {
        if self.spells.is_empty() || self.nest.get() > 19 {
            return Ok(());
        }
        if self.quick && self.nest.get() > 1 {
            return Ok(());
        }
        let Some(sp) = find_spell(self.spells, spell) else {
            return Ok(());
        };
        let mut level = level;
        let mut use_level = calc_level;
        if use_level {
            if level == 0
                && let Some(t) = self.level_text
            {
                level = vb::cint(vb::val(t)).map_err(code)?;
            }
            if i32::from(level) > sp.cap && sp.cap > 0 {
                level = to_i16(sp.cap)?;
            }
            if i32::from(level) < sp.req_level {
                level = to_i16(sp.req_level)?;
            }
            if level < 1 {
                level = to_i16(sp.req_level)?;
            }
            if level == 0 {
                use_level = false;
            }
        }
        let req = MinMaxRequest {
            use_level,
            level,
            ..MinMaxRequest::default()
        };
        let mm = get_current_spell_min_max(Some(sp), self.avg_level_max_all_stats, &req);
        let (n_min, n_max) = (mm.n_min, mm.n_max);
        let clng = |c: touchstone_vb6::Currency| -> Res<i32> {
            // CLng of a Currency: banker's rounding
            let s = c.scaled();
            let q = s.div_euclid(10_000);
            let r = s.rem_euclid(10_000);
            let v = match (r * 2).cmp(&10_000) {
                std::cmp::Ordering::Less => q,
                std::cmp::Ordering::Greater => q + 1,
                std::cmp::Ordering::Equal => q + (q & 1),
            };
            i32::try_from(v).map_err(|_| code(vb::Vb6Error::Overflow))
        };
        let teleport_map = || {
            let mut map = 0;
            for s in &sp.abilities {
                if s.ability == 141 {
                    map = s.value;
                }
            }
            map
        };
        for slot in &sp.abilities {
            let abil = slot.ability;
            if abil == 0 || abil == 122 || abil == 137 {
                continue;
            }
            let v = slot.value;
            let list_end_two = matches!(
                abil,
                23 | 51 | 52 | 80 | 97 | 98 | 100 | 108..=113 | 119 | 138 | 144 | 178
            );
            if v == 0 {
                match abil {
                    140 => {
                        let map = teleport_map();
                        if map > 0 {
                            // For y = Val(sMin) To Val(sMax): y is a Long, so the bounds round
                            let lo = vb::clng(vb::val(&mm.s_min)).map_err(code)?;
                            let hi = vb::clng(vb::val(&mm.s_max)).map_err(code)?;
                            for y in lo..=hi {
                                Self::push(
                                    rows,
                                    LocationKind::Room,
                                    pc,
                                    format!("Teleport: {}", self.room_name(map, y)),
                                    format!("{map}/{y}"),
                                );
                            }
                        }
                    }
                    148 => {
                        let lo = vb::clng(vb::val(&mm.s_min)).map_err(code)?;
                        let hi = vb::clng(vb::val(&mm.s_max)).map_err(code)?;
                        for y in lo..=hi {
                            Self::push(
                                rows,
                                LocationKind::Textblock,
                                pc,
                                format!("Execute: Textblock {y}"),
                                y.to_string(),
                            );
                        }
                    }
                    164 => {}
                    151 => {
                        let (lo, hi) = (clng(n_min)?, clng(n_max)?);
                        if self.quick {
                            // the short text; no rows
                        } else if n_min >= n_max {
                            self.spell_rows(rows, calc_level, level, lo, pc)?;
                        } else {
                            self.spell_rows(rows, calc_level, level, lo, pc)?;
                            let from = clng(touchstone_vb6::Currency::from_scaled(
                                n_min.scaled() + 10_000,
                            ))?;
                            for y in from..=hi {
                                self.spell_rows(rows, calc_level, level, y, pc)?;
                            }
                        }
                    }
                    12 => {
                        let (lo, hi) = (clng(n_min)?, clng(n_max)?);
                        if !self.quick {
                            if n_min >= n_max {
                                Self::push(
                                    rows,
                                    LocationKind::Monster,
                                    pc,
                                    format!("Summon: {}", self.monster_name(lo)),
                                    lo.to_string(),
                                );
                            } else {
                                Self::push(
                                    rows,
                                    LocationKind::Monster,
                                    pc,
                                    format!("Summon: {}", self.monster_name(lo)),
                                    lo.to_string(),
                                );
                                let from = clng(touchstone_vb6::Currency::from_scaled(
                                    n_min.scaled() + 10_000,
                                ))?;
                                for y in from..=hi {
                                    Self::push(
                                        rows,
                                        LocationKind::Monster,
                                        pc,
                                        format!("Summon: {}", self.monster_name(y)),
                                        y.to_string(),
                                    );
                                }
                            }
                        }
                    }
                    _ if list_end_two => {}
                    _ => {
                        // GetAbilityStats with no value adds no rows
                    }
                }
            } else {
                match abil {
                    148 => {
                        self.ability_rows(rows, abil, v, true, pc)?;
                        Self::push(
                            rows,
                            LocationKind::Textblock,
                            pc,
                            format!("Execute: Textblock {v}"),
                            v.to_string(),
                        );
                    }
                    12 => {
                        if !self.quick {
                            Self::push(
                                rows,
                                LocationKind::Monster,
                                pc,
                                format!("Summon: {}", self.monster_name(v)),
                                v.to_string(),
                            );
                        }
                    }
                    140 => {
                        let map = teleport_map();
                        if map > 0 {
                            Self::push(
                                rows,
                                LocationKind::Room,
                                pc,
                                format!("Teleport: {}", self.room_name(map, v)),
                                format!("{map}/{v}"),
                            );
                        }
                    }
                    164 => {}
                    _ if list_end_two => {}
                    _ => self.ability_rows(rows, abil, v, calc_level, pc)?,
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// PullItemDetail

/// The item a compare block is against: `None` where `Seek` found no row (reading it is error 3021).
type Equipped<'a> = Option<&'a Item>;

fn rec<'a>(r: Equipped<'a>) -> Res<&'a Item> {
    r.ok_or(NO_CURRENT_RECORD)
}

/// `in_array_long_md(arr, value, row)`: the value is in that row of a restriction array.
fn in_row(arr: &[[i32; 10]; 3], value: i32, row: usize) -> bool {
    arr[row].contains(&value)
}

/// `getval_array_long_3d` over `nAbils`, searching one item's slots from the last to the first for the
/// ability number (and, with `must_match`, a slot holding that value).
fn find_ability(
    abils: &[[(i32, i32); 20]; 3],
    item: usize,
    ability: i32,
    must_match: Option<i32>,
) -> Option<i32> {
    for slot in (0..20).rev() {
        let (a, v) = abils[item][slot];
        if a == ability {
            if let Some(m) = must_match
                && v != m
            {
                continue;
            }
            return Some(v);
        }
    }
    None
}

struct Run<'a> {
    env: &'a ItemDetailEnv<'a>,
    state: Option<&'a CharacterState>,
    out: ItemDetail,
    ability: AbilityEnv<'a>,
    walker: RowWalker<'a>,
}

/// Port of modMain `PullItemDetail`. `number` is the item the cursor is on, `attack_type` the optional
/// `nAttackTypeMUD` argument (0: omitted) and `full_details` the optional `bFullDetails`.
///
/// `lairs` answers `GetLairInfo` for the reference rows. `character` is the loaded character the
/// frmMain controls hold (`None`: no character; the global filter is read from `env.options`).
#[must_use]
pub fn pull_item_detail(
    env: &ItemDetailEnv,
    lairs: &mut dyn LocationLairs,
    number: i32,
    attack_type: i32,
    full_details: bool,
    character: Option<&CharacterState>,
) -> ItemDetail {
    let o = env.options;
    let level_text = o.global_filter.then_some(o.global_level_text.as_str());
    let ability = AbilityEnv {
        profile: env.profile,
        names: env.names,
        textblocks: env.textblocks,
        spells: &env.world.spells,
        hide_record_numbers: o.hide_record_numbers,
        quick_spell: o.quick_spell,
        global_level_text: level_text,
        avg_level_max_all_stats: o.avg_level_max_all_stats,
        nest: Cell::new(0),
    };
    let walker = RowWalker {
        spells: &env.world.spells,
        names: env.names,
        data: env.locations,
        hide: o.hide_record_numbers,
        quick: o.quick_spell,
        level_text,
        avg_level_max_all_stats: o.avg_level_max_all_stats,
        textblocks: env.textblocks,
        nest: Cell::new(0),
    };
    let mut run = Run {
        env,
        state: character,
        out: ItemDetail::default(),
        ability,
        walker,
    };
    match run.steps(lairs, number, attack_type, full_details) {
        Ok(()) => {}
        Err(e) => {
            run.out.errors.push(e);
            run.out.text.clear();
            run.out.rows.clear();
            run.out.sort = None;
        }
    }
    run.out
}

impl Run<'_> {
    #[allow(clippy::too_many_lines)] // one MME procedure, kept in its order
    fn steps(
        &mut self,
        lairs: &mut dyn LocationLairs,
        number: i32,
        attack_arg: i32,
        full: bool,
    ) -> Res<()> {
        let env = self.env;
        let o = env.options;
        let world = env.world;
        let item = world.get_item(number).ok_or(NO_CURRENT_RECORD)?;
        let hide = o.hide_record_numbers;

        let use_char = o.global_filter;
        let mut attack = attack_arg;
        let mut speed_adj: i16 = 100;
        if o.calc_combat {
            if attack == 0 {
                attack = o.combo_mode.ok_or(BAD_INDEX)?;
            }
            if o.slow_85 {
                speed_adj = 85;
            }
        } else if attack == 0 {
            attack = 5;
        }

        // tCharacter: only weapons populate it; every other item reads an empty profile
        let mud = AttackTypeMud::from_code(attack).unwrap_or_default();
        let mut tchar = CharacterProfile::default();
        if item.item_type == 1 {
            tchar = match self.state {
                Some(s) if use_char => s.profile_for(world, mud, number),
                _ => no_character_profile(
                    env.profile,
                    world,
                    o.nmr_ver,
                    o.avg_level_max_all_stats,
                    mud,
                ),
            };
        }

        // which equipped items to compare against
        let ne = &o.equipped;
        let mut slot1: i32 = -1;
        let mut slot2: i32 = -1;
        let mut cmp_weapon = false;
        let mut cmp_armour = false;
        let mut aura = false;
        match item.item_type {
            0 => {
                if item.worn <= 0 {
                    aura = true;
                } else if item.worn <= 19 {
                    match item.worn {
                        0 => aura = true,
                        1 => slot1 = 19,
                        2 => slot1 = 0,
                        3 => slot1 = 8,
                        4 | 13 => {
                            if ne[9] > 0 {
                                slot1 = 9;
                                if ne[10] > 0 {
                                    slot2 = 10;
                                }
                            } else if ne[10] > 0 {
                                slot1 = 10;
                            }
                        }
                        5 => slot1 = 13,
                        6 => slot1 = 5,
                        7 => slot1 = 3,
                        8 => slot1 = 2,
                        9 => slot1 = 12,
                        10 => slot1 = 11,
                        11 => slot1 = 4,
                        12 => slot1 = 15,
                        14 => {
                            if ne[6] > 0 {
                                slot1 = 6;
                                if ne[7] > 0 {
                                    slot2 = 7;
                                }
                            } else if ne[7] > 0 {
                                slot1 = 7;
                            }
                        }
                        15 => slot1 = 1,
                        16 => slot1 = 14,
                        18 => slot1 = 17,
                        19 => slot1 = 18,
                        _ => {}
                    }
                    if slot1 >= 0 {
                        if ne[slot1 as usize] > 0 {
                            cmp_armour = true;
                        } else {
                            slot1 = -1;
                        }
                    }
                    if slot2 >= 0 {
                        if ne[slot2 as usize] > 0 {
                            cmp_armour = true;
                        } else {
                            slot2 = -1;
                        }
                    }
                }
            }
            1 => {
                slot1 = 16;
                if ne[16] > 0 {
                    cmp_weapon = true;
                }
            }
            10 => aura = true,
            _ => {}
        }
        let _ = aura; // only GreaterMUD reads it
        if !cmp_weapon && !cmp_armour {
            slot1 = -1;
            slot2 = -1;
        }
        if slot1 >= 0 && ne[slot1 as usize] == number {
            slot1 = -1;
        }
        if slot2 >= 0 && ne[slot2 as usize] == number {
            slot2 = -1;
        }
        if slot2 >= 0 && slot1 < 0 {
            slot1 = slot2;
            slot2 = -1;
        }
        if slot1 < 0 && slot2 < 0 {
            cmp_weapon = false;
            cmp_armour = false;
        }

        // the equipped items' records (tabItems1 / tabItems2); a failed Seek leaves a recordset at EOF
        let mut item1: Equipped = None;
        let mut item2: Equipped = None;
        if cmp_weapon || (cmp_armour && slot1 >= 0) {
            item1 = world.get_item(ne[slot1 as usize]);
            if item1.is_none() && slot2 < 0 {
                cmp_weapon = false;
                cmp_armour = false;
                slot1 = -1;
                slot2 = -1;
            }
            if slot2 >= 0 {
                item2 = world.get_item(ne[slot2 as usize]);
                if item2.is_none() && slot1 < 0 {
                    cmp_weapon = false;
                    cmp_armour = false;
                    slot1 = -1;
                    slot2 = -1;
                }
            }
        }

        let mut cmp1 = String::new();
        let mut cmp2 = String::new();

        // uses
        let uses_text = |it: &Item, prefix: &str| {
            let mut s = format!("{prefix}{}", it.use_count);
            s.push_str(if it.retain_after_uses {
                " start/max"
            } else {
                " (destroys after uses)"
            });
            s
        };
        let mut s_uses = String::new();
        if item.use_count > 0 {
            s_uses = uses_text(item, "");
        }
        let mut t1 = String::new();
        let mut t2 = String::new();
        if slot1 >= 0 {
            let i1 = rec(item1)?;
            if i1.use_count > 0 {
                t1 = uses_text(i1, "Uses: ");
            }
            if t1 != s_uses {
                cmp1 = aa(&cmp1, &t1);
            }
        }
        if slot2 >= 0 {
            let i2 = rec(item2)?;
            if i2.use_count > 0 {
                t2 = uses_text(i2, "Uses: ");
            }
            if t2 != s_uses {
                cmp2 = aa(&cmp2, &t2);
            }
        }

        // getable, droppable, destroyed on death
        let mut get_drop = String::new();
        // Gettable = 0 is "not getable"; Not Droppable = 1; Destroy On Death = 1
        if !item.gettable {
            get_drop = aa(&get_drop, "Not Getable");
        }
        if slot1 >= 0 {
            let i1 = rec(item1)?;
            if item.gettable != i1.gettable {
                cmp1 = aa(
                    &cmp1,
                    if !i1.gettable {
                        "+Getable"
                    } else {
                        "+Not Getable"
                    },
                );
            }
        }
        if slot2 >= 0 {
            let i2 = rec(item2)?;
            if item.gettable != i2.gettable {
                cmp2 = aa(
                    &cmp2,
                    if !i2.gettable {
                        "+Getable"
                    } else {
                        "+Not Getable"
                    },
                );
            }
        }
        if item.not_droppable {
            get_drop = aa(&get_drop, "Not Droppable");
        }
        if slot1 >= 0 {
            let i1 = rec(item1)?;
            if item.not_droppable != i1.not_droppable {
                cmp1 = aa(
                    &cmp1,
                    if i1.not_droppable {
                        "+Droppable"
                    } else {
                        "+Not Droppable"
                    },
                );
            }
        }
        if slot2 >= 0 {
            let i2 = rec(item2)?;
            if item.not_droppable != i2.not_droppable {
                cmp2 = aa(
                    &cmp2,
                    if i2.not_droppable {
                        "+Droppable"
                    } else {
                        "+Not Droppable"
                    },
                );
            }
        }
        if item.destroy_on_death {
            get_drop = aa(&get_drop, "Destroys On Death");
        }
        if slot1 >= 0 {
            let i1 = rec(item1)?;
            if item.destroy_on_death != i1.destroy_on_death {
                cmp1 = aa(
                    &cmp1,
                    if i1.destroy_on_death {
                        "-Destroy On Death"
                    } else {
                        "+Destroy On Death"
                    },
                );
            }
        }
        if slot2 >= 0 {
            let i2 = rec(item2)?;
            if item.destroy_on_death != i2.destroy_on_death {
                cmp2 = aa(
                    &cmp2,
                    if i2.destroy_on_death {
                        "-Destroy On Death"
                    } else {
                        "+Destroy On Death"
                    },
                );
            }
        }

        // abilities of the three items
        let mut abils = [[(0i32, 0i32); 20]; 3];
        for x in 0..20 {
            let slot = item.abilities[x];
            if slot.ability > 0 {
                abils[0][x] = (slot.ability, slot.value);
            }
            if slot1 >= 0 {
                let s = rec(item1)?.abilities[x];
                if s.ability > 0 {
                    abils[1][x] = (s.ability, s.value);
                }
            }
            if slot2 >= 0 {
                let s = rec(item2)?.abilities[x];
                if s.ability > 0 {
                    abils[2][x] = (s.ability, s.value);
                }
            }
        }

        let mut s_abil = String::new();
        let mut s_casts = String::new();
        let mut s_class_ok = String::new();
        let mut n_percent: i16 = 0;
        let mut bs_acc: i16 = 0;
        let mut lvl_req: i16 = 0;
        let mut n_acc: i16 = 0;
        for x in 0..20 {
            let (a, v) = abils[0][x];
            if a <= 0 {
                continue;
            }
            match a {
                116 => {
                    self.walker
                        .ability_rows(&mut self.out.locations.rows, a, v, true, true)?;
                    let t = self
                        .ability
                        .get_ability_stats(to_i16(a)?, to_i16(v)?, true, true);
                    s_abil = aa(&s_abil, &t);
                    bs_acc = to_i16(v)?;
                }
                22 | 105 | 106 | 135 => {
                    self.walker
                        .ability_rows(&mut self.out.locations.rows, a, v, true, true)?;
                    let t = self
                        .ability
                        .get_ability_stats(to_i16(a)?, to_i16(v)?, true, true);
                    s_abil = aa(&s_abil, &t);
                    if a == 135 {
                        lvl_req = to_i16(v)?;
                    } else {
                        n_acc = vb::add_i16(n_acc, to_i16(v)?).map_err(code)?;
                    }
                }
                59 => {
                    let t = get_class_name(env.names, v);
                    s_class_ok = aa(&s_class_ok, &t);
                }
                43 => {
                    let eq = self.ability.pull_spell_eq(&PullArgs {
                        calc_level: true,
                        level: 0,
                        spell: v,
                        percent_column: true,
                        spell_bonus: tchar.spell_dmg_bonus,
                        ..PullArgs::default()
                    });
                    let mut t = format!("[{}, {eq}", get_spell_name(env.names, v, hide));
                    if i32::from(n_percent) > 0 {
                        t.push_str(&format!(", {n_percent}%]"));
                    } else {
                        t.push(']');
                    }
                    s_casts = aa(&s_casts, &t);
                }
                114 => n_percent = to_i16(v)?,
                _ => {
                    self.walker
                        .ability_rows(&mut self.out.locations.rows, a, v, true, true)?;
                    let t = self
                        .ability
                        .get_ability_stats(to_i16(a)?, to_i16(v)?, true, true);
                    s_abil = aa(&s_abil, &t);
                }
            }
        }
        n_acc = vb::add_i16(n_acc, to_i16(item.accy)?).map_err(code)?;

        // ability differences
        let mut s_cast_sp1 = String::new();
        let mut s_cast_sp2 = String::new();
        let mut s_class_ok1 = String::new();
        let mut s_class_ok2 = String::new();
        if slot1 >= 0 {
            let mut t1 = String::new();
            let mut t2 = String::new();
            let mut cast_flag = [false; 3];
            let mut flag1 = false;
            let mut flag2 = false;
            for y in 0..3 {
                let mut n_pct = [0i16; 3];
                for x in 0..20 {
                    let (a, v) = abils[y][x];
                    if a <= 0 {
                        continue;
                    }
                    let mut must_match: Option<i32> = None;
                    if a == 59 {
                        must_match = Some(i32::from(to_i16(v)?));
                    }
                    if a == 43 {
                        must_match = Some(i32::from(to_i16(v)?));
                        cast_flag[y] = true;
                    }
                    if a == 114 {
                        n_pct[y] = to_i16(v)?;
                    }
                    if y == 0 {
                        for (which, flag, t, class_ok, cast_sp) in [
                            (
                                1usize,
                                &mut flag1,
                                &mut t1,
                                &mut s_class_ok1,
                                &mut s_cast_sp1,
                            ),
                            (
                                2usize,
                                &mut flag2,
                                &mut t2,
                                &mut s_class_ok2,
                                &mut s_cast_sp2,
                            ),
                        ] {
                            if which == 2 && slot2 < 0 {
                                continue;
                            }
                            match find_ability(&abils, which, a, must_match) {
                                None => {
                                    let t3 =
                                        self.ability.get_abil_diff_text(a, v, 0, n_pct[y], false);
                                    if !t3.is_empty() {
                                        if a == 59 {
                                            *class_ok = aa(class_ok, &format!("+{t3}"));
                                        } else if a == 43 {
                                            *cast_sp = aa(cast_sp, &format!("+{t3}"));
                                        } else {
                                            *flag = true;
                                            *t = aa(
                                                t,
                                                &format!("{}{t3}", if v == 0 { "+" } else { "" }),
                                            );
                                        }
                                    }
                                }
                                Some(r) if r != v => {
                                    let t3 =
                                        self.ability.get_abil_diff_text(a, v, r, n_pct[y], false);
                                    if !t3.is_empty() {
                                        *flag = true;
                                        *t = aa(t, &t3);
                                    }
                                }
                                Some(_) => {}
                            }
                        }
                    } else if find_ability(&abils, 0, a, must_match).is_none() {
                        let t3 = self.ability.get_abil_diff_text(a, v, 0, n_pct[y], true);
                        if !t3.is_empty() {
                            if a == 59 {
                                if y == 1 {
                                    s_class_ok1 = aa(&s_class_ok1, &format!("-{t3}"));
                                } else {
                                    s_class_ok2 = aa(&s_class_ok2, &format!("-{t3}"));
                                }
                            } else if a == 43 {
                                if y == 1 {
                                    s_cast_sp1 = aa(&s_cast_sp1, &format!("-{t3}"));
                                } else {
                                    s_cast_sp2 = aa(&s_cast_sp2, &format!("-{t3}"));
                                }
                            } else if y == 1 {
                                flag1 = true;
                                t1 = aa(&t1, &format!("{}{t3}", if v == 0 { "-" } else { "" }));
                            } else {
                                flag2 = true;
                                t2 = aa(&t2, &format!("{}{t3}", if v == 0 { "-" } else { "" }));
                            }
                        }
                    }
                }
            }
            for (t, flag, cmp) in [(&t1, flag1, &mut cmp1), (&t2, flag2, &mut cmp2)] {
                if !t.is_empty() {
                    let mut head = "Abilities".to_string();
                    if s_abil.is_empty() && flag {
                        head.push_str(&format!(" [none]: {t}"));
                    } else {
                        head.push_str(&format!(": {t}"));
                    }
                    *cmp = auto_append(cmp, &head, " -- ");
                }
            }
            for (t, flag, cmp) in [
                (&s_cast_sp1, cast_flag[1], &mut cmp1),
                (&s_cast_sp2, cast_flag[2], &mut cmp2),
            ] {
                if !t.is_empty() {
                    let mut head = "Casts".to_string();
                    if s_casts.is_empty() && flag {
                        head.push_str(&format!(" [none]: {t}"));
                    } else {
                        head.push_str(&format!(": {t}"));
                    }
                    *cmp = auto_append(cmp, &head, " -- ");
                }
            }
            for (t, cmp) in [(&s_class_ok1, &mut cmp1), (&s_class_ok2, &mut cmp2)] {
                if !t.is_empty() {
                    *cmp = auto_append(cmp, &format!("ClassOk: {t}"), " -- ");
                }
            }
        }

        // class, race and negated-spell restrictions
        let mut s_classes = String::new();
        let mut s_races = String::new();
        let mut s_negate = String::new();
        let mut classes = [[0i32; 10]; 3];
        let mut races = [[0i32; 10]; 3];
        let mut negates = [[0i32; 10]; 3];
        for x in 0..10 {
            if item.class_rest[x] != 0 {
                s_classes = aa(&s_classes, &get_class_name(env.names, item.class_rest[x]));
                classes[0][x] = item.class_rest[x];
            }
            if slot1 >= 0 && rec(item1)?.class_rest[x] != 0 {
                classes[1][x] = rec(item1)?.class_rest[x];
            }
            if slot2 >= 0 && rec(item2)?.class_rest[x] != 0 {
                classes[2][x] = rec(item2)?.class_rest[x];
            }
        }
        self.restriction_compare(
            slot1,
            slot2,
            &classes,
            &s_classes,
            "Classes",
            |n| get_class_name(env.names, n),
            RestrictionStyle::Restricted,
            (&mut cmp1, &mut cmp2),
        );

        let race_name = |n: i32| -> String {
            if n == 0 {
                "None".to_string()
            } else {
                world
                    .get_race(n)
                    .map_or_else(|| n.to_string(), |r| r.name.clone())
            }
        };
        for x in 0..10 {
            if item.race_rest[x] != 0 {
                s_races = aa(&s_races, &race_name(item.race_rest[x]));
                races[0][x] = item.race_rest[x];
            }
            if slot1 >= 0 && rec(item1)?.race_rest[x] != 0 {
                races[1][x] = rec(item1)?.race_rest[x];
            }
            if slot2 >= 0 && rec(item2)?.race_rest[x] != 0 {
                races[2][x] = rec(item2)?.race_rest[x];
            }
        }
        self.restriction_compare(
            slot1,
            slot2,
            &races,
            &s_races,
            "Races",
            race_name,
            RestrictionStyle::Restricted,
            (&mut cmp1, &mut cmp2),
        );

        let spell_name_full = |n: i32| get_spell_name(env.names, n, false);
        for x in 0..10 {
            if item.negate_spell[x] != 0 {
                s_negate = aa(&s_negate, &spell_name_full(item.negate_spell[x]));
                negates[0][x] = item.negate_spell[x];
            }
            if slot1 >= 0 && rec(item1)?.negate_spell[x] != 0 {
                negates[1][x] = rec(item1)?.negate_spell[x];
            }
            if slot2 >= 0 && rec(item2)?.negate_spell[x] != 0 {
                negates[2][x] = rec(item2)?.negate_spell[x];
            }
        }
        self.restriction_compare(
            slot1,
            slot2,
            &negates,
            &s_negate,
            "Negate",
            spell_name_full,
            RestrictionStyle::None,
            (&mut cmp1, &mut cmp2),
        );

        // the reference list
        let lopt = LocationOptions {
            nmr_ver: o.nmr_ver,
            hide_record_numbers: hide,
            global_filter: use_char,
            charm_tag: o.charm_tag.clone(),
            attack_type: o.attack_type_mme,
        };
        get_locations(
            env.locations,
            lairs,
            &lopt,
            &LocationArgs {
                loc: item.obtained_from.as_deref().unwrap_or(""),
                dont_clear: true,
                aux_value: number,
                percent_column: true,
                ..LocationArgs::default()
            },
            &mut self.out.locations,
        );
        if o.nmr_ver >= 1.7 {
            get_locations(
                env.locations,
                lairs,
                &lopt,
                &LocationArgs {
                    loc: item.references.as_deref().unwrap_or(""),
                    dont_clear: true,
                    percent_column: true,
                    ..LocationArgs::default()
                },
                &mut self.out.locations,
            );
        }
        for x in 0..20 {
            let (a, v) = abils[0][x];
            if a == 43 {
                // PullSpellEQ with the list only adds its references; then the "Casts:" row
                let mut rows = std::mem::take(&mut self.out.locations.rows);
                let r = self.walker.spell_rows(&mut rows, true, 0, v, true);
                rows.push(LocationRow {
                    kind: LocationKind::Spell,
                    text: String::new(),
                    tag: None,
                    cells: vec![LocationCell {
                        text: format!("Casts: {}", get_spell_name(env.names, v, hide)),
                        tag: Some(v.to_string()),
                    }],
                });
                self.out.locations.rows = rows;
                r?;
            }
        }

        // the text
        let mut s_str = String::new();
        let mut parts: Vec<DetailRow> = Vec::new();
        let mut piece = |kind: DetailKind,
                         label: &str,
                         value: &str,
                         s: &mut String,
                         glue: &str,
                         text: String| {
            *s = auto_append(s, &text, glue);
            parts.push(DetailRow {
                kind,
                label: label.to_string(),
                value: value.to_string(),
            });
        };
        if !get_drop.is_empty() {
            piece(
                DetailKind::Flags,
                "",
                &get_drop,
                &mut s_str,
                " -- ",
                get_drop.clone(),
            );
        }
        if !s_uses.is_empty() {
            piece(
                DetailKind::Uses,
                "Uses",
                &s_uses,
                &mut s_str,
                " -- ",
                format!("Uses: {s_uses}"),
            );
        }
        if !s_abil.is_empty() {
            piece(
                DetailKind::Abilities,
                "Abilities",
                &s_abil,
                &mut s_str,
                " -- ",
                format!("Abilities: {s_abil}"),
            );
        }
        if !s_casts.is_empty() {
            piece(
                DetailKind::Casts,
                "Casts",
                &s_casts,
                &mut s_str,
                " -- ",
                format!("Casts: {s_casts}"),
            );
        }
        if !s_class_ok.is_empty() {
            piece(
                DetailKind::ClassOk,
                "ClassOK",
                &s_class_ok,
                &mut s_str,
                " -- ",
                format!("ClassOK: {s_class_ok}"),
            );
        }
        if !s_classes.is_empty() {
            piece(
                DetailKind::Classes,
                "Classes",
                &s_classes,
                &mut s_str,
                " -- ",
                format!("Classes: {s_classes}"),
            );
        }
        if !s_races.is_empty() {
            piece(
                DetailKind::Races,
                "Races",
                &s_races,
                &mut s_str,
                " -- ",
                format!("Races: {s_races}"),
            );
        }
        if !s_negate.is_empty() {
            piece(
                DetailKind::Negates,
                "Negates",
                &s_negate,
                &mut s_str,
                " -- ",
                format!("Negates: {s_negate}"),
            );
        }
        for (slot, which, cmp) in [(slot1, item1, &mut cmp1), (slot2, item2, &mut cmp2)] {
            if slot >= 0 {
                let other = rec(which)?;
                if cmp_weapon {
                    *cmp = auto_prepend(cmp, &compare_weapons(item, other));
                }
                if cmp_armour {
                    *cmp = auto_prepend(cmp, &compare_armour(item, other));
                }
                let label = format!("Compared to {}", other.name);
                let text = format!("{label}: {cmp}");
                piece(
                    DetailKind::Compared,
                    &label,
                    cmp,
                    &mut s_str,
                    "\r\n\r\n",
                    text,
                );
            }
        }

        // weapon damage / armour block
        let mut s_weapon = String::new();
        let mut weapon_rows: Vec<DetailRow> = Vec::new();
        if item.item_type == 1 {
            let mut force_calc = false;
            if attack == 4 && use_char {
                let (cs, rs) = self.stealth();
                if !cs && !rs {
                    force_calc = true;
                }
            }
            let ab_env = &self.ability;
            let equipped = self
                .state
                .map_or_else(EquippedGlobals::default, CharacterState::equipped_globals);
            let attack_env = AttackEnv {
                items: &world.items,
                classes: &world.classes,
                races: &world.races,
                equipped: &equipped,
                cast_text: ab_env,
                dat_ver_caption: &o.dat_ver_caption,
                avg_level_max_all_stats: o.avg_level_max_all_stats,
                accuracy_tag: &o.accuracy_tag,
            };
            let extra = |k: usize| -> Res<i32> {
                vb::clng(if o.calc_combat {
                    vb::val(&o.extras[k])
                } else {
                    0.0
                })
                .map_err(code)
            };
            let req = AttackRequest {
                attack_type: attack,
                weapon_number: item.number,
                speed_adj,
                vs_ac: extra(0)?,
                vs_dr: extra(1)?,
                vs_dodge: extra(2)?,
                force_calc,
                ..AttackRequest::default()
            };
            let ch = CharacterState::attack_profile_of(&tchar);
            let mut casts = s_casts.clone();
            let dmg = attack::calculate_attack(env.profile, &attack_env, &ch, &req, &mut casts);
            if dmg.swings > 0.0 {
                let mut label = match attack {
                    1 => "Punch Damage",
                    2 => "Kick Damage",
                    3 => "Jumpkick Damage",
                    4 => "Backstab Damage",
                    6 => "Bash Damage",
                    7 => "Smash Damage",
                    _ => "Damage",
                }
                .to_string();
                if !use_char {
                    label.push_str(&format!(" (@lvl {})", tchar.level));
                }
                let mut w = format!(
                    "{}/round @ {} swings w/{}% hit chance",
                    dmg.round_total,
                    vb::cstr(truncate(dmg.swings, 1)),
                    dmg.hit_chance
                );
                if attack == 4 && force_calc {
                    w.push_str(" (forced race stealth)");
                }
                w.push_str(&format!(" - Avg Hit: {}", dmg.avg_hit));
                if dmg.max_crit > 0 && dmg.crit_chance > 0 {
                    w = aa(
                        &w,
                        &format!("Avg/Max Crit: {}/{}", dmg.avg_crit, dmg.max_crit),
                    );
                    w.push_str(&format!(" ({}%", dmg.crit_chance));
                    if dmg.qnd_bonus > 0 {
                        w.push_str(&format!(" w/{}qnd", dmg.qnd_bonus));
                    }
                    w.push(')');
                }
                if dmg.avg_extra_hit > 0 {
                    w = aa(&w, &format!("Avg Extra: {}", dmg.avg_extra_hit));
                    if dmg.avg_extra_hit != dmg.avg_extra_swing {
                        w.push_str(&format!(" (avg {}/swing)", dmg.avg_extra_swing));
                    }
                }
                if dmg.round_total > 0
                    && dmg.round_total != dmg.first_round_damage
                    && dmg.swings != vb::fix(dmg.swings)
                {
                    w.push_str(&format!(
                        " - 1st Round: {} dmg @ {} swings",
                        dmg.first_round_damage,
                        vb::cstr(vb::fix(dmg.swings))
                    ));
                }
                let mut head = String::new();
                if full {
                    let mut t = format!("Min/Max Hit: {}/{}", dmg.min_dmg, dmg.max_dmg);
                    t.push_str(&format!(", Speed: {}", item.speed));
                    if lvl_req > 0 {
                        t.push_str(&format!(", LVL Req: {lvl_req}"));
                    }
                    if item.str_req > 0 {
                        t.push_str(&format!(", STR Req: {}", item.str_req));
                    }
                    if item.armour_class + item.damage_resist > 0 {
                        t.push_str(&format!(
                            ", AC/DR: {}/{}",
                            vb::cstr(round_up(f64::from(item.armour_class) / 10.0)),
                            vb::cstr(f64::from(item.damage_resist) / 10.0)
                        ));
                    }
                    if n_acc > 0 {
                        t.push_str(&format!(", Accy: {n_acc}"));
                    }
                    t.push_str(&format!(
                        ", BS: {}",
                        if bs_acc > 0 {
                            bs_acc.to_string()
                        } else {
                            "No".to_string()
                        }
                    ));
                    if item.limit > 0 {
                        t.push_str(&format!(", Limit: {}", item.limit));
                    }
                    s_weapon = format!("{t}\r\n");
                    head = t;
                }
                let line = format!("{label}: {w}");
                s_weapon.push_str(&line);
                s_weapon.push_str("\r\n");
                if !head.is_empty() {
                    weapon_rows.push(DetailRow {
                        kind: DetailKind::Summary,
                        label: String::new(),
                        value: head,
                    });
                }
                weapon_rows.push(DetailRow {
                    kind: DetailKind::WeaponDamage,
                    label,
                    value: w,
                });
                if use_char && f64::from(item.str_req) > vb::val(&o.strength_tag) {
                    let notice = format!(
                        "Notice: Character Strength ({}) < Strength Requirement ({})",
                        vb::cstr(vb::val(&o.strength_tag)),
                        item.str_req
                    );
                    s_weapon.push_str(&notice);
                    s_weapon.push_str("\r\n");
                    weapon_rows.push(DetailRow {
                        kind: DetailKind::StrengthNotice,
                        label: String::new(),
                        value: notice,
                    });
                }
                s_weapon.push_str("\r\n");
            }
        } else if item.item_type == 0 && full {
            let mut t = format!("Armour Type: {}", armour_type_text(item.armour_type));
            if lvl_req > 0 {
                t.push_str(&format!(", LVL Req: {lvl_req}"));
            }
            if item.armour_class + item.damage_resist > 0 {
                t.push_str(&format!(
                    ", AC/DR: {}/{}",
                    vb::cstr(round_up(f64::from(item.armour_class) / 10.0)),
                    vb::cstr(f64::from(item.damage_resist) / 10.0)
                ));
            }
            if n_acc > 0 {
                t.push_str(&format!(", Accy: {n_acc}"));
            }
            if item.limit > 0 {
                t.push_str(&format!(", Limit: {}", item.limit));
            }
            s_weapon = format!("{t}\r\n\r\n");
            weapon_rows.push(DetailRow {
                kind: DetailKind::Summary,
                label: String::new(),
                value: t,
            });
        }

        self.out.text = format!("{s_weapon}{s_str}");
        weapon_rows.extend(parts);
        self.out.rows = weapon_rows;

        if !self.out.locations.rows.is_empty() {
            self.out.sort = Some(RefreshSort {
                shops_first: o.shops_first,
                rows: self.out.locations.rows.len(),
            });
        }
        Ok(())
    }

    /// `GetClassStealth` and `GetRaceStealth` with no argument: the global filter's class and race.
    fn stealth(&self) -> (bool, bool) {
        let world = self.env.world;
        let Some(state) = self.state else {
            return (false, false);
        };
        let class = state.character.class.unwrap_or(0);
        let race = state.character.race.unwrap_or(0);
        let cs = class != 0 && stats::get_class_stealth(world.get_class(class));
        let rs = race > 0 && stats::get_race_stealth(world.get_race(race));
        (cs, rs)
    }

    /// The compare lines for class, race and negated-spell restrictions (one MME block each).
    #[allow(clippy::too_many_arguments)]
    fn restriction_compare(
        &self,
        slot1: i32,
        slot2: i32,
        arr: &[[i32; 10]; 3],
        shown: &str,
        label: &str,
        name: impl Fn(i32) -> String,
        style: RestrictionStyle,
        (cmp1, cmp2): (&mut String, &mut String),
    ) {
        if slot1 < 0 {
            return;
        }
        let mut t1 = String::new();
        let mut t2 = String::new();
        let mut f1 = false;
        let mut f2 = false;
        for y in 0..3 {
            for x in 0..10 {
                let v = arr[y][x];
                if v > 0 {
                    if y == 0 {
                        if !in_row(arr, v, 1) {
                            t1 = aa(&t1, &format!("+{}", name(v)));
                        }
                        if slot2 >= 0 && !in_row(arr, v, 2) {
                            t2 = aa(&t2, &format!("+{}", name(v)));
                        }
                    } else {
                        if y == 1 {
                            f1 = true;
                        } else {
                            f2 = true;
                        }
                        if !in_row(arr, v, 0) {
                            if y == 1 {
                                t1 = aa(&t1, &format!("-{}", name(v)));
                            } else {
                                t2 = aa(&t2, &format!("-{}", name(v)));
                            }
                        }
                    }
                }
            }
        }
        for (t, flag, cmp) in [(&t1, f1, cmp1), (&t2, f2, cmp2)] {
            if !t.is_empty() {
                let mut head = label.to_string();
                match style {
                    RestrictionStyle::Restricted => {
                        if shown.is_empty() {
                            head.push_str(" [not-restricted]");
                        } else if !flag {
                            head.push_str(&format!(" [+restricted]: {t}"));
                        } else {
                            head.push_str(&format!(": {t}"));
                        }
                    }
                    RestrictionStyle::None => {
                        if shown.is_empty() && flag {
                            head.push_str(&format!(" [none]: {t}"));
                        } else {
                            head.push_str(&format!(": {t}"));
                        }
                    }
                }
                *cmp = auto_append(cmp, &head, " -- ");
            }
        }
    }
}

/// How a restriction block words an item with nothing listed.
#[derive(Clone, Copy)]
enum RestrictionStyle {
    /// Classes and races: "[not-restricted]" and "[+restricted]".
    Restricted,
    /// Negated spells: "[none]".
    None,
}
