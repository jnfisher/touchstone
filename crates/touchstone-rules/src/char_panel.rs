//! Char panel derivations (FAM-05): the formulas that live in frmMain's Char-tab event code.
//! Card P3-19.
//!
//! MME keeps these in control `.Text`, `.Caption` and `.Tag` strings and re-reads them with `Val`
//! (`RefreshCPs`, `RefreshHitPoints`, `RefreshMagic`, `RefreshPicklocks`, `RefreshMagicRes`,
//! `RefreshDodge`, `RefreshMainStatBonuses`, `SetupClass`, the encumbrance level of
//! `lblInvenCharStat_Change`, the answer handling of `CharStatAdjustmentPrompt`, and the
//! mana-per-round total of `RefreshCharBless`). The port takes the numbers those controls hold
//! (`f64` where MME applies `Val`, so its Double to Long conversions and their overflows are
//! reproduced) and returns typed values plus the short caption text, whose wording and signs are
//! part of what MME shows.
//!
//! Conventions, as in the gear cards:
//!
//! - A VB6 error that MME's handler leaves the procedure on stops the port at the same place:
//!   what was assigned before is returned as `Some`, what was not is `None`, and the error number
//!   is in `error`.
//! - Combo boxes are numbers: a class of 0 is the combo's first entry (`ListIndex` 0 holds the
//!   "(none)" item); `ListIndex > 0` and `ItemData > 0` agree for every real class. A race of
//!   `None` is `ListIndex < 0`.
//! - Stock engine only (`bGreaterMUD` false).
//! - Not ported (display only): tool tips, enabled states, colours and fonts of the controls, the
//!   dodge summary, mirroring the strength and charm into the inventory and shop boxes,
//!   `PutCommas` (the exp requirement is a number), the `RefreshAll` that follows an adjustment.
//!   `InvenResetStats` is [`crate::gear::CharStats::reset`]; its INI read is IO.

use touchstone_model::World;
use touchstone_vb6 as vb;
use touchstone_vb6::Vb6Error;

use crate::Profile;
use crate::exp::calc_exp_needed_by_race_class;
use crate::gear::{BLESS_SLOTS, refresh_char_bless};
use crate::spells::{MinMaxRequest, get_current_spell_min_max};
use crate::stats::{
    ManaRegenInput, calc_cp_level, calc_mana_regen, calc_max_hp, calc_max_mana,
    calc_meditate_mana_regen, calc_mr, calc_picklocks, calc_rest_mana_regen, calc_resting_rate,
    calc_spell_casting, get_class_magery, get_class_magery_lvl, get_class_max_hp, get_class_min_hp,
    get_race_cp, regen_on, soft_cap_extra, soft_caps_on,
};

type Res<T> = Result<T, Vb6Error>;

fn code(e: Vb6Error) -> i32 {
    i32::try_from(e.code()).unwrap_or(0)
}

/// `ByVal ... As Long` argument or `Long` variable fed a `Val(...)`: banker's rounding, error 6
/// out of range.
fn lng(x: f64) -> Res<i32> {
    vb::clng(x)
}

// ---------------------------------------------------------------------------------------------
// RefreshCPs

/// What `RefreshCPs` reads. `stat_texts` is `Val(txtCharStats(i).Text)` and `max_tags`
/// `Val(txtCharMaxStats(i).Tag)` (the race and class ceilings).
#[derive(Debug, Clone)]
pub struct CpInput<'a> {
    pub profile: Profile,
    pub world: &'a World,
    pub stat_texts: [f64; 6],
    pub max_tags: [f64; 6],
    /// The race combo's `ItemData`; `None` when `ListIndex < 0` (MME stops after the per-stat costs).
    pub race: Option<i32>,
    /// The class combo's `ItemData` (0 for none).
    pub class: i32,
    /// `Val(txtGlobalLevel(0).Text)`.
    pub level: f64,
}

/// The exp line of the creation-point box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExpRequired {
    /// Class or race is "(none)": no line.
    NotShown,
    /// "EXP Req: a lot." (more than 500 levels needed).
    ALot,
    /// `CalcExpNeededByRaceClass`; 0 where its own handler swallowed an overflow.
    Value(f64),
}

