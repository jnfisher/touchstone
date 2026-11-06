//! Quicksilver's weave (row R6 of `docs/quicksilver-profile.md`), first half: which spell a
//! weave proc fires, how big a proc may be at a level, and what one volley is expected to deal.
//!
//! A hybrid class (magery level 1 or 2, never Kai) may add a free spell volley to a normal melee
//! hit that leaves its target alive. The proc fires the best single-target attack spell the
//! character can cast, rolled at a power that holds the volley's expected damage to a per-level
//! budget. A Bard's proc is one song hit that deals the budget itself. Wiring the chance into the
//! round is Q-013b; this module has the pure pieces.
//!
//! Port of the rule in Quicksilver's `crates/silvermere-rules/src/v120/weave.rs`, with the way its
//! engine applies a proc's hits (`crates/silvermere-engine/src/combat/weave.rs`). MME has nothing
//! like it, so the arithmetic follows Quicksilver's integer forms, which `fixtures/quicksilver/weave.json`
//! pins. Under [`Profile::Stock`] there is no weave: nothing is picked and nothing is dealt.

use std::cmp::Reverse;

use touchstone_model::{AbilitySlot, Class, Magery, Spell};

use super::Balance;
use crate::Profile;
use crate::spell_cast::{SpellCasterProfile, UsableEnv, scale_spell_damage};
use crate::stats::{get_class_magery, get_class_magery_lvl};

/// The energy one volley is simulated on: a full round of auto-combat.
pub const VOLLEY_ENERGY: i32 = 1000;
/// The most casts one volley makes.
pub const MAX_CASTS: u32 = 20;
/// How deep a "cast on ending" chain is followed.
const MAX_CHAIN_DEPTH: u32 = 8;
/// Rolls of the engine's 1 to 99 resist die.
const RESIST_ROLLS: i32 = 99;
/// The highest resist chance the engine's rolls allow, in rolls out of 99.
const RESIST_CAP: i32 = 98;
/// Magic resistance 50 takes nothing off and adds nothing; each point under it adds a percent.
const MR_NEUTRAL: i32 = 50;
const MRCUT_MAX: i32 = 50;
const MRCUT_ANTIMAG: i32 = 75;

// Ability numbers (the MME and silvermere numbering agree).
const AB_DAMAGE: i32 = 1;
const AB_DAMAGE_ARMOUR: i32 = 17;
const AB_KILL_DEAD: i32 = 23;
const AB_AFFECTS_ANIMAL: i32 = 80;
const AB_AFFECTS_LIVING: i32 = 108;
const AB_NON_MAGICAL: i32 = 144;
const AB_TELEPORT_ROOM: i32 = 140;
const AB_TELEPORT_MAP: i32 = 141;
const AB_END_CAST: i32 = 151;
const AB_END_CAST_CHANCE: i32 = 164;

/// A class that weaves, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weaver {
    /// The proc chance on a qualifying hit, in percent.
    pub chance_pct: u32,
    /// A Bard: the proc deals the budget as one song hit instead of a power-scaled volley.
    pub bard: bool,
    /// The class number, for its share of a proc (`class_weave_pct`).
    pub class: u16,
}

/// Whether the class weaves: magery Mage, Priest, Druid or Bard at class magery level 2 procs at
/// `weave_pct`, at level 1 at `weave_pct_magery1`. Level 3 casters, Kai and no magic never do.
/// Always `None` under Stock.
#[must_use]
pub fn weaver(profile: Profile, class: &Class) -> Option<Weaver> {
    let Profile::Quicksilver(b) = profile else {
        return None;
    };
    let magery = get_class_magery(Some(class));
    if matches!(magery, Magery::None | Magery::Kai) {
        return None;
    }
    let chance_pct = match get_class_magery_lvl(Some(class)) {
        1 => b.weave_pct_magery1,
        2 => b.weave_pct,
        _ => return None,
    };
    Some(Weaver {
        chance_pct,
        bard: magery == Magery::Bard,
        class: u16::try_from(class.number).unwrap_or(0),
    })
}

