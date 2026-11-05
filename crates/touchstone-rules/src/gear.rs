//! Gear aggregation I (FAM-04): the first half of frmMain `CalcCharacterStats`. Card P3-17.
//!
//! MME's procedure reads and writes controls throughout; the port takes a [`GearInput`] (what the
//! controls and globals hold) and returns a [`CharStats`] (what `lblInvenCharStat(0..46)`,
//! `txtCharStats(i).Tag` and the `nGlobalChar*` globals hold afterwards).
//!
//! The split with P3-18 is at the section banner `'stealth`: this module ends where the equipped
//! items loop, the hit-magic fold-in and the shadow AC are done, and P3-18 starts at stealth, quest
//! bonuses, accuracy, crits, MR and dodge, which continue on the same [`CharStats`].
//!
//! Conventions of the port:
//!
//! - `lblInvenCharStat(i).Caption` is a String that MME re-reads with `Val` after every update, so
//!   each store goes through `CStr` (15 significant digits) and back ([`CharStats::put`]). `stat[i]`
//!   is that value; the caption text is `vb::cstr(stat[i])`.
//! - Where MME's procedure has an `On Error` handler that leaves the procedure, the port stops at the
//!   same place with the state computed so far and records the error number in
//!   [`CharStats::error`]. `InvenCalcEncum` has its own handler: an Integer overflow in its
//!   arguments leaves the max-encumbrance caption as it was.
//! - Display text is not ported: tool tips (`StatTips`, `ToolTipText`), the colour of `chkEquipHold`
//!   (the "item unusable" red is [`item_is_usable_by_char`] for the caller), `cmbEquip` re-selection
//!   (the equipped numbers are taken as consistent), and the `sSources` text of
//!   `AdjMainStatBonus`.
//! - Only the stock engine is ported (`bGreaterMUD` false): GreaterMUD quest bonuses, shadow AC from
//!   class, race and gear abilities, vile ward and the `+stat` gear abilities are skipped as they
//!   are in MME's stock path.

use touchstone_model::{Item, Spell, World};
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Vb6Error};

use crate::abilities::get_ability_stat_slot;
use crate::spells::{MinMaxRequest, get_current_spell_min_max};
use crate::stats::{calc_encum, get_race_hp_bonus};
use crate::{Profile, RulesError};

type Res<T> = Result<T, Vb6Error>;

/// `TOTAL_STAT_LBLS + 1`: the length of the `lblInvenCharStat` array.
pub const STAT_COUNT: usize = 47;
/// Length of MME's `bless_Stats` array (`bless_Stats(200)`).
pub const BLESS_COUNT: usize = 201;
/// `nEquippedItem(0 To 19)`.
pub const EQUIP_SLOTS: usize = 20;
/// Number of bless slots (`cmbCharBless(0..9)`).
pub const BLESS_SLOTS: usize = 10;

/// `SPELL_ROUND_SECS`: the length of a spell tick, in seconds.
const SPELL_ROUND_SECS: i64 = 3;

/// Stat index of the encumbrance bonus percentage in `lblInvenCharStat`.
const STAT_ENC_BONUS: usize = 4;

fn lbl(e: Vb6Error) -> RulesError {
    RulesError::Vb6(i32::try_from(e.code()).unwrap_or(0))
}

/// `CStr` then `Val`: a number stored to a label caption and read back.
fn caption_round_trip(x: f64) -> f64 {
    vb::val(&vb::cstr(x))
}

fn cur_long(v: i64) -> Currency {
    Currency::from_scaled(v * 10_000)
}

/// Implicit Double -> Long (banker's rounding, error 6 out of range).
fn to_long(x: f64) -> Res<i32> {
    vb::clng(x)
}

/// Long -> Integer (error 6 out of range).
fn to_integer(v: i32) -> Res<i16> {
    i16::try_from(v).map_err(|_| Vb6Error::Overflow)
}

// ---------------------------------------------------------------------------------------------
// Inputs

/// The character fields the global filter controls hold: `txtGlobalLevel(0)`, `cmbGlobalClass(0)`,
/// `cmbGlobalRace(0)`, `cmbGlobalAlignment`, `txtGlobalMinLVL`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Character {
    /// `Val(txtGlobalLevel(0).Text)`.
    pub level: i32,
    /// The selected class's number; 0 is the combo's first entry (`ListIndex` < 1).
    pub class: i32,
    /// The selected race's number; 0 for the first entry.
    pub race: i32,
    /// `cmbGlobalAlignment.ListIndex`: 0 any, 1 good, 2 neutral, 3 evil.
    pub alignment: i32,
    /// `Val(txtGlobalMinLVL.Text)`.
    pub min_item_level: i32,
}

/// One Item Manager row flagged CARRIED, in list order (see [`carried_items`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarriedItem {
    pub item: i32,
    pub qty: i32,
}

/// A row of `lvItemManager`: `ListItems(x).Text` and the texts of `ListSubItems(1..)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemManagerRow {
    pub text: String,
    pub sub_items: Vec<String>,
}