/// `lblStatCalc` after `RefreshCPs` ran to the end.
#[derive(Debug, Clone, PartialEq)]
pub struct CpSummary {
    /// "CPs Used": the total cost of the six stats.
    pub used: i32,
    /// "Avail" and `lblStatCalc.Tag`: creation points of the race and level less the cost.
    pub avail: i32,
    /// "Level Required": the first level whose points cover the cost (the loop's result even when
    /// a level is typed).
    pub level_required: i32,
    pub exp: ExpRequired,
}

impl CpSummary {
    /// The first two lines of `lblStatCalc.Caption`.
    #[must_use]
    pub fn caption_head(&self) -> String {
        format!(
            "CPs Used/Avail:  {}/{}\r\nLevel Required:  {}",
            self.used, self.avail, self.level_required
        )
    }
}

/// Result of [`refresh_cps`].
#[derive(Debug, Clone, PartialEq)]
pub struct CpPanel {
    /// The cost of each stat (the tool tip "CP Used").
    pub stat_costs: [i32; 6],
    pub total_cost: i32,
    /// `None` when MME left before writing `lblStatCalc` (no race selected, or an error).
    pub summary: Option<CpSummary>,
    pub error: Option<i32>,
}

/// Creation points one stat costs: 10 per point for the first ten points above the race ceiling,
/// then 20, and so on up to 10 per point's worth of cost, the tenth tier being open-ended.
fn stat_cp_cost(used: i32) -> Res<i32> {
    // For nCPCostPer = 1 To Fix(nCPsUsed / 10), leaving at tier 10
    let limit = vb::fix(f64::from(used) / 10.0);
    let mut base: i32 = 0;
    let mut tier: i32 = 1;
    while f64::from(tier) <= limit {
        if tier == 10 {
            break;
        }
        base = vb::add_i32(base, vb::mul_i32(10, tier)?)?;
        tier += 1;
    }
    if tier == 10 {
        vb::add_i32(base, vb::mul_i32(vb::sub_i32(used, 90)?, tier)?)
    } else {
        vb::add_i32(base, vb::mul_i32(used % 10, tier)?)
    }
}

/// The cost of stat `x` raised `used` points above its race minimum. MME prices every point on the
/// stock ladder; under a Quicksilver profile with soft caps the part above the race maximum is
/// priced by [`soft_cap_extra`] instead (R14). Without a known race the ladder stands.
fn stat_cost(input: &CpInput, x: usize, used: i32) -> Res<i32> {
    let race = input.race.and_then(|n| input.world.get_race(n));
    let (Some(r), Some(_)) = (race, soft_caps_on(input.profile)) else {
        return stat_cp_cost(used);
    };
    let race_max = [r.x_str, r.x_int, r.x_wil, r.x_agl, r.x_hea, r.x_chm][x];
    let min = lng(input.max_tags[x])?;
    let span = vb::sub_i32(race_max, min)?.max(0);
    let target = vb::add_i32(min, used)?;
    match soft_cap_extra(input.profile, race_max.max(min), target) {
        Ok(None) => stat_cp_cost(used),
        Ok(Some(extra)) => vb::add_i32(stat_cp_cost(used.min(span))?, extra),
        Err(_) => Err(Vb6Error::Overflow),
    }
}

/// Port of frmMain `RefreshCPs`: creation point cost of the six stats, points available, the level
/// that pays for them and the exp that level needs.
#[must_use]
pub fn refresh_cps(input: &CpInput) -> CpPanel {
    let mut out = CpPanel {
        stat_costs: [0; 6],
        total_cost: 0,
        summary: None,
        error: None,
    };
    match cps_steps(input, &mut out) {
        Ok(()) => {}
        Err(e) => out.error = Some(code(e)),
    }
    out
}

