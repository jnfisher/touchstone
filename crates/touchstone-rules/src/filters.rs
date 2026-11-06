//! List filters (FAM-23): which records the weapons, armour, sundry and equipment lists show. Card P3-29.
//!
//! MME's `Filter*` procedures read the filter controls of a tab, loop over the item table and call an
//! `Add*2LV` procedure for every record that passes. The port takes the controls as a [`ListFilter`]
//! (its `Default` is what `ResetFilterOptions` leaves in them) and returns the calls MME would make, in
//! MME's order: a record number plus the arguments of the `Add*2LV` call (and, for weapons, the profile
//! `PopulateCharacterProfile` made for it). The caller turns each call into a row with
//! [`crate::rows`]. The sort that follows in MME (`LV_RefreshSort`) is the grid's job, so the lists are
//! in the order MME added the rows.
//!
//! Ports: `FilterWeapons`, `FilterArmour`, `FilterSundry`, `FilterInvenItems` (with `InvenAddEquip` and
//! `LoadInvenItems`) and `FilterAll`. `FilterSpells` and `FilterMonsters` are card P3-30.
//!
//! # Dialogs
//!
//! `FilterWeapons` asks questions with `MsgBox` (martial arts without the skill, maximum effort, a
//! backstab without stealth). The answers come from the `ask` closure; a Cancel, or a No where MME
//! gives up, leaves the list as it was (`cancelled`).
//!
//! # Errors
//!
//! A VB6 run-time error (a box that overflows `CLng`, a division by zero in a martial-arts row) ends the
//! procedure where it happens with the list built so far, as MME's `Resume out` does (`aborted`).

use touchstone_model::{Item, World};
use touchstone_vb6 as vb;

use crate::Profile;
use crate::attack::{self, AttackRequest};
use crate::character::CharacterProfile;
use crate::gear::{Character, EQUIP_SLOTS, item_is_usable_by_char};
use crate::pipeline::CharacterState;
use crate::rows::{Cell, Row, RowEnv, calculate_weapon_attack};
use crate::stats::{get_class_stealth, get_race_stealth};

/// The answer to one of the dialogs (`vbYes`, `vbNo`, `vbCancel`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Yes,
    No,
    Cancel,
}

/// The `MsgBox` calls of `FilterWeapons`, in the order they can be asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialog {
    /// "You do not appear to have the required skill ... Calculate anyway at +1 skill?" (yes/no).
    MartialNoSkill,
    /// "Without character stats, only base damage will be calculated." (information; the answer is ignored).
    MartialNoCharacter,
    /// "Calculate Bash Damage?" (yes/no/cancel).
    MaxBash,
    /// "Calculate Smash Damage?".
    MaxSmash,
    /// "Calculate Backstab Damage?", asked when the class or race has stealth.
    MaxBackstab,
    /// "You do not appear to have class or race stealth ... Calculate anyway?" (yes/no).
    BackstabNoStealth,
}

/// `cmbArmourWorn`-style ability filter: `cmbXAbilityList`, `cmbXAbilityOp` and `txtXAbilityVal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityFilter {
    /// `ItemData` of the selected entry: 0 "Any", -1 "Negates (= Spell#)", else an ability number.
    pub selected: i32,
    /// `cmbXAbilityOp.ListIndex = 0` (the value is at most the box); MME's default is 1 (at least).
    pub at_most: bool,
    /// `txtXAbilityVal.Text`.
    pub value_text: String,
}

impl Default for AbilityFilter {
    fn default() -> Self {
        Self {
            selected: 0,
            at_most: false,
            value_text: "1".to_string(),
        }
    }
}

