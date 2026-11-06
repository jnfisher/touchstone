//! The 1.20.0 balance, frozen: one constant per Quicksilver `[balance]` key, so each value carries
//! the key it came from. A retune is a new content version, never an edit here.

use super::balance::{
    Anchors, Balance, ClassMeleePct, ClassSpellPct, ClassWeavePct, LevelPct, RankWeights,
};

/// A stab costs this percentage of a normal blow's energy.
///
/// Quicksilver key `balance.backstab_energy_pct`.
pub(super) const BACKSTAB_ENERGY_PCT: u32 = 150;

/// Stealth adds stealth over this to a stab (stock: 10).
///
/// Quicksilver key `balance.backstab_stealth_div`.
pub(super) const BACKSTAB_STEALTH_DIV: u32 = 4;

/// A stab's damage is multiplied by (100 + level x this / 100) percent: 200 is +2% a level (stock: 100).
///
/// Quicksilver key `balance.backstab_level_pct`.
pub(super) const BACKSTAB_LEVEL_PCT: u32 = 200;

/// A stab crit rolls from max up to this percentage of max.
///
/// Quicksilver key `balance.backstab_crit_max_pct`.
pub(super) const BACKSTAB_CRIT_MAX_PCT: u32 = 200;

/// Percentage chance of the Thief's extra blow.
///
/// Quicksilver key `balance.opportunist_pct`.
pub(super) const OPPORTUNIST_PCT: u32 = 10;

/// The opportunist bonus grows by this much per level, in hundredths of a percent.
///
/// Quicksilver key `balance.opportunist_level_pct`.
pub(super) const OPPORTUNIST_LEVEL_PCT: u32 = 50;

/// Percentage chance of a weave proc.
///
/// Quicksilver key `balance.weave_pct`.
pub(super) const WEAVE_PCT: u32 = 20;

/// The proc chance for the magery-1 hybrids (Paladin, Ranger).
///
/// Quicksilver key `balance.weave_pct_magery1`.
pub(super) const WEAVE_PCT_MAGERY1: u32 = 10;

/// A proc's expected damage is this times the (capped) level.
///
/// Quicksilver key `balance.weave_budget_per_level`.
pub(super) const WEAVE_BUDGET_PER_LEVEL: u32 = 3;

/// The least power of a proc, a percentage of the spell's damage.
///
/// Quicksilver key `balance.weave_power_min_pct`.
pub(super) const WEAVE_POWER_MIN_PCT: u32 = 25;

/// The most power of a proc, a percentage of the spell's damage.
///
/// Quicksilver key `balance.weave_power_max_pct`.
pub(super) const WEAVE_POWER_MAX_PCT: u32 = 100;

/// The share of the casting-stat damage bonus a proc gets, in percent.
///
/// Quicksilver key `balance.weave_stat_bonus_pct`.
pub(super) const WEAVE_STAT_BONUS_PCT: u32 = 50;

/// Per mille added to spell damage per point of casting stat over 50.
///
/// Quicksilver key `balance.spell_stat_permille`.
pub(super) const SPELL_STAT_PERMILLE: u32 = 5;

/// `rest` multiplies mana regeneration by this.
///
/// Quicksilver key `balance.rest_mana_mult`.
pub(super) const REST_MANA_MULT: u32 = 3;

/// `meditate` multiplies mana regeneration by this.
///
/// Quicksilver key `balance.meditate_mana_mult`.
pub(super) const MEDITATE_MANA_MULT: u32 = 5;

/// Crit chance above this counts a third (stock: 40).
///
/// Quicksilver key `balance.crit_knee`.
pub(super) const CRIT_KNEE: u32 = 50;

/// The casting stat above this counts a third.
///
/// Quicksilver key `balance.spell_stat_knee`.
pub(super) const SPELL_STAT_KNEE: u32 = 100;

/// A player's stat part of magic resistance above this counts a third.
///
/// Quicksilver key `balance.mr_stat_knee`.
pub(super) const MR_STAT_KNEE: u32 = 110;

/// A player's Agility above this counts a third in the dodge term.
///
/// Quicksilver key `balance.dodge_agi_knee`.
pub(super) const DODGE_AGI_KNEE: u32 = 150;

