//! `POST /api/calc/weapon-sim`: the best weapons for a character, by simulation.
//!
//! touchstone's own tool (MME has none; `docs/differences.md`, "Weapon simulator"). Every weapon
//! the character may wield is first scored with `CalculateAttack`, which is cheap; only the best
//! few by each attack are then rolled round by round (`touchstone_rules::weapon_sim`). The
//! response carries both figures, so the list can be checked against the Weapons tab.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::extract::State;
use serde::{Deserialize, Serialize};
use touchstone_model::{Item, World};
use touchstone_rules::Profile;
use touchstone_rules::ability_text::AbilityEnv;
use touchstone_rules::attack::{
    AttackDamage, AttackEnv, AttackRequest, calc_stab_opener, calculate_attack,
};
use touchstone_rules::character::AttackTypeMud;
use touchstone_rules::gear::{Character as GlobalChar, item_is_usable_by_char};
use touchstone_rules::pipeline::CharacterState;
use touchstone_rules::stats::{compute_avg_level_max_stats, get_class_stealth, get_race_stealth};
use touchstone_rules::weapon_sim::{
    Blow, KillStats, Opener, Rng, RoundStats, simulate_fights, simulate_openers, simulate_rounds,
};
use touchstone_vb6 as vb;

use crate::calc::{ApiResult, Body, bad, json};
use crate::character::{CharacterDto, refresh};
use crate::state::AppState;

/// Limits that keep one request small on a Raspberry Pi: at the worst (every limit reached) a run
/// is a few hundred million dice, a couple of seconds of one core.
pub const MAX_ROUNDS: u32 = 50_000;
pub const MAX_CANDIDATES: usize = 60;
pub const MAX_PINNED: usize = 20;
/// The rounds-to-kill figures get about this many rounds of dice per attack, so a tough target
/// costs no more than a soft one: fewer fights when each is long.
const FIGHT_BUDGET: f64 = 6_000.0;
const MAX_FIGHTS: f64 = 1_000.0;
const MIN_FIGHTS: f64 = 100.0;
/// Past this many rounds a fight is not rolled: over that long the dice average out, and hit
/// points over damage a round is the answer.
const LONG_FIGHT: f64 = 60.0;

/// One simulation at a time: a second request waits its turn instead of taking a second core
/// from the game server beside us.
static GATE: Mutex<()> = Mutex::new(());

const fn yes() -> bool {
    true
}
const fn default_rounds() -> u32 {
    5_000
}
const fn default_candidates() -> usize {
    25
}
const fn default_seed() -> u64 {
    1
}

/// What the blows land on. All zero is MME's "vs 0 ac/dr".
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct Target {
    pub ac: i32,
    pub dr: i32,
    pub dodge: i32,
    pub bs_defense: i32,
    /// Hit points; above 0 the rounds to kill are simulated too.
    pub hp: i32,
}

#[derive(Debug, Deserialize)]
pub struct WeaponSimReq {
    pub character: CharacterDto,
    /// Keep weapons above the character's level (to plan ahead); damage still uses today's stats.
    #[serde(default)]
    pub ignore_level: bool,
    #[serde(default)]
    pub ignore_alignment: bool,
    /// Limited items are the realm's rare ones; on by default, as a best-weapon list wants them.
    #[serde(default = "yes")]
    pub include_limited: bool,
    #[serde(default)]
    pub target: Target,
    #[serde(default = "default_rounds")]
    pub rounds: u32,
    /// How many of the best weapons by each attack go on to the simulation.
    #[serde(default = "default_candidates")]
    pub candidates: usize,
    #[serde(default = "default_seed")]
    pub seed: u64,
    /// Weapons simulated whatever their score (ones the user wants to compare).
    #[serde(default)]
    pub pinned: Vec<i32>,
}

/// One attack with one weapon: MME's figures and the simulated ones.
#[derive(Debug, Serialize)]
pub struct ModeJson {
    /// `CalculateAttack`'s damage a round (for a stab, its opening round).
    pub expected: i32,
    pub sim: RoundStats,
    pub swings: f64,
    pub hit: i16,
    pub crit: i16,
    pub min_dmg: i32,
    pub max_dmg: i32,
    pub avg_hit: i32,
    pub avg_crit: i32,
    pub max_crit: i32,
    /// The weapon's cast spell: its average when it fires.
    pub extra: i32,
    /// MME's detail line.
    pub detail: String,
    /// Rounds to bring the target down, when it has hit points.
    pub kill: Option<KillJson>,
}

/// Rounds to bring the target down.
#[derive(Debug, Serialize, Clone, Copy, PartialEq)]
pub struct KillJson {
    pub mean: f64,
    pub median: u32,
    /// Nine fights in ten are over by this round.
    pub p90: u32,
    /// Too long a fight to roll: hit points over the damage a round instead.
    pub estimated: bool,
}

