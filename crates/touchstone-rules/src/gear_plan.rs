//! The gear planner's search: the worn items that do the most for one goal.
//!
//! touchstone's own; MME's nearest tool is Find Best, which ranks a slot's items by one ability
//! (`docs/differences.md`, "Gear planner"). Gear is not additive: an item's weight moves the
//! swings and the dodge, its strength the weight the character can carry, its accuracy only counts
//! until every blow lands. So a set is judged by MME's own answer for the character wearing it
//! ([`refresh_all`], and `CalculateAttack` for the damage goals), never by adding up abilities.
//!
//! The search is a climb, slot by slot: every candidate of a slot is tried in the set as it
//! stands and the best stays; later passes retry only each slot's best few, until a pass changes
//! nothing. It is quick and finds a set no single swap improves, which is not always the best
//! set there is (two items that only pay off together can be missed).

use touchstone_model::World;

use crate::Profile;
use crate::ability_text::AbilityEnv;
use crate::attack::{AttackEnv, AttackRequest, calculate_attack};
use crate::character::{AttackTypeMud, Character};
use crate::equip::{FINGER_SLOTS, OFF_HAND_SLOT, WEAPON_SLOT, WRIST_SLOTS};
use crate::gear::EQUIP_SLOTS;
use crate::pipeline::{CharacterState, PipelineOptions, refresh_all};

/// What a set is judged by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Goal {
    /// Damage a round of a normal attack with the weapon in hand.
    Melee,
    /// Damage of a backstab with the weapon in hand.
    Backstab,
    /// Armour class, then damage resistance.
    Armour,
    Dodge,
    /// Spellcasting, then spell damage and mana.
    Caster,
    /// Mana regeneration, then mana.
    ManaRegen,
    /// Hit points, then their regeneration.
    HitPoints,
    Stealth,
}

/// The defender the damage goals swing at. All zero is MME's "vs 0 ac/dr".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Target {
    pub ac: i32,
    pub dr: i32,
    pub dodge: i32,
    pub bs_defense: i32,
}

/// What a set must also keep. A set that breaks a limit loses to any that does not.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Limits {
    pub min_ac: Option<f64>,
    pub max_encum_pct: Option<f64>,
}

/// How many figures a plan reports before and after: armour class, damage resistance, dodge,
/// magic resistance, accuracy, crits, max damage, min damage, stealth, hit points, hit point
/// regeneration, mana, mana regeneration, spellcasting, spell damage and the weight carried.
pub const REPORTED_STATS: usize = 16;

/// One set's numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measure {
    /// The goal's own figure (damage a round, armour class, ...).
    pub primary: f64,
    /// What separates sets the goal ranks alike.
    pub secondary: f64,
    pub ac: f64,
    pub dr: f64,
    pub encum_pct: f64,
    /// The [`REPORTED_STATS`] figures, in the order its comment gives.
    pub stats: [f64; REPORTED_STATS],
}

impl Measure {
    fn violation(&self, limits: &Limits) -> f64 {
        let short = limits.min_ac.map_or(0.0, |min| (min - self.ac).max(0.0));
        let over = limits
            .max_encum_pct
            .map_or(0.0, |max| (self.encum_pct - max).max(0.0));
        short + over
    }

    /// Does this set do more for the goal than `other` (or break the limits less)? What decides
    /// whether an item is worth swapping in: a tie on the goal never is.
    fn improves_on(&self, other: &Self, limits: &Limits) -> bool {
        const EPS: f64 = 1e-9;
        let (a, b) = (self.violation(limits), other.violation(limits));
        a < b - EPS || (a <= b + EPS && self.primary > other.primary + EPS)
    }

    /// Is this set strictly better than `other`? Limits first, then the goal, then its
    /// tie-breaker, then armour (so equal sets keep the protection). Ranks the candidates that
    /// improve on what is worn.
    fn beats(&self, other: &Self, limits: &Limits) -> bool {
        const EPS: f64 = 1e-9;
        let keys = |m: &Self| [-m.violation(limits), m.primary, m.secondary, m.ac + m.dr];
        for (a, b) in keys(self).into_iter().zip(keys(other)) {
            if a > b + EPS {
                return true;
            }
            if a < b - EPS {
                return false;
            }
        }
        false
    }
}

pub struct PlanRequest<'a> {
    pub goal: Goal,
    pub target: Target,
    pub limits: Limits,
    /// Per slot, the items the planner may put there (the caller decides what the character may
    /// wear). The weapon slot is never changed, whatever it lists.
    pub candidates: &'a [Vec<i32>; EQUIP_SLOTS],
    /// Slots the planner may change; a locked slot keeps its item.
    pub open: [bool; EQUIP_SLOTS],
    /// How many of a slot's best candidates the later passes and the alternatives look at.
    pub keep_top: usize,
    pub max_passes: usize,
}

