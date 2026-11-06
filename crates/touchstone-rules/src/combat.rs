//! Damage vs monster and combat rounds (FAM-08). Card P3-12.
//!
//! [`get_damage_output`] is "how much damage does the character do to monster N per round" for the
//! current attack choice; [`calc_combat_rounds`] turns a damage per round into rounds to kill and
//! to die; [`is_mob_killable`] is the yes/no version used for list colouring.
//!
//! MME's `GetDamageOutput` also keeps the answer in six module-level arrays and hands it back from
//! there while the attack configuration is unchanged. The port is a pure function: it always
//! computes and returns the value, with no cache. `ClearSavedDamageVsMonster` only empties that
//! cache, so it is not ported (see [`crate::character::AttackConfigKey::changed`] for when MME
//! would call it).
//!
//! The character side is the P3-11 input set (`Context`, `Character`) plus the equipped-weapon
//! globals and spell text that `calculate_attack` takes. MME calls `PopulateCharacterProfile` up to
//! three times on one reused `tCharacterProfile`; so does the port, because fields a later call
//! leaves alone keep what an earlier one wrote.
//!
//! A VB6 error inside `GetDamageOutput` returns the start value (MME assigns the result before
//! doing any work and `Resume out` hands it back); inside `CalcCombatRounds` and `IsMobKillable` it
//! returns what was built so far, as those handlers do. GreaterMUD branches are not ported.

use touchstone_model::{Monster, Race};
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Vb6Error};

use crate::Profile;
use crate::abilities::item_has_ability;
use crate::attack::{
    self, AttackEnv, AttackRequest, CastText, CharacterProfile as AttackProfile, EquippedGlobals,
    calculate_attack,
};
use crate::character::{AttackType, AttackTypeMud, Character, Context, populate_character_profile};
use crate::quicksilver::weave::{self, WeaveTarget};
use crate::spell_cast::{CastRequest, SpellCastEnv, UsableEnv, calculate_resist_damage};

/// `STOCK_MOB_HPREGEN_ROUNDS`: rounds between a monster's HP regeneration ticks.
pub const STOCK_MOB_HPREGEN_ROUNDS: f64 = 18.0;

// ---------------------------------------------------------------------------------------------
// ccr_* helpers (Private in MME; public here so the fixture can pin their edges)

/// Port of modMMudFunc ccr_Saturate: clamp to 0..=1.
// `f64::clamp` would keep -0.0 where MME returns 0.
#[allow(clippy::manual_clamp)]
#[must_use]
pub fn ccr_saturate(x: f64) -> f64 {
    if x <= 0.0 {
        0.0
    } else if x >= 1.0 {
        1.0
    } else {
        x
    }
}

/// Port of modMMudFunc ccr_SmoothStep: 0 below `edge0`, 1 above `edge1`, a cubic between; equal
/// edges make a step at `edge1`.
#[must_use]
pub fn ccr_smooth_step(edge0: f64, edge1: f64, x: f64) -> f64 {
    // The comparison is exact, as in MME (no tolerance).
    #[allow(clippy::float_cmp)]
    if edge0 == edge1 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ccr_saturate((x - edge0) / (edge1 - edge0));
    t * t * (3.0 - 2.0 * t)
}

/// Port of modMMudFunc ccr_Lerp.
#[must_use]
pub fn ccr_lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Port of modMMudFunc ccr_SafeDiv: `n / d`, or `def` when `d` is exactly 0.
#[must_use]
pub fn ccr_safe_div(n: f64, d: f64, def: f64) -> f64 {
    #[allow(clippy::float_cmp)]
    if d == 0.0 { def } else { n / d }
}

/// Port of modMMudFunc ccr_Min (returns `b` when they are equal or unordered).
#[must_use]
pub fn ccr_min(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}

/// Port of modMMudFunc ccr_Max (returns `b` when they are equal or unordered).
#[must_use]
pub fn ccr_max(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

// ---------------------------------------------------------------------------------------------
// CalcCombatRounds

/// The arguments of `CalcCombatRounds`; [`Default`] is MME's defaults for the optional ones.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CombatRoundsArgs {
    /// `nDamageOut` (default -9999): the character's damage per round.
    pub damage_out: i32,
    pub mob_health: i32,
    /// `nMobDamage` (default -1): the monster's damage per round against the character.
    pub mob_damage: i32,
    pub char_health: i32,
    pub mob_hp_regen: i32,
    /// `nNumMobs` (default 1).
    pub num_mobs: f64,
    /// `nOverrideRTK`: rounds to kill one monster, when the caller has its own figure.
    pub override_rtk: f64,
    /// `nSurpriseDamageOut` (default -9999).
    pub surprise_damage_out: f64,
    /// `nFirstRoundDamageOut` (default -9999).
    pub first_round_damage_out: i32,
}

impl Default for CombatRoundsArgs {
    fn default() -> Self {
        Self {
            damage_out: -9999,
            mob_health: 0,
            mob_damage: -1,
            char_health: 0,
            mob_hp_regen: 0,
            num_mobs: 1.0,
            override_rtk: 0.0,
            surprise_damage_out: -9999.0,
            first_round_damage_out: -9999,
        }
    }
}

/// MME's `tCombatRoundInfo`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CombatRoundInfo {
    /// Rounds to kill (a whole pack when `num_mobs` > 1).
    pub rtk: f64,
    /// Rounds to die.
    pub rtd: f64,
    pub rtk_text: String,
    pub rtd_text: String,
    /// Percent chance of winning.
    pub success: i16,
    pub success_text: String,
}