/// The rounds a fight to `hp` takes: `opener` (a stab's round, with its mean damage) and then
/// `blow` every round, which deals `per_round` on average. `None` when nothing gets it there.
fn fight(
    opener: Option<(&Opener, f64)>,
    blow: Option<(&Blow, f64)>,
    hp: u32,
    rng: &mut Rng,
    buf: &mut Vec<u32>,
) -> Option<KillJson> {
    if hp == 0 {
        return None;
    }
    let opened = opener.map_or(0.0, |(_, mean)| mean);
    let left = (f64::from(hp) - opened).max(0.0);
    let per_round = blow.map_or(0.0, |(_, mean)| mean);
    if left > 0.0 && per_round <= 0.0 {
        return None;
    }
    let first = if opener.is_some() { 1.0 } else { 0.0 };
    let rounds = first + if left > 0.0 { left / per_round } else { 0.0 };
    if rounds > LONG_FIGHT {
        let whole = rounds.ceil() as u32;
        return Some(KillJson {
            mean: rounds,
            median: whole,
            p90: whole,
            estimated: true,
        });
    }
    let fights = (FIGHT_BUDGET / rounds.max(1.0)).clamp(MIN_FIGHTS, MAX_FIGHTS) as u32;
    let limit = (rounds * 4.0) as u32 + 20;
    let KillStats {
        mean, median, p90, ..
    } = simulate_fights(
        opener.map(|(o, _)| o),
        blow.map(|(b, _)| b),
        hp,
        fights,
        limit,
        rng,
        buf,
    )?;
    Some(KillJson {
        mean,
        median,
        p90,
        estimated: false,
    })
}

#[derive(Debug, Serialize)]
pub struct RowJson {
    pub number: i32,
    pub name: String,
    /// The level the weapon asks for (0: none).
    pub level: i32,
    /// MME's `WeaponType`: 0 one-handed blunt, 1 two-handed blunt, 2 one-handed sharp, 3 two-handed sharp.
    pub weapon_type: i32,
    pub two_handed: bool,
    pub min: i32,
    pub max: i32,
    pub speed: i32,
    pub str_req: i32,
    pub accy: i32,
    pub limit: i32,
    pub equipped: bool,
    pub pinned: bool,
    /// Kept only because a restriction is being ignored.
    pub needs_level: bool,
    pub wrong_alignment: bool,
    pub attack: Option<ModeJson>,
    pub bash: Option<ModeJson>,
    pub backstab: Option<ModeJson>,
}

#[derive(Debug, Serialize)]
pub struct WeaponSimJson {
    pub rows: Vec<RowJson>,
    /// Weapons the character may wield under the request's restrictions.
    pub usable: usize,
    pub simulated: usize,
    pub rounds: u32,
    pub candidates: usize,
    pub seed: u64,
    pub target: Target,
    /// The character's class or race can backstab.
    pub can_backstab: bool,
    /// The weapon in the character's hand, 0 for none.
    pub equipped_weapon: i32,
    pub level: i32,
    /// Server time for the whole request, milliseconds.
    pub elapsed_ms: f64,
}

/// MME's score of one weapon, kept until the shortlist is known.
struct Scored<'a> {
    item: &'a Item,
    needs_level: bool,
    wrong_alignment: bool,
    attack: AttackDamage,
    bash: AttackDamage,
    /// The stab and, under Quicksilver, the normal blows after it and their expected damage.
    backstab: Option<Stab>,
}

struct Stab {
    stab: AttackDamage,
    follow: Option<(AttackDamage, u32)>,
    expected: i32,
}

fn level_req(item: &Item) -> i32 {
    item.abilities
        .iter()
        .find(|s| s.ability == 135)
        .map_or(0, |s| s.value)
}

fn mode(a: &AttackDamage, expected: i32, sim: RoundStats, kill: Option<KillJson>) -> ModeJson {
    ModeJson {
        expected,
        sim,
        swings: a.swings,
        hit: a.hit_chance,
        crit: a.crit_chance,
        min_dmg: a.min_dmg,
        max_dmg: a.max_dmg,
        avg_hit: a.avg_hit,
        avg_crit: a.avg_crit,
        max_crit: a.max_crit,
        extra: a.avg_extra_hit,
        detail: a.attack_detail.clone(),
        kill,
    }
}