fn cps_steps(input: &CpInput, out: &mut CpPanel) -> Res<()> {
    let mut total: i32 = 0;
    for x in 0..6 {
        let mut used = lng(input.stat_texts[x] - input.max_tags[x])?;
        if used < 0 {
            used = 0;
        }
        let cost = stat_cost(input, x, used)?;
        out.stat_costs[x] = cost;
        total = vb::add_i32(total, cost)?;
        out.total_cost = total;
    }
    let Some(race) = input.race else {
        return Ok(());
    };
    let mut level_req: i32 = 1;
    let base = get_race_cp(input.world.get_race(race));
    let mut cps = base;
    while cps < total {
        let step = vb::fix(f64::from(level_req) / 10.0) * 5.0 + 10.0;
        cps = lng(f64::from(cps) + step)?;
        level_req = vb::add_i32(level_req, 1)?;
    }
    if input.level > 0.0 {
        let lvl = lng(input.level)?;
        let pts = calc_cp_level(lvl).map_err(|_| Vb6Error::Overflow)?;
        cps = vb::add_i32(base, pts)?;
    }
    let avail = vb::sub_i32(cps, total)?;
    let exp = if level_req > 500 {
        ExpRequired::ALot
    } else if input.class > 0 && race > 0 {
        let exp = calc_exp_needed_by_race_class(
            input.profile,
            level_req,
            input.class,
            race,
            &input.world.classes,
            &input.world.races,
        );
        ExpRequired::Value(exp.unwrap_or(0.0))
    } else {
        ExpRequired::NotShown
    };
    out.summary = Some(CpSummary {
        used: total,
        avail,
        level_required: level_req,
        exp,
    });
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// RefreshHitPoints

/// What `RefreshHitPoints` reads.
#[derive(Debug, Clone)]
pub struct HpInput<'a> {
    pub world: &'a World,
    /// `Val(txtGlobalLevel(0).Text)`.
    pub level: f64,
    /// The class combo's `ItemData`; 0 for the "(none)" entry.
    pub class: i32,
    /// `Val(txtCharStats(4).Tag)`: health with bless and gear.
    pub health: f64,
    /// `Val(txtCharHPRegen.Text)`.
    pub hp_regen: f64,
    /// `Val(lblInvenCharStat(5).Caption)`: the HP bonus from gear and race.
    pub hp_bonus: f64,
}

/// The class HP range, when a class is selected.
#[derive(Debug, Clone, PartialEq)]
pub struct HpRange {
    pub min: i32,
    pub max: i32,
    /// `lblCharMaxHP.Tag`: the middle of the range plus the bonus.
    pub average: f64,
}

/// Result of [`refresh_hit_points`].
#[derive(Debug, Clone, PartialEq)]
pub struct HitPoints {
    /// `CalcRestingRate` as shown ("Normal"), and with resting (`lblCharRestRate.Tag`).
    pub rest_normal: Option<i32>,
    pub rest_resting: Option<i32>,
    pub rest_caption: Option<String>,
    pub range: Option<HpRange>,
    /// `lblCharMaxHP.Caption`.
    pub max_hp_caption: Option<String>,
    pub error: Option<i32>,
}

/// Port of frmMain `RefreshHitPoints`: resting rate and the HP range of the class at this level.
#[must_use]
pub fn refresh_hit_points(profile: Profile, input: &HpInput) -> HitPoints {
    let mut out = HitPoints {
        rest_normal: None,
        rest_resting: None,
        rest_caption: None,
        range: None,
        max_hp_caption: None,
        error: None,
    };
    if let Err(e) = hp_steps(profile, input, &mut out) {
        out.error = Some(code(e));
    }
    out
}

fn hp_steps(profile: Profile, i: &HpInput, out: &mut HitPoints) -> Res<()> {
    let level = lng(i.level)?;
    let health = lng(i.health)?;
    let regen = lng(i.hp_regen)?;
    let resting = calc_resting_rate(profile, level, health, regen, true);
    let normal = calc_resting_rate(profile, level, health, regen, false);
    out.rest_resting = Some(resting);
    out.rest_normal = Some(normal);
    out.rest_caption = Some(format!("Normal:  {normal}\r\nResting:  {resting}"));

    if i.class <= 0 {
        out.max_hp_caption = Some("HP Range: ? - ?".to_string());
        return Ok(());
    }
    let class = i.world.get_class(i.class);
    // GetClassMinHP and GetClassMaxHP return Integers
    let min = get_class_min_hp(class);
    let max = get_class_max_hp(class);
    let spread = min_spread(max, min)?;
    let lo = calc_max_hp(spread, level, health, min);
    let hi = calc_max_hp(lng(f64::from(spread) * i.level)?, level, health, min);
    let avg = vb::round((f64::from(lo) + f64::from(hi)) / 2.0, 0)? + i.hp_bonus;
    let tag = vb::cstr(avg);
    let mut cap = String::from("HP: ");
    if lo != hi {
        cap.push('~');
    }
    cap.push_str(&tag);
    if lo != hi {
        cap.push_str(&format!(" ({lo}-{hi})"));
    }
    if i.hp_bonus != 0.0 {
        if i.hp_bonus > 0.0 {
            cap.push('+');
        }
        cap.push_str(&vb::cstr(i.hp_bonus));
    }
    out.range = Some(HpRange {
        min: lo,
        max: hi,
        average: avg,
    });
    out.max_hp_caption = Some(cap);
    Ok(())
}

