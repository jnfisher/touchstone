//! The Monsters list row (FAM-21, monsters). Card P3-27.
//!
//! [`add_monster_row`] ports modMain `AddMonster2LV`: the columns of one `Monsters` record, the damage it
//! deals, and in lair mode the exp/hr that `CalcExpPerHour` gives over the monster's average lair. MME
//! builds the row while `FilterMonsters` walks the table, and `tLastAvgLairInfo` (the cache's
//! `last_avg_lair_info`) carries over from one row to the next, so call the function in table order on
//! one [`CharLairEnv`] to get the list MME shows.
//!
//! The frmMain controls the procedure reads are the env's [`PartyFilter`](crate::character::PartyFilter)
//! (`optMonsterFilter(1)`, `txtMonsterLairFilter(0)`) and `use_character` (`chkGlobalFilter`), plus
//! [`MonsterRowInputs`]. `GetPreCalculatedMonsterDamage`, `GetDamageOutput` and the lair averages are
//! answered by the env. A VB error part-way leaves the row built so far, as `Resume out` does; one before
//! the row exists (a party size that does not fit an Integer) gives `None`, like a skipped record.
//!
//! The version before 1.83 shows the "script value" column instead of the lair's resting percentage; it is
//! ported too. Colours are the light theme's (`TColor` is the identity there).

use touchstone_model::Monster;
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Vb6Error};

use super::{Cell, Row, cur_text, round_up};
use crate::RulesError;
use crate::character::{AttackConfig, AttackType, CharacterProfile};
use crate::combat::DamageRequest;
use crate::exp_per_hour::{ExpPerHourInfo, ExpPerHourInput, ExpPerHourSettings, calc_exp_per_hour};
use crate::lairs::{
    CharLairEnv, LairCallouts, LairEnv, LairInfo, LairLocsInput, get_lair_averages_from_locs,
    instr_count,
};
use crate::monster_glue::{GlueEnv, get_monster_attack_summary};

type Res<T> = Result<T, Vb6Error>;

/// `TColor(RGB(193, 0, 232))`: the damage was calculated against this character or party.
pub const CALCULATED_COLOR: i32 = 193 + 232 * 65536;
/// `TColor(RGB(192, 192, 192))`: the monster does not match the filter.
pub const NOT_MATCHING_COLOR: i32 = 192 + 192 * 256 + 192 * 65536;

/// The arguments of `AddMonster2LV` after the list and the profile, with MME's defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonsterRowArgs {
    /// `nDamageOut`; -9999: not worked out yet.
    pub damage_out: i32,
    /// `nPassEXP`: exp/hr `FilterMonsters` already computed; negative: compute here.
    pub pass_exp: Currency,
    /// `nPassRecovery`.
    pub pass_recovery: f64,
    /// `nSurpriseDamageOut`; -9999: not worked out yet.
    pub surprise_damage_out: i32,
    pub surprise_min_dmg: i32,
    pub surprise_chance: i16,
    pub first_round_dmg: i32,
    pub min_round_dmg: i32,
    /// `bDoesNotMatchFilter`: grey the whole row.
    pub does_not_match_filter: bool,
}

impl Default for MonsterRowArgs {
    fn default() -> Self {
        Self {
            damage_out: -9999,
            pass_exp: Currency::from_scaled(-10_000),
            pass_recovery: -1.0,
            surprise_damage_out: -9999,
            surprise_min_dmg: 0,
            surprise_chance: 0,
            first_round_dmg: 0,
            min_round_dmg: 0,
            does_not_match_filter: false,
        }
    }
}

