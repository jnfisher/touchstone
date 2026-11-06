//! Spell casting (FAM-09). Card P3-10.
//!
//! What a character gets from casting a spell: cast chance, damage after magic resist, anti-magic
//! and elemental resists, rounds until the mana runs out, mana recovery, and whether a class may
//! use a spell at all. Lookups take the Spells and Classes tables as slices; a failed
//! `tabSpells.Seek` is "no row with that number".
//!
//! Where an MME procedure has an `On Error` handler that resumes at `out:`, the port returns what
//! the procedure had computed when the error hit (partly filled result, or the zero/False value
//! of a function that had not assigned yet), as MME does; no `Result` escapes from those. The
//! two helpers without a handler (`ResistPct_SignedOfBase`, `NegResistPct_ShareOfTotal`) return
//! `Result`, because their error reaches the caller.
//!
//! GreaterMUD branches (`GMUD_GetSpDmgMultiplierFromSC`, the GreaterMUD-only ability cases) are not
//! ported. `CalculateSpellCast` prints ability names through `GetAbilityStats` (P3-07's); the
//! caller supplies them through [`SpellText::ability_stats`].

use touchstone_model::{Class, Magery, Spell};
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Vb6Error};

use crate::Profile;
use crate::accuracy::get_spell_hit_cap;
use crate::quicksilver::Balance;
use crate::spells::{MinMaxRequest, SpellText, get_current_spell_min_max};
use crate::stats::{get_class_magery, get_class_magery_lvl};

type Res<T> = Result<T, Vb6Error>;

/// MME's `ROUND_SECS` and `SPELL_ROUND_SECS`.
const ROUND_SECS: i32 = 5;
const SPELL_ROUND_SECS: i32 = 3;

fn to_i16(v: i32) -> Res<i16> {
    i16::try_from(v).map_err(|_| Vb6Error::Overflow)
}

/// A Double assigned to a Long (or a rounded value): CLng.
fn clng(x: f64) -> Res<i32> {
    vb::clng(x)
}

fn div_half_even(n: i64, d: i64) -> i64 {
    let q = n / d;
    let twice = (n % d).abs() * 2;
    if twice > d.abs() || (twice == d.abs() && q % 2 != 0) {
        q + n.signum() * d.signum()
    } else {
        q
    }
}

fn cur_long(v: i32) -> Currency {
    Currency::from_scaled(i64::from(v) * 10_000)
}

/// A Currency assigned to a Long: CLng, banker's rounding.
fn cur_to_long(c: Currency) -> Res<i32> {
    i32::try_from(div_half_even(c.scaled(), 10_000)).map_err(|_| Vb6Error::Overflow)
}

/// `Fix` of a Currency: truncation toward zero, still a Currency.
fn fix_cur(c: Currency) -> Currency {
    Currency::from_scaled(c.scaled() / 10_000 * 10_000)
}

/// `Round(c)` of a Currency: whole units, half to even.
fn round_cur(c: Currency) -> Currency {
    Currency::from_scaled(div_half_even(c.scaled(), 10_000) * 10_000)
}

/// `CStr` of a Currency: up to four decimals, trailing zeros dropped.
fn cur_str(c: Currency) -> String {
    let s = c.scaled();
    let a = s.unsigned_abs();
    let (whole, frac) = (a / 10_000, a % 10_000);
    let sign = if s < 0 { "-" } else { "" };
    if frac == 0 {
        format!("{sign}{whole}")
    } else {
        let f = format!("{frac:04}");
        format!("{sign}{whole}.{}", f.trim_end_matches('0'))
    }
}

