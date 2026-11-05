//! Spell damage equations (FAM-10). Card P3-04.
//!
//! Lookups take the Spells table as a slice; a failed `tabSpells.Seek` is "no row with that
//! number", and MME's `MoveFirst` after a failed seek has no visible effect once the cursor is
//! gone. Globals (`bQuickSpell`, `nSpellNest`, the global level box, `gAvgLevelMaxAllStats`) live
//! in [`SpellEnv`].
//!
//! Where an MME procedure has an `On Error` handler that resumes at `out:`, the port returns what
//! the procedure had computed when the error hit, as MME does; no `Result` escapes from those.
//!
//! `PullSpellEQ` builds its text from `GetAbilityStats`, `GetSpellName` and `GetMonsterName`,
//! which other cards port; the caller supplies them through [`SpellText`]. The ListView rows that
//! `PullSpellEQ` adds when given a list (teleport, summon and textblock rows) are UI and are not
//! ported: the port always behaves as MME does with `lv = Nothing`. MME's `nSpellNest` guard is
//! kept, but a [`SpellText`] implementation that calls back into `pull_spell_eq` (as MME's
//! `GetAbilityStats` does) shares the same counter through [`SpellEnv::nest`].

use std::cell::Cell;

use touchstone_model::Spell;
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Vb6Error};

use crate::{Profile, RulesError};

type Res<T> = Result<T, Vb6Error>;

fn to_i16(v: i32) -> Res<i16> {
    i16::try_from(v).map_err(|_| Vb6Error::Overflow)
}

fn cur_long(v: i32) -> Currency {
    Currency::from_scaled(i64::from(v) * 10_000)
}