/// Globals and controls `AddMonster2LV` reads that the lair env does not hold.
pub struct MonsterRowInputs<'a> {
    /// `UseExpMulti`: the Monsters table has an `ExpMulti` column to multiply `EXP` by.
    pub use_exp_multi: bool,
    /// `lv.hWnd = frmMain.lvMonsters.hWnd`: the row goes into the main list, not the compare list.
    pub is_main_list: bool,
    /// The exp/hr settings (`bGlobal_ceph*`).
    pub settings: &'a ExpPerHourSettings,
    /// `nMonsterPossy` and `nMonsterSpawnChance`, indexed by monster number.
    pub possy: &'a [Currency],
    pub spawn_chance: &'a [Currency],
    /// `nGlobalAttackTypeMME > a0_oneshot And bGlobalAttackBackstab`.
    pub backstab_attack: bool,
}

/// The backstab test `AddMonster2LV` makes of the attack set-up.
#[must_use]
pub fn backstab_attack(a: &AttackConfig) -> bool {
    a.attack_type != AttackType::OneShot && a.backstab
}

fn rt(e: RulesError) -> Vb6Error {
    match e {
        RulesError::Vb6(_) => Vb6Error::Overflow,
        RulesError::Unsupported(_) => Vb6Error::Overflow,
    }
}

fn i16c(v: i32) -> Res<i16> {
    i16::try_from(v).map_err(|_| Vb6Error::Overflow)
}

fn cur(v: i32) -> Currency {
    Currency::from_scaled(i64::from(v) * 10_000)
}

fn cur_f(x: f64) -> Res<Currency> {
    Currency::from_f64(x)
}

/// `Round(x, digits)` on a Currency (the result is a Currency again).
fn cur_round(c: Currency, digits: i32) -> Res<Currency> {
    Currency::from_f64(vb::round(c.to_f64(), digits)?)
}

/// `Format(x, pattern)` with `IIf(x > 0, Format(..), 0)`: the formatted text, or `0`.
fn fmt_pos(x: f64, pattern: &str) -> Res<String> {
    if x > 0.0 {
        vb::format(x, pattern)
    } else {
        Ok("0".to_string())
    }
}

/// modSyntaxsFunc `PutCommas` (without the shorten flag).
#[must_use]
pub fn put_commas(number: &str) -> String {
    let s = number.trim_matches(' ');
    if s.is_empty() {
        return number.to_string();
    }
    let (sign, s) = if let Some(r) = s.strip_prefix('-') {
        ("-", r)
    } else if let Some(r) = s.strip_prefix('+') {
        ("", r)
    } else {
        ("", s)
    };
    let s = s.replace([',', ' '], "");
    let (int_part, frac) = match s.find('.') {
        Some(p) => (&s[..p], &s[p..]),
        None => (s.as_str(), ""),
    };
    let n = int_part.chars().count();
    if n < 4 {
        return format!("{sign}{int_part}{frac}");
    }
    let chars: Vec<char> = int_part.chars().collect();
    let mut out = String::new();
    for (k, ch) in chars.iter().rev().enumerate() {
        out.insert(0, *ch);
        let z = k + 1;
        if z > 2 && z != n && z % 3 == 0 {
            out.insert(0, ',');
        }
    }
    format!("{sign}{out}{frac}")
}

