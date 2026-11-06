//! The Hit Calculator window (FAM-26). Card P3-42.
//!
//! MME keeps the window's logic in the form's event code, reading and writing its controls and a
//! handful of module-level variables. Here the controls are [`HitCalcForm`], the module variables
//! are [`HitCalcState`] and what `SetHitCalcVals` reads from the main window is
//! [`HitCalcCharacter`]. Each ported procedure takes the form and state and returns the new ones in
//! a [`HitCalcResult`]; a label the procedure did not write keeps the string it had.
//!
//! A run-time error that MME's handler catches stops the procedure where its `Resume out` would
//! and is listed in [`HitCalcResult::errors`]. `SetHitCalcVals` always ends by running `DoHitCalc`
//! (its `out:` label), also after an error.
//!
//! Events are the caller's: a Change or Click on a control runs [`do_hit_calc`] (MME's
//! `bDontRefresh` only mutes them inside `SetHitCalcVals`), so after [`modify_hit_calc_values`] the
//! caller runs it too. Stock rules only; GreaterMUD branches (vile ward, the evilness combo) are
//! not ported.

use touchstone_model::{Class, Item, Monster};
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Vb6Error};

use crate::Profile;
use crate::abilities::item_has_ability;
use crate::accuracy::{
    AttackDefenseInput, BackstabAccuracyInput, calculate_attack_defense,
    calculate_backstab_accuracy, get_dodge_cap, get_hit_cap, get_hit_min,
};
use crate::character::{self, AttackType, CharStats, Character};
use crate::stats::get_class_stealth;

/// `optHitCalcType`: which kind of attack is calculated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitCalcType {
    Normal,
    Backstab,
}

/// `optAttacker`: where the accuracy comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attacker {
    Character,
    Monster,
    Manual,
}

/// `optDefender`: where the defences come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Defender {
    /// The loaded character (also counts as a player in `DoHitCalc`).
    Character,
    Monster,
    /// A remembered player: the values last typed while this was selected.
    Player,
    Manual,
}

/// The controls of frmHitCalc that the ported procedures read or write.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HitCalcForm {
    /// `optHitCalcType`; `None` when no button is selected.
    pub calc_type: Option<HitCalcType>,
    pub attacker: Option<Attacker>,
    pub defender: Option<Defender>,
    /// `txtHitCalc(0..5).Text`: accuracy, AC, dodge, protection from evil, vile ward, and
    /// perception or backstab defense.
    pub text: [String; 6],
    /// `txtHitCalc(3).Enabled` (the label and the two buttons follow it), `(4)` and `(5)`.
    pub prot_evil_enabled: bool,
    pub vile_ward_enabled: bool,
    pub percep_enabled: bool,
    /// `chkShadow.Value = 1` and `.Enabled`.
    pub shadow: bool,
    pub shadow_enabled: bool,
    /// `chkSeeHidden.Value = 1` and `.Enabled`.
    pub see_hidden: bool,
    pub see_hidden_enabled: bool,
    /// `cmbEvil.ListIndex` and `.Enabled`.
    pub evil_index: i32,
    pub evil_enabled: bool,
    /// `cmbMonsterList.Enabled`.
    pub monster_list_enabled: bool,
    /// `lblBSPercep.Caption`.
    pub bs_label: String,
    /// `lblMain.Caption`: the hit, dodge and overall hit lines.
    pub main_text: String,
    /// `lblSubLeft.Caption`.
    pub sub_left: String,
    /// `lblSubRight.Caption` and its `FontBold`.
    pub sub_right: String,
    pub sub_right_bold: bool,
}

/// The module-level variables of frmHitCalc.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HitCalcState {
    pub monster_accy: i32,
    pub monster_ac: i32,
    pub monster_dodge: i32,
    pub monster_bs_defense: i32,
    pub monster_evil: bool,
    pub monster_see_hidden: bool,
    pub defender_ac: i32,
    pub defender_dodge: i32,
    pub defender_perception: i32,
    pub defender_prot_evil: i32,
    pub defender_vile_ward: i32,
    pub defender_evilness: i32,
    pub defender_shadow: bool,
    pub char_perception: i32,
}