/// Port of modMMudFunc CalcCombatRounds: rounds to kill and to die, with monster HP regeneration,
/// packs, a first-round damage and a surprise opener.
///
/// A VB6 error (Long overflow of the regenerated HP) returns the partly filled result.
#[must_use]
pub fn calc_combat_rounds(profile: Profile, a: &CombatRoundsArgs) -> CombatRoundInfo {
    // Quicksilver (R2) needs no branch here: its surprise figure already holds the whole opening
    // round (the stab and the blows after it), and the credit below counts that round once, as the
    // one it replaces. Stock's stab was the whole round.
    let (Profile::Stock | Profile::Quicksilver(_)) = profile;
    let mut r = CombatRoundInfo::default();
    let _ = combat_rounds_steps(a, &mut r);
    r
}

/// `-Int(-(x * 2)) / 2`: round up to the next half.
fn round_up_half(x: f64) -> f64 {
    -vb::int(-(x * 2.0)) / 2.0
}

#[allow(clippy::too_many_lines)] // one MME procedure, kept in its order
fn combat_rounds_steps(a: &CombatRoundsArgs, r: &mut CombatRoundInfo) -> Result<(), Vb6Error> {
    let mut num_mobs = a.num_mobs;
    let mut mob_health = a.mob_health;
    let damage_out = a.damage_out;
    let regen_rounds = STOCK_MOB_HPREGEN_ROUNDS;

    if num_mobs < 1.0 {
        num_mobs = 1.0;
    }
    if a.mob_damage > 0 && a.char_health > 0 {
        r.rtd = vb::round(f64::from(a.char_health) / f64::from(a.mob_damage), 1)?;
    }
    if a.override_rtk > 0.0 {
        r.rtk = a.override_rtk * num_mobs;
    }

    if damage_out > 0 && mob_health > 1 {
        let mut mob_hp = mob_health;
        if num_mobs > 1.0 {
            mob_hp = vb::clng(f64::from(mob_hp) / num_mobs)?;
        }

        if a.override_rtk < 1.0 {
            r.rtk = vb::round(f64::from(mob_hp) / f64::from(damage_out), 2)?;
            r.rtk = round_up_half(r.rtk);
            r.rtk *= num_mobs;
        }

        if a.mob_hp_regen > 0 && r.rtk >= regen_rounds * 0.9 {
            // add hp if it takes that long to kill mob
            let mut test = 1.0;
            while (r.rtk - test * regen_rounds) / regen_rounds >= 0.9 {
                test += 1.0;
            }
            mob_health = vb::clng(f64::from(mob_health) + test * f64::from(a.mob_hp_regen))?;
            r.rtk = vb::round(f64::from(mob_health) / f64::from(damage_out), 2)?;
        }

        // The comparison with 1 is exact: the value is a rounded figure or a whole count.
        #[allow(clippy::float_cmp)]
        if r.rtk == 1.0
            && a.first_round_damage_out >= 0
            && a.first_round_damage_out < damage_out
            && a.first_round_damage_out < mob_hp
        {
            let first = f64::from(a.first_round_damage_out);
            let min_dmg_pct = (f64::from(mob_hp) - first) / (f64::from(damage_out) - first);
            if min_dmg_pct >= 0.5 {
                r.rtk = 1.5;
            }
        }
    }

    // Surprise opener credit (the first target replaces the first normal round).
    if a.surprise_damage_out > 0.0 && mob_health > 1 && r.rtk > 0.0 {
        let hp_per_mob = ccr_safe_div(f64::from(mob_health), num_mobs, f64::from(mob_health));

        let mut rtk_single_normal;
        if a.override_rtk > 0.0 {
            rtk_single_normal = a.override_rtk;
        } else {
            rtk_single_normal = ccr_safe_div(hp_per_mob, ccr_max(1.0, f64::from(damage_out)), 1.0);
            rtk_single_normal = round_up_half(rtk_single_normal);
        }

        let mut rtk_single_surp = 1.0
            + ccr_safe_div(
                ccr_max(0.0, hp_per_mob - a.surprise_damage_out),
                ccr_max(1.0, f64::from(damage_out)),
                0.0,
            );
        rtk_single_surp = round_up_half(rtk_single_surp);

        let delta_first = rtk_single_normal - rtk_single_surp;

        #[allow(clippy::float_cmp)]
        if delta_first != 0.0 {
            let regen_per_round = f64::from(a.mob_hp_regen) / regen_rounds;
            let regen_ratio =
                ccr_safe_div(regen_per_round, ccr_max(1.0, f64::from(damage_out)), 0.0);
            let regen_atten = 1.0 - 0.45 * ccr_smooth_step(0.0, 0.6, regen_ratio);

            let pack_fade = 1.0 / vb::sqr(ccr_max(1.0, num_mobs))?;
            let fade_gate = ccr_smooth_step(3.0, 8.0, num_mobs);

            let adj = delta_first * regen_atten * ccr_lerp(1.0, pack_fade, fade_gate);
            r.rtk = ccr_max(num_mobs, r.rtk - adj);
        }
    }

    if num_mobs > 1.0 && r.rtk > 0.0 && r.rtk < num_mobs {
        r.rtk = num_mobs;
    }
    if r.rtk > 0.0 && r.rtk < 1.0 {
        r.rtk = 1.0;
    }

    if mob_health > 1 && (r.rtk < 1.0 || r.rtk > 200.0) {
        r.rtk_text = "<infinitely attacking>".to_string();
    } else if r.rtk > 0.0 {
        r.rtk_text = format!(
            "{}{}",
            vb::cstr(vb::round(r.rtk, 1)?),
            if num_mobs > 1.0 { " RTC" } else { " RTK" }
        );
    }

    #[allow(clippy::float_cmp)]
    if r.rtd > 0.0 && r.rtd < 200.0 {
        r.rtd_text = format!("vs {} RTD", vb::cstr(r.rtd));
    } else if (r.rtd == 0.0 || r.rtd >= 200.0) && a.mob_damage >= 0 && a.char_health > 0 {
        r.rtd_text = "vs <unfazed by damage>".to_string();
    }

    if !(r.rtk_text.is_empty() && r.rtd_text.is_empty()) && r.rtd > 0.0 && r.rtk >= 1.0 {
        let rtd2 = vb::pow(r.rtd, 2.0)?;
        let rtk2 = vb::pow(r.rtk, 2.0)?;
        r.success = vb::cint(vb::round(rtd2 / (rtk2 + rtd2) * 100.0, 0)?)?;

        if r.success >= 95 {
            r.success_text = " - certain success".to_string();
        } else if r.success >= 5 {
            r.success_text = format!(" - {}% chance of success", r.success);
        } else {
            r.success_text = " - certain failure".to_string();
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// IsMobKillable, CalcAverageNonZero

/// Port of modMain IsMobKillable: could the character kill the monster before it dies, with a 25%
/// margin either way and the monster's HP regeneration. A VB6 error (Long overflow of the
/// regenerated or total HP) gives `false`, as MME's handler does.
#[must_use]
pub fn is_mob_killable(
    profile: Profile,
    char_dmg: f64,
    char_hp: i32,
    mob_dmg: f64,
    mob_hp: i32,
    char_hp_regen: i16,
    mob_hp_regen: i32,
) -> bool {
    let (Profile::Stock | Profile::Quicksilver(_)) = profile;
    killable_steps(
        char_dmg,
        char_hp,
        mob_dmg,
        mob_hp,
        char_hp_regen,
        mob_hp_regen,
    )
    .unwrap_or(false)
}

fn killable_steps(
    char_dmg: f64,
    char_hp: i32,
    mob_dmg: f64,
    mob_hp: i32,
    char_hp_regen: i16,
    mob_hp_regen: i32,
) -> Result<bool, Vb6Error> {
    if char_dmg <= 0.0 && mob_hp > 0 {
        return Ok(false);
    }
    if mob_hp < 1 {
        return Ok(true);
    }
    let regen_rounds = STOCK_MOB_HPREGEN_ROUNDS;

    let factor = 0.25;
    let char_dmg = char_dmg * (factor + 1.0);
    let mut rounds_to_kill = vb::div(f64::from(mob_hp), char_dmg)?;
    if rounds_to_kill < 1.0 {
        rounds_to_kill = 1.0;
    }

    let mut regen_per_round = 0.0;
    if rounds_to_kill > 1.0 {
        regen_per_round = f64::from(mob_hp_regen) / regen_rounds;
        if rounds_to_kill < regen_rounds {
            // reducing by percentage change to reach the hp tick at 90 seconds
            regen_per_round *= rounds_to_kill / regen_rounds;
        }
    }

    if regen_per_round > 0.0 {
        let eff_dmg = char_dmg - regen_per_round;
        if eff_dmg <= 0.0 {
            return Ok(false);
        }
        rounds_to_kill = vb::div(f64::from(mob_hp), eff_dmg)?;
        if rounds_to_kill < 1.0 {
            rounds_to_kill = 1.0;
        }
        // The total is never read afterwards, but the Long assignment can still overflow.
        vb::clng(f64::from(mob_hp) + regen_per_round * rounds_to_kill)?;
    }

    if rounds_to_kill > 720.0 {
        // would take over an hour to kill
        return Ok(false);
    }

    let mob_dmg = mob_dmg * (1.0 - factor);
    if mob_dmg <= 0.0 {
        return Ok(true);
    }

    let char_total_hp = if char_hp_regen > 0 {
        vb::clng(
            f64::from(char_hp)
                + (f64::from(char_hp) / mob_dmg) * (f64::from(char_hp_regen) / 3.0 / 6.0),
        )?
    } else {
        char_hp
    };
    let rounds_to_death = f64::from(char_total_hp) / mob_dmg;

    Ok(rounds_to_death >= rounds_to_kill)
}

/// Port of modMain CalcAverageNonZero: the mean of the non-zero entries, 0 when there are none.
#[must_use]
pub fn calc_average_non_zero(data: &[f64]) -> f64 {
    let mut sum = 0.0;
    let mut cnt: i32 = 0;
    for &d in data {
        #[allow(clippy::float_cmp)]
        if d != 0.0 {
            sum += d;
            cnt += 1;
        }
    }
    if cnt > 0 { sum / f64::from(cnt) } else { 0.0 }
}

// ---------------------------------------------------------------------------------------------
// GetDamageOutput

/// `eDefenseFlags`: the monster is undead (ability 23 or the Undead column).
pub const DF_UNDEAD: i32 = 0x1;
/// The monster is an animal (ability 78).
pub const DF_ANIMAL: i32 = 0x2;
/// The monster is living (it has no ability 109).
pub const DF_LIVING: i32 = 0x4;
/// The monster is anti-magic (ability 51).
pub const DF_ANTI_MAGIC: i32 = 0x8;

/// `eAttackRestrictions`: who a spell may hit. 0 asks `GetDamageOutput` to read the spell.
pub const AR_UNKNOWN: i32 = 0x0;
pub const AR_NONE: i32 = 0x1;
pub const AR_UNDEAD: i32 = 0x2;
pub const AR_ANIMAL: i32 = 0x4;
pub const AR_LIVING: i32 = 0x8;

/// MME's `tDamageOutput`. A figure of -9990 or less is not damage: -9999 means none could be
/// worked out and -9998 means the monster is immune.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DamageOutput {
    pub average_damage: Currency,
    pub first_round_damage: Currency,
    pub min_round_damage: Currency,
    pub surprise_damage: Currency,
    pub surprise_min_damage: Currency,
    pub surprise_damage_chance: i16,
    pub swings: f64,
}

fn cur_long(v: i32) -> Currency {
    Currency::from_scaled(i64::from(v) * 10_000)
}

impl DamageOutput {
    /// What `GetDamageOutput` assigns before doing any work, and so what it returns after an error
    /// or when the monster row is missing.
    #[must_use]
    pub fn start() -> Self {
        Self {
            average_damage: cur_long(-9999),
            first_round_damage: cur_long(-9999),
            min_round_damage: cur_long(0),
            surprise_damage: cur_long(-9999),
            surprise_min_damage: cur_long(0),
            surprise_damage_chance: 0,
            swings: 0.0,
        }
    }
}

/// Everything `GetDamageOutput` reads besides its arguments.
pub struct DamageEnv<'a> {
    /// Tables, stats and the lair-mode controls (`optMonsterFilter(1)`, `txtMonsterLairFilter`).
    pub ctx: &'a Context<'a>,
    /// The attack choice (`nGlobalAttack*`) and the global filter check box.
    pub character: &'a Character,
    pub races: &'a [Race],
    pub monsters: &'a [Monster],
    /// The `nGlobalCharWeapon*` arrays and accuracy globals `calculate_attack` reads.
    pub equipped: &'a EquippedGlobals,
    pub cast_text: &'a dyn CastText,
    /// `frmMain.lblDatVer.Caption`.
    pub dat_ver_caption: &'a str,
    /// `frmMain.lblInvenCharStat(10).Tag`.
    pub accuracy_tag: &'a str,
    /// `txtMonsterDamageOUT(0).Text` (physical) and `(1).Text` (spell): the party's damage.
    pub damage_out_text: [&'a str; 2],
    /// `txtMonsterLairFilter(9).Text`: the party's swings per round (`PartyFilter` holds only 0 to 8).
    pub party_swings_text: &'a str,
}

/// The arguments of `GetDamageOutput`. `Default` is MME's defaults (the speed adjustment argument
/// is overwritten with 100 by MME, so there is none).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageRequest {
    /// `nSingleMonster`: a Monsters row number, or below 1 to use the values that follow.
    pub single_monster: i32,
    pub vs_ac: i32,
    pub vs_dr: i32,
    pub vs_mr: i32,
    /// Default -1; below 0 means 0.
    pub vs_dodge: i32,
    /// `ePassedDefenseFlags`: `DF_*` bits.
    pub defense_flags: i32,
    pub spell_immunity_lvl: i16,
    pub vs_magic_lvl: i16,
    /// `eAttackFlags`: `AR_*` bits.
    pub attack_flags: i32,
    pub vs_bs_defense: i16,
    pub vs_cold: i16,
    pub vs_fire: i16,
    pub vs_stone: i16,
    pub vs_lightning: i16,
    pub vs_water: i16,
    /// `bForceCharacter`: ignore the lair-mode party and build the profile from the character.
    pub force_character: bool,
}

impl Default for DamageRequest {
    fn default() -> Self {
        Self {
            single_monster: 0,
            vs_ac: 0,
            vs_dr: 0,
            vs_mr: 0,
            vs_dodge: -1,
            defense_flags: 0,
            spell_immunity_lvl: 0,
            vs_magic_lvl: 0,
            attack_flags: AR_UNKNOWN,
            vs_bs_defense: 0,
            vs_cold: 0,
            vs_fire: 0,
            vs_stone: 0,
            vs_lightning: 0,
            vs_water: 0,
            force_character: false,
        }
    }
}

/// A VB6 error, or the early exit for a missing monster: both end at the start value.
struct Abort;

impl From<Vb6Error> for Abort {
    fn from(_: Vb6Error) -> Self {
        Abort
    }
}

type R<T> = Result<T, Abort>;

fn to_i16(v: i32) -> R<i16> {
    i16::try_from(v).map_err(|_| Abort)
}

/// The slice of `tCharacterProfile` that `calculate_attack` reads.
#[must_use]
pub fn attack_profile(t: &crate::character::CharacterProfile) -> AttackProfile {
    AttackProfile {
        is_loaded_character: t.is_loaded_character,
        level: t.level,
        class: t.class,
        race: t.race,
        combat: t.combat,
        strength: t.str_,
        agility: t.agi,
        intellect: t.int,
        charm: t.cha,
        crit: t.crit,
        plus_max_damage: t.plus_max_damage,
        plus_min_damage: t.plus_min_damage,
        plus_bs_accuracy: t.plus_bs_accy,
        plus_bs_min_damage: t.plus_bs_min_dmg,
        plus_bs_max_damage: t.plus_bs_max_dmg,
        ma_plus_skill: t.ma_plus_skill,
        ma_plus_accuracy: t.ma_plus_accy,
        ma_plus_damage: t.ma_plus_dmg,
        stealth: t.stealth,
        class_stealth: t.class_stealth,
        race_stealth: t.race_stealth,
        accuracy: t.accuracy,
        encum_pct: t.encum_pct,
        encum_current: t.encum_current,
        encum_max: t.encum_max,
        spell_dmg_bonus: t.spell_dmg_bonus,
        energy_pct: t.energy_pct,
    }
}

/// Port of modMain GetDamageOutput: the damage per round of the character's current attack choice
/// against one monster (or against the values in `req`), its first-round and minimum-round damage,
/// the backstab (surprise) opener and the swings or casts per round.
///
/// Without the cache MME keeps, every call computes. A missing Monsters row, or a VB6 error,
/// gives [`DamageOutput::start`].
#[must_use]
pub fn get_damage_output(env: &DamageEnv, req: &DamageRequest) -> DamageOutput {
    let (Profile::Stock | Profile::Quicksilver(_)) = env.ctx.profile;
    damage_steps(env, req).unwrap_or_else(|Abort| DamageOutput::start())
}

/// Calls `calculate_attack` for the profile `t` with the arguments `GetDamageOutput` always uses.
pub(crate) fn attack_env<'a>(env: &'a DamageEnv) -> AttackEnv<'a> {
    AttackEnv {
        items: env.ctx.items,
        classes: env.ctx.classes,
        races: env.races,
        equipped: env.equipped,
        cast_text: env.cast_text,
        dat_ver_caption: env.dat_ver_caption,
        avg_level_max_all_stats: env.ctx.avg_level_max_all_stats,
        accuracy_tag: env.accuracy_tag,
    }
}

