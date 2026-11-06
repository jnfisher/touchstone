//! Detail-pane endpoints: ability text, spell numbers at a caster level, monster max damage.
//! The text and numbers come from `touchstone-rules`; this module looks the rows up, applies the
//! slot rules of MME's detail procedures and shapes the JSON.

use std::cell::Cell;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use touchstone_model::{AbilitySlot, Spell, World};
use touchstone_rules::Profile;
use touchstone_rules::ability_text::{
    AbilityEnv, NameLookup, get_ability_name, get_class_name, get_spell_name,
};
use touchstone_rules::monster_glue::{GlueEnv, calculate_monster_avg_dmg};
use touchstone_rules::monster_sim::MonsterSimInput;
use touchstone_rules::spells::{
    PullArgs, get_spell_duration, get_spell_max_damage, get_spell_min_damage,
};
use touchstone_vb6::Rnd;
use touchstone_vb6::val;

use crate::calc::{ApiResult, bad, json, not_found};
use crate::state::AppState;

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
pub(crate) fn nmr_ver(s: &str) -> f64 {
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

pub async fn monster_max_damage(
    State(state): State<Arc<AppState>>,
    Path(number): Path<i32>,
) -> ApiResult {
    let loaded = state.current();
    let world = &loaded.world;
    let m = world
        .get_monster(number)
        .ok_or_else(|| not_found(format!("no monster {number}")))?;
    let env = GlueEnv {
        profile: Profile::Stock,
        nmr_ver: nmr_ver(&world.info.nmr_version),
    };
    let mut sim = MonsterSimInput::default();
    // 0 rounds: MME's CalculateMonsterAvgDmg then skips the simulated run and only takes the maximum.
    let ret = calculate_monster_avg_dmg(world, &env, m.number, 0, &mut sim, &mut Rnd::default())
        .map_err(|e| bad(format!("monster {number}: {e}")))?;
    json(&MaxDamageJson {
        number,
        max_damage: ret.max_damage.scaled() / 10_000,
        energy_per_round: sim.energy_per_round,
    })
}