/// What `SetHitCalcVals` reads from the main window and the `nGlobal*` variables. The `*_tag`
/// fields are `Val()` of the control text.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HitCalcCharacter {
    /// `lblInvenCharStat(10).Tag`, `(2)`, `(8)`, `(20)`: accuracy, AC, dodge, protection from evil.
    pub accuracy: f64,
    pub ac: f64,
    pub dodge: f64,
    pub prot_evil: f64,
    /// `lblInvenCharStat(19).Tag`, `(13)`, `txtCharStats(3).Tag`, `(0)`: stealth, backstab
    /// accuracy, agility, strength.
    pub stealth: f64,
    pub bs_accuracy: f64,
    pub agility: f64,
    pub strength: f64,
    /// `Val(txtGlobalLevel(0).Text)`.
    pub level: f64,
    /// `cmbGlobalClass(0).ItemData(ListIndex)`; `None` when nothing is selected (error 381).
    pub class: Option<i32>,
    pub shadow_ac: i32,
    pub accy_abilities: i32,
    pub accy_other: i32,
    pub accy_items: i32,
    /// `nGlobalCharWeaponNumber(0)`, `nGlobalCharWeaponAccy(0)`, `nGlobalCharWeaponBSaccy(0)`.
    pub weapon_number: i32,
    pub weapon_accy: i32,
    pub weapon_bs_accy: i32,
    pub attack_type: AttackType,
    pub attack_accy_adj: i16,
    pub attack_backstab: bool,
    pub attack_backstab_weapon: i32,
    pub attack_martial_art: i16,
    /// `GetCharActiveVileWard()`.
    pub vile_ward: i32,
}

/// Which half of the window `SetHitCalcVals` refreshes (its two Optional Boolean arguments).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Refresh {
    pub character_only: bool,
    pub monster_only: bool,
}

/// A VB6 error MME's handler caught.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandledError {
    /// The procedure whose handler caught it.
    pub source: &'static str,
    pub number: u32,
}

/// The three percentages `DoHitCalc` prints in `lblMain`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitCalcNumbers {
    pub hit: i32,
    pub dodge: i32,
    pub overall_hit: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HitCalcResult {
    pub form: HitCalcForm,
    pub state: HitCalcState,
    pub errors: Vec<HandledError>,
    /// Set when the last `DoHitCalc` got as far as writing its labels.
    pub numbers: Option<HitCalcNumbers>,
}

/// A run-time error: VB6's own, or 381 for `ItemData` of a combo with nothing selected.
#[derive(Debug, Clone, Copy)]
enum Fail {
    Vb(Vb6Error),
    NothingSelected,
}

impl From<Vb6Error> for Fail {
    fn from(e: Vb6Error) -> Self {
        Fail::Vb(e)
    }
}

impl Fail {
    fn code(self) -> u32 {
        match self {
            Fail::Vb(e) => e.code(),
            Fail::NothingSelected => 381,
        }
    }
}

type R<T> = Result<T, Fail>;

/// `Fix(Val(text))` assigned to a Long.
fn fix_long(text: &str) -> R<i32> {
    Ok(vb::clng(vb::fix(vb::val(text)))?)
}

/// A Long assigned to an Integer.
fn to_i16(x: i32) -> Result<i16, Vb6Error> {
    i16::try_from(x).map_err(|_| Vb6Error::Overflow)
}

/// Port of modSyntaxsFunc AutoAppend.
fn auto_append(base: &str, add: &str, glue: &str) -> String {
    if base.is_empty() {
        add.to_string()
    } else if add.trim_matches(' ').is_empty() {
        base.to_string()
    } else {
        format!("{base}{glue}{add}")
    }
}

// ---------------------------------------------------------------------------------------------

/// Port of frmHitCalc GetVileWardValue (private, and unused by the window): the share of a vile
/// ward that counts for a defender of the given evilness.
#[must_use]
pub fn get_vile_ward_value(vile_ward: i32, evilness: i32) -> i32 {
    if vile_ward > 0 {
        if evilness >= 2 {
            vile_ward
        } else if evilness >= 1 {
            vile_ward / 2
        } else {
            0
        }
    } else {
        0
    }
}

/// Port of frmHitCalc ModifyHitCalcValues: one of the 12 arrow buttons changes a box by one.
/// `index` 0..=11 is accuracy -/+, AC -/+, dodge -/+, protection -/+, vile ward -/+, perception -/+.
/// The caller runs [`do_hit_calc`] afterwards (the box's Change event). An index outside 0..=11
/// does nothing.
pub fn modify_hit_calc_values(form: &mut HitCalcForm, index: usize) {
    if index > 11 {
        return;
    }
    let delta = if index.is_multiple_of(2) { -1.0 } else { 1.0 };
    let t = &mut form.text[index / 2];
    *t = vb::cstr(vb::fix(vb::val(t)) + delta);
}