pub(crate) fn attack_with(
    env: &DamageEnv,
    t: &crate::character::CharacterProfile,
    ar: &AttackRequest,
) -> attack::AttackDamage {
    let mut casts = String::new();
    calculate_attack(
        env.ctx.profile,
        &attack_env(env),
        &attack_profile(t),
        ar,
        &mut casts,
    )
}

/// What a weave needs to know of the monster a blow lands on, from the request's own values.
///
/// A target is "living" only when the request says so, the same test MME's spell branch makes for
/// an Affects Living spell, so a request with no defence flags is a non-living one.
pub fn weave_target(flags: i32, mr: i32, spell_immunity: i16, elements: [i16; 5]) -> WeaveTarget {
    let [cold, fire, stone, lightning, water] = elements.map(i32::from);
    WeaveTarget {
        mr,
        anti_magic: (flags & DF_ANTI_MAGIC) != 0,
        cold,
        fire,
        stone,
        lightning,
        water,
        poison: 0,
        spell_immunity: i32::from(spell_immunity),
        undead: (flags & DF_UNDEAD) != 0,
        animal: (flags & DF_ANIMAL) != 0,
        non_living: (flags & DF_LIVING) == 0,
    }
}

/// The weave of a Quicksilver hybrid (R6) in the damage of a normal blow: adds the expected damage of
/// the proc to `hit`'s round and first-round figures and says so in its detail text. `blows` is the
/// whole blows the first-round figure counts. `hit` is a non-stab blow of the character `t` against
/// `target`; the caller keeps manual damage out.
///
/// Steady state, as the opportunist is: every landed blow that deals damage is assumed to leave the
/// target alive (Q-014 owns the killing blow). Nothing changes, not even the text, under Stock, the
/// neutral balance (chance 0), for a class that does not weave or when no spell qualifies.
///
/// Returns the proc chance in percent and the mean proc damage, when there is one.
///
/// # Errors
/// The VB6 overflow of the sum.
pub fn add_weave(
    env: &DamageEnv,
    t: &crate::character::CharacterProfile,
    target: &WeaveTarget,
    blows: f64,
    hit: &mut attack::AttackDamage,
) -> Result<Option<(u32, i32)>, Vb6Error> {
    let ctx = env.ctx;
    let Some(class) = ctx.classes.iter().find(|c| c.number == t.class) else {
        return Ok(None);
    };
    let Some(weaver) = weave::weaver(ctx.profile, class) else {
        return Ok(None);
    };
    let usable = UsableEnv {
        profile: ctx.profile,
        spells: ctx.spells,
        classes: ctx.classes,
        nmr_ver: ctx.nmr_ver,
        only_in_game: false,
        disable_kai_autolearn: false,
    };
    let per_hit = weave::expected_per_hit(&usable, class, t.align, &t.spell_caster(), target);
    if per_hit <= 0.0 || hit.swings <= 0.0 {
        return Ok(None);
    }
    let landed = hit.landed_per_swing;
    hit.round_total = vb::add_i32(
        hit.round_total,
        vb::clng(vb::round(per_hit * hit.swings * landed, 0)?)?,
    )?;
    hit.first_round_damage = vb::add_i32(
        hit.first_round_damage,
        vb::clng(vb::round(per_hit * blows * landed, 0)?)?,
    )?;
    let mean = vb::clng(vb::round(
        per_hit * 100.0 / f64::from(weaver.chance_pct),
        0,
    )?)?;
    if !hit.attack_detail.is_empty() {
        let text = format!("Weave: {}% for {mean}", weaver.chance_pct);
        hit.attack_detail = crate::textblock::auto_append(&hit.attack_detail, &text, ", ");
    }
    Ok(Some((weaver.chance_pct, mean)))
}

