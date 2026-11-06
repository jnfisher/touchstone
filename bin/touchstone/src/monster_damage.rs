//! MME's calculated monster damage: `CalculateMonsterDamageVsCharALL`, `ClearMonsterDamageVsCharALL` and the
//! globals around them (`sMonsterDamageVsCharDefenseConfig`, `bMonsterDamageVsPartyCalculated`,
//! `bDontPromptCalc*MonsterDamage`). Card P4-30.
//!
//! The arrays live in [`MonsterDamage`], one field of `AppState`, tagged with the load they were calculated for, so
//! a reload of the data starts again from "nothing calculated". The Monsters list and pane read a [`Snapshot`] of it.
//! MME keeps the arrays across character edits and compares the stored defense description with the current one (the
//! "sims may be stale" note, the prompt before a lair list); the server does the same, as every request carries the
//! character. The "already asked" flags MME resets whenever the description or the party boxes change are stored as
//! the description / box key at the time of the answer.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use touchstone_model::World;
use touchstone_rules::Profile;
use touchstone_rules::character::{PartyFilter, set_char_defense_description};
use touchstone_rules::detail::monster::DamageCaches;
use touchstone_rules::filters::DamagePrompts;
use touchstone_rules::monster_glue::{
    CharSimInputs, GlueEnv, calculate_monster_damage_vs_char_all_with,
};
use touchstone_rules::monster_sim::MonsterSimInput;
use touchstone_vb6::{Currency, Rnd};

use crate::calc::{ApiResult, Body, json};
use crate::character::CharacterDto;
use crate::evaluate::{
    Ctx, EvaluateBody, GlobalFilterBody, MonsterControls, Options, PartyBoxes, prepare,
    run_settings, uncalculated, with_monster_env,
};
use crate::state::{AppState, Loaded};

/// Which array a request is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// `nMonsterDamageVsChar`.
    Char,
    /// `nMonsterDamageVsParty`.
    Party,
}

/// The party boxes `txtMonsterLairFilter_KeyUp` watches (AC, DR, MR, dodge and anti-magic): a change in any of them
/// resets `bDontPromptCalcPartyMonsterDamage`. The party size is not among them.
pub fn party_key(p: &PartyFilter) -> String {
    [1, 2, 3, 4, 6]
        .map(|k| p.lair_text[k].as_str())
        .join("\u{1f}")
}

/// [`party_key`] of the boxes as a request carries them.
pub fn party_boxes_key(p: &PartyBoxes) -> String {
    let controls = MonsterControls {
        party: p.clone(),
        ..MonsterControls::default()
    };
    party_key(&controls.party())
}

/// The module globals, as one request sees them.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// `nMonsterDamageVsChar` and `nMonsterDamageVsParty`, indexed by monster number; -1 is "not calculated".
    pub vs_char: Vec<Currency>,
    pub vs_party: Vec<Currency>,
    /// `nMonsterDamageVsDefault`. MME fills it lazily with the Monsters table's average (`GetMonsterAvgDmgFromDB`),
    /// which is also the fallback when it is not filled, so the server never writes it.
    pub vs_default: Vec<Currency>,
    /// `sMonsterDamageVsCharDefenseConfig`.
    pub char_config: String,
    /// The defense description at the moment `bDontPromptCalcCharMonsterDamage` was set.
    pub char_asked: Option<String>,
    /// `bMonsterDamageVsPartyCalculated`.
    pub party_calculated: bool,
    /// The party-box key at the moment `bDontPromptCalcPartyMonsterDamage` was set.
    pub party_asked: Option<String>,
    /// Counts every change, for cache keys.
    pub generation: u64,
}

impl Snapshot {
    /// What MME starts with (and `ClearMonsterDamageVsCharALL` leaves).
    pub fn uncalculated(world: &World) -> Self {
        Self {
            vs_char: uncalculated(world),
            vs_party: uncalculated(world),
            vs_default: uncalculated(world),
            char_config: String::new(),
            char_asked: None,
            party_calculated: false,
            party_asked: None,
            generation: 0,
        }
    }

    /// The inputs of `FilterMonsters`' "calculate first?" questions for the current character `desc` and party boxes.
    pub fn prompts(&self, desc: &str, party: &PartyFilter) -> DamagePrompts {
        DamagePrompts {
            vs_char_config: self.char_config.clone(),
            char_defense_description: desc.to_owned(),
            dont_prompt_char: self.char_asked.as_deref() == Some(desc),
            party_calculated: self.party_calculated,
            dont_prompt_party: self.party_asked.as_deref() == Some(party_key(party).as_str()),
        }
    }

    /// `bDontPromptCalcPartyMonsterDamage` for the pane's "may be incomplete" note.
    pub fn dont_prompt_party(&self, party: &PartyFilter) -> bool {
        self.party_asked.as_deref() == Some(party_key(party).as_str())
    }
}

