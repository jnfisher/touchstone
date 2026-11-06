//! List-row builders (FAM-21 without monsters). Card P3-28.
//!
//! MME shows weapons, armour, spells and the rest as `ListView` rows that its `Add*2LV`
//! procedures build from the record the table cursor is on, and a few passes then colour or tag
//! the finished list. Each function here builds the [`Row`] one procedure would add for one
//! record, so a UI can show exactly the cells MME shows.
//!
//! # Rows
//!
//! A row is the `ListItem`'s `Text` and `Tag` plus its `ListSubItems`; `ListSubItems(i)` is
//! `cells[i - 1]`. Texts are what VB6 stores in a String property (`CStr` of what the procedure
//! assigns); a tag is `CStr` of the Variant assigned and `None` where nothing was assigned. A
//! cell's colour is set only where MME assigns one ([`race_color_code`]).
//!
//! A builder returns `None` where MME skips the record (an empty name, the `sdf` placeholder
//! records) and, where a VB6 run-time error stops a procedure part way, the row built so far:
//! the procedures run under `On Error ... Resume out`, so MME leaves the half-built row in the
//! list.
//!
//! # The frmMain controls
//!
//! The procedures read a few controls directly: [`WeaponOptions`] and [`SpellOptions`] stand for
//! them. The character comes in as the profile `PopulateCharacterProfile` made;
//! [`no_character_profile`] builds the profile of "no character" (the global filter off) and
//! [`party_profile`] the one of a lair party, which is not a loaded character.
//!
//! Not ported: `InvenAddEquip` (the `AddToInven` argument; it only touches the inventory list)
//! and the dark-mode colour map `TColor`, which the colours here are the light-theme values of.

pub mod monster;

use std::cell::Cell as StdCell;

use touchstone_model::{Class, Item, Race, Shop, Spell, World};
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Vb6Error};

use crate::Profile;
use crate::ability_text::{AbilityEnv, NameLookup, get_spell_name};
use crate::attack::{self, AttackEnv, AttackRequest, EquippedGlobals};
use crate::character::{
    AttackTypeMud, CharStats, Character, CharacterProfile, Context, PartyFilter,
    populate_character_profile,
};
use crate::spell_cast::{CastRequest, SpellCastEnv};
use crate::spells::{PullArgs, SpellText};
use crate::stats::{get_enc_ratio, get_magery_enum};
use crate::textblock::TextblockData;

type Res<T> = Result<T, Vb6Error>;

/// `ListSubItem`: Text, Tag and ForeColor.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cell {
    pub text: String,
    /// `None`: never assigned.
    pub tag: Option<String>,
    /// `None`: never assigned (the control's own colour).
    pub fore_color: Option<i32>,
}

impl Cell {
    fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            tag: None,
            fore_color: None,
        }
    }
}

/// `ListItem` with its sub-items.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Row {
    pub text: String,
    pub tag: Option<String>,
    /// `ListItem.ForeColor`; `None`: never assigned.
    pub fore_color: Option<i32>,
    pub cells: Vec<Cell>,
}

/// The tables, globals and form values the row builders read besides the record and character.
pub struct RowEnv<'a> {
    pub profile: Profile,
    /// Items, spells, classes and races (lookups by number).
    pub world: &'a World,
    /// `GetSpellName`, `GetMonsterName`, `GetItemName` and `GetClassName`.
    pub names: &'a dyn NameLookup,
    /// `TBInfo`, for the cast-textblock ability.
    pub textblocks: &'a dyn TextblockData,
    /// The weapon-related `nGlobalChar*` globals.
    pub equipped: &'a EquippedGlobals,
    /// `frmMain.lblDatVer.Caption`.
    pub dat_ver_caption: &'a str,
    /// `gAvgLevelMaxAllStats`.
    pub avg_level_max_all_stats: f64,
    /// `frmMain.lblInvenCharStat(10).Tag`.
    pub accuracy_tag: &'a str,
    /// `txtGlobalLevel(0).Text` while `chkGlobalFilter` is on.
    pub global_level_text: Option<&'a str>,
    /// `bHideRecordNumbers`: names inside ability text print without their record numbers.
    pub hide_record_numbers: bool,
}

impl<'a> RowEnv<'a> {
    /// The environment over a loaded database, which answers the name and textblock lookups.
    #[must_use]
    pub fn for_world(world: &'a World, profile: Profile, equipped: &'a EquippedGlobals) -> Self {
        Self {
            profile,
            world,
            names: world,
            textblocks: world,
            equipped,
            dat_ver_caption: "",
            avg_level_max_all_stats: 0.0,
            accuracy_tag: "",
            global_level_text: None,
            hide_record_numbers: false,
        }
    }

    fn ability_env(&self, quick_spell: bool) -> AbilityEnv<'a> {
        AbilityEnv {
            profile: self.profile,
            names: self.names,
            textblocks: self.textblocks,
            spells: &self.world.spells,
            hide_record_numbers: self.hide_record_numbers,
            quick_spell,
            global_level_text: self.global_level_text,
            avg_level_max_all_stats: self.avg_level_max_all_stats,
            nest: StdCell::new(0),
        }
    }
}