/// The level the budget reads: the character's, held to `scalecap`.
#[must_use]
pub fn scaled_level(b: &Balance, level: i32) -> i32 {
    level.min(i32::try_from(b.scalecap).unwrap_or(i32::MAX))
}

/// A proc's damage budget at a level: `weave_budget_per_level` times the scaled level. 0 under Stock.
#[must_use]
pub fn budget(profile: Profile, level: i32) -> i64 {
    match profile {
        Profile::Stock => 0,
        Profile::Quicksilver(b) => {
            i64::from(b.weave_budget_per_level) * i64::from(scaled_level(b, level))
        }
    }
}

fn find(spells: &[Spell], number: i32) -> Option<&Spell> {
    spells.iter().find(|s| s.number == number)
}

fn has(spell: &Spell, ability: i32) -> bool {
    spell.abilities.iter().any(|s| s.ability == ability)
}

fn value_of(spell: &Spell, ability: i32) -> i32 {
    spell
        .abilities
        .iter()
        .find(|s| s.ability == ability)
        .map_or(0, |s| s.value)
}

fn is_damage_slot(slot: &AbilitySlot) -> bool {
    slot.ability == AB_DAMAGE || slot.ability == AB_DAMAGE_ARMOUR
}

fn deals_damage(spell: &Spell) -> bool {
    spell.abilities.iter().any(is_damage_slot)
}

/// The strength range of a spell at a caster level, in the engine's integer form: the minimum and
/// maximum each grow by `inc * level / levels`, the level held to the spell's cap, and a minimum
/// above the maximum comes down to it. (MME's currency form can come out one lower at some levels.)
#[must_use]
pub fn amount_range(spell: &Spell, caster_level: i32) -> (i32, i32) {
    let level = if spell.cap > 0 && caster_level > spell.cap {
        spell.cap
    } else {
        caster_level
    };
    let grow = |inc: i32, lvls: i32| if lvls > 0 { inc * level / lvls } else { 0 };
    let min = spell.min_base + grow(spell.min_inc, spell.min_inc_lvls);
    let max = spell.max_base + grow(spell.max_inc, spell.max_inc_lvls);
    (min.min(max), max)
}

/// Twice the average of one hit of the spell: `min + max` for a rolled damage slot, twice the
/// value for a slot with its own.
fn twice_hit_average(spell: &Spell, level: i32) -> i64 {
    let (min, max) = amount_range(spell, level);
    spell
        .abilities
        .iter()
        .filter(|s| is_damage_slot(s))
        .map(|s| {
            if s.value == 0 {
                i64::from(min) + i64::from(max)
            } else {
                2 * i64::from(s.value)
            }
        })
        .sum()
}

/// The spell a landed instant cast chains into: its "cast on ending" spell when the chain's chance
/// is the full 100. A chance below 100 is rolled per cast and is not followed (no player spell has one).
fn sure_chain(spell: &Spell) -> Option<i32> {
    if spell.dur != 0 {
        return None;
    }
    let next = value_of(spell, AB_END_CAST);
    let chance = if has(spell, AB_END_CAST_CHANCE) {
        value_of(spell, AB_END_CAST_CHANCE)
    } else {
        100
    };
    // the chained spell number is a 16-bit field
    (next != 0 && chance >= 100).then_some(i32::from(next as i16))
}

/// One volley: the spell of each hit in casting order, and twice the sum of their average damage.
#[derive(Debug, Clone, PartialEq)]
pub struct Volley<'a> {
    pub hits: Vec<&'a Spell>,
    pub twice_average: i64,
}

impl Volley<'_> {
    /// The volley's average damage.
    #[must_use]
    pub fn average(&self) -> f64 {
        self.twice_average as f64 / 2.0
    }
}