/// `Fix` of a Currency: truncation toward zero, still a Currency.
fn fix_cur(c: Currency) -> Currency {
    Currency::from_scaled(c.scaled() / 10_000 * 10_000)
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

/// `Round(c, 2)` on a Currency: exact decimal rounding half to even, still a Currency.
fn round2_cur(c: Currency) -> Currency {
    Currency::from_scaled(div_half_even(c.scaled(), 100) * 100)
}

/// A Currency passed to a `ByVal ... As Long` parameter: `CLng`, banker's rounding.
fn cur_clng(c: Currency) -> Res<i32> {
    i32::try_from(div_half_even(c.scaled(), 10_000)).map_err(|_| Vb6Error::Overflow)
}

/// `&` on a Currency: up to four decimals, trailing zeros dropped.
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

/// Port of modSyntaxsFunc AutoAppend with the default ", " glue.
fn auto_append(base: &str, add: &str) -> String {
    if base.is_empty() {
        add.to_string()
    } else if add.trim_matches(' ').is_empty() {
        base.to_string()
    } else {
        format!("{base}, {add}")
    }
}

/// `Replace(s, find, repl, , , vbTextCompare)` for an ASCII `find`.
fn replace_ci(s: &str, find: &str, repl: &str) -> String {
    let (ls, lf) = (s.to_ascii_lowercase(), find.to_ascii_lowercase());
    let mut out = String::new();
    let mut at = 0;
    while let Some(i) = ls[at..].find(&lf) {
        out.push_str(&s[at..at + i]);
        out.push_str(repl);
        at += i + lf.len();
    }
    out.push_str(&s[at..]);
    out
}

// ---------------------------------------------------------------------------------------------
// Simple lookups

/// Port of modMMudDatabase GetSpellAttackType: `AttType`, 0 for no such spell.
#[must_use]
pub fn get_spell_attack_type(spells: &[Spell], number: i32) -> i32 {
    if number == 0 {
        return 0;
    }
    // The result is an Integer; a Long outside its range is error 6, which the handler turns into 0.
    find(spells, number).map_or(0, |s| to_i16(s.att_type).map_or(0, i32::from))
}

/// Port of modMMudDatabase SpellIsAreaAttack: targets "Divided Attack Area", "Full Area" or
/// "Full Attack Area" (9, 11, 12).
#[must_use]
pub fn spell_is_area_attack(spells: &[Spell], number: i32) -> bool {
    find(spells, number).is_some_and(|s| matches!(s.targets, 9 | 11 | 12))
}

/// Port of modMMudDatabase SpellIsBlessSpell: a timed spell that a character can learn or cast
/// on themselves (targets "Self", "Self or User", "Divided Area (incl self)" or "Full Party
/// Area").
#[must_use]
pub fn spell_is_bless_spell(spells: &[Spell], number: i32) -> bool {
    let Some(s) = find(spells, number) else {
        return false;
    };
    if s.dur == 0 && (s.dur_inc == 0 || s.dur_inc_lvls == 0) {
        return false;
    }
    if s.learnable
        || s.learned_from.as_deref().is_some_and(|f| !f.is_empty())
        || (s.magery == 5 && s.req_level > 0)
    {
        return matches!(s.targets, 1 | 2 | 5 | 13);
    }
    true
}

/// Port of modMMudDatabase SpellDoesDamage. `not_duration` rejects timed spells; a spell whose
/// EndCast spell does damage counts too.
#[must_use]
pub fn spell_does_damage(spells: &[Spell], number: i32, not_duration: bool) -> bool {
    if number == 0 {
        return false;
    }
    let Some(s) = find(spells, number) else {
        return false;
    };
    if not_duration && (s.dur > 0 || (s.dur_inc_lvls > 0 && s.dur_inc > 0)) {
        return false;
    }
    let mut end_cast = 0;
    for slot in &s.abilities {
        match slot.ability {
            1 | 8 | 17 => return true,
            151 => end_cast = slot.value,
            _ => {}
        }
    }
    end_cast > 0 && spell_does_damage(spells, end_cast, false)
}

/// Port of modSyntaxsFunc RoundUpTo5.
///
/// # Errors
/// Overflow near the top of the Long range.
fn round_up_to_5(n: i32) -> Res<i32> {
    let r = vb::vmod(f64::from(n), 5.0)?;
    if r == 0 {
        Ok(n)
    } else if n >= 0 {
        vb::add_i32(n, 5 - r)
    } else {
        vb::sub_i32(n, r)
    }
}

/// Port of modMMudDatabase GetMaxLevel: 255, or the average level to max all stats (`gAvgLevelMaxAllStats`)
/// rounded up to a multiple of 5 when that is higher.
///
/// # Errors
/// VB6 error 6 when the average does not fit a Long (MME has no handler there).
pub fn get_max_level(avg_level_max_all_stats: f64) -> Result<i32, RulesError> {
    let vb6 = |e: Vb6Error| RulesError::Vb6(i32::try_from(e.code()).unwrap_or(0));
    let mut max = 255;
    if avg_level_max_all_stats > 0.0 {
        let n = round_up_to_5(vb::clng(avg_level_max_all_stats).map_err(vb6)?).map_err(vb6)?;
        if n > 255 {
            max = n;
        }
    }
    Ok(max)
}

// ---------------------------------------------------------------------------------------------
// Damage and duration at a cast level

#[derive(Clone, Copy, PartialEq, Eq)]
enum Which {
    Min,
    Max,
}

fn clamp_cast_level(s: &Spell, level: i16) -> Res<i16> {
    let mut level = level;
    if i32::from(level) > s.cap && s.cap > 0 {
        level = to_i16(s.cap)?;
    }
    if i32::from(level) < s.req_level {
        level = to_i16(s.req_level)?;
    }
    Ok(level)
}

/// `base + Fix((inc / lvls) * level)` assigned to a Long, or just `base` for no increment.
fn level_scaled(base: i32, inc: i32, lvls: i32, level: i16) -> Res<i32> {
    if lvls == 0 || level < 1 {
        return Ok(base);
    }
    let per = vb::div(f64::from(inc), f64::from(lvls))?;
    vb::clng(f64::from(base) + vb::fix(per * f64::from(level)))
}

#[derive(Clone, Copy)]
struct DamageArgs {
    level: i16,
    for_monster: bool,
    heals_instead: bool,
    which: Which,
}

fn damage_inner(
    spells: &[Spell],
    number: i32,
    a: &DamageArgs,
    energy_rem: &mut i16,
    result: &mut i32,
) -> Res<()> {
    *result = 0;
    if number == 0 {
        return Ok(());
    }
    let Some(s) = find(spells, number) else {
        return Ok(());
    };
    let heals = a.heals_instead;
    let mut does = false;
    let mut end_cast = 0;
    for slot in &s.abilities {
        match slot.ability {
            1 | 8 | 17 | 18 => {
                let heal_kind = slot.ability == 18 || (slot.ability == 8 && heals);
                if heal_kind != heals {
                    continue;
                }
                does = true;
                if slot.value != 0 {
                    *result = slot.value;
                }
            }
            151 => end_cast = slot.value,
            _ => {}
        }
    }
    let mut level = a.level;
    if *result == 0 {
        if !does {
            return Ok(());
        }
        if !a.for_monster || level == 0 {
            level = clamp_cast_level(s, level)?;
        }
        *result = match a.which {
            Which::Min => level_scaled(s.min_base, s.min_inc, s.min_inc_lvls, level)?,
            Which::Max => level_scaled(s.max_base, s.max_inc, s.max_inc_lvls, level)?,
        };
    }
    if a.for_monster {
        return Ok(());
    }
    if *energy_rem == 0 {
        *energy_rem = 1000;
    }
    *energy_rem = to_i16(i32::from(*energy_rem) - s.energy_cost)?;
    if *energy_rem < 1 {
        *energy_rem = 1;
    }
    if *energy_rem >= 143 && s.energy_cost >= 143 {
        if end_cast == 0 {
            if s.energy_cost <= 500 {
                let times = vb::fix(vb::div(f64::from(*energy_rem), f64::from(s.energy_cost))?);
                *result = vb::clng(f64::from(*result) + f64::from(*result) * times)?;
            }
        } else {
            // The recursive call does not pass bHealsInstead; nEnergyRem is shared (ByRef).
            let inner = DamageArgs {
                heals_instead: false,
                ..*a
            };
            let mut sub = 0;
            let _ = damage_inner(
                spells,
                end_cast,
                &DamageArgs { level, ..inner },
                energy_rem,
                &mut sub,
            );
            *result = vb::add_i32(*result, sub)?;
        }
    }
    Ok(())
}

fn damage(spells: &[Spell], number: i32, a: &DamageArgs, energy_rem: &mut i16) -> i32 {
    let mut r = 0;
    let _ = damage_inner(spells, number, a, energy_rem, &mut r);
    r
}

/// Port of modMMudDatabase GetSpellMinDamage: the lowest damage (or heal, with `heals_instead`)
/// at `cast_level`, counting the extra casts a full round of energy allows and the EndCast
/// spell. `energy_rem` is MME's ByRef `nEnergyRem` (0 means a full 1000); it is left as MME leaves it.
pub fn get_spell_min_damage(
    spells: &[Spell],
    number: i32,
    cast_level: i16,
    energy_rem: &mut i16,
    for_monster: bool,
    heals_instead: bool,
) -> i32 {
    let a = DamageArgs {
        level: cast_level,
        for_monster,
        heals_instead,
        which: Which::Min,
    };
    damage(spells, number, &a, energy_rem)
}

/// Port of modMMudDatabase GetSpellMaxDamage; see [`get_spell_min_damage`].
pub fn get_spell_max_damage(
    spells: &[Spell],
    number: i32,
    cast_level: i16,
    energy_rem: &mut i16,
    for_monster: bool,
    heals_instead: bool,
) -> i32 {
    let a = DamageArgs {
        level: cast_level,
        for_monster,
        heals_instead,
        which: Which::Max,
    };
    damage(spells, number, &a, energy_rem)
}

/// Port of modMMudDatabase GetSpellDuration: rounds at `cast_level`.
#[must_use]
pub fn get_spell_duration(
    spells: &[Spell],
    number: i32,
    cast_level: i16,
    for_monster: bool,
) -> i32 {
    fn inner(spells: &[Spell], number: i32, level: i16, for_monster: bool, r: &mut i32) -> Res<()> {
        if number == 0 {
            return Ok(());
        }
        let Some(s) = find(spells, number) else {
            return Ok(());
        };
        let mut level = level;
        if !for_monster || level == 0 {
            level = clamp_cast_level(s, level)?;
        }
        *r = level_scaled(s.dur, s.dur_inc, s.dur_inc_lvls, level)?;
        Ok(())
    }
    let mut r = 0;
    let _ = inner(spells, number, cast_level, for_monster, &mut r);
    r
}

// ---------------------------------------------------------------------------------------------
// GetCurrentSpellMinMax

/// Inputs of `GetCurrentSpellMinMax`; `use_level` and `no_header` are its ByRef arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MinMaxRequest {
    pub use_level: bool,
    pub level: i16,
    pub no_header: bool,
    pub override_min: i32,
    pub override_max: i32,
    pub spell_bonus: i16,
}

