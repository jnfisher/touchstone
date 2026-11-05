//! Detail-pane endpoints: ability text, spell numbers at a caster level, monster max damage.
//! The text and numbers come from `touchstone-rules`; this module looks the rows up, applies the
//! slot rules of MME's detail procedures and shapes the JSON.

use std::cell::Cell;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use touchstone_model::{AbilitySlot, Monster, Spell, World};
use touchstone_rules::Profile;
use touchstone_rules::abilities::{NO_ABILITY, item_has_ability, spell_has_ability};
use touchstone_rules::ability_text::{
    AbilityEnv, NameLookup, get_ability_name, get_class_name, get_spell_name,
};
use touchstone_rules::monster_sim::{MonsterSim, MonsterSimInput};
use touchstone_rules::spells::{
    PullArgs, get_spell_duration, get_spell_max_damage, get_spell_min_damage,
};
use touchstone_vb6::{Vb6Error, add_i16, cint, val};

use crate::calc::{ApiResult, bad, json, not_found};
use crate::state::AppState;

type Res<T> = Result<T, Vb6Error>;

/// MME's database name lookups, answered from the loaded tables. A NULL name is the empty string
/// MME's Access column would hold.
struct WorldNames<'a>(&'a World);

impl NameLookup for WorldNames<'_> {
    fn spell_name(&self, n: i32) -> Option<String> {
        self.0
            .get_spell(n)
            .map(|s| s.name.clone().unwrap_or_default())
    }
    fn monster_name(&self, n: i32) -> Option<String> {
        self.0
            .get_monster(n)
            .map(|m| m.name.clone().unwrap_or_default())
    }
    fn item_name(&self, n: i32) -> Option<String> {
        self.0.get_item(n).map(|i| i.name.clone())
    }
    fn class_name(&self, n: i32) -> Option<String> {
        self.0.get_class(n).map(|c| c.name.clone())
    }
}

fn env<'a>(world: &'a World, names: &'a WorldNames<'a>) -> AbilityEnv<'a> {
    AbilityEnv {
        profile: Profile::Stock,
        names,
        textblocks: world,
        spells: &world.spells,
        // MME's defaults: record numbers shown, no quick-spell mode, no global level filter.
        hide_record_numbers: false,
        quick_spell: false,
        global_level_text: None,
        avg_level_max_all_stats: 0.0,
        nest: Cell::new(0),
    }
}

// ---------- ability text ----------

#[derive(Serialize, Debug, PartialEq)]
pub struct AbilityLine {
    pub ability: i32,
    pub value: i32,
    /// `GetAbilityName`.
    pub name: String,
    /// What `GetAbilityStats` returns: the name followed by the value.
    pub text: String,
    /// `text` without the leading name, for a label/value layout.
    pub detail: String,
}

#[derive(Serialize)]
struct AbilitiesJson {
    table: String,
    number: i32,
    lines: Vec<AbilityLine>,
    /// Item ability 59 ("class OK"): the class names, which MME lists apart from the abilities.
    class_ok: Vec<String>,
}

fn line(env: &AbilityEnv, slot: AbilitySlot, text: String) -> Option<AbilityLine> {
    if text.is_empty() {
        return None;
    }
    let name = i16::try_from(slot.ability)
        .map(|n| get_ability_name(env.profile, n, false))
        .unwrap_or_default();
    let detail = text
        .strip_prefix(&name)
        .map_or(text.as_str(), str::trim_start)
        .to_owned();
    Some(AbilityLine {
        ability: slot.ability,
        value: slot.value,
        name,
        text,
        detail,
    })
}

/// `GetAbilityStats` for a slot whose numbers fit MME's Integers; a slot outside that range is a
/// run-time error in MME and prints nothing past the name.
fn stats(env: &AbilityEnv, s: AbilitySlot, percent_column: bool) -> String {
    match (i16::try_from(s.ability), i16::try_from(s.value)) {
        (Ok(a), Ok(v)) => env.get_ability_stats(a, v, true, percent_column),
        (Ok(a), Err(_)) => get_ability_name(env.profile, a, false),
        _ => String::new(),
    }
}

/// `PullItemDetail`'s ability loop: ability 43 prints the spell and its effect with the percent
/// set by the last ability 114 before it; 114 and 59 print nowhere else.
fn item_lines(env: &AbilityEnv, slots: &[AbilitySlot]) -> (Vec<AbilityLine>, Vec<String>) {
    let mut lines = Vec::new();
    let mut class_ok = Vec::new();
    let mut percent = 0;
    for s in slots.iter().filter(|s| s.ability > 0) {
        match s.ability {
            59 => class_ok.push(get_class_name(env.names, s.value)),
            114 => percent = s.value,
            43 => {
                let eq = env.pull_spell_eq(&PullArgs {
                    calc_level: true,
                    spell: s.value,
                    percent_column: true,
                    ..PullArgs::default()
                });
                let mut t = format!(
                    "{} [{}, {eq}",
                    get_ability_name(env.profile, 43, false),
                    get_spell_name(env.names, s.value, env.hide_record_numbers)
                );
                if percent > 0 {
                    t.push_str(&format!(", {percent}%"));
                }
                t.push(']');
                lines.extend(line(env, *s, t));
            }
            _ => lines.extend(line(env, *s, stats(env, *s, true))),
        }
    }
    (lines, class_ok)
}