struct Running {
    mode: Mode,
    load: u64,
    total: usize,
    done: Arc<AtomicUsize>,
    /// Set by `POST /api/monster-damage/cancel`; the loop looks at it between monsters.
    cancel: Arc<AtomicBool>,
    started: Instant,
}

/// The damage store of `AppState`.
#[derive(Default)]
pub struct MonsterDamage {
    stored: Mutex<Option<(u64, Snapshot)>>,
    running: Mutex<Option<Running>>,
}

impl MonsterDamage {
    /// The state for load number `load`; a store made for another load (or none) is replaced by "nothing calculated".
    pub fn snapshot(&self, load: u64, world: &World) -> Snapshot {
        let mut g = self
            .stored
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Self::fresh(&mut g, load, world).clone()
    }

    /// The change counter alone, for the evaluate cache key.
    pub fn generation(&self, load: u64) -> u64 {
        self.stored
            .lock()
            .ok()
            .and_then(|g| {
                g.as_ref()
                    .filter(|(l, _)| *l == load)
                    .map(|(_, s)| s.generation)
            })
            .unwrap_or(0)
    }

    fn fresh<'a>(g: &'a mut Option<(u64, Snapshot)>, load: u64, world: &World) -> &'a mut Snapshot {
        if g.as_ref().is_none_or(|(l, _)| *l != load) {
            *g = Some((load, Snapshot::uncalculated(world)));
        }
        &mut g.as_mut().expect("just filled").1
    }

    /// Changes the state for `load` under the lock and counts the change.
    pub fn update(&self, load: u64, world: &World, f: impl FnOnce(&mut Snapshot)) {
        let mut g = self
            .stored
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let s = Self::fresh(&mut g, load, world);
        f(s);
        s.generation += 1;
    }

    /// Keeps what a monster pane wrote into the arrays (a monster it simulated on the spot): MME's arrays are global, so
    /// the next pane and list read those values. Only entries that changed are taken, so a pane that wrote nothing
    /// leaves the counter alone.
    pub fn keep_written(&self, load: u64, world: &World, w: &DamageCaches) {
        let mut g = self
            .stored
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let s = Self::fresh(&mut g, load, world);
        let mut changed = false;
        for (to, from) in [(&mut s.vs_char, &w.vs_char), (&mut s.vs_party, &w.vs_party)] {
            for (slot, v) in to.iter_mut().zip(from) {
                if slot != v {
                    *slot = *v;
                    changed = true;
                }
            }
        }
        if changed {
            s.generation += 1;
        }
    }

    /// `ClearMonsterDamageVsCharALL`: for one array, or (the Options menu's Clear) for both.
    pub fn clear(&self, load: u64, world: &World, mode: Option<Mode>) {
        self.update(load, world, |s| {
            let fresh = Snapshot::uncalculated(world);
            if mode != Some(Mode::Party) {
                s.vs_char = fresh.vs_char;
                s.char_config = fresh.char_config;
                s.char_asked = None;
            }
            if mode != Some(Mode::Char) {
                s.vs_party = fresh.vs_party;
                s.party_calculated = false;
                s.party_asked = None;
            }
        });
    }

    /// The question was answered No: do not ask again until the description (or the party boxes) change.
    pub fn dismiss(&self, load: u64, world: &World, mode: Mode, key: String) {
        self.update(load, world, |s| match mode {
            Mode::Char => s.char_asked = Some(key),
            Mode::Party => s.party_asked = Some(key),
        });
    }

    fn begin(&self, mode: Mode, load: u64, total: usize) -> Result<Progress, ()> {
        let mut r = self
            .running
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if r.is_some() {
            return Err(());
        }
        let done = Arc::new(AtomicUsize::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        *r = Some(Running {
            mode,
            load,
            total,
            done: done.clone(),
            cancel: cancel.clone(),
            started: Instant::now(),
        });
        Ok(Progress { done, cancel })
    }

    /// Asks the running calculation to stop after the monster it is on. False when none is running.
    pub fn cancel(&self) -> bool {
        let r = self
            .running
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match r.as_ref() {
            Some(run) => {
                run.cancel.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    fn end(&self) {
        *self
            .running
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
}

/// What a running calculation shares with its status and cancel requests.
struct Progress {
    done: Arc<AtomicUsize>,
    cancel: Arc<AtomicBool>,
}

/// Clears `running` however the calculation ends (an error, a panic in the worker).
struct EndOnDrop<'a>(&'a MonsterDamage);

impl Drop for EndOnDrop<'_> {
    fn drop(&mut self) {
        self.0.end();
    }
}

// ---------- requests ----------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalcBody {
    pub character: CharacterDto,
    pub mode: Mode,
    /// `txtMonsterLairFilter(0..9)`, read for the party; MME's start-up values when absent.
    #[serde(default)]
    pub party: PartyBoxes,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DismissBody {
    pub mode: Mode,
    /// The `key` the list's `prompt` carried.
    pub key: String,
}

/// What a finished calculation did.
#[derive(Debug, Serialize)]
pub struct Calculated {
    pub mode: Mode,
    pub monsters: usize,
    /// Monsters whose calculation stored nothing (a VB error in MME): their entries keep what they had.
    pub skipped: usize,
    pub elapsed_ms: u64,
    /// A cancel request stopped the run: nothing was stored and the arrays are as they were.
    pub cancelled: bool,
}

/// `CalculateMonsterDamageVsCharALL` for the request's character (or party boxes) and store the result.
///
/// # Errors
/// A character that cannot be evaluated, or a calculation already running.
pub fn calculate(
    state: &AppState,
    loaded: &Loaded,
    body: &CalcBody,
    sim_rounds: i32,
) -> Result<Calculated, (StatusCode, String)> {
    let world = &loaded.world;
    let profile = loaded.profile;
    let load = loaded.info.load_count;
    let Progress { done, cancel } = state
        .monster_damage()
        .begin(body.mode, load, world.monsters.len())
        .map_err(|()| {
            (
                StatusCode::CONFLICT,
                "a monster damage calculation is already running".to_owned(),
            )
        })?;
    let _end = EndOnDrop(state.monster_damage());
    let started = Instant::now();

    let controls = MonsterControls {
        party: body.party.clone(),
        ..MonsterControls::default()
    };
    let (ctx, chr, desc) = character_side(
        profile,
        world,
        &body.character,
        &controls,
        state,
        sim_rounds,
    )?;
    let party = controls.party();
    let env = GlueEnv {
        profile,
        nmr_ver: ctx.nmr,
    };
    // MME's generator is one for the whole session; a calculation starts it from the initial state so that the same
    // request gives the same numbers.
    let mut rnd = Rnd::from_state(327_680);
    let mut sim = MonsterSimInput::default();
    let total = world.monsters.len();
    let results = calculate_monster_damage_vs_char_all_with(
        world,
        &env,
        body.mode == Mode::Party,
        sim_rounds,
        &chr,
        &party,
        &mut sim,
        &mut rnd,
        &mut |n, _| {
            done.store(n, Ordering::Relaxed);
            !cancel.load(Ordering::Relaxed)
        },
    )
    .map_err(|e| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("the calculation failed: {e}"),
        )
    })?;
    // a cancelled run keeps the previous arrays, whatever it had finished
    if cancel.load(Ordering::Relaxed) {
        return Ok(Calculated {
            mode: body.mode,
            monsters: total,
            skipped: 0,
            elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            cancelled: true,
        });
    }
    let skipped = results.iter().filter(|(_, v)| v.is_none()).count();
    let key = party_key(&party);
    state.monster_damage().update(load, world, |s| {
        let a = match body.mode {
            Mode::Char => &mut s.vs_char,
            Mode::Party => &mut s.vs_party,
        };
        for (n, v) in &results {
            if let (Some(v), Some(slot)) = (v, usize::try_from(*n).ok().and_then(|i| a.get_mut(i)))
            {
                *slot = *v;
            }
        }
        match body.mode {
            Mode::Party => {
                s.party_calculated = true;
                s.party_asked = Some(key);
            }
            Mode::Char => {
                // SetCharDefenseDescription exits without a class; the config then stays what the description was
                let d = desc.unwrap_or_default();
                s.char_config.clone_from(&d);
                s.char_asked = Some(d);
            }
        }
    });
    Ok(Calculated {
        mode: body.mode,
        monsters: total,
        skipped,
        elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        cancelled: false,
    })
}