/// MME's `SpellMinMaxDur` plus the two ByRef arguments as the call leaves them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellMinMaxDur {
    pub n_min: Currency,
    pub n_max: Currency,
    pub n_dur: Currency,
    pub s_min: String,
    pub s_max: String,
    pub s_dur: String,
    pub use_level: bool,
    pub no_header: bool,
}

fn min_max_inner(spell: &Spell, avg: f64, w: &mut SpellMinMaxDur, req: &MinMaxRequest) -> Res<()> {
    let s = spell;
    let bonus = if req.spell_bonus < 1 {
        cur_long(1)
    } else {
        Currency::from_f64(1.0 + f64::from(req.spell_bonus) / 100.0)?
    };
    let boosted = bonus > cur_long(1);
    let ov = req.override_min != 0 || req.override_max != 0;
    let mut level = req.level;
    w.use_level = req.use_level;
    w.no_header = req.no_header;

    w.n_min = cur_long(if ov { req.override_min } else { s.min_base });
    let (min_incr, min_lvls) = (cur_long(s.min_inc), cur_long(s.min_inc_lvls));
    w.n_max = cur_long(if ov { req.override_max } else { s.max_base });
    let (max_incr, max_lvls) = (cur_long(s.max_inc), cur_long(s.max_inc_lvls));
    w.n_dur = cur_long(s.dur);
    let (dur_incr, dur_lvls) = (cur_long(s.dur_inc), cur_long(s.dur_inc_lvls));
    let zero = cur_long(0);

    if level == 0 && !ov && (min_lvls > zero || max_lvls > zero || dur_lvls > zero) {
        let m = get_max_level(avg).map_err(|_| Vb6Error::Overflow)?;
        level = to_i16(m)?;
    }
    if w.use_level
        && (min_incr == zero || min_lvls == zero)
        && (max_incr == zero || max_lvls == zero)
        && (dur_incr == zero || dur_lvls == zero)
    {
        w.use_level = false;
    }
    if s.cap == 0 && s.req_level == 0 && !w.use_level && level == 0 {
        if boosted {
            w.n_min = fix_cur(w.n_min.checked_mul(bonus)?);
            w.n_max = fix_cur(w.n_max.checked_mul(bonus)?);
        }
        w.s_dur = cur_str(w.n_dur);
        w.s_max = cur_str(w.n_max);
        w.s_min = cur_str(w.n_min);
        return Ok(());
    }
    // (value, increment, levels, override) for min then max share one shape.
    let bonus_text = format!("+{}%", req.spell_bonus);
    for is_min in [true, false] {
        let (val, incr, lvls) = if is_min {
            (&mut w.n_min, min_incr, min_lvls)
        } else {
            (&mut w.n_max, max_incr, max_lvls)
        };
        let mut text = String::new();
        if ov || lvls == zero || incr == zero {
            if boosted {
                *val = fix_cur(val.checked_mul(bonus)?);
            }
            text = cur_str(*val);
        } else {
            let per = incr.checked_div(lvls)?;
            if !w.use_level {
                w.no_header = true;
                text = format!("{}+({}*lvl)", cur_str(*val), cur_str(round2_cur(per)));
                if boosted {
                    text.push_str(&bonus_text);
                }
            }
            if w.use_level || level > 0 {
                *val = val.checked_add(fix_cur(per.checked_mul(cur_long(i32::from(level)))?))?;
                if boosted {
                    *val = fix_cur(val.checked_mul(bonus)?);
                }
                if w.use_level {
                    text = cur_str(*val);
                }
            }
        }
        if is_min {
            w.s_min = text;
        } else {
            w.s_max = text;
        }
    }
    if dur_lvls == zero || dur_incr == zero {
        w.s_dur = cur_str(w.n_dur);
    } else {
        let per = dur_incr.checked_div(dur_lvls)?;
        if !w.use_level {
            w.s_dur = format!("{}+({}*lvl)", cur_str(w.n_dur), cur_str(round2_cur(per)));
        }
        if w.use_level || level > 0 {
            w.n_dur = w
                .n_dur
                .checked_add(fix_cur(per.checked_mul(cur_long(i32::from(level)))?))?;
            w.n_dur = fix_cur(w.n_dur);
            if w.use_level {
                w.s_dur = cur_str(w.n_dur);
            }
        }
    }
    Ok(())
}