/// The weapons tab's controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponFilter {
    /// `chkWeaponOptions(2)`: include weapons with a limit.
    pub include_limited: bool,
    /// `txtWeaponSpeed.Text`.
    pub max_speed_text: String,
    /// `chkHanded(0..3)`: the weapon types shown.
    pub handed: [bool; 4],
    /// `txtWeaponExtras(0).Text`: the largest strength requirement.
    pub max_str_text: String,
    /// `txtWeaponExtras(1).Text`: the smallest damage per round (needs a calculation).
    pub min_dmg_text: String,
    /// `cmbWeaponCombos(0).ListIndex`: the least magic level.
    pub magic_level: i32,
    /// `chkWeaponOptions(0)`: only non-magical weapons.
    pub non_magic: bool,
    /// `chkWeaponOptions(1)`: only weapons that can backstab.
    pub backstab_only: bool,
    pub ability: AbilityFilter,
    /// `chkWeaponOptions(3)`: calculate combat values.
    pub calc_combat: bool,
    /// `chkWeaponOptions(4)`: the 85% speed adjustment.
    pub slow_85: bool,
    /// `cmbWeaponCombos(1).ItemData(ListIndex)`: 5 normal, 4 backstab, 6 bash, 7 smash, -1 martial arts,
    /// -2 maximum effort. `None` when nothing is selected (VB6 error 381).
    pub combo: Option<i32>,
    /// `txtWeaponExtras(2..4).Text`: versus AC, DR and dodge.
    pub vs: [String; 3],
}

impl Default for WeaponFilter {
    fn default() -> Self {
        Self {
            include_limited: true,
            max_speed_text: "999999".to_string(),
            handed: [true; 4],
            max_str_text: "999".to_string(),
            min_dmg_text: "0".to_string(),
            magic_level: 0,
            non_magic: false,
            backstab_only: false,
            ability: AbilityFilter::default(),
            calc_combat: false,
            slow_85: false,
            combo: Some(5),
            vs: ["0".to_string(), "0".to_string(), "0".to_string()],
        }
    }
}

/// The armour tab's controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArmourFilter {
    /// `chkArmourType(0..6)`: the armour types shown (cloth, leather, ..., with 3..6 sharing box 3).
    pub types: [bool; 7],
    /// `chkArmourNoLimit`: only armour without a limit.
    pub no_limit: bool,
    /// `chkArmourNonMagic`: only non-magical armour.
    pub non_magic: bool,
    /// `cmbArmourWorn.ItemData(ListIndex)`: 0 anywhere, 1..18 a place.
    pub worn: i32,
    pub ability: AbilityFilter,
}

impl Default for ArmourFilter {
    fn default() -> Self {
        Self {
            types: [true; 7],
            no_limit: false,
            non_magic: false,
            worn: 0,
            ability: AbilityFilter::default(),
        }
    }
}

/// All the tabs' controls; `Default` is what `ResetFilterOptions` sets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListFilter {
    pub weapons: WeaponFilter,
    pub armour: ArmourFilter,
    /// The sundry tab has only the ability filter.
    pub sundry: AbilityFilter,
}

/// What the filters read besides their own tab's controls.
pub struct FilterEnv<'a> {
    pub profile: Profile,
    pub world: &'a World,
    /// `bOnlyInGame`.
    pub only_in_game: bool,
    /// `chkGlobalFilter.Value = 1`.
    pub global_filter: bool,
    /// The global filter's class, race, level, alignment and minimum item level.
    pub who: Character,
    /// `nEquippedItem`.
    pub equipped: [i32; EQUIP_SLOTS],
    /// `Val(lblInvenCharStat(37..39).Tag)`: punch, kick and jumpkick skill.
    pub martial_skill: [f64; 3],
}

impl FilterEnv<'_> {
    fn usable(&self, number: i32, ignore_min_level: bool) -> bool {
        item_is_usable_by_char(
            self.profile,
            self.world,
            &self.who,
            &self.equipped,
            number,
            ignore_min_level,
            false,
        )
    }
}

/// The `PopulateCharacterProfile(tChar, bUseCharacter, True, mud, weapon)` call behind a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileRequest {
    pub use_character: bool,
    /// `eAttackTypeMUD`.
    pub mud: i32,
    /// The weapon number (0: the current weapon; -1: fists).
    pub weapon: i32,
}