/// `SetCharDefenseDescription` for a refreshed character. MME reads the MR box, which `RefreshMagicRes` fills in, where
/// the saved character's own `mr_text` is only what the file held, so the refreshed value goes in.
pub(crate) fn char_defense_description(
    cctx: &touchstone_rules::character::Context<'_>,
    state: &touchstone_rules::pipeline::CharacterState,
) -> Option<String> {
    let mut ch = state.character.clone();
    ch.mr_text = state
        .magic_res
        .map_or_else(|_| String::new(), |m| m.to_string());
    set_char_defense_description(cctx, &ch, "").map(|d| d.description)
}

/// The character as the sim and the stale check read it: the prepared request, `CharSimInputs` and
/// `SetCharDefenseDescription`'s text (`None` where MME exits without one: no class).
fn character_side<'a>(
    profile: Profile,
    world: &'a World,
    character: &CharacterDto,
    controls: &MonsterControls,
    state: &AppState,
    sim_rounds: i32,
) -> Result<(Ctx<'a>, CharSimInputs, Option<String>), (StatusCode, String)> {
    let req = EvaluateBody {
        character: Some(character.clone()),
        global_filter: Some(GlobalFilterBody {
            use_character: true,
            ..GlobalFilterBody::default()
        }),
        options: Options {
            monsters: controls.clone(),
            ..Options::default()
        },
    };
    let ctx = prepare(profile, world, &req, state.hide_record_numbers())?;
    let exp = run_settings(state, Some(character)).0;
    let snap = Snapshot::uncalculated(world);
    let (chr, desc) = with_monster_env(
        &ctx,
        controls,
        &exp,
        sim_rounds,
        &snap,
        &crate::evaluate::NoCastText,
        |_, s| {
            (
                s.state.sim_inputs(),
                char_defense_description(s.cctx, s.state),
            )
        },
    )?;
    Ok((ctx, chr, desc))
}

