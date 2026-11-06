//! Lair statistics (FAM-11, statistics half). Card P3-15.
//!
//! A monster's "Summoned By" text lists the places it spawns; a lair entry reads
//! `[7-8-9][6]Group(lair): 1/2345` (group index, max regen, then two numbers MME ignores). From those
//! entries MME builds the per-monster averages ([`get_lair_averages_from_locs`]), counts lairs by regen
//! ([`count_lairs_by_max_regen`]) and fills the per-monster spawn arrays
//! ([`calculate_average_lairs`]). The median/outlier helpers behind the walk-time average are here too.
//!
//! `GetLairInfo` (the per-group cache) and `GetDamageOutput` are callees the statistics reach through
//! [`LairEnv`]. Card P3-16 adds the cache ([`LairCache`]: `LoadLairInfo`, `GetLairInfoIndex`,
//! `SetLairInfo`, `GetLairInfo`) and the real [`LairEnv`] over it ([`CharLairEnv`]).
//!
//! Where an MME procedure has an `On Error ... Resume out` handler, the port returns what the
//! procedure had assigned when the error hit, as MME does; no `Result` escapes from those.
//! Stock only: the GreaterMUD and Paramud lair branches are not ported.

use std::collections::HashMap;

use touchstone_model::{Lair, Monster, World};
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Rnd, Vb6Error};

use crate::character::PartyFilter;
use crate::combat::{
    CombatRoundsArgs, DF_ANIMAL, DF_ANTI_MAGIC, DF_LIVING, DF_UNDEAD, DamageEnv, DamageOutput,
    DamageRequest, calc_average_non_zero, calc_combat_rounds, get_damage_output,
};
use crate::monster_glue::{
    GlueEnv, PreCalcInput, calculate_monster_damage_vs_char_all, get_pre_calculated_monster_damage,
};
use crate::monster_sim::MonsterSimInput;
use crate::{Profile, RulesError};

type Res<T> = Result<T, Vb6Error>;

/// MME's `LAIR_FLAG_RATIO`: share of a lair's mobs that must have a trait for the lair to count as having it.
const LAIR_FLAG_RATIO: f64 = 0.9;
/// MME's `MAJ_THRESH_PCT`: percent of the lairs that must agree for an accuracy majority.
const MAJ_THRESH_PCT: i32 = 51;

// ---------------------------------------------------------------------------------------------
// Types

/// MME's `LairInfoType`, field for field (`n` and `s` prefixes dropped; Currency stays [`Currency`],
/// Integer is `i16`, Long is `i32`). `GetLairInfo` returns the same type.
#[derive(Debug, Clone, PartialEq)]
pub struct LairInfo {
    pub group_index: String,
    pub mob_list: String,
    pub mobs: Currency,
    pub max_regen: Currency,
    pub avg_exp: Currency,
    /// Average damage per mob per round (one mob, alone).
    pub avg_dmg: Currency,
    pub accy_majority: i32,
    pub accy_max: i32,
    pub avg_hp: i32,
    pub avg_ac: i16,
    pub avg_dr: i16,
    pub avg_mr: i16,
    pub avg_rcol: i16,
    pub avg_rfir: i16,
    pub avg_rsto: i16,
    pub avg_rlit: i16,
    pub avg_rwat: i16,
    pub avg_dodge: i16,
    pub avg_bs_defense: i16,
    pub total_lairs: i32,
    pub avg_walk: Currency,
    pub avg_delay: f64,
    pub damage_mitigated: i32,
    pub damage_out: i32,
    pub first_round_damage_out: i32,
    pub min_round_damage_out: i32,
    pub surprise_damage_out: i32,
    pub surprise_min_damage_out: i32,
    pub surprise_chance: i16,
    pub poss_spawns: i32,
    pub global_attack_config: String,
    pub avg_dmg_lair: Currency,
    pub avg_dmg_clear_round: Currency,
    pub avg_dmg_clear: Currency,
    /// Rounds to kill each mob.
    pub rtk: f64,
    /// Rounds to clear the lair.
    pub rtc: f64,
    pub magic_lvl: i16,
    pub max_magic_lvl: i16,
    pub spell_immu_lvl: i16,
    pub max_spell_immu_lvl: i16,
    pub num_undeads: i16,
    pub num_anti_magic: i16,
    pub num_animals: i16,
    pub num_living: i16,
}

impl Default for LairInfo {
    /// A fresh `LairInfoType`: empty strings and zeros.
    fn default() -> Self {
        let z = Currency::from_scaled(0);
        Self {
            group_index: String::new(),
            mob_list: String::new(),
            mobs: z,
            max_regen: z,
            avg_exp: z,
            avg_dmg: z,
            accy_majority: 0,
            accy_max: 0,
            avg_hp: 0,
            avg_ac: 0,
            avg_dr: 0,
            avg_mr: 0,
            avg_rcol: 0,
            avg_rfir: 0,
            avg_rsto: 0,
            avg_rlit: 0,
            avg_rwat: 0,
            avg_dodge: 0,
            avg_bs_defense: 0,
            total_lairs: 0,
            avg_walk: z,
            avg_delay: 0.0,
            damage_mitigated: 0,
            damage_out: 0,
            first_round_damage_out: 0,
            min_round_damage_out: 0,
            surprise_damage_out: 0,
            surprise_min_damage_out: 0,
            surprise_chance: 0,
            poss_spawns: 0,
            global_attack_config: String::new(),
            avg_dmg_lair: z,
            avg_dmg_clear_round: z,
            avg_dmg_clear: z,
            rtk: 0.0,
            rtc: 0.0,
            magic_lvl: 0,
            max_magic_lvl: 0,
            spell_immu_lvl: 0,
            max_spell_immu_lvl: 0,
            num_undeads: 0,
            num_anti_magic: 0,
            num_animals: 0,
            num_living: 0,
        }
    }
}

/// What `GetLairAveragesFromLocs` asks `GetDamageOutput` about an average lair: the monster number
/// is 0 and the party percentage is 100; everything else is left at its default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvgDefence {
    pub ac: i16,
    pub dr: i16,
    pub mr: i16,
    pub dodge: i16,
    /// `eDefenseFlags` (`DF_*` in `combat`).
    pub flags: i32,
    pub spell_immu_lvl: i16,
    pub magic_lvl: i16,
}

/// The two callees these procedures leave to other cards.
pub trait LairEnv {
    /// `GetLairInfo(group_index, max_regen)`: the cached lair for a group index like `7-8-9`. A group
    /// MME does not know comes back as a default [`LairInfo`] (no mobs).
    fn get_lair_info(&mut self, group_index: &str, max_regen: i16) -> LairInfo;
    /// `GetDamageOutput(0, ac, dr, mr, dodge, flags, 100, spell_immu_lvl, magic_lvl)`.
    fn get_damage_output(&mut self, q: &AvgDefence) -> DamageOutput;
}

/// The globals `GetLairAveragesFromLocs` reads besides its argument.
#[derive(Debug, Clone, Copy)]
pub struct LairLocsInput<'a> {
    /// `nNMRVer`.
    pub nmr_ver: f64,
    /// `tabMonsters.Fields("Summoned By")` of the current monster (counted for `nPossSpawns`).
    pub summoned_by: &'a str,
    /// `sGlobalAttackConfig`.
    pub global_attack_config: &'a str,
}

// ---------------------------------------------------------------------------------------------
// Parsing the lair entries

/// Which spelling of a lair entry to look for: the regex MME builds depends on `nNMRVer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LairPattern {
    /// `Group(lair): n/m` (data before 1.82). Submatches: n, m.
    Plain,
    /// `[regen]Group(lair): n/m` (1.82). Submatches: regen, n, m.
    Regen,
    /// `[a-b-c][regen]Group(lair): n/m` (1.83 on). Submatches: group index, regen, n, m.
    Grouped,
}

fn run_of(b: &[u8], mut i: usize, ok: impl Fn(u8) -> bool) -> usize {
    while i < b.len() && ok(b[i]) {
        i += 1;
    }
    i
}

