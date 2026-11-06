//! The Monster Attack Simulator window (frmMonsterAttackSim): `POST /api/calc/monster-sim` runs
//! `cmdRunSim_Click` over the ported simulator and answers what the form's labels and boxes show;
//! `POST /api/calc/monster-sim/defender` answers the defense boxes alone, as `cmdResetUserDefs_Click`
//! fills them from the current character.
//!
//! MME seeds the generator with `Randomize`; this endpoint seeds it itself so a run can be repeated.

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use touchstone_model::World;
use touchstone_rules::accuracy::{get_dodge_cap, get_hit_cap, get_hit_min, get_spell_hit_cap};
use touchstone_rules::monster_glue::{
    CharSimInputs, GlueEnv, populate_monster_data_to_attack_sim, reset_values,
};
use touchstone_rules::monster_sim::{MonsterSim, MonsterSimInput};
use touchstone_rules::{Profile, RulesError};
use touchstone_vb6::{Currency, Rnd, clng, cstr, round, val};

use std::sync::Arc;

use axum::extract::State;

use crate::calc::{ApiResult, Body, Text, bad, json, vb_error_text};
use crate::character::{CharacterDto, refresh};
use crate::detail::nmr_ver;
use crate::state::AppState;

/// The generator state VB6 starts with before any `Randomize`; the seed when a request has none.
pub const DEFAULT_SEED: u32 = 0x5_0000;
/// `cmdRunSim_Click` cuts the rounds box to this.
pub const MAX_ROUNDS: f64 = 500_000.0;
/// The form fixes the combat log at 100 rounds; a request may ask for more, up to this.
pub const MAX_LOG_ROUNDS: i32 = 1000;
const DEFAULT_LOG_ROUNDS: i32 = 100;
const DEFAULT_ROUNDS: &str = "500";

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct DefenderReq {
    /// The Character tab's character, evaluated here; its numbers fill the boxes below.
    pub character: Option<CharacterDto>,
    /// `cmbGlobalClass(0)`: the class whose hit minimum and dodge caps apply. Typed mode only.
    pub class: Option<i32>,
    pub ac: Text,
    pub dr: Text,
    pub dodge: Text,
    pub mr: Text,
    pub anti_magic: bool,
    pub prot_evil: Text,
    /// GreaterMUD only; accepted and ignored.
    pub vile_ward: Text,
    pub resist_cold: Text,
    pub resist_fire: Text,
    pub resist_stone: Text,
    pub resist_lightning: Text,
    pub resist_water: Text,
}

#[derive(Debug, Deserialize)]
pub struct SimReq {
    pub monster: i32,
    #[serde(default = "default_rounds")]
    pub rounds: Text,
    pub seed: Option<u32>,
    /// `chkDynamicRounds`: run until the average settles.
    #[serde(default)]
    pub dynamic: bool,
    pub log_rounds: Option<i32>,
    /// `chkCombatMaxRoundOnly`: keep only the log of the worst round.
    #[serde(default)]
    pub log_max_round_only: bool,
    /// `chkHideEnergy`.
    #[serde(default)]
    pub hide_energy: bool,
    /// `chkAlwaysDodge`: dodge is rolled before armour class.
    #[serde(default)]
    pub always_dodge: bool,
    #[serde(default)]
    pub defender: DefenderReq,
}

fn default_rounds() -> Text {
    Text(DEFAULT_ROUNDS.to_owned())
}

/// The defense text boxes as the form holds them.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Boxes {
    pub ac: String,
    pub dr: String,
    pub dodge: String,
    pub mr: String,
    pub anti_magic: bool,
    pub prot_evil: String,
    pub vile_ward: String,
    pub resist_cold: String,
    pub resist_fire: String,
    pub resist_stone: String,
    pub resist_lightning: String,
    pub resist_water: String,
}

impl Boxes {
    /// `cmdResetUserDefs_Click(0)`.
    fn defaults() -> Self {
        let zero = || "0".to_owned();
        Self {
            ac: zero(),
            dr: zero(),
            dodge: zero(),
            mr: "50".to_owned(),
            anti_magic: false,
            prot_evil: zero(),
            vile_ward: zero(),
            resist_cold: zero(),
            resist_fire: zero(),
            resist_stone: zero(),
            resist_lightning: zero(),
            resist_water: zero(),
        }
    }

    /// `cmdResetUserDefs_Click(1)` for a party of one.
    fn from_character(c: &CharSimInputs) -> Result<Self, (StatusCode, String)> {
        let rounded = |x: f64| round(x, 0).map(cstr).map_err(fail);
        // A tag is copied as text; an empty one reads as 0 when the run applies it.
        let tag = |t: Option<f64>| t.map_or_else(String::new, cstr);
        Ok(Self {
            ac: rounded(c.ac)?,
            dr: rounded(c.dr)?,
            dodge: rounded(c.dodge)?,
            mr: rounded(c.mr)?,
            anti_magic: c.anti_magic,
            prot_evil: cstr(c.prot_evil),
            vile_ward: "0".to_owned(),
            resist_cold: tag(c.resist_col),
            resist_fire: tag(c.resist_fir),
            resist_stone: tag(c.resist_sto),
            resist_lightning: tag(c.resist_lit),
            resist_water: tag(c.resist_wat),
        })
    }