/// One `AddWeapon2LV` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponAdd {
    /// The record; `None` for the surprise punch (`nAttackTypeMUD = -4`), which reads none.
    pub item: Option<i32>,
    pub ability: i16,
    pub attack_type: i32,
    pub force_calc: bool,
    pub profile: ProfileRequest,
}

/// A row of the weapons list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WeaponEntry {
    /// A call to `AddWeapon2LV`.
    Add(WeaponAdd),
    /// A martial-arts row `FilterWeapons` builds itself.
    Built(Row),
}

/// The finished weapons list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WeaponList {
    pub entries: Vec<WeaponEntry>,
    /// A dialog was cancelled (or nothing could be listed): MME left the list as it was.
    pub cancelled: bool,
    /// A VB6 error ended the procedure; `entries` is what was built.
    pub aborted: bool,
    /// Some record was filtered out (the "*Weapons*" caption).
    pub filtered: bool,
    /// The Ability column sorts as numbers (an ability other than 43 is shown).
    pub numeric_ability_sort: bool,
}

/// One `AddArmour2LV` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArmourAdd {
    pub item: i32,
    pub ability: i16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArmourList {
    pub entries: Vec<ArmourAdd>,
    pub aborted: bool,
    pub filtered: bool,
}

/// The records `AddOtherItem2LV` is called for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SundryList {
    pub entries: Vec<i32>,
    pub aborted: bool,
    pub filtered: bool,
}

/// The equipment combos (`cmbEquip(0..19)`) after `FilterInvenItems`: the item numbers of each slot's
/// list, after the leading "(none)" entry (number 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvenLists {
    pub slots: Vec<Vec<i32>>,
    /// Some item was filtered out (the "*EQ*" caption).
    pub filtered: bool,
    /// An equipped item is no longer in its list (`bPromptSave`).
    pub equipped_removed: bool,
}

/// What `FilterAll` leaves in the lists it fills.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllLists {
    pub weapons: WeaponList,
    pub armour: ArmourList,
    pub inven: Option<InvenLists>,
}

type Res<T> = Result<T, vb::Vb6Error>;

fn ability_hit(it: &Item, f: &AbilityFilter, value: f64) -> bool {
    it.abilities.iter().any(|s| {
        s.ability == f.selected
            && if f.at_most {
                f64::from(s.value) <= value
            } else {
                f64::from(s.value) >= value
            }
    })
}

fn negates(it: &Item, spell: i32) -> bool {
    it.negate_spell.contains(&spell)
}

// ---------------------------------------------------------------------------------------------
// FilterWeapons

/// Port of frmMain `FilterWeapons`. `remove_filter` is `bREMOVE_FILTER` (every in-game weapon is
/// listed). `profile_for` is `PopulateCharacterProfile`; `ask` answers the dialogs.
#[must_use]
pub fn filter_weapons(
    env: &FilterEnv,
    rows: &RowEnv,
    f: &WeaponFilter,
    remove_filter: bool,
    profile_for: &dyn Fn(ProfileRequest) -> CharacterProfile,
    ask: &mut dyn FnMut(Dialog) -> Answer,
) -> WeaponList {
    let mut out = WeaponList::default();
    if env.world.items.is_empty() {
        out.cancelled = true;
        return out;
    }
    match weapon_steps(env, rows, f, remove_filter, profile_for, ask, &mut out) {
        Ok(()) => {}
        Err(_) => out.aborted = true,
    }
    out
}

#[derive(Default)]
struct Plan {
    force_calc: bool,
    max_effort: bool,
    do_bash: bool,
    do_smash: bool,
    do_backstab: bool,
    martial: bool,
}

