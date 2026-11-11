//! The weapon simulator's dice: the blows `CalculateAttack` describes, rolled round by round.
//!
//! touchstone's own; MME has no such window (`docs/differences.md`, "Weapon simulator"). Every
//! number a blow is rolled from comes out of [`crate::attack::calculate_attack`], so the mean of a
//! long run lands on MME's damage a round and the simulation adds what MME has no figure for: the
//! spread of a round, how often it does nothing, and how many rounds a target lasts.
//!
//! Pure and deterministic: the caller owns the generator and its seed.

use serde::Serialize;

use crate::attack::{AttackDamage, MAX_SWINGS};

/// The energy a round gives, MME's `1000`.
const ROUND_ENERGY: u32 = 1000;

/// xoshiro256++, seeded through splitmix64. Small, fast on a 64-bit ARM core and good enough for
/// dice; kept here so the crate needs no dependency for it.
#[derive(Debug, Clone)]
pub struct Rng([u64; 4]);

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        let mut s = seed;
        Self(std::array::from_fn(|_| splitmix(&mut s)))
    }

    /// A generator for one stream of a run: the same seed, weapon and stream always roll the same
    /// dice, whichever other weapons are simulated beside it.
    #[must_use]
    pub fn for_stream(seed: u64, weapon: i32, stream: u32) -> Self {
        let mut s = seed ^ (u64::from(weapon.cast_unsigned()) << 32) ^ u64::from(stream);
        let mixed = splitmix(&mut s);
        Self::new(mixed)
    }

    fn next(&mut self) -> u64 {
        let s = &mut self.0;
        let out = s[0].wrapping_add(s[3]).rotate_left(23).wrapping_add(s[0]);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        out
    }

    /// A whole number below `n` (0 for `n` of 0).
    fn below(&mut self, n: u32) -> u32 {
        // multiply-shift: the bias for ranges this small is far below the simulation's noise
        (((self.next() >> 32) * u64::from(n)) >> 32) as u32
    }

    /// True `pct` times in a hundred.
    fn percent(&mut self, pct: u32) -> bool {
        self.below(100) < pct
    }

    /// A number in `[0, 1)`.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn between(&mut self, lo: u32, hi: u32) -> u32 {
        if hi <= lo {
            lo
        } else {
            lo + self.below(hi - lo + 1)
        }
    }
}

/// One swing of an attack as the dice see it, taken from `CalculateAttack`'s answer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blow {
    /// Chance to land, percent, the defender's dodge already taken off.
    pub hit_pct: u32,
    pub crit_pct: u32,
    /// Damage of a plain hit, after damage reduction.
    pub min: u32,
    pub max: u32,
    pub crit_min: u32,
    pub crit_max: u32,
    /// The weapon's cast spell: the chance it goes off on a landed blow and what it then adds.
    pub extra_chance: f64,
    pub extra_dmg: u32,
    /// Quicksilver's opportunist: the chance of a bonus on a blow that dealt damage, and its mean.
    pub opp_pct: u32,
    pub opp_dmg: u32,
    /// Energy one swing costs.
    pub energy: u32,
}

fn unsigned(v: i32) -> u32 {
    v.max(0).cast_unsigned()
}

impl Blow {
    /// The blow of an attack `CalculateAttack` answered; `None` for the all-zero answer (an attack
    /// the character or weapon cannot make).
    #[must_use]
    pub fn from_attack(a: &AttackDamage) -> Option<Self> {
        if a.swings <= 0.0 {
            return None;
        }
        let per_hit = unsigned(a.avg_extra_hit);
        let per_swing = unsigned(a.avg_extra_swing);
        // MME keeps the spell's average when it fires and its average over every swing; their
        // ratio is the chance it fires.
        let (extra_chance, extra_dmg) = match (per_hit, per_swing) {
            (_, 0) => (0.0, 0),
            (0, s) => (1.0, s),
            (h, s) => ((f64::from(s) / f64::from(h)).min(1.0), h),
        };
        Some(Self {
            hit_pct: unsigned(i32::from(a.hit_chance)).min(100),
            crit_pct: unsigned(i32::from(a.crit_chance)).min(100),
            min: unsigned(a.min_dmg),
            max: unsigned(a.max_dmg),
            crit_min: unsigned(a.min_crit),
            crit_max: unsigned(a.max_crit),
            extra_chance,
            extra_dmg,
            opp_pct: unsigned(i32::from(a.opportunist_pct)).min(100),
            opp_dmg: unsigned(a.opportunist_avg),
            energy: if a.energy > 0 {
                a.energy.cast_unsigned()
            } else {
                ROUND_ENERGY
            },
        })
    }

    /// Whether any roll of this blow can deal damage.
    #[must_use]
    pub fn can_hurt(&self) -> bool {
        self.hit_pct > 0
            && (self.max > 0
                || (self.crit_pct > 0 && self.crit_max > 0)
                || (self.extra_chance > 0.0 && self.extra_dmg > 0))
    }

    fn roll(&self, rng: &mut Rng) -> u32 {
        if !rng.percent(self.hit_pct) {
            return 0;
        }
        let mut dmg = if self.crit_pct > 0 && rng.percent(self.crit_pct) {
            rng.between(self.crit_min, self.crit_max)
        } else {
            rng.between(self.min, self.max)
        };
        if self.opp_pct > 0 && dmg > 0 && rng.percent(self.opp_pct) {
            dmg += self.opp_dmg;
        }
        if self.extra_chance > 0.0 && rng.unit() < self.extra_chance {
            dmg += self.extra_dmg;
        }
        dmg
    }
}