/// `nMax - nMin` with both Integers.
fn min_spread(max: i32, min: i32) -> Res<i32> {
    let d = max - min;
    if i16::try_from(d).is_ok() {
        Ok(d)
    } else {
        Err(Vb6Error::Overflow)
    }
}

// ---------------------------------------------------------------------------------------------
// RefreshMagic

/// What `RefreshMagic` reads.
#[derive(Debug, Clone)]
pub struct MagicInput<'a> {
    pub profile: Profile,
    pub world: &'a World,
    pub level: f64,
    pub class: i32,
    /// `Val(txtCharStats(1).Tag)`, `(2)` and `(5)`: intellect, willpower and charm with bless.
    pub intellect: f64,
    pub willpower: f64,
    pub charm: f64,
    /// `Val(txtCharManaRegen.Text)`: extra mana regeneration percent.
    pub mana_regen_pct: f64,
    /// `Val(lblInvenCharStat(6).Caption)` and `(9)`: the mana and spellcasting bonuses of gear.
    pub mana_bonus: f64,
    pub spellcasting_bonus: f64,
}

/// Result of [`refresh_magic`]; a field is `None` when MME had not assigned it (an error ended
/// the procedure first, or the branch leaves it as it was).
#[derive(Debug, Clone, PartialEq)]
pub struct Magic {
    /// `lblCharManaRate.Tag`: mana regenerated per tick when not meditating.
    pub mana_regen: Option<i32>,
    /// `txtCharManaRegen.Tag`: mana per meditation tick. Under Quicksilver the tick counts the
    /// regeneration buffs and is the average over its carry cycle ([`calc_meditate_mana_regen`]).
    pub medi_ticks: Option<f64>,
    /// Quicksilver only (R10): mana per rest tick, since resting restores mana there. `None`
    /// under the stock rules.
    pub rest_ticks: Option<f64>,
    pub mana_regen_caption: Option<String>,
    /// `lblCharMaxMana.Tag`: max mana with the gear bonus.
    pub max_mana: Option<i32>,
    pub max_mana_caption: Option<String>,
    /// `lblCharSC.Tag`: spellcasting with the gear bonus.
    pub spellcasting: Option<i32>,
    pub spellcasting_caption: Option<String>,
    pub error: Option<i32>,
}

/// Port of frmMain `RefreshMagic`: mana regeneration, meditation ticks, maximum mana and
/// spellcasting for the selected class.
#[must_use]
pub fn refresh_magic(input: &MagicInput) -> Magic {
    let mut out = Magic {
        mana_regen: None,
        medi_ticks: None,
        rest_ticks: None,
        mana_regen_caption: None,
        max_mana: None,
        max_mana_caption: None,
        spellcasting: None,
        spellcasting_caption: None,
        error: None,
    };
    if let Err(e) = magic_steps(input, &mut out) {
        out.error = Some(code(e));
    }
    out
}

fn bonus_text(base: i32, bonus: i32) -> Res<(i32, String)> {
    let total = vb::add_i32(base, bonus)?;
    let sign = if bonus > 0 { "+" } else { "" };
    Ok((total, format!("{total} ({base}{sign}{bonus})")))
}