/// Port of modMMudDatabase GetCurrentSpellMinMax for the spell the cursor is on (`None` is a
/// cursor at EOF: everything empty). `avg_level_max_all_stats` is the `gAvgLevelMaxAllStats`
/// global that `GetMaxLevel` reads.
///
/// With a level the strings are the numbers at that level; without one they are formulas such as
/// `4+(0.5*lvl)`. On a VB6 error the partly filled result is returned, as MME's handler does.
#[must_use]
pub fn get_current_spell_min_max(
    spell: Option<&Spell>,
    avg_level_max_all_stats: f64,
    req: &MinMaxRequest,
) -> SpellMinMaxDur {
    let zero = Currency::from_scaled(0);
    let mut w = SpellMinMaxDur {
        n_min: zero,
        n_max: zero,
        n_dur: zero,
        s_min: String::new(),
        s_max: String::new(),
        s_dur: String::new(),
        use_level: req.use_level,
        no_header: req.no_header,
    };
    if let Some(s) = spell {
        let _ = min_max_inner(s, avg_level_max_all_stats, &mut w, req);
    }
    w
}

// ---------------------------------------------------------------------------------------------
// PullSpellEQ

/// The text procedures `PullSpellEQ` calls that belong to other cards.
pub trait SpellText {
    /// modMMudFunc GetAbilityStats with `lv = Nothing`: the ability's name and value text.
    /// `calc_level` is its `bCalcSpellLevel` (True when MME's call leaves it out).
    fn ability_stats(
        &self,
        ability: i16,
        value: i16,
        calc_level: bool,
        percent_column: bool,
    ) -> String;
    /// modMMudDatabase GetSpellName(number, bHideRecordNumbers).
    fn spell_name(&self, number: i32) -> String;
    /// modMMudDatabase GetMonsterName(number, bHideRecordNumbers).
    fn monster_name(&self, number: i32) -> String;
}