#[allow(clippy::too_many_lines)] // one MME procedure, kept in its order
fn weapon_steps(
    env: &FilterEnv,
    rows: &RowEnv,
    f: &WeaponFilter,
    remove_filter: bool,
    profile_for: &dyn Fn(ProfileRequest) -> CharacterProfile,
    ask: &mut dyn FnMut(Dialog) -> Answer,
    out: &mut WeaponList,
) -> Res<()> {
    let use_char = env.global_filter;
    let calc_combat = f.calc_combat;
    let mut attack: i32 = 5;
    let mut speed_adj: i16 = 100;
    let mut plan = Plan::default();
    out.numeric_ability_sort = false;

    if calc_combat && !remove_filter {
        if f.slow_85 {
            speed_adj = 85;
        }
        let Some(combo) = f.combo else {
            out.cancelled = true;
            return Ok(());
        };
        attack = combo;
        if attack == -1 {
            if use_char && env.martial_skill.iter().all(|s| *s <= 0.0) {
                if ask(Dialog::MartialNoSkill) != Answer::Yes {
                    out.cancelled = true;
                    return Ok(());
                }
                plan.force_calc = true;
            } else if !use_char {
                let _ = ask(Dialog::MartialNoCharacter);
            }
            plan.martial = true;
        } else if attack == -2 {
            match ask(Dialog::MaxBash) {
                Answer::Yes => plan.do_bash = true,
                Answer::Cancel => {
                    out.cancelled = true;
                    return Ok(());
                }
                Answer::No => {}
            }
            match ask(Dialog::MaxSmash) {
                Answer::Yes => plan.do_smash = true,
                Answer::Cancel => {
                    out.cancelled = true;
                    return Ok(());
                }
                Answer::No => {}
            }
            // GetClassStealth / GetRaceStealth read the global filter's class and race while it is on
            let class = use_char
                .then(|| env.world.get_class(env.who.class))
                .flatten();
            let race = use_char.then(|| env.world.get_race(env.who.race)).flatten();
            if get_class_stealth(class) || get_race_stealth(race) {
                match ask(Dialog::MaxBackstab) {
                    Answer::Yes => plan.do_backstab = true,
                    Answer::Cancel => {
                        out.cancelled = true;
                        return Ok(());
                    }
                    Answer::No => {}
                }
            }
            plan.max_effort = true;
            plan.martial = true;
        } else if attack == 4 && use_char {
            let class = env.world.get_class(env.who.class);
            let race = env.world.get_race(env.who.race);
            if !get_class_stealth(class)
                && !get_race_stealth(race)
                && env.who.class > 0
                && env.who.race > 0
            {
                if ask(Dialog::BackstabNoStealth) != Answer::Yes {
                    out.cancelled = true;
                    return Ok(());
                }
                plan.force_calc = true;
            }
        }
    }

    if plan.martial {
        martial_arts_rows(env, rows, f, speed_adj, &plan, use_char, profile_for, out)?;
        if !plan.max_effort {
            out.filtered = true;
            return Ok(());
        }
    }

    // normal_op: the profile the loop's rows use (maximum effort makes its own per weapon)
    let profile_req = ProfileRequest {
        use_character: use_char,
        mud: attack,
        weapon: 0,
    };
    let loop_profile = (!plan.max_effort).then(|| profile_for(profile_req));

    let mut negate: i32 = -1;
    let mut ability: i16 = 0;
    if !remove_filter {
        if f.ability.selected > 0 {
            ability = i16::try_from(f.ability.selected).map_err(|_| vb::Vb6Error::Overflow)?;
            if f.ability.selected != 43 {
                out.numeric_ability_sort = true;
            }
        } else if f.ability.selected == -1 {
            negate = vb::clng(vb::val(&f.ability.value_text))?;
        }
    }
    let ability_value = vb::val(&f.ability.value_text);
    let min_dmg = vb::val(&f.min_dmg_text);

    for it in &env.world.items {
        if it.item_type != 1 {
            continue;
        }
        if env.only_in_game && !it.in_game {
            continue;
        }
        if !remove_filter && !weapon_passes(env, f, it, negate, attack, ability_value) {
            out.filtered = true;
            continue;
        }

        if plan.max_effort && !remove_filter {
            let bs_able = it.abilities.iter().any(|s| s.ability == 116);
            let mut kind = if plan.do_backstab && bs_able { 4 } else { 5 };
            while kind > 0 {
                let req = ProfileRequest {
                    use_character: use_char,
                    mud: kind,
                    weapon: it.number,
                };
                let ch = CharacterState::attack_profile_of(&profile_for(req));
                let mut keep = true;
                if min_dmg > 0.0 {
                    let mut casts = String::new();
                    let dmg = calculate_weapon_attack(
                        rows,
                        &ch,
                        &AttackRequest {
                            attack_type: kind,
                            weapon_number: it.number,
                            speed_adj,
                            vs_ac: vb::clng(vb::val(&f.vs[0]))?,
                            vs_dr: vb::clng(vb::val(&f.vs[1]))?,
                            vs_dodge: vb::clng(vb::val(&f.vs[2]))?,
                            force_calc: plan.force_calc,
                            ..AttackRequest::default()
                        },
                        &mut casts,
                    );
                    keep = f64::from(dmg.round_total) >= min_dmg;
                }
                if keep {
                    out.entries.push(WeaponEntry::Add(WeaponAdd {
                        item: Some(it.number),
                        ability: -1,
                        attack_type: kind,
                        force_calc: plan.force_calc,
                        profile: req,
                    }));
                }
                kind = if kind < 5 {
                    5
                } else if kind < 6 && plan.do_bash {
                    6
                } else if kind < 7 && plan.do_smash {
                    7
                } else {
                    0
                };
            }
        } else {
            if !remove_filter && min_dmg > 0.0 {
                let ch = CharacterState::attack_profile_of(
                    loop_profile.as_ref().ok_or(vb::Vb6Error::Overflow)?,
                );
                let extra = |k: usize| -> Res<i32> {
                    if calc_combat {
                        vb::clng(vb::val(&f.vs[k]))
                    } else {
                        Ok(0)
                    }
                };
                let mut casts = String::new();
                let dmg = calculate_weapon_attack(
                    rows,
                    &ch,
                    &AttackRequest {
                        attack_type: attack,
                        weapon_number: it.number,
                        speed_adj,
                        vs_ac: extra(0)?,
                        vs_dr: extra(1)?,
                        vs_dodge: extra(2)?,
                        force_calc: plan.force_calc,
                        ..AttackRequest::default()
                    },
                    &mut casts,
                );
                if f64::from(dmg.round_total) < min_dmg {
                    out.filtered = true;
                    continue;
                }
            }
            out.entries.push(WeaponEntry::Add(WeaponAdd {
                item: Some(it.number),
                ability,
                attack_type: attack,
                force_calc: plan.force_calc,
                profile: profile_req,
            }));
        }
    }

    // the surprise punch ends a backstab list unless an ability narrows it
    if (attack == 4 || (plan.max_effort && plan.do_backstab))
        && negate <= 0
        && f.ability.selected == 0
    {
        let req = ProfileRequest {
            use_character: false,
            mud: 4,
            weapon: -1,
        };
        let punch = profile_for(req);
        if f.magic_level <= punch.hit_magic_non_weapon {
            out.entries.push(WeaponEntry::Add(WeaponAdd {
                item: None,
                ability: 0,
                attack_type: -4,
                force_calc: false,
                profile: req,
            }));
        }
    }
    Ok(())
}