pub async fn abilities(
    State(state): State<Arc<AppState>>,
    Path((table, number)): Path<(String, i32)>,
) -> ApiResult {
    let loaded = state.current();
    let world = &loaded.world;
    let names = WorldNames(world);
    let env = env(world, &names);
    let missing = || not_found(format!("no {table} record {number}"));
    let mut class_ok = Vec::new();
    // Each pane's MME loop decides which slots print and with which flags.
    let lines: Vec<AbilityLine> = match table.as_str() {
        "items" => {
            let it = world.get_item(number).ok_or_else(missing)?;
            let (l, c) = item_lines(&env, &it.abilities);
            class_ok = c;
            l
        }
        "spells" => plain(
            &env,
            &world.get_spell(number).ok_or_else(missing)?.abilities,
            |a| a != 0,
        ),
        "races" => plain(
            &env,
            &world.get_race(number).ok_or_else(missing)?.abilities,
            |a| a != 0,
        ),
        // Ability 59 only marks the class as usable.
        "classes" => plain(
            &env,
            &world.get_class(number).ok_or_else(missing)?.abilities,
            |a| a != 0 && a != 59,
        ),
        // 146 (guarded by) is a row of its own in MME's monster pane.
        "monsters" => plain(
            &env,
            &world.get_monster(number).ok_or_else(missing)?.abilities,
            |a| a > 0 && a != 146,
        ),
        _ => {
            return Err(bad(
                "table must be items, spells, races, classes or monsters",
            ));
        }
    };
    json(&AbilitiesJson {
        table,
        number,
        lines,
        class_ok,
    })
}

fn plain(env: &AbilityEnv, slots: &[AbilitySlot], keep: impl Fn(i32) -> bool) -> Vec<AbilityLine> {
    slots
        .iter()
        .filter(|s| keep(s.ability))
        .filter_map(|s| line(env, *s, stats(env, *s, false)))
        .collect()
}

// ---------- spell damage and duration ----------

