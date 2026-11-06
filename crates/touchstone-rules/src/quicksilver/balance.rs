//! `Balance`: every number the 1.20 rules tune, under Quicksilver's own `[balance]` key names so a
//! table pasted from its config reads unchanged. Integers and percentages only.
//!
//! Written from Quicksilver's documented key list (`docs/config.md`, `[balance]`) and checked
//! against its engine vectors; see `docs/quicksilver-profile.md` part 3.

use serde::{Deserialize, Serialize};

use super::v120;

/// Why a balance was refused. The message names the key, as Quicksilver's does.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct BalanceError(pub String);

/// The most anchor points a [`LevelPct`] list holds.
pub const MAX_ANCHORS: usize = 8;

/// Anchor points of a per-level percentage: `(level, percent)` pairs with strictly rising levels.
/// Between two anchors the percentage runs in a straight line by level (kept as a fraction, not
/// rounded to a whole percent); below the first it is the first's, above the last the last's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<(u16, u32)>", into = "Vec<(u16, u32)>")]
pub struct Anchors {
    len: u8,
    points: [(u16, u32); MAX_ANCHORS],
}

impl Anchors {
    /// Builds anchors from 1 to [`MAX_ANCHORS`] points with rising levels.
    ///
    /// # Panics
    /// On an empty or oversized list, or levels that do not rise (a compile error in a `const`).
    #[must_use]
    pub const fn new<const N: usize>(points: [(u16, u32); N]) -> Self {
        assert!(N >= 1 && N <= MAX_ANCHORS, "1 to MAX_ANCHORS anchor points");
        let mut out = [(0u16, 0u32); MAX_ANCHORS];
        let mut i = 0;
        while i < N {
            assert!(
                i == 0 || points[i].0 > points[i - 1].0,
                "anchor levels must rise"
            );
            out[i] = points[i];
            i += 1;
        }
        Self {
            len: N as u8,
            points: out,
        }
    }

    /// The points, in level order.
    #[must_use]
    pub fn points(&self) -> &[(u16, u32)] {
        &self.points[..usize::from(self.len)]
    }

    /// The percentage at `level` as `(numerator, denominator)`.
    fn fraction(&self, level: u16) -> (i64, i64) {
        let pts = self.points();
        let (first_level, first_pct) = pts[0];
        if level <= first_level {
            return (i64::from(first_pct), 1);
        }
        for w in pts.windows(2) {
            let ((l0, p0), (l1, p1)) = (w[0], w[1]);
            if level < l1 {
                let span = i64::from(l1 - l0);
                let num =
                    i64::from(p0) * span + (i64::from(p1) - i64::from(p0)) * i64::from(level - l0);
                return (num, span);
            }
        }
        (i64::from(pts[pts.len() - 1].1), 1)
    }
}

impl TryFrom<Vec<(u16, u32)>> for Anchors {
    type Error = String;

    fn try_from(v: Vec<(u16, u32)>) -> Result<Self, String> {
        if v.is_empty() || v.len() > MAX_ANCHORS {
            return Err(format!(
                "{} anchor points; allowed 1 to {MAX_ANCHORS} [level, percent] pairs",
                v.len()
            ));
        }
        if v.windows(2).any(|w| w[1].0 <= w[0].0) {
            return Err("anchor levels must rise: [[30, 70], [60, 65]]".to_owned());
        }
        let mut points = [(0u16, 0u32); MAX_ANCHORS];
        points[..v.len()].copy_from_slice(&v);
        Ok(Self {
            len: v.len() as u8,
            points,
        })
    }
}

impl From<Anchors> for Vec<(u16, u32)> {
    fn from(a: Anchors) -> Self {
        a.points().to_vec()
    }
}

/// A class's percentage: one number for every level, or anchors by level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LevelPct {
    /// The same percentage at every level.
    One(u32),
    /// `(level, percent)` anchors.
    Points(Anchors),
}