/// The best `n` weapons by `score`, highest first; a weapon that scores nothing is never best.
fn top<'a>(scored: &[Scored<'a>], n: usize, score: impl Fn(&Scored<'a>) -> i32) -> Vec<i32> {
    let mut order: Vec<(i32, i32)> = scored
        .iter()
        .map(|s| (score(s), s.item.number))
        .filter(|(v, _)| *v > 0)
        .collect();
    order.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    order
        .into_iter()
        .take(n)
        .map(|(_, number)| number)
        .collect()
}

/// The whole request, off the async threads.
///
/// # Errors
/// 400 for a request outside the limits.
#[allow(clippy::too_many_lines)] // one pass: usable weapons, MME's score, the shortlist, the dice
pub fn run(
    profile: Profile,
    world: &World,
    req: &WeaponSimReq,
    ch: &touchstone_rules::character::Character,
    st: &CharacterState,
) -> Result<WeaponSimJson, (axum::http::StatusCode, String)> {
    let started = Instant::now();
    if !(1..=MAX_ROUNDS).contains(&req.rounds) {
        return Err(bad(format!("rounds must be 1 to {MAX_ROUNDS}")));
    }
    if !(1..=MAX_CANDIDATES).contains(&req.candidates) {
        return Err(bad(format!("candidates must be 1 to {MAX_CANDIDATES}")));
    }
    if req.pinned.len() > MAX_PINNED {
        return Err(bad(format!("at most {MAX_PINNED} pinned weapons")));
    }
    let t = req.target;
    if [t.ac, t.dr, t.bs_defense, t.hp]
        .iter()
        .any(|v| !(0..=1_000_000).contains(v))
        || !(-100..=1_000).contains(&t.dodge)
    {
        return Err(bad("target values are out of range"));
    }

    let class = ch.class.unwrap_or(0);
    let race = ch.race.unwrap_or(0);
    let can_backstab =
        get_class_stealth(world.get_class(class)) || get_race_stealth(world.get_race(race));
    let who = GlobalChar {
        level: ch.level,
        class,
        race,
        alignment: i32::from(ch.alignment),
        min_item_level: 0,
    };
    let usable = |item: &Item, level: i32, skip_align: bool| {
        item_is_usable_by_char(
            profile,
            world,
            &GlobalChar { level, ..who },
            &ch.equipped,
            item.number,
            true,
            skip_align,
        )
    };

    let names = AbilityEnv::for_world(world, profile);
    let equipped = st.equipped_globals();
    let accuracy_tag = st.stats.stat_tag[10].map_or_else(String::new, vb::cstr);
    let env = AttackEnv {
        items: &world.items,
        classes: &world.classes,
        races: &world.races,
        equipped: &equipped,
        cast_text: &names,
        dat_ver_caption: &world.info.dat_file_version,
        avg_level_max_all_stats: compute_avg_level_max_stats(profile, &world.races).all_stats,
        accuracy_tag: &accuracy_tag,
    };
    let request = |attack_type: i32, weapon: i32| AttackRequest {
        attack_type,
        weapon_number: weapon,
        vs_ac: t.ac,
        vs_dr: t.dr,
        vs_dodge: t.dodge,
        bs_defense: t.bs_defense,
        ..AttackRequest::default()
    };
    let attack = |p: &touchstone_rules::attack::CharacterProfile, attack_type: i32, weapon: i32| {
        calculate_attack(
            profile,
            &env,
            p,
            &request(attack_type, weapon),
            &mut String::new(),
        )
    };
    // Only a stab's profile depends on the weapon (PopulateCharacterProfile recounts its accuracy).
    let normal =
        CharacterState::attack_profile_of(&st.profile_for(world, AttackTypeMud::Normal, 0));
    let bashing = CharacterState::attack_profile_of(&st.profile_for(world, AttackTypeMud::Bash, 0));

    // ---- every weapon the character may wield, scored by MME
    let equipped_weapon = equipped.hands[0].number;
    let mut scored = Vec::new();
    for item in &world.items {
        if item.item_type != 1 || !item.in_game {
            continue;
        }
        let is_equipped = item.number == equipped_weapon;
        if !req.include_limited && item.limit != 0 && !is_equipped {
            continue;
        }
        let strict = usable(item, ch.level, false);
        let mut needs_level = false;
        let mut wrong_alignment = false;
        if !strict && !is_equipped {
            // a level above the character's own only ever lifts the minimum-level rule
            let level = if req.ignore_level {
                ch.level.max(level_req(item))
            } else {
                ch.level
            };
            if !usable(item, level, req.ignore_alignment) {
                continue;
            }
            needs_level = !usable(item, ch.level, true);
            wrong_alignment = !usable(item, level, false);
        }
        let n = item.number;
        // MME's weapon lists stab only with a weapon that has backstab accuracy (ability 116)
        let stabs = can_backstab && item.abilities.iter().any(|s| s.ability == 116);
        let backstab = stabs.then(|| {
            let p = CharacterState::attack_profile_of(&st.profile_for(
                world,
                AttackTypeMud::Surprise,
                n,
            ));
            let stab = attack(&p, 4, n);
            // Quicksilver: the rest of the opening round pays for normal blows
            let opener = calc_stab_opener(profile, &env, &p, &request(4, n));
            let follow = opener.map(|o| (attack(&p, 5, n), o.follow_blows.max(0).cast_unsigned()));
            let expected = stab.round_total + opener.map_or(0, |o| o.follow_damage);
            Stab {
                stab,
                follow,
                expected,
            }
        });
        scored.push(Scored {
            item,
            needs_level,
            wrong_alignment,
            attack: attack(&normal, 5, n),
            bash: attack(&bashing, 6, n),
            backstab,
        });
    }

    // ---- the shortlist: the best by each attack, the weapon in hand, the pinned ones
    let mut keep: BTreeSet<i32> = BTreeSet::new();
    keep.extend(top(&scored, req.candidates, |s| s.attack.round_total));
    keep.extend(top(&scored, req.candidates, |s| s.bash.round_total));
    keep.extend(top(&scored, req.candidates, |s| {
        s.backstab.as_ref().map_or(0, |b| b.expected)
    }));
    keep.insert(equipped_weapon);
    keep.extend(req.pinned.iter().copied());

    // ---- the dice
    let hp = t.hp.max(0).cast_unsigned();
    let mut buf = Vec::with_capacity(req.rounds as usize);
    let mut rows = Vec::new();
    for s in scored.iter().filter(|s| keep.contains(&s.item.number)) {
        let n = s.item.number;
        let regular = |a: &AttackDamage, stream: u32, buf: &mut Vec<u32>| {
            let blow = Blow::from_attack(a)?;
            let sim = simulate_rounds(
                &blow,
                req.rounds,
                &mut Rng::for_stream(req.seed, n, stream),
                buf,
            );
            let kill = fight(
                None,
                Some((&blow, sim.mean)),
                hp,
                &mut Rng::for_stream(req.seed, n, stream + 10),
                buf,
            );
            Some(mode(a, a.round_total, sim, kill))
        };
        let attack_mode = regular(&s.attack, 0, &mut buf);
        let bash_mode = regular(&s.bash, 1, &mut buf);
        let backstab_mode = s.backstab.as_ref().and_then(|b| {
            let opener = Opener {
                stab: Blow::from_attack(&b.stab)?,
                follow: b
                    .follow
                    .as_ref()
                    .and_then(|(a, blows)| Some((Blow::from_attack(a)?, *blows))),
            };
            let sim = simulate_openers(
                &opener,
                req.rounds,
                &mut Rng::for_stream(req.seed, n, 2),
                &mut buf,
            );
            // the fight a stab opens goes on with normal attacks
            let after = Blow::from_attack(&s.attack);
            let after = after.as_ref().zip(attack_mode.as_ref().map(|m| m.sim.mean));
            let kill = fight(
                Some((&opener, sim.mean)),
                after,
                hp,
                &mut Rng::for_stream(req.seed, n, 12),
                &mut buf,
            );
            Some(mode(&b.stab, b.expected, sim, kill))
        });
        rows.push(RowJson {
            number: n,
            name: s.item.name.clone(),
            level: level_req(s.item),
            weapon_type: s.item.weapon_type,
            two_handed: matches!(s.item.weapon_type, 1 | 3),
            min: s.item.min,
            max: s.item.max,
            speed: s.item.speed,
            str_req: s.item.str_req,
            accy: s.item.accy,
            limit: s.item.limit,
            equipped: n == equipped_weapon,
            pinned: req.pinned.contains(&n),
            needs_level: s.needs_level,
            wrong_alignment: s.wrong_alignment,
            attack: attack_mode,
            bash: bash_mode,
            backstab: backstab_mode,
        });
    }

    Ok(WeaponSimJson {
        simulated: rows.len(),
        rows,
        usable: scored.len(),
        rounds: req.rounds,
        candidates: req.candidates,
        seed: req.seed,
        target: t,
        can_backstab,
        equipped_weapon,
        level: ch.level,
        elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
    })
}

pub async fn route(State(app): State<Arc<AppState>>, Body(req): Body<WeaponSimReq>) -> ApiResult {
    let loaded = app.current();
    let profile = loaded.profile;
    let hide = app.hide_record_numbers();
    let out = tokio::task::spawn_blocking(move || {
        let _turn = GATE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (ch, st) = refresh(profile, &loaded.world, &req.character, hide)?;
        run(profile, &loaded.world, &req, &ch, &st)
    })
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))??;
    json(&out)
}