/// Port of frmHitCalc GetMonsterData: loads the monster's numbers into the state. Returns false
/// (state unchanged) when there is no such monster. `nmr_ver` is the database version; backstab
/// defense exists from 1.83.
pub fn get_monster_data(
    monsters: &[Monster],
    number: i32,
    nmr_ver: f64,
    state: &mut HitCalcState,
) -> bool {
    let Some(m) = monsters.iter().find(|m| m.number == number) else {
        return false;
    };
    state.monster_accy = 0;
    state.monster_dodge = 0;
    state.monster_ac = m.armour_class;
    state.monster_bs_defense = if nmr_ver >= 1.83 { m.bs_defense } else { 0 };
    state.monster_see_hidden = false;
    for a in &m.abilities {
        if a.ability == 34 && a.value > 0 {
            state.monster_dodge = a.value;
        } else if a.ability == 57 {
            state.monster_see_hidden = true;
        }
    }
    // Only melee attacks (normal and rob) count, and only ones the monster uses.
    for att in &m.attacks {
        if att.att_type > 0
            && att.att_type <= 3
            && att.pct > 0
            && matches!(att.att_type, 1 | 3)
            && att.acc > state.monster_accy
        {
            state.monster_accy = att.acc;
        }
    }
    state.monster_evil = matches!(m.align, 1 | 2 | 5 | 6);
    true
}

/// The mapping `SetHitCalcVals` does from the loaded character: what it reads from the main
/// window's controls and the `nGlobal*` variables. `shadow_ac` is `nGlobalCharShadowAC`, which
/// `CharStats` does not carry.
#[must_use]
pub fn hit_calc_input_from_character(
    profile: Profile,
    ch: &Character,
    stats: &CharStats,
    shadow_ac: i32,
) -> HitCalcCharacter {
    HitCalcCharacter {
        accuracy: stats.stat[10],
        ac: stats.stat[2],
        dodge: stats.stat[8],
        prot_evil: stats.stat[20],
        stealth: stats.stat[19],
        bs_accuracy: stats.stat[13],
        agility: stats.base_stat[3],
        strength: stats.base_stat[0],
        level: f64::from(ch.level),
        class: ch.class,
        shadow_ac,
        accy_abilities: stats.accy_abils,
        accy_other: stats.accy_other,
        accy_items: stats.accy_items,
        weapon_number: stats.weapon_number[0],
        weapon_accy: stats.weapon_accy[0],
        weapon_bs_accy: stats.weapon_bs_accy[0],
        attack_type: ch.attack.attack_type,
        attack_accy_adj: ch.attack.accy_adj,
        attack_backstab: ch.attack.backstab,
        attack_backstab_weapon: ch.attack.backstab_weapon,
        attack_martial_art: ch.attack.martial_art,
        vile_ward: character::get_char_active_vile_ward(profile),
    }
}

// ---------------------------------------------------------------------------------------------
// DoHitCalc

/// Port of frmHitCalc DoHitCalc: reads the boxes, remembers the "player" defender's values in the
/// state, and writes the hit, dodge and overall hit lines and the min and cap lines.
/// `class` is `cmbGlobalClass(0).ItemData(ListIndex)` (read only against the character; `None`
/// is error 381).
#[must_use]
pub fn do_hit_calc(
    profile: Profile,
    form: &HitCalcForm,
    state: &HitCalcState,
    class: Option<i32>,
) -> HitCalcResult {
    let mut r = HitCalcResult {
        form: form.clone(),
        state: state.clone(),
        errors: Vec::new(),
        numbers: None,
    };
    if let Err(e) = do_hit_calc_steps(profile, &mut r, class) {
        r.errors.push(HandledError {
            source: "DoHitCalc",
            number: e.code(),
        });
    }
    r
}

