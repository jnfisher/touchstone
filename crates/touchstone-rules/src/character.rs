//! Character profile and attack config (FAM-05, the modMain half). Card P3-11.
//!
//! MME keeps the loaded character in `frmMain` controls and about forty module globals, and every
//! damage formula reads a `tCharacterProfile` that `PopulateCharacterProfile` builds from them.
//! Here the same state is three plain inputs:
//!
//! - [`Character`]: what the user sets and an `.mmec` file stores (race, class, level, stats,
//!   equipped items, bless spells, adjustments, learned spells and the attack choice).
//! - [`CharStats`]: what `CalcCharacterStats` and the Char-tab refreshers leave behind in control
//!   `.Tag`/`.Caption` text and in the `nGlobalChar*` weapon globals, as the `Val()` numbers the
//!   readers take. Gear aggregation (P3-17/18) and the panel derivations (P3-19) produce it.
//! - [`Context`]: tables, database-version globals, the lair-mode controls and the two above.
//!
//! [`CharacterProfile`] mirrors `tCharacterProfile` field for field (Integer is `i16`, Long `i32`,
//! Double `f64`; the 1-based `nMAPlus*(1 To 3)` arrays are 0-based). [`populate_character_profile`]
//! fills it in place, as callers reuse one, and on a VB6 error leaves it half filled with no final
//! clamps, as MME's handler does.
//!
//! A combo box with nothing selected (`ItemData(-1)`, error 381) is `class`/`race` = `None`;
//! each procedure then stops where MME's handler would.
//!
//! GreaterMUD branches are not ported (`Profile` only has `Stock`); `PopulateCharacterProfile`'s
//! `CalculateAccuracy` branch is reachable only for GreaterMUD. UI side effects are reported, not
//! done: `ClearSavedDamageVsMonster` as [`AttackConfigKey::changed`], the Populate write of `1` into
//! the lair HP box as [`Populated::lair_hp_reset`], `bDontPromptCalcCharMonsterDamage` as
//! [`DefenseDescription::reset_prompt`].

use touchstone_model::{AbilitySlot, Class, Item, Spell};
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Vb6Error};

use crate::Profile;
use crate::abilities::item_has_ability;
use crate::accuracy::{BackstabAccuracyInput, calculate_backstab_accuracy};
use crate::exp::get_max_level;
use crate::spell_cast::{CastRequest, SpellCastEnv, SpellCasterProfile};
use crate::spells::{SpellText, get_spell_duration};
use crate::stats::{
    calc_encumbrance_percent, calc_movement_speed, get_class_combat, get_class_stealth,
};

/// A VB6 error that ends the procedure at the point MME's handler would (the handler resumes at
/// `out:`); the value is whatever the procedure had built by then.
#[derive(Debug, Clone, Copy)]
struct Abort;

impl From<Vb6Error> for Abort {
    fn from(_: Vb6Error) -> Self {
        Abort
    }
}

impl From<crate::RulesError> for Abort {
    fn from(_: crate::RulesError) -> Self {
        Abort
    }
}

type R<T> = Result<T, Abort>;

// ---------------------------------------------------------------------------------------------
// Types

/// `eAttackTypeMME`: what the current character attacks with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AttackType {
    #[default]
    OneShot = 0,
    PhysAttack = 1,
    Spell = 2,
    SpellAny = 3,
    MartialArts = 4,
    Manual = 5,
    PhysBash = 6,
    PhysSmash = 7,
}

impl AttackType {
    /// The enum member for a stored number (`CurrentAttackType` in an `.mmec`).
    #[must_use]
    pub fn from_code(n: i32) -> Option<Self> {
        Some(match n {
            0 => Self::OneShot,
            1 => Self::PhysAttack,
            2 => Self::Spell,
            3 => Self::SpellAny,
            4 => Self::MartialArts,
            5 => Self::Manual,
            6 => Self::PhysBash,
            7 => Self::PhysSmash,
            _ => return None,
        })
    }
}

/// `eAttackTypeMUD`: the kind of swing a profile is built for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AttackTypeMud {
    #[default]
    None = 0,
    Punch = 1,
    Kick = 2,
    Jumpkick = 3,
    Surprise = 4,
    Normal = 5,
    Bash = 6,
    Smash = 7,
}

impl AttackTypeMud {
    #[must_use]
    pub fn from_code(n: i32) -> Option<Self> {
        Some(match n {
            0 => Self::None,
            1 => Self::Punch,
            2 => Self::Kick,
            3 => Self::Jumpkick,
            4 => Self::Surprise,
            5 => Self::Normal,
            6 => Self::Bash,
            7 => Self::Smash,
            _ => return None,
        })
    }
}