/// The swings of successive rounds: what the round's energy and the last round's leftover pay
/// for, at most five, as frmSwingCalc's `CalcSwings` counts them.
#[derive(Debug, Clone, Copy)]
struct Energy {
    cost: u32,
    have: u32,
}

impl Energy {
    fn new(cost: u32) -> Self {
        Self {
            cost: cost.max(1),
            have: ROUND_ENERGY,
        }
    }

    fn swings(&mut self) -> u32 {
        let n = self.have / self.cost;
        self.have = self.have % self.cost + ROUND_ENERGY;
        n.min(MAX_SWINGS as u32)
    }
}

/// How the damage of many simulated rounds is spread.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Default)]
pub struct RoundStats {
    pub rounds: u32,
    pub mean: f64,
    /// Standard deviation of a round.
    pub sd: f64,
    pub min: u32,
    pub p10: u32,
    pub median: u32,
    pub p90: u32,
    pub max: u32,
    /// Percent of rounds that dealt nothing.
    pub zero_pct: f64,
}

/// `buf` holds one damage per round; it is sorted in place.
fn stats(buf: &mut [u32]) -> RoundStats {
    if buf.is_empty() {
        return RoundStats::default();
    }
    buf.sort_unstable();
    let n = buf.len();
    let sum: u64 = buf.iter().map(|d| u64::from(*d)).sum();
    let mean = sum as f64 / n as f64;
    let var = buf
        .iter()
        .map(|d| (f64::from(*d) - mean).powi(2))
        .sum::<f64>()
        / n as f64;
    let at = |p: usize| buf[(n - 1) * p / 100];
    let zeros = buf.partition_point(|d| *d == 0);
    RoundStats {
        rounds: u32::try_from(n).unwrap_or(u32::MAX),
        mean,
        sd: var.sqrt(),
        min: buf[0],
        p10: at(10),
        median: at(50),
        p90: at(90),
        max: buf[n - 1],
        zero_pct: zeros as f64 * 100.0 / n as f64,
    }
}

/// `rounds` rounds of one attack fought back to back, leftover energy carried from each to the
/// next. `buf` is scratch space the caller can reuse between runs.
pub fn simulate_rounds(blow: &Blow, rounds: u32, rng: &mut Rng, buf: &mut Vec<u32>) -> RoundStats {
    buf.clear();
    let mut energy = Energy::new(blow.energy);
    for _ in 0..rounds {
        let mut dmg = 0;
        for _ in 0..energy.swings() {
            dmg += blow.roll(rng);
        }
        buf.push(dmg);
    }
    stats(buf)
}

/// An opening round: the stab and, under Quicksilver, the normal blows the rest of the round pays
/// for (`attack::calc_stab_opener`). Stock's stab is the whole round.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Opener {
    pub stab: Blow,
    pub follow: Option<(Blow, u32)>,
}

impl Opener {
    fn roll(&self, rng: &mut Rng) -> u32 {
        let mut dmg = self.stab.roll(rng);
        if let Some((blow, n)) = &self.follow {
            for _ in 0..*n {
                dmg += blow.roll(rng);
            }
        }
        dmg
    }
}

/// `trials` opening rounds, each on its own (a fight has one).
pub fn simulate_openers(
    opener: &Opener,
    trials: u32,
    rng: &mut Rng,
    buf: &mut Vec<u32>,
) -> RoundStats {
    buf.clear();
    for _ in 0..trials {
        buf.push(opener.roll(rng));
    }
    stats(buf)
}

/// How long a target of a given hit points lasts.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct KillStats {
    /// Mean rounds to the kill; a fight that ran out of rounds counts as the limit.
    pub mean: f64,
    pub median: u32,
    pub p90: u32,
    /// Percent of fights that ended in a kill inside the limit.
    pub killed_pct: f64,
}

/// `fights` fights to `hp` damage: an optional opening round, then `blow` every round with the
/// energy carried, each fight starting fresh. The target does not regenerate. `None` when nothing
/// here can hurt it.
pub fn simulate_fights(
    opener: Option<&Opener>,
    blow: Option<&Blow>,
    hp: u32,
    fights: u32,
    max_rounds: u32,
    rng: &mut Rng,
    buf: &mut Vec<u32>,
) -> Option<KillStats> {
    let hurts = blow.is_some_and(Blow::can_hurt)
        || opener.is_some_and(|o| o.stab.can_hurt() || o.follow.is_some_and(|(b, _)| b.can_hurt()));
    if !hurts || hp == 0 || fights == 0 || max_rounds == 0 {
        return None;
    }
    buf.clear();
    let mut killed = 0u32;
    for _ in 0..fights {
        let mut dealt = 0u32;
        let mut rounds = 0u32;
        if let Some(o) = opener {
            dealt = o.roll(rng);
            rounds = 1;
        }
        if let Some(b) = blow {
            let mut energy = Energy::new(b.energy);
            while dealt < hp && rounds < max_rounds {
                for _ in 0..energy.swings() {
                    dealt = dealt.saturating_add(b.roll(rng));
                }
                rounds += 1;
            }
        }
        if dealt >= hp {
            killed += 1;
        } else {
            rounds = max_rounds;
        }
        buf.push(rounds);
    }
    let s = stats(buf);
    Some(KillStats {
        mean: s.mean,
        median: s.median,
        p90: s.p90,
        killed_pct: f64::from(killed) * 100.0 / f64::from(fights),
    })
}