fn do_hit_calc_steps(profile: Profile, r: &mut HitCalcResult, class: Option<i32>) -> R<()> {
    let form = &mut r.form;
    let st = &mut r.state;
    let backstab = form.calc_type == Some(HitCalcType::Backstab);
    let vs_player = matches!(form.defender, Some(Defender::Character | Defender::Player));
    let def_player = form.defender == Some(Defender::Player);
    let def_char = form.defender == Some(Defender::Character);

    let mut accy = fix_long(&form.text[0])?;
    let mut ac = fix_long(&form.text[1])?;
    let mut dodge = fix_long(&form.text[2])?;
    if def_player {
        st.defender_ac = ac;
        st.defender_dodge = dodge;
    }
    let mut prot_evil = 0;
    if form.prot_evil_enabled {
        prot_evil = fix_long(&form.text[3])?;
        if def_player {
            st.defender_prot_evil = prot_evil;
        }
    }
    let mut vile_ward = 0;
    if form.vile_ward_enabled {
        vile_ward = fix_long(&form.text[4])?;
        if def_player {
            st.defender_vile_ward = vile_ward;
        }
    }
    let mut perception = 0;
    let mut bs_defense = 0;
    if form.percep_enabled {
        perception = fix_long(&form.text[5])?;
        if def_char {
            st.char_perception = perception;
        }
        if def_player {
            st.defender_perception = perception;
        }
        bs_defense = fix_long(&form.text[5])?;
    }

    // MME writes these clamps as `If x > 9999 Then x = 9999: If x < 1 Then x = 1` on one line, so
    // the lower bound is tested only inside the upper test and never changes anything.
    accy = accy.min(9999);
    ac = ac.min(9999);
    dodge = dodge.min(9999);
    perception = perception.min(9999);
    bs_defense = bs_defense.min(9999);

    let mut shadow = false;
    if form.shadow_enabled {
        shadow = form.shadow;
        if def_player {
            st.defender_shadow = shadow;
        }
    }
    let see_hidden = form.see_hidden_enabled && form.see_hidden;

    let class = if def_char {
        i32::from(to_i16(class.ok_or(Fail::NothingSelected)?)?)
    } else {
        0
    };

    let d = calculate_attack_defense(
        profile,
        &AttackDefenseInput {
            accy,
            ac,
            dodge,
            bs_defense,
            prot_evil,
            prot_good: 0,
            perception,
            vile_ward,
            shadow,
            see_hidden,
            backstab,
            vs_player,
            class,
        },
    );

    // The percentages are Currency in MME; Round of a Currency rounds half to even.
    let hit = Currency::from_scaled(i64::from(d.hit_chance) * 10_000);
    let dodge_chance = Currency::from_scaled(i64::from(d.dodge_chance) * 10_000);
    let mut text = format!("Hit: {}%", d.hit_chance);
    let overall;
    if d.dodge_chance > 0 {
        text.push_str(&format!("\r\nDodge: {}%", d.dodge_chance));
        let share = dodge_chance.checked_div(Currency::from_scaled(100 * 10_000))?;
        let left = hit.checked_sub(hit.checked_mul(share)?)?;
        overall = round_currency(left);
        text.push_str(&format!("\r\nOverall Hit: {overall}%"));
    } else {
        text.push_str("\r\nDodge: 0%");
        overall = d.hit_chance;
        text.push_str(&format!("\r\nOverall Hit: {}%", d.hit_chance));
    }
    form.main_text = text;
    r.numbers = Some(HitCalcNumbers {
        hit: d.hit_chance,
        dodge: d.dodge_chance,
        overall_hit: overall,
    });

    let min_hit = get_hit_min(profile, class);
    let max_hit = get_hit_cap(profile);
    let mut sub = String::from("Hit Min-Cap:\r\n");
    if min_hit == get_hit_min(profile, 0) {
        sub.push_str(&format!("{min_hit}%"));
        form.sub_right_bold = false;
    } else {
        sub.push_str(&format!("*{min_hit}%*"));
        form.sub_right_bold = true;
    }
    sub.push_str(&format!(" - {max_hit}%"));
    sub.push_str(&format!(
        "\r\nDodge Cap:\r\n{}%",
        get_dodge_cap(profile, 0, false)
    ));
    form.sub_right = sub;
    Ok(())
}

/// `Round(Currency)` to a whole number, ties to even.
fn round_currency(c: Currency) -> i32 {
    let s = c.scaled();
    let q = s.div_euclid(10_000);
    let rem = s.rem_euclid(10_000);
    let up = rem > 5_000 || (rem == 5_000 && q % 2 != 0);
    (q + i64::from(up)) as i32
}

// ---------------------------------------------------------------------------------------------
// SetHitCalcVals