/// The `nGlobalAttack*` / `bGlobalAttack*` globals: the attack, backstab and heal choice.
/// `heal_value`, `heal_cost`, `heal_rounds` and `heal_spell_lvl` are also written by
/// [`refresh_combat_healing_values`]; the rest is the user's choice.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AttackConfig {
    pub attack_type: AttackType,
    /// `nGlobalAttackAccyAdj` (set by gear aggregation).
    pub accy_adj: i16,
    pub backstab: bool,
    pub backstab_weapon: i32,
    /// `nGlobalAttackMA`: 1 punch, 2 kick, 3 jumpkick.
    pub martial_art: i16,
    pub spell_num: i32,
    pub spell_lvl: i16,
    pub manual_p: i32,
    pub manual_m: i32,
    pub use_meditate: bool,
    /// 0 infinite, 1 base, 2 spell, 3 spell at a level, 4 manual.
    pub heal_type: i16,
    pub heal_spell_num: i32,
    pub heal_spell_lvl: i16,
    pub heal_rounds: i16,
    pub heal_manual: i32,
    pub heal_value: i32,
    pub heal_cost: f64,
}

/// The learned-spell list (`nLearnedSpells(0 To 99)` and `nLearnedSpellClass`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LearnedSpells {
    pub spells: [i32; 100],
    pub class: i16,
}

impl Default for LearnedSpells {
    fn default() -> Self {
        Self {
            spells: [0; 100],
            class: 0,
        }
    }
}

/// The user's character: what an `.mmec` file's PlayerInfo, Inventory, Bless and LearnedSpells
/// sections hold, in the order of `docs/inventory.md` section 5.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Character {
    pub name: String,
    /// `cmbGlobalRace(0).ItemData(ListIndex)`; `None` when nothing is selected.
    pub race: Option<i32>,
    /// `cmbGlobalClass(0).ItemData(ListIndex)`; `None` when nothing is selected.
    pub class: Option<i32>,
    pub level: i32,
    /// `cmbGlobalAlignment.ListIndex` (3 is evil).
    pub alignment: i16,
    /// `chkGlobalFilter.Value = 1`: the global filter uses the character.
    pub global_filter: bool,
    /// The six base stats: STR INT WIL AGL HEA CHM.
    pub stats: [i32; 6],
    /// `char_StatAdjustments(0..46)`.
    pub stat_adjustments: Vec<i32>,
    /// `nEquippedItem(0..19)`; slot 16 is the weapon.
    pub equipped: [i32; 20],
    /// `nGlobalCharBlessSpells(0..9)` / `cmbCharBless`.
    pub bless: [i32; 10],
    /// `chkCharQuests(0..11)`.
    pub quests: [bool; 12],
    /// `cmbCharQuestOpts(0..3).ListIndex`: 2nd, 6th, extra 1, extra 2.
    pub quest_options: [i16; 4],
    /// `txtCharMR.Text`, kept as text because `SetCharDefenseDescription` uses it verbatim.
    pub mr_text: String,
    pub anti_magic: bool,
    pub add_weight: i32,
    pub use_add_weight: bool,
    pub attack: AttackConfig,
    pub learned: LearnedSpells,
}

/// What gear aggregation and the Char-tab panels leave in controls and globals (the `Val()` of
/// each `.Tag`/`.Caption` text). `stat[i]` is `lblInvenCharStat(i).Tag` with the indexes of
/// `docs/inventory.md` section 5b; a gear-aggregation result with that vector and the
/// `nGlobalChar*` globals below fills it directly.
#[derive(Debug, Clone, PartialEq)]
pub struct CharStats {
    pub stat: [f64; 47],
    /// `lblInvenCharStat(0).Caption`, `(1).Caption`.
    pub encum_current: f64,
    pub encum_max: f64,
    /// `txtCharStats(0..5).Tag`: the six stats with bless and gear applied.
    pub base_stat: [f64; 6],
    /// `lblInvenStats(19).Tag`: 2 or more means the class has stealth.
    pub class_stealth_tag: f64,
    /// `lblCharMaxHP.Tag`, `lblCharRestRate.Tag`, `txtCharManaRegen.Tag`, `lblCharMaxMana.Tag`,
    /// `lblCharManaRate.Tag`, `lblCharSC.Tag`, `lblCharBless.Caption`.
    pub max_hp: f64,
    pub rest_rate: f64,
    pub meditate_regen: f64,
    pub max_mana: f64,
    pub mana_rate: f64,
    pub spellcasting: f64,
    pub bless_total: f64,
    /// `nGlobalCharWeaponNumber(0..1)`: weapon and off-hand.
    pub weapon_number: [i32; 2],
    pub weapon_accy: [i32; 2],
    pub weapon_bs_accy: [i32; 2],
    pub punch_skill: [i32; 2],
    pub punch_accy: [i32; 2],
    pub punch_dmg: [i32; 2],
    pub kick_skill: [i32; 2],
    pub kick_accy: [i32; 2],
    pub kick_dmg: [i32; 2],
    pub jk_skill: [i32; 2],
    pub jk_accy: [i32; 2],
    pub jk_dmg: [i32; 2],
    pub accy_items: i32,
    pub accy_other: i32,
    pub accy_abils: i32,
    pub hit_magic_non_weapon: i32,
    pub qnd_bonus: i32,
}