/// The `frmMain` controls `AddWeapon2LV` reads.
#[derive(Debug, Clone, Default)]
pub struct WeaponOptions {
    /// `chkGlobalFilter.Value = 1`.
    pub global_filter: bool,
    /// `chkWeaponOptions(3).Value = 1`: calculate combat values.
    pub calc_combat: bool,
    /// `chkWeaponOptions(4).Value = 1`: the 85% speed adjustment.
    pub slow_85: bool,
    /// `cmbWeaponCombos(1).ItemData(ListIndex)`, read when the caller gives no attack kind. `None`
    /// when nothing is selected, where VB6 raises an error and MME adds no row.
    pub combo_mode: Option<i32>,
    /// `txtWeaponExtras(2..4).Text`: versus AC, DR and dodge.
    pub extras: [String; 3],
}

/// The arguments of `AddWeapon2LV` after the list and the profile.
#[derive(Debug, Clone, Copy, Default)]
pub struct WeaponArgs {
    /// `nAbility`: an ability number to show (43 shows the cast spell), -1 for the attack's name.
    pub ability: i16,
    /// `nAttackTypeMUD`; 0 for "the form's choice", -4 for the surprise punch.
    pub attack_type: i32,
    /// `bForceCalc`.
    pub force_calc: bool,
}

/// The `frmMain` controls `AddSpell2LV` reads.
#[derive(Debug, Clone, Default)]
pub struct SpellOptions {
    /// `chkGlobalFilter.Value = 1`.
    pub global_filter: bool,
    /// `txtGlobalLevel(1).Text`.
    pub global_level_text: String,
    /// `chkSpellOptions(0).Value = 1`: calculate combat values.
    pub calc_combat: bool,
    /// `chkSpellOptions(2).Value = 1`: the target is anti-magic.
    pub anti_magic: bool,
    /// `txtSpellOptions(0).Text`: the target's magic resistance.
    pub vs_mr_text: String,
}

/// What `AddSpell2LV` adds: the row, and the entry it puts into each `cmbCharBless(0..9)` when
/// asked to and the spell can be a bless.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellRow {
    pub row: Row,
    /// The `AddItem` text and its `ItemData`.
    pub bless_entry: Option<(String, i32)>,
}

// ---------------------------------------------------------------------------------------------
// Text helpers

fn labelled<T>(n: i32, from: Option<T>, label: impl Fn(T) -> &'static str) -> String {
    from.map_or_else(|| format!("Unknown ({n})"), |v| label(v).to_string())
}

fn to_i16(v: i32) -> Res<i16> {
    i16::try_from(v).map_err(|_| Vb6Error::Overflow)
}