/// The globals and lookups `PullSpellEQ` reads.
pub struct SpellEnv<'a> {
    pub profile: Profile,
    pub spells: &'a [Spell],
    pub text: &'a dyn SpellText,
    /// `bQuickSpell`: the short "click" style used in lists.
    pub quick_spell: bool,
    /// `Val(txtGlobalLevel(0).Text)` source: `Some(text)` when `chkGlobalFilter` is on.
    pub global_level_text: Option<&'a str>,
    /// `gAvgLevelMaxAllStats`.
    pub avg_level_max_all_stats: f64,
    /// `nSpellNest`: recursion depth, shared with anything that calls back.
    pub nest: Cell<i32>,
}

/// Arguments of `PullSpellEQ` (all but `lv`, which is always Nothing here).
#[derive(Debug, Clone, Copy, Default)]
pub struct PullArgs {
    pub calc_level: bool,
    pub level: i16,
    /// The spell number. MME's 0 ("the spell the cursor is on") is passed as that spell's number.
    pub spell: i32,
    pub min_max_damage_only: bool,
    pub for_monster: bool,
    pub percent_column: bool,
    pub is_nested: bool,
    pub no_show_level: bool,
    pub override_min: i32,
    pub override_max: i32,
    pub spell_bonus: i16,
}