fn magic_steps(i: &MagicInput, out: &mut Magic) -> Res<()> {
    if i.class <= 0 {
        out.mana_regen_caption = Some("Mana Regen: ?".to_string());
        out.spellcasting_caption = Some("Spellcasting: ?".to_string());
        out.max_mana_caption = Some("Max Mana: ?".to_string());
        out.medi_ticks = Some(0.0);
        if regen_on(i.profile).is_some() {
            out.rest_ticks = Some(0.0);
        }
        out.mana_regen = Some(0);
        return Ok(());
    }
    let level = lng(i.level)?;
    let class = i.world.get_class(i.class);
    let magery = get_class_magery(class);
    let magery_code = magery as i32;
    let magery_lvl = get_class_magery_lvl(class);

    let regen_in = ManaRegenInput {
        level,
        intellect: lng(i.intellect)?,
        willpower: lng(i.willpower)?,
        charm: lng(i.charm)?,
        magic_level: magery_lvl,
        magic_type: magery_code,
        mp_regen: lng(i.mana_regen_pct)?,
        meditating: false,
    };
    let regen = vb::fix(calc_mana_regen(i.profile, &regen_in).to_f64());
    let regen_i = lng(regen)?;
    out.mana_regen = Some(regen_i);
    // MME's meditation tick takes no "Alter Mana Regeneration"; Quicksilver's counts it (R10)
    let medi = calc_meditate_mana_regen(i.profile, &regen_in).to_f64();
    out.medi_ticks = Some(medi);
    if regen_on(i.profile).is_some() {
        let rest = calc_rest_mana_regen(i.profile, &regen_in).to_f64();
        out.rest_ticks = Some(rest);
        out.mana_regen_caption = Some(format!(
            "Mana Regen: {regen_i}\r\nRest Ticks: {}\r\nMedi. Ticks: {}",
            vb::cstr(rest),
            vb::cstr(medi)
        ));
    } else {
        out.mana_regen_caption = Some(format!(
            "Mana Regen: {regen_i}\r\nMedi. Ticks: {}",
            vb::cstr(medi)
        ));
    }

    let max_mana = calc_max_mana(level, magery_lvl);
    let mana_bonus = lng(i.mana_bonus)?;
    if mana_bonus != 0 {
        let (total, text) = bonus_text(max_mana, mana_bonus)?;
        out.max_mana_caption = Some(format!("Max Mana: {text}"));
        out.max_mana = Some(total);
    } else {
        out.max_mana_caption = Some(format!("Max Mana: {max_mana}"));
        out.max_mana = Some(max_mana);
    }

    let sc = calc_spell_casting(
        level,
        regen_in.intellect,
        regen_in.willpower,
        regen_in.charm,
        magery_lvl,
        magery_code,
    );
    let sc_bonus = lng(i.spellcasting_bonus)?;
    if sc_bonus != 0 {
        let (total, text) = bonus_text(sc, sc_bonus)?;
        out.spellcasting_caption = Some(format!("Spellcasting: {text}"));
        out.spellcasting = Some(total);
    } else {
        out.spellcasting_caption = Some(format!("Spellcasting: {sc}"));
        out.spellcasting = Some(sc);
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// RefreshPicklocks, RefreshMagicRes, RefreshDodge

/// Result of [`refresh_picklocks`].
#[derive(Debug, Clone, PartialEq)]
pub struct Picklocks {
    /// `lblCharPicklocks.Tag`: base plus the gear bonus.
    pub total: Option<i32>,
    pub caption: Option<String>,
    pub error: Option<i32>,
}

/// Port of frmMain `RefreshPicklocks`. `agility`, `intellect` and `charm` are
/// `Val(txtCharStats(3/1/5).Tag)`; `bonus` is `Val(lblInvenCharStat(22).Caption)`.
#[must_use]
pub fn refresh_picklocks(
    profile: Profile,
    level: f64,
    agility: f64,
    intellect: f64,
    charm: f64,
    bonus: f64,
) -> Picklocks {
    let run = || -> Res<(i32, String)> {
        let lvl = lng(level)?;
        let agl = lng(agility)?;
        let int = lng(intellect)?;
        // the charm argument is converted too, though CalcPicklocks does not use it
        lng(charm)?;
        let base = calc_picklocks(profile, lvl, agl, int).map_err(|_| Vb6Error::Overflow)?;
        let bonus = lng(bonus)?;
        if bonus != 0 {
            let sep = if bonus > 0 { " +" } else { " " };
            let total = vb::add_i32(base, bonus)?;
            Ok((total, format!("Picklocks: {total} ({base}{sep}{bonus})")))
        } else {
            Ok((base, format!("Picklocks: {base}")))
        }
    };
    match run() {
        Ok((total, caption)) => Picklocks {
            total: Some(total),
            caption: Some(caption),
            error: None,
        },
        Err(e) => Picklocks {
            total: None,
            caption: None,
            error: Some(code(e)),
        },
    }
}

/// Port of frmMain `RefreshMagicRes`: the text of `txtCharMR` from intellect and willpower (with
/// bless) and `nGlobalCharPlusMR`.
///
/// # Errors
/// The VB6 error number where an argument does not fit a Long (MME's handler leaves the box as it was).
pub fn refresh_magic_res(
    profile: Profile,
    intellect: f64,
    willpower: f64,
    plus_mr: i32,
) -> Result<i32, i32> {
    let i = lng(intellect).map_err(code)?;
    let w = lng(willpower).map_err(code)?;
    Ok(calc_mr(profile, i, w, plus_mr))
}

/// Result of [`refresh_dodge`].
#[derive(Debug, Clone, PartialEq)]
pub struct Dodge {
    /// `lblCharDodge.Tag`: `Val(lblInvenCharStat(8).Tag)`.
    pub value: f64,
    /// The first line of the caption; with a positive dodge MME adds the dodge summary and
    /// "(After AC)" below it.
    pub caption_head: String,
    pub has_summary: bool,
}

/// Port of frmMain `RefreshDodge`: echoes the dodge the aggregation computed (`stat_tag` is
/// `lblInvenCharStat(8).Tag`, `None` while empty).
#[must_use]
pub fn refresh_dodge(stat_tag: Option<f64>) -> Dodge {
    let value = stat_tag.unwrap_or(0.0);
    Dodge {
        value,
        caption_head: format!("Dodge: {}", vb::cstr(value)),
        has_summary: value > 0.0,
    }
}

// ---------------------------------------------------------------------------------------------
// RefreshMainStatBonuses

/// Result of [`main_stat_labels`]: the six stat label captions (STR INT WIL AGL HEA CHM).
#[derive(Debug, Clone, PartialEq)]
pub struct StatLabels {
    /// `None` for a label MME did not reach because of an error.
    pub labels: [Option<String>; 6],
    pub error: Option<i32>,
}

/// Port of frmMain `RefreshMainStatBonuses`: "Str (+3):" style labels when the bless bonus is not
/// 0, plain "Strength:" otherwise. `bonus` is `Val(lblLabelArray(..).Tag)` per stat.
#[must_use]
pub fn main_stat_labels(bonus: &[f64; 6]) -> StatLabels {
    const SHORT: [&str; 6] = ["Str", "Int", "Wil", "Agi", "Hea", "Cha"];
    const LONG: [&str; 6] = [
        "Strength:",
        "Intellect:",
        "Willpower:",
        "Agility:",
        "Health:",
        "Charm:",
    ];
    let mut out = StatLabels {
        labels: Default::default(),
        error: None,
    };
    for x in 0..6 {
        match lng(bonus[x]) {
            Ok(0) => out.labels[x] = Some(LONG[x].to_string()),
            Ok(n) => {
                let sign = if n > 0 { "+" } else { "" };
                out.labels[x] = Some(format!("{} ({sign}{n}):", SHORT[x]));
            }
            Err(e) => {
                out.error = Some(code(e));
                break;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// SetupClass

/// What `SetupClass` reads.
#[derive(Debug, Clone)]
pub struct ClassSetupInput<'a> {
    pub world: &'a World,
    /// The class combo's `ItemData`; `None` when `ListIndex < 0` (MME does nothing).
    pub class: Option<i32>,
    /// The combo's text is "(none)" (an item with a number can still be called that).
    pub none_text: bool,
    /// `chkCharQuests(5, 9, 10, 11).Value = 1`: the 2nd, 6th, dread and Renfry quests.
    pub quest_checks: [bool; 4],
    /// `cmbCharQuestOpts(0..3).ListIndex` before the call.
    pub quest_options: [i32; 4],
    /// `cmbSpellMageryLevel.ListCount` and `cmbSpellMagery.ListCount`.
    pub magery_level_count: i32,
    pub magery_count: i32,
}

/// What `SetupClass` leaves in the controls and globals.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassSetup {
    /// `cmbCharQuestOpts(0..3).ListIndex`.
    pub quest_options: [i32; 4],
    /// `nGlobalCharMageryLVL` (clamped to 3) and `nGlobalCharMagery`.
    pub magery_lvl: i32,
    pub magery: i32,
    /// `cmbSpellMageryLevel.ListIndex` / `cmbSpellMagery.ListIndex` when set.
    pub magery_level_index: Option<i32>,
    pub magery_index: Option<i32>,
    /// Picklocks panel enabled.
    pub has_picks: bool,
    /// `chkCharAntiMagic`, `chkArmourNonMagic` and the weapon option: set for a class with ability 51.
    pub anti_magic: bool,
    /// Spell option and the spellcasting panel enabled.
    pub has_magic: bool,
    /// The armour-type and handedness filters are all ticked (a class is selected).
    pub class_filters_ticked: bool,
}

/// Port of frmMain `SetupClass` (stock): the class's default 2nd-quest option, magery level and
/// kind, picklock and anti-magic flags. `None` where MME exits without touching anything.
#[must_use]
pub fn setup_class(input: &ClassSetupInput) -> Option<ClassSetup> {
    let class = input.class?;
    let mut opts = input.quest_options;
    let checks = input.quest_checks;
    let mut found = None;
    let no_class = class == 0 || input.none_text || input.world.classes.is_empty();
    if !no_class {
        if !checks[0] || opts[0] == 0 {
            // witchunter and mystic only differ from the others on GreaterMUD
            let pick = match class {
                1 | 2 | 3 | 15 => Some(1),
                4 | 11 => Some(2),
                5 | 12 | 13 => Some(3),
                6 | 9 | 10 => Some(4),
                7 | 8 | 14 => Some(5),
                _ => None,
            };
            if let Some(p) = pick {
                opts[0] = p;
            }
        }
        found = input.world.get_class(class);
    }
    let mut s = ClassSetup {
        quest_options: opts,
        magery_lvl: 3,
        magery: 0,
        magery_level_index: None,
        magery_index: None,
        has_picks: true,
        anti_magic: false,
        has_magic: true,
        class_filters_ticked: false,
    };
    if let Some(c) = found {
        s.class_filters_ticked = true;
        s.has_picks = false;
        s.has_magic = false;
        for a in &c.abilities {
            match a.ability {
                51 => s.anti_magic = true,
                37 => s.has_picks = true,
                _ => {}
            }
        }
        s.magery_lvl = c.magery_lvl.min(3);
        s.magery = c.magery_type;
        s.has_magic = c.magery_type > 0;
    } else {
        for (opt, &checked) in s.quest_options.iter_mut().zip(&checks) {
            if !checked {
                *opt = 0;
            }
        }
    }
    if s.magery_lvl < input.magery_level_count {
        s.magery_level_index = Some(s.magery_lvl);
    }
    if s.magery < input.magery_count {
        s.magery_index = Some(s.magery);
    }
    Some(s)
}

// ---------------------------------------------------------------------------------------------
// lblInvenCharStat_Change: encumbrance level

/// The encumbrance level box (`lblEncumLevel(1)`).
#[derive(Debug, Clone, PartialEq)]
pub struct EncumLevel {
    /// "None", "Light", "Medium" or "Heavy".
    pub word: &'static str,
    /// The percentage text as MME prints it: a Single, `Fix` of the ratio.
    pub percent: String,
    /// `lblEncumLevel(1).Caption` (word, line break, "(n%)").
    pub caption: String,
    /// `lblEncumLevel(1).Tag` ("- Light (n%)").
    pub tag: String,
}

/// A Single printed by `CStr`: integers up to 7 digits plain, larger or tiny values in E notation
/// with 7 significant digits.
fn single_text(x: f32) -> String {
    let v = f64::from(x);
    if v == 0.0 {
        return "0".to_string();
    }
    if v.abs() < 1e7 && v.abs() >= 1e-4 && v.fract() == 0.0 {
        return format!("{}", v as i64);
    }
    let s = format!("{v:.6e}");
    let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
    let mant = if mant.contains('.') {
        mant.trim_end_matches('0').trim_end_matches('.')
    } else {
        mant
    };
    let e: i32 = exp.parse().unwrap_or(0);
    format!("{mant}E{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
}

/// Port of the encumbrance branch (indexes 0 and 1) of frmMain `lblInvenCharStat_Change`: the
/// level word and percentage from current and maximum encumbrance (`Val` of the stat captions).
///
/// # Errors
/// 6 where the ratio does not fit a Single (MME's handler leaves the box as it was).
pub fn encum_level(current: f64, max: f64) -> Result<EncumLevel, i32> {
    let (word, pct) = if max == 0.0 {
        if current >= 1.0 {
            ("Heavy", vb::cstr(current))
        } else {
            ("None", "0".to_string())
        }
    } else {
        let t = vb::csng(vb::fix((current / max) * 100.0)).map_err(code)?;
        let word = if t < 17.0 {
            "None"
        } else if t < 34.0 {
            "Light"
        } else if t < 67.0 {
            "Medium"
        } else {
            "Heavy"
        };
        (word, single_text(t))
    };
    Ok(EncumLevel {
        word,
        caption: format!("{word}\r\n({pct}%)"),
        tag: format!("- {word} ({pct}%)"),
        percent: pct,
    })
}

// ---------------------------------------------------------------------------------------------
// CharStatAdjustmentPrompt

/// What typing an answer into the stat-adjustment prompt does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdjustmentOutcome {
    /// Stat 46 (swings) opens the combat setup instead.
    OpenCombatPicker,
    /// Cancel or an empty answer.
    Cancelled,
    /// The value equals the stored one: nothing is saved.
    NoChange,
    /// `char_StatAdjustments(index)` becomes this; the caller marks the character changed and
    /// refreshes everything.
    Changed(i32),
    /// The assignment failed (VB6 error number); MME's handler leaves everything as it was.
    Error(i32),
}

/// Port of the answer handling of frmMain `CharStatAdjustmentPrompt`. `current` is the stored
/// `char_StatAdjustments(index)` (AC and DR, indexes 2 and 3, are typed in whole points and stored
/// times ten). An answer below -9999 becomes -999 and one above 9999 becomes 9999.
#[must_use]
pub fn stat_adjustment_answer(index: usize, current: i32, answer: &str) -> AdjustmentOutcome {
    if index == 46 {
        return AdjustmentOutcome::OpenCombatPicker;
    }
    let tenths = index == 2 || index == 3;
    let mut cur = f64::from(current);
    if tenths {
        cur /= 10.0;
    }
    let text = answer.trim_matches(' ');
    if text.is_empty() {
        return AdjustmentOutcome::Cancelled;
    }
    let mut v = vb::val(text);
    if v < -9999.0 {
        v = -999.0;
    }
    if v > 9999.0 {
        v = 9999.0;
    }
    let new = if tenths {
        cur *= 10.0;
        v * 10.0
    } else {
        v
    };
    if new == cur {
        return AdjustmentOutcome::NoChange;
    }
    match lng(new) {
        Ok(n) => AdjustmentOutcome::Changed(n),
        Err(e) => AdjustmentOutcome::Error(code(e)),
    }
}

// ---------------------------------------------------------------------------------------------
// RefreshCharBless: mana per round

/// Port of the mana total of frmMain `RefreshCharBless` (`lblCharBless`): the mana the ten bless
/// slots cost per mana-regeneration period of six combat rounds, as a Double rounded to two
/// decimals. `level` is `Val(txtGlobalLevel(0).Text)`. `None` where MME leaves the label alone:
/// no spells in the database, or a VB6 error ended the pass (a spell of duration 0 divides by zero).
///
/// The stat totals of the same procedure are [`refresh_char_bless`]; this function runs it for
/// the error behaviour and repeats its level and duration steps for the total.
#[must_use]
pub fn bless_mana_per_round(
    profile: Profile,
    world: &World,
    level: f64,
    slots: &[i32; BLESS_SLOTS],
) -> Option<f64> {
    if world.spells.is_empty() {
        return None;
    }
    let level = lng(level).ok()?;
    let stats = refresh_char_bless(profile, world, level, slots, 0);
    if stats.error.is_some() {
        return None;
    }
    let set_level = if level == 0 { 1 } else { level };
    let mut total = 0.0_f64;
    for &number in slots {
        if number <= 0 {
            continue;
        }
        let Some(spell) = world.get_spell(number) else {
            continue;
        };
        let mut lv = set_level;
        if lv > spell.cap && spell.cap > 0 {
            lv = spell.cap;
        }
        if lv < spell.req_level {
            lv = spell.req_level;
        }
        let req = MinMaxRequest {
            use_level: lv > 0,
            level: i16::try_from(lv).ok()?,
            ..MinMaxRequest::default()
        };
        let mm = get_current_spell_min_max(Some(spell), 0.0, &req);
        let per_sec = f64::from(spell.mana_cost) / (mm.n_dur.to_f64() * 3.0);
        total += vb::round(per_sec, 3).ok()?;
    }
    vb::round(total * 5.0 * 6.0, 2).ok()
}