/// A candidate the planner did not pick for a slot, with what the goal loses by it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Alternative {
    pub item: i32,
    /// The goal's figure with this item in the final set, less the final set's.
    pub delta: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlotPlan {
    pub slot: usize,
    pub current: i32,
    pub suggested: i32,
    /// What this swap alone adds to the goal on today's gear (0 for an unchanged slot).
    pub gain_alone: f64,
    pub alternatives: Vec<Alternative>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub slots: Vec<SlotPlan>,
    pub equipped: [i32; EQUIP_SLOTS],
    pub before: Measure,
    pub after: Measure,
    /// Sets evaluated.
    pub evaluations: u32,
    pub passes: u32,
}

fn pair_of(slot: usize) -> Option<usize> {
    let other = |pair: [usize; 2]| {
        if pair[0] == slot {
            Some(pair[1])
        } else if pair[1] == slot {
            Some(pair[0])
        } else {
            None
        }
    };
    other(FINGER_SLOTS).or_else(|| other(WRIST_SLOTS))
}

/// Evaluates sets for one character and goal.
struct Planner<'a> {
    profile: Profile,
    world: &'a World,
    base: &'a Character,
    opts: &'a PipelineOptions,
    names: AbilityEnv<'a>,
    goal: Goal,
    target: Target,
    evaluations: u32,
}

impl Planner<'_> {
    /// `CalculateAttack` for the weapon in hand, as the combat setup calls it.
    fn attack(&self, st: &CharacterState, mud: AttackTypeMud) -> crate::attack::AttackDamage {
        let equipped = st.equipped_globals();
        let accuracy_tag = st.stats.stat_tag[10].map_or_else(String::new, touchstone_vb6::cstr);
        let env = AttackEnv {
            items: &self.world.items,
            classes: &self.world.classes,
            races: &self.world.races,
            equipped: &equipped,
            cast_text: &self.names,
            dat_ver_caption: &self.opts.dat_ver_caption,
            avg_level_max_all_stats: self.opts.avg_level_max_all_stats,
            accuracy_tag: &accuracy_tag,
        };
        let weapon = equipped.hands[0].number;
        let who = CharacterState::attack_profile_of(&st.profile_for(self.world, mud, 0));
        calculate_attack(
            self.profile,
            &env,
            &who,
            &AttackRequest {
                attack_type: mud as i32,
                weapon_number: weapon,
                vs_ac: self.target.ac,
                vs_dr: self.target.dr,
                vs_dodge: self.target.dodge,
                bs_defense: self.target.bs_defense,
                ..AttackRequest::default()
            },
            &mut String::new(),
        )
    }

    /// The numbers of the character wearing `equipped`; `None` for a set MME cannot evaluate.
    fn measure(&mut self, equipped: &[i32; EQUIP_SLOTS]) -> Option<Measure> {
        self.evaluations += 1;
        let mut ch = self.base.clone();
        ch.equipped = *equipped;
        let st = refresh_all(self.profile, self.world, &ch, self.opts).ok()?;
        let s = &st.stats.stat;
        // the sheet's totals, not the gear's share of them
        let sheet = &st.profile;
        let dodge = st.dodge.value;
        let (primary, secondary) = match self.goal {
            Goal::Melee => {
                let a = self.attack(&st, AttackTypeMud::Normal);
                // accuracy and crit separate sets whose rounded damage a round is the same
                (
                    f64::from(a.round_total),
                    f64::from(a.accy) + f64::from(a.crit_chance),
                )
            }
            Goal::Backstab => {
                let a = self.attack(&st, AttackTypeMud::Surprise);
                (
                    f64::from(a.round_total),
                    f64::from(a.max_dmg) + f64::from(a.accy),
                )
            }
            Goal::Armour => (s[2], s[3]),
            Goal::Dodge => (dodge, s[2]),
            Goal::Caster => (
                f64::from(sheet.spellcasting),
                s[33] + sheet.max_mana / 100.0,
            ),
            Goal::ManaRegen => (sheet.mana_regen, sheet.max_mana),
            Goal::HitPoints => (sheet.hp, sheet.hp_regen),
            Goal::Stealth => (s[19], dodge),
        };
        Some(Measure {
            primary,
            secondary,
            ac: s[2],
            dr: s[3],
            encum_pct: f64::from(st.stats.encum_pct),
            stats: [
                s[2],
                s[3],
                dodge,
                f64::from(st.magic_res.unwrap_or(0)),
                s[10],
                s[7],
                s[11],
                s[30],
                s[19],
                sheet.hp,
                sheet.hp_regen,
                sheet.max_mana,
                sheet.mana_regen,
                f64::from(sheet.spellcasting),
                s[33],
                s[0],
            ],
        })
    }
}