impl LevelPct {
    fn fraction(&self, level: u16) -> (i64, i64) {
        match self {
            Self::One(p) => (i64::from(*p), 1),
            Self::Points(a) => a.fraction(level),
        }
    }

    /// The percentage at `level`, truncated to a whole percent.
    #[must_use]
    pub fn at(&self, level: u16) -> u32 {
        let (num, den) = self.fraction(level);
        (num / den) as u32
    }

    /// `amount` scaled by the exact percentage at `level` (the fraction is kept, not the
    /// truncated whole percent), truncating toward zero in 64 bits.
    #[must_use]
    pub fn scale(&self, level: u16, amount: i32) -> i32 {
        let (num, den) = self.fraction(level);
        let scaled = i64::from(amount) * num / (100 * den);
        scaled.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    }
}

/// A percentage per stock class, numbered 1 Warrior to 15 Mystic. A class outside the fifteen
/// reads 100 and `scale` leaves the amount whole.
macro_rules! class_pct {
    ($(#[$doc:meta])* $name:ident, $default:path) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(default, deny_unknown_fields)]
        #[allow(missing_docs)]
        pub struct $name {
            pub warrior: LevelPct,
            pub witchunter: LevelPct,
            pub paladin: LevelPct,
            pub cleric: LevelPct,
            pub priest: LevelPct,
            pub missionary: LevelPct,
            pub ninja: LevelPct,
            pub thief: LevelPct,
            pub bard: LevelPct,
            pub gypsy: LevelPct,
            pub warlock: LevelPct,
            pub mage: LevelPct,
            pub druid: LevelPct,
            pub ranger: LevelPct,
            pub mystic: LevelPct,
        }

        impl Default for $name {
            fn default() -> Self {
                $default
            }
        }

        impl $name {
            /// Every class at `pct` at every level.
            #[must_use]
            pub const fn even(pct: u32) -> Self {
                let p = LevelPct::One(pct);
                Self {
                    warrior: p,
                    witchunter: p,
                    paladin: p,
                    cleric: p,
                    priest: p,
                    missionary: p,
                    ninja: p,
                    thief: p,
                    bard: p,
                    gypsy: p,
                    warlock: p,
                    mage: p,
                    druid: p,
                    ranger: p,
                    mystic: p,
                }
            }

            /// The percentage of class number `class`, `None` outside 1..=15.
            #[must_use]
            pub const fn get(&self, class: u16) -> Option<LevelPct> {
                Some(match class {
                    1 => self.warrior,
                    2 => self.witchunter,
                    3 => self.paladin,
                    4 => self.cleric,
                    5 => self.priest,
                    6 => self.missionary,
                    7 => self.ninja,
                    8 => self.thief,
                    9 => self.bard,
                    10 => self.gypsy,
                    11 => self.warlock,
                    12 => self.mage,
                    13 => self.druid,
                    14 => self.ranger,
                    15 => self.mystic,
                    _ => return None,
                })
            }

            /// The whole percent of `class` at `level`; 100 outside the stock fifteen.
            #[must_use]
            pub fn of(&self, class: u16, level: u16) -> u32 {
                self.get(class).map_or(100, |p| p.at(level))
            }

            /// `amount` scaled by the class's percentage at `level` ([`LevelPct::scale`]).
            #[must_use]
            pub fn scale(&self, class: u16, level: u16, amount: i32) -> i32 {
                self.get(class).map_or(amount, |p| p.scale(level, amount))
            }
        }
    };
}

class_pct!(
    /// `balance.class_melee_pct`: the share of its damage range a class's non-stab blows deal.
    ClassMeleePct,
    v120::CLASS_MELEE_PCT
);
class_pct!(
    /// `balance.class_spell_pct`: the share of their damage a class's damage spells deal, after
    /// the casting-stat bonus.
    ClassSpellPct,
    v120::CLASS_SPELL_PCT
);
class_pct!(
    /// `balance.class_weave_pct`: the share of their damage a weaving class's weave procs deal,
    /// on top of `class_spell_pct`.
    ClassWeavePct,
    v120::CLASS_WEAVE_PCT
);

/// `balance.rank_weights`: how likely a monster is to target a player in each party rank. Carried
/// so a pasted table loads; no touchstone function uses it (MME has no party ranks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RankWeights {
    /// `balance.rank_weights.front`.
    pub front: u32,
    /// `balance.rank_weights.mid`. At least 1.
    pub mid: u32,
    /// `balance.rank_weights.back`.
    pub back: u32,
    /// `balance.rank_weights.solo`.
    pub solo: u32,
}