/// The opening round of a stab, as `GetDamageOutput` and the monster pane report it.
pub(crate) struct SurpriseRound {
    /// `nSurpriseDamage`: the stab's average hit plus its weapon's cast damage; under Quicksilver
    /// (R2) also the normal blows the rest of the round pays for.
    pub damage: i32,
    /// `nSurpriseMinDamage`, with the same addition.
    pub min_damage: i32,
    pub hit_chance: i16,
    /// The stab's detail line, naming the follow-up blows under Quicksilver.
    pub detail: String,
}

/// The stab (attack type 4) of `ar` and, under Quicksilver, the blows that follow it in the same
/// round (R2). MME's surprise figure is the stab alone, because in stock the stab ended the round.
pub(crate) fn surprise_round(
    env: &DamageEnv,
    t: &crate::character::CharacterProfile,
    ar: &AttackRequest,
    target: &WeaveTarget,
) -> Result<SurpriseRound, Vb6Error> {
    let bs = attack_with(env, t, ar);
    let mut damage = vb::add_i32(bs.avg_hit, bs.avg_extra_hit)?;
    let mut min_damage = vb::add_i32(bs.min_dmg, bs.avg_extra_swing)?;
    let mut detail = bs.attack_detail;
    if let Some(mut open) =
        attack::calc_stab_opener(env.ctx.profile, &attack_env(env), &attack_profile(t), ar)
    {
        // The blows after the stab are normal ones, so a hybrid's weave fires on them (R6).
        let mut weave_text = String::new();
        if matches!(env.ctx.profile, Profile::Quicksilver(_)) && open.follow_blows > 0 {
            let mut normal = attack_with(
                env,
                t,
                &AttackRequest {
                    attack_type: 5,
                    ..*ar
                },
            );
            let before = normal.first_round_damage;
            if let Some((pct, mean)) =
                add_weave(env, t, target, f64::from(open.follow_blows), &mut normal)?
            {
                open.follow_damage =
                    vb::add_i32(open.follow_damage, normal.first_round_damage - before)?;
                weave_text = format!(" incl. Weave: {pct}% for {mean}");
            }
        }
        damage = vb::add_i32(damage, open.follow_damage)?;
        min_damage = vb::add_i32(min_damage, open.follow_min_damage)?;
        if !detail.is_empty() {
            if !detail.ends_with(' ') {
                detail.push(' ');
            }
            detail.push_str(&format!("+ {}{weave_text}", open.text()));
        }
    }
    Ok(SurpriseRound {
        damage,
        min_damage,
        hit_chance: bs.hit_chance,
        detail,
    })
}