/// The best gear this search finds for `ch` and the request's goal. `None` when the character as
/// it stands cannot be evaluated.
#[must_use]
pub fn plan(
    profile: Profile,
    world: &World,
    ch: &Character,
    opts: &PipelineOptions,
    req: &PlanRequest,
) -> Option<Plan> {
    let mut p = Planner {
        profile,
        world,
        base: ch,
        opts,
        names: AbilityEnv::for_world(world, profile),
        goal: req.goal,
        target: req.target,
        evaluations: 0,
    };
    let current = ch.equipped;
    let before = p.measure(&current)?;
    let limits = &req.limits;

    let slots: Vec<usize> = (0..EQUIP_SLOTS)
        .filter(|&s| s != WEAPON_SLOT && req.open[s])
        .collect();
    // 0 is the empty slot: a heavy item can cost more than it gives
    let mut lists: Vec<Vec<i32>> = slots
        .iter()
        .map(|&s| {
            let mut v = req.candidates[s].clone();
            v.push(0);
            if !v.contains(&current[s]) {
                v.push(current[s]);
            }
            v.sort_unstable();
            v.dedup();
            v
        })
        .collect();

    let mut set = current;
    let mut best = before;
    let mut passes = 0;
    for pass in 0..req.max_passes.max(1) {
        passes += 1;
        let mut changed = false;
        for (k, &slot) in slots.iter().enumerate() {
            let pair = pair_of(slot);
            let mut scored: Vec<(i32, Measure)> = Vec::with_capacity(lists[k].len());
            for &item in &lists[k] {
                if item == set[slot] {
                    scored.push((item, best));
                    continue;
                }
                // the two fingers (and wrists) share a list; one item fills one of them
                if item != 0 && pair.is_some_and(|other| set[other] == item) {
                    continue;
                }
                let mut trial = set;
                trial[slot] = item;
                if let Some(m) = p.measure(&trial) {
                    scored.push((item, m));
                }
            }
            // only an item that does more for the goal displaces what is worn; among those, the
            // best by the full ranking
            let worn = best;
            let mut pick: Option<(i32, Measure)> = None;
            for (item, m) in &scored {
                if *item != set[slot]
                    && m.improves_on(&worn, limits)
                    && pick.is_none_or(|p| m.beats(&p.1, limits))
                {
                    pick = Some((*item, *m));
                }
            }
            if let Some((item, m)) = pick {
                best = m;
                set[slot] = item;
                changed = true;
            }
            if pass == 0 {
                // later passes look only at the slot's best few, the pick and today's item
                scored.sort_by(|a, b| {
                    if a.1.beats(&b.1, limits) {
                        std::cmp::Ordering::Less
                    } else if b.1.beats(&a.1, limits) {
                        std::cmp::Ordering::Greater
                    } else {
                        a.0.cmp(&b.0)
                    }
                });
                let mut keep: Vec<i32> = scored
                    .iter()
                    .take(req.keep_top.max(1))
                    .map(|s| s.0)
                    .collect();
                for must in [set[slot], current[slot], 0] {
                    if !keep.contains(&must) {
                        keep.push(must);
                    }
                }
                lists[k] = keep;
            }
        }
        if !changed {
            break;
        }
    }

    // ---- per slot: what the swap alone is worth today, and the runners-up in the final set
    let mut out = Vec::with_capacity(EQUIP_SLOTS);
    for slot in 0..EQUIP_SLOTS {
        let mut sp = SlotPlan {
            slot,
            current: current[slot],
            suggested: set[slot],
            gain_alone: 0.0,
            alternatives: Vec::new(),
        };
        if let Some(k) = slots.iter().position(|&s| s == slot) {
            if set[slot] != current[slot] {
                let mut alone = current;
                // a swap that needs its pair's slot emptied first is worth what both do
                if let Some(other) = pair_of(slot).filter(|&o| alone[o] == set[slot]) {
                    alone[other] = set[other];
                }
                alone[slot] = set[slot];
                if let Some(m) = p.measure(&alone) {
                    sp.gain_alone = m.primary - before.primary;
                }
            }
            let pair = pair_of(slot);
            let mut alts: Vec<(i32, Measure)> = Vec::new();
            for &item in &lists[k] {
                if item == set[slot] || item == 0 {
                    continue;
                }
                if pair.is_some_and(|other| set[other] == item) {
                    continue;
                }
                let mut trial = set;
                trial[slot] = item;
                if let Some(m) = p.measure(&trial) {
                    alts.push((item, m));
                }
            }
            alts.sort_by(|a, b| {
                if a.1.beats(&b.1, limits) {
                    std::cmp::Ordering::Less
                } else if b.1.beats(&a.1, limits) {
                    std::cmp::Ordering::Greater
                } else {
                    a.0.cmp(&b.0)
                }
            });
            sp.alternatives = alts
                .into_iter()
                .take(3)
                .map(|(item, m)| Alternative {
                    item,
                    delta: m.primary - best.primary,
                })
                .collect();
        }
        out.push(sp);
    }

    Some(Plan {
        slots: out,
        equipped: set,
        before,
        after: best,
        evaluations: p.evaluations,
        passes,
    })
}

/// The slots a plan leaves alone whatever the caller opens: the weapon, and the off-hand while a
/// two-handed weapon is held.
#[must_use]
pub fn fixed_slots(world: &World, equipped: &[i32; EQUIP_SLOTS]) -> Vec<usize> {
    let mut v = vec![WEAPON_SLOT];
    let two_handed = world
        .get_item(equipped[WEAPON_SLOT])
        .is_some_and(|w| w.item_type == 1 && matches!(w.weapon_type, 1 | 3));
    if two_handed {
        v.push(OFF_HAND_SLOT);
    }
    v
}