/// One match of the pattern starting exactly at `start`: the end and the submatch ranges.
fn lair_at(b: &[u8], start: usize, p: LairPattern) -> Option<(usize, Vec<(usize, usize)>)> {
    let mut subs = Vec::with_capacity(4);
    let mut i = start;
    let digit = |c: u8| c.is_ascii_digit();
    let bracketed = |i: usize, subs: &mut Vec<(usize, usize)>, ok: &dyn Fn(u8) -> bool| {
        if b.get(i) != Some(&b'[') {
            return None;
        }
        let end = run_of(b, i + 1, ok);
        if end == i + 1 || b.get(end) != Some(&b']') {
            return None;
        }
        subs.push((i + 1, end));
        Some(end + 1)
    };
    if p == LairPattern::Grouped {
        i = bracketed(i, &mut subs, &|c| c.is_ascii_digit() || c == b'-')?;
    }
    if p != LairPattern::Plain {
        i = bracketed(i, &mut subs, &digit)?;
    }
    let lit = b"Group(lair): ";
    if !b[i..].starts_with(lit) {
        return None;
    }
    i += lit.len();
    let end = run_of(b, i, digit);
    if end == i || b.get(end) != Some(&b'/') {
        return None;
    }
    subs.push((i, end));
    i = end + 1;
    let end = run_of(b, i, digit);
    if end == i {
        return None;
    }
    subs.push((i, end));
    Some((end, subs))
}

/// `RegExpFindv2(text, pattern)` for MME's lair patterns: every non-overlapping match, left to
/// right, each as its submatch texts. (ASCII digits only, as VBScript's `\d`.)
#[must_use]
pub fn find_lairs(text: &str, p: LairPattern) -> Vec<Vec<&str>> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if let Some((end, subs)) = lair_at(b, i, p) {
            out.push(subs.iter().map(|&(s, e)| &text[s..e]).collect());
            i = end;
        } else {
            i += 1;
        }
    }
    out
}

/// Port of modMain `InstrCount`: how many times `find` occurs, as `UBound(Split(s, find))`. The
/// split of an empty string has no parts, so its upper bound is -1.
#[must_use]
pub fn instr_count(s: &str, find: &str) -> i32 {
    if find.is_empty() {
        return 0;
    }
    if s.is_empty() {
        return -1;
    }
    i32::try_from(s.matches(find).count()).unwrap_or(i32::MAX)
}

/// Port of modSyntaxsFunc `AutoAppend` (the glue is always "," here).
fn auto_append(base: &str, add: &str, glue: &str) -> String {
    if base.is_empty() {
        add.to_string()
    } else if add.trim_matches(' ').is_empty() {
        base.to_string()
    } else {
        format!("{base}{glue}{add}")
    }
}

/// Port of modSyntaxsFunc `RemoveDuplicateNumbersFromString`: the comma-separated parts, trimmed,
/// empty ones dropped, first occurrence of each kept. Without a comma it is `CStr(Val(s))`.
#[must_use]
pub fn remove_duplicate_numbers_from_string(input: &str) -> String {
    if input.chars().count() < 2 || !input.contains(',') {
        return vb::cstr(vb::val(input));
    }
    let mut seen: Vec<&str> = Vec::new();
    for part in input.split(',') {
        let part = part.trim_matches(' ');
        if !part.is_empty() && !seen.contains(&part) {
            seen.push(part);
        }
    }
    if seen.is_empty() {
        "0".to_string()
    } else {
        seen.join(",")
    }
}

// ---------------------------------------------------------------------------------------------
// Count dictionary (DICT_*)

/// A `Scripting.Dictionary` of Long keys to Long counts, in insertion order (MME's `dictMagicLvlCounts`,
/// `dictAccyMajCounts`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CountDict {
    pub items: Vec<(i32, i32)>,
}

impl CountDict {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Port of `DICT_BumpCount`: count one more for `level`.
    ///
    /// # Errors
    /// Overflow (6) when the count passes the Long range; MME has no handler there.
    pub fn bump_count(&mut self, level: i32) -> Res<()> {
        self.add_to_count(level, 1)
    }

    /// Port of `DICT_AddToCount`: add `delta` to the count for `key` (a zero delta does nothing).
    ///
    /// # Errors
    /// Overflow (6) when the count passes the Long range.
    pub fn add_to_count(&mut self, key: i32, delta: i32) -> Res<()> {
        if delta == 0 {
            return Ok(());
        }
        if let Some(item) = self.items.iter_mut().find(|(k, _)| *k == key) {
            item.1 = vb::add_i32(item.1, delta)?;
        } else {
            self.items.push((key, delta));
        }
        Ok(())
    }

    /// Port of `DICT_ModeFromCounts`: the key with the highest count, the higher key on a tie;
    /// `default_level` for an empty dictionary.
    #[must_use]
    pub fn mode(&self, default_level: i32) -> i32 {
        let mut best_level = default_level;
        let mut best_count = -1;
        for &(level, count) in &self.items {
            if count > best_count {
                best_count = count;
                best_level = level;
            } else if count == best_count && level > best_level {
                best_level = level;
            }
        }
        best_level
    }

    fn count_of(&self, key: i32) -> i32 {
        self.items
            .iter()
            .find(|(k, _)| *k == key)
            .map_or(0, |&(_, c)| c)
    }

    fn total(&self) -> i32 {
        self.items.iter().map(|&(_, c)| c).sum()
    }
}

// ---------------------------------------------------------------------------------------------
// Sorting, median, spread

fn at(a: &[f64], i: i64) -> Option<f64> {
    usize::try_from(i).ok().and_then(|i| a.get(i).copied())
}

/// Port of modMMudDatabase `QuickSort`: Hoare partition around the middle element, sorting
/// `a[first..=last]` in place. Kept as written (not `sort_by`) so equal values end up where MME
/// puts them. An index outside the slice is VB error 9, which MME's handler turns into a return
/// from that call; so does this.
pub fn quick_sort(a: &mut [f64], first: i32, last: i32) {
    let (mut i, mut j) = (i64::from(first), i64::from(last));
    let Some(pivot) = at(a, (i + j) / 2) else {
        return;
    };
    while i <= j {
        loop {
            match at(a, i) {
                Some(x) if x < pivot => i += 1,
                Some(_) => break,
                None => return,
            }
        }
        loop {
            match at(a, j) {
                Some(x) if x > pivot => j -= 1,
                Some(_) => break,
                None => return,
            }
        }
        if i <= j {
            a.swap(i as usize, j as usize);
            i += 1;
            j -= 1;
        }
    }
    if i64::from(first) < j {
        quick_sort(a, first, j as i32);
    }
    if i < i64::from(last) {
        quick_sort(a, i as i32, last);
    }
}

/// Port of `GetMedian`: the middle value (mean of the two middle ones for an even count) of a
/// sorted copy; 0 for no values.
#[must_use]
pub fn get_median(vals: &[f64]) -> f64 {
    if vals.is_empty() {
        return 0.0;
    }
    let mut tmp = vals.to_vec();
    let last = (tmp.len() - 1) as i32;
    quick_sort(&mut tmp, 0, last);
    let n = tmp.len();
    if n % 2 == 1 {
        tmp[n / 2]
    } else {
        (tmp[n / 2 - 1] + tmp[n / 2]) / 2.0
    }
}

/// Port of `GetMedianAbsDev`: the median of the distances from `median`.
#[must_use]
pub fn get_median_abs_dev(vals: &[f64], median: f64) -> f64 {
    let devs: Vec<f64> = vals.iter().map(|v| (v - median).abs()).collect();
    get_median(&devs)
}

/// Port of `GetStdDev`: the sample standard deviation (n - 1); 0 under two values. A VB overflow
/// while squaring ends the function with its default, 0.
#[must_use]
pub fn get_std_dev(vals: &[f64]) -> f64 {
    let n = vals.len();
    if n < 2 {
        return 0.0;
    }
    let sum: f64 = vals.iter().sum();
    let mean = sum / n as f64;
    let mut sumsq = 0.0;
    for &v in vals {
        match vb::pow(v - mean, 2.0) {
            Ok(sq) => sumsq += sq,
            Err(_) => return 0.0,
        }
    }
    vb::sqr(sumsq / (n - 1) as f64).unwrap_or(0.0)
}

/// Port of `RemoveOutliers`: keeps the values within 3 median-absolute-deviations of the median (3
/// standard deviations when the deviation is 0). If nothing would be left the data is left alone.
pub fn remove_outliers(data: &mut Vec<f64>) {
    if data.is_empty() {
        return;
    }
    let med = get_median(data);
    let mut mad = get_median_abs_dev(data, med);
    #[allow(clippy::float_cmp)]
    if mad == 0.0 {
        mad = get_std_dev(data);
    }
    let cutoff = 3.0 * mad;
    let kept: Vec<f64> = data
        .iter()
        .copied()
        .filter(|v| (v - med).abs() <= cutoff)
        .collect();
    if !kept.is_empty() {
        *data = kept;
    }
}

// ---------------------------------------------------------------------------------------------
// Currency and conversion helpers