pub(crate) fn populate(
    env: &DamageEnv,
    t: &mut crate::character::CharacterProfile,
    force_use_char: bool,
    force_no_party: bool,
    mud: AttackTypeMud,
    weapon: i32,
) {
    // `Populated::lair_hp_reset` is a control write; there is no control here.
    let _ = populate_character_profile(
        env.ctx,
        env.character,
        t,
        force_use_char,
        force_no_party,
        mud,
        weapon,
        false,
    );
}

pub(crate) fn item_ability(env: &DamageEnv, item: i32, ability: i16) -> i32 {
    let slots = env
        .ctx
        .items
        .iter()
        .find(|i| i.number == item)
        .map(|i| &i.abilities[..]);
    item_has_ability(env.ctx.profile, slots, item, ability)
}

#[allow(clippy::too_many_lines, clippy::cognitive_complexity)] // one MME procedure, in order
fn damage_steps(env: &DamageEnv, req: &DamageRequest) -> R<DamageOutput> {
    let ctx = env.ctx;
    let a = &env.character.attack;
    let st = ctx.stats;

    let mut average = cur_long(-9999);
    let mut first_round = cur_long(-9999);
    let mut min_round = cur_long(0);
    let mut surprise = cur_long(-9999);
    let mut surprise_min = cur_long(0);
    let mut surprise_chance: i16 = 0;
    let mut swings_out = 0.0_f64;

    let mut vs_ac = req.vs_ac;
    let mut vs_dr = req.vs_dr;
    let mut vs_mr = req.vs_mr;
    let mut vs_dodge = req.vs_dodge;
    let mut df = req.defense_flags;
    let mut immunity_lvl = req.spell_immunity_lvl;
    let mut vs_magic_lvl = req.vs_magic_lvl;
    let mut attack_flags = req.attack_flags;
    let mut vs_bs_defense = req.vs_bs_defense;
    let (mut rcol, mut rfir, mut rsto, mut rlit, mut rwat) = (
        req.vs_cold,
        req.vs_fire,
        req.vs_stone,
        req.vs_lightning,
        req.vs_water,
    );

    let speed_adj: i16 = 100;
    let mut accy: i32 = -1;
    let mut party: i16 = 1;
    if ctx.party.lair_mode {
        party = vb::cint(vb::val(&ctx.party.lair_text[0]))?;
    }
    party = party.clamp(1, 6);
    if req.force_character {
        party = 1;
    }

    let mut dmg_physical = 0.0_f64;
    let mut dmg_spell = 0.0_f64;
    let mut party_swings = 0.0_f64;
    let mut attack_swings = 0.0_f64;
    let mut attack_first_round: i32 = 0;
    let mut spell_min_cast: i32 = 0;

    // `done:` is the end of this block; `break 'done` is `GoTo done`.
    'done: {
        if party > 1 {
            dmg_physical = vb::val(env.damage_out_text[0]);
            dmg_spell = vb::val(env.damage_out_text[1]);
            accy = vb::clng(vb::val(&ctx.party.lair_text[8]))?;
            party_swings = vb::val(env.party_swings_text);
            party_swings = party_swings.clamp(1.0, 6.0);
        } else if a.attack_type == AttackType::OneShot {
            average = cur_long(9_999_999);
            min_round = cur_long(9_999_999);
            first_round = average;
            swings_out = 1.0;
            break 'done;
        } else if a.attack_type == AttackType::Manual {
            dmg_physical = f64::from(a.manual_p);
            dmg_spell = f64::from(a.manual_m);
            accy = if env.character.global_filter {
                vb::clng(vb::val(env.accuracy_tag))?
            } else {
                9999
            };
        }

        if req.single_monster >= 1 {
            // (the per-monster cache read is not ported)
            let m = env
                .monsters
                .iter()
                .find(|m| m.number == req.single_monster)
                .ok_or(Abort)?;
            vs_ac = m.armour_class;
            vs_dr = m.damage_resist;
            vs_mr = m.magic_res;
            if ctx.nmr_ver >= 1.83 {
                vs_bs_defense = to_i16(m.bs_defense)?;
            }
            let mut living = true;
            for slot in &m.abilities {
                match slot.ability {
                    3 => rcol = to_i16(slot.value)?,
                    5 => rfir = to_i16(slot.value)?,
                    65 => rsto = to_i16(slot.value)?,
                    66 => rlit = to_i16(slot.value)?,
                    147 => rwat = to_i16(slot.value)?,
                    28 => vs_magic_lvl = to_i16(slot.value)?,
                    34 => vs_dodge = slot.value,
                    51 => df |= DF_ANTI_MAGIC,
                    78 => df |= DF_ANIMAL,
                    109 => living = false,
                    139 => immunity_lvl = to_i16(slot.value)?,
                    _ => {}
                }
            }
            if living {
                df |= DF_LIVING;
            }
            if m.undead == 1 {
                df |= DF_UNDEAD;
            }
        }

        if vs_dodge < 0 {
            vs_dodge = 0;
        }

        let mut t = crate::character::CharacterProfile::default();

        // SURPRISE DAMAGE
        if party == 1 && a.attack_type != AttackType::OneShot && a.backstab {
            let mut weapon = 0;
            if a.backstab_weapon > 0 {
                weapon = a.backstab_weapon;
            } else if a.backstab_weapon == 0 && st.weapon_number[0] > 0 {
                weapon = st.weapon_number[0];
            }

            populate(
                env,
                &mut t,
                req.force_character,
                false,
                AttackTypeMud::Surprise,
                weapon,
            );

            let mut bs_magic: i32 = 0;
            if vs_magic_lvl > 0 {
                if weapon > 0 {
                    bs_magic = item_ability(env, weapon, 28);
                    let hit_magic = item_ability(env, weapon, 142);
                    if hit_magic > -500 {
                        bs_magic = vb::add_i32(bs_magic, hit_magic)?;
                    }
                    bs_magic = vb::add_i32(bs_magic, t.hit_magic_non_weapon)?;
                } else {
                    bs_magic = t.hit_magic_non_weapon;
                }
            }
            if bs_magic < 0 {
                bs_magic = 0;
            }

            if i32::from(vs_magic_lvl) <= bs_magic {
                let bs = surprise_round(
                    env,
                    &t,
                    &AttackRequest {
                        attack_type: 4,
                        weapon_number: weapon,
                        speed_adj,
                        vs_ac,
                        vs_dr,
                        vs_dodge,
                        bs_defense: i32::from(vs_bs_defense),
                        ..AttackRequest::default()
                    },
                    &weave_target(df, vs_mr, immunity_lvl, [rcol, rfir, rsto, rlit, rwat]),
                )?;
                surprise = cur_long(bs.damage);
                surprise_min = cur_long(bs.min_damage);
                surprise_chance = bs.hit_chance;
                if swings_out < 1.0 {
                    swings_out = 1.0;
                }
            } else {
                surprise = cur_long(-9998);
                swings_out = 0.0;
            }
        }

        if party > 1 || a.attack_type == AttackType::Manual {
            average = cur_long(0);
            if dmg_physical > 0.0 {
                populate(
                    env,
                    &mut t,
                    req.force_character,
                    false,
                    AttackTypeMud::Normal,
                    0,
                );
                let ar = if party == 1 {
                    AttackRequest {
                        attack_type: 5,
                        speed_adj,
                        vs_ac,
                        vs_dr,
                        vs_dodge,
                        force_calc: true,
                        specify_damage: dmg_physical,
                        specify_accy: f64::from(accy),
                        ..AttackRequest::default()
                    }
                } else {
                    AttackRequest {
                        attack_type: 5,
                        speed_adj,
                        vs_ac,
                        vs_dr: 0,
                        vs_dodge,
                        force_calc: true,
                        specify_damage: dmg_physical - (f64::from(vs_dr) * party_swings),
                        specify_accy: f64::from(accy),
                        ..AttackRequest::default()
                    }
                };
                let hit = attack_with(env, &t, &ar);
                average = average.checked_add(cur_long(hit.round_total))?;
            }
            if dmg_spell > 0.0 {
                let resisted = calculate_resist_damage(
                    Currency::from_f64(dmg_spell)?,
                    vs_mr,
                    2,
                    true,
                    false,
                    (df & DF_ANTI_MAGIC) != 0,
                    0,
                );
                average = average.checked_add(resisted)?;
            }
            first_round = average;
            min_round = average;
            if swings_out < 1.0 && first_round.checked_add(average)? > cur_long(0) {
                swings_out = 1.0;
            }
            break 'done;
        }

        match a.attack_type {
            AttackType::PhysAttack | AttackType::PhysBash | AttackType::PhysSmash => {
                let mud = if matches!(a.attack_type, AttackType::PhysBash | AttackType::PhysSmash) {
                    if a.attack_type == AttackType::PhysBash {
                        AttackTypeMud::Bash
                    } else {
                        AttackTypeMud::Smash
                    }
                } else {
                    AttackTypeMud::Normal
                };
                populate(
                    env,
                    &mut t,
                    req.force_character,
                    false,
                    mud,
                    st.weapon_number[0],
                );

                let mut weapon_magic = t.hit_magic;
                if weapon_magic < 0 {
                    weapon_magic = 0;
                }

                if i32::from(vs_magic_lvl) <= weapon_magic {
                    if st.weapon_number[0] > 0 {
                        let mut hit = attack_with(
                            env,
                            &t,
                            &AttackRequest {
                                attack_type: mud as i32,
                                weapon_number: st.weapon_number[0],
                                speed_adj,
                                vs_ac,
                                vs_dr,
                                vs_dodge,
                                ..AttackRequest::default()
                            },
                        );
                        let foe =
                            weave_target(df, vs_mr, immunity_lvl, [rcol, rfir, rsto, rlit, rwat]);
                        add_weave(env, &t, &foe, vb::fix(hit.swings), &mut hit)?;
                        average = cur_long(hit.round_total);
                        swings_out = hit.swings;
                        min_round = cur_long(hit.min_round_damage);
                        attack_swings = hit.swings;
                        attack_first_round = hit.first_round_damage;
                    }
                } else {
                    average = cur_long(-9998);
                    swings_out = 0.0;
                }
            }

            AttackType::Spell | AttackType::SpellAny => {
                if a.spell_num > 0 {
                    populate(
                        env,
                        &mut t,
                        req.force_character,
                        true,
                        AttackTypeMud::None,
                        0,
                    );

                    let spell_env = SpellCastEnv {
                        profile: ctx.profile,
                        spells: ctx.spells,
                        text: ctx.text,
                        avg_level_max_all_stats: ctx.avg_level_max_all_stats,
                    };
                    let cast_level = if a.attack_type == AttackType::SpellAny {
                        i32::from(a.spell_lvl)
                    } else {
                        t.level
                    };
                    let cast = spell_env.calculate_spell_cast(
                        &t.spell_caster(),
                        &CastRequest {
                            spell: a.spell_num,
                            cast_level,
                            vs_mr,
                            vs_anti_magic: (df & DF_ANTI_MAGIC) != 0,
                            vs_cold: rcol,
                            vs_fire: rfir,
                            vs_stone: rsto,
                            vs_lightning: rlit,
                            vs_water: rwat,
                        },
                    );
                    spell_min_cast = cast.n_min_cast;

                    let mut valid_target = false;
                    let immu = i32::from(immunity_lvl);
                    if immu == 0
                        || i32::from(cast.n_required_level) >= immu
                        || (cast.n_required_level == 0 && i32::from(cast.n_cast_level) >= immu)
                    {
                        if attack_flags == AR_UNKNOWN {
                            if let Some(sp) = ctx.spells.iter().find(|s| s.number == a.spell_num) {
                                for slot in &sp.abilities {
                                    match slot.ability {
                                        23 => attack_flags |= AR_UNDEAD,
                                        80 => attack_flags |= AR_ANIMAL,
                                        108 => attack_flags |= AR_LIVING,
                                        _ => {}
                                    }
                                }
                                if attack_flags <= AR_NONE {
                                    valid_target = true;
                                }
                            }
                        } else if attack_flags == AR_NONE {
                            valid_target = true;
                        }

                        if !valid_target {
                            if attack_flags > AR_NONE {
                                if (attack_flags & AR_UNDEAD) != 0 {
                                    if (df & DF_UNDEAD) != 0 {
                                        valid_target = true;
                                    }
                                } else if (attack_flags & AR_ANIMAL) != 0 {
                                    if (df & DF_ANIMAL) != 0 {
                                        valid_target = true;
                                    }
                                } else if (attack_flags & AR_LIVING) != 0 && (df & DF_LIVING) != 0 {
                                    valid_target = true;
                                }
                            } else {
                                valid_target = true;
                            }
                        }
                    }

                    if valid_target {
                        average = cur_long(cast.n_avg_round_dmg);
                        min_round = cur_long(cast.n_min_round_dmg);
                        swings_out = cast.n_num_casts;
                    } else {
                        average = cur_long(-9998);
                        swings_out = 0.0;
                    }
                }
            }

            AttackType::MartialArts => {
                // 1 punch, 2 kick, 3 jumpkick
                let ma = i32::from(a.martial_art);
                let mud_code = if ma > 1 { ma } else { 1 };
                populate(
                    env,
                    &mut t,
                    req.force_character,
                    false,
                    AttackTypeMud::from_code(mud_code).unwrap_or(AttackTypeMud::None),
                    0,
                );
                if i32::from(vs_magic_lvl) <= t.hit_magic_non_weapon {
                    let kind = match a.martial_art {
                        2 => 2,
                        3 => 3,
                        _ => 1,
                    };
                    let mut hit = attack_with(
                        env,
                        &t,
                        &AttackRequest {
                            attack_type: kind,
                            speed_adj,
                            vs_ac,
                            vs_dr,
                            vs_dodge,
                            ..AttackRequest::default()
                        },
                    );
                    let foe = weave_target(df, vs_mr, immunity_lvl, [rcol, rfir, rsto, rlit, rwat]);
                    add_weave(env, &t, &foe, vb::fix(hit.swings), &mut hit)?;
                    average = cur_long(hit.round_total);
                    min_round = cur_long(hit.min_round_damage);
                    swings_out = hit.swings;
                    attack_swings = hit.swings;
                    attack_first_round = hit.first_round_damage;
                } else {
                    average = cur_long(-9998);
                    swings_out = 0.0;
                }
            }

            // Not reachable: one-shot and manual leave above.
            AttackType::OneShot | AttackType::Manual => {}
        }

        if average > cur_long(-9990) {
            if attack_swings > 0.0 {
                first_round = cur_long(attack_first_round);
            } else if spell_min_cast > 0 {
                first_round = average;
            }
        }
    }

    Ok(DamageOutput {
        average_damage: average,
        first_round_damage: first_round,
        min_round_damage: min_round,
        surprise_damage: surprise,
        surprise_min_damage: surprise_min,
        surprise_damage_chance: surprise_chance,
        swings: swings_out,
    })
}