impl Default for RankWeights {
    fn default() -> Self {
        v120::RANK_WEIGHTS
    }
}

/// Every number the 1.20 rules tune. Field names are Quicksilver's `[balance]` keys.
/// Missing keys read as the 1.20.0 values; an unknown key is an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Balance {
    /// A stab costs this percentage of a normal blow's energy, at most the round's 1000; the rest of the
    /// round is normal blows.
    pub backstab_energy_pct: u32,
    /// Stealth adds `stealth / backstab_stealth_div` to a stab. At least 1.
    pub backstab_stealth_div: u32,
    /// A stab's damage is multiplied by `(100 + L * backstab_level_pct / 100)` percent.
    pub backstab_level_pct: u32,
    /// A stab crit rolls from max up to this percentage of max. touchstone reads 0 as "a stab
    /// never crits" (the neutral setting); the engine would deal the max every time.
    pub backstab_crit_max_pct: u32,
    /// Percentage chance of the Thief's extra blow, 0..=100.
    pub opportunist_pct: u32,
    /// The opportunist bonus is multiplied by `(100 + L * opportunist_level_pct / 100)` percent.
    pub opportunist_level_pct: u32,
    /// Percentage chance of a weave proc, 0..=100.
    pub weave_pct: u32,
    /// The proc chance for the magery-1 hybrids (Paladin, Ranger), 0..=100.
    pub weave_pct_magery1: u32,
    /// A proc's expected damage is `weave_budget_per_level * level`.
    pub weave_budget_per_level: u32,
    /// The least power of a proc, a percentage of the spell's damage.
    pub weave_power_min_pct: u32,
    /// The most power of a proc, a percentage of the spell's damage.
    pub weave_power_max_pct: u32,
    /// The share of the casting-stat damage bonus a proc gets, 0..=100.
    pub weave_stat_bonus_pct: u32,
    /// Per mille added to spell damage per point of casting stat over 50.
    pub spell_stat_permille: u32,
    /// `rest`: mana regeneration multiplier.
    pub rest_mana_mult: u32,
    /// `meditate`: mana regeneration multiplier.
    pub meditate_mana_mult: u32,
    /// Crit chance above this counts a third: `(crit - knee) / 3 + knee`.
    pub crit_knee: u32,
    /// The casting stat above this counts a third.
    pub spell_stat_knee: u32,
    /// A player's stat part of magic resistance, `(Int + 3 Wil) / 4`, above this counts a third.
    pub mr_stat_knee: u32,
    /// A player's Agility above this counts a third in the dodge term.
    pub dodge_agi_knee: u32,
    /// A stat trains at the floor price up to this value (the soft cap is the larger of this
    /// and the race maximum).
    pub softcap_floor: u32,
    /// Character points per stat point from the race maximum up to `softcap_floor`.
    pub softcap_floor_price: u32,
    /// Character points per point in the bands +1..10, +11..20 and +21..30 over the soft cap.
    pub softcap_band_prices: [u32; 3],
    /// The most a monster's resist roll cuts an area spell, a percentage, 0..=100.
    pub aoe_resist_cut_max_pct: u32,
    /// The cut is the monster's magic resistance over this, up to the maximum. At least 1.
    pub aoe_resist_cut_div: u32,
    /// Monster targeting weights (unused by touchstone).
    pub rank_weights: RankWeights,
    /// `guard`: base percentage chance to intercept (unused by touchstone).
    pub guard_chance_base: u32,
    /// `guard`: percentage added per point of combat rating (unused by touchstone).
    pub guard_chance_per_combat: u32,
    /// The level past which backstab, opportunist and weave stop scaling.
    pub scalecap: u32,
    /// `balance.class_melee_pct`.
    pub class_melee_pct: ClassMeleePct,
    /// `balance.class_spell_pct`.
    pub class_spell_pct: ClassSpellPct,
    /// `balance.class_weave_pct`.
    pub class_weave_pct: ClassWeavePct,
}