/// Everything `SetHitCalcVals` reads besides the form and state.
#[derive(Debug, Clone, Copy)]
pub struct SetValsInput<'a> {
    pub form: &'a HitCalcForm,
    pub state: &'a HitCalcState,
    pub character: &'a HitCalcCharacter,
    pub items: &'a [Item],
    pub classes: &'a [Class],
    pub refresh: Refresh,
}

/// Port of frmHitCalc SetHitCalcVals: fills the boxes from the chosen attacker and defender, sets
/// which boxes are enabled, then runs [`do_hit_calc`]. With no option selected in one of the three
/// groups it only clears the two side labels.
#[must_use]
pub fn set_hit_calc_vals(profile: Profile, input: &SetValsInput) -> HitCalcResult {
    let mut r = HitCalcResult {
        form: input.form.clone(),
        state: input.state.clone(),
        errors: Vec::new(),
        numbers: None,
    };
    r.form.sub_left.clear();
    r.form.sub_right.clear();
    if input.form.calc_type.is_none()
        || input.form.attacker.is_none()
        || input.form.defender.is_none()
    {
        return r;
    }
    if let Err(e) = set_vals_steps(profile, input, &mut r) {
        r.errors.push(HandledError {
            source: "SetHitCalcVals",
            number: e.code(),
        });
    }
    // out: bDontRefresh = False, then DoHitCalc.
    let d = do_hit_calc(profile, &r.form, &r.state, input.character.class);
    r.form = d.form;
    r.state = d.state;
    r.numbers = d.numbers;
    r.errors.extend(d.errors);
    r
}