impl Default for CharStats {
    fn default() -> Self {
        Self {
            stat: [0.0; 47],
            encum_current: 0.0,
            encum_max: 0.0,
            base_stat: [0.0; 6],
            class_stealth_tag: 0.0,
            max_hp: 0.0,
            rest_rate: 0.0,
            meditate_regen: 0.0,
            max_mana: 0.0,
            mana_rate: 0.0,
            spellcasting: 0.0,
            bless_total: 0.0,
            weapon_number: [0; 2],
            weapon_accy: [0; 2],
            weapon_bs_accy: [0; 2],
            punch_skill: [0; 2],
            punch_accy: [0; 2],
            punch_dmg: [0; 2],
            kick_skill: [0; 2],
            kick_accy: [0; 2],
            kick_dmg: [0; 2],
            jk_skill: [0; 2],
            jk_accy: [0; 2],
            jk_dmg: [0; 2],
            accy_items: 0,
            accy_other: 0,
            accy_abils: 0,
            hit_magic_non_weapon: 0,
            qnd_bonus: 0,
        }
    }
}

/// The lair-mode controls: `optMonsterFilter(1).Value`, `txtMonsterLairFilter(0..8).Text` and
/// `txtMonsterDamage.Text`. Party size, HP, HP regen and accuracy of a party are typed there.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PartyFilter {
    pub lair_mode: bool,
    pub lair_text: [String; 9],
    pub monster_damage_text: String,
}

/// Everything the procedures read besides the character.
pub struct Context<'a> {
    pub profile: Profile,
    /// `nNMRVer`.
    pub nmr_ver: f64,
    /// `gAvgLevelMaxAllStats`.
    pub avg_level_max_all_stats: f64,
    /// `bHideRecordNumbers`.
    pub hide_record_numbers: bool,
    /// `bGlobal_cephRecoveryOnly`.
    pub recovery_only: bool,
    pub classes: &'a [Class],
    pub items: &'a [Item],
    pub spells: &'a [Spell],
    /// Supplies `GetAbilityStats` to `CalculateSpellCast` (the text is not used here).
    pub text: &'a dyn SpellText,
    pub stats: &'a CharStats,
    pub party: &'a PartyFilter,
}

/// MME's `tCharacterProfile`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CharacterProfile {
    pub is_loaded_character: bool,
    pub party: i16,
    pub hp: f64,
    pub hp_regen: f64,
    pub damage_threshold: f64,
    pub spell_attack_cost: f64,
    pub spell_overhead: f64,
    pub spell_dmg_bonus: i16,
    pub max_mana: f64,
    pub spellcasting: i16,
    pub mana_regen: f64,
    pub meditate_rate: f64,
    pub encum_pct: i16,
    pub encum_current: i32,
    pub encum_max: i32,
    pub walk_speed: f64,
    pub accuracy: f64,
    pub level: i32,
    pub class: i32,
    pub race: i32,
    pub align: i16,
    pub combat: i16,
    pub str_: i16,
    pub agi: i16,
    pub cha: i16,
    pub wis: i16,
    pub int: i16,
    pub hea: i16,
    pub crit: i16,
    pub dodge: i16,
    pub plus_max_damage: i16,
    pub plus_min_damage: i16,
    pub plus_bs_accy: i16,
    pub plus_bs_min_dmg: i16,
    pub plus_bs_max_dmg: i16,
    /// `nMAPlusSkill(1 To 3)`: punch, kick, jumpkick.
    pub ma_plus_skill: [i16; 3],
    pub ma_plus_accy: [i16; 3],
    pub ma_plus_dmg: [i16; 3],
    pub stealth: i16,
    pub class_stealth: bool,
    pub race_stealth: bool,
    pub hit_magic: i32,
    pub hit_magic_non_weapon: i32,
}