fn cur_long(v: i32) -> Currency {
    Currency::from_scaled(i64::from(v) * 10_000)
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

/// `Round(Currency, digits)`: stays Currency, rounded half to even at that many decimals.
fn cur_round(c: Currency, digits: u32) -> Currency {
    if digits >= 4 {
        return c;
    }
    let q = 10_i64.pow(4 - digits);
    Currency::from_scaled(div_half_even(c.scaled(), q) * q)
}

/// A Currency assigned to a Long: `CLng`, banker's rounding.
fn cur_to_long(c: Currency) -> Res<i32> {
    i32::try_from(div_half_even(c.scaled(), 10_000)).map_err(|_| Vb6Error::Overflow)
}

/// `Round(x, digits)` of a Double.
fn rnd(x: f64, digits: i32) -> Res<f64> {
    vb::round(x, digits)
}

/// `RoundUp` (modSyntaxsFunc): up to the next whole number unless already whole.
fn round_up(n: f64) -> f64 {
    if 0.0 < n - vb::int(n) {
        vb::int(n) + 1.0
    } else {
        vb::int(n)
    }
}

/// An Integer-typed result of `RoundUp`/`Round` (CInt, Overflow past 32767).
fn to_int(x: f64) -> Res<i16> {
    vb::cint(x)
}

// ---------------------------------------------------------------------------------------------
// CountLairsByMaxRegen

/// Port of modMMudDatabase `CountLairsByMaxRegen`: how many lair entries in `loc` have a regen
/// within `min_regen..=max_regen` (regen 0 never counts). Data older than 1.82 has no regen: 0.
/// A regen too large for a Long ends the count there (MME's handler), keeping what was counted.
#[must_use]
pub fn count_lairs_by_max_regen(loc: &str, nmr_ver: f64, min_regen: f64, max_regen: f64) -> i32 {
    if nmr_ver < 1.82 {
        return 0;
    }
    let (pattern, regen_at) = if nmr_ver >= 1.83 {
        (LairPattern::Grouped, 1)
    } else {
        (LairPattern::Regen, 0)
    };
    let mut count = 0;
    for m in find_lairs(loc, pattern) {
        let Ok(regen) = vb::clng(vb::val(m[regen_at])) else {
            break;
        };
        let regen = f64::from(regen);
        if regen > 0.0 && regen >= min_regen && regen <= max_regen {
            count += 1;
        }
    }
    count
}

// ---------------------------------------------------------------------------------------------
// GetLairAveragesFromLocs

/// Port of modMMudDatabase `GetLairAveragesFromLocs`: the average lair over every lair entry in
/// `loc`. Each entry with a regen is looked up with `env.get_lair_info`; lairs without mobs count
/// in the divisor but add nothing. Before data 1.83, or with no entry, only `poss_spawns` is set.
/// A VB error part-way (a regen that does not fit an Integer, Currency overflow) returns the
/// fields assigned so far, which at that point is `poss_spawns` alone.
pub fn get_lair_averages_from_locs(
    env: &mut dyn LairEnv,
    loc: &str,
    input: &LairLocsInput,
) -> LairInfo {
    let mut out = LairInfo::default();
    let _ = averages(env, loc, input, &mut out);
    out
}

#[allow(clippy::too_many_lines)]
fn averages(
    env: &mut dyn LairEnv,
    loc: &str,
    input: &LairLocsInput,
    out: &mut LairInfo,
) -> Res<()> {
    let zero = Currency::from_scaled(0);
    out.poss_spawns = instr_count(input.summoned_by, "Group:");
    if input.nmr_ver < 1.83 {
        return Ok(());
    }
    let matches = find_lairs(loc, LairPattern::Grouped);
    if matches.is_empty() {
        return Ok(());
    }
    let n_lairs = i32::try_from(matches.len()).map_err(|_| Vb6Error::Overflow)?;
    let mut avg_walk = vec![0.0_f64; matches.len()];

    let (mut avg_dmg, mut avg_exp, mut avg_hp, mut avg_dmg_lair) = (zero, zero, zero, zero);
    let (mut max_regen_sum, mut mitigation, mut damage_out_sum) = (zero, zero, zero);
    let (mut surprise_damage_out, mut surprise_min) = (zero, zero);
    let (mut avg_ac, mut avg_dr, mut avg_mr, mut avg_dodge) = (0_i32, 0_i32, 0_i32, 0_i32);
    let (mut rtc, mut rtk, mut avg_delay, mut avg_mobs) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    let (mut clear_round, mut clear, mut n_clear) = (0.0_f64, 0.0_f64, 0_i32);
    let (mut first_dmg_out, mut min_dmg_out, mut surprise_chance) = (0.0_f64, 0.0_f64, 0.0_f64);
    let (mut bs_defense, mut rcol, mut rfir, mut rsto, mut rlit, mut rwat) =
        (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    let (mut num_undeads, mut num_anti_magic, mut num_animals, mut num_living) =
        (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    let (mut max_magic_lvl, mut max_spell_immu_lvl) = (0_i16, 0_i16);
    let mut accy_max = 0.0_f64;
    let mut mob_list = String::new();
    let mut magic_counts = CountDict::new();
    let mut spell_counts = CountDict::new();
    let mut accy_counts = CountDict::new();

    for (i, m) in matches.iter().enumerate() {
        let group_index = m[0];
        let max_regen = Currency::from_f64(vb::val(m[1]))?;
        if max_regen.scaled() <= 0 {
            continue;
        }
        let info = env.get_lair_info(group_index, to_int(max_regen.to_f64())?);
        if info.mobs.scaled() <= 0 {
            continue;
        }
        if f64::from(info.accy_max) > accy_max {
            accy_max = f64::from(info.accy_max);
        }
        if info.accy_majority > 0 {
            accy_counts.bump_count(info.accy_majority)?;
        }
        avg_mobs += info.mobs.to_f64();
        avg_exp = avg_exp.checked_add(info.avg_exp.checked_mul(info.max_regen)?)?;
        avg_hp = avg_hp.checked_add(cur_long(info.avg_hp).checked_mul(info.max_regen)?)?;
        avg_dmg = avg_dmg.checked_add(info.avg_dmg)?;
        avg_dmg_lair = avg_dmg_lair.checked_add(info.avg_dmg_lair)?;
        clear_round += info.avg_dmg_clear_round.to_f64();
        if info.avg_dmg_clear.scaled() > 0 {
            clear += info.avg_dmg_clear.to_f64();
            n_clear += 1;
        }
        rtc += info.rtc;
        rtk += info.rtk;
        avg_ac = vb::add_i32(avg_ac, i32::from(info.avg_ac))?;
        avg_dr = vb::add_i32(avg_dr, i32::from(info.avg_dr))?;
        avg_mr = vb::add_i32(avg_mr, i32::from(info.avg_mr))?;
        rcol += f64::from(info.avg_rcol);
        rfir += f64::from(info.avg_rfir);
        rsto += f64::from(info.avg_rsto);
        rlit += f64::from(info.avg_rlit);
        rwat += f64::from(info.avg_rwat);
        avg_dodge = vb::add_i32(avg_dodge, i32::from(info.avg_dodge))?;
        damage_out_sum = damage_out_sum.checked_add(cur_long(info.damage_out))?;
        first_dmg_out += f64::from(info.first_round_damage_out);
        min_dmg_out += f64::from(info.min_round_damage_out);
        surprise_damage_out =
            surprise_damage_out.checked_add(cur_long(info.surprise_damage_out))?;
        surprise_chance += f64::from(info.surprise_chance);
        surprise_min = surprise_min.checked_add(cur_long(info.surprise_min_damage_out))?;
        mitigation = mitigation.checked_add(cur_long(info.damage_mitigated))?;
        bs_defense += f64::from(info.avg_bs_defense);

        magic_counts.bump_count(i32::from(info.magic_lvl))?;
        spell_counts.bump_count(i32::from(info.spell_immu_lvl))?;
        max_magic_lvl = max_magic_lvl.max(info.max_magic_lvl);
        max_spell_immu_lvl = max_spell_immu_lvl.max(info.max_spell_immu_lvl);

        num_undeads += f64::from(info.num_undeads);
        num_anti_magic += f64::from(info.num_anti_magic);
        num_animals += f64::from(info.num_animals);
        num_living += f64::from(info.num_living);

        max_regen_sum = max_regen_sum.checked_add(info.max_regen)?;
        avg_delay += info.avg_delay;
        avg_walk[i] = info.avg_walk.to_f64();
        mob_list = auto_append(&mob_list, &info.mob_list, ",");
    }

    // Finalize the averages
    let nl = cur_long(n_lairs);
    let nl_f = f64::from(n_lairs);
    out.avg_dmg = cur_round(avg_dmg.checked_div(nl)?, 0);
    out.avg_dmg_lair = cur_round(avg_dmg_lair.checked_div(nl)?, 0);
    out.avg_dmg_clear_round = Currency::from_f64(rnd(clear_round / nl_f, 0)?)?;
    if n_clear > 0 {
        out.avg_dmg_clear = Currency::from_f64(rnd(clear / f64::from(n_clear), 0)?)?;
    }
    out.rtc = rnd(rtc / nl_f, 1)?;
    out.rtk = rnd(rtk / nl_f, 1)?;
    out.avg_exp = cur_round(avg_exp.checked_div(nl)?, 0);
    out.avg_hp = cur_to_long(cur_round(avg_hp.checked_div(nl)?, 0))?;
    out.avg_ac = to_int(rnd(f64::from(avg_ac) / nl_f, 0)?)?;
    out.avg_dr = to_int(rnd(f64::from(avg_dr) / nl_f, 0)?)?;
    out.avg_mr = to_int(rnd(f64::from(avg_mr) / nl_f, 0)?)?;
    out.avg_rcol = to_int(rnd(rcol / nl_f, 0)?)?;
    out.avg_rfir = to_int(rnd(rfir / nl_f, 0)?)?;
    out.avg_rsto = to_int(rnd(rsto / nl_f, 0)?)?;
    out.avg_rlit = to_int(rnd(rlit / nl_f, 0)?)?;
    out.avg_rwat = to_int(rnd(rwat / nl_f, 0)?)?;
    out.avg_dodge = to_int(rnd(f64::from(avg_dodge) / nl_f, 0)?)?;
    out.avg_bs_defense = to_int(rnd(bs_defense / nl_f, 0)?)?;
    out.damage_mitigated = cur_to_long(cur_round(mitigation.checked_div(nl)?, 0))?;
    out.mobs = Currency::from_f64(avg_mobs / nl_f)?;
    out.max_regen = cur_round(max_regen_sum.checked_div(nl)?, 1);
    out.avg_delay = rnd(avg_delay / nl_f, 1)?;

    // Majority of the majorities: the accuracy most lairs agree on (zeros do not vote)
    let mut accy_majority = 0;
    if !accy_counts.items.is_empty() {
        let dom = accy_counts.mode(0);
        if accy_counts.count_of(dom) * 100 >= MAJ_THRESH_PCT * accy_counts.total() {
            accy_majority = dom;
        }
    }
    out.accy_majority = accy_majority;
    out.accy_max = vb::clng(accy_max)?;

    let mode_magic = if magic_counts.items.is_empty() {
        0
    } else {
        magic_counts.mode(0)
    };
    let mode_spell = if spell_counts.items.is_empty() {
        0
    } else {
        spell_counts.mode(0)
    };
    out.magic_lvl = i16::try_from(mode_magic).map_err(|_| Vb6Error::Overflow)?;
    out.max_magic_lvl = max_magic_lvl;
    out.spell_immu_lvl = i16::try_from(mode_spell).map_err(|_| Vb6Error::Overflow)?;
    out.max_spell_immu_lvl = max_spell_immu_lvl;

    out.num_undeads = to_int(round_up(num_undeads / nl_f))?;
    out.num_anti_magic = to_int(round_up(num_anti_magic / nl_f))?;
    out.num_animals = to_int(round_up(num_animals / nl_f))?;
    out.num_living = to_int(round_up(num_living / nl_f))?;

    let mut walk = avg_walk;
    remove_outliers(&mut walk);
    out.avg_walk = Currency::from_f64(rnd(calc_average_non_zero(&walk), 1)?)?;
    out.total_lairs = n_lairs;
    if out.max_regen < cur_long(1) {
        out.max_regen = cur_long(1);
    }

    out.damage_out = cur_to_long(cur_round(damage_out_sum.checked_div(nl)?, 0))?;
    out.first_round_damage_out = vb::clng(rnd(first_dmg_out / nl_f, 0)?)?;
    out.min_round_damage_out = vb::clng(rnd(min_dmg_out / nl_f, 0)?)?;
    out.surprise_damage_out = cur_to_long(cur_round(surprise_damage_out.checked_div(nl)?, 0))?;
    out.surprise_chance = to_int(rnd(surprise_chance / nl_f, 0)?)?;
    out.surprise_min_damage_out = cur_to_long(cur_round(surprise_min.checked_div(nl)?, 0))?;
    out.poss_spawns = vb::add_i32(out.poss_spawns, n_lairs)?;
    out.group_index = loc.to_string();
    out.global_attack_config = input.global_attack_config.to_string();
    out.mob_list = remove_duplicate_numbers_from_string(&mob_list);

    if out.spell_immu_lvl > 0
        || out.magic_lvl > 0
        || out.num_undeads > 0
        || out.num_anti_magic > 0
        || out.num_living > 0
        || out.num_animals > 0
    {
        let mut flags = 0;
        let half_mobs = out.mobs.checked_div(cur_long(2))?;
        if out.num_anti_magic > 0 && cur_long(i32::from(out.num_anti_magic)) >= half_mobs {
            flags |= DF_ANTI_MAGIC;
        }
        let ratio_mobs = out.mobs.to_f64() * LAIR_FLAG_RATIO;
        if out.num_undeads > 0 && f64::from(out.num_undeads) >= ratio_mobs {
            flags |= DF_UNDEAD;
        }
        if out.num_living > 0 && f64::from(out.num_living) >= ratio_mobs {
            flags |= DF_LIVING;
        }
        if out.num_animals > 0 && f64::from(out.num_animals) >= ratio_mobs {
            flags |= DF_ANIMAL;
        }
        let dmg = env.get_damage_output(&AvgDefence {
            ac: out.avg_ac,
            dr: out.avg_dr,
            mr: out.avg_mr,
            dodge: out.avg_dodge,
            flags,
            spell_immu_lvl: out.spell_immu_lvl,
            magic_lvl: out.magic_lvl,
        });
        let immune = cur_long(-9998);
        if dmg.average_damage == immune {
            out.damage_out = 0;
            out.first_round_damage_out = 0;
            out.min_round_damage_out = 0;
        }
        if dmg.surprise_damage == immune {
            out.surprise_damage_out = 0;
            out.surprise_min_damage_out = 0;
            out.surprise_chance = 0;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// CalculateAverageLairs

/// Port of modMMudDatabase `CalculateAverageLairs`: for every monster whose "Summoned By" text has
/// a lair entry, fills `possy[number]` (average regen per lair, one decimal) and, from data 1.83,
/// `spawn_chance[number]` (chance a given mob of the lair spawns, two decimals).
///
/// `monsters` are the `(Number, Summoned By)` rows in table order; those without `(lair)`
/// (any case, as the query's `Like`) are skipped. The two slices are MME's `nMonsterPossy` and
/// `nMonsterSpawnChance`. A number outside them, or a VB error, ends the procedure there (MME's
/// handler); what was filled stays.
pub fn calculate_average_lairs(
    env: &mut dyn LairEnv,
    nmr_ver: f64,
    monsters: &[(i32, &str)],
    possy: &mut [Currency],
    spawn_chance: &mut [Currency],
) {
    let _ = average_lairs(env, nmr_ver, monsters, possy, spawn_chance);
}

fn slot(v: &mut [Currency], number: i32) -> Option<&mut Currency> {
    usize::try_from(number).ok().and_then(|i| v.get_mut(i))
}

fn average_lairs(
    env: &mut dyn LairEnv,
    nmr_ver: f64,
    monsters: &[(i32, &str)],
    possy: &mut [Currency],
    spawn_chance: &mut [Currency],
) -> Res<()> {
    let pattern = if nmr_ver >= 1.83 {
        LairPattern::Grouped
    } else if nmr_ver >= 1.82 {
        LairPattern::Regen
    } else {
        LairPattern::Plain
    };
    let zero = Currency::from_scaled(0);
    for &(number, text) in monsters {
        if !text.to_ascii_lowercase().contains("(lair)") {
            continue;
        }
        let matches = find_lairs(text, pattern);
        if matches.is_empty() {
            continue;
        }
        let n_lairs = i32::try_from(matches.len()).map_err(|_| Vb6Error::Overflow)?;
        let (mut mobs_total, mut chance) = (zero, zero);
        for m in &matches {
            let max_regen;
            if nmr_ver >= 1.83 {
                max_regen = Currency::from_f64(vb::val(m[1]))?;
                if max_regen.scaled() > 0 {
                    let info = env.get_lair_info(m[0], to_int(max_regen.to_f64())?);
                    if info.mobs.scaled() > 0 {
                        // 1 - (1 - 1/mobs) ^ regen: one mob of the lair spawning in `regen` tries
                        let inv = cur_long(1).checked_div(info.mobs)?;
                        let base = cur_long(1).checked_sub(inv)?;
                        let p = vb::pow(base.to_f64(), max_regen.to_f64())?;
                        let add = rnd(1.0 - p, 2)?;
                        chance = Currency::from_f64(chance.to_f64() + add)?;
                    }
                }
            } else if nmr_ver >= 1.82 {
                max_regen = Currency::from_f64(vb::val(m[0]))?;
            } else {
                max_regen = cur_long(1);
            }
            mobs_total = mobs_total.checked_add(max_regen)?;
        }
        let nl = cur_long(n_lairs);
        if mobs_total.scaled() > 0 {
            let Some(s) = slot(possy, number) else {
                return Ok(());
            };
            *s = cur_round(mobs_total.checked_div(nl)?, 1);
        }
        if nmr_ver >= 1.83 {
            let Some(s) = slot(spawn_chance, number) else {
                return Ok(());
            };
            *s = cur_round(chance.checked_div(nl)?, 2);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// The lair cache (card P3-16)

/// MME keeps the per-group lair cache in two module globals, `colLairs` and `dictLairInfo`. This is
/// both, plus `tLastAvgLairInfo`. Slot 0 of `colLairs` is the one `ReDim colLairs(0)` leaves, which
/// no group uses. Build it with [`LairCache::load`] (`LoadLairInfo`).
#[derive(Debug, Clone, Default)]
pub struct LairCache {
    lairs: Vec<LairInfo>,
    /// `dictLairInfo`: `vbTextCompare` keys (case-insensitive), values are `colLairs` indexes.
    index: HashMap<String, usize>,
    /// `tLastAvgLairInfo`: the monster list's last average lair; the party-damage refresh resets it.
    pub last_avg_lair_info: LairInfo,
}

/// What `GetLairInfo` reads from frmMain and the module globals besides its arguments.
#[derive(Debug, Clone, Copy)]
pub struct LairQuery<'a> {
    pub profile: Profile,
    /// `bStartup`: while the app is starting, `GetLairInfo` returns the cached lair as it is.
    pub startup: bool,
    /// `frmMain.chkGlobalFilter.Value = 1`.
    pub use_character: bool,
    /// `optMonsterFilter(1)` and `txtMonsterLairFilter(0)`: the party size.
    pub party: &'a PartyFilter,
    /// `sGlobalAttackConfig`.
    pub global_attack_config: &'a str,
}

/// What `GetLairInfo` asks `GetDamageOutput` about an average lair: `GetDamageOutput(0, ac, dr, mr,
/// dodge, flags, 100, spell_immu_lvl, magic_lvl, , bs_defense, rcol, rfir, rsto, rlit, rwat)`.
/// (`AvgDefence` is the shorter question `GetLairAveragesFromLocs` asks.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LairDefence {
    pub ac: i16,
    pub dr: i16,
    pub mr: i16,
    pub dodge: i16,
    /// `eDefenseFlags` (`DF_*` in `combat`).
    pub flags: i32,
    pub spell_immu_lvl: i16,
    pub magic_lvl: i16,
    pub bs_defense: i16,
    pub rcol: i16,
    pub rfir: i16,
    pub rsto: i16,
    pub rlit: i16,
    pub rwat: i16,
}

/// The outside procedures `GetLairInfo` calls. `CalcCombatRounds` is called directly.
pub trait LairCallouts {
    /// `GetDamageOutput` against an average lair.
    fn damage_output(&mut self, q: &LairDefence) -> DamageOutput;
    /// `GetPreCalculatedMonsterDamage(number, label, n_party)`: the value only.
    fn monster_damage(&mut self, number: i32, n_party: i16) -> f64;
    /// `UBound(nMonsterDamageVsChar())`; `None` when MME never dimensioned the array (error 9).
    fn damage_array_bound(&self) -> Option<i32>;
}

fn to_i16(x: i32) -> Res<i16> {
    i16::try_from(x).map_err(|_| Vb6Error::Overflow)
}

impl LairCache {
    /// An empty cache: only the unused slot 0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            lairs: vec![LairInfo::default()],
            index: HashMap::new(),
            last_avg_lair_info: LairInfo::default(),
        }
    }

    /// The stored lair of a group, by its `colLairs` slot (0 and unknown slots read as `None`).
    #[must_use]
    pub fn stored(&self, group_index: &str) -> Option<&LairInfo> {
        self.index
            .get(&group_index.to_lowercase())
            .map(|&i| &self.lairs[i])
    }

    /// Port of modMMudDatabase `GetLairInfoIndex`: the slot of a group index, appending a new
    /// (empty) slot for one not seen before. The empty text has no slot: 0.
    pub fn get_lair_info_index(&mut self, group_index: &str) -> i32 {
        if group_index.is_empty() {
            return 0;
        }
        if self.lairs.is_empty() {
            self.lairs.push(LairInfo::default());
        }
        let key = group_index.to_lowercase();
        let slot = if let Some(&i) = self.index.get(&key) {
            i
        } else {
            let i = self.lairs.len();
            self.lairs.push(LairInfo {
                group_index: group_index.to_string(),
                ..LairInfo::default()
            });
            self.index.insert(key, i);
            i
        };
        i32::try_from(slot).unwrap_or(i32::MAX)
    }

    fn slot(&mut self, group_index: &str) -> usize {
        usize::try_from(self.get_lair_info_index(group_index)).unwrap_or(0)
    }

    /// Port of modMMudDatabase `SetLairInfo`: stores the lair under its group index. The damage
    /// fields and the attack config are stored only when the update carries a config; the max
    /// regen only when positive. A group index shorter than 5 characters is ignored.
    pub fn set_lair_info(&mut self, u: &LairInfo) {
        if u.group_index.chars().count() < 5 {
            return;
        }
        let x = self.slot(&u.group_index);
        let c = &mut self.lairs[x];
        c.mob_list.clone_from(&u.mob_list);
        c.mobs = u.mobs;
        c.avg_exp = u.avg_exp;
        c.avg_dmg = u.avg_dmg;
        c.avg_hp = u.avg_hp;
        c.avg_ac = u.avg_ac;
        c.avg_dr = u.avg_dr;
        c.avg_mr = u.avg_mr;
        c.avg_dodge = u.avg_dodge;
        c.avg_delay = u.avg_delay;
        c.avg_walk = u.avg_walk;
        c.total_lairs = u.total_lairs;
        c.magic_lvl = u.magic_lvl;
        c.max_magic_lvl = u.max_magic_lvl;
        c.spell_immu_lvl = u.spell_immu_lvl;
        c.max_spell_immu_lvl = u.max_spell_immu_lvl;
        c.num_undeads = u.num_undeads;
        c.num_anti_magic = u.num_anti_magic;
        c.num_animals = u.num_animals;
        c.num_living = u.num_living;
        c.avg_bs_defense = u.avg_bs_defense;
        c.avg_rcol = u.avg_rcol;
        c.avg_rfir = u.avg_rfir;
        c.avg_rsto = u.avg_rsto;
        c.avg_rlit = u.avg_rlit;
        c.avg_rwat = u.avg_rwat;
        c.accy_majority = u.accy_majority;
        c.accy_max = u.accy_max;
        if !u.global_attack_config.is_empty() {
            c.damage_out = u.damage_out;
            c.first_round_damage_out = u.first_round_damage_out;
            c.min_round_damage_out = u.min_round_damage_out;
            c.surprise_chance = u.surprise_chance;
            c.surprise_damage_out = u.surprise_damage_out;
            c.surprise_min_damage_out = u.surprise_min_damage_out;
            c.global_attack_config.clone_from(&u.global_attack_config);
        }
        if u.max_regen.scaled() > 0 {
            c.max_regen = u.max_regen;
        }
    }

    /// Port of modMMudDatabase `LoadLairInfo`: builds the cache from the `Lairs` table, reading each
    /// lair's monsters from `Monsters` for the trait counts, resist and defence averages and the
    /// melee accuracy majority. Stock only (the GreaterMUD delay offset is not ported).
    ///
    /// A VB error (a value that does not fit its field, an average over a mob count that rounds to
    /// zero) ends the load there, as MME's handler does: the lairs before it stay.
    #[must_use]
    pub fn load(world: &World, profile: Profile, nmr_ver: f64) -> Self {
        let Profile::Stock = profile;
        let mut cache = Self::new();
        let mut carry = AccuracyCarry::default();
        for lair in &world.lairs {
            if load_one(&mut cache, world, nmr_ver, lair, &mut carry).is_err() {
                break;
            }
        }
        cache
    }

    /// Port of modMMudDatabase `GetLairInfo`: the cached lair of `group_index` for a given max regen,
    /// with what the loaded character (or the lair-mode party) does to it and it to them: damage per
    /// round, rounds to kill (`rtk`) and to clear (`rtc`), damage taken. `max_regen` 0 (the
    /// argument left out) is read from the fourth part of the index. The damage is stored back
    /// in the cache for a party of one, and reused while `sGlobalAttackConfig` stays the same.
    ///
    /// A group index of fewer than 5 characters, or without a regen, gives an empty lair. A VB error
    /// part-way returns the fields assigned so far, as MME's handler does.
    pub fn get_lair_info(
        &mut self,
        q: &LairQuery,
        co: &mut dyn LairCallouts,
        group_index: &str,
        max_regen: i16,
    ) -> LairInfo {
        let mut out = LairInfo::default();
        let _ = self.lair_info_steps(q, co, group_index, max_regen, &mut out);
        out
    }

    #[allow(clippy::too_many_lines)]
    fn lair_info_steps(
        &mut self,
        q: &LairQuery,
        co: &mut dyn LairCallouts,
        group_index: &str,
        mut max_regen: i16,
        out: &mut LairInfo,
    ) -> Res<()> {
        let Profile::Stock = q.profile;
        if group_index.chars().count() < 5 {
            return Ok(());
        }
        if max_regen == 0 {
            let parts: Vec<&str> = group_index.split('-').collect();
            if parts.len() < 4 {
                return Ok(());
            }
            max_regen = vb::cint(vb::val(parts[3]))?;
        }
        let x = self.slot(group_index);
        let c = &self.lairs[x];
        out.group_index.clone_from(&c.group_index);
        out.mob_list.clone_from(&c.mob_list);
        out.mobs = c.mobs;
        out.avg_exp = c.avg_exp;
        out.avg_dmg = c.avg_dmg;
        out.avg_hp = c.avg_hp;
        out.avg_ac = c.avg_ac;
        out.avg_dr = c.avg_dr;
        out.avg_mr = c.avg_mr;
        out.avg_dodge = c.avg_dodge;
        out.damage_out = c.damage_out;
        out.first_round_damage_out = c.first_round_damage_out;
        out.min_round_damage_out = c.min_round_damage_out;
        out.surprise_damage_out = c.surprise_damage_out;
        out.surprise_min_damage_out = c.surprise_min_damage_out;
        out.surprise_chance = c.surprise_chance;
        out.global_attack_config.clone_from(&c.global_attack_config);
        out.max_regen = cur_long(i32::from(max_regen));
        out.avg_delay = c.avg_delay;
        out.avg_walk = c.avg_walk;
        out.total_lairs = c.total_lairs;
        out.magic_lvl = c.magic_lvl;
        out.max_magic_lvl = c.max_magic_lvl;
        out.spell_immu_lvl = c.spell_immu_lvl;
        out.max_spell_immu_lvl = c.max_spell_immu_lvl;
        out.num_undeads = c.num_undeads;
        out.num_anti_magic = c.num_anti_magic;
        out.num_animals = c.num_animals;
        out.num_living = c.num_living;
        out.avg_bs_defense = c.avg_bs_defense;
        out.avg_rcol = c.avg_rcol;
        out.avg_rfir = c.avg_rfir;
        out.avg_rsto = c.avg_rsto;
        out.avg_rlit = c.avg_rlit;
        out.avg_rwat = c.avg_rwat;
        out.accy_majority = c.accy_majority;
        out.accy_max = c.accy_max;
        out.rtk = 1.0;
        out.rtc = f64::from(max_regen);
        out.damage_mitigated = 0;

        if out.mob_list.is_empty() || q.startup {
            return Ok(());
        }
        // From here MME's locals: -9999 means "no figure".
        let mut n_party: i16 = 1;
        if q.party.lair_mode {
            n_party = vb::cint(vb::val(&q.party.lair_text[0]))?;
        }
        n_party = n_party.clamp(1, 6);

        let (mut damage_out, mut first_round, mut surprise);
        if n_party == 1
            && out.global_attack_config.chars().count() > 1
            && out.global_attack_config == q.global_attack_config
        {
            // the cached figures were copied into `out` above
            damage_out = out.damage_out;
            first_round = out.first_round_damage_out;
            surprise = out.surprise_damage_out;
        } else {
            let mut flags = 0;
            let half_mobs = out.mobs.checked_div(cur_long(2))?;
            if out.num_anti_magic > 0 && cur_long(i32::from(out.num_anti_magic)) >= half_mobs {
                flags |= DF_ANTI_MAGIC;
            }
            let ratio_mobs = out.mobs.to_f64() * LAIR_FLAG_RATIO;
            if out.num_undeads > 0 && f64::from(out.num_undeads) >= ratio_mobs {
                flags |= DF_UNDEAD;
            }
            if out.num_living > 0 && f64::from(out.num_living) >= ratio_mobs {
                flags |= DF_LIVING;
            }
            if out.num_animals > 0 && f64::from(out.num_animals) >= ratio_mobs {
                flags |= DF_ANIMAL;
            }
            let d = co.damage_output(&LairDefence {
                ac: out.avg_ac,
                dr: out.avg_dr,
                mr: out.avg_mr,
                dodge: out.avg_dodge,
                flags,
                spell_immu_lvl: out.spell_immu_lvl,
                magic_lvl: out.magic_lvl,
                bs_defense: out.avg_bs_defense,
                rcol: out.avg_rcol,
                rfir: out.avg_rfir,
                rsto: out.avg_rsto,
                rlit: out.avg_rlit,
                rwat: out.avg_rwat,
            });
            damage_out = cur_to_long(d.average_damage)?;
            first_round = cur_to_long(d.first_round_damage)?;
            surprise = cur_to_long(d.surprise_damage)?;
            let surprise_min = cur_to_long(d.surprise_min_damage)?;
            let min_round = cur_to_long(d.min_round_damage)?;
            if damage_out > -9999 || surprise > -9999 {
                if damage_out > -9990 {
                    out.damage_out = damage_out;
                    out.first_round_damage_out = first_round;
                    out.min_round_damage_out = min_round;
                } else {
                    out.damage_out = 0;
                    out.first_round_damage_out = 0;
                    out.min_round_damage_out = 0;
                }
                if surprise > -9990 {
                    out.surprise_damage_out = surprise;
                    out.surprise_chance = d.surprise_damage_chance;
                    out.surprise_min_damage_out = surprise_min;
                } else {
                    out.surprise_damage_out = 0;
                    out.surprise_chance = 0;
                    out.surprise_min_damage_out = 0;
                }
                if n_party == 1 {
                    out.global_attack_config = q.global_attack_config.to_string();
                    self.set_lair_info(out);
                }
            } else {
                damage_out = 9_999_999;
            }
        }
        if damage_out <= -9990 {
            damage_out = 0;
        }
        if first_round <= -9990 {
            first_round = 0;
        }
        if surprise <= -9990 {
            surprise = 0;
        }

        let vs_others = q.use_character || n_party > 1;
        if vs_others {
            let mobs: Vec<&str> = out.mob_list.split(',').collect();
            let bound = co
                .damage_array_bound()
                .ok_or(Vb6Error::InvalidProcedureCall)?;
            for m in &mobs {
                let n = vb::val(m);
                if n <= f64::from(bound) {
                    let d = co.monster_damage(vb::clng(n)?, n_party);
                    out.damage_mitigated = vb::clng(f64::from(out.damage_mitigated) + d)?;
                }
            }
            let count = i32::try_from(mobs.len()).map_err(|_| Vb6Error::Overflow)?;
            out.damage_mitigated =
                vb::clng(rnd(f64::from(out.damage_mitigated) / f64::from(count), 1)?)?;
        }
        if vs_others && out.avg_dmg.scaled() > 0 && cur_long(out.damage_mitigated) != out.avg_dmg {
            out.damage_mitigated =
                cur_to_long(out.avg_dmg.checked_sub(cur_long(out.damage_mitigated))?)?;
            out.avg_dmg = out.avg_dmg.checked_sub(cur_long(out.damage_mitigated))?;
        } else {
            out.damage_mitigated = 0;
        }
        out.avg_dmg_lair = out.avg_dmg;

        let mut rtk;
        if vb::add_i32(damage_out, surprise)? > 0 {
            let info = calc_combat_rounds(
                q.profile,
                &CombatRoundsArgs {
                    damage_out,
                    mob_health: out.avg_hp,
                    mob_damage: cur_to_long(out.avg_dmg_lair)?,
                    num_mobs: 1.0,
                    surprise_damage_out: f64::from(surprise),
                    first_round_damage_out: first_round,
                    ..CombatRoundsArgs::default()
                },
            );
            rtk = info.rtk;
            if rtk > 0.0 && rtk < 1.0 {
                rtk = 1.0;
            }
            out.rtk = rtk;
            if rtk > 1.0 {
                out.avg_dmg_lair = Currency::from_f64(rnd(out.avg_dmg_lair.to_f64() * rtk, 1)?)?;
            }
        } else {
            rtk = 0.0;
            out.rtk = rtk;
        }

        if out.max_regen > cur_long(1) && out.avg_dmg_lair.scaled() > 0 {
            // (regen + 1) / (2 * regen) is a Currency quotient, so it keeps four decimals
            let alive = out
                .max_regen
                .checked_add(cur_long(1))?
                .checked_div(cur_long(2).checked_mul(out.max_regen)?)?
                .to_f64();
            out.avg_dmg_lair =
                Currency::from_f64(rnd(vb::div(out.avg_dmg_lair.to_f64(), alive)?, 1)?)?;
        }

        if vb::add_i32(damage_out, surprise)? > 0 {
            let rtc = if out.max_regen > cur_long(1) {
                rtk * out.max_regen.to_f64()
            } else {
                rtk
            };
            out.rtc = rtc;
        } else {
            out.rtc = 0.0;
        }

        // display-only damage taken over a full clear
        let mut mob_count = out.max_regen.to_f64();
        if mob_count < 1.0 {
            mob_count = 1.0;
        }
        let dmg = out.avg_dmg.to_f64();
        if out.rtc > 0.0 {
            out.avg_dmg_clear_round = Currency::from_f64(rnd(dmg * (mob_count + 1.0) / 2.0, 1)?)?;
            out.avg_dmg_clear =
                Currency::from_f64(rnd(dmg * out.rtk * mob_count * (mob_count + 1.0) / 2.0, 1)?)?;
        } else {
            out.avg_dmg_clear_round = Currency::from_f64(rnd(dmg * mob_count, 1)?)?;
        }
        Ok(())
    }
}

/// The variables `LoadLairInfo` declares inside its loop (`Dim` is not re-run per iteration): a lair
/// without a positive mob count does not assign them, so it sees the previous lair's values.
#[derive(Debug, Clone, Copy, Default)]
struct AccuracyCarry {
    dom_acc_lair: i32,
    has_majority: bool,
}

/// One lair of `LoadLairInfo`; an error is MME's handler ending the load.
#[allow(clippy::too_many_lines)]
fn load_one(
    cache: &mut LairCache,
    world: &World,
    nmr_ver: f64,
    lair: &Lair,
    carry: &mut AccuracyCarry,
) -> Res<()> {
    let mut t = LairInfo {
        group_index: lair.group_index.clone(),
        ..LairInfo::default()
    };
    t.mob_list.clone_from(&lair.mob_list);
    t.mobs = cur_long(lair.mobs);
    t.avg_delay = f64::from(lair.avg_delay);
    t.avg_exp = Currency::from_f64(lair.avg_exp)?;
    t.avg_dmg = Currency::from_f64(lair.avg_dmg)?;
    t.avg_hp = vb::clng(lair.avg_hp)?;
    t.avg_ac = to_i16(lair.avg_ac)?;
    t.avg_dr = to_i16(lair.avg_dr)?;
    t.avg_mr = to_i16(lair.avg_mr)?;
    t.avg_dodge = to_i16(lair.avg_dodge)?;
    t.avg_walk = Currency::from_f64(lair.avg_walk)?;
    t.total_lairs = lair.total_lairs;

    let mut accy = CountDict::new();
    let mut melee_total_pct = 0_i32;
    let mut max_acc_lair = 0_i32;
    let (mut num_undeads, mut num_anti_magic, mut num_animals, mut num_living) = (0_i32, 0, 0, 0);
    let (mut bs_defense, mut max_magic, mut max_immu) = (0_i32, 0_i32, 0_i32);
    let (mut rcol, mut rfir, mut rsto, mut rlit, mut rwat) = (0_i32, 0_i32, 0_i32, 0_i32, 0_i32);
    let (mut magic_lvl, mut spell_immu_lvl) = (0_i32, 0_i32);

    if t.mobs.scaled() > 0 && !world.monsters.is_empty() {
        let mut magic_counts = CountDict::new();
        let mut spell_counts = CountDict::new();
        for part in t.mob_list.split(',') {
            let n = vb::clng(vb::val(part))?;
            if n <= 0 {
                continue;
            }
            let Some(m) = world.get_monster(n) else {
                continue;
            };
            load_monster(
                m,
                nmr_ver,
                &mut MonsterTally {
                    num_undeads: &mut num_undeads,
                    num_anti_magic: &mut num_anti_magic,
                    num_animals: &mut num_animals,
                    num_living: &mut num_living,
                    bs_defense: &mut bs_defense,
                    max_magic: &mut max_magic,
                    max_immu: &mut max_immu,
                    resists: [&mut rcol, &mut rfir, &mut rsto, &mut rlit, &mut rwat],
                    magic_counts: &mut magic_counts,
                    spell_counts: &mut spell_counts,
                    accy: &mut accy,
                    melee_total_pct: &mut melee_total_pct,
                    max_acc_lair: &mut max_acc_lair,
                },
            )?;
        }
        magic_lvl = magic_counts.mode(0);
        spell_immu_lvl = spell_counts.mode(0);

        // `\` rounds the mob count to a Long: a count under one half is a division by zero
        let mobs = t.mobs.to_f64();
        for v in [
            &mut bs_defense,
            &mut rcol,
            &mut rfir,
            &mut rsto,
            &mut rlit,
            &mut rwat,
        ] {
            if *v != 0 {
                *v = vb::idiv(f64::from(*v), mobs)?;
            }
        }

        carry.dom_acc_lair = 0;
        let mut dom_pct_lair = 0_i32;
        carry.has_majority = false;
        if !accy.items.is_empty() {
            carry.dom_acc_lair = accy.mode(0);
            dom_pct_lair = accy.count_of(carry.dom_acc_lair);
        }
        let sum_pct = if melee_total_pct < 1 {
            100
        } else {
            melee_total_pct
        };
        if i64::from(dom_pct_lair) * 100 >= i64::from(MAJ_THRESH_PCT) * i64::from(sum_pct) {
            carry.has_majority = true;
        }
    }

    t.num_undeads = to_i16(num_undeads)?;
    t.num_anti_magic = to_i16(num_anti_magic)?;
    t.num_animals = to_i16(num_animals)?;
    t.num_living = to_i16(num_living)?;
    t.magic_lvl = to_i16(magic_lvl)?;
    t.max_magic_lvl = to_i16(max_magic)?;
    t.spell_immu_lvl = to_i16(spell_immu_lvl)?;
    t.max_spell_immu_lvl = to_i16(max_immu)?;
    t.avg_bs_defense = to_i16(bs_defense)?;
    t.avg_rcol = to_i16(rcol)?;
    t.avg_rfir = to_i16(rfir)?;
    t.avg_rsto = to_i16(rsto)?;
    t.avg_rlit = to_i16(rlit)?;
    t.avg_rwat = to_i16(rwat)?;
    // both branches of MME's majority test end up with the dominant accuracy when any melee was pooled
    t.accy_majority = if carry.has_majority || !accy.items.is_empty() {
        carry.dom_acc_lair
    } else {
        0
    };
    t.accy_max = max_acc_lair;
    cache.set_lair_info(&t);
    Ok(())
}

/// The accumulators one monster adds to (`LoadLairInfo`'s per-lair variables).
struct MonsterTally<'a> {
    num_undeads: &'a mut i32,
    num_anti_magic: &'a mut i32,
    num_animals: &'a mut i32,
    num_living: &'a mut i32,
    bs_defense: &'a mut i32,
    max_magic: &'a mut i32,
    max_immu: &'a mut i32,
    /// Cold, fire, stone, lightning, water.
    resists: [&'a mut i32; 5],
    magic_counts: &'a mut CountDict,
    spell_counts: &'a mut CountDict,
    accy: &'a mut CountDict,
    melee_total_pct: &'a mut i32,
    max_acc_lair: &'a mut i32,
}

fn load_monster(m: &Monster, nmr_ver: f64, t: &mut MonsterTally) -> Res<()> {
    if m.undead == 1 {
        *t.num_undeads = vb::add_i32(*t.num_undeads, 1)?;
    }
    *t.bs_defense = vb::add_i32(*t.bs_defense, m.bs_defense)?;
    let (mut had_magic, mut had_immu) = (false, false);
    let (mut is_living, mut is_animal) = (true, false);
    for slot in &m.abilities {
        if slot.ability == 0 {
            continue;
        }
        match slot.ability {
            3 => *t.resists[0] = vb::add_i32(*t.resists[0], slot.value)?,
            5 => *t.resists[1] = vb::add_i32(*t.resists[1], slot.value)?,
            65 => *t.resists[2] = vb::add_i32(*t.resists[2], slot.value)?,
            66 => *t.resists[3] = vb::add_i32(*t.resists[3], slot.value)?,
            147 => *t.resists[4] = vb::add_i32(*t.resists[4], slot.value)?,
            28 => {
                had_magic = true;
                t.magic_counts.bump_count(slot.value)?;
                *t.max_magic = (*t.max_magic).max(slot.value);
            }
            51 => *t.num_anti_magic = vb::add_i32(*t.num_anti_magic, 1)?,
            78 => is_animal = true,
            109 => is_living = false,
            139 => {
                had_immu = true;
                t.spell_counts.bump_count(slot.value)?;
                *t.max_immu = (*t.max_immu).max(slot.value);
            }
            _ => {}
        }
    }
    if is_animal {
        *t.num_animals = vb::add_i32(*t.num_animals, 1)?;
    }
    if is_living {
        *t.num_living = vb::add_i32(*t.num_living, 1)?;
    }
    if !had_magic {
        t.magic_counts.bump_count(0)?;
    }
    if !had_immu {
        t.spell_counts.bump_count(0)?;
    }

    // Melee accuracy distribution: this monster's buckets, then pooled into the lair's
    let mut prev_cum = 0_i32;
    let mut uniq: Vec<(i32, i32)> = Vec::with_capacity(5);
    let mut max_acc = 0_i32;
    for a in &m.attacks {
        if !(1..=3).contains(&a.att_type) {
            continue;
        }
        let percent = if nmr_ver >= 1.8 {
            vb::clng(vb::round(a.true_pct, 0)?)?
        } else {
            let cum = a.pct;
            let p = vb::sub_i32(cum, prev_cum)?;
            prev_cum = cum;
            p
        }
        .max(0);
        if percent == 0 {
            continue;
        }
        // attack types 1 and 3 are melee; 2 (spell) is skipped after its percent was read
        if a.att_type == 1 || a.att_type == 3 {
            if let Some(e) = uniq.iter_mut().find(|e| e.0 == a.acc) {
                e.1 = vb::add_i32(e.1, percent)?;
            } else {
                uniq.push((a.acc, percent));
            }
            max_acc = max_acc.max(a.acc);
        }
    }
    let mut monster_pct = 0_i32;
    for &(acc, pct) in &uniq {
        if pct > 0 {
            t.accy.add_to_count(acc, pct)?;
            monster_pct = vb::add_i32(monster_pct, pct)?;
        }
    }
    *t.melee_total_pct = vb::add_i32(*t.melee_total_pct, monster_pct)?;
    *t.max_acc_lair = (*t.max_acc_lair).max(max_acc);
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// The real LairEnv

/// The callees `GetLairInfo` needs, over a loaded character (or party): `GetDamageOutput` from
/// [`combat`](crate::combat), the monster-damage caches through [`monster_glue`](crate::monster_glue).
pub struct CharCallouts<'a> {
    pub world: &'a World,
    /// Everything `GetDamageOutput` reads: the character, tables and party controls.
    pub damage: DamageEnv<'a>,
    /// `nMonsterDamageVsParty`, `nMonsterDamageVsChar`, `nMonsterDamageVsDefault`, indexed by
    /// monster number; -1 is "not calculated". Empty: MME never dimensioned the array.
    pub vs_party: &'a [Currency],
    pub vs_char: &'a [Currency],
    pub vs_default: &'a [Currency],
    /// `nGlobalMonsterSimRounds`.
    pub sim_rounds: i32,
    /// MME's one global attack simulator, and the generator its runs roll with.
    pub sim: &'a mut MonsterSimInput,
    pub rnd: &'a mut Rnd,
}

impl LairCallouts for CharCallouts<'_> {
    fn damage_output(&mut self, q: &LairDefence) -> DamageOutput {
        get_damage_output(
            &self.damage,
            &DamageRequest {
                single_monster: 0,
                vs_ac: i32::from(q.ac),
                vs_dr: i32::from(q.dr),
                vs_mr: i32::from(q.mr),
                vs_dodge: i32::from(q.dodge),
                defense_flags: q.flags,
                spell_immunity_lvl: q.spell_immu_lvl,
                vs_magic_lvl: q.magic_lvl,
                vs_bs_defense: q.bs_defense,
                vs_cold: q.rcol,
                vs_fire: q.rfir,
                vs_stone: q.rsto,
                vs_lightning: q.rlit,
                vs_water: q.rwat,
                ..DamageRequest::default()
            },
        )
    }

    fn monster_damage(&mut self, number: i32, n_party: i16) -> f64 {
        let ctx = self.damage.ctx;
        let env = GlueEnv {
            profile: ctx.profile,
            nmr_ver: ctx.nmr_ver,
        };
        let input = PreCalcInput {
            use_character: self.damage.character.global_filter,
            party: ctx.party,
            vs_party: self.vs_party,
            vs_char: self.vs_char,
            vs_default: self.vs_default,
        };
        // The only errors besides VB's own (handled inside) are Unsupported profiles, which
        // `GetLairInfo` cannot reach: stock is the only profile.
        get_pre_calculated_monster_damage(
            self.world,
            &env,
            &input,
            number,
            n_party,
            self.sim_rounds,
            self.sim,
            self.rnd,
        )
        .map_or(0.0, |(v, _)| v)
    }

    fn damage_array_bound(&self) -> Option<i32> {
        i32::try_from(self.vs_char.len())
            .ok()
            .and_then(|n| n.checked_sub(1))
            .filter(|&b| b >= 0)
    }
}

/// The real [`LairEnv`]: the cache, the `GetLairInfo` inputs and the callees over a character.
/// `GetLairAveragesFromLocs` and `CalculateAverageLairs` run on it.
pub struct CharLairEnv<'a> {
    pub cache: &'a mut LairCache,
    pub query: LairQuery<'a>,
    pub callouts: CharCallouts<'a>,
}

impl LairEnv for CharLairEnv<'_> {
    fn get_lair_info(&mut self, group_index: &str, max_regen: i16) -> LairInfo {
        self.cache
            .get_lair_info(&self.query, &mut self.callouts, group_index, max_regen)
    }

    fn get_damage_output(&mut self, q: &AvgDefence) -> DamageOutput {
        self.callouts.damage_output(&LairDefence {
            ac: q.ac,
            dr: q.dr,
            mr: q.mr,
            dodge: q.dodge,
            flags: q.flags,
            spell_immu_lvl: q.spell_immu_lvl,
            magic_lvl: q.magic_lvl,
            bs_defense: 0,
            rcol: 0,
            rfir: 0,
            rsto: 0,
            rlit: 0,
            rwat: 0,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// CalculateMonsterPartyDamage

/// Port of frmMain `CalculateMonsterPartyDamage` without the screen work (hourglass, colouring the
/// list, re-selecting the monster): `CalculateMonsterDamageVsCharALL(True)` for the lair-mode
/// party (`party`), whose per-monster result is what MME stores in `nMonsterDamageVsParty`, then
/// `tLastAvgLairInfo = GetLairInfo("")`, which empties the cache's last average lair (also when
/// the calculation failed, as MME's handler resumes at that line).
///
/// # Errors
/// Only what [`calculate_monster_damage_vs_char_all`] reports.
#[allow(clippy::too_many_arguments)]
pub fn calculate_monster_party_damage(
    cache: &mut LairCache,
    world: &World,
    env: &GlueEnv,
    sim_rounds: i32,
    chr: &crate::monster_glue::CharSimInputs,
    party: &PartyFilter,
    sim: &mut MonsterSimInput,
    rnd: &mut Rnd,
) -> Result<Vec<(i32, Option<Currency>)>, RulesError> {
    let r =
        calculate_monster_damage_vs_char_all(world, env, true, sim_rounds, chr, party, sim, rnd);
    cache.last_avg_lair_info = LairInfo::default();
    r
}