/// The per-record tests of `FilterWeapons` between the in-game check and the maximum-effort branch.
fn weapon_passes(
    env: &FilterEnv,
    f: &WeaponFilter,
    it: &Item,
    negate: i32,
    attack: i32,
    ability_value: f64,
) -> bool {
    if !f.include_limited && it.limit != 0 {
        return false;
    }
    if vb::val(&f.max_speed_text) < f64::from(it.speed) {
        return false;
    }
    for (x, shown) in f.handed.iter().enumerate() {
        if !shown && usize::try_from(it.weapon_type).is_ok_and(|t| t == x) {
            return false;
        }
    }
    if f64::from(it.str_req) > vb::val(&f.max_str_text) {
        return false;
    }
    let mut bs_able = false;
    let mut magical = 0;
    let mut hit_magic = 0;
    let mut has_ability = false;
    for s in &it.abilities {
        match s.ability {
            116 => bs_able = true,
            28 => magical = s.value,
            142 => hit_magic = s.value,
            _ => {}
        }
        if f.ability.selected > 0
            && s.ability == f.ability.selected
            && if f.ability.at_most {
                f64::from(s.value) <= ability_value
            } else {
                f64::from(s.value) >= ability_value
            }
        {
            has_ability = true;
        }
    }
    if f.ability.selected > 0 && !has_ability {
        return false;
    }
    if env.global_filter && !env.usable(it.number, false) {
        return false;
    }
    // not bGreaterMUD: a plain magical weapon also counts as hitting magic
    hit_magic += magical;
    if f.magic_level > magical && f.magic_level > hit_magic {
        return false;
    }
    if f.non_magic && magical > 0 {
        return false;
    }
    if (f.backstab_only || attack == 4) && !bs_able {
        return false;
    }
    if negate > 0 && !negates(it, negate) {
        return false;
    }
    true
}