#[allow(clippy::too_many_lines)]
fn set_vals_steps(profile: Profile, input: &SetValsInput, r: &mut HitCalcResult) -> R<()> {
    let ch = input.character;
    let scope = input.refresh;
    let form = &mut r.form;
    let st = &mut r.state;
    let backstab = form.calc_type == Some(HitCalcType::Backstab);
    let normal = !backstab;
    let char_attack = form.attacker == Some(Attacker::Character);
    let mob_attack = form.attacker == Some(Attacker::Monster);
    let vs_char = form.defender == Some(Defender::Character);
    let vs_mob = form.defender == Some(Defender::Monster);
    let vs_player = form.defender == Some(Defender::Player);
    let vs_manual = form.defender == Some(Defender::Manual);

    // ATTACKER / ACCY
    if backstab && char_attack && !scope.monster_only {
        let bs_weapon = if ch.attack_backstab && ch.attack_backstab_weapon > 0 {
            ch.attack_backstab_weapon
        } else if !ch.attack_backstab || ch.attack_backstab_weapon == 0 {
            ch.weapon_number
        } else {
            0
        };
        let mut norm_adj: i16 = 0;
        let mut bs_adj: i16 = 0;
        // Not accounting for removal of a shield if the backstab weapon is two-handed (as MME).
        if bs_weapon != ch.weapon_number {
            if bs_weapon > 0 {
                norm_adj = to_i16(item_ability(input.items, bs_weapon, 22))?.max(0);
                bs_adj = to_i16(item_ability(input.items, bs_weapon, 116))?.max(0);
            }
            norm_adj = to_i16(vb::sub_i32(i32::from(norm_adj), ch.weapon_accy)?)?;
            bs_adj = to_i16(vb::sub_i32(i32::from(bs_adj), ch.weapon_bs_accy)?)?;
        }
        // VB evaluates every argument first (the class combo can raise 381, a Long sum can
        // overflow) and only then converts them to the Integer parameters, in order; GreaterMUD's
        // level, strength and strength requirement are converted too, so they can overflow here.
        let class = ch.class.ok_or(Fail::NothingSelected)?;
        let class_stealth = get_class_stealth(input.classes.iter().find(|c| c.number == class));
        let sum = vb::add_i32(
            vb::add_i32(ch.accy_abilities, ch.accy_other)?,
            i32::from(norm_adj),
        )?;
        let stealth = vb::cint(ch.stealth)?;
        let agility = vb::cint(ch.agility)?;
        let plus_bs = vb::cint(ch.bs_accuracy + f64::from(bs_adj))?;
        let plus_normal = to_i16(sum)?;
        vb::cint(ch.level)?;
        vb::cint(ch.strength)?;
        to_i16(item_str_req(input.items, bs_weapon))?;
        let accy = calculate_backstab_accuracy(
            profile,
            &BackstabAccuracyInput {
                stealth,
                agility,
                plus_bs_accuracy: plus_bs,
                class_stealth,
                plus_normal_accuracy: plus_normal,
            },
        );
        form.text[0] = accy.to_string();
        form.sub_left = "BS Accuracy Calculated for Char.".to_string();
    } else if normal && char_attack && !scope.monster_only {
        form.text[0] = vb::cstr(ch.accuracy);
        let adj = ch.attack_accy_adj;
        let name = match ch.attack_type {
            AttackType::PhysBash => Some("Bash"),
            AttackType::PhysSmash => Some("Smash"),
            AttackType::MartialArts => match ch.attack_martial_art {
                2 => Some("Kick"),
                3 => Some("Jumpkick"),
                _ => None,
            },
            _ => None,
        };
        if let Some(name) = name {
            form.text[0] = vb::cstr(vb::val(&form.text[0]) + f64::from(adj));
            form.sub_left = auto_append(&form.sub_left, &format!("{name} {adj} acc"), "\r\n");
        }
    } else if normal && mob_attack && !scope.character_only {
        form.text[0] = st.monster_accy.to_string();
    }

    // DEFENDER - AC, DODGE, SECONDARY DEFENSE
    if vs_mob && !scope.character_only {
        form.text[1] = st.monster_ac.to_string();
        form.text[2] = st.monster_dodge.to_string();
        form.text[3] = "0".into();
        form.text[4] = "0".into();
        form.bs_label = "vs BS Defense".into();
        form.text[5] = st.monster_bs_defense.to_string();
        form.shadow = false;
        form.evil_index = 0;
        form.see_hidden = st.monster_see_hidden;
    } else if vs_char && !scope.monster_only {
        form.text[1] = vb::cstr(vb::fix(ch.ac) - f64::from(ch.shadow_ac));
        form.text[2] = vb::cstr(ch.dodge);
        form.text[3] = vb::cstr(ch.prot_evil);
        form.text[4] = ch.vile_ward.to_string();
        form.text[5] = st.char_perception.to_string();
        form.bs_label = if backstab { "vs Perception" } else { "N/A" }.into();
        form.shadow = ch.shadow_ac > 0;
        form.evil_index = 0;
        form.see_hidden = false;
    } else if vs_player && !scope.monster_only {
        form.text[1] = st.defender_ac.to_string();
        form.text[2] = st.defender_dodge.to_string();
        form.text[3] = st.defender_prot_evil.to_string();
        form.text[4] = st.defender_vile_ward.to_string();
        form.text[5] = st.defender_perception.to_string();
        form.bs_label = if backstab { "vs Perception" } else { "N/A" }.into();
        form.shadow = st.defender_shadow;
        form.evil_index = match st.defender_evilness {
            1 => 1,
            2 => 2,
            _ => 0,
        };
        form.see_hidden = false;
    } else if scope.character_only || scope.monster_only {
        // nothing: only one half was asked for
    } else if vs_manual && backstab {
        form.bs_label = "vs BS Defense".into();
    } else {
        form.bs_label = "N/A".into();
    }

    form.monster_list_enabled = mob_attack || vs_mob;

    if vs_mob || backstab {
        // GreaterMUD would keep the vile ward and evilness boxes alive for a backstab.
        form.prot_evil_enabled = false;
        form.vile_ward_enabled = false;
        form.percep_enabled = backstab;
        form.see_hidden_enabled = backstab;
        form.shadow_enabled = false;
        form.evil_enabled = false;
    } else {
        form.prot_evil_enabled = !(mob_attack && !st.monster_evil);
        form.shadow_enabled = true;
        form.vile_ward_enabled = false;
        form.evil_enabled = false;
        let manual_bs = backstab && vs_manual;
        form.percep_enabled = manual_bs;
        form.see_hidden_enabled = manual_bs;
    }

    if mob_attack && !st.monster_evil {
        form.sub_left = auto_append(&form.sub_left, "Mob Not Evil", ", ");
    }
    Ok(())
}

fn item_ability(items: &[Item], number: i32, ability: i16) -> i32 {
    let slots = items
        .iter()
        .find(|i| i.number == number)
        .map(|i| &i.abilities[..]);
    item_has_ability(Profile::Stock, slots, number, ability)
}

/// Port of modMMudDatabase GetItemStrReq: 0 for item 0 or a failed Seek.
fn item_str_req(items: &[Item], number: i32) -> i32 {
    if number == 0 {
        return 0;
    }
    items
        .iter()
        .find(|i| i.number == number)
        .map_or(0, |i| i.str_req)
}