/// The volley one round of the spell makes on [`VOLLEY_ENERGY`]: it casts while the energy lasts
/// (at most [`MAX_CASTS`] tries, each paying the spell's energy, at least 1), and a cast whose sure
/// chain fires hands the rest of the round to the chained spell, which loops in its turn. A cast
/// with no damage slot adds no hit but still pays its energy.
#[must_use]
pub fn volley<'a>(spell: &'a Spell, level: i32, spells: &'a [Spell]) -> Volley<'a> {
    fn cast<'a>(
        rec: &'a Spell,
        level: i32,
        spells: &'a [Spell],
        energy: &mut i32,
        depth: u32,
        out: &mut Volley<'a>,
    ) {
        let cost = rec.energy_cost.clamp(0, i32::from(u16::MAX)).max(1);
        let mut tries = 0;
        while tries < MAX_CASTS && *energy >= cost {
            tries += 1;
            *energy -= cost;
            if deals_damage(rec) {
                out.hits.push(rec);
                out.twice_average += twice_hit_average(rec, level);
            }
            if depth < MAX_CHAIN_DEPTH
                && let Some(next) = sure_chain(rec).and_then(|n| find(spells, n))
            {
                cast(next, level, spells, energy, depth + 1, out);
                return;
            }
        }
    }
    let mut out = Volley {
        hits: Vec::new(),
        twice_average: 0,
    };
    let mut energy = VOLLEY_ENERGY;
    cast(spell, level, spells, &mut energy, 0, &mut out);
    out
}

/// The power a volley is rolled at, in percent of its normal roll: `budget * 100 / average` held
/// to `weave_power_min_pct..=weave_power_max_pct`; a volley that deals nothing gets the maximum.
/// 0 under Stock.
#[must_use]
pub fn power_pct(profile: Profile, budget: i64, volley: &Volley<'_>) -> u32 {
    let Profile::Quicksilver(b) = profile else {
        return 0;
    };
    let (lo, hi) = (
        i64::from(b.weave_power_min_pct),
        i64::from(b.weave_power_max_pct),
    );
    if volley.twice_average <= 0 {
        return hi as u32;
    }
    // the average is held as twice itself
    (budget * 200 / volley.twice_average).clamp(lo, hi.max(lo)) as u32
}

/// What a weave target is: its resistances and the facts that decide which spells spare it.
/// The default is a monster with none of them (and no magic resistance).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WeaveTarget {
    /// Magic resistance; below 1 reads as 1.
    pub mr: i32,
    pub anti_magic: bool,
    /// Element resistance in percent, by the spell's attack type.
    pub cold: i32,
    pub fire: i32,
    pub stone: i32,
    pub lightning: i32,
    pub water: i32,
    pub poison: i32,
    /// "Spell Immunity": spells of a lower level than this do nothing to it.
    pub spell_immunity: i32,
    pub undead: bool,
    pub animal: bool,
    pub non_living: bool,
}

impl WeaveTarget {
    /// The target's resistance to a spell's element (attack types 4 and above 6 have none).
    fn element_resist(&self, att_type: i32) -> i32 {
        match att_type {
            0 => self.cold,
            1 => self.fire,
            2 => self.stone,
            3 => self.lightning,
            5 => self.water,
            6 => self.poison,
            _ => 0,
        }
    }

    /// Whether the spell spares the target: Kill Dead needs an undead one, Affects Animal an animal,
    /// and Affects Living refuses a non-living one.
    fn spares(&self, spell: &Spell) -> bool {
        (has(spell, AB_KILL_DEAD) && !self.undead)
            || (has(spell, AB_AFFECTS_ANIMAL) && !self.animal)
            || (has(spell, AB_AFFECTS_LIVING) && self.non_living)
    }

    /// Whether the target takes nothing from the spell: its Spell Immunity is above the spell's
    /// level (Non Magical spells pass it), or the spell spares it. The pick never chooses such a spell.
    fn immune_to(&self, spell: &Spell) -> bool {
        (!has(spell, AB_NON_MAGICAL) && self.spell_immunity > spell.req_level) || self.spares(spell)
    }
}