fn find(spells: &[Spell], number: i32) -> Option<&Spell> {
    spells.iter().find(|s| s.number == number)
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
// Resist helpers

/// Port of modMMudFunc CalculateResistDamage: a spell's damage after the target's magic resist.
///
/// `resist_type` is the spell's `TypeOfResists` (0 never, 1 anti-magic only, 2 everyone; MME's
/// default is 2). `bonus_resist` is the matching elemental resist. A VB6 error inside the
/// procedure leaves the result at 0, as MME's handler does.
#[must_use]
pub fn calculate_resist_damage(
    damage: Currency,
    vs_magic_resist: i32,
    resist_type: i16,
    damage_resistable: bool,
    include_total_resist: bool,
    vs_anti_magic: bool,
    bonus_resist: i32,
) -> Currency {
    resist_inner(
        damage,
        vs_magic_resist,
        resist_type,
        damage_resistable,
        include_total_resist,
        vs_anti_magic,
        bonus_resist,
    )
    .unwrap_or(Currency::from_scaled(0))
}

fn resist_inner(
    mut damage: Currency,
    mut mr: i32,
    resist_type: i16,
    resistable: bool,
    include_total: bool,
    anti_magic: bool,
    bonus: i32,
) -> Res<Currency> {
    let mut resist = 0.0f64;
    if mr <= 0 {
        mr = 1;
    }
    if bonus > 0 {
        let scaled = cur_long(vb::sub_i32(100, bonus)?).checked_mul(damage)?;
        damage = fix_cur(scaled.checked_div(cur_long(100))?);
    }
    if resistable {
        if anti_magic {
            resist = vb::fix(f64::from(mr) / 2.0);
            if resist > 75.0 {
                resist = 75.0;
            }
        } else if mr > 51 {
            resist = vb::fix(f64::from(vb::sub_i32(mr, 50)?) / 2.0);
            if resist > 50.0 {
                resist = 50.0;
            }
        }
        if resist > 0.0 {
            damage = Currency::from_f64(damage.to_f64() * (1.0 - resist / 100.0))?;
        } else if !anti_magic && mr < 50 {
            // +damage for mr < 50
            let boost = damage
                .checked_mul(cur_long(vb::sub_i32(50, mr)?))?
                .checked_div(cur_long(100))?;
            damage = damage.checked_add(boost)?;
        }
    }
    if include_total && mr > 1 && (resist_type == 2 || (anti_magic && resist_type == 1)) {
        let mut total = vb::fix(f64::from(mr) / 2.0);
        if total > 98.0 {
            total = 98.0;
        }
        damage = Currency::from_f64(damage.to_f64() * (1.0 - total / 100.0))?;
    }
    Ok(round_cur(damage))
}

/// Port of modMMudFunc ResistPct_SignedOfBase: the percentage of `base` that did not get through
/// (negative when the net came out as healing). 0 within 1 point of the base.
///
/// # Errors
/// Error 6 when the percentage does not fit a Long (MME has no handler here).
pub fn resist_pct_signed_of_base(base: f64, final_net: f64) -> Res<i32> {
    if base <= 0.0 {
        return Ok(0);
    }
    if final_net >= 0.0 {
        if vb::abs(base - final_net) <= 1.0 {
            return Ok(0);
        }
        clng(vb::round((base - final_net) / base * 100.0, 0)?)
    } else {
        Ok(-clng(vb::round((-final_net) / base * 100.0, 0)?)?)
    }
}

/// Port of modMMudFunc NegResistPct_ShareOfTotal: the healed share of the total resisted, as a
/// negative whole percentage; 0 unless the net is below zero.
///
/// # Errors
/// Error 6 if the share does not fit a Long (it cannot for finite inputs).
pub fn neg_resist_pct_share_of_total(base: f64, final_net: f64) -> Res<i32> {
    if base <= 0.0 {
        return Ok(0);
    }
    if final_net < 0.0 {
        let heal = -final_net;
        let total = base + heal;
        Ok(-clng(vb::round(heal / total * 100.0, 0)?)?)
    } else {
        Ok(0)
    }
}

/// Port of modMMudFunc SpellAttackTypeEnum: the attack type's name, or its first letter when
/// `short`; an unknown number comes back as itself.
#[must_use]
pub fn spell_attack_type_enum(attack_type: i16, short: bool) -> String {
    let (long, abbr) = match attack_type {
        0 => ("Cold", "C"),
        1 => ("Fire", "F"),
        2 => ("Stone", "S"),
        3 => ("Lightning", "L"),
        4 => ("Normal", "N"),
        5 => ("Water", "W"),
        6 => ("Poison", "P"),
        n => return n.to_string(),
    };
    (if short { abbr } else { long }).to_string()
}

/// Port of modMMudFunc GetSpellCastChance: the percentage of casts that go off.
///
/// With `difficulty` 0 and a `spell` number, the difficulty (and the Kai flag) come from that
/// spell's row; a missing row gives 0. Spellcasting of 0 or less, or a difficulty of 200 or more
/// (never fails), gives 100. A VB6 error (Integer overflow of the sum) gives 0.
#[must_use]
pub fn get_spell_cast_chance(
    profile: Profile,
    spells: &[Spell],
    difficulty: i16,
    spellcasting: i16,
    kai: bool,
    spell: i32,
) -> i16 {
    cast_chance_inner(profile, spells, difficulty, spellcasting, kai, spell).unwrap_or(0)
}

fn cast_chance_inner(
    profile: Profile,
    spells: &[Spell],
    mut difficulty: i16,
    spellcasting: i16,
    mut kai: bool,
    spell: i32,
) -> Res<i16> {
    if difficulty == 0 && spell == 0 && spellcasting == 0 {
        return Ok(0);
    }
    if !(difficulty != 0 || spell == 0) {
        if spells.is_empty() {
            return Ok(0);
        }
        let Some(s) = find(spells, spell) else {
            return Ok(0);
        };
        difficulty = to_i16(s.diff)?;
        if s.magery == 5 {
            kai = true;
        }
    }
    if spellcasting > 0 && difficulty < 200 {
        let mut chance = vb::add_i16(spellcasting, difficulty)?;
        if chance < 0 {
            chance = 0;
        }
        if kai {
            if chance > 100 {
                chance = 100;
            }
        } else {
            let cap = to_i16(get_spell_hit_cap(profile))?;
            if chance > cap {
                chance = cap;
            }
        }
        Ok(chance)
    } else {
        Ok(100)
    }
}

// ---------------------------------------------------------------------------------------------
// Mana

/// Port of modMain CalcRoundsToOOM: combat rounds until the mana is gone casting a spell every
/// round (an aura spell, `duration` over 1, every time it runs out), or 0 when it never runs out
/// (or would only after 999 rounds). `cast_chance` 0 or less means 100; `duration` is in 3-second
/// spell ticks and 1 for an instant spell. A VB6 error gives 0.
#[must_use]
pub fn calc_rounds_to_oom(
    mana_cost: f64,
    max_mana: i32,
    regen_rate: f64,
    cast_chance: i16,
    duration: i32,
) -> i16 {
    oom_inner(mana_cost, max_mana, regen_rate, cast_chance, duration).unwrap_or(0)
}

fn oom_inner(
    mana_cost: f64,
    max_mana: i32,
    regen_rate: f64,
    mut cast_chance: i16,
    mut duration: i32,
) -> Res<i16> {
    const REGEN_SECS: i32 = 30;
    if mana_cost > f64::from(max_mana) {
        return Ok(0); // never cast
    }
    let rounds_per_regen = REGEN_SECS / ROUND_SECS;
    if duration < 1 {
        duration = 1;
    }
    if cast_chance <= 0 {
        cast_chance = 100;
    }
    let mut return_on_fail = 0;
    if cast_chance < 100 && mana_cost > 0.0 {
        return_on_fail = clng(vb::fix(mana_cost / 2.0))?;
    }
    let mut duration_rounds = 0;
    if duration > 1 {
        // Aura spell: duration counts SPELL_ROUND_SECS ticks.
        let aura_secs = vb::mul_i32(duration, SPELL_ROUND_SECS)?;
        let regen_ticks = aura_secs / REGEN_SECS;
        let regen_between = clng(f64::from(regen_ticks) * regen_rate)?;
        if f64::from(regen_between)
            >= mana_cost + ((mana_cost / 2.0) * (1.0 - (f64::from(cast_chance) / 100.0)))
        {
            return Ok(0); // never oom maintaining this aura
        }
        duration_rounds = vb::add_i32(aura_secs, ROUND_SECS)? - 1;
        duration_rounds /= ROUND_SECS;
        if duration_rounds < 1 {
            duration_rounds = 1;
        }
    } else if regen_rate >= mana_cost * f64::from(rounds_per_regen) {
        return Ok(0); // never oom
    }

    let mut current = f64::from(max_mana);
    let mut rounds: i32 = 0;
    let mut rounds_duration: i16 = 0;
    let mut fail_acc: i32 = 0;
    while current >= mana_cost && rounds < 999 {
        rounds += 1;
        if duration > 1 {
            rounds_duration = vb::add_i16(rounds_duration, 1)?;
        }
        // Instant spells cast every round; auras on the first round and then every
        // `duration_rounds` (a failed cast resets that counter).
        let cast_attempt = duration == 1
            || rounds == 1
            || rounds_duration == 1
            || i32::from(rounds_duration) % duration_rounds == 0;
        if cast_attempt {
            current -= mana_cost;
        }
        if cast_chance < 100 && cast_attempt {
            fail_acc = vb::add_i32(fail_acc, 100 - i32::from(cast_chance))?;
            if f64::from(fail_acc) >= 100.0 - (f64::from(100 - i32::from(cast_chance)) / 2.0) {
                current += f64::from(return_on_fail);
                fail_acc = vb::sub_i32(fail_acc, 100)?;
                if duration > 1 {
                    rounds_duration = 0;
                }
            }
        }
        if rounds % rounds_per_regen == 0 {
            current += regen_rate;
            if current > f64::from(max_mana) {
                current = f64::from(max_mana);
                if rounds > 200 {
                    return Ok(0); // assume it won't run out
                }
            }
        }
    }
    if rounds == 999 {
        rounds = 0;
    }
    to_i16(rounds)
}

/// Port of modMain CalcManaRecoveryRounds: 5-second rounds to go from no mana to `percentage`
/// (MME's default 95) of the maximum while meditating (a 30-second regen tick of `regen_rate` and
/// a 10-second tick of `meditate_rate`), or 999 if that is not reached. A VB6 error gives 0.
#[must_use]
pub fn calc_mana_recovery_rounds(
    max_mana: i32,
    regen_rate: i32,
    meditate_rate: i32,
    percentage: i16,
) -> i32 {
    recovery_inner(max_mana, regen_rate, meditate_rate, percentage).unwrap_or(0)
}

fn recovery_inner(max_mana: i32, regen_rate: i32, meditate_rate: i32, percentage: i16) -> Res<i32> {
    const REGEN_TICK: i32 = 30;
    const MEDI_TICK: i32 = 10;
    let target = if percentage > 0 {
        clng(vb::fix(
            f64::from(max_mana) * (f64::from(percentage) / 100.0),
        ))?
    } else {
        max_mana
    };
    if target <= 0 {
        return Ok(0);
    }
    let (mut mana, mut rounds, mut t) = (0i32, 0i32, 0i32);
    let (mut next_regen, mut next_medi) = (REGEN_TICK, MEDI_TICK);
    while mana < target && rounds < 999 {
        rounds += 1;
        t = vb::add_i32(t, ROUND_SECS)?;
        while t >= next_regen {
            mana = vb::add_i32(mana, regen_rate)?;
            next_regen = vb::add_i32(next_regen, REGEN_TICK)?;
            if mana >= target {
                break;
            }
        }
        if meditate_rate > 0 {
            while t >= next_medi {
                mana = vb::add_i32(mana, meditate_rate)?;
                next_medi = vb::add_i32(next_medi, MEDI_TICK)?;
                if mana >= target {
                    break;
                }
            }
        }
    }
    Ok(if mana < target { 999 } else { rounds })
}

// ---------------------------------------------------------------------------------------------
// Which spells a class can use

/// The tables and globals `SpellSeek`, `SpellIsInGame` and `SpellIsUsable` read.
pub struct UsableEnv<'a> {
    pub profile: Profile,
    pub spells: &'a [Spell],
    pub classes: &'a [Class],
    /// `nNMRVer`: the data version (1.7 and 1.8 gate the class lists).
    pub nmr_ver: f64,
    /// `bOnlyInGame`: hide spells no one can get.
    pub only_in_game: bool,
    /// `bDisableKaiAutolearn`.
    pub disable_kai_autolearn: bool,
}