// ---------- routes ----------

/// MME runs "Calc. Monster Dmg vs Party" with no character loaded (its menu item depends on the lair filter's party
/// size, not on a character) and the party calculation reads only the party boxes. The pipeline wants a class and a
/// race to build the lair environment, so a party request without them gets the first class and race of the data.
pub fn with_stand_in_character(world: &World, mut body: CalcBody) -> CalcBody {
    if body.mode == Mode::Party {
        let c = &mut body.character;
        c.class = c.class.or_else(|| world.classes.first().map(|k| k.number));
        c.race = c.race.or_else(|| world.races.first().map(|r| r.number));
    }
    body
}

pub async fn calculate_route(
    State(state): State<Arc<AppState>>,
    Body(body): Body<CalcBody>,
) -> ApiResult {
    let loaded = state.current();
    let body = with_stand_in_character(&loaded.world, body);
    let (_, rounds) = run_settings(&state, Some(&body.character));
    let st = state.clone();
    let out = tokio::task::spawn_blocking(move || calculate(&st, &loaded, &body, rounds))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))??;
    json(&out)
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClearBody {
    /// One array only; both when absent, as the menu does.
    pub mode: Option<Mode>,
}

pub async fn clear_route(
    State(state): State<Arc<AppState>>,
    Body(body): Body<ClearBody>,
) -> ApiResult {
    let loaded = state.current();
    if state.monster_damage().is_running() {
        return Err((
            StatusCode::CONFLICT,
            "a monster damage calculation is running".to_owned(),
        ));
    }
    state
        .monster_damage()
        .clear(loaded.info.load_count, &loaded.world, body.mode);
    json(&status_json(&state, &loaded))
}

/// `POST /api/monster-damage/cancel`: stop the running calculation after the monster it is on. The status answers;
/// with nothing running it is the plain status.
pub async fn cancel_route(State(state): State<Arc<AppState>>) -> ApiResult {
    let loaded = state.current();
    state.monster_damage().cancel();
    json(&status_json(&state, &loaded))
}

pub async fn dismiss_route(
    State(state): State<Arc<AppState>>,
    Body(body): Body<DismissBody>,
) -> ApiResult {
    let loaded = state.current();
    state
        .monster_damage()
        .dismiss(loaded.info.load_count, &loaded.world, body.mode, body.key);
    json(&status_json(&state, &loaded))
}

pub async fn status_route(State(state): State<Arc<AppState>>) -> ApiResult {
    let loaded = state.current();
    json(&status_json(&state, &loaded))
}

/// `{char, party, running, generation}`: whether each array holds a calculation, and the progress of a running one.
pub fn status_json(state: &AppState, loaded: &Loaded) -> Value {
    let md = state.monster_damage();
    let load = loaded.info.load_count;
    let s = md.snapshot(load, &loaded.world);
    let running = md
        .running
        .lock()
        .ok()
        .and_then(|r| {
            r.as_ref().filter(|r| r.load == load).map(|r| {
                serde_json::json!({
                    "mode": r.mode,
                    "done": r.done.load(Ordering::Relaxed),
                    "total": r.total,
                    "cancelling": r.cancel.load(Ordering::Relaxed),
                    "elapsed_ms": u64::try_from(r.started.elapsed().as_millis()).unwrap_or(u64::MAX),
                })
            })
        })
        .unwrap_or(Value::Null);
    serde_json::json!({
        "char": { "calculated": !s.char_config.is_empty() },
        "party": { "calculated": s.party_calculated },
        "running": running,
        "generation": s.generation,
    })
}

impl MonsterDamage {
    /// A calculation is running.
    pub fn is_running(&self) -> bool {
        self.running.lock().is_ok_and(|r| r.is_some())
    }
}