    fn from_typed(d: &DefenderReq) -> Self {
        Self {
            ac: d.ac.0.clone(),
            dr: d.dr.0.clone(),
            dodge: d.dodge.0.clone(),
            mr: d.mr.0.clone(),
            anti_magic: d.anti_magic,
            prot_evil: d.prot_evil.0.clone(),
            vile_ward: d.vile_ward.0.clone(),
            resist_cold: d.resist_cold.0.clone(),
            resist_fire: d.resist_fire.0.clone(),
            resist_stone: d.resist_stone.0.clone(),
            resist_lightning: d.resist_lightning.0.clone(),
            resist_water: d.resist_water.0.clone(),
        }
    }
}

fn fail(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

/// A box above 0 replaces the simulator's default; a value past a Long is the form's error 6.
fn positive(label: &str, text: &str) -> Result<Option<i32>, (StatusCode, String)> {
    let v = val(text);
    if v > 0.0 {
        return clng(v)
            .map(Some)
            .map_err(|_| bad(format!("{label} is too large")));
    }
    Ok(None)
}

// ---------- the per-attack numbers (cmdRunSim_Click's result block) ----------

fn scaled(n: i64) -> Currency {
    Currency::from_scaled(n * 10_000)
}

/// `Round(c, digits)` on a Currency: exact decimal, half to even.
fn round_cur(c: Currency, digits: u32) -> Currency {
    if digits >= 4 {
        return c;
    }
    let q = 10i64.pow(4 - digits);
    let (n, r) = (c.scaled().div_euclid(q), c.scaled().rem_euclid(q));
    let n = match (2 * r).cmp(&q) {
        std::cmp::Ordering::Greater => n + 1,
        std::cmp::Ordering::Equal if n % 2 != 0 => n + 1,
        _ => n,
    };
    Currency::from_scaled(n * q)
}

/// `Round(a / b, 3) * 100`: a share of one as a percentage with one decimal.
fn percent(a: Currency, b: Currency) -> Result<f64, (StatusCode, String)> {
    let share = round_cur(a.checked_div(b).map_err(fail)?, 3);
    Ok(share.checked_mul(scaled(100)).map_err(fail)?.to_f64())
}

#[derive(Debug, Serialize, PartialEq)]
pub struct AttackStats {
    /// How often the attack was tried, of all attempts (`txtStatTrueCast`).
    pub true_cast_pct: f64,
    /// `txtStatAvgRound`: damage per landed hit.
    pub avg_per_hit: f64,
    /// `txtStatSuccess`.
    pub success_pct: f64,
    /// `txtStatDmgResist`: the share of the damage that armour or resists took.
    pub dmg_resisted_pct: f64,
    /// `txtStatResistDodge`: tries the character dodged or resisted.
    pub resist_dodge_pct: f64,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct AttackRow {
    pub index: usize,
    /// `lblAttack`: the attack's name, empty for an unnamed one.
    pub name: String,
    /// `nAtkType`: 0 none, 1 melee, 2 spell, 3 other; the form shows numbers only above 0.
    pub kind: i16,
    pub stats: Option<AttackStats>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Summary {
    /// Total damage over the rounds run.
    pub total_damage: f64,
    /// `nAverageDamage`: the class's own average. It divides by one round more than ran, as MME's
    /// `RunSim` does after its For loop, so it differs slightly from `avg_damage`.
    pub average_damage: f64,
    /// What the label shows: the total over the rounds box.
    pub avg_damage: f64,
    pub max_damage: i64,
    pub max_seen: i64,
    pub avg_phys: f64,
    pub avg_spell: f64,
    /// `lblResultsAvgDmg`, `lblResultsMaxRound`, `lblResultsAttBreakdown`.
    pub avg_caption: String,
    pub max_caption: String,
    pub breakdown_caption: String,
}

fn attack_stats(sim: &MonsterSim, x: usize) -> Result<AttackStats, (StatusCode, String)> {
    let attempted = sim.stat_atk_attempted[x];
    let hits = sim.stat_atk_hits[x];
    let total = sim.stat_atk_total_damage[x];
    let resisted = sim.stat_atk_dmg_resisted[x];
    let dodged = sim.stat_atk_attempt_dodged_or_resisted[x];
    let zero = Currency::from_scaled(0);
    let attacks = scaled(sim.total_attacks);

    let avg_per_hit = if total > zero && hits != zero {
        round_cur(total.checked_div(hits).map_err(fail)?, 0).to_f64()
    } else {
        0.0
    };
    let success = if attempted > zero {
        percent(hits, attempted)?
    } else {
        0.0
    };
    let dmg_resisted = if resisted == zero {
        0.0
    } else if total == zero {
        100.0
    } else {
        percent(resisted, resisted.checked_add(total).map_err(fail)?)?
    };
    // A spell always shows the share; any other attack only when something was dodged.
    let resist_dodge = if attempted > zero && (sim.input.atk_type[x] == 2 || dodged > zero) {
        percent(dodged, attempted)?
    } else {
        0.0
    };
    Ok(AttackStats {
        true_cast_pct: percent(attempted, attacks)?,
        avg_per_hit,
        success_pct: success,
        dmg_resisted_pct: dmg_resisted,
        resist_dodge_pct: resist_dodge,
    })
}

#[derive(Debug, Serialize)]
pub struct SimError {
    pub number: u32,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct MonsterRef {
    pub number: i32,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct SimJson {
    pub monster: MonsterRef,
    /// The rounds run (a dynamic run stops where the average settles).
    pub rounds: i32,
    /// Whether `RunSim` was called: the rounds box was above 0.
    pub ran: bool,
    pub seed: u32,
    /// The generator state after the run.
    pub rng_after: u32,
    pub energy_per_round: i16,
    pub defender: Boxes,
    pub class: i32,
    pub attacks: Vec<AttackRow>,
    pub summary: Option<Summary>,
    /// `txtCombatLog`: trimmed, CRLF between lines.
    pub combat_log: String,
    pub error: Option<SimError>,
}

/// `cmdRunSim_Click` over `world`, for the monster and defender in `req`, with `chr` the evaluated
/// character when the request has one.
pub fn run(
    profile: Profile,
    world: &World,
    req: &SimReq,
    chr: Option<&CharSimInputs>,
) -> Result<SimJson, (StatusCode, String)> {
    let Some(monster) = world.get_monster(req.monster).filter(|_| req.monster > 0) else {
        return Err(bad(format!("no monster {}", req.monster)));
    };
    let class = match (chr, req.defender.class) {
        (Some(c), _) => c.class,
        (None, c) => c.unwrap_or(0),
    };
    if class > 0 && world.get_class(class).is_none() {
        return Err(bad(format!("no class {class}")));
    }
    let log_rounds = req.log_rounds.unwrap_or(DEFAULT_LOG_ROUNDS);
    if !(0..=MAX_LOG_ROUNDS).contains(&log_rounds) {
        return Err(bad(format!(
            "log_rounds must be 0 to {MAX_LOG_ROUNDS}, not {log_rounds}"
        )));
    }

    let boxes = match chr {
        Some(c) => Boxes::from_character(c)?,
        None => Boxes::from_typed(&req.defender),
    };
    let rounds_box = val(&req.rounds.0).min(MAX_ROUNDS);
    let rounds = clng(rounds_box).map_err(|_| bad("rounds is out of range"))?;

    let env = GlueEnv {
        profile,
        nmr_ver: nmr_ver(&world.info.nmr_version),
    };
    let mut input = MonsterSimInput::default();
    reset_values(&mut input);
    input.combat_log_max_rounds = log_rounds;
    input.combat_log_max_round_only = req.log_max_round_only;
    input.number_of_rounds = rounds;
    input.user_mr = 50;
    input.greater_mud = false;
    // The caps come from the main window's class (stock engine: its own constants).
    input.hit_min = get_hit_min(profile, class) as i16;
    input.hit_cap = get_hit_cap(profile) as i16;
    input.spell_hit_cap = get_spell_hit_cap(profile) as i16;
    input.dodge_softcap = get_dodge_cap(profile, class, true) as i16;
    input.dodge_cap = get_dodge_cap(profile, class, false) as i16;
    input.dynamic_calc = req.dynamic;
    input.dynamic_calc_difference = Currency::from_scaled(1);
    input.hide_energy_info = req.hide_energy;
    input.dodge_before_ac = req.always_dodge;
    if let Some(v) = positive("AC", &boxes.ac)? {
        input.user_ac = v;
    }
    if let Some(v) = positive("DR", &boxes.dr)? {
        input.user_dr = v;
    }
    if let Some(v) = positive("Dodge", &boxes.dodge)? {
        input.user_dodge = v;
    }
    if let Some(v) = positive("MR", &boxes.mr)? {
        input.user_mr = v;
    }
    if let Some(v) = positive("Prot. Evil", &boxes.prot_evil)? {
        input.user_prot_evil = v;
    }
    for (label, text, slot) in [
        ("Cold resist", &boxes.resist_cold, &mut input.user_rcol),
        ("Fire resist", &boxes.resist_fire, &mut input.user_rfir),
        ("Stone resist", &boxes.resist_stone, &mut input.user_rsto),
        (
            "Lightning resist",
            &boxes.resist_lightning,
            &mut input.user_rlit,
        ),
        ("Water resist", &boxes.resist_water, &mut input.user_rwat),
    ] {
        if let Some(v) = positive(label, text)? {
            *slot = v;
        }
    }
    if boxes.anti_magic {
        input.user_anti_magic = 1;
    }

    populate_monster_data_to_attack_sim(world, &env, req.monster, &mut input);

    let seed = req.seed.unwrap_or(DEFAULT_SEED);
    let mut rnd = Rnd::from_state(seed);
    let mut sim = MonsterSim::new(input);
    let ran = sim.input.number_of_rounds > 0;
    let mut error = None;
    if ran {
        match sim.run(&mut rnd) {
            Ok(()) => {}
            Err(RulesError::Vb6(code)) => {
                let number = u32::try_from(code).unwrap_or(0);
                error = Some(SimError {
                    number,
                    text: vb_error_text(number),
                });
            }
            Err(e) => return Err(fail(e)),
        }
    }

    let attacks = (0..5)
        .map(|x| {
            let kind = sim.input.atk_type[x];
            Ok(AttackRow {
                index: x,
                name: sim.input.atk_name[x].clone(),
                kind,
                stats: if ran && sim.total_attacks > 0 && kind > 0 {
                    Some(attack_stats(&sim, x)?)
                } else {
                    None
                },
            })
        })
        .collect::<Result<Vec<_>, (StatusCode, String)>>()?;

    let n = sim.input.number_of_rounds;
    let summary = if ran && sim.total_attacks > 0 && n > 0 {
        // The label divides by the rounds box, not by the class's own count.
        let avg = round_cur(
            sim.total_damage
                .checked_div(scaled(i64::from(n)))
                .map_err(fail)?,
            1,
        );
        let max = sim.max_damage();
        let phys = round_cur(sim.average_damage_phys, 0);
        let spell = round_cur(sim.average_damage_spell, 0);
        Some(Summary {
            total_damage: sim.total_damage.to_f64(),
            average_damage: sim.average_damage.to_f64(),
            avg_damage: avg.to_f64(),
            max_damage: max,
            max_seen: sim.max_round_damage,
            avg_phys: phys.to_f64(),
            avg_spell: spell.to_f64(),
            avg_caption: format!("AVG Dmg/Rnd: {}", cstr(avg.to_f64())),
            max_caption: format!("Max/Seen: {max}/{}", sim.max_round_damage),
            breakdown_caption: format!(
                "(Physical/Spell: {}/{})",
                cstr(phys.to_f64()),
                cstr(spell.to_f64())
            ),
        })
    } else {
        None
    };

    Ok(SimJson {
        monster: MonsterRef {
            number: monster.number,
            name: monster.name.clone().unwrap_or_default(),
        },
        rounds: n,
        ran,
        seed: seed & 0xFF_FFFF,
        rng_after: rnd.state(),
        energy_per_round: sim.input.energy_per_round,
        defender: boxes,
        class,
        attacks,
        summary,
        combat_log: sim.combat_log.trim_matches(' ').to_owned(),
        error,
    })
}

pub async fn sim_route(State(app): State<Arc<AppState>>, Body(req): Body<SimReq>) -> ApiResult {
    let loaded = app.current();
    let profile = loaded.profile;
    let chr = match &req.defender.character {
        Some(dto) => Some(
            refresh(profile, &loaded.world, dto, app.hide_record_numbers())?
                .1
                .sim_inputs(),
        ),
        None => None,
    };
    // Half a million rounds is real work; keep it off the async threads.
    let out = tokio::task::spawn_blocking(move || run(profile, &loaded.world, &req, chr.as_ref()))
        .await
        .map_err(fail)??;
    json(&out)
}

#[derive(Debug, Deserialize)]
pub struct DefenderQuery {
    pub character: CharacterDto,
}

#[derive(Debug, Serialize)]
pub struct DefenderJson {
    pub class: i32,
    pub defender: Boxes,
}

pub async fn defender_route(
    State(app): State<Arc<AppState>>,
    Body(req): Body<DefenderQuery>,
) -> ApiResult {
    let loaded = app.current();
    let profile = loaded.profile;
    let chr = refresh(
        profile,
        &loaded.world,
        &req.character,
        app.hide_record_numbers(),
    )?
    .1
    .sim_inputs();
    json(&DefenderJson {
        class: chr.class,
        defender: Boxes::from_character(&chr)?,
    })
}

/// `cmdResetUserDefs_Click(0)`, for the page's reset button without a round trip.
#[must_use]
pub fn default_boxes() -> Boxes {
    Boxes::defaults()
}