impl Default for Balance {
    fn default() -> Self {
        Self::V120
    }
}

impl Balance {
    /// The frozen 1.20.0 values, built from the per-key constants in `v120.rs`.
    pub const V120: Self = v120::BALANCE;

    /// Every parametric rule at its stock-equivalent setting, so a Quicksilver profile built on it
    /// answers as MME does (layer 1 of `docs/quicksilver-profile.md` part 4): crit knee 40,
    /// stealth divisor 10, level percentage 100, the other knees out of reach, class shares 100,
    /// opportunist and weave chances 0, area cut 0, rest and meditate multiples 1, no
    /// casting-stat bonus, no scale cap, no stab crit (`backstab_crit_max_pct` 0 reads as off
    /// in touchstone; MME's stab never crits) and a stab that fills the round (`backstab_energy_pct`
    /// 100000: the energy is capped at the round's, so no blow follows the stab). Keys with no stock
    /// counterpart (weave sizing, targeting, guard) keep their 1.20.0 values. The soft-cap keys are floor 0 and
    /// 10 a point in every band, the nearest the type comes to MME's ladder (equal up to the race
    /// maximum; Q-007 decides above it).
    pub const NEUTRAL: Self = v120::NEUTRAL;

    /// Whether every value is the stock-equivalent one ([`Balance::NEUTRAL`]). Rules the balance
    /// has no key for (Alter Energy Usage) read this to switch off, so the neutral profile answers
    /// as MME does.
    #[must_use]
    pub fn is_neutral(&self) -> bool {
        *self == Self::NEUTRAL
    }

    /// Whether every value is the 1.20.0 one.
    #[must_use]
    pub fn is_v120(&self) -> bool {
        *self == Self::V120
    }

    /// A value above `knee` counts a third of the excess: `(value - knee) / 3 + knee`
    /// (truncating). Used by the crit, casting-stat, magic resistance and dodge knees.
    #[must_use]
    pub fn knee_third(value: i64, knee: u32) -> i64 {
        let knee = i64::from(knee);
        if value > knee {
            (value - knee) / 3 + knee
        } else {
            value
        }
    }

    /// Checks the values a rule divides by or bounds; the error names the key.
    ///
    /// # Errors
    /// A divisor of 0, a percentage chance above 100, or a weave power minimum above the maximum.
    pub fn validate(&self) -> Result<(), BalanceError> {
        for (key, v) in [
            ("backstab_stealth_div", self.backstab_stealth_div),
            ("aoe_resist_cut_div", self.aoe_resist_cut_div),
            ("rank_weights.mid", self.rank_weights.mid),
        ] {
            if v == 0 {
                return Err(BalanceError(format!(
                    "balance.{key} = 0; allowed 1 or more"
                )));
            }
        }
        for (key, v) in [
            ("opportunist_pct", self.opportunist_pct),
            ("weave_pct", self.weave_pct),
            ("weave_pct_magery1", self.weave_pct_magery1),
            ("weave_stat_bonus_pct", self.weave_stat_bonus_pct),
            ("aoe_resist_cut_max_pct", self.aoe_resist_cut_max_pct),
        ] {
            if v > 100 {
                return Err(BalanceError(format!(
                    "balance.{key} = {v} is out of range; allowed 0..=100"
                )));
            }
        }
        if self.weave_power_min_pct > self.weave_power_max_pct {
            return Err(BalanceError(format!(
                "balance.weave_power_min_pct = {} is above balance.weave_power_max_pct = {}",
                self.weave_power_min_pct, self.weave_power_max_pct
            )));
        }
        Ok(())
    }
}