/// Everything `CalcCharacterStats` reads from controls and globals.
#[derive(Debug, Clone)]
pub struct GearInput<'a> {
    pub profile: Profile,
    pub world: &'a World,
    /// `frmMain.lblDatVer.Caption` (CalcEncum's data-version switch).
    pub dat_ver_caption: &'a str,
    pub character: Character,
    /// `Val(txtCharStats(0..5).Text)`: STR INT WIL AGL HEA CHM.
    pub base_stats: [f64; 6],
    /// `chkInvenHideCharStats.Value = 1`: class and race abilities and the rest are left out.
    pub hide_class_race: bool,
    /// `chkInvenAddWeight` checked: `Val(txtInvenAddWeight.Text)`.
    pub add_weight: Option<f64>,
    /// `nEquippedItem(0..19)`: Head, Ears, Neck, Back, Torso, Arms, Wrist, Wrist2, Hands, Finger1,
    /// Finger2, Waist, Legs, Feet, Worn, Off-Hand (15), Weapon (16), Eyes, Face, Everywhere.
    pub equipped: [i32; EQUIP_SLOTS],
    /// Carried Item Manager rows, from [`carried_items`].
    pub carried: Vec<CarriedItem>,
    /// `char_StatAdjustments(0..46)`.
    pub adjustments: [i32; STAT_COUNT],
    /// `cmbCharBless(0..9)` selections: spell numbers, 0 for none.
    pub bless_slots: [i32; BLESS_SLOTS],
    /// `nGlobalAttackTypeMME = a4_MartialArts`: the weapon's hit magic is not used.
    pub martial_arts: bool,
}

// ---------------------------------------------------------------------------------------------
// Output

/// What a weapon (index 0) or off-hand (index 1) slot contributes: `nGlobalCharWeapon*(0..1)`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WeaponGear {
    pub number: i32,
    pub encum: i32,
    pub accy: i32,
    pub crit: i32,
    pub strength: i32,
    pub agility: i32,
    pub max_dmg: i32,
    pub bs_accy: i32,
    pub bs_min_dmg: i32,
    pub bs_max_dmg: i32,
    pub punch_skill: i32,
    pub punch_accy: i32,
    pub punch_dmg: i32,
    pub kick_skill: i32,
    pub kick_accy: i32,
    pub kick_dmg: i32,
    pub jumpkick_skill: i32,
    pub jumpkick_accy: i32,
    pub jumpkick_dmg: i32,
    pub stealth: i32,
}

/// The aggregate stat vector and the globals `CalcCharacterStats` fills.
#[derive(Debug, Clone, PartialEq)]
pub struct CharStats {
    /// `Val(lblInvenCharStat(i).Caption)`. Index meanings are in `docs/inventory.md` section 5b:
    /// 0 current encumbrance, 1 max encumbrance, 2 AC, 3 DR, 4 encumbrance bonus %, ...
    pub stat: [f64; STAT_COUNT],
    /// `Val(txtCharStats(i).Tag)`: the base stat with bless bonuses applied.
    pub stat_with_bonus: [f64; 6],
    /// `Val(lblLabelArray(..).Tag)` per stat: the bless bonus alone.
    pub label_bonus: [f64; 6],
    pub accy_items: i32,
    pub accy_abils: i32,
    pub accy_other: i32,
    pub plus_dodge: i32,
    pub plus_mr: i32,
    pub vile_ward: i32,
    pub shadow_ac: i32,
    /// `nGlobalCharWornArmourType`: the highest `ItemType` among worn non-weapon items.
    pub worn_armour_type: i32,
    pub hit_magic_non_weapon: i32,
    /// `nGlobalCharWeapon*(0)` weapon and `(1)` off-hand.
    pub weapons: [WeaponGear; 2],
    /// Local `nEncumPCT`: encumbrance as a percentage of the maximum, capped at 100.
    pub encum_pct: i32,
    /// Local `nWeaponHitMagic`: the weapon's own hit magic (already folded into stat 12).
    pub weapon_hit_magic: i32,
    /// Locals `bClassStealth` / `bRaceStealth`, consumed by the stealth section.
    pub class_stealth: bool,
    pub race_stealth: bool,
    /// The module-level `bless_Stats` array, as `RefreshCharBless` left it.
    pub bless: Box<[f64; BLESS_COUNT]>,
    /// `nGlobalCharBlessSpells(0..9)`.
    pub bless_spells: [i32; BLESS_SLOTS],
    /// The VB6 run-time error that ended the calculation early, if any.
    pub error: Option<i32>,
}

impl CharStats {
    /// The state `InvenResetStats` leaves: all captions "0", the tags back at the typed stats.
    #[must_use]
    pub fn reset(base_stats: &[f64; 6]) -> Self {
        Self {
            stat: [0.0; STAT_COUNT],
            stat_with_bonus: *base_stats,
            label_bonus: [0.0; 6],
            accy_items: 0,
            accy_abils: 0,
            accy_other: 0,
            plus_dodge: 0,
            plus_mr: 0,
            vile_ward: 0,
            shadow_ac: 0,
            worn_armour_type: 0,
            hit_magic_non_weapon: 0,
            weapons: [WeaponGear::default(); 2],
            encum_pct: 0,
            weapon_hit_magic: 0,
            class_stealth: false,
            race_stealth: false,
            bless: Box::new([0.0; BLESS_COUNT]),
            bless_spells: [0; BLESS_SLOTS],
            error: None,
        }
    }