fn text_len(s: &Option<String>) -> i32 {
    s.as_deref().map_or(0, vb::len)
}

impl UsableEnv<'_> {
    fn seek(&self, number: i32) -> Option<&Spell> {
        // SpellSeek: numbers below 1 and an empty table never match.
        if number < 1 {
            return None;
        }
        find(self.spells, number)
    }

    /// Port of modMMudDatabase SpellSeek: is there a Spells row with this number?
    #[must_use]
    pub fn spell_seek(&self, number: i32) -> bool {
        self.seek(number).is_some()
    }

    fn in_game(&self, number: i32) -> bool {
        let Some(s) = self.seek(number) else {
            return false;
        };
        if !s.learnable
            && text_len(&s.learned_from) <= 1
            && text_len(&s.casted_by) <= 1
            && (s.magery != 5
                || (s.magery == 5 && s.req_level < 1)
                || (s.magery == 5 && self.disable_kai_autolearn))
        {
            if self.nmr_ver >= 1.8 {
                if text_len(&s.classes) <= 1 {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }

    /// Port of modMMudFunc SpellIsInGame: can the spell be obtained at all (learnable, taught,
    /// cast by something, or listed for a class)?
    #[must_use]
    pub fn spell_is_in_game(&self, number: i32) -> bool {
        let (Profile::Stock | Profile::Quicksilver(_)) = self.profile;
        self.in_game(number)
    }

    /// Port of modMMudFunc SpellIsUsable: can this class (at this level and alignment) learn or
    /// cast the spell? `class` below 1 means any class. `level` 0 skips the level test and
    /// `align` 0 the alignment test (1 good, 2 neutral, 3 evil). `and_learnable` also requires the
    /// spell to be learnable.
    #[must_use]
    pub fn spell_is_usable(
        &self,
        number: i32,
        class: i32,
        level: i16,
        align: i16,
        and_learnable: bool,
    ) -> bool {
        let (Profile::Stock | Profile::Quicksilver(_)) = self.profile;
        if number < 1 {
            return false;
        }
        if class < 1 {
            return true;
        }
        let level = level.max(0);
        let align = align.max(0);
        let s = if self.only_in_game {
            if !self.in_game(number) {
                return false;
            }
            self.seek(number)
        } else {
            self.seek(number)
        };
        let Some(s) = s else {
            return false;
        };
        if and_learnable
            && !s.learnable
            && text_len(&s.learned_from) < 5
            && (s.magery != 5 || (s.magery == 5 && (self.disable_kai_autolearn || s.req_level < 1)))
        {
            return false; // not learnable
        }

        let class_row = self.classes.iter().find(|c| c.number == class);
        let magery = get_class_magery(class_row);
        let magery_lvl = get_class_magery_lvl(class_row);
        // The Magery enum's variants are numbered 0..=5 in the database's order.
        let kai = magery == touchstone_model::Magery::Kai;
        let class_text = s.classes.as_deref().unwrap_or("");
        let class_tag = format!("({class})");
        let listed = || class_text.to_lowercase().contains(&class_tag);

        if s.magery != 0 {
            if magery != touchstone_model::Magery::None {
                if magery as i32 != s.magery {
                    // The Magery = 0 test is dead code behind the guard above; kept as written.
                    if s.learnable && s.magery == 0 && self.nmr_ver >= 1.7 {
                        if class_text == "(*)" || listed() {
                            // skip the magery check
                        } else {
                            return false;
                        }
                    } else {
                        return false;
                    }
                } else if self.magery_gate(s, magery_lvl, kai) {
                    return false;
                }
            } else {
                return false;
            }
        }

        if self.nmr_ver >= 1.7
            && class > 0
            && vb::len(class_text) > 2
            && class_text != "(*)"
            && !listed()
        {
            return false;
        }
        if level > 0 && i32::from(level) < s.req_level {
            return false;
        }
        if align > 0 {
            for slot in &s.abilities {
                match slot.ability {
                    97 | 98 | 112 => {
                        let is_align = slot.ability;
                        match align {
                            1 if is_align != 97 => return false,
                            2 if is_align != 112 => return false,
                            3 if is_align != 98 => return false,
                            _ => {}
                        }
                    }
                    110 | 111 | 113 => {
                        let not_align = slot.ability;
                        match align {
                            1 if not_align == 110 => return false,
                            2 if not_align == 113 => return false,
                            3 if not_align == 111 => return false,
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        true
    }

    /// The checks after the magery type matched: class magery level, and spells that are neither
    /// learnable nor (for Kai, with autolearn off) automatic.
    fn magery_gate(&self, s: &Spell, magery_lvl: i32, kai: bool) -> bool {
        (magery_lvl > 0 && magery_lvl < s.magery_lvl)
            || (!kai && !s.learnable)
            || (kai && self.disable_kai_autolearn && !s.learnable)
    }
}

// ---------------------------------------------------------------------------------------------
// CalculateSpellCast

/// The fields of MME's `tCharacterProfile` that `CalculateSpellCast` reads. (P3-11 maps the full
/// profile onto this.)
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SpellCasterProfile {
    /// `nSpellDmgBonus`: percent added to spell damage.
    pub spell_dmg_bonus: i16,
    /// `nSpellcasting`.
    pub spellcasting: i16,
    /// `nMaxMana`.
    pub max_mana: f64,
    /// `nSpellOverhead`: extra mana per round on top of the spell's cost.
    pub spell_overhead: f64,
    /// `nManaRegen`.
    pub mana_regen: f64,
    /// The character's casting stat ([`casting_stat`]): Quicksilver's R7 reads it. 0 for no
    /// character and for classes without mana.
    pub casting_stat: i32,
    /// The character's class number, for Quicksilver's class spell share (R8); 0 is no class.
    pub class: i32,
    /// The character's level (not the cast level), for the class share.
    pub level: i32,
}

/// The stat that drives a class's mana, which Quicksilver's R7 scales spell damage by: Mage
/// Intellect, Priest Willpower, Druid their mean (truncated), Bard Charm; 0 for Kai and no magic.
/// It is the stat `CalcManaRegen` multiplies, so the same stats are passed.
#[must_use]
pub fn casting_stat(magery: Magery, intellect: i32, willpower: i32, charm: i32) -> i32 {
    match magery {
        Magery::Mage => intellect,
        Magery::Priest => willpower,
        Magery::Druid => ((i64::from(intellect) + i64::from(willpower)) / 2) as i32,
        Magery::Bard => charm,
        Magery::None | Magery::Kai => 0,
    }
}

/// Quicksilver R7 and R8: a player's damage spell amount after the casting-stat bonus (per mille
/// added per stat point over 50, the stat counting a third above `spell_stat_knee`) and then the
/// class's share at the character's level, both truncating in 64 bits. Stock leaves the amount
/// alone, and so does the neutral balance.
///
/// A class number outside the stock fifteen has no share and keeps the whole amount.
#[must_use]
pub fn scale_spell_damage(profile: Profile, amount: i32, who: &SpellCasterProfile) -> i32 {
    let Profile::Quicksilver(b) = profile else {
        return amount;
    };
    let stat = Balance::knee_third(i64::from(who.casting_stat), b.spell_stat_knee);
    let bonus = (stat - 50).max(0) * i64::from(b.spell_stat_permille);
    let boosted = (i64::from(amount) * (1000 + bonus) / 1000)
        .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    let class = u16::try_from(who.class).unwrap_or(0);
    let level = u16::try_from(who.level.max(0)).unwrap_or(u16::MAX);
    b.class_spell_pct.scale(class, level, boosted)
}

/// A monster's chance to resist an area spell and the cut it then takes (Quicksilver R9), both
/// whole percents of the engine: the chance is `min(MR / 2, 98)` taken against the 99 values the
/// host's `genrdn(1, 100)` returns, the cut `min(aoe_resist_cut_max_pct, MR / aoe_resist_cut_div)`.
fn area_resist(b: &Balance, mr: i32) -> (i32, i32) {
    if mr <= 0 {
        return (0, 0);
    }
    let chance = (mr / 2).min(98);
    let div = i32::try_from(b.aoe_resist_cut_div.max(1)).unwrap_or(i32::MAX);
    let max = i32::try_from(b.aoe_resist_cut_max_pct).unwrap_or(i32::MAX);
    (chance, (mr / div).clamp(0, max))
}

/// Quicksilver R9 for tests and callers that show it: the chance, in rolls out of the engine's 99,
/// that a monster of magic resistance `mr` resists an area spell, and the cut it then takes in
/// percent. `(0, 0)` in stock.
#[must_use]
pub fn area_resist_chance(profile: Profile, mr: i32) -> (i32, i32) {
    match profile {
        Profile::Stock => (0, 0),
        Profile::Quicksilver(b) => area_resist(b, mr),
    }
}

/// The text for R9 when it takes anything off: the chance a monster's roll resists, as a whole
/// percent of the 99 rolls, and the cut it then takes.
fn area_resist_note(profile: Profile, mr: i32) -> Option<String> {
    let Profile::Quicksilver(b) = profile else {
        return None;
    };
    let (chance, cut) = area_resist(b, mr);
    (chance > 0 && cut > 0).then(|| {
        let pct = (f64::from(chance) * 100.0 / 99.0).round();
        format!("{pct}% chance for a monster to resist and take {cut}% less")
    })
}

/// Quicksilver R9: the share of an area damage spell a monster of magic resistance `mr` is
/// expected to keep, `1 - p * cut / 100`, where `p` is the chance its resist roll succeeds. 1 in
/// stock (a Never-resist area spell is never resisted there) and for a monster with no resistance.
#[must_use]
pub fn area_resist_keep(profile: Profile, mr: i32) -> f64 {
    let Profile::Quicksilver(b) = profile else {
        return 1.0;
    };
    let (chance, cut) = area_resist(b, mr);
    1.0 - f64::from(chance) * f64::from(cut) / 9900.0
}

/// The arguments of `CalculateSpellCast` after the profile. `cast_level` 0 or less means the
/// spell's cap; the resists are the target's (`vs_*` percentages by element).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CastRequest {
    pub spell: i32,
    pub cast_level: i32,
    pub vs_mr: i32,
    pub vs_anti_magic: bool,
    pub vs_cold: i16,
    pub vs_fire: i16,
    pub vs_stone: i16,
    pub vs_lightning: i16,
    pub vs_water: i16,
}

/// MME's `tSpellCastValues`, with the VB6 field types behind the integer widths.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpellCastValues {
    pub n_min_cast: i32,
    pub n_max_cast: i32,
    pub n_avg_cast: i32,
    pub n_num_casts: f64,
    pub n_cast_chance: i16,
    pub n_avg_round_dmg: i32,
    pub n_min_round_dmg: i32,
    pub n_avg_round_heals: i32,
    pub n_duration: i16,
    /// Percent of the damage that was resisted (negative for a net heal).
    pub n_damage_resisted: i32,
    pub n_full_resist_chance: i16,
    pub n_mana_cost: i16,
    pub n_oom: i16,
    pub b_does_heal: bool,
    pub b_does_damage: bool,
    pub s_avg_round: String,
    pub s_lvl_increases: String,
    pub s_mma: String,
    pub s_spell_name: String,
    pub n_cast_level: i16,
    pub n_spell_attack_type: i16,
    pub n_required_level: i16,
}

/// What `CalculateSpellCast` reads besides its arguments.
pub struct SpellCastEnv<'a> {
    pub profile: Profile,
    pub spells: &'a [Spell],
    /// Supplies `GetAbilityStats` (only `ability_stats` is called).
    pub text: &'a dyn SpellText,
    /// `gAvgLevelMaxAllStats`, which `GetCurrentSpellMinMax` reads when no level is given.
    pub avg_level_max_all_stats: f64,
}

/// Abilities whose names the "LVL Increases" text leaves out.
fn ability_hidden_from_increases(a: i32) -> bool {
    matches!(
        a,
        23 | 51 | 52 | 80 | 97 | 98 | 100 | 108
            ..=113 | 115 | 119 | 122 | 138 | 144 | 151 | 164 | 178 | 137
    )
}

impl SpellCastEnv<'_> {
    /// Port of modMMudFunc CalculateSpellCast: damage, healing, cast chance, mana and the
    /// description strings for casting a spell at a level against a target's resists.
    ///
    /// No such spell gives an all-zero result. On a VB6 error the partly filled result is
    /// returned, as MME's handler does.
    #[must_use]
    pub fn calculate_spell_cast(
        &self,
        who: &SpellCasterProfile,
        req: &CastRequest,
    ) -> SpellCastValues {
        let mut r = SpellCastValues::default();
        let _ = self.cast_inner(who, req, &mut r);
        r
    }

    #[allow(clippy::too_many_lines)] // one MME procedure, kept in its order
    fn cast_inner(
        &self,
        who: &SpellCasterProfile,
        req: &CastRequest,
        r: &mut SpellCastValues,
    ) -> Res<()> {
        let (Profile::Stock | Profile::Quicksilver(_)) = self.profile;
        if req.spell == 0 {
            return Ok(());
        }
        let Some(sp) = find(self.spells, req.spell) else {
            return Ok(());
        };
        r.s_spell_name = sp.name.clone().unwrap_or_default();
        r.n_spell_attack_type = to_i16(sp.att_type)?;
        let elemental = i32::from(match sp.att_type {
            0 => req.vs_cold,
            1 => req.vs_fire,
            2 => req.vs_stone,
            3 => req.vs_lightning,
            5 => req.vs_water,
            _ => 0,
        });

        let mut cast_lvl = req.cast_level;
        let mut lvl_specified = false;
        if cast_lvl <= 0 {
            if cast_lvl < sp.cap {
                cast_lvl = sp.cap;
            }
        } else {
            lvl_specified = true;
        }
        if cast_lvl < sp.req_level {
            cast_lvl = sp.req_level;
        }
        if cast_lvl > sp.cap && sp.cap > 0 {
            cast_lvl = sp.cap;
        }
        r.n_cast_level = to_i16(cast_lvl)?;
        r.n_required_level = to_i16(sp.req_level)?;

        let increases = (sp.min_inc != 0 && sp.min_inc_lvls > 0)
            || (sp.max_inc != 0 && sp.max_inc_lvls > 0)
            || (sp.dur_inc != 0 && sp.dur_inc_lvls > 0);
        let show_at_level = (sp.cap == 0 || sp.cap > sp.req_level) && increases;

        let mm = get_current_spell_min_max(
            Some(sp),
            self.avg_level_max_all_stats,
            &MinMaxRequest {
                use_level: cast_lvl > 0,
                level: to_i16(cast_lvl)?,
                ..MinMaxRequest::default()
            },
        );
        let mut duration = cur_to_long(mm.n_dur)?;
        if duration < 1 {
            duration = 1;
        }
        let mut min_cast = cur_to_long(mm.n_min)?;
        let mut max_cast = cur_to_long(mm.n_max)?;
        let mut avg_cast = clng(vb::round(
            f64::from(vb::add_i32(min_cast, max_cast)?) / 2.0,
            0,
        )?)?;

        let (multiplier, avg_modified, min_modified);
        if who.spell_dmg_bonus > 0 {
            multiplier = 1.0 + vb::round(f64::from(who.spell_dmg_bonus) / 100.0, 2)?;
            avg_modified = clng(vb::fix(f64::from(avg_cast) * multiplier))?;
            min_modified = clng(vb::fix(f64::from(min_cast) * multiplier))?;
        } else {
            multiplier = 1.0;
            avg_modified = avg_cast;
            min_modified = min_cast;
        }

        let cast_chance: i16 = if who.spellcasting > 0 && sp.diff < 200 {
            get_spell_cast_chance(
                self.profile,
                self.spells,
                to_i16(sp.diff)?,
                who.spellcasting,
                sp.magery == 5,
                0,
            )
        } else {
            100
        };

        let non_magic = sp.abilities.iter().any(|a| a.ability == 144);

        // Quicksilver R7/R8: the amount a damage slot adds is scaled by the caster's casting stat
        // and class. Inflict Damage and Inflict Damage on Armour always scale on a monster; a drain
        // only when it does not last. Stock and the neutral balance leave every amount alone.
        let who_scale = |a: i32| scale_spell_damage(self.profile, a, who);
        let lasting = sp.dur != 0;
        let drain_scale = |a: i32| if lasting { a } else { who_scale(a) };
        let mut roll_scaled = false;
        let mut area_damage = false;

        let (mut damage, mut heals, mut min_damage) = (0i32, 0i32, 0i32);
        let mut damage_minus_mr = false;
        let mut value_modified = false;
        for slot in &sp.abilities {
            let v = slot.value;
            match slot.ability {
                1 => {
                    r.b_does_damage = true;
                    area_damage = true;
                    if v == 0 {
                        roll_scaled = true;
                        damage = vb::add_i32(damage, who_scale(avg_modified))?;
                        min_damage = vb::add_i32(min_damage, who_scale(min_modified))?;
                        if multiplier > 1.0 {
                            value_modified = true;
                        }
                    } else {
                        damage = vb::add_i32(damage, who_scale(v))?;
                        min_damage = vb::add_i32(min_damage, who_scale(v))?;
                    }
                    if elemental != 0 {
                        let t2 = clng(vb::round(
                            f64::from(damage) * (f64::from(elemental) / 100.0),
                            0,
                        )?)?;
                        damage = vb::sub_i32(damage, t2)?;
                        r.n_damage_resisted = vb::add_i32(r.n_damage_resisted, t2)?;
                    }
                }
                8 => {
                    // drain
                    r.b_does_damage = true;
                    r.b_does_heal = true;
                    if v == 0 {
                        roll_scaled |= !lasting;
                        damage = vb::add_i32(damage, drain_scale(avg_modified))?;
                        min_damage = vb::add_i32(min_damage, drain_scale(min_modified))?;
                        heals = vb::add_i32(heals, drain_scale(avg_modified))?;
                    } else {
                        damage = drain_scale(v);
                        min_damage = drain_scale(v);
                        heals = drain_scale(v);
                    }
                }
                17 => {
                    // damage reduced by magic resist
                    r.b_does_damage = true;
                    area_damage = true;
                    let (temp, temp_min);
                    if v == 0 {
                        roll_scaled = true;
                        damage_minus_mr = true;
                        temp = who_scale(avg_modified);
                        temp_min = who_scale(min_modified);
                        if multiplier > 1.0 {
                            value_modified = true;
                        }
                    } else {
                        temp = who_scale(v);
                        temp_min = temp;
                    }
                    let resist = |dmg: i32| -> Res<i32> {
                        cur_to_long(calculate_resist_damage(
                            cur_long(dmg),
                            req.vs_mr,
                            to_i16(sp.type_of_resists)?,
                            true,
                            false,
                            req.vs_anti_magic,
                            0,
                        ))
                    };
                    if req.vs_mr > 0 && !non_magic {
                        let mut t2 = resist(temp)?;
                        r.n_damage_resisted =
                            vb::add_i32(r.n_damage_resisted, vb::sub_i32(temp, t2)?)?;
                        damage = vb::add_i32(damage, t2)?;
                        if v == 0 {
                            t2 = resist(temp_min)?;
                        }
                        min_damage = vb::add_i32(min_damage, t2)?;
                    } else {
                        damage = vb::add_i32(damage, temp)?;
                        min_damage = vb::add_i32(min_damage, temp_min)?;
                    }
                    if elemental != 0 {
                        let mut t2 = clng(vb::round(
                            f64::from(damage) * (f64::from(elemental) / 100.0),
                            0,
                        )?)?;
                        damage = vb::sub_i32(damage, t2)?;
                        r.n_damage_resisted = vb::add_i32(r.n_damage_resisted, t2)?;
                        t2 = clng(vb::round(
                            f64::from(min_damage) * (f64::from(elemental) / 100.0),
                            0,
                        )?)?;
                        min_damage = vb::sub_i32(min_damage, t2)?;
                    }
                }
                18 => {
                    r.b_does_heal = true;
                    if v == 0 {
                        heals = vb::add_i32(heals, avg_modified)?;
                    } else {
                        heals = v;
                    }
                }
                // 150, 174, 175, 176 only act on GreaterMUD.
                _ => {}
            }
        }

        if value_modified {
            min_cast = clng(vb::fix(f64::from(min_cast) * multiplier))?;
            max_cast = clng(vb::fix(f64::from(max_cast) * multiplier))?;
        }
        if roll_scaled {
            min_cast = who_scale(min_cast);
            max_cast = who_scale(max_cast);
        }
        let avg_before_resist = clng(vb::round(
            f64::from(vb::add_i32(min_cast, max_cast)?) / 2.0,
            0,
        )?)?;

        let mut full_resist: i16 = 0;
        if req.vs_mr > 0 {
            if damage_minus_mr && !non_magic {
                let rt = to_i16(sp.type_of_resists)?;
                min_cast = cur_to_long(calculate_resist_damage(
                    cur_long(min_cast),
                    req.vs_mr,
                    rt,
                    damage_minus_mr,
                    false,
                    req.vs_anti_magic,
                    0,
                ))?;
                max_cast = cur_to_long(calculate_resist_damage(
                    cur_long(max_cast),
                    req.vs_mr,
                    rt,
                    damage_minus_mr,
                    false,
                    req.vs_anti_magic,
                    0,
                ))?;
            }
            if sp.type_of_resists == 2 || (sp.type_of_resists == 1 && req.vs_anti_magic) {
                full_resist = vb::cint(vb::fix(f64::from(req.vs_mr) / 2.0))?;
                if full_resist > 98 {
                    full_resist = 98;
                }
            }
        }
        if elemental != 0 {
            let e = f64::from(elemental) / 100.0;
            min_cast = clng(vb::round(f64::from(min_cast) - f64::from(min_cast) * e, 0)?)?;
            max_cast = clng(vb::round(f64::from(max_cast) - f64::from(max_cast) * e, 0)?)?;
        }
        avg_cast = clng(vb::round(
            f64::from(vb::add_i32(min_cast, max_cast)?) / 2.0,
            0,
        )?)?;

        let casts = if sp.energy_cost > 0 && sp.energy_cost <= 500 {
            vb::fix(1000.0 / f64::from(sp.energy_cost))
        } else {
            1.0
        };

        r.n_min_cast = min_cast;
        r.n_max_cast = max_cast;
        r.n_avg_cast = if value_modified {
            who_scale(avg_modified)
        } else {
            avg_cast
        };
        r.n_num_casts = casts;
        r.n_mana_cost = vb::cint(f64::from(sp.mana_cost) * casts)?;
        r.n_cast_chance = cast_chance;
        // Quicksilver R9: an area damage spell that reaches monsters is cut, in expectation, by the
        // monsters that resist it (1.0 in stock, where the factor is exactly neutral).
        let area_keep = if area_damage && matches!(sp.targets, 3 | 5 | 9 | 11 | 12) {
            area_resist_keep(self.profile, req.vs_mr)
        } else {
            1.0
        };
        let area_note = if area_keep < 1.0 {
            area_resist_note(self.profile, req.vs_mr)
        } else {
            None
        };
        let scale = |n: i32, keep: f64| -> Res<i32> {
            clng(vb::round(
                ((f64::from(n) * casts) * (f64::from(cast_chance) / 100.0))
                    * (1.0 - (f64::from(full_resist) / 100.0))
                    * keep,
                0,
            )?)
        };
        r.n_avg_round_dmg = scale(damage, area_keep)?;
        r.n_min_round_dmg = scale(min_damage, area_keep)?;
        r.n_avg_round_heals = scale(heals, 1.0)?;
        r.n_duration = to_i16(duration)?;
        r.n_full_resist_chance = full_resist;

        if r.n_mana_cost > 0 && f64::from(r.n_mana_cost) <= who.max_mana {
            r.n_oom = calc_rounds_to_oom(
                f64::from(r.n_mana_cost) + who.spell_overhead,
                clng(who.max_mana)?,
                who.mana_regen,
                cast_chance,
                duration,
            );
        }

        // The accumulated resisted amount becomes a whole percentage.
        r.n_damage_resisted =
            resist_pct_signed_of_base(f64::from(avg_before_resist), f64::from(damage))?;

        // ---- description strings
        let mut s_avg = String::new();
        let mut s_mma = String::new();
        let mut s_inc = String::new();
        let mut s_cast_lvl = String::new();
        let show_lvl = (!lvl_specified || show_at_level) && cast_lvl > 0;
        if r.b_does_damage || r.b_does_heal {
            if show_lvl {
                s_cast_lvl = format!("(@lvl {cast_lvl}) ");
            }
            let shown = |other: i32| if duration > 1 { avg_cast } else { other };
            if r.b_does_damage && r.b_does_heal {
                s_avg = format!(
                    "{s_cast_lvl}{} damage + {} heals",
                    shown(r.n_avg_round_dmg),
                    r.n_avg_round_heals
                );
            } else if r.b_does_damage {
                s_avg = format!("{s_cast_lvl}{} damage", shown(r.n_avg_round_dmg));
            } else if r.b_does_heal {
                s_avg = format!("{s_cast_lvl}{} healing", shown(r.n_avg_round_heals));
            }
            if duration > 1 {
                s_avg.push_str(&format!("/{SPELL_ROUND_SECS}sec"));
            } else {
                s_avg.push_str("/round");
            }
            if duration > 1 {
                let secs = vb::mul_i32(duration, SPELL_ROUND_SECS)?;
                let total = vb::mul_i32(
                    vb::add_i32(r.n_avg_round_dmg, r.n_avg_round_heals)?,
                    duration,
                )?;
                s_avg.push_str(&format!(
                    " for {secs} secs/{} rounds ({total} total)",
                    vb::cstr(vb::fix(f64::from(secs) / f64::from(ROUND_SECS)))
                ));
                if r.n_damage_resisted != 0 {
                    s_avg.push_str(&format!(" after {}% damage resisted", r.n_damage_resisted));
                }
                let mut s_not = String::new();
                if lvl_specified && cast_chance < 100 {
                    s_not = auto_append(
                        &s_not,
                        &format!("{}% chance to fail cast", 100 - i32::from(cast_chance)),
                        " and ",
                    );
                }
                if r.n_full_resist_chance > 0 {
                    s_not = auto_append(
                        &s_not,
                        &format!("{}% chance to fully-resist", r.n_full_resist_chance),
                        " and ",
                    );
                }
                if let Some(note) = &area_note {
                    s_not = auto_append(&s_not, note, " and ");
                }
                if !s_not.is_empty() {
                    s_avg.push_str(&format!(", not including {s_not}"));
                }
            } else {
                if lvl_specified && cast_chance < 100 {
                    s_avg.push_str(&format!(" @ {cast_chance}% chance to cast"));
                }
                if r.n_damage_resisted != 0 {
                    s_avg.push_str(&format!(", {}% damage resisted", r.n_damage_resisted));
                }
                if r.n_full_resist_chance > 0 {
                    s_avg.push_str(&format!(
                        ", {}% chance to fully-resist",
                        r.n_full_resist_chance
                    ));
                }
                if let Some(note) = &area_note {
                    s_avg.push_str(&format!(", {note}"));
                }
            }
        }

        if r.n_min_cast > 0 && (r.n_min_cast != r.n_max_cast || r.n_max_cast != avg_cast) {
            if show_lvl {
                s_cast_lvl = format!(" (@lvl {cast_lvl})");
            }
            s_mma = format!(
                "Min/Avg/Max Cast{s_cast_lvl}: {}/{avg_cast}/{}",
                r.n_min_cast, r.n_max_cast
            );
            if r.n_num_casts > 1.0 {
                let n = r.n_num_casts;
                s_mma.push_str(&format!(
                    " x{}/round ({}/{}/{})",
                    vb::cstr(n),
                    vb::cstr(f64::from(r.n_min_cast) * n),
                    vb::cstr(f64::from(avg_cast) * n),
                    vb::cstr(f64::from(r.n_max_cast) * n)
                ));
            }
            if lvl_specified && duration == 1 {
                if r.n_full_resist_chance > 0 && cast_chance < 100 {
                    s_mma.push_str(" (before full resist & cast % reductions)");
                } else if r.n_full_resist_chance > 0 {
                    s_mma.push_str(" (before full resist reduction)");
                }
            }
        }

        if increases && (sp.cap == 0 || sp.cap > sp.req_level) {
            let mut names = String::new();
            let mut extra = String::new();
            let mut y = 0i16;
            for slot in &sp.abilities {
                if slot.ability > 0 && !ability_hidden_from_increases(slot.ability) {
                    let text = self
                        .text
                        .ability_stats(to_i16(slot.ability)?, 0, false, false);
                    if vb::len(&text) > 0 {
                        y = vb::add_i16(y, 1)?;
                        if slot.value == 0 {
                            names = auto_append(&names, &text, ", ");
                        } else {
                            extra = auto_append(&extra, &text, ", ");
                        }
                    }
                }
            }
            if !names.is_empty() {
                if !extra.is_empty() {
                    names = format!("{names} (not {extra})");
                }
                let mut diffs = String::new();
                let m = get_current_spell_min_max(
                    Some(sp),
                    self.avg_level_max_all_stats,
                    &MinMaxRequest::default(),
                );
                if cur_str(m.n_dur) != m.s_dur {
                    diffs = auto_append(&diffs, &format!("Duration: {}", m.s_dur), ", ");
                }
                if cur_str(m.n_min) != m.s_min {
                    diffs = auto_append(&diffs, &format!("Min: {}", m.s_min), ", ");
                }
                if cur_str(m.n_max) != m.s_max {
                    diffs = auto_append(&diffs, &format!("Max: {}", m.s_max), ", ");
                }
                if !diffs.is_empty() {
                    s_inc = format!("LVL Increases: {diffs}");
                    if y > 1 {
                        s_inc.push_str(&format!(" for: {names}"));
                    }
                }
            }
        }

        r.s_avg_round = s_avg;
        r.s_mma = s_mma;
        r.s_lvl_increases = s_inc;
        Ok(())
    }
}