/// The punch, kick and jumpkick rows `FilterWeapons` builds itself (the `martial_arts:` section).
#[allow(clippy::too_many_arguments)]
fn martial_arts_rows(
    env: &FilterEnv,
    rows: &RowEnv,
    f: &WeaponFilter,
    speed_adj: i16,
    plan: &Plan,
    use_char: bool,
    profile_for: &dyn Fn(ProfileRequest) -> CharacterProfile,
    out: &mut WeaponList,
) -> Res<()> {
    for x in 37..=39usize {
        if !(plan.force_calc || !use_char || env.martial_skill[x - 37] > 0.0) {
            continue;
        }
        let kind = (x - 36) as i32;
        // without the character the profile is MME's empty tChar, not "no character"
        let ch = if use_char {
            CharacterState::attack_profile_of(&profile_for(ProfileRequest {
                use_character: true,
                mud: kind,
                weapon: 0,
            }))
        } else {
            attack::CharacterProfile::default()
        };
        let (name, kind_of) = match x {
            37 => ("Punch", "Fists"),
            38 => ("Kick", "Feet"),
            _ => ("Jumpkick", "Feet"),
        };
        let weapon = if use_char {
            rows.equipped.hands[0].number
        } else {
            0
        };
        let mut casts = String::new();
        let dmg = calculate_weapon_attack(
            rows,
            &ch,
            &AttackRequest {
                attack_type: kind,
                weapon_number: weapon,
                speed_adj,
                vs_ac: vb::clng(vb::val(&f.vs[0]))?,
                vs_dr: vb::clng(vb::val(&f.vs[1]))?,
                vs_dodge: vb::clng(vb::val(&f.vs[2]))?,
                force_calc: plan.force_calc,
                ..AttackRequest::default()
            },
            &mut casts,
        );
        let cell = |t: String| Cell {
            text: t,
            tag: None,
            fore_color: None,
        };
        let mut row = Row {
            text: "0".to_string(),
            tag: Some((-kind).to_string()),
            fore_color: None,
            cells: Vec::new(),
        };
        let c = &mut row.cells;
        c.push(cell(name.to_string()));
        c.push(cell(kind_of.to_string()));
        c.push(cell(dmg.min_dmg.to_string()));
        c.push(cell(dmg.max_dmg.to_string()));
        c.push(cell(dmg.attack_speed.to_string()));
        for _ in 0..4 {
            c.push(cell("0".to_string()));
        }
        c.push(cell(dmg.accy.to_string()));
        c.push(cell("No".to_string()));
        c.push(cell(dmg.crit_chance.to_string()));
        c.push(cell("0".to_string()));
        // the next cells are added only when the division works; a VB6 error ends the procedure with the row so far
        let per_speed = vb::div(f64::from(dmg.round_total), dmg.swings)
            .and_then(|v| vb::div(v, f64::from(dmg.attack_speed)))
            .and_then(|v| vb::round(v, 0));
        let per_speed = match per_speed {
            Ok(v) => v,
            Err(e) => {
                out.entries.push(WeaponEntry::Built(row));
                return Err(e);
            }
        };
        let c = &mut row.cells;
        c.push(cell(vb::cstr(per_speed)));
        c.push(cell(vb::cstr(vb::round(dmg.swings, 2)?)));
        c.push(cell(dmg.round_physical.to_string()));
        c.push(cell("0".to_string()));
        c.push(cell(dmg.round_total.to_string()));
        let mut first = cell(dmg.first_round_damage.to_string());
        first.tag = Some(vb::cstr(
            f64::from(dmg.first_round_damage) + vb::round(f64::from(dmg.round_total) / 100.0, 2)?,
        ));
        c.push(first);
        c.push(cell(format!(
            "{}% hit, Avg/Max Crit: {}/{}",
            dmg.hit_chance, dmg.avg_crit, dmg.max_crit
        )));
        out.entries.push(WeaponEntry::Built(row));
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// FilterArmour, FilterSundry

/// Port of frmMain `FilterArmour`: `use_global_filter` is its argument (`ItemIsUsableByChar` must pass).
#[must_use]
pub fn filter_armour(env: &FilterEnv, f: &ArmourFilter, use_global_filter: bool) -> ArmourList {
    let mut out = ArmourList::default();
    if env.world.items.is_empty() {
        return out;
    }
    let sel = f.ability.selected;
    let mut negate = -1;
    let mut ability: i16 = 0;
    if sel > 0 {
        let Ok(a) = i16::try_from(sel) else {
            out.aborted = true;
            return out;
        };
        ability = a;
    } else if sel == -1 {
        match vb::clng(vb::val(&f.ability.value_text)) {
            Ok(n) => negate = n,
            Err(_) => {
                out.aborted = true;
                return out;
            }
        }
    }
    let value = vb::val(&f.ability.value_text);
    for it in &env.world.items {
        if it.item_type != 0 {
            continue;
        }
        if env.only_in_game && !it.in_game {
            continue;
        }
        if armour_passes(env, f, it, use_global_filter, negate, value) {
            out.entries.push(ArmourAdd {
                item: it.number,
                ability,
            });
        } else {
            out.filtered = true;
        }
    }
    out
}

fn armour_passes(
    env: &FilterEnv,
    f: &ArmourFilter,
    it: &Item,
    use_global_filter: bool,
    negate: i32,
    value: f64,
) -> bool {
    if f.no_limit && it.limit != 0 {
        return false;
    }
    if f.worn > 0 {
        let w = it.worn;
        let ok = match f.worn {
            1 => w == 0,
            2 => w == 2,
            3 => w == 3,
            4 => w == 4 || w == 13,
            5 => w == 5,
            6 => w == 6,
            7 => w == 7,
            8 => w == 8,
            9 => w == 9,
            10 => w == 10,
            11 => w == 11,
            12 => w == 12,
            13 => w == 14 || w == 17,
            14 => w == 15,
            15 => w == 16,
            16 => w == 18,
            17 => w == 19,
            18 => w == 1,
            _ => true,
        };
        if !ok {
            return false;
        }
    }
    for (x, shown) in f.types.iter().enumerate() {
        if *shown {
            continue;
        }
        let t = it.armour_type;
        let hidden = match x {
            0 => t == 0,
            1 => t == 1,
            2 => t == 2,
            3 => (3..=6).contains(&t),
            4 => t == 7,
            5 => t == 8,
            _ => t == 9,
        };
        if hidden {
            return false;
        }
    }
    let magical = it.abilities.iter().any(|s| s.ability == 28);
    if f.ability.selected > 0 && !ability_hit(it, &f.ability, value) {
        return false;
    }
    if use_global_filter && !env.usable(it.number, false) {
        return false;
    }
    if f.non_magic && magical {
        return false;
    }
    if negate > 0 && !negates(it, negate) {
        return false;
    }
    true
}

/// Port of frmMain `FilterSundry`: armour worn nowhere and every item that is neither armour nor weapon.
#[must_use]
pub fn filter_sundry(env: &FilterEnv, f: &AbilityFilter) -> SundryList {
    let mut out = SundryList::default();
    if env.world.items.is_empty() {
        return out;
    }
    let mut negate = -1;
    if f.selected == -1 {
        match vb::clng(vb::val(&f.value_text)) {
            Ok(n) => negate = n,
            Err(_) => {
                out.aborted = true;
                return out;
            }
        }
    }
    let value = vb::val(&f.value_text);
    for it in &env.world.items {
        if !((it.item_type == 0 && it.worn == 0) || it.item_type > 1) {
            continue;
        }
        if env.only_in_game && !it.in_game {
            continue;
        }
        if (f.selected > 0 && !ability_hit(it, f, value)) || (negate > 0 && !negates(it, negate)) {
            out.filtered = true;
            continue;
        }
        out.entries.push(it.number);
    }
    out
}

// ---------------------------------------------------------------------------------------------
// FilterInvenItems

/// `InvenAddEquip`: the `cmbEquip` indexes an item is offered in (fingers and wrists in two).
fn equip_slots(item_type: i32, worn: i32) -> Vec<usize> {
    let slot: i32 = match item_type {
        0 => match worn {
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
    match slot {
        9 | 10 => vec![9, 10],
        6 | 7 => vec![6, 7],
        s if s >= 0 => vec![s as usize],
        _ => Vec::new(),
    }
}

fn empty_slots() -> Vec<Vec<i32>> {
    vec![Vec::new(); EQUIP_SLOTS]
}

/// Port of frmMain `FilterInvenItems`. With `remove_filter` (`LoadInvenItems`) every in-game weapon and
/// armour is offered; otherwise only those the global filter lets the character use. `None` where MME
/// does nothing (no items, or no classes when filtering).
#[must_use]
pub fn filter_inven_items(env: &FilterEnv, remove_filter: bool) -> Option<InvenLists> {
    if env.world.items.is_empty() || (!remove_filter && env.world.classes.is_empty()) {
        return None;
    }
    let mut slots = empty_slots();
    let mut filtered = false;
    for it in &env.world.items {
        if env.only_in_game && !it.in_game {
            continue;
        }
        if !remove_filter {
            if it.item_type > 1 {
                filtered = true;
                continue;
            }
            if !env.usable(it.number, true) {
                filtered = true;
                continue;
            }
        }
        for s in equip_slots(it.item_type, it.worn) {
            slots[s].push(it.number);
        }
    }
    // an equipped item that is no longer offered asks to save the character
    let equipped_removed = !remove_filter
        && env
            .equipped
            .iter()
            .enumerate()
            .any(|(x, n)| *n > 0 && !slots[x].contains(n));
    Some(InvenLists {
        slots,
        filtered,
        equipped_removed,
    })
}

// ---------------------------------------------------------------------------------------------
// FilterAll

/// Port of frmMain `FilterAll` (without `FilterSpells`, card P3-30, and the `RefreshAll` that follows):
/// every control goes back to its default, then the weapons, armour and equipment lists are filled.
/// With `enable_global` the global filter is switched on and applies; without it the filters are removed.
#[must_use]
pub fn filter_all(
    env: &FilterEnv,
    rows: &RowEnv,
    enable_global: bool,
    profile_for: &dyn Fn(ProfileRequest) -> CharacterProfile,
    ask: &mut dyn FnMut(Dialog) -> Answer,
) -> AllLists {
    let f = ListFilter::default();
    let on = FilterEnv {
        profile: env.profile,
        world: env.world,
        only_in_game: env.only_in_game,
        global_filter: if enable_global {
            true
        } else {
            env.global_filter
        },
        who: env.who,
        equipped: env.equipped,
        martial_skill: env.martial_skill,
    };
    AllLists {
        weapons: filter_weapons(&on, rows, &f.weapons, !enable_global, profile_for, ask),
        armour: filter_armour(&on, &f.armour, enable_global),
        inven: filter_inven_items(&on, !enable_global),
    }
}