impl CharacterProfile {
    /// The five fields `CalculateSpellCast` reads.
    #[must_use]
    pub fn spell_caster(&self) -> SpellCasterProfile {
        SpellCasterProfile {
            spell_dmg_bonus: self.spell_dmg_bonus,
            spellcasting: self.spellcasting,
            max_mana: self.max_mana,
            spell_overhead: self.spell_overhead,
            mana_regen: self.mana_regen,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Small helpers

fn int16(x: f64) -> R<i16> {
    Ok(vb::cint(x)?)
}

fn int32(x: f64) -> R<i32> {
    Ok(vb::clng(x)?)
}

fn to_i16(v: i32) -> R<i16> {
    i16::try_from(v).map_err(|_| Abort)
}

fn cur_long(v: i32) -> Currency {
    Currency::from_scaled(i64::from(v) * 10_000)
}

fn find_spell(spells: &[Spell], n: i32) -> Option<&Spell> {
    spells.iter().find(|s| s.number == n)
}

fn find_class(classes: &[Class], n: i32) -> Option<&Class> {
    classes.iter().find(|c| c.number == n)
}

fn find_item(items: &[Item], n: i32) -> Option<&Item> {
    items.iter().find(|i| i.number == n)
}

fn auto_append(base: &str, add: &str, glue: &str) -> String {
    if base.is_empty() {
        add.to_string()
    } else if add.trim_matches(' ').is_empty() {
        base.to_string()
    } else {
        format!("{base}{glue}{add}")
    }
}

fn remove_spaces(s: &str) -> String {
    s.chars().filter(|c| *c != ' ').collect()
}

/// Port of modMMudDatabase GetSpellManaCost: the mana cost, times how many casts fit in a second
/// for a spell that takes less than a round of energy.
#[must_use]
pub fn get_spell_mana_cost(spells: &[Spell], n: i32) -> i32 {
    if n == 0 {
        return 0;
    }
    let Some(s) = find_spell(spells, n) else {
        return 0;
    };
    if s.energy_cost > 0 && s.energy_cost <= 500 {
        let per = vb::fix(1000.0 / f64::from(s.energy_cost));
        return vb::clng(f64::from(s.mana_cost) * per).unwrap_or(s.mana_cost);
    }
    s.mana_cost
}

fn get_item_name(ctx: &Context, n: i32) -> String {
    if n == 0 {
        return "None".into();
    }
    match find_item(ctx.items, n) {
        Some(i) if ctx.hide_record_numbers => i.name.clone(),
        Some(i) => format!("{}({n})", i.name),
        None => n.to_string(),
    }
}

fn get_spell_name(ctx: &Context, n: i32) -> String {
    if n == 0 {
        return "None".into();
    }
    match find_spell(ctx.spells, n) {
        Some(s) if ctx.hide_record_numbers => s.name.clone().unwrap_or_default(),
        Some(s) => format!("{}({n})", s.name.clone().unwrap_or_default()),
        None => n.to_string(),
    }
}

fn get_spell_short(ctx: &Context, n: i32) -> String {
    if n == 0 {
        return "n/a".into();
    }
    find_spell(ctx.spells, n).map_or_else(|| "n/a".into(), |s| s.short.clone().unwrap_or_default())
}

fn is_two_handed(items: &[Item], n: i32) -> bool {
    n != 0
        && find_item(items, n).is_some_and(|i| i.item_type == 1 && matches!(i.weapon_type, 1 | 3))
}

fn item_ability(items: &[Item], n: i32, ability: i16) -> i32 {
    let slots: Option<&[AbilitySlot]> = find_item(items, n).map(|i| &i.abilities[..]);
    item_has_ability(Profile::Stock, slots, n, ability)
}

// ---------------------------------------------------------------------------------------------
// Alignment and vile ward

/// Port of modMain bCharIsEvilAligned.
#[must_use]
pub fn char_is_evil_aligned(ch: &Character) -> bool {
    ch.global_filter && ch.alignment == 3
}

/// Port of modMain GetCharActiveVileWard: GreaterMUD only, so 0 for the stock rules.
#[must_use]
pub fn get_char_active_vile_ward(profile: Profile) -> i32 {
    let Profile::Stock = profile;
    0
}

// ---------------------------------------------------------------------------------------------
// PopulateCharacterProfile

/// What [`populate_character_profile`] did besides filling the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Populated {
    /// MME wrote `1` into the lair HP box (a party HP below 1).
    pub lair_hp_reset: bool,
}

/// Port of modMain PopulateCharacterProfile: builds the profile of the current character
/// (loaded, party or an unbeatable default) for an attack of kind `mud`. `weapon` is
/// `nWeaponNumber` (positive: that weapon, negative: fists, 0: the current one).
#[allow(clippy::too_many_arguments)]
pub fn populate_character_profile(
    ctx: &Context,
    ch: &Character,
    t: &mut CharacterProfile,
    force_use_char: bool,
    force_no_party: bool,
    mud: AttackTypeMud,
    weapon: i32,
    force_no_char: bool,
) -> Populated {
    let mut out = Populated::default();
    let _ = populate_steps(
        ctx,
        ch,
        t,
        force_use_char,
        force_no_party,
        mud,
        weapon,
        force_no_char,
        &mut out,
    );
    out
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn populate_steps(
    ctx: &Context,
    ch: &Character,
    t: &mut CharacterProfile,
    force_use_char: bool,
    force_no_party: bool,
    mud: AttackTypeMud,
    weapon_number: i32,
    force_no_char: bool,
    out: &mut Populated,
) -> R<()> {
    let Profile::Stock = ctx.profile;
    let st = ctx.stats;
    let a = &ch.attack;
    let use_character = ch.global_filter || force_use_char;
    let lair = |k: usize| vb::val(&ctx.party.lair_text[k]);

    if !force_no_party && ctx.party.lair_mode && lair(0) > 1.0 {
        t.party = int16(lair(0))?;
    }
    t.party = t.party.clamp(1, 6);

    if !force_no_char && ((use_character && t.party < 2) || force_use_char) {
        t.is_loaded_character = true;
        t.level = ch.level;
        t.class = ch.class.ok_or(Abort)?;
        t.race = ch.race.ok_or(Abort)?;
        t.align = ch.alignment;
        t.combat = to_i16(get_class_combat(find_class(ctx.classes, t.class)))?;
        t.encum_current = int32(st.encum_current)?;
        t.encum_max = int32(st.encum_max)?;
        t.encum_pct = calc_encumbrance_percent(cur_long(t.encum_current), cur_long(t.encum_max))?;
        if t.encum_pct > 0 {
            let speed =
                calc_movement_speed(ctx.profile, i32::from(t.encum_pct), int32(st.stat[31])?, 0);
            t.walk_speed = vb::round(f64::from(speed) / 1000.0, 2)?;
        } else {
            t.walk_speed = 1.25;
        }
        t.dodge = int16(st.stat[8])?;
        t.str_ = int16(st.base_stat[0])?;
        t.agi = int16(st.base_stat[3])?;
        t.int = int16(st.base_stat[1])?;
        t.cha = int16(st.base_stat[5])?;
        t.crit = int16(st.stat[7])?;
        t.plus_max_damage = int16(st.stat[11])?;
        t.plus_min_damage = int16(st.stat[30])?;
        t.plus_bs_accy = int16(st.stat[13])?;
        t.plus_bs_min_dmg = int16(st.stat[14])?;
        t.plus_bs_max_dmg = int16(st.stat[15])?;
        t.stealth = int16(st.stat[19])?;
        t.hp = st.max_hp;
        t.hp_regen = st.rest_rate;
        if a.use_meditate {
            t.meditate_rate = st.meditate_regen;
        }
        t.max_mana = st.max_mana;
        t.mana_regen = st.mana_rate;
        t.damage_threshold = f64::from(a.heal_value);
        t.spellcasting = int16(st.spellcasting)?;
        t.spell_dmg_bonus = int16(st.stat[33])?;
        t.spell_overhead = a.heal_cost + st.bless_total / 6.0;

        if matches!(a.attack_type, AttackType::Spell | AttackType::SpellAny) && a.spell_num > 0 {
            t.spell_attack_cost = f64::from(get_spell_mana_cost(ctx.spells, a.spell_num));
        }

        // Only a backstab needs its accuracy recalculated for a stock database.
        if mud == AttackTypeMud::Surprise {
            let mut w = 0;
            if weapon_number > 0 {
                w = weapon_number;
            } else if weapon_number < 0 {
                w = 0;
            } else if a.backstab && a.backstab_weapon > 0 {
                w = a.backstab_weapon;
            } else if !a.backstab || a.backstab_weapon == 0 {
                w = st.weapon_number[0];
            }

            let mut norm_adj: i16 = 0;
            let mut bs_adj: i16 = 0;
            if w != st.weapon_number[0] {
                if w > 0 {
                    norm_adj = to_i16(item_ability(ctx.items, w, 22))?;
                    if norm_adj < 0 {
                        norm_adj = 0;
                    }
                    bs_adj = to_i16(item_ability(ctx.items, w, 116))?;
                    if bs_adj < 0 {
                        bs_adj = 0;
                    }
                    if st.weapon_number[1] > 0 && is_two_handed(ctx.items, w) {
                        t.plus_bs_accy = to_i16(vb::sub_i32(
                            i32::from(t.plus_bs_accy),
                            st.weapon_bs_accy[1],
                        )?)?;
                        norm_adj = to_i16(vb::sub_i32(i32::from(norm_adj), st.weapon_accy[1])?)?;
                    }
                }
                let sum = vb::add_i32(i32::from(t.plus_bs_accy), i32::from(bs_adj))?;
                t.plus_bs_accy = to_i16(vb::sub_i32(sum, st.weapon_bs_accy[0])?)?;
                norm_adj = to_i16(vb::sub_i32(i32::from(norm_adj), st.weapon_accy[0])?)?;
            }

            let plus_normal = vb::add_i32(
                vb::add_i32(
                    vb::add_i32(st.accy_abils, st.accy_other)?,
                    i32::from(norm_adj),
                )?,
                0,
            )?;
            let accy = calculate_backstab_accuracy(
                ctx.profile,
                &BackstabAccuracyInput {
                    stealth: t.stealth,
                    agility: t.agi,
                    plus_bs_accuracy: t.plus_bs_accy,
                    class_stealth: get_class_stealth(find_class(ctx.classes, t.class)),
                    plus_normal_accuracy: to_i16(plus_normal)?,
                },
            );
            t.accuracy = f64::from(accy);
        } else {
            t.accuracy = st.stat[10];
        }

        t.hit_magic = int32(st.stat[12])?;
        t.hit_magic_non_weapon = st.hit_magic_non_weapon;

        // punch, kick, jumpkick
        for (k, (skill, accy, dmg)) in [(37, 40, 34), (38, 41, 35), (39, 42, 36)]
            .into_iter()
            .enumerate()
        {
            t.ma_plus_skill[k] = int16(st.stat[skill])?;
            t.ma_plus_accy[k] = int16(st.stat[accy])?;
            t.ma_plus_dmg[k] = int16(st.stat[dmg])?;
        }
    } else if t.party > 1 && !force_no_party {
        t.hp = lair(5);
        if t.hp < 1.0 {
            out.lair_hp_reset = true;
            t.hp = 1.0;
        }
        t.hp *= f64::from(t.party);
        t.hp_regen = lair(7) * f64::from(t.party);
        t.damage_threshold = vb::val(&ctx.party.monster_damage_text);
        t.accuracy = lair(8);
        t.hit_magic = 9999;
        t.hit_magic_non_weapon = 9999;
        t.walk_speed = 1.25;
        if matches!(
            mud,
            AttackTypeMud::Punch | AttackTypeMud::Kick | AttackTypeMud::Jumpkick
        ) {
            t.ma_plus_skill = [1, 1, 1];
        }
    } else {
        t.level = get_max_level(ctx.avg_level_max_all_stats)?;
        t.combat = 5;
        t.str_ = 255;
        t.agi = 255;
        t.stealth = 255;
        t.accuracy = 999.0;
        t.hit_magic = 9999;
        t.hit_magic_non_weapon = 9999;
        t.plus_bs_accy = 999;
        t.class_stealth = true;
        t.race_stealth = true;
        t.hp = 10000.0;
        t.hp_regen = t.hp * 0.05;
        t.walk_speed = 1.25;
        if matches!(
            mud,
            AttackTypeMud::Punch | AttackTypeMud::Kick | AttackTypeMud::Jumpkick
        ) {
            t.ma_plus_skill = [1, 1, 1];
        }
        if ctx.nmr_ver < 1.83 {
            t.damage_threshold = vb::val(&ctx.party.monster_damage_text);
        } else {
            t.damage_threshold = f64::from(a.heal_value);
            if matches!(a.attack_type, AttackType::Spell | AttackType::SpellAny) && a.spell_num > 0
            {
                t.spell_attack_cost = f64::from(get_spell_mana_cost(ctx.spells, a.spell_num));
            }
        }
    }

    t.damage_threshold = t.damage_threshold.clamp(0.0, 9_999_999.0);
    if t.hp < 1.0 {
        t.hp = 1.0;
    }
    if t.hp_regen < 1.0 {
        t.hp_regen = 1.0;
    }
    t.max_mana = t.max_mana.clamp(0.0, 9_999_999.0);
    t.mana_regen = t.mana_regen.clamp(0.0, 9_999_999.0);
    t.spell_overhead = t.spell_overhead.clamp(0.0, 9_999_999.0);
    t.spell_attack_cost = t.spell_attack_cost.clamp(0.0, 9_999_999.0);
    t.accuracy = t.accuracy.clamp(0.0, 9_999_999.0);
    t.encum_pct = t.encum_pct.clamp(0, 100);
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// SetCurrentAttackTypeConfig

/// The attack-config key and whether it differs from the previous one (MME then calls
/// `ClearSavedDamageVsMonster`, which copies the previous key into `sCharDamageVsMonsterConfig`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttackConfigKey {
    pub config: String,
    pub changed: bool,
}

/// Port of modMain SetCurrentAttackTypeConfig: the string that says whether the attack set-up
/// changed. `previous` is `sGlobalAttackConfig`. On a VB6 error the key stays `previous`.
#[must_use]
pub fn set_current_attack_type_config(
    ctx: &Context,
    ch: &Character,
    previous: &str,
) -> AttackConfigKey {
    match config_key(ctx, ch) {
        Ok(config) => {
            let changed = config != previous;
            AttackConfigKey { config, changed }
        }
        Err(Abort) => AttackConfigKey {
            config: previous.to_string(),
            changed: false,
        },
    }
}

fn encum_percent(ctx: &Context) -> R<i16> {
    Ok(calc_encumbrance_percent(
        Currency::from_f64(ctx.stats.encum_current)?,
        Currency::from_f64(ctx.stats.encum_max)?,
    )?)
}

fn config_key(ctx: &Context, ch: &Character) -> R<String> {
    let st = ctx.stats;
    let a = &ch.attack;
    let n = |x: f64| vb::cstr(x);
    let mut s = (a.attack_type as i32).to_string();
    if ch.global_filter {
        s += &format!("_{}", ch.level);
        s += &format!("_{}", ch.class.ok_or(Abort)?);
        s += &format!("_{}", encum_percent(ctx)?);
    }

    match a.attack_type {
        AttackType::PhysAttack
        | AttackType::PhysBash
        | AttackType::PhysSmash
        | AttackType::MartialArts => {
            s += &format!("_{}", st.weapon_number[0]);
            s += &format!("_{}", st.weapon_number[1]);
            if ch.global_filter {
                s += &format!("_{}", n(st.base_stat[0]));
                s += &format!("_{}", n(st.base_stat[3]));
                s += &format!("_{}", n(st.stat[7] - f64::from(st.qnd_bonus)));
                for i in [11, 30, 10, 13, 14, 15, 19] {
                    s += &format!("_{}", n(st.stat[i]));
                }
                if st.class_stealth_tag >= 2.0 {
                    s += "1";
                }
            } else {
                s += "_default";
            }
        }
        AttackType::Spell | AttackType::SpellAny => {
            s += &format!("_{}", a.spell_num);
            if ch.global_filter {
                s += &format!("_{}", ch.level);
            } else {
                s += &format!("_{}", a.spell_lvl);
            }
            if a.use_meditate {
                s += "_med";
            }
        }
        AttackType::Manual => s += &format!("_{}_{}", a.manual_p, a.manual_m),
        AttackType::OneShot => {}
    }

    if a.attack_type == AttackType::MartialArts {
        s += &format!("_{}", a.martial_art);
        let sum = |x: [i32; 2]| vb::add_i32(x[0], x[1]);
        let triple = match a.martial_art {
            1 => Some((st.punch_skill, st.punch_accy, st.punch_dmg)),
            2 => Some((st.kick_skill, st.kick_accy, st.kick_dmg)),
            3 => Some((st.jk_skill, st.jk_accy, st.jk_dmg)),
            _ => None,
        };
        if let Some((skill, accy, dmg)) = triple {
            s += &format!("_{}", sum(skill)?);
            s += &format!("_{}", sum(accy)?);
            s += &format!("_{}", sum(dmg)?);
        }
    }

    if a.attack_type != AttackType::OneShot && a.backstab {
        s += &format!("_BS{}", a.backstab_weapon);
    }
    Ok(s)
}

// ---------------------------------------------------------------------------------------------
// SetCharDefenseDescription

/// The defense key and whether it differs from the previous one (MME then clears
/// `bDontPromptCalcCharMonsterDamage`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefenseDescription {
    pub description: String,
    pub reset_prompt: bool,
}

/// Port of modMain SetCharDefenseDescription: a key over the defensive numbers.
/// `previous` is `sGlobalCharDefenseDescription`; `None` where MME exits without touching it
/// (no class selected, or a VB6 error).
#[must_use]
pub fn set_char_defense_description(
    ctx: &Context,
    ch: &Character,
    previous: &str,
) -> Option<DefenseDescription> {
    let st = ctx.stats;
    let n = |x: f64| vb::cstr(x);
    ch.class?;
    let mut s = encum_percent(ctx).ok()?.to_string();
    if ch.global_filter {
        s += &format!("_{}_{}", ch.level, ch.class?);
    }
    s += &format!("_{}_{}", n(st.stat[2]), n(st.stat[3]));
    s += &format!("_{}", ch.mr_text);
    s += &format!("_{}", n(st.stat[8]));
    s += &format!("_{}", i32::from(ch.anti_magic));
    for i in [28, 27, 25, 29, 26] {
        s += &format!("_{}", n(st.stat[i]));
    }
    s += &format!("_{}", get_char_active_vile_ward(ctx.profile));
    let reset_prompt = s != previous;
    Some(DefenseDescription {
        description: s,
        reset_prompt,
    })
}

// ---------------------------------------------------------------------------------------------
// RefreshCombatHealingValues

/// Port of modMain RefreshCombatHealingValues: works out what the chosen heal gives per round
/// (`heal_value`) and costs in mana per round (`heal_cost`), and clamps the heal rounds and level
/// it was given. On a VB6 error the values stay as they were when it hit, without the final clamps.
pub fn refresh_combat_healing_values(ctx: &Context, ch: &mut Character) {
    let _ = heal_steps(ctx, ch);
}

fn heal_steps(ctx: &Context, ch: &mut Character) -> R<()> {
    ch.attack.heal_cost = 0.0;
    ch.attack.heal_value = 0;
    if ctx.party.lair_mode && vb::val(&ctx.party.lair_text[0]) > 1.0 {
        ch.attack.heal_value = int32(vb::val(&ctx.party.monster_damage_text))?;
        return Ok(());
    }
    let use_character = ch.global_filter;

    match ch.attack.heal_type {
        0 => ch.attack.heal_value = 999_999,
        1 => ch.attack.heal_value = 0,
        2 | 3 => {
            if ch.attack.heal_spell_num > 0 {
                let a = &mut ch.attack;
                a.heal_spell_lvl = a.heal_spell_lvl.clamp(0, 9999);
                a.heal_rounds = a.heal_rounds.clamp(1, 50);
                let mut t = CharacterProfile::default();
                populate_character_profile(
                    ctx,
                    ch,
                    &mut t,
                    false,
                    true,
                    AttackTypeMud::None,
                    0,
                    false,
                );
                let env = SpellCastEnv {
                    profile: ctx.profile,
                    spells: ctx.spells,
                    text: ctx.text,
                    avg_level_max_all_stats: ctx.avg_level_max_all_stats,
                };
                let cast = env.calculate_spell_cast(
                    &t.spell_caster(),
                    &CastRequest {
                        spell: ch.attack.heal_spell_num,
                        cast_level: if use_character { t.level } else { 0 },
                        ..CastRequest::default()
                    },
                );
                let dur = get_spell_duration(
                    ctx.spells,
                    ch.attack.heal_spell_num,
                    to_i16(t.level)?,
                    false,
                );
                let a = &mut ch.attack;
                let rounds = f64::from(a.heal_rounds);
                if dur > 1 {
                    a.heal_value = cast.n_avg_cast;
                    if i32::from(a.heal_rounds) < dur {
                        a.heal_rounds = to_i16(dur)?;
                    } else {
                        let v = f64::from(a.heal_value);
                        a.heal_value = int32(v - (v * (1.0 - (f64::from(dur) / rounds))))?;
                    }
                } else {
                    a.heal_value = int32(vb::round(f64::from(cast.n_avg_cast) / rounds, 2)?)?;
                }
                a.heal_cost = vb::round(f64::from(cast.n_mana_cost) / f64::from(a.heal_rounds), 2)?;
            }
        }
        4 => ch.attack.heal_value = ch.attack.heal_manual,
        _ => {}
    }

    let a = &mut ch.attack;
    if a.heal_cost < 0.1 {
        a.heal_cost = 0.0;
    }
    if a.heal_cost > 9999.0 {
        a.heal_cost = 9999.0;
    }
    a.heal_value = a.heal_value.clamp(0, 999_999);
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// GetCurrentAttackName

/// Port of modMain GetCurrentAttackName: a short (or, with `force_desc`, long) name of the
/// current attack and heal choice. Without `force_desc` in lair mode (NMR 1.83+) it refreshes the
/// heal values first, which changes `ch.attack`. On a VB6 error it returns what it had built.
pub fn get_current_attack_name(ctx: &Context, ch: &mut Character, force_desc: bool) -> String {
    let mut name = String::new();
    let _ = attack_name(ctx, ch, force_desc, &mut name);
    name
}

fn attack_name(ctx: &Context, ch: &mut Character, force: bool, name: &mut String) -> R<()> {
    let a = ch.attack;
    match a.attack_type {
        AttackType::PhysAttack | AttackType::PhysBash | AttackType::PhysSmash => {
            if ch.equipped[16] > 0 {
                *name = match a.attack_type {
                    AttackType::PhysBash => "bash",
                    AttackType::PhysSmash => "smash",
                    _ if force => "normal attack",
                    _ => "weapon",
                }
                .into();
                if force {
                    *name += &format!(" w/{}", get_item_name(ctx, ch.equipped[16]));
                }
            } else {
                *name = "no wepn!".into();
            }
        }
        AttackType::Spell => {
            if force {
                *name = format!("{} / ", get_spell_name(ctx, a.spell_num));
            }
            *name += &get_spell_short(ctx, a.spell_num);
            if force {
                *name += " @ LVL";
            }
        }
        AttackType::SpellAny => {
            if force {
                *name = format!("{} / ", get_spell_name(ctx, a.spell_num));
            }
            *name += &format!("{}@{}", get_spell_short(ctx, a.spell_num), a.spell_lvl);
        }
        AttackType::MartialArts => {
            *name = match a.martial_art {
                2 => "kick",
                3 => "jumpkick",
                _ => "punch",
            }
            .into();
        }
        AttackType::Manual => {
            let total = vb::add_i32(a.manual_p, a.manual_m)?;
            *name = if total == 0 {
                "zero".into()
            } else if a.manual_p > 0 && a.manual_m <= 0 {
                format!("{} phys", a.manual_p)
            } else if a.manual_p <= 0 && a.manual_m > 0 {
                format!("{} mag", a.manual_m)
            } else {
                "man dmg".into()
            };
        }
        AttackType::OneShot => *name = "one-shot".into(),
    }

    if a.attack_type != AttackType::OneShot && a.backstab {
        if force && a.backstab_weapon > 0 {
            *name = auto_append(name, "+bs", "\r\n");
        } else {
            *name += "+bs";
        }
        if a.backstab_weapon > 0 {
            if force {
                *name += &format!(" w/{}", get_item_name(ctx, a.backstab_weapon));
            } else {
                *name += "*";
            }
        }
    }

    if !force && ctx.party.lair_mode && ctx.nmr_ver >= 1.83 && !ctx.recovery_only {
        refresh_combat_healing_values(ctx, ch);
        let a = ch.attack;
        match a.heal_type {
            0 => *name = auto_append(name, "invincible", " / "),
            1 => *name = auto_append(name, "passive", " / "),
            2 | 3 => {
                if a.heal_spell_num > 0 {
                    *name = auto_append(name, &get_spell_short(ctx, a.heal_spell_num), " / ");
                    if a.heal_type == 3 {
                        *name += &format!("@{}", a.heal_spell_lvl);
                        *name = remove_spaces(name);
                    }
                    if a.heal_value > 0 {
                        *name += &format!("({})", a.heal_value);
                    }
                }
            }
            4 => *name = auto_append(name, &format!("{} heals", a.heal_value), " / "),
            _ => {}
        }
        if a.use_meditate
            && (matches!(a.attack_type, AttackType::Spell | AttackType::SpellAny)
                || matches!(a.heal_type, 2 | 3))
        {
            *name = auto_append(name, "+m", "");
            if a.attack_type == AttackType::SpellAny || a.heal_type == 3 {
                *name = remove_spaces(name);
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Learned spells

impl Character {
    /// Port of modMain ClearLearnedSpells (the form unload and colour refresh are UI).
    pub fn clear_learned_spells(&mut self) {
        self.learned = LearnedSpells::default();
    }

    /// Port of modMain LearnSpell: puts the spell in the first free slot and remembers the class
    /// it was learned for. With no class selected MME's handler stops after the slot is set.
    pub fn learn_spell(&mut self, spell: i32) {
        for x in 0..100 {
            if self.learned.spells[x] == 0 {
                self.learned.spells[x] = spell;
                let Some(class) = self.class else {
                    return;
                };
                if self.learned.spells[x] > 0
                    && class > 0
                    && i32::from(self.learned.class) != class
                    && let Ok(c) = i16::try_from(class)
                {
                    self.learned.class = c;
                }
                break;
            }
        }
    }

    /// Port of modMain UnLearnSpell: clears every slot holding the spell.
    pub fn unlearn_spell(&mut self, spell: i32) {
        for s in &mut self.learned.spells {
            if *s == spell {
                *s = 0;
            }
        }
    }

    /// Port of modMain LearnOrUnlearnSpell. The list includes its empty slots, so spell 0 always
    /// counts as learned.
    pub fn learn_or_unlearn_spell(&mut self, spell: i32) {
        if self.learned.spells.contains(&spell) {
            self.unlearn_spell(spell);
        } else {
            self.learn_spell(spell);
        }
    }
}