/// `CStr` of a Currency: whole units, then the fraction without trailing zeros.
fn cur_text(c: Currency) -> String {
    let n = c.scaled();
    let sign = if n < 0 { "-" } else { "" };
    let n = n.unsigned_abs();
    let (whole, frac) = (n / 10_000, n % 10_000);
    if frac == 0 {
        format!("{sign}{whole}")
    } else {
        format!(
            "{sign}{whole}.{}",
            format!("{frac:04}").trim_end_matches('0')
        )
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

fn ac_dr_text(ac: i32, dr: i32) -> String {
    format!(
        "{}/{}",
        vb::cstr(round_up(f64::from(ac) / 10.0)),
        vb::cstr(f64::from(dr) / 10.0)
    )
}

pub(crate) fn weapon_type_text(code: i32) -> String {
    labelled(code, touchstone_model::WeaponType::from_code(code), |v| {
        v.label()
    })
}

pub(crate) fn armour_type_text(code: i32) -> String {
    labelled(code, touchstone_model::ArmourType::from_code(code), |v| {
        v.label()
    })
}

pub(crate) fn worn_text(code: i32) -> String {
    labelled(code, touchstone_model::WornSlot::from_code(code), |v| {
        v.label()
    })
}

fn class_weapon_text(code: i32) -> String {
    labelled(
        code,
        touchstone_model::ClassWeaponType::from_code(code),
        |v| v.label(),
    )
}

fn shop_type_text(code: i32) -> String {
    labelled(code, touchstone_model::ShopType::from_code(code), |v| {
        v.label()
    })
}

/// GetItemTypeEnum answers with the bare number for a code it does not know.
fn item_type_text(code: i32) -> String {
    touchstone_model::ItemType::from_code(code)
        .map_or_else(|| code.to_string(), |v| v.label().to_string())
}

// ---------------------------------------------------------------------------------------------
// AddOtherItem2LV, AddShop2LV, AddRace2LV, AddClass2LV

/// Port of modMain AddOtherItem2LV.
#[must_use]
pub fn add_other_item_row(item: &Item) -> Option<Row> {
    if item.name.is_empty() {
        return None;
    }
    Some(Row {
        text: item.number.to_string(),
        tag: None,
        fore_color: None,
        cells: vec![
            Cell::new(item.name.clone()),
            Cell::new(item_type_text(item.item_type)),
            Cell::new(item.encum.to_string()),
            Cell::new(item.limit.to_string()),
        ],
    })
}

/// Port of modMain AddShop2LV.
#[must_use]
pub fn add_shop_row(shop: &Shop) -> Option<Row> {
    let name = &shop.name;
    if name.is_empty()
        || name.chars().take(3).collect::<String>().to_lowercase() == "sdf"
        || name == "Leave this blank"
    {
        return None;
    }
    Some(Row {
        text: shop.number.to_string(),
        tag: None,
        fore_color: None,
        cells: vec![
            Cell::new(name.clone()),
            Cell::new(shop_type_text(shop.shop_type)),
        ],
    })
}

/// The ability list of a class or race row: each name with its value, joined by ", ".
fn ability_list(
    env: &RowEnv,
    slots: &[touchstone_model::AbilitySlot],
    skip_class_ok: bool,
) -> Res<String> {
    let abilities = env.ability_env(false);
    let mut text = String::new();
    for slot in slots {
        if slot.ability == 0 || (skip_class_ok && slot.ability == 59) {
            continue;
        }
        if !text.is_empty() {
            text.push_str(", ");
        }
        text.push_str(&abilities.get_ability_stats(
            to_i16(slot.ability)?,
            to_i16(slot.value)?,
            true,
            false,
        ));
        if text.ends_with(", ") {
            text.truncate(text.len() - 2);
        }
    }
    Ok(text)
}

/// Port of modMain AddRace2LV.
#[must_use]
pub fn add_race_row(env: &RowEnv, race: &Race) -> Option<Row> {
    if race.name.is_empty() {
        return None;
    }
    let mut row = Row {
        text: race.number.to_string(),
        tag: None,
        fore_color: None,
        cells: vec![
            Cell::new(race.name.clone()),
            Cell::new(format!("{}%", race.exp_table)),
            Cell::new(race.hp_per_lvl.to_string()),
        ],
    };
    for (lo, hi) in [
        (race.m_str, race.x_str),
        (race.m_int, race.x_int),
        (race.m_wil, race.x_wil),
        (race.m_agl, race.x_agl),
        (race.m_hea, race.x_hea),
        (race.m_chm, race.x_chm),
    ] {
        row.cells.push(Cell::new(format!("{lo}-{hi}")));
    }
    if let Ok(abilities) = ability_list(env, &race.abilities, false) {
        row.cells.push(Cell::new(abilities));
    }
    Some(row)
}

/// Port of modMain AddClass2LV.
#[must_use]
pub fn add_class_row(env: &RowEnv, class: &Class) -> Option<Row> {
    if class.name.is_empty() {
        return None;
    }
    let mut row = Row {
        text: class.number.to_string(),
        tag: None,
        fore_color: None,
        cells: vec![Cell::new(class.name.clone())],
    };
    let _ = class_steps(env, class, &mut row);
    Some(row)
}

fn class_steps(env: &RowEnv, class: &Class, row: &mut Row) -> Res<()> {
    row.cells.push(Cell::new(format!(
        "{}%",
        vb::cstr(f64::from(class.exp_table) + 100.0)
    )));
    row.cells
        .push(Cell::new(class_weapon_text(class.weapon_type)));
    row.cells
        .push(Cell::new(armour_type_text(class.armour_type)));
    row.cells.push(Cell::new(get_magery_enum(
        to_i16(class.magery_type)?,
        to_i16(class.magery_lvl)?,
    )));
    row.cells
        .push(Cell::new(vb::sub_i32(class.combat_lvl, 2)?.to_string()));
    row.cells.push(Cell::new(format!(
        "{}-{}",
        class.min_hits,
        vb::add_i32(class.min_hits, class.max_hits)?
    )));
    row.cells
        .push(Cell::new(ability_list(env, &class.abilities, true)?));
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// AddArmour2LV and the AC/DR sort tags

/// Port of modMain AddArmour2LV (without `AddToInven`). `ability` is `nAbility`: an ability
/// number whose value the last cell shows.
#[must_use]
pub fn add_armour_row(item: &Item, ability: i16) -> Option<Row> {
    if item.name.is_empty() {
        return None;
    }
    let mut row = Row {
        text: item.number.to_string(),
        tag: None,
        fore_color: None,
        cells: Vec::new(),
    };
    let _ = armour_steps(item, ability, &mut row);
    Some(row)
}

fn armour_steps(item: &Item, ability: i16, row: &mut Row) -> Res<()> {
    row.cells.push(Cell::new(item.name.clone()));
    let mut worn = Cell::new(worn_text(item.worn));
    worn.tag = Some(item.worn.to_string());
    row.cells.push(worn);
    row.cells
        .push(Cell::new(armour_type_text(item.armour_type)));
    row.cells.push(Cell::new("0"));
    row.cells.push(Cell::new(item.encum.to_string()));
    row.cells.push(Cell::new(format!(
        "{}/{}",
        vb::cstr(f64::from(item.armour_class) / 10.0),
        vb::cstr(f64::from(item.damage_resist) / 10.0)
    )));
    // the tag is assigned after the cell exists, so an overflow leaves the cell without it
    let tag = vb::add_i32(vb::mul_i32(item.armour_class, 100_000)?, item.damage_resist)?;
    row.cells[5].tag = Some(tag.to_string());
    row.cells.push(Cell::new(item.accy.to_string()));
    row.cells.push(Cell::new("0"));
    row.cells.push(Cell::new(item.limit.to_string()));
    let ratio = get_enc_ratio(item.encum, item.armour_class, item.damage_resist)
        .map_err(|_| Vb6Error::Overflow)?;
    row.cells.push(Cell::new(cur_text(ratio)));

    let mut ability_val: i16 = 0;
    for slot in &item.abilities {
        match slot.ability {
            58 => row.cells[7].text = slot.value.to_string(),
            135 => row.cells[3].text = slot.value.to_string(),
            22 | 105 | 106 => {
                row.cells[6].text = vb::cstr(vb::val(&row.cells[6].text) + f64::from(slot.value));
            }
            _ => {}
        }
        if ability > 0 && slot.ability == i32::from(ability) {
            ability_val = to_i16(slot.value)?;
        }
    }
    row.cells.push(Cell::new(if ability > 0 {
        ability_val.to_string()
    } else {
        String::new()
    }));
    Ok(())
}

/// The AC and DR (times ten) in an armour row's "AC/DR" cell, or `None` where the cell does not
/// split into two parts. A cell that is not a number is a VB6 error.
fn ac_dr_parts(row: &Row) -> Res<Option<(i32, i32)>> {
    let cell = row.cells.get(5).ok_or(Vb6Error::Overflow)?;
    let parts: Vec<&str> = cell.text.split('/').collect();
    if parts.len() != 2 {
        return Ok(None);
    }
    let ac = vb::clng(vb::cdbl_str(parts[0])? * 10.0)?;
    let dr = vb::clng(vb::cdbl_str(parts[1])? * 10.0)?;
    Ok(Some((ac, dr)))
}

/// Port of modMain SetArmourACDRSortTags: the sort tag of every armour row's "AC/DR" cell, AC
/// first (or DR first with `by_dr`). A VB6 error ends the pass, leaving the earlier rows tagged.
pub fn set_armour_ac_dr_sort_tags(rows: &mut [Row], by_dr: bool) {
    let _ = sort_tag_steps(rows, by_dr);
}

fn sort_tag_steps(rows: &mut [Row], by_dr: bool) -> Res<()> {
    for row in rows {
        if let Some((ac, dr)) = ac_dr_parts(row)? {
            let (first, second) = if by_dr { (dr, ac) } else { (ac, dr) };
            let tag = vb::add_i32(vb::mul_i32(first, 100_000)?, second)?;
            row.cells[5].tag = Some(tag.to_string());
        }
    }
    Ok(())
}

/// Port of modMain ArmourACDRTagsAreByDR: which of the two orders the sort tags were built for,
/// judged by the first row whose AC and DR differ. False when no row can tell (or on a VB6 error).
#[must_use]
pub fn armour_ac_dr_tags_are_by_dr(rows: &[Row]) -> bool {
    tags_are_by_dr_steps(rows).unwrap_or(false)
}

fn tags_are_by_dr_steps(rows: &[Row]) -> Res<bool> {
    for row in rows {
        if let Some((ac, dr)) = ac_dr_parts(row)?
            && ac != dr
        {
            let by_dr = vb::add_i32(vb::mul_i32(dr, 100_000)?, ac)?;
            let tag = row.cells[5].tag.as_deref().unwrap_or("");
            return Ok(vb::val(tag) == f64::from(by_dr));
        }
    }
    Ok(false)
}

// ---------------------------------------------------------------------------------------------
// RaceColorCode

/// `ForeColor` of a stat range well below the average (`&H80&`) and well above it (`&H8000&`),
/// as the light theme shows them.
const BELOW_AVERAGE: i32 = 0x80;
const ABOVE_AVERAGE: i32 = 0x8000;

/// Port of modMain RaceColorCode: colours each race row's six stat-range cells red or green when
/// the race's min plus max is more than 20 under or over the average of the listed races.
pub fn race_color_code(world: &World, rows: &mut [Row]) {
    let _ = race_color_steps(world, rows);
}

fn race_of<'a>(world: &'a World, row: &Row) -> Option<&'a Race> {
    let n = vb::val(&row.text);
    if n.fract() != 0.0 || n.abs() > f64::from(i32::MAX) {
        return None;
    }
    world.get_race(n as i32)
}

fn race_stats(r: &Race) -> ([i32; 6], [i32; 6]) {
    (
        [r.m_str, r.m_int, r.m_wil, r.m_agl, r.m_hea, r.m_chm],
        [r.x_str, r.x_int, r.x_wil, r.x_agl, r.x_hea, r.x_chm],
    )
}

fn race_color_steps(world: &World, rows: &mut [Row]) -> Res<()> {
    let (mut min, mut max) = ([0i32; 6], [0i32; 6]);
    let mut races = 0i32;
    for row in rows.iter() {
        if let Some(r) = race_of(world, row) {
            races += 1;
            let (lo, hi) = race_stats(r);
            for x in 0..6 {
                min[x] += lo[x];
                max[x] += hi[x];
            }
        }
    }
    if races == 0 {
        return Ok(());
    }
    let mut total = [0i32; 6];
    for x in 0..6 {
        let avg_min = i32::from(vb::cint(f64::from(min[x]) / f64::from(races))?);
        let avg_max = i32::from(vb::cint(f64::from(max[x]) / f64::from(races))?);
        total[x] = avg_min + avg_max;
    }
    for row in rows.iter_mut() {
        let Some(r) = race_of(world, row) else {
            continue;
        };
        let (lo, hi) = race_stats(r);
        for x in 0..6 {
            let sum = lo[x] + hi[x];
            let color = if sum < total[x] - 20 {
                Some(BELOW_AVERAGE)
            } else if sum > total[x] + 20 {
                Some(ABOVE_AVERAGE)
            } else {
                None
            };
            if let Some(color) = color {
                row.cells
                    .get_mut(3 + x)
                    .ok_or(Vb6Error::Overflow)?
                    .fore_color = Some(color);
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// AddWeapon2LV

/// Port of modMain AddWeapon2LV (without `AddToInven`): the row of one weapon (or, with
/// `args.attack_type == -4`, the surprise punch, which has no record) for the profile `ch`.
///
/// `item` is the record the cursor is on. Returns `None` where MME adds no row.
#[must_use]
pub fn add_weapon_row(
    env: &RowEnv,
    item: Option<&Item>,
    ch: &attack::CharacterProfile,
    opts: &WeaponOptions,
    args: &WeaponArgs,
) -> Option<Row> {
    let mut calc_combat = opts.calc_combat;
    let mut speed_adj: i16 = 100;
    let mut attack_type = args.attack_type;
    if calc_combat {
        if attack_type == 0 {
            attack_type = opts.combo_mode?;
        }
        if opts.slow_85 {
            speed_adj = 85;
        }
    } else if attack_type == 0 {
        attack_type = 5;
    }

    let (name, number) = if attack_type == -4 {
        attack_type = 4;
        calc_combat = true;
        ("Punch (Surprise)".to_string(), 0)
    } else {
        let it = item?;
        if it.name.is_empty() {
            return None;
        }
        (it.name.clone(), it.number)
    };
    let mut row = Row {
        text: number.to_string(),
        tag: Some(attack_type.to_string()),
        fore_color: None,
        cells: Vec::new(),
    };
    let record = if number > 0 { item } else { None };
    let _ = weapon_steps(
        env,
        record,
        ch,
        opts,
        args,
        (&name, number, attack_type, speed_adj, calc_combat),
        &mut row,
    );
    Some(row)
}

/// `CalculateAttack` as `FilterWeapons` calls it (the damage check and the maximum-effort lists), with the
/// same tables and globals `AddWeapon2LV` gives it. `casts` is the ByRef `sCasts`.
#[must_use]
pub fn calculate_weapon_attack(
    env: &RowEnv,
    ch: &attack::CharacterProfile,
    req: &AttackRequest,
    casts: &mut String,
) -> attack::AttackDamage {
    let abilities = env.ability_env(false);
    let attack_env = AttackEnv {
        items: &env.world.items,
        classes: &env.world.classes,
        races: &env.world.races,
        equipped: env.equipped,
        cast_text: &abilities,
        dat_ver_caption: env.dat_ver_caption,
        avg_level_max_all_stats: env.avg_level_max_all_stats,
        accuracy_tag: env.accuracy_tag,
    };
    attack::calculate_attack(env.profile, &attack_env, ch, req, casts)
}

#[allow(clippy::too_many_lines)] // one MME procedure, kept in its order
fn weapon_steps(
    env: &RowEnv,
    item: Option<&Item>,
    ch: &attack::CharacterProfile,
    opts: &WeaponOptions,
    args: &WeaponArgs,
    (name, number, attack_type, speed_adj, calc_combat): (&str, i32, i32, i16, bool),
    row: &mut Row,
) -> Res<()> {
    let extra = |k: usize| -> Res<i32> {
        vb::clng(if calc_combat {
            vb::val(&opts.extras[k])
        } else {
            0.0
        })
    };
    let req = AttackRequest {
        attack_type,
        weapon_number: number,
        speed_adj,
        vs_ac: extra(0)?,
        vs_dr: extra(1)?,
        vs_dodge: extra(2)?,
        force_calc: args.force_calc,
        ..AttackRequest::default()
    };
    let abilities = env.ability_env(false);
    let attack_env = AttackEnv {
        items: &env.world.items,
        classes: &env.world.classes,
        races: &env.world.races,
        equipped: env.equipped,
        cast_text: &abilities,
        dat_ver_caption: env.dat_ver_caption,
        avg_level_max_all_stats: env.avg_level_max_all_stats,
        accuracy_tag: env.accuracy_tag,
    };
    let mut casts = String::new();
    let dmg = attack::calculate_attack(env.profile, &attack_env, ch, &req, &mut casts);

    let cells = &mut row.cells;
    cells.push(Cell::new(name));
    cells.push(Cell::new(match item {
        Some(it) => weapon_type_text(it.weapon_type),
        None => "Fists".to_string(),
    }));
    let record_or = |f: fn(&Item) -> i32, absent: i32| item.map_or(absent, f);
    cells.push(Cell::new(
        if calc_combat {
            dmg.min_dmg
        } else {
            record_or(|i| i.min, 0)
        }
        .to_string(),
    ));
    cells.push(Cell::new(
        if calc_combat {
            dmg.max_dmg
        } else {
            record_or(|i| i.max, 0)
        }
        .to_string(),
    ));
    cells.push(Cell::new(
        if dmg.attack_speed > 0 {
            i32::from(dmg.attack_speed)
        } else {
            record_or(|i| i.speed, 1150)
        }
        .to_string(),
    ));
    cells.push(Cell::new("0"));
    cells.push(Cell::new(record_or(|i| i.str_req, 0).to_string()));
    cells.push(Cell::new(record_or(|i| i.encum, 0).to_string()));
    cells.push(Cell::new(item.map_or_else(
        || "0/0".to_string(),
        |i| ac_dr_text(i.armour_class, i.damage_resist),
    )));
    cells.push(Cell::new("0"));
    cells.push(Cell::new(if number > 0 { "No" } else { "0" }));
    cells.push(Cell::new("0"));
    cells.push(Cell::new(record_or(|i| i.limit, 0).to_string()));

    let mut ability_val: i16 = 0;
    let mut speed: i16 = 0;
    if let Some(it) = item {
        for slot in &it.abilities {
            match slot.ability {
                58 => cells[11].text = slot.value.to_string(),
                22 | 105 | 106 => cells[9].text = slot.value.to_string(),
                135 => cells[5].text = slot.value.to_string(),
                116 => cells[10].text = slot.value.to_string(),
                _ => {}
            }
            if args.ability > 0 && slot.ability == i32::from(args.ability) {
                ability_val = to_i16(slot.value)?;
            }
        }
        cells[9].text = vb::cstr(vb::val(&cells[9].text) + f64::from(it.accy));
        speed = to_i16(it.speed)?;
    }

    // a backstab (the surprise attack) never gets a damage-per-speed figure
    cells.push(Cell::new(
        if attack_type != 4 && speed > 0 && dmg.round_total > 0 && dmg.swings > 0.0 {
            let per_speed = vb::round(
                f64::from(dmg.round_total) / dmg.swings / f64::from(speed),
                4,
            )?;
            vb::cstr(per_speed * 1000.0)
        } else {
            "0".to_string()
        },
    ));
    cells.push(Cell::new(vb::cstr(truncate(dmg.swings, 2))));
    cells.push(Cell::new(
        if attack_type == 4 {
            dmg.avg_hit
        } else {
            dmg.round_physical
        }
        .to_string(),
    ));
    cells.push(Cell::new(
        if dmg.avg_extra_swing > 0 && dmg.swings > 0.0 && dmg.hit_chance > 0 {
            vb::cstr(vb::round(
                f64::from(dmg.avg_extra_swing) * dmg.swings * (f64::from(dmg.hit_chance) / 100.0),
                0,
            )?)
        } else {
            "0".to_string()
        },
    ));
    cells.push(Cell::new(dmg.round_total.to_string()));
    let mut first = Cell::new(dmg.first_round_damage.to_string());
    first.tag = Some(vb::cstr(
        f64::from(dmg.first_round_damage) + vb::round(f64::from(dmg.round_total) / 100.0, 2)?,
    ));
    cells.push(first);

    let text = if args.ability > 0 {
        if args.ability == 43 {
            let spell_name = get_spell_name(env.names, i32::from(ability_val), true);
            let effect = abilities.pull_spell_eq(&PullArgs {
                calc_level: opts.global_filter,
                level: to_i16(ch.level)?,
                spell: i32::from(ability_val),
                no_show_level: true,
                spell_bonus: ch.spell_dmg_bonus,
                ..PullArgs::default()
            });
            format!("{spell_name}: {effect}")
        } else {
            ability_val.to_string()
        }
    } else if args.ability == -1 {
        match attack_type {
            1 => "Punch",
            2 => "Kick",
            3 => "Jumpkick",
            4 => "Backstab",
            5 => "Normal",
            6 => "Bash",
            7 => "Smash",
            _ => "",
        }
        .to_string()
    } else if attack_type == 4 && number == 0 {
        let sum = vb::add_i32(
            vb::add_i32(dmg.min_dmg, dmg.max_dmg)?,
            vb::mul_i32(dmg.avg_extra_swing, 2)?,
        )?;
        let avg = vb::idiv(f64::from(sum), 2.0)?;
        format!("{}% hit, Avg Hit: {avg}", dmg.hit_chance)
    } else {
        String::new()
    };
    cells.push(Cell::new(text));
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// AddSpell2LV

/// `sArea`: the marker for spells that hit an area.
fn area_marker(targets: i32) -> &'static str {
    if matches!(targets, 3 | 5 | 9..=13) {
        "^"
    } else {
        ""
    }
}

/// Port of modMain AddSpell2LV: the row of one spell for the profile `ch`, and with `add_bless`
/// the entry for the bless lists when the spell is one a character can use as a bless.
///
/// Returns `None` where MME adds no row (no name, a name starting with "1" or "sdf").
#[must_use]
pub fn add_spell_row(
    env: &RowEnv,
    spell: &Spell,
    ch: &CharacterProfile,
    opts: &SpellOptions,
    add_bless: bool,
) -> Option<SpellRow> {
    // the cast level is worked out before the row exists, so an overflow there adds no row
    let setup = spell_setup(spell, ch, opts).ok()?;
    let name = spell.name.clone().unwrap_or_default();
    if name.is_empty()
        || name.starts_with('1')
        || name.chars().take(3).collect::<String>().to_lowercase() == "sdf"
    {
        return None;
    }
    let mut out = SpellRow {
        row: Row {
            text: spell.number.to_string(),
            tag: None,
            fore_color: None,
            cells: Vec::new(),
        },
        bless_entry: None,
    };
    let _ = spell_steps(env, spell, &name, ch, opts, setup, add_bless, &mut out);
    Some(out)
}

/// Whether the character's values apply, whether combat is calculated, and the level to cast at.
struct SpellSetup {
    use_character: bool,
    calc_combat: bool,
    cast_level: i32,
}

fn spell_setup(sp: &Spell, ch: &CharacterProfile, opts: &SpellOptions) -> Res<SpellSetup> {
    let level_box = vb::val(&opts.global_level_text);
    let mut use_character = false;
    let mut calc_combat = false;
    if (ch.level == 0 || ch.is_loaded_character) && opts.global_filter && level_box > 0.0 {
        use_character = true;
        if opts.calc_combat && vb::val(&opts.vs_mr_text) > 0.0 {
            calc_combat = true;
        }
    }
    let mut cast_level = i32::from(to_i16(ch.level)?);
    if cast_level == 0 {
        cast_level = if use_character && level_box > 0.0 {
            i32::from(vb::cint(level_box)?)
        } else if sp.cap > sp.req_level {
            sp.cap
        } else {
            sp.req_level
        };
    }
    Ok(SpellSetup {
        use_character,
        calc_combat,
        cast_level,
    })
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments)] // one MME procedure, kept in its order
fn spell_steps(
    env: &RowEnv,
    sp: &Spell,
    name: &str,
    ch: &CharacterProfile,
    opts: &SpellOptions,
    setup: SpellSetup,
    add_bless: bool,
    out: &mut SpellRow,
) -> Res<()> {
    let SpellSetup {
        use_character,
        calc_combat,
        cast_level,
    } = setup;

    let cells = &mut out.row.cells;
    cells.push(Cell::new(name));
    cells.push(Cell::new(sp.short.clone().unwrap_or_default()));
    cells.push(Cell::new(get_magery_enum(
        to_i16(sp.magery)?,
        to_i16(sp.magery_lvl)?,
    )));
    cells.push(Cell::new(sp.req_level.to_string()));

    let abilities = env.ability_env(false);
    let cast_env = SpellCastEnv {
        profile: env.profile,
        spells: &env.world.spells,
        text: &abilities as &dyn SpellText,
        avg_level_max_all_stats: env.avg_level_max_all_stats,
    };
    let who = ch.spell_caster();
    let cast = if calc_combat {
        cast_env.calculate_spell_cast(
            &who,
            &CastRequest {
                spell: sp.number,
                cast_level,
                vs_mr: vb::clng(vb::val(&opts.vs_mr_text))?,
                vs_anti_magic: opts.anti_magic,
                ..CastRequest::default()
            },
        )
    } else {
        cast_env.calculate_spell_cast(
            &who,
            &CastRequest {
                spell: sp.number,
                cast_level,
                ..CastRequest::default()
            },
        )
    };

    let mut mana_cost = 0;
    if sp.mana_cost > 0 {
        mana_cost = if cast.n_num_casts > 1.0 {
            vb::clng(f64::from(sp.mana_cost) * cast.n_num_casts)?
        } else {
            sp.mana_cost
        };
    }
    cells.push(Cell::new(mana_cost.to_string()));

    cells.push(Cell::new(if use_character {
        format!("{}%", cast.n_cast_chance)
    } else {
        sp.diff.to_string()
    }));

    let area = area_marker(sp.targets);
    let duration = i32::from(cast.n_duration);
    let per_mana = |amount: f64, whole: i32| -> Res<Currency> {
        let per = if cast.n_num_casts > 1.0 {
            amount / f64::from(mana_cost)
        } else {
            f64::from(whole) / f64::from(sp.mana_cost)
        };
        Currency::from_f64(vb::round(per, 1)?)
    };
    if sp.learnable || sp.mana_cost > 0 {
        let mut use_val;
        if duration > 1
            && cast.n_avg_round_dmg > 0
            && cast.n_avg_round_dmg == cast.n_min_cast
            && cast.n_max_cast > cast.n_min_cast
        {
            use_val = vb::clng(vb::round(
                f64::from(vb::add_i32(
                    vb::mul_i32(cast.n_min_cast, duration)?,
                    vb::mul_i32(cast.n_max_cast, duration)?,
                )?) / 2.0,
                0,
            )?)?;
        } else {
            use_val = vb::clng(vb::round(
                f64::from(vb::mul_i32(cast.n_avg_round_dmg, duration)?),
                0,
            )?)?;
        }
        cells.push(Cell::new(format!(
            "{use_val}{}",
            if cast.n_avg_round_dmg > 0 { area } else { "" }
        )));
        let mut per = Currency::from_scaled(0);
        if cast.n_avg_round_dmg > 0 && sp.mana_cost > 0 {
            per = per_mana(f64::from(cast.n_avg_round_dmg), use_val)?;
        }
        cells.push(Cell::new(cur_text(per)));

        if duration > 1
            && cast.n_avg_round_heals > 0
            && cast.n_avg_round_heals == cast.n_min_cast
            && cast.n_max_cast > cast.n_min_cast
        {
            use_val = vb::clng(vb::round(
                f64::from(vb::add_i32(
                    vb::mul_i32(cast.n_min_cast, duration)?,
                    vb::mul_i32(cast.n_max_cast, duration)?,
                )?) / 2.0,
                0,
            )?)?;
        } else {
            use_val = vb::mul_i32(cast.n_avg_round_heals, duration)?;
        }
        cells.push(Cell::new(format!(
            "{use_val}{}",
            if cast.n_avg_round_heals > 0 { area } else { "" }
        )));
        let mut per = Currency::from_scaled(0);
        if cast.n_avg_round_heals != 0 && sp.mana_cost > 0 {
            per = per_mana(f64::from(cast.n_avg_round_heals), use_val)?;
        }
        cells.push(Cell::new(cur_text(per)));
    } else {
        for _ in 0..4 {
            cells.push(Cell::new("0"));
        }
    }

    let quick = env.ability_env(true);
    let detail = if cast_level > 0 {
        quick.pull_spell_eq(&PullArgs {
            calc_level: true,
            level: to_i16(cast_level)?,
            spell: sp.number,
            no_show_level: true,
            override_min: cast.n_min_cast,
            override_max: cast.n_max_cast,
            ..PullArgs::default()
        })
    } else {
        quick.pull_spell_eq(&PullArgs {
            calc_level: false,
            spell: sp.number,
            no_show_level: true,
            ..PullArgs::default()
        })
    };
    cells.push(Cell::new(detail));

    if add_bless
        && (sp.learnable
            || sp.learned_from.as_deref().is_some_and(|s| !s.is_empty())
            || (sp.magery == 5 && sp.req_level > 0))
        && matches!(sp.targets, 1 | 2 | 5 | 13)
        && (sp.dur > 0 || (sp.dur_inc > 0 && sp.dur_inc_lvls > 0))
    {
        out.bless_entry = Some((format!("{name} ({})", sp.number), sp.number));
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// The profile of "no character"

/// What `PopulateCharacterProfile` leaves for "no character": the global filter off, so the
/// profile is the best any character could be (maximum level, strength, agility and stealth).
/// `mud` is the kind of attack the profile is for.
#[must_use]
pub fn no_character_profile(
    profile: Profile,
    world: &World,
    nmr_ver: f64,
    avg_level_max_all_stats: f64,
    mud: AttackTypeMud,
) -> CharacterProfile {
    unloaded_profile(
        profile,
        world,
        nmr_ver,
        avg_level_max_all_stats,
        &PartyFilter::default(),
        false,
        mud,
    )
}

/// The profile for a lair party of two or more, called with the global filter on and without
/// `bForceNoParty`: HP, regeneration and accuracy come from the lair controls, and the level
/// stays 0 (not a loaded character), which makes `AddSpell2LV` take its level from
/// `txtGlobalLevel(1)`. A party size under two gives the profile of a loaded character, which
/// needs the character state ([`crate::pipeline::CharacterState::profile_for`]).
#[must_use]
pub fn party_profile(
    profile: Profile,
    world: &World,
    nmr_ver: f64,
    avg_level_max_all_stats: f64,
    party: &PartyFilter,
    mud: AttackTypeMud,
) -> CharacterProfile {
    unloaded_profile(
        profile,
        world,
        nmr_ver,
        avg_level_max_all_stats,
        party,
        true,
        mud,
    )
}

fn unloaded_profile(
    profile: Profile,
    world: &World,
    nmr_ver: f64,
    avg_level_max_all_stats: f64,
    party: &PartyFilter,
    global_filter: bool,
    mud: AttackTypeMud,
) -> CharacterProfile {
    struct NoText;
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
    let stats = CharStats::default();
    let ctx = Context {
        profile,
        nmr_ver,
        avg_level_max_all_stats,
        hide_record_numbers: false,
        recovery_only: false,
        classes: &world.classes,
        items: &world.items,
        spells: &world.spells,
        text: &NoText,
        stats: &stats,
        party,
    };
    let character = Character {
        global_filter,
        ..Character::default()
    };
    let mut t = CharacterProfile::default();
    // bForceNoParty is False only for the party profile
    populate_character_profile(
        &ctx,
        &character,
        &mut t,
        false,
        !global_filter,
        mud,
        0,
        false,
    );
    t
}