    /// `lblInvenCharStat(i).Caption` as MME shows it.
    #[must_use]
    pub fn caption(&self, i: usize) -> String {
        vb::cstr(self.stat[i])
    }

    /// `lblInvenCharStat(i).Caption = x`.
    pub fn put(&mut self, i: usize, x: f64) {
        self.stat[i] = caption_round_trip(x);
    }

    /// `lblInvenCharStat(i).Caption = Val(lblInvenCharStat(i).Caption) + x`.
    pub fn add(&mut self, i: usize, x: f64) {
        self.put(i, self.stat[i] + x);
    }

    /// Port of frmMain `AdjMainStatBonus` (display text dropped): adds `value` to the stat's bless
    /// bonus and refreshes the stat's `Tag`. `stat_index` is 0..5; `label_index` is the ability slot
    /// (101 STR, 104 INT, 124 WIL, 102 AGL, 123 HEA, 103 CHM) when the stat is not given.
    pub fn adj_main_stat_bonus(
        &mut self,
        base_stats: &[f64; 6],
        value: i32,
        stat_index: Option<usize>,
        label_index: Option<i32>,
    ) {
        let s = match (stat_index, label_index) {
            (Some(s), _) if s < 6 => s,
            (Some(_), _) => return,
            (None, Some(l)) if l >= 101 => match l {
                101 => 0,
                104 => 1,
                124 => 2,
                102 => 3,
                123 => 4,
                103 => 5,
                _ => return,
            },
            (None, _) => return,
        };
        self.label_bonus[s] = caption_round_trip(self.label_bonus[s] + f64::from(value));
        self.stat_with_bonus[s] = caption_round_trip(base_stats[s] + self.label_bonus[s]);
    }

    /// Port of frmMain `InvenCalcEncum`: max encumbrance from strength and the encumbrance bonus.
    /// An argument that does not fit an Integer raises inside MME's own handler, which leaves the
    /// caption alone.
    pub fn inven_calc_encum(&mut self, dat_ver_caption: &str) {
        let (Ok(strength), Ok(bonus)) = (
            vb::cint(self.stat_with_bonus[0]),
            vb::cint(self.stat[STAT_ENC_BONUS]),
        ) else {
            return;
        };
        self.put(1, f64::from(calc_encum(strength, bonus, dat_ver_caption)));
    }
}

// ---------------------------------------------------------------------------------------------
// Text helpers

/// `InStr(1, s, find, vbTextCompare)` as a position (0 for not found).
fn instr_text(s: &str, find: &str) -> usize {
    let hay: Vec<char> = s.chars().map(lower).collect();
    let needle: Vec<char> = find.chars().map(lower).collect();
    if hay.is_empty() {
        return 0;
    }
    if needle.is_empty() {
        return 1;
    }
    (0..hay.len().saturating_sub(needle.len() - 1))
        .find(|&i| hay[i..i + needle.len()] == needle[..])
        .map_or(0, |i| i + 1)
}

fn lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Port of frmMain `PastedStatIsBuffed`: is the label found in the text followed (after spaces) by
/// `*`?
#[must_use]
pub fn pasted_stat_is_buffed(search: &str, label: &str) -> bool {
    let at = instr_text(search, label);
    if at == 0 {
        return false;
    }
    let chars: Vec<char> = search.chars().collect();
    let mut x = at + label.chars().count();
    while x <= chars.len() {
        match chars[x - 1] {
            ' ' => x += 1,
            '*' => return true,
            _ => return false,
        }
    }
    false
}

// ---------------------------------------------------------------------------------------------
// Item Manager

fn carried_text(row: &ItemManagerRow) -> Option<&str> {
    if row.sub_items.len() >= 2 {
        Some(row.sub_items[1].as_str())
    } else {
        None
    }
}

/// Port of frmMain `ItemIsFlaggedCarried`: is the item in a CARRIED Item Manager row?
#[must_use]
pub fn item_is_flagged_carried(item: i32, rows: &[ItemManagerRow]) -> bool {
    if item < 1 {
        return false;
    }
    rows.iter().any(|r| {
        f64::from(item) == vb::val(&r.text)
            && carried_text(r).is_some_and(|t| instr_text(t, "CARRIED") > 0)
    })
}