/// modSyntaxsFunc `SortLettersWithSeparator`: every part between separators sorted by character.
#[must_use]
pub fn sort_letters_with_separator(input: &str, sep: &str) -> String {
    input
        .split(sep)
        .map(|p| {
            let mut c: Vec<char> = p.chars().collect();
            c.sort_unstable();
            c.into_iter().collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(sep)
}

struct Tmp {
    exp: Currency,
    hp: Currency,
    avg_dmg: i32,
    asterisks: bool,
    time_recovering: f64,
}

/// Port of modMain `AddMonster2LV`: the row for monster `number`, or `None` where MME adds none (no such
/// record, an empty or `sdf` name, or an error before the row exists). `t_char` is the profile
/// `PopulateCharacterProfile` made for the list.
#[must_use]
pub fn add_monster_row(
    env: &mut CharLairEnv<'_>,
    inp: &MonsterRowInputs<'_>,
    t_char: &CharacterProfile,
    number: i32,
    args: &MonsterRowArgs,
) -> Option<Row> {
    let world = env.callouts.world;
    let m = world.get_monster(number)?;
    let mut row = Row::default();
    let n_party = party_size(env).ok()?;
    let summoned = m.summoned_by.clone().unwrap_or_default();

    // tLastAvgLairInfo: the lair averages of the monster in lair mode, reset otherwise
    let nmr = env.callouts.damage.ctx.nmr_ver;
    let lair_mode = env.query.party.lair_mode;
    let by_lair = nmr >= 1.83 && inp.is_main_list && lair_mode;
    if by_lair
        && (env.cache.last_avg_lair_info.group_index != summoned
            || env.cache.last_avg_lair_info.global_attack_config != env.query.global_attack_config)
    {
        let cfg = env.query.global_attack_config.to_string();
        let avg = get_lair_averages_from_locs(
            env,
            &summoned,
            &LairLocsInput {
                nmr_ver: nmr,
                summoned_by: &summoned,
                global_attack_config: &cfg,
            },
        );
        env.cache.last_avg_lair_info = avg;
    } else if !by_lair && !env.cache.last_avg_lair_info.group_index.is_empty() {
        env.cache.last_avg_lair_info = env.get_lair_info("", 0);
    }

    let name = m.name.clone().unwrap_or_default();
    if name.is_empty() || name.starts_with("sdf") {
        return None;
    }
    let mut magic_lvl: i16 = 0;
    let mut mob_dodge: i16 = 0;
    for slot in &m.abilities {
        if slot.ability != 0 {
            match slot.ability {
                28 => magic_lvl = i16c(slot.value).ok()?,
                34 => mob_dodge = i16c(slot.value).ok()?,
                _ => {}
            }
        }
    }
    row.text = number.to_string();
    let _ = steps(
        env, inp, t_char, m, number, args, n_party, &summoned, magic_lvl, mob_dodge, &mut row,
    );
    if args.does_not_match_filter {
        row.fore_color = Some(NOT_MATCHING_COLOR);
        for c in &mut row.cells {
            c.fore_color = Some(NOT_MATCHING_COLOR);
        }
    }
    Some(row)
}

/// `nParty` from the lair-mode controls: the party size above 1 (an Integer: it can overflow), clamped to 1..6.
fn party_size(env: &CharLairEnv<'_>) -> Res<i16> {
    let mut n: i16 = 0;
    if env.query.party.lair_mode {
        let v = vb::val(&env.query.party.lair_text[0]);
        if v > 1.0 {
            n = vb::cint(v)?;
        }
    }
    Ok(n.clamp(1, 6))
}

fn color_for(
    env: &CharLairEnv<'_>,
    number: i32,
    n_party: i16,
    both_arrays: bool,
) -> Res<Option<i32>> {
    let at = |a: &[Currency]| -> Res<f64> {
        usize::try_from(number)
            .ok()
            .and_then(|i| a.get(i))
            .map(|c| c.to_f64())
            .ok_or(Vb6Error::Overflow)
    };
    if n_party > 1 {
        return Ok((at(env.callouts.vs_party)? >= 0.0).then_some(CALCULATED_COLOR));
    }
    // VB6's And evaluates both operands: the array is read whether or not the character is in use
    let c = at(env.callouts.vs_char)? >= 0.0;
    let _ = both_arrays;
    Ok((env.query.use_character && c).then_some(CALCULATED_COLOR))
}

fn push(row: &mut Row, text: impl Into<String>, tag: Option<String>) {
    row.cells.push(Cell {
        text: text.into(),
        tag,
        fore_color: None,
    });
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn steps(
    env: &mut CharLairEnv<'_>,
    inp: &MonsterRowInputs<'_>,
    t_char: &CharacterProfile,
    m: &Monster,
    number: i32,
    args: &MonsterRowArgs,
    n_party: i16,
    summoned: &str,
    magic_lvl: i16,
    mob_dodge: i16,
    row: &mut Row,
) -> Res<()> {
    let world = env.callouts.world;
    let nmr = env.callouts.damage.ctx.nmr_ver;
    let profile = env.callouts.damage.ctx.profile;
    let lair_mode = env.query.party.lair_mode;
    let t_avg: LairInfo = env.cache.last_avg_lair_info.clone();
    let mut tmp = Tmp {
        exp: cur(0),
        hp: cur(0),
        avg_dmg: -1,
        asterisks: false,
        time_recovering: 0.0,
    };

    push(row, m.name.clone().unwrap_or_default(), None);
    let regen = vb::cstr(f64::from(m.regen_time));
    push(row, regen.clone(), Some(regen));

    tmp.exp = if inp.use_exp_multi {
        cur_f(m.exp * m.exp_multi)?
    } else {
        cur_f(m.exp)?
    };
    push(
        row,
        fmt_pos(tmp.exp.to_f64(), "#,#")?,
        Some(cur_text(tmp.exp)),
    );

    tmp.hp = cur(m.hp);
    push(
        row,
        fmt_pos(f64::from(m.hp), "#,#")?,
        Some(cur_text(tmp.hp)),
    );

    push(
        row,
        format!("{}/{}", m.armour_class, m.damage_resist),
        Some((i64::from(m.armour_class) + i64::from(m.damage_resist)).to_string()),
    );
    push(row, mob_dodge.to_string(), Some(mob_dodge.to_string()));
    push(row, m.magic_res.to_string(), Some(m.magic_res.to_string()));

    let mut summary = get_monster_attack_summary(
        world,
        &GlueEnv {
            profile,
            nmr_ver: nmr,
        },
        number,
        true,
        false,
    );
    if summary.acc_majority != summary.acc_max {
        push(
            row,
            format!("{}/{}", summary.acc_majority, summary.acc_max),
            Some(summary.acc_max.to_string()),
        );
    } else {
        push(
            row,
            summary.acc_max.to_string(),
            Some(summary.acc_max.to_string()),
        );
    }

    // Damage
    let mut suffix = "";
    if t_avg.total_lairs > 0 && m.regen_time == 0 {
        tmp.avg_dmg = vb::clng(t_avg.avg_dmg_lair.to_f64())?;
        suffix = "*";
        tmp.asterisks = true;
    } else {
        tmp.avg_dmg = vb::clng(env.callouts.monster_damage(number, n_party))?;
    }
    let dmg_text = if tmp.avg_dmg > 0 {
        vb::format(f64::from(tmp.avg_dmg), "#,##")?
    } else if tmp.avg_dmg == 0 {
        "0".to_string()
    } else {
        "?".to_string()
    };
    push(
        row,
        format!("{dmg_text}{suffix}"),
        Some(tmp.avg_dmg.to_string()),
    );
    row.cells.last_mut().unwrap().fore_color = color_for(env, number, n_party, false)?;

    let mut poss_spawns: i32 = 0;
    if summoned.to_lowercase().contains("(lair)") {
        poss_spawns = instr_count(summoned, "(lair)");
    }

    // Exp/(Dmg+HP), or exp/hr in lair mode
    let mut dmg_out = args.damage_out;
    let (mut min_round, mut first_round) = (args.min_round_dmg, args.first_round_dmg);
    let (mut sur_out, mut sur_min, mut sur_chance) = (
        args.surprise_damage_out,
        args.surprise_min_dmg,
        args.surprise_chance,
    );
    if nmr >= 1.83 && lair_mode && inp.is_main_list {
        tmp.asterisks = false;
        let mut info = ExpPerHourInfo::default();
        if args.pass_exp.scaled() < 0 || args.pass_recovery < 0.0 {
            if m.regen_time == 0 && t_avg.total_lairs > 0 {
                let l = &t_avg;
                info = calc_exp_per_hour(
                    &ExpPerHourInput {
                        n_exp: l.avg_exp,
                        n_regen_time: l.avg_delay,
                        n_num_mobs: l.max_regen.to_f64(),
                        n_total_lairs: l.total_lairs,
                        n_poss_spawns: l.poss_spawns,
                        n_rtk: l.rtk,
                        n_char_dmg: f64::from(l.damage_out),
                        n_char_hp: vb::clng(t_char.hp)?,
                        n_char_hp_regen: vb::clng(t_char.hp_regen)?,
                        n_mob_dmg: l.avg_dmg_lair.to_f64(),
                        n_mob_hp: l.avg_hp,
                        n_mob_hp_regen: 0,
                        n_damage_threshold: vb::clng(t_char.damage_threshold)?,
                        n_spell_cost: vb::cint(t_char.spell_attack_cost)?,
                        n_spell_overhead: t_char.spell_overhead,
                        n_char_mana: vb::clng(t_char.max_mana)?,
                        n_char_mp_regen: vb::clng(t_char.mana_regen)?,
                        n_meditate_rate: vb::clng(t_char.meditate_rate)?,
                        n_avg_walk: l.avg_walk.to_f64(),
                        n_walk_speed: t_char.walk_speed,
                        n_surprise_dmg: f64::from(l.surprise_damage_out),
                        n_surprise_min_dmg: f64::from(l.surprise_min_damage_out),
                        n_surprise_chance: l.surprise_chance,
                        n_char_first_round_dmg: f64::from(l.first_round_damage_out),
                        n_min_round_dmg: f64::from(l.min_round_damage_out),
                    },
                    inp.settings,
                    profile,
                )
                .map_err(rt)?;
            } else if m.regen_time > 0 || summoned.to_lowercase().contains("room") {
                if dmg_out == -9999
                    || (t_char.party == 1 && sur_out == -9999 && inp.backstab_attack)
                {
                    let d = crate::combat::get_damage_output(
                        &env.callouts.damage,
                        &DamageRequest {
                            single_monster: number,
                            ..DamageRequest::default()
                        },
                    );
                    let l = |c: Currency| vb::clng(c.to_f64());
                    if d.average_damage.to_f64() > -9990.0 {
                        dmg_out = l(d.average_damage)?;
                        min_round = l(d.min_round_damage)?;
                        first_round = l(d.first_round_damage)?;
                    } else {
                        dmg_out = 0;
                        min_round = 0;
                        first_round = 0;
                    }
                    if d.surprise_damage.to_f64() > -9990.0 {
                        sur_out = l(d.surprise_damage)?;
                        sur_min = l(d.surprise_min_damage)?;
                        sur_chance = d.surprise_damage_chance;
                    } else {
                        sur_out = 0;
                        sur_min = 0;
                        sur_chance = 0;
                    }
                }
                info = calc_exp_per_hour(
                    &ExpPerHourInput {
                        n_exp: tmp.exp,
                        n_regen_time: f64::from(m.regen_time),
                        n_num_mobs: 1.0,
                        n_total_lairs: -1,
                        n_poss_spawns: 0,
                        n_rtk: 0.0,
                        n_char_dmg: f64::from(dmg_out),
                        n_char_hp: vb::clng(t_char.hp)?,
                        n_char_hp_regen: vb::clng(t_char.hp_regen)?,
                        n_mob_dmg: f64::from(tmp.avg_dmg),
                        n_mob_hp: m.hp,
                        n_mob_hp_regen: m.hp_regen,
                        n_damage_threshold: vb::clng(t_char.damage_threshold)?,
                        n_spell_cost: vb::cint(t_char.spell_attack_cost)?,
                        n_spell_overhead: t_char.spell_overhead,
                        n_char_mana: vb::clng(t_char.max_mana)?,
                        n_char_mp_regen: vb::clng(t_char.mana_regen)?,
                        n_meditate_rate: vb::clng(t_char.meditate_rate)?,
                        n_avg_walk: 0.0,
                        n_walk_speed: t_char.walk_speed,
                        n_surprise_dmg: f64::from(sur_out),
                        n_surprise_min_dmg: f64::from(sur_min),
                        n_surprise_chance: sur_chance,
                        n_char_first_round_dmg: f64::from(first_round),
                        n_min_round_dmg: f64::from(min_round),
                    },
                    inp.settings,
                    profile,
                )
                .map_err(rt)?;
            }
        }

        let mut exp_hr = if args.pass_exp.scaled() >= 0 {
            args.pass_exp
        } else {
            let mut x = cur_f(info.n_exp_per_hour)?;
            if x.scaled() > 0 && n_party > 1 {
                x = cur_f(vb::round_half_even(
                    x.checked_div(cur(i32::from(n_party)))?.to_f64(),
                ))?;
            }
            x
        };
        // the Currency variable keeps four decimals
        exp_hr = Currency::from_scaled(exp_hr.scaled());
        tmp.time_recovering = if args.pass_recovery >= 0.0 {
            args.pass_recovery
        } else {
            info.n_time_recovering
        };
        let mut text = if exp_hr.scaled() > 10_000 * 1_000_000 {
            format!(
                "{} M",
                vb::format(exp_hr.checked_div(cur(1_000_000))?.to_f64(), "#,#.0")?
            )
        } else if exp_hr.scaled() > 10_000 * 1000 {
            format!(
                "{} K",
                vb::format(exp_hr.checked_div(cur(1000))?.to_f64(), "#,#.0")?
            )
        } else if exp_hr.scaled() > 0 {
            vb::format(round_up(exp_hr.to_f64()), "#,#")?
        } else {
            "0".to_string()
        };
        text.push_str(if exp_hr.scaled() > 0 && n_party > 1 {
            "/hr ea."
        } else {
            "/hr"
        });
        push(row, text, Some(cur_text(exp_hr)));
        row.cells.last_mut().unwrap().fore_color = color_for(env, number, n_party, false)?;
    } else if tmp.exp.scaled() > 0 {
        let exp_dmg_hp = if tmp.avg_dmg > 0 || m.hp > 0 {
            if tmp.avg_dmg < 0 {
                tmp.avg_dmg = 0;
            }
            let doubled = vb::mul_i32(tmp.avg_dmg, 2)?;
            let denom = cur_f(f64::from(doubled))?.checked_add(tmp.hp)?;
            cur_round(tmp.exp.checked_div(denom)?, 2)?.checked_mul(cur(100))?
        } else {
            tmp.exp.checked_mul(cur(100))?
        };
        let star = if tmp.asterisks { "*" } else { "" };
        push(
            row,
            format!("{}{star}", fmt_pos(exp_dmg_hp.to_f64(), "#,#")?),
            Some(cur_text(exp_dmg_hp)),
        );
        row.cells.last_mut().unwrap().fore_color = color_for(env, number, n_party, false)?;
    } else {
        push(row, "0", Some(cur_text(tmp.exp)));
    }

    // Lair Exp / resting percentage, or the script value before data 1.83
    if nmr >= 1.83 {
        if lair_mode && inp.is_main_list && !inp.settings.recovery_only {
            let r = vb::round_half_even(tmp.time_recovering * 100.0);
            push(row, format!("{}%", vb::cstr(r)), Some(vb::cstr(r)));
        } else {
            let v = vb::cstr(m.avg_lair_exp);
            push(row, put_commas(&v), Some(v));
        }
    } else {
        let possy = *inp
            .possy
            .get(usize::try_from(number).unwrap_or(usize::MAX))
            .ok_or(Vb6Error::Overflow)?;
        let mut max_before = cur(36);
        if possy.scaled() > 0 {
            max_before = cur_round(max_before.checked_div(possy)?, 2)?;
        }
        let spawns = cur(poss_spawns);
        let lair_pct = if spawns.scaled() < max_before.scaled() {
            cur_round(spawns.checked_div(max_before)?, 2)?
        } else {
            cur(1)
        };
        let mut script = cur(0);
        if m.regen_time == 0 && lair_pct.scaled() > 0 {
            let mut possy_pct = cur(1);
            let possy_now = *inp
                .possy
                .get(usize::try_from(number).unwrap_or(usize::MAX))
                .ok_or(Vb6Error::Overflow)?;
            if possy_now.scaled() > 0 {
                possy_pct =
                    cur(1).checked_add(possy_now.checked_sub(cur(1))?.checked_div(cur(5))?)?;
                if possy_pct.scaled() > cur(3).scaled() {
                    possy_pct = cur(3);
                }
                if possy_pct.scaled() < cur(1).scaled() {
                    possy_pct = cur(1);
                }
            }
            let exp_dmg_hp = if tmp.avg_dmg > 0 || m.hp > 0 {
                let doubled = vb::mul_i32(tmp.avg_dmg, 2)?;
                let denom = cur_f(f64::from(doubled))?
                    .checked_add(tmp.hp)?
                    .checked_mul(possy_pct)?;
                cur_round(tmp.exp.checked_div(denom)?, 2)?.checked_mul(cur(100))?
            } else {
                tmp.exp
            };
            script = exp_dmg_hp.checked_mul(lair_pct)?;
        }
        let sv = script.to_f64();
        let text = if sv > 1_000_000_000.0 {
            format!("{}M", vb::format(sv / 1_000_000.0, "#,#")?)
        } else if sv > 1_000_000.0 {
            format!("{}K", vb::format(sv / 1000.0, "#,#")?)
        } else {
            fmt_pos(round_up(sv), "#,#")?
        };
        push(row, text, Some(cur_text(script)));
        row.cells.last_mut().unwrap().fore_color = color_for(env, number, n_party, true)?;
    }

    push(
        row,
        if poss_spawns > 0 {
            poss_spawns.to_string()
        } else {
            String::new()
        },
        Some(poss_spawns.to_string()),
    );

    if nmr >= 1.82 {
        let at = |a: &[Currency]| {
            usize::try_from(number)
                .ok()
                .and_then(|i| a.get(i))
                .copied()
                .ok_or(Vb6Error::Overflow)
        };
        let possy = at(inp.possy)?;
        if possy.scaled() > 0 {
            let chance = at(inp.spawn_chance)?;
            if chance.scaled() > 0 {
                let pct = chance.checked_mul(cur(100))?;
                push(
                    row,
                    format!("{} / {}%", cur_text(possy), cur_text(pct)),
                    Some(cur_text(pct.checked_mul(possy)?)),
                );
            } else {
                push(row, cur_text(possy), Some(cur_text(possy)));
            }
        } else {
            push(row, "", Some("0".to_string()));
        }
    }

    push(
        row,
        if magic_lvl > 0 {
            magic_lvl.to_string()
        } else {
            String::new()
        },
        Some(magic_lvl.to_string()),
    );

    if nmr >= 1.83 {
        push(
            row,
            if m.bs_defense > 0 {
                m.bs_defense.to_string()
            } else {
                String::new()
            },
            Some(m.bs_defense.to_string()),
        );
    }

    push(
        row,
        if m.undead > 0 { "X" } else { "" },
        Some(m.undead.to_string()),
    );

    if !summary.spell_extra_types.is_empty() {
        summary.spell_attack_types = format!(
            "{}+{}",
            summary.spell_attack_types, summary.spell_extra_types
        );
    }
    if !summary.spell_attack_types.is_empty() {
        summary.spell_attack_types = sort_letters_with_separator(&summary.spell_attack_types, "+");
    }
    push(row, summary.spell_attack_types, None);
    Ok(())
}