impl SpellEnv<'_> {
    /// Port of modMMudDatabase PullSpellEQ: a spell's effect text at a level
    /// (`"Damage 5 to 9, ... x2 times/round for 10 rounds"`), or `"min:max:dur"` with
    /// `min_max_damage_only`.
    #[must_use]
    pub fn pull_spell_eq(&self, a: &PullArgs) -> String {
        self.nest.set(self.nest.get() + 1);
        let mut out = String::new();
        let _ = self.pull_inner(a, &mut out);
        self.nest.set(self.nest.get() - 1);
        out
    }

    fn pull(&self, a: &PullArgs, level: i16, spell: i32) -> String {
        self.pull_spell_eq(&PullArgs {
            calc_level: a.calc_level,
            level,
            spell,
            percent_column: a.percent_column,
            ..PullArgs::default()
        })
    }

    #[allow(clippy::too_many_lines)]
    fn pull_inner(&self, a: &PullArgs, out: &mut String) -> Res<()> {
        let quick = self.quick_spell;
        if self.spells.is_empty() {
            *out = "(No Spell Records)".into();
            return Ok(());
        }
        if self.nest.get() > 19 {
            *out = " ... to infinity and beyond?".into();
            return Ok(());
        }
        if quick && self.nest.get() > 1 {
            *out = "(click)".into();
            return Ok(());
        }
        let Some(sp) = find(self.spells, a.spell) else {
            *out = "?".into();
            return Ok(());
        };
        let mut level = a.level;
        let mut use_level = a.calc_level;
        if use_level {
            if level == 0
                && let Some(t) = self.global_level_text
            {
                level = vb::cint(vb::val(t))?;
            }
            if !a.for_monster {
                if i32::from(level) > sp.cap && sp.cap > 0 {
                    level = to_i16(sp.cap)?;
                }
                if i32::from(level) < sp.req_level {
                    level = to_i16(sp.req_level)?;
                }
            }
            if level < 1 {
                level = to_i16(sp.req_level)?;
            }
            if level == 0 {
                use_level = false;
            }
        }

        let mut req = MinMaxRequest {
            use_level,
            level,
            no_header: false,
            override_min: a.override_min,
            override_max: a.override_max,
            spell_bonus: 0,
        };
        let base = get_current_spell_min_max(Some(sp), self.avg_level_max_all_stats, &req);
        req.use_level = base.use_level;
        req.no_header = base.no_header;
        let (use_level, no_header) = (base.use_level, base.no_header);
        let (n_min, n_max, n_dur) = (base.n_min, base.n_max, base.n_dur);
        let (s_min, s_max, s_dur) = (
            base.s_min.as_str(),
            base.s_max.as_str(),
            base.s_dur.as_str(),
        );
        let bonus_vals = if a.spell_bonus > 0 {
            req.spell_bonus = a.spell_bonus;
            Some(get_current_spell_min_max(
                Some(sp),
                self.avg_level_max_all_stats,
                &req,
            ))
        } else {
            None
        };
        // The bonus call may flip the ByRef flags again; later code reads the final ones.
        let (use_level, no_header) = bonus_vals
            .as_ref()
            .map_or((use_level, no_header), |b| (b.use_level, b.no_header));
        let (s_min_b, s_max_b) = bonus_vals
            .as_ref()
            .map_or(("", ""), |b| (b.s_min.as_str(), b.s_max.as_str()));

        let mut detail = String::new();
        let mut removes = String::new();
        let mut end_one = String::new();
        let mut end_two = String::new();
        let mut end_cast_pct = String::new();
        let mut does_damage = false;
        let mut gets_bonus = false;
        let mut non_magical = false;
        let range =
            |lo: Currency, hi: Currency| -> Res<(i32, i32)> { Ok((cur_clng(lo)?, cur_clng(hi)?)) };

        for slot in &sp.abilities {
            let abil = slot.ability;
            if abil == 0 {
                continue;
            }
            if abil == 144 {
                non_magical = true;
            }
            gets_bonus = false;
            match abil {
                1 | 17 => {
                    gets_bonus = true;
                    does_damage = true;
                }
                8 | 18 => {
                    match self.profile {
                        Profile::Stock => {}
                    }
                    does_damage = true;
                }
                19 => does_damage = true,
                _ => {}
            }
            if a.min_max_damage_only {
                break;
            }
            let (mut min_h, mut max_h) = ("", "");
            let v = slot.value;
            let ab16 = to_i16(abil)?;
            if abil == 122 {
                if quick {
                    if removes.is_empty() {
                        removes = "click".into();
                    }
                } else {
                    if !removes.is_empty() {
                        removes.push_str(", ");
                    }
                    removes.push_str(&self.text.spell_name(v));
                }
            } else if abil == 137 {
                // shock: only the message
            } else {
                let both = |mn: &str, mx: &str, mh: &str, xh: &str| {
                    if mn == mx {
                        format!("{mh}{mn}")
                    } else {
                        format!("{mh}{mn} to {xh}{mx}")
                    }
                };
                let list_end_two = matches!(
                    abil,
                    23 | 51 | 52 | 80 | 97 | 98 | 100 | 108..=113 | 119 | 138 | 144 | 178
                );
                if v == 0 {
                    match abil {
                        140 | 148 => {
                            let t = self.text.ability_stats(ab16, 0, true, a.percent_column);
                            detail = auto_append(
                                &detail,
                                &format!("{t} {}", both(s_min, s_max, "", "")),
                            );
                        }
                        164 => end_cast_pct = format!("{v}% "),
                        151 => {
                            let (lo, hi) = range(n_min, n_max)?;
                            if quick {
                                let t = if n_max > n_min {
                                    format!(
                                        "{end_cast_pct}End cast {} to {}",
                                        cur_str(n_min),
                                        cur_str(n_max)
                                    )
                                } else {
                                    format!("{end_cast_pct}End cast {}", cur_str(n_min))
                                };
                                end_one = auto_append(&end_one, &t);
                            } else if n_min >= n_max {
                                let t = format!(
                                    "{end_cast_pct}EndCast [{}, {}]",
                                    self.text.spell_name(lo),
                                    self.pull(a, level, lo)
                                );
                                end_one = auto_append(&end_one, &t);
                            } else {
                                let mut t = format!(
                                    "{end_cast_pct}EndCast [{{{}, {}}}",
                                    self.text.spell_name(lo),
                                    self.pull(a, level, lo)
                                );
                                for y in
                                    (cur_clng(Currency::from_scaled(n_min.scaled() + 10_000))?)..=hi
                                {
                                    t.push_str(&format!(
                                        " OR {{{}, {}}}",
                                        self.text.spell_name(y),
                                        self.pull(a, level, y)
                                    ));
                                }
                                t.push(']');
                                end_one = auto_append(&end_one, &t);
                            }
                        }
                        12 => {
                            let (lo, hi) = range(n_min, n_max)?;
                            if quick {
                                detail = auto_append(&detail, "Summon");
                            } else if n_min >= n_max {
                                detail = auto_append(
                                    &detail,
                                    &format!("Summon {}", self.text.monster_name(lo)),
                                );
                            } else {
                                detail = auto_append(
                                    &detail,
                                    &format!("Summons{{{}", self.text.monster_name(lo)),
                                );
                                let from =
                                    cur_clng(Currency::from_scaled(n_min.scaled() + 10_000))?;
                                for y in from..=hi {
                                    detail.push_str(&format!(" OR {}", self.text.monster_name(y)));
                                }
                                detail.push('}');
                            }
                        }
                        _ if list_end_two => {
                            end_two = auto_append(
                                &end_two,
                                &self.text.ability_stats(ab16, 0, true, false),
                            );
                        }
                        7 => {
                            if !no_header {
                                if vb::val(s_min) > 0.0 {
                                    min_h = "+";
                                }
                                if vb::val(s_max) > 0.0 {
                                    max_h = "+";
                                }
                            }
                            let t = self.text.ability_stats(ab16, 0, true, a.percent_column);
                            let tail = if use_level {
                                let ten = cur_long(10);
                                let (dmin, dmax) =
                                    (n_min.checked_div(ten)?, n_max.checked_div(ten)?);
                                if n_min == n_max {
                                    format!("{min_h}{}", cur_str(dmin))
                                } else {
                                    format!("{min_h}{} to {max_h}{}", cur_str(dmin), cur_str(dmax))
                                }
                            } else {
                                both(s_min, s_max, min_h, max_h)
                            };
                            detail = auto_append(&detail, &format!("{t} {tail}"));
                        }
                        _ => {
                            if !no_header && !matches!(abil, 1 | 8 | 17 | 18 | 19 | 140 | 141 | 148)
                            {
                                if vb::val(s_min) > 0.0 {
                                    min_h = "+";
                                }
                                if vb::val(s_max) > 0.0 {
                                    max_h = "+";
                                }
                            }
                            let t =
                                self.text
                                    .ability_stats(ab16, 0, a.calc_level, a.percent_column);
                            let tail = if a.spell_bonus > 0 && gets_bonus {
                                both(s_min_b, s_max_b, min_h, max_h)
                            } else {
                                both(s_min, s_max, min_h, max_h)
                            };
                            detail = auto_append(&detail, &format!("{t} {tail}"));
                        }
                    }
                } else {
                    let v16 = to_i16(v)?;
                    match abil {
                        148 | 140 => {
                            let t = self.text.ability_stats(ab16, v16, true, a.percent_column);
                            detail = auto_append(&detail, &t);
                        }
                        12 => {
                            if quick {
                                detail = auto_append(&detail, "Summon");
                            } else {
                                detail = auto_append(
                                    &detail,
                                    &format!("Summon {}", self.text.monster_name(v)),
                                );
                            }
                        }
                        164 => end_cast_pct = format!("{v}% "),
                        151 => {
                            let t =
                                self.text
                                    .ability_stats(ab16, v16, a.calc_level, a.percent_column);
                            end_one = auto_append(&end_one, &format!("{end_cast_pct}{t}"));
                        }
                        _ if list_end_two => {
                            end_two = auto_append(
                                &end_two,
                                &self.text.ability_stats(ab16, v16, true, false),
                            );
                        }
                        _ => {
                            let t =
                                self.text
                                    .ability_stats(ab16, v16, a.calc_level, a.percent_column);
                            detail = auto_append(&detail, &t);
                        }
                    }
                }
                if detail.ends_with(", ") {
                    detail.truncate(detail.len() - 2);
                }
            }
        }
        if non_magical {
            detail = replace_ci(&detail, "Damage(-MR)", "Damage");
        }

        if a.min_max_damage_only {
            *out = if does_damage {
                let mut r = if a.spell_bonus > 0 && gets_bonus {
                    format!("{s_min_b}:{s_max_b}")
                } else {
                    format!("{s_min}:{s_max}")
                };
                if n_dur > cur_long(0) {
                    r.push(':');
                    r.push_str(s_dur);
                }
                r
            } else {
                "0:0:0".into()
            };
            return Ok(());
        }
        if detail.is_empty() && removes.is_empty() && end_one.is_empty() && end_two.is_empty() {
            *out = "(No EQ)".into();
            return Ok(());
        }
        *out = detail;
        if !end_one.is_empty() {
            *out = auto_append(out, &end_one);
        }
        if !a.is_nested && sp.energy_cost > 0 && sp.energy_cost <= 500 {
            let times = vb::fix(vb::div(1000.0, f64::from(sp.energy_cost))?);
            out.push_str(&format!(" x{}", vb::cstr(times)));
            out.push_str(if quick { "/rnd" } else { " times/round" });
        }
        if !end_two.is_empty() {
            *out = auto_append(out, &end_two);
        }
        let grows = (sp.min_inc != 0 && sp.min_inc_lvls > 0)
            || (sp.max_inc != 0 && sp.max_inc_lvls > 0)
            || (sp.dur_inc != 0 && sp.dur_inc_lvls > 0);
        if !a.no_show_level
            && level > 0
            && (use_level || ((sp.cap == 0 || sp.cap > sp.req_level) && grows))
            && (sp.cap > 0 || sp.req_level > 0)
        {
            *out = format!("(@lvl {level}): {out}");
        }
        if s_dur != "0" {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&format!("for {s_dur} rounds"));
        }
        if quick {
            return Ok(());
        }
        if !removes.is_empty() {
            if !out.is_empty() {
                out.push_str(" -- ");
            }
            out.push_str(&format!("RemovesSpells({removes})"));
        }
        Ok(())
    }
}