/// Whether the spell may be woven by a character of this class, level and alignment (1 good, 2
/// neutral, 3 evil): an instant spell aimed at one target (User, Monster or Monster or User, never an
/// area) with an Inflict Damage or Inflict Damage on Armour slot, without Kill Dead or a teleport,
/// that [`UsableEnv::spell_is_usable`] allows and that the class can learn.
///
/// MME's spell rows carry no spell type, so Quicksilver's "auto-combat" test is approximated by
/// "a player class can learn it" (see `docs/differences.md`).
#[must_use]
pub fn is_candidate(
    env: &UsableEnv<'_>,
    spell: &Spell,
    class: i32,
    level: i16,
    align: i16,
) -> bool {
    matches!(spell.targets, 0 | 4 | 8)
        && spell.dur == 0
        && deals_damage(spell)
        && !has(spell, AB_KILL_DEAD)
        && !has(spell, AB_TELEPORT_ROOM)
        && !has(spell, AB_TELEPORT_MAP)
        && env.spell_is_usable(spell.number, class, level.max(1), align, true)
}

/// The spell a weave fires, its volley and the power it is rolled at.
#[derive(Debug, Clone, PartialEq)]
pub struct Pick<'a> {
    /// The first spell of its chain, which names the proc.
    pub spell: &'a Spell,
    pub volley: Volley<'a>,
    /// The power each hit is rolled at in percent. A Bard's proc ignores it and deals the budget.
    pub power_pct: u32,
}

/// The weave pick: among the candidates the target does not spare, the one with the highest volley
/// average (ties to the higher spell level, then the lower number), with its volley and power.
/// `None` under Stock, for a class that does not weave, and when nothing qualifies.
#[must_use]
pub fn pick<'a>(
    env: &UsableEnv<'a>,
    class: &Class,
    level: i16,
    align: i16,
    target: Option<&WeaveTarget>,
) -> Option<Pick<'a>> {
    let profile = env.profile;
    weaver(profile, class)?;
    let lvl = i32::from(level);
    let best = env
        .spells
        .iter()
        .filter(|s| is_candidate(env, s, class.number, level, align))
        .filter(|s| target.is_none_or(|t| !t.immune_to(s)))
        .map(|s| (s, volley(s, lvl, env.spells)))
        .max_by_key(|(s, v)| (v.twice_average, s.req_level, Reverse(s.number)))?;
    let (spell, volley) = best;
    let power_pct = power_pct(profile, budget(profile, lvl), &volley);
    Some(Pick {
        spell,
        volley,
        power_pct,
    })
}

/// The expected damage of one proc.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ProcExpectation {
    /// The damage if the target's resist roll does not stop the proc: the exact mean over the rolls
    /// of every hit, through element and magic resistance.
    pub unresisted: f64,
    /// How many of the engine's 99 resist rolls stop the proc (0 when the spell cannot be resisted).
    pub resist_rolls: i32,
    /// `unresisted` times the share of resist rolls that let it through.
    pub expected: f64,
}

/// The casting stat a proc is scaled by: the part over 50 held to `weave_stat_bonus_pct` of itself.
fn weave_stat(b: &Balance, stat: i32) -> i32 {
    let over = i64::from(stat - 50).max(0) * i64::from(b.weave_stat_bonus_pct) / 100;
    (50 + over).clamp(0, i64::from(i32::MAX)) as i32
}

/// A damage slot's hit on a monster, given the strength it works with (its own value, or the
/// rolled strength after the target's element). "Inflict Damage" takes the caster's spell damage
/// bonus and the casting stat; "Inflict Damage on Armour" also loses to magic resistance.
fn slot_damage(
    profile: Profile,
    slot: &AbilitySlot,
    amount: i32,
    who: &SpellCasterProfile,
    t: &WeaveTarget,
) -> i32 {
    let pct = i32::from(who.spell_dmg_bonus);
    if slot.ability == AB_DAMAGE {
        let bonus = (i64::from(amount) * (i64::from(pct) + 100) / 100)
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        return scale_spell_damage(profile, bonus, who);
    }
    let boosted = scale_spell_damage(profile, pct.wrapping_mul(amount) / 100 + amount, who);
    let mr = t.mr.max(1);
    let cut = if t.anti_magic {
        (mr / 2).clamp(0, MRCUT_ANTIMAG)
    } else if mr - MR_NEUTRAL > 0 {
        ((mr - MR_NEUTRAL) / 2).min(MRCUT_MAX)
    } else {
        0
    };
    if cut == 0 {
        if t.anti_magic {
            boosted
        } else {
            boosted.wrapping_mul(MR_NEUTRAL - mr) / 100 + boosted
        }
    } else {
        boosted - boosted.wrapping_mul(cut) / 100
    }
}