/// Training prices a stat at the floor price up to this value.
///
/// Quicksilver key `balance.softcap_floor`.
pub(super) const SOFTCAP_FLOOR: u32 = 90;

/// Character points per stat point from the race maximum up to the floor.
///
/// Quicksilver key `balance.softcap_floor_price`.
pub(super) const SOFTCAP_FLOOR_PRICE: u32 = 10;

/// Character points per point in the bands +1..10, +11..20, +21..30 over the soft cap.
///
/// Quicksilver key `balance.softcap_band_prices`.
pub(super) const SOFTCAP_BAND_PRICES: [u32; 3] = [15, 25, 40];

/// The most a monster's resist roll cuts an area spell, a percentage.
///
/// Quicksilver key `balance.aoe_resist_cut_max_pct`.
pub(super) const AOE_RESIST_CUT_MAX_PCT: u32 = 25;

/// The area cut is the monster's magic resistance over this, up to the maximum.
///
/// Quicksilver key `balance.aoe_resist_cut_div`.
pub(super) const AOE_RESIST_CUT_DIV: u32 = 4;

/// `guard`: base percentage chance to intercept (not used by touchstone).
///
/// Quicksilver key `balance.guard_chance_base`.
pub(super) const GUARD_CHANCE_BASE: u32 = 25;

/// `guard`: percentage added per point of combat rating (not used by touchstone).
///
/// Quicksilver key `balance.guard_chance_per_combat`.
pub(super) const GUARD_CHANCE_PER_COMBAT: u32 = 5;

/// The level past which backstab, opportunist and weave stop scaling.
///
/// Quicksilver key `balance.scalecap`.
pub(super) const SCALECAP: u32 = 60;

/// Monster targeting weights by party rank (front 3, middle 2, back 1, solo 2).
///
/// Quicksilver key `balance.rank_weights` (`front`, `mid`, `back`, `solo`).
pub(super) const RANK_WEIGHTS: RankWeights = RankWeights {
    front: 3,
    mid: 2,
    back: 1,
    solo: 2,
};

/// Each class's share of its damage range on non-stab blows. The Warrior is the benchmark at 100;
/// the weaving hybrids keep 60 to 70 percent and let the weave carry the rest of their cut.
///
/// Quicksilver key `balance.class_melee_pct`.
pub(super) const CLASS_MELEE_PCT: ClassMeleePct = ClassMeleePct {
    warrior: LevelPct::One(100),
    witchunter: LevelPct::One(114),
    paladin: LevelPct::One(88),
    cleric: LevelPct::One(70),
    priest: LevelPct::One(100),
    missionary: LevelPct::One(70),
    ninja: LevelPct::Points(Anchors::new([(40, 77), (60, 96)])),
    thief: LevelPct::One(96),
    bard: LevelPct::One(70),
    gypsy: LevelPct::One(60),
    warlock: LevelPct::One(70),
    mage: LevelPct::One(100),
    druid: LevelPct::One(100),
    ranger: LevelPct::One(60),
    mystic: LevelPct::Points(Anchors::new([(40, 50), (60, 77)])),
};

/// Each class's share of damage-spell damage, after the casting-stat bonus. Only the
/// casting-only classes are trimmed (Priest, Mage); the hybrids' spells stay whole.
///
/// Quicksilver key `balance.class_spell_pct`.
pub(super) const CLASS_SPELL_PCT: ClassSpellPct = ClassSpellPct {
    priest: LevelPct::One(67),
    mage: LevelPct::Points(Anchors::new([(40, 87), (60, 93)])),
    ..ClassSpellPct::even(100)
};

/// Each weaving class's share of weave-proc damage, on top of `class_spell_pct`; every other
/// class is whole.
///
/// Quicksilver key `balance.class_weave_pct`.
pub(super) const CLASS_WEAVE_PCT: ClassWeavePct = ClassWeavePct {
    cleric: LevelPct::One(49),
    missionary: LevelPct::Points(Anchors::new([(30, 70), (40, 86)])),
    bard: LevelPct::Points(Anchors::new([(40, 35), (60, 38)])),
    gypsy: LevelPct::Points(Anchors::new([(30, 50), (40, 68), (60, 122)])),
    warlock: LevelPct::Points(Anchors::new([(40, 64), (60, 84)])),
    ranger: LevelPct::One(50),
    ..ClassWeavePct::even(100)
};

