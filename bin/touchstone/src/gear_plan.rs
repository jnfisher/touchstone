//! `POST /api/calc/gear-plan`: the worn gear that does the most for one goal, and where to get it.
//!
//! touchstone's own tool (`docs/differences.md`, "Gear planner"). The search is
//! `touchstone_rules::gear_plan`; this module decides which items a slot may hold
//! (`crate::usable`), names things, and reads each suggested item's Obtained From into the
//! monsters, chests, shops and places that yield it (`touchstone_rules::acquire`).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use touchstone_model::World;
use touchstone_rules::Profile;
use touchstone_rules::acquire::{Acquisition, acquisition};
use touchstone_rules::character::Character;
use touchstone_rules::equip::list_slots;
use touchstone_rules::gear::EQUIP_SLOTS;
use touchstone_rules::gear_plan::{
    Goal, Limits, PlanRequest, REPORTED_STATS, Target, fixed_slots, plan,
};
use touchstone_rules::pipeline::PipelineOptions;

use crate::calc::{ApiResult, Body, bad, json};
use crate::character::{CharacterDto, pipeline_options};
use crate::state::AppState;
use crate::usable::{self, Allow, Verdict};

/// The slots as MME's EQ tab names them (`nEquippedItem(0..19)`).
pub const SLOT_NAMES: [&str; EQUIP_SLOTS] = [
    "Head",
    "Ears",
    "Neck",
    "Back",
    "Torso",
    "Arms",
    "Wrist",
    "Wrist 2",
    "Hands",
    "Finger 1",
    "Finger 2",
    "Waist",
    "Legs",
    "Feet",
    "Worn",
    "Off-Hand",
    "Weapon",
    "Eyes",
    "Face",
    "Everywhere",
];

/// The names of the figures a plan reports, in `gear_plan::REPORTED_STATS`' order.
const STAT_NAMES: [&str; REPORTED_STATS] = [
    "Armour class",
    "Damage resistance",
    "Dodge",
    "Magic resistance",
    "Accuracy",
    "Crits",
    "Max damage",
    "Min damage",
    "Stealth",
    "Hit points",
    "HP regen",
    "Mana",
    "Mana regen",
    "Spellcasting",
    "Spell damage",
    "Weight carried",
];

/// Later passes and the alternatives look at this many of a slot's best items.
const KEEP_TOP: usize = 12;
const MAX_PASSES: usize = 6;
/// Sources listed for one item, of each kind.
const SOURCE_CAP: usize = 6;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoalDto {
    Melee,
    Backstab,
    Armour,
    Dodge,
    Caster,
    ManaRegen,
    HitPoints,
    Stealth,
}

impl GoalDto {
    const fn goal(self) -> Goal {
        match self {
            Self::Melee => Goal::Melee,
            Self::Backstab => Goal::Backstab,
            Self::Armour => Goal::Armour,
            Self::Dodge => Goal::Dodge,
            Self::Caster => Goal::Caster,
            Self::ManaRegen => Goal::ManaRegen,
            Self::HitPoints => Goal::HitPoints,
            Self::Stealth => Goal::Stealth,
        }
    }