/// The exact mean damage of one proc, hit by hit in the order the engine applies them.
///
/// A rolled hit is the spell's roll times the pick's power, cut to the class's weave share, cut by
/// the target's element, then run through each damage slot. The slot's own value, if it has one,
/// stands in for the roll and skips the first three steps. A Bard's proc is one hit of the class's
/// share of the budget. The casting stat counts at `weave_stat_bonus_pct` of its excess over 50, and
/// the class spell share applies after it. `who` is the character (real level, class, casting stat,
/// spell damage bonus). The target is assumed to survive every hit, as the steady-state model does.
///
/// All zero under Stock.
#[must_use]
pub fn expected_proc(
    profile: Profile,
    pick: &Pick<'_>,
    weaver: &Weaver,
    who: &SpellCasterProfile,
    target: &WeaveTarget,
) -> ProcExpectation {
    let Profile::Quicksilver(b) = profile else {
        return ProcExpectation::default();
    };
    let level = who.level;
    let share = |amount: i32| {
        b.class_weave_pct.scale(
            weaver.class,
            u16::try_from(level.max(0)).unwrap_or(u16::MAX),
            amount,
        )
    };
    let caster = SpellCasterProfile {
        casting_stat: weave_stat(b, who.casting_stat),
        ..*who
    };
    let mut unresisted = 0.0;
    let hits: Vec<&Spell> = if weaver.bard {
        vec![pick.spell]
    } else {
        pick.volley.hits.clone()
    };
    for spell in hits {
        let resist = target.element_resist(spell.att_type);
        // The strengths the hit is rolled at, one per roll of the spell's range (a Bard's single
        // hit is the share of the budget).
        let strengths: Vec<i32> = if weaver.bard {
            let amount = budget(profile, level).clamp(0, i64::from(i32::MAX)) as i32;
            vec![share(amount)]
        } else {
            let (min, max) = amount_range(spell, level);
            (min..=max)
                .map(|roll| share((i64::from(roll) * i64::from(pick.power_pct) / 100) as i32))
                .collect()
        };
        let mut total = 0i64;
        for &power in &strengths {
            let after_element = (i64::from(power) * (100 - i64::from(resist)) / 100)
                .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                as i32;
            for slot in spell.abilities.iter().filter(|s| is_damage_slot(s)) {
                let amount = if slot.value == 0 {
                    after_element
                } else {
                    slot.value
                };
                total += i64::from(slot_damage(profile, slot, amount, &caster, target));
            }
        }
        if !strengths.is_empty() {
            unresisted += total as f64 / strengths.len() as f64;
        }
    }
    let can_resist =
        pick.spell.type_of_resists == 2 || (pick.spell.type_of_resists == 1 && target.anti_magic);
    let resist_rolls = if can_resist {
        (target.mr.max(1) / 2).clamp(0, RESIST_CAP)
    } else {
        0
    };
    ProcExpectation {
        unresisted,
        resist_rolls,
        expected: unresisted * f64::from(RESIST_ROLLS - resist_rolls) / f64::from(RESIST_ROLLS),
    }
}

/// The expected weave damage of one qualifying melee hit: the proc chance times the pick's expected
/// proc. 0 under Stock, for a class that does not weave, and when no spell qualifies. The weave
/// fires only on a hit that leaves the target alive; like Quicksilver's balance tool, this assumes
/// it does (Q-013b wires it into the round).
#[must_use]
pub fn expected_per_hit(
    env: &UsableEnv<'_>,
    class: &Class,
    align: i16,
    who: &SpellCasterProfile,
    target: &WeaveTarget,
) -> f64 {
    let Some(w) = weaver(env.profile, class) else {
        return 0.0;
    };
    let level = i16::try_from(who.level).unwrap_or(i16::MAX);
    let Some(p) = pick(env, class, level, align, Some(target)) else {
        return 0.0;
    };
    f64::from(w.chance_pct) / 100.0 * expected_proc(env.profile, &p, &w, who, target).expected
}