pub(super) const BALANCE: Balance = Balance {
    backstab_energy_pct: BACKSTAB_ENERGY_PCT,
    backstab_stealth_div: BACKSTAB_STEALTH_DIV,
    backstab_level_pct: BACKSTAB_LEVEL_PCT,
    backstab_crit_max_pct: BACKSTAB_CRIT_MAX_PCT,
    opportunist_pct: OPPORTUNIST_PCT,
    opportunist_level_pct: OPPORTUNIST_LEVEL_PCT,
    weave_pct: WEAVE_PCT,
    weave_pct_magery1: WEAVE_PCT_MAGERY1,
    weave_budget_per_level: WEAVE_BUDGET_PER_LEVEL,
    weave_power_min_pct: WEAVE_POWER_MIN_PCT,
    weave_power_max_pct: WEAVE_POWER_MAX_PCT,
    weave_stat_bonus_pct: WEAVE_STAT_BONUS_PCT,
    spell_stat_permille: SPELL_STAT_PERMILLE,
    rest_mana_mult: REST_MANA_MULT,
    meditate_mana_mult: MEDITATE_MANA_MULT,
    crit_knee: CRIT_KNEE,
    spell_stat_knee: SPELL_STAT_KNEE,
    mr_stat_knee: MR_STAT_KNEE,
    dodge_agi_knee: DODGE_AGI_KNEE,
    softcap_floor: SOFTCAP_FLOOR,
    softcap_floor_price: SOFTCAP_FLOOR_PRICE,
    softcap_band_prices: SOFTCAP_BAND_PRICES,
    aoe_resist_cut_max_pct: AOE_RESIST_CUT_MAX_PCT,
    aoe_resist_cut_div: AOE_RESIST_CUT_DIV,
    rank_weights: RANK_WEIGHTS,
    guard_chance_base: GUARD_CHANCE_BASE,
    guard_chance_per_combat: GUARD_CHANCE_PER_COMBAT,
    scalecap: SCALECAP,
    class_melee_pct: CLASS_MELEE_PCT,
    class_spell_pct: CLASS_SPELL_PCT,
    class_weave_pct: CLASS_WEAVE_PCT,
};

/// Every parametric rule at its stock-equivalent value; see `Balance::NEUTRAL`.
pub(super) const NEUTRAL: Balance = Balance {
    backstab_stealth_div: 10,
    backstab_level_pct: 100,
    // R2: a stab that costs 1000 times a normal blow fills the round whatever the blow costs (the
    // energy is capped at the round's), which is MME's stab: it ends the round.
    backstab_energy_pct: 100_000,
    // 0 turns the stab crit off (R3), the nearest the type comes to MME, where a stab never crits.
    backstab_crit_max_pct: 0,
    opportunist_pct: 0,
    weave_pct: 0,
    weave_pct_magery1: 0,
    weave_stat_bonus_pct: 0,
    spell_stat_permille: 0,
    rest_mana_mult: 1,
    meditate_mana_mult: 1,
    crit_knee: 40,
    // R1: the stab reads the level held to this; MME's stab has no cap.
    scalecap: KNEE_OUT_OF_REACH,
    spell_stat_knee: KNEE_OUT_OF_REACH,
    mr_stat_knee: KNEE_OUT_OF_REACH,
    dodge_agi_knee: KNEE_OUT_OF_REACH,
    softcap_floor: 0,
    softcap_floor_price: 10,
    softcap_band_prices: [10, 10, 10],
    aoe_resist_cut_max_pct: 0,
    aoe_resist_cut_div: 1,
    class_melee_pct: ClassMeleePct::even(100),
    class_spell_pct: ClassSpellPct::even(100),
    class_weave_pct: ClassWeavePct::even(100),
    ..BALANCE
};

/// A knee no value reaches, so the third-above-the-knee rule never applies. MME's formulas take
/// 32-bit stats (the fixtures go to the extremes), so this is the largest 32-bit value rather than
/// the game's 16-bit stat ceiling.
const KNEE_OUT_OF_REACH: u32 = i32::MAX as u32;

/// The 1.20.0 balance as a `static`, for `Profile::Quicksilver(&V120_BALANCE)`.
pub static V120_BALANCE: Balance = Balance::V120;