#[derive(Deserialize)]
pub struct EffectQuery {
    level: Option<i32>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Range {
    pub min: i32,
    pub max: i32,
}

#[derive(Serialize)]
struct EffectJson {
    number: i32,
    /// The caster level asked for (the spell's required level when none was given).
    level: i32,
    /// The level MME's formulas use after the spell's required level and cap are applied.
    used_level: i32,
    req_level: i32,
    cap: i32,
    damage: Option<Range>,
    heal: Option<Range>,
    duration: i32,
    /// `PullSpellEQ` at the level.
    effect: String,
}

fn used_level(s: &Spell, level: i32) -> i32 {
    let mut l = level;
    if l > s.cap && s.cap > 0 {
        l = s.cap;
    }
    l.max(s.req_level)
}

pub async fn spell_effect(
    State(state): State<Arc<AppState>>,
    Path(number): Path<i32>,
    Query(q): Query<EffectQuery>,
) -> ApiResult {
    let loaded = state.current();
    let world = &loaded.world;
    let sp = world
        .get_spell(number)
        .ok_or_else(|| not_found(format!("no spell {number}")))?;
    let level = q.level.unwrap_or(sp.req_level);
    let lvl = i16::try_from(level)
        .ok()
        .filter(|l| *l >= 0)
        .ok_or_else(|| bad("level must be between 0 and 32767"))?;
    let range = |heals: bool| {
        // The column and detail callers leave the energy at zero: a full round of energy.
        let min = get_spell_min_damage(&world.spells, number, lvl, &mut 0, false, heals);
        let max = get_spell_max_damage(&world.spells, number, lvl, &mut 0, false, heals);
        (min != 0 || max != 0).then_some(Range { min, max })
    };
    let names = WorldNames(world);
    let effect = env(world, &names).pull_spell_eq(&PullArgs {
        calc_level: true,
        level: lvl,
        spell: number,
        ..PullArgs::default()
    });
    json(&EffectJson {
        number,
        level,
        used_level: used_level(sp, level),
        req_level: sp.req_level,
        cap: sp.cap,
        damage: range(false),
        heal: range(true),
        duration: get_spell_duration(&world.spells, number, lvl, false),
        effect,
    })
}

// ---------- monster max damage ----------

#[derive(Serialize)]
struct MaxDamageJson {
    number: i32,
    max_damage: i64,
    energy_per_round: i16,
}

/// `ExtractNumbersFromString(NMR Version)`: "v1.8.3" is 1.83.
fn nmr_ver(s: &str) -> f64 {
    let mut out = String::new();
    let mut seen_dot = false;
    for c in s.chars() {
        match c {
            '0'..='9' => out.push(c),
            '.' if !out.is_empty() && !seen_dot => {
                out.push(c);
                seen_dot = true;
            }
            '-' if out.is_empty() => out.push(c),
            _ if out == "-" => out.clear(),
            _ if !out.is_empty() => break,
            _ => {}
        }
    }
    val(&out)
}

fn i16c(v: i32) -> Res<i16> {
    i16::try_from(v).map_err(|_| Vb6Error::Overflow)
}

/// `CalculateMonsterItemBonuses` (Integer): the sum of the abilities on the monster's weapon and,
/// weighted by drop percent, on its drops, for items with Limit 0. An overflow leaves the sum so
/// far, as MME's handler does.
fn item_bonuses(world: &World, m: &Monster, abilities: &[i16]) -> i16 {
    fn inner(world: &World, m: &Monster, abilities: &[i16], acc: &mut i16) -> Res<()> {
        let has = |item: i32, ab: i16| {
            let it = world.get_item(item).filter(|i| i.limit == 0)?;
            let t = item_has_ability(Profile::Stock, Some(&it.abilities[..]), item, ab);
            (t != NO_ABILITY).then_some(t)
        };
        if m.weapon > 0 {
            for &ab in abilities {
                if let Some(t) = has(m.weapon, ab) {
                    *acc = add_i16(*acc, i16c(t)?)?;
                }
            }
        }
        for d in m.drops.iter().filter(|d| d.item > 0) {
            for &ab in abilities {
                if let Some(t) = has(d.item, ab) {
                    let pct = f64::from(d.pct.min(100)) / 100.0;
                    *acc = cint(f64::from(*acc) + f64::from(i16c(t)?) * pct)?;
                }
            }
        }
        Ok(())
    }
    let mut acc = 0;
    let _ = inner(world, m, abilities, &mut acc);
    acc
}

/// The damage-bearing spell abilities in the order `PopulateMonsterDataToAttackSim` tests them
/// (1 damage, 17 damage vs MR, 8 drain) with the resist flag each one sets.
const DAMAGE_ABILITIES: [(i16, i16); 3] = [(1, 0), (17, 1), (8, 0)];

/// `SpellHasAbility` on a spell row.
fn spell_has(sp: &Spell, ability: i16) -> i32 {
    spell_has_ability(Profile::Stock, Some(&sp.abilities[..]), sp.number, ability)
}

/// Min and max of a spell attack or between-round spell: the ability's own value when it has one,
/// else `GetSpellMinDamage`/`GetSpellMaxDamage` at `level` for a monster (energy -1).
fn spell_dmg(world: &World, sp: &Spell, value: i32, level: i16) -> Res<(i16, i16)> {
    if value > 0 {
        let v = i16c(value)?;
        return Ok((v, v));
    }
    let min = get_spell_min_damage(&world.spells, sp.number, level, &mut -1, true, false);
    let max = get_spell_max_damage(&world.spells, sp.number, level, &mut -1, true, false);
    Ok((i16c(min)?, i16c(max)?))
}

/// Port of modMMudDatabase PopulateMonsterDataToAttackSim (stock branch), on top of the
/// `ResetValues` state `CalculateMonsterAvgDmg` starts from. Attack and spell names, the evil
/// flag and the spell types are filled too, though `GetMaxDamage` reads none of them. An
/// overflow stops the fill where MME's handler would leave it.
fn populate(world: &World, m: &Monster) -> MonsterSimInput {
    let mut inp = MonsterSimInput {
        atk_spell_type: [4; 5],
        atk_hit_spell_type: [4; 5],
        between_round_spell_type: [4; 5],
        user_mr: 50,
        ..MonsterSimInput::default()
    };
    let _ = populate_inner(world, m, &mut inp);
    inp
}

fn populate_inner(world: &World, m: &Monster, inp: &mut MonsterSimInput) -> Res<()> {
    let nmr = nmr_ver(&world.info.nmr_version);
    inp.energy_per_round = if nmr >= 1.71 { i16c(m.energy)? } else { 1000 };
    // 4 is MaxDamage; 22, 105 and 106 are the accuracy abilities.
    let dmg_bonus = item_bonuses(world, m, &[4]);
    let acc_bonus = item_bonuses(world, m, &[22, 105, 106]);
    inp.mob_is_evil = matches!(m.align, 1 | 2 | 5 | 6);
    let spells = &world.spells;

    for (x, a) in m.attacks.iter().enumerate() {
        if !(a.att_type > 0 && a.att_type < 4) {
            continue;
        }
        inp.atk_type[x] = i16c(a.att_type)?;
        inp.atk_energy[x] = i16c(a.energy)?;
        inp.atk_chance[x] = i16c(a.pct)?;
        if a.att_type == 2 {
            let Some(sp) = world.get_spell(a.acc) else {
                continue;
            };
            let lvl = i16c(a.max)?;
            // An area spell in a plain attack slot using ability 1 is never cast by the game.
            if sp.targets == 12
                && get_spell_duration(spells, a.acc, lvl, true) == 0
                && spell_has(sp, 1) > -1
            {
                inp.atk_duration[x] = 0;
                inp.atk_min[x] = 0;
                inp.atk_max[x] = 0;
                inp.atk_energy[x] = 0;
                continue;
            }
            if nmr >= 1.8 {
                inp.atk_resist[x] = i16c(sp.type_of_resists)?;
            }
            inp.atk_spell_type[x] = i16c(sp.att_type)?;
            inp.atk_duration[x] = i16c(get_spell_duration(spells, a.acc, lvl, true))?;
            inp.atk_min[x] = 0;
            inp.atk_max[x] = 0;
            for (ab, mr) in DAMAGE_ABILITIES {
                let t = spell_has(sp, ab);
                if t >= 0 {
                    inp.atk_mr_dmg_resist[x] = mr;
                    (inp.atk_min[x], inp.atk_max[x]) = spell_dmg(world, sp, t, lvl)?;
                }
            }
            inp.atk_success[x] = i16c(a.min)?;
        } else {
            inp.atk_min[x] = add_i16(i16c(a.min)?, dmg_bonus)?;
            inp.atk_max[x] = add_i16(i16c(a.max)?, dmg_bonus)?;
            inp.atk_success[x] = add_i16(i16c(a.acc)?, acc_bonus)?;
            if a.hit_spell > 0 {
                let Some(sp) = world.get_spell(a.hit_spell) else {
                    continue;
                };
                if nmr >= 1.8 {
                    inp.atk_resist[x] = i16c(sp.type_of_resists)?;
                }
                inp.atk_hit_spell_type[x] = i16c(sp.att_type)?;
                inp.atk_duration[x] = i16c(get_spell_duration(spells, a.hit_spell, 0, false))?;
                // Ability 1 wins over 17; the numbers are the spell's own at level 0, energy 0.
                let hit = |ab| spell_has(sp, ab) >= 0;
                let dmg = |heal_max: bool| {
                    let f = if heal_max {
                        get_spell_max_damage
                    } else {
                        get_spell_min_damage
                    };
                    i16c(f(spells, a.hit_spell, 0, &mut 0, false, false))
                };
                if hit(1) || hit(17) {
                    inp.atk_mr_dmg_resist[x] = i16::from(!hit(1));
                    inp.atk_hit_spell_min[x] = dmg(false)?;
                    inp.atk_hit_spell_max[x] = dmg(true)?;
                } else {
                    inp.atk_hit_spell_min[x] = 0;
                    inp.atk_hit_spell_max[x] = 0;
                }
            }
        }
    }

    for (x, ms) in m.mid_spells.iter().enumerate() {
        if ms.spell <= 0 {
            continue;
        }
        let Some(sp) = world.get_spell(ms.spell) else {
            continue;
        };
        let lvl = i16c(ms.lvl)?;
        inp.between_round_spell_type[x] = i16c(sp.att_type)?;
        if nmr >= 1.8 {
            inp.between_round_resist_type[x] = i16c(sp.type_of_resists)?;
        }
        inp.between_round_chance[x] = i16c(ms.pct)?;
        inp.between_round_duration[x] = i16c(get_spell_duration(spells, ms.spell, lvl, true))?;
        for (ab, mr) in DAMAGE_ABILITIES {
            let t = spell_has(sp, ab);
            if t >= 0 {
                inp.between_round_resist_dmg_mr[x] = mr;
                (inp.between_round_min[x], inp.between_round_max[x]) =
                    spell_dmg(world, sp, t, lvl)?;
            }
        }
    }
    Ok(())
}

pub async fn monster_max_damage(
    State(state): State<Arc<AppState>>,
    Path(number): Path<i32>,
) -> ApiResult {
    let loaded = state.current();
    let world = &loaded.world;
    let m = world
        .get_monster(number)
        .ok_or_else(|| not_found(format!("no monster {number}")))?;
    let input = populate(world, m);
    let energy_per_round = input.energy_per_round;
    json(&MaxDamageJson {
        number,
        max_damage: MonsterSim::new(input).max_damage(),
        energy_per_round,
    })
}