    /// What the goal's figure is called.
    const fn metric(self) -> &'static str {
        match self {
            Self::Melee => "Damage a round",
            Self::Backstab => "Backstab damage",
            Self::Armour => "Armour class",
            Self::Dodge => "Dodge",
            Self::Caster => "Spellcasting",
            Self::ManaRegen => "Mana regen",
            Self::HitPoints => "Hit points",
            Self::Stealth => "Stealth",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct TargetDto {
    pub ac: i32,
    pub dr: i32,
    pub dodge: i32,
    pub bs_defense: i32,
}

#[derive(Debug, Deserialize)]
pub struct GearPlanReq {
    pub character: CharacterDto,
    pub goal: GoalDto,
    #[serde(flatten)]
    pub allow: Allow,
    /// What the damage goals swing at.
    #[serde(default)]
    pub target: TargetDto,
    /// The set must keep at least this armour class.
    #[serde(default)]
    pub min_ac: Option<f64>,
    /// The set must stay at or under this encumbrance, percent.
    #[serde(default)]
    pub max_encum: Option<f64>,
    /// Slots to leave as they are.
    #[serde(default)]
    pub locked: Vec<usize>,
    /// Fill the second wrist (MME's `bInvenUse2ndWrist`).
    #[serde(default)]
    pub second_wrist: bool,
}

#[derive(Debug, Serialize)]
pub struct ItemJson {
    pub number: i32,
    pub name: String,
    pub level: i32,
    pub armour_class: i32,
    pub damage_resist: i32,
    pub encum: i32,
    pub limited: bool,
    pub needs_level: bool,
    pub wrong_alignment: bool,
    pub unobtainable: bool,
}

#[derive(Debug, Serialize)]
pub struct AltJson {
    pub item: ItemJson,
    /// The goal's figure with this item in the suggested set, less the suggested set's.
    pub delta: f64,
}

#[derive(Debug, Serialize)]
pub struct SlotJson {
    pub slot: usize,
    pub name: &'static str,
    /// Why the planner left the slot alone: `locked`, `weapon`, `two_handed`, `second_wrist`.
    pub closed: Option<&'static str>,
    pub current: Option<ItemJson>,
    pub suggested: Option<ItemJson>,
    pub changed: bool,
    /// What the swap alone adds to the goal on today's gear.
    pub gain: f64,
    pub alternatives: Vec<AltJson>,
    /// Where to get the suggested item, for a changed slot.
    pub acquisition: Option<Acquisition>,
}

#[derive(Debug, Serialize)]
pub struct StatJson {
    pub label: &'static str,
    pub before: f64,
    pub after: f64,
}

#[derive(Debug, Serialize)]
pub struct GearPlanJson {
    pub goal: GoalDto,
    pub metric: &'static str,
    pub before: f64,
    pub after: f64,
    pub slots: Vec<SlotJson>,
    pub stats: Vec<StatJson>,
    /// Items offered across the open slots.
    pub candidates: usize,
    /// Sets evaluated and passes made.
    pub evaluations: u32,
    pub passes: u32,
    pub level: i32,
    /// A limit the suggested set still breaks (nothing the character can wear meets it).
    pub unmet: Vec<String>,
    pub elapsed_ms: f64,
}

/// One decimal: MME's stats carry at most that, and sums of them pick up float noise.
fn tenth(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// The whole request, off the async threads.
///
/// # Errors
/// 400 for a request outside the limits; 422 for a character MME cannot evaluate.
pub fn run(
    profile: Profile,
    world: &World,
    req: &GearPlanReq,
    ch: &Character,
    opts: &PipelineOptions,
) -> Result<GearPlanJson, (StatusCode, String)> {
    let started = Instant::now();
    if req.locked.iter().any(|s| *s >= EQUIP_SLOTS) {
        return Err(bad("locked holds a slot that does not exist"));
    }
    let t = req.target;
    if [t.ac, t.dr, t.bs_defense]
        .iter()
        .any(|v| !(0..=1_000_000).contains(v))
        || !(-100..=1_000).contains(&t.dodge)
    {
        return Err(bad("target values are out of range"));
    }
    for limit in [req.min_ac, req.max_encum].into_iter().flatten() {
        if !limit.is_finite() || !(0.0..=100_000.0).contains(&limit) {
            return Err(bad("a limit is out of range"));
        }
    }

    // ---- which slots the planner may change
    let fixed = fixed_slots(world, &ch.equipped);
    let closed: [Option<&'static str>; EQUIP_SLOTS] = std::array::from_fn(|slot| {
        if slot == touchstone_rules::equip::WEAPON_SLOT {
            Some("weapon")
        } else if fixed.contains(&slot) {
            Some("two_handed")
        } else if req.locked.contains(&slot) {
            Some("locked")
        } else if slot == touchstone_rules::equip::WRIST_SLOTS[1] && !req.second_wrist {
            Some("second_wrist")
        } else {
            None
        }
    });

    // ---- what each open slot may hold
    let who = usable::who(ch);
    let mut verdicts: HashMap<i32, Verdict> = HashMap::new();
    let mut candidates: [Vec<i32>; EQUIP_SLOTS] = std::array::from_fn(|_| Vec::new());
    for item in &world.items {
        if item.item_type != 0 {
            continue;
        }
        let slots = list_slots(item.item_type, item.worn);
        if slots.iter().all(|s| closed[*s].is_some()) {
            continue;
        }
        let held = ch.equipped.contains(&item.number);
        let Some(v) = usable::offered(profile, world, &who, item, &req.allow, held, false) else {
            continue;
        };
        verdicts.insert(item.number, v);
        for s in slots {
            if closed[s].is_none() {
                candidates[s].push(item.number);
            }
        }
    }
    let offered = verdicts.len();

    let found = plan(
        profile,
        world,
        ch,
        opts,
        &PlanRequest {
            goal: req.goal.goal(),
            target: Target {
                ac: t.ac,
                dr: t.dr,
                dodge: t.dodge,
                bs_defense: t.bs_defense,
            },
            limits: Limits {
                min_ac: req.min_ac,
                max_encum_pct: req.max_encum,
            },
            candidates: &candidates,
            open: closed.map(|c| c.is_none()),
            keep_top: KEEP_TOP,
            max_passes: MAX_PASSES,
        },
    )
    .ok_or((
        StatusCode::UNPROCESSABLE_ENTITY,
        "cannot evaluate this character".to_owned(),
    ))?;

    let item_json = |number: i32| {
        let item = world.get_item(number)?;
        let v = verdicts.get(&number).copied().unwrap_or_default();
        Some(ItemJson {
            number,
            name: item.name.clone(),
            level: usable::level_req(item),
            armour_class: item.armour_class,
            damage_resist: item.damage_resist,
            encum: item.encum,
            limited: item.limit != 0,
            needs_level: v.needs_level,
            wrong_alignment: v.wrong_alignment,
            unobtainable: !usable::obtainable(item),
        })
    };
    let slots = found
        .slots
        .iter()
        .map(|s| {
            let changed = s.suggested != s.current;
            SlotJson {
                slot: s.slot,
                name: SLOT_NAMES[s.slot],
                closed: closed[s.slot],
                current: item_json(s.current),
                suggested: item_json(s.suggested),
                changed,
                gain: tenth(s.gain_alone),
                alternatives: s
                    .alternatives
                    .iter()
                    .filter_map(|a| {
                        Some(AltJson {
                            item: item_json(a.item)?,
                            delta: tenth(a.delta),
                        })
                    })
                    .collect(),
                acquisition: (changed && s.suggested > 0)
                    .then(|| acquisition(world, s.suggested, SOURCE_CAP))
                    .flatten(),
            }
        })
        .collect();

    let mut stats: Vec<StatJson> = (0..REPORTED_STATS)
        .map(|k| StatJson {
            label: STAT_NAMES[k],
            before: tenth(found.before.stats[k]),
            after: tenth(found.after.stats[k]),
        })
        .collect();
    stats.push(StatJson {
        label: "Encumbrance %",
        before: found.before.encum_pct,
        after: found.after.encum_pct,
    });

    let mut unmet = Vec::new();
    if let Some(min) = req.min_ac.filter(|min| found.after.ac < *min) {
        unmet.push(format!(
            "armour class {} is under the {min} asked for",
            found.after.ac
        ));
    }
    if let Some(max) = req.max_encum.filter(|max| found.after.encum_pct > *max) {
        unmet.push(format!(
            "encumbrance {}% is over the {max}% asked for",
            found.after.encum_pct
        ));
    }

    Ok(GearPlanJson {
        goal: req.goal,
        metric: req.goal.metric(),
        before: found.before.primary,
        after: found.after.primary,
        slots,
        stats,
        candidates: offered,
        evaluations: found.evaluations,
        passes: found.passes,
        level: ch.level,
        unmet,
        elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
    })
}

pub async fn route(State(app): State<Arc<AppState>>, Body(req): Body<GearPlanReq>) -> ApiResult {
    let loaded = app.current();
    let profile = loaded.profile;
    let hide = app.hide_record_numbers();
    let out = tokio::task::spawn_blocking(move || {
        // one heavy calculation at a time, shared with the Weapon Simulator
        let _turn = crate::weapon_sim::GATE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let ch = req.character.to_character().map_err(bad)?;
        if ch.class.is_none() || ch.race.is_none() {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                "pick a class and race".to_owned(),
            ));
        }
        let opts = pipeline_options(profile, &loaded.world, &req.character, hide);
        run(profile, &loaded.world, &req, &ch, &opts)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))??;
    json(&out)
}