/// The carried-item scan at the top of `CalcCharacterStats`: rows with a positive item number and a
/// status column containing CARRIED, whose item exists, with the quantity from a trailing ` xN`
/// (`Split` is case-sensitive where the `InStr` that triggers it is not; a quantity below 1 is 1).
///
/// # Errors
/// Error 6 when the quantity does not fit an Integer.
pub fn carried_items(
    world: &World,
    rows: &[ItemManagerRow],
) -> Result<Vec<CarriedItem>, RulesError> {
    let mut out = Vec::new();
    for r in rows {
        let number = vb::val(&r.text);
        let Some(status) = carried_text(r) else {
            continue;
        };
        if number <= 0.0 || instr_text(status, "CARRIED") == 0 {
            continue;
        }
        let item = to_long(number).map_err(lbl)?;
        if world.get_item(item).is_none() {
            continue;
        }
        let mut qty = 1;
        if instr_text(status, " x") > 0 {
            let mut parts = status.split(" x");
            if let Some(second) = parts.nth(1) {
                qty = i32::from(vb::cint(vb::val(second)).map_err(lbl)?);
            }
        }
        out.push(CarriedItem {
            item,
            qty: qty.max(1),
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// ItemIsUsableByChar and friends

/// Port of frmMain `ItemIsUsableByChar`: may the selected class, level and alignment use the item?
/// `equipped` is `nEquippedItem` (the minimum-item-level rule lets an equipped item through).
#[must_use]
pub fn item_is_usable_by_char(
    profile: Profile,
    world: &World,
    who: &Character,
    equipped: &[i32; EQUIP_SLOTS],
    item_number: i32,
    ignore_min_item_level: bool,
    skip_align_check: bool,
) -> bool {
    let Profile::Stock = profile;
    if item_number == 0 || world.items.is_empty() {
        return false;
    }
    if who.level >= 999 && who.class < 1 && who.alignment == 0 {
        return true;
    }
    let Some(item) = world.get_item(item_number) else {
        return false;
    };

    let mut class_ok = false;
    let mut magical = false;
    let mut item_level = 0;
    for slot in &item.abilities {
        match slot.ability {
            135 => {
                item_level = slot.value;
                if slot.value > who.level {
                    return false;
                }
            }
            136 if slot.value < who.level => return false,
            59 if slot.value > 0 && slot.value == who.class => class_ok = true,
            28 => magical = true,
            97 | 98 | 112 => {
                let want = match who.alignment {
                    1 => 97,
                    2 => 112,
                    3 => 98,
                    _ => slot.ability,
                };
                if slot.ability != want && !skip_align_check {
                    return false;
                }
            }
            110 | 111 | 113 => {
                let bad = match who.alignment {
                    1 => 110,
                    2 => 113,
                    3 => 111,
                    _ => 0,
                };
                if slot.ability == bad && !skip_align_check {
                    return false;
                }
            }
            _ => {}
        }
    }

    if !ignore_min_item_level
        && who.min_item_level > 0
        && item_level < who.min_item_level
        && !equipped.contains(&item_number)
    {
        return false;
    }

    if world.classes.is_empty() || who.class < 1 {
        return true;
    }
    let Some(class) = world.get_class(who.class) else {
        return true;
    };
    for slot in &class.abilities {
        if slot.ability == 51 {
            if magical {
                return false;
            }
            break;
        }
    }

    if !class_ok {
        // 0 = undetermined, -1 = restrictions and no match yet, 1 = match found
        let mut n_class = 0;
        for &rest in &item.class_rest {
            if rest != 0 && n_class == 0 {
                n_class = -1;
            }
            if rest == who.class {
                class_ok = true;
                n_class = 1;
                break;
            }
        }
        if n_class == -1 {
            return false;
        }
    }
    if class_ok {
        return true;
    }

    match item.item_type {
        0 => {
            if class.armour_type < item.armour_type {
                return false;
            }
            // A shield (Worn 12) for a one-handed-weapon class needs the class-ok ability.
            if item.worn == 12 && matches!(class.weapon_type, 0 | 2 | 4 | 9) {
                return false;
            }
        }
        1 => {
            let wt = item.weapon_type;
            let ok = match class.weapon_type {
                0 => wt == 0,
                1 => wt == 1,
                2 => wt == 2,
                3 => wt == 3,
                4 => wt == 0 || wt == 2,
                5 => wt == 1 || wt == 3,
                6 => wt >= 2,
                7 => wt <= 1,
                // "Staff": MME hard-codes the dagger and the quarterstaff
                9 => item.number == 68 || item.number == 100,
                _ => true,
            };
            if !ok {
                return false;
            }
        }
        _ => {}
    }
    true
}

/// Special items (type 10) and armour that is worn nowhere are the carried items that count.
fn carried_item_applies(item: &Item) -> bool {
    item.item_type == 10 || (item.item_type == 0 && item.worn == 0)
}

/// Port of frmMain `ItemIsCarryActive`: does a carried copy of the item add its abilities?
#[must_use]
pub fn item_is_carry_active(
    profile: Profile,
    world: &World,
    who: &Character,
    equipped: &[i32; EQUIP_SLOTS],
    item_number: i32,
) -> bool {
    if item_number < 1 || world.items.is_empty() {
        return false;
    }
    let Some(item) = world.get_item(item_number) else {
        return false;
    };
    carried_item_applies(item)
        && item_is_usable_by_char(profile, world, who, equipped, item_number, true, false)
}

/// The `nBonus()` and `sSources()` arrays of `AccumItemStatBonus`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatBonus {
    /// STR INT WIL AGL HEA CHM.
    pub bonus: [i32; 6],
    /// `Item name (+5), Other item (-2)` per stat.
    pub sources: [String; 6],
}

/// Port of frmMain `AccumItemStatBonus`: adds one item's `+stat` abilities to `acc`; true when any
/// were added. `carried_qty` is the quantity of a carried copy (`None` when worn). A carried item must be active ([`item_is_carry_active`]) and its quantity multiplies
/// every stat except STR, as in `CalcCharacterStats`.
#[must_use]
pub fn accum_item_stat_bonus(
    profile: Profile,
    world: &World,
    who: &Character,
    equipped: &[i32; EQUIP_SLOTS],
    item_number: i32,
    carried_qty: Option<i32>,
    acc: &mut StatBonus,
) -> bool {
    if item_number < 1 || world.items.is_empty() {
        return false;
    }
    if carried_qty.is_some() && !item_is_carry_active(profile, world, who, equipped, item_number) {
        return false;
    }
    let Some(item) = world.get_item(item_number) else {
        return false;
    };
    let carried = carried_qty.is_some();
    let qty = carried_qty.unwrap_or(1).max(1);
    let mut any = false;
    for slot in &item.abilities {
        let stat = match slot.ability {
            46 => 0,
            44 => 1,
            45 => 2,
            48 => 3,
            47 => 4,
            49 => 5,
            _ => continue,
        };
        if slot.value == 0 {
            continue;
        }
        let multi = if carried && stat != 0 { qty } else { 1 };
        let v = slot.value.wrapping_mul(multi);
        acc.bonus[stat] = acc.bonus[stat].wrapping_add(v);
        let text = format!("{} ({}{})", item.name, if v > 0 { "+" } else { "" }, v);
        acc.sources[stat] = if acc.sources[stat].is_empty() {
            text
        } else {
            format!("{}, {}", acc.sources[stat], text)
        };
        any = true;
    }
    any
}

// ---------------------------------------------------------------------------------------------
// RefreshCharBless

/// What `RefreshCharBless` leaves behind.
#[derive(Debug, Clone, PartialEq)]
pub struct BlessStats {
    /// `bless_Stats(0..200)`: the bless total per stat slot (see `get_ability_stat_slot`), 100 the
    /// shadow AC and 101..124 the `+stat` abilities.
    pub stats: Box<[f64; BLESS_COUNT]>,
    /// `nGlobalCharBlessSpells(0..9)`.
    pub spells: [i32; BLESS_SLOTS],
    /// The VB6 error that ended the pass (a spell with duration 0 divides by zero while the mana
    /// per second is totalled); spells after it were not applied.
    pub error: Option<i32>,
}

/// Port of frmMain `RefreshCharBless` (stock): totals the stat bonuses of the ten bless spells.
/// `level` is `Val(txtGlobalLevel(0).Text)` and `worn_armour_type` the current
/// `nGlobalCharWornArmourType` (0 while `CalcCharacterStats` runs it, the last value otherwise).
/// The mana-per-round total and the tool tips are not ported.
#[must_use]
pub fn refresh_char_bless(
    profile: Profile,
    world: &World,
    level: i32,
    slots: &[i32; BLESS_SLOTS],
    worn_armour_type: i32,
) -> BlessStats {
    let Profile::Stock = profile;
    let mut out = BlessStats {
        stats: Box::new([0.0; BLESS_COUNT]),
        spells: [0; BLESS_SLOTS],
        error: None,
    };
    if world.spells.is_empty() {
        return out;
    }
    let set_level = if level == 0 { 1 } else { level };
    let mut accy_win = 0i16;
    let mut shadow_ac = 0;
    for (x, &number) in slots.iter().enumerate() {
        out.spells[x] = number;
        if number <= 0 {
            continue;
        }
        let Some(spell) = world.get_spell(number) else {
            continue;
        };
        if let Err(e) = bless_one(
            spell,
            set_level,
            worn_armour_type,
            &mut out.stats,
            &mut accy_win,
            &mut shadow_ac,
        ) {
            out.error = Some(i32::try_from(e.code()).unwrap_or(0));
            break;
        }
    }
    out
}

fn bless_one(
    spell: &Spell,
    set_level: i32,
    worn_armour_type: i32,
    stats: &mut [f64; BLESS_COUNT],
    accy_win: &mut i16,
    shadow_ac: &mut i32,
) -> Res<()> {
    let mut level = set_level;
    if level > spell.cap && spell.cap > 0 {
        level = spell.cap;
    }
    if level < spell.req_level {
        level = spell.req_level;
    }
    let req = MinMaxRequest {
        use_level: level > 0,
        level: to_integer(level)?,
        ..MinMaxRequest::default()
    };
    // `gAvgLevelMaxAllStats` only matters for a level of 0, which the lines above rule out.
    let mm = get_current_spell_min_max(Some(spell), 0.0, &req);
    let avg_cast = to_long(
        mm.n_min
            .checked_add(mm.n_max)?
            .checked_div(cur_long(2))?
            .to_f64(),
    )?;
    // ManaCost / (nDur * SPELL_ROUND_SECS) feeds the unported mana-per-round total, but a zero
    // duration still raises here and ends the pass
    if mm.n_dur.checked_mul(cur_long(SPELL_ROUND_SECS))?.scaled() == 0 {
        return Err(Vb6Error::DivisionByZero);
    }

    for slot in &spell.abilities {
        if slot.ability <= 0 {
            continue;
        }
        if slot.ability == 9 {
            *shadow_ac = 10;
        } else {
            let mut val = f64::from(slot.value);
            if val == 0.0 {
                val = f64::from(avg_cast);
            }
            if slot.ability == 7 {
                val = vb::round(val / 10.0, 1)?;
            }
            let equip =
                get_ability_stat_slot(i16::try_from(slot.ability).map_err(|_| Vb6Error::Overflow)?);
            if equip > 0 {
                let e = equip as usize;
                if equip == 10 {
                    // accuracy: only the best bless counts
                    if val > f64::from(*accy_win) {
                        *accy_win = vb::cint(val)?;
                        stats[e] = val;
                    }
                } else if equip == 2 && slot.ability == 10 {
                    // AC blur: a fraction of the value that depends on the armour worn
                    let mut t = val;
                    t = match worn_armour_type - 3 {
                        6.. => vb::fix(t / 4.0),
                        4 | 5 => vb::fix(t / 3.0),
                        _ => vb::fix(t / 2.0),
                    };
                    if t > 0.0 {
                        stats[e] += t;
                    }
                } else {
                    stats[e] += val;
                }
            }
        }
    }
    if *shadow_ac > 0 {
        stats[100] = f64::from(*shadow_ac);
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// CalcCharacterStats, first half

/// Port of the first half of frmMain `CalcCharacterStats`: manual adjustments, class and race
/// abilities, encumbrance, bless spells and the equipped and carried items, up to the `'stealth`
/// banner. See the module docs for what is left out.
///
/// The return value holds what MME's controls and globals hold at that point; a VB6 error that
/// MME's handler would have swallowed shows in [`CharStats::error`], with the stats as they were
/// when it hit.
///
/// # Errors
/// Never in practice: kept so P3-18 can chain on the same signature.
pub fn calc_gear_stats(input: &GearInput) -> Result<CharStats, RulesError> {
    let Profile::Stock = input.profile;
    let mut s = CharStats::reset(&input.base_stats);
    if let Err(e) = run(input, &mut s) {
        s.error = Some(i32::try_from(e.code()).unwrap_or(0));
    }
    Ok(s)
}

/// One pass over the equipped slots then the carried items.
struct Slot {
    index: usize,
    item: i32,
    qty: i32,
    carried: bool,
}

fn slots_of(input: &GearInput) -> Vec<Slot> {
    let mut v: Vec<Slot> = input
        .equipped
        .iter()
        .enumerate()
        .map(|(index, &item)| Slot {
            index,
            item,
            qty: 1,
            carried: false,
        })
        .collect();
    v.extend(input.carried.iter().enumerate().map(|(i, c)| Slot {
        index: EQUIP_SLOTS + i,
        item: c.item,
        qty: c.qty,
        carried: true,
    }));
    v
}

fn run(input: &GearInput, s: &mut CharStats) -> Res<()> {
    let world = input.world;
    let who = &input.character;
    let dat = input.dat_ver_caption;
    let usable = |item: i32, ignore_min: bool| {
        item_is_usable_by_char(
            input.profile,
            world,
            who,
            &input.equipped,
            item,
            ignore_min,
            false,
        )
    };

    abilities_of_class_and_race(input, s)?;

    // +encum from the manual adjustment
    if input.adjustments[STAT_ENC_BONUS] != 0 {
        s.add(STAT_ENC_BONUS, f64::from(input.adjustments[STAT_ENC_BONUS]));
    }

    let slots = slots_of(input);

    // +encum from items; every item's weight counts, only the usable ones add +encum
    for sl in &slots {
        if sl.item < 1 {
            continue;
        }
        let Some(item) = world.get_item(sl.item) else {
            continue;
        };
        if item.encum != 0 {
            s.add(0, f64::from(item.encum));
            if sl.carried && sl.qty > 1 {
                s.add(0, f64::from(item.encum) * f64::from(sl.qty - 1));
            }
        }
        if sl.carried && !(carried_item_applies(item) && usable(sl.item, true)) {
            continue;
        }
        for a in &item.abilities {
            if a.ability == 96 && a.value != 0 {
                s.add(STAT_ENC_BONUS, f64::from(a.value));
            }
        }
    }

    if let Some(w) = input.add_weight
        && w > 0.0
    {
        s.add(0, w);
    }

    // encumbrance as-is, then with the bless spells
    s.inven_calc_encum(dat);
    let bless = refresh_char_bless(
        input.profile,
        world,
        who.level,
        &input.bless_slots,
        s.worn_armour_type,
    );
    s.bless = bless.stats;
    s.bless_spells = bless.spells;

    if s.bless[STAT_ENC_BONUS] != 0.0 {
        s.add(STAT_ENC_BONUS, s.bless[STAT_ENC_BONUS]);
        s.inven_calc_encum(dat);
    }
    if s.bless[101] != 0.0 {
        s.adj_main_stat_bonus(&input.base_stats, to_long(s.bless[101])?, None, Some(101));
        s.inven_calc_encum(dat);
    }

    if s.stat[1] > 0.0 {
        let pct = vb::fix((s.stat[0] / s.stat[1]) * 100.0);
        s.encum_pct = i32::from(vb::cint(pct)?);
    }
    if s.encum_pct > 100 {
        s.encum_pct = 100;
    }

    // the rest of the manual and bless stats
    for x in 0..STAT_COUNT {
        if x == STAT_ENC_BONUS {
            continue;
        }
        let adj = input.adjustments[x];
        if adj != 0 {
            if x == 2 || x == 3 {
                s.add(x, f64::from(adj) / 10.0);
            } else {
                s.add(x, f64::from(adj));
            }
            match x {
                10 => s.accy_other = s.accy_other.wrapping_add(adj),
                8 => s.plus_dodge = s.plus_dodge.wrapping_add(adj),
                24 => s.plus_mr = s.plus_mr.wrapping_add(adj),
                _ => {}
            }
        }
        let b = s.bless[x];
        if b != 0.0 {
            if x == 10 {
                if b > f64::from(s.accy_abils) {
                    s.accy_abils = to_long(b)?;
                }
            } else {
                if x == 8 {
                    s.plus_dodge = to_long(f64::from(s.plus_dodge) + b)?;
                }
                if x == 24 {
                    s.plus_mr = to_long(f64::from(s.plus_mr) + b)?;
                }
                s.add(x, b);
            }
        }
    }

    if s.bless[100] > 0.0 {
        s.shadow_ac = 10;
    }
    for x in 102..=124usize {
        let stat = match x {
            104 => 1,
            124 => 2,
            102 => 3,
            123 => 4,
            103 => 5,
            _ => continue,
        };
        if s.bless[x] != 0.0 {
            s.label_bonus[stat] = caption_round_trip(s.label_bonus[stat] + s.bless[x]);
            s.stat_with_bonus[stat] =
                caption_round_trip(input.base_stats[stat] + s.label_bonus[stat]);
        }
    }

    equipped_items(input, s, &slots)?;

    s.hit_magic_non_weapon = to_long(s.stat[12])?;
    if !input.martial_arts {
        s.put(
            12,
            f64::from(s.hit_magic_non_weapon) + f64::from(s.weapon_hit_magic),
        );
    }
    if s.shadow_ac > 0 {
        s.add(2, f64::from(s.shadow_ac));
    }
    Ok(())
}

/// Class then race: stealth flags, the race's hit points per level and the abilities that map to a
/// stat slot.
fn abilities_of_class_and_race(input: &GearInput, s: &mut CharStats) -> Res<()> {
    let world = input.world;
    let who = &input.character;
    if who.class > 0
        && !world.classes.is_empty()
        && !input.hide_class_race
        && let Some(class) = world.get_class(who.class)
    {
        for a in &class.abilities {
            if a.ability == 103 {
                s.class_stealth = true;
            }
            apply_group_ability(input, s, a.ability, a.value)?;
        }
    }
    if who.race > 0 && !world.races.is_empty() && !input.hide_class_race {
        let hp = get_race_hp_bonus(world.get_race(who.race));
        if hp != 0 {
            s.add(5, f64::from(hp) * f64::from(who.level));
        }
        if let Some(race) = world.get_race(who.race) {
            for a in &race.abilities {
                if a.ability == 102 {
                    s.race_stealth = true;
                }
                apply_group_ability(input, s, a.ability, a.value)?;
            }
        }
    }
    Ok(())
}

fn apply_group_ability(input: &GearInput, s: &mut CharStats, ability: i32, value: i32) -> Res<()> {
    let equip = get_ability_stat_slot(i16::try_from(ability).map_err(|_| Vb6Error::Overflow)?);
    if equip <= 0 {
        return Ok(());
    }
    let e = equip as usize;
    if equip > 100 {
        s.adj_main_stat_bonus(&input.base_stats, value, None, Some(equip));
    } else if equip == 3 {
        s.put(3, vb::round(s.stat[3] + f64::from(value) / 10.0, 1)?);
    } else if equip == 10 {
        // only the best accuracy ability counts
        if value > 0 && value > s.accy_abils {
            s.accy_abils = value;
        }
    } else {
        if equip == 8 {
            s.plus_dodge = s.plus_dodge.wrapping_add(value);
        }
        if equip == 24 {
            s.plus_mr = s.plus_mr.wrapping_add(value);
        }
        s.add(e, f64::from(value));
    }
    Ok(())
}

/// The "equipped items" loop: worn items (slots 0..19) and carried items.
fn equipped_items(input: &GearInput, s: &mut CharStats, slots: &[Slot]) -> Res<()> {
    let world = input.world;
    let who = &input.character;
    // nWeaponStatIndex is never reset between slots: a carried item sees whatever the last
    // equipped item left, and 0 when nothing was equipped.
    let mut weapon_index: i32 = 0;
    for sl in slots {
        let multi_i = i64::from(sl.qty.max(1));
        if sl.item < 1 {
            continue;
        }
        let Some(item) = world.get_item(sl.item) else {
            continue;
        };
        // Single, as MME declares them
        let mut n_ac: f32 = 0.0;
        let mut n_dr: f32 = 0.0;

        if !sl.carried && sl.index != 16 && item.item_type > s.worn_armour_type {
            s.worn_armour_type = item.item_type;
        }
        let mut abilities_only = false;
        if sl.carried {
            if carried_item_applies(item)
                && item_is_usable_by_char(
                    input.profile,
                    world,
                    who,
                    &input.equipped,
                    sl.item,
                    true,
                    false,
                )
            {
                abilities_only = true;
            } else {
                continue;
            }
        } else {
            weapon_index = match sl.index {
                15 => 1,
                16 => 0,
                _ => -1,
            };
            if weapon_index >= 0 {
                let w = &mut s.weapons[weapon_index as usize];
                w.number = sl.item;
                w.accy = item.accy;
                w.encum = item.encum;
            }
        }

        if !abilities_only {
            n_ac = ((i64::from(item.armour_class) * multi_i) as f64 / 10.0) as f32;
            n_dr = ((i64::from(item.damage_resist) * multi_i) as f64 / 10.0) as f32;
            if item.accy != 0 {
                let v = i64::from(item.accy) * multi_i;
                s.accy_items = s.accy_items.wrapping_add(v as i32);
                s.add(10, v as f64);
            }
        }

        for a in &item.abilities {
            if a.ability <= 0 || a.value == 0 {
                continue;
            }
            let abil_val = a.value;
            let total = i64::from(abil_val) * multi_i;
            if sl.index == 16 && (a.ability == 28 || a.ability == 142) {
                // the equipped weapon's own hit magic; ignored for a martial-arts attack
                if !input.martial_arts {
                    s.weapon_hit_magic = i32::from(to_integer(abil_val)?);
                }
                continue;
            }
            let equip =
                get_ability_stat_slot(i16::try_from(a.ability).map_err(|_| Vb6Error::Overflow)?);
            if equip <= 0 || equip > 100 {
                // no slot, or a +stat ability that only GreaterMUD gear applies
                continue;
            }
            let e = equip as usize;
            if equip == 2 && a.ability == 10 {
                let t = total as f64;
                let t = match s.worn_armour_type - 3 {
                    6.. => vb::fix(t / 4.0),
                    4 | 5 => vb::fix(t / 3.0),
                    _ => vb::fix(t / 2.0),
                };
                if t > 0.0 {
                    s.add(e, t);
                }
            } else if equip == 2 {
                n_ac = (f64::from(n_ac) + total as f64) as f32;
            } else if equip == 3 {
                n_dr = (f64::from(n_dr) + total as f64 / 10.0) as f32;
            } else if equip == 10 {
                if abil_val > 0 && abil_val > s.accy_abils {
                    s.accy_abils = abil_val;
                }
            } else {
                if weapon_index >= 0 {
                    let w = &mut s.weapons[weapon_index as usize];
                    match equip {
                        7 => w.crit = abil_val,
                        11 => w.max_dmg = abil_val,
                        13 => w.bs_accy = abil_val,
                        14 => w.bs_min_dmg = abil_val,
                        15 => w.bs_max_dmg = abil_val,
                        37 => w.punch_skill = abil_val,
                        40 => w.punch_accy = abil_val,
                        34 => w.punch_dmg = abil_val,
                        38 => w.kick_skill = abil_val,
                        41 => w.kick_accy = abil_val,
                        35 => w.kick_dmg = abil_val,
                        39 => w.jumpkick_skill = abil_val,
                        42 => w.jumpkick_accy = abil_val,
                        36 => w.jumpkick_dmg = abil_val,
                        19 => w.stealth = abil_val,
                        _ => {}
                    }
                }
                if equip != 4 {
                    // +encum was added in the encumbrance pass
                    if equip == 8 {
                        s.plus_dodge = s.plus_dodge.wrapping_add(total as i32);
                    }
                    if equip == 24 {
                        s.plus_mr = s.plus_mr.wrapping_add(total as i32);
                    }
                    s.add(e, total as f64);
                }
            }
        }

        if n_ac != 0.0 || n_dr != 0.0 {
            s.put(2, vb::round(s.stat[2] + f64::from(n_ac), 1)?);
            s.put(3, vb::round(s.stat[3] + f64::from(n_dr), 1)?);
        }
    }
    Ok(())
}
