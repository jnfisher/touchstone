//! Monster attack simulator (FAM-12): port of clsMonsterAttackSim. Card P3-13.
//!
//! The simulator rolls with VB6's `Rnd`, so a run is reproduced roll for roll when the caller
//! hands in a generator in the same state. `Randomize` (no argument, called at the top of
//! `RunSim`) is deliberately not ported: the caller owns the generator and seeds it.
//!
//! The GreaterMUD branches (`bGreaterMUD = True`) are not ported: `run` returns
//! [`RulesError::Unsupported`] for them.

use crate::RulesError;
use touchstone_vb6::{Currency, Rnd, Vb6Error};

type Res<T> = Result<T, Vb6Error>;

const SEP: &str = "==================================================================";

/// The class's settable properties. Integer properties are `i16`, Long properties `i32`, as in
/// the class; the defaults are what `Class_Initialize` leaves (not `ResetValues`).
#[derive(Debug, Clone, PartialEq)]
pub struct MonsterSimInput {
    pub atk_name: [String; 5],
    pub atk_type: [i16; 5],
    pub atk_spell_type: [i16; 5],
    pub atk_energy: [i16; 5],
    pub atk_min: [i16; 5],
    pub atk_max: [i16; 5],
    pub atk_chance: [i16; 5],
    pub atk_success: [i16; 5],
    pub atk_hit_spell_min: [i16; 5],
    pub atk_hit_spell_max: [i16; 5],
    pub atk_hit_spell_type: [i16; 5],
    pub atk_hit_spell_name: [String; 5],
    pub atk_resist: [i16; 5],
    pub atk_mr_dmg_resist: [i16; 5],
    pub atk_duration: [i16; 5],
    pub between_round_name: [String; 5],
    pub between_round_min: [i16; 5],
    pub between_round_max: [i16; 5],
    pub between_round_spell_type: [i16; 5],
    pub between_round_chance: [i16; 5],
    pub between_round_resist_type: [i16; 5],
    pub between_round_resist_dmg_mr: [i16; 5],
    pub between_round_duration: [i16; 5],
    pub user_ac: i32,
    pub user_dr: i32,
    pub user_dodge: i32,
    pub user_mr: i32,
    pub user_anti_magic: i16,
    pub dodge_before_ac: bool,
    pub user_prot_evil: i32,
    pub mob_is_evil: bool,
    pub user_vile_ward: i32,
    pub user_rfir: i32,
    pub user_rcol: i32,
    pub user_rlit: i32,
    pub user_rwat: i32,
    pub user_rsto: i32,
    pub energy_per_round: i16,
    pub number_of_rounds: i32,
    pub greater_mud: bool,
    pub dynamic_calc: bool,
    /// Currency, scaled by 10 000 (`0.0001` is `Currency::from_scaled(1)`).
    pub dynamic_calc_difference: Currency,
    pub combat_log_max_rounds: i32,
    pub combat_log_max_round_only: bool,
    pub hide_energy_info: bool,
    pub hit_min: i16,
    pub hit_cap: i16,
    pub spell_hit_cap: i16,
    pub dodge_softcap: i16,
    pub dodge_cap: i16,
}

impl Default for MonsterSimInput {
    fn default() -> Self {
        Self {
            atk_name: Default::default(),
            atk_type: [0; 5],
            atk_spell_type: [0; 5],
            atk_energy: [0; 5],
            atk_min: [0; 5],
            atk_max: [0; 5],
            atk_chance: [0; 5],
            atk_success: [0; 5],
            atk_hit_spell_min: [0; 5],
            atk_hit_spell_max: [0; 5],
            atk_hit_spell_type: [0; 5],
            atk_hit_spell_name: Default::default(),
            atk_resist: [0; 5],
            atk_mr_dmg_resist: [0; 5],
            atk_duration: [0; 5],
            between_round_name: Default::default(),
            between_round_min: [0; 5],
            between_round_max: [0; 5],
            between_round_spell_type: [0; 5],
            between_round_chance: [0; 5],
            between_round_resist_type: [0; 5],
            between_round_resist_dmg_mr: [0; 5],
            between_round_duration: [0; 5],
            user_ac: 0,
            user_dr: 0,
            user_dodge: 0,
            user_mr: 0,
            user_anti_magic: 0,
            dodge_before_ac: false,
            user_prot_evil: 0,
            mob_is_evil: true,
            user_vile_ward: 0,
            user_rfir: 0,
            user_rcol: 0,
            user_rlit: 0,
            user_rwat: 0,
            user_rsto: 0,
            energy_per_round: 0,
            number_of_rounds: 0,
            greater_mud: false,
            dynamic_calc: false,
            dynamic_calc_difference: Currency::from_scaled(1),
            combat_log_max_rounds: 10,
            combat_log_max_round_only: false,
            hide_energy_info: false,
            hit_min: 8,
            hit_cap: 99,
            spell_hit_cap: 98,
            dodge_softcap: 0,
            dodge_cap: 95,
        }
    }
}

/// The simulator object: inputs plus the results `run` leaves behind. Results persist between
/// runs exactly as the class's fields do (a second `run` starts from the first run's stats).
#[derive(Debug, Clone, PartialEq)]
pub struct MonsterSim {
    pub input: MonsterSimInput,
    pub stat_atk_attempted: [Currency; 5],
    pub stat_atk_hits: [Currency; 5],
    pub stat_atk_dmg_resisted: [Currency; 5],
    pub stat_atk_attempt_dodged_or_resisted: [Currency; 5],
    pub stat_atk_total_damage: [Currency; 5],
    pub stat_hit_spell_atk_dmg_resisted: [Currency; 5],
    pub stat_hit_spell_atk_total_damage: [Currency; 5],
    pub stat_between_round_atk_dmg_resisted: [Currency; 5],
    pub stat_between_round_atk_total_damage: [Currency; 5],
    pub total_attacks: i64,
    pub total_damage: Currency,
    pub total_damage_phys: Currency,
    pub total_damage_spell: Currency,
    pub max_round_damage: i64,
    pub max_energy_per_round: i64,
    pub average_damage: Currency,
    pub average_damage_phys: Currency,
    pub average_damage_spell: Currency,
    /// `sCombatLog`, with CR LF line breaks.
    pub combat_log: String,
    combat_log_round_count: i64,
    // Active duration spells: attack/hit-spell ones and between-round ones.
    atk_ticks: [i16; 5],
    atk_left: [Currency; 5],
    atk_value: [i16; 5],
    atk_value_orig: [i16; 5],
    btw_ticks: [i16; 5],
    btw_left: [Currency; 5],
    btw_value: [i16; 5],
    btw_value_orig: [i16; 5],
}

fn c0() -> Currency {
    Currency::from_scaled(0)
}

fn ci(n: i64) -> Currency {
    Currency::from_scaled(n * 10_000)
}
fn cadd(a: Currency, b: Currency) -> Currency {
    Currency::from_scaled(a.scaled() + b.scaled())
}
fn csub(a: Currency, b: Currency) -> Currency {
    Currency::from_scaled(a.scaled() - b.scaled())
}

/// Half-to-even division of scaled integers (`d > 0`).
fn div_half_even(n: i64, d: i64) -> i64 {
    let q = n.div_euclid(d);
    let twice = 2 * n.rem_euclid(d);
    if twice > d || (twice == d && q % 2 != 0) {
        q + 1
    } else {
        q
    }
}

/// `Round(x, digits)` on a Currency: exact decimal, half to even, stays Currency.
fn round_cur(c: Currency, digits: u32) -> Currency {
    if digits >= 4 {
        return c;
    }
    let q = 10i64.pow(4 - digits);
    Currency::from_scaled(div_half_even(c.scaled(), q) * q)
}

/// Assigning a Currency to a Long (or `CLng`): banker's rounding.
fn cur_to_long(c: Currency) -> Res<i32> {
    i32::try_from(div_half_even(c.scaled(), 10_000)).map_err(|_| Vb6Error::Overflow)
}

/// Assigning a Currency to an Integer.
fn cur_to_i16(c: Currency) -> Res<i16> {
    i16::try_from(div_half_even(c.scaled(), 10_000)).map_err(|_| Vb6Error::Overflow)
}

fn to_i16(v: i64) -> Res<i16> {
    i16::try_from(v).map_err(|_| Vb6Error::Overflow)
}

fn to_i32(v: i64) -> Res<i32> {
    i32::try_from(v).map_err(|_| Vb6Error::Overflow)
}

impl MonsterSim {
    #[must_use]
    pub fn new(input: MonsterSimInput) -> Self {
        Self {
            input,
            stat_atk_attempted: [c0(); 5],
            stat_atk_hits: [c0(); 5],
            stat_atk_dmg_resisted: [c0(); 5],
            stat_atk_attempt_dodged_or_resisted: [c0(); 5],
            stat_atk_total_damage: [c0(); 5],
            stat_hit_spell_atk_dmg_resisted: [c0(); 5],
            stat_hit_spell_atk_total_damage: [c0(); 5],
            stat_between_round_atk_dmg_resisted: [c0(); 5],
            stat_between_round_atk_total_damage: [c0(); 5],
            total_attacks: 0,
            total_damage: c0(),
            total_damage_phys: c0(),
            total_damage_spell: c0(),
            max_round_damage: 0,
            max_energy_per_round: 0,
            average_damage: c0(),
            average_damage_phys: c0(),
            average_damage_spell: c0(),
            combat_log: String::new(),
            combat_log_round_count: 0,
            atk_ticks: [0; 5],
            atk_left: [c0(); 5],
            atk_value: [0; 5],
            atk_value_orig: [0; 5],
            btw_ticks: [0; 5],
            btw_left: [c0(); 5],
            btw_value: [0; 5],
            btw_value_orig: [0; 5],
        }
    }

    /// Port of clsMonsterAttackSim `IsSpellResisted`. Rolls only when the attack can be
    /// resisted. VB6 reports a run-time error through its handler and returns False; the only
    /// one possible here is none, so this is infallible.
    pub fn is_spell_resisted(rnd: &mut Rnd, resist_type: i16, mr: i16, anti_magic: i16) -> bool {
        if (resist_type == 1 && anti_magic == 1) || resist_type == 2 {
            let roll = rnd.random_number(1, 100);
            let mr = mr.min(196);
            // nMR / 2 is a Double compare.
            return f64::from(roll) <= f64::from(mr) / 2.0;
        }
        false
    }

    /// Port of clsMonsterAttackSim `CalcResistedDamage`. An error inside the procedure
    /// (Integer overflow on extreme MR) leaves the function value at 0, as MME's handler does.
    #[must_use]
    pub fn resisted_damage(damage: Currency, mr: i16, anti_magic: i16) -> Currency {
        Self::resisted_damage_inner(damage, mr, anti_magic).unwrap_or(c0())
    }

    fn resisted_damage_inner(damage: Currency, mr: i16, anti_magic: i16) -> Res<Currency> {
        let mr = mr.min(150);
        let pct = if anti_magic == 0 {
            if mr < 50 {
                let d = touchstone_vb6::sub_i16(mr, 50)?;
                Currency::from_f64(touchstone_vb6::round(f64::from(d) / 100.0, 2)?)?
            } else {
                let d = touchstone_vb6::sub_i16(mr, 50)?;
                Currency::from_f64(touchstone_vb6::round(f64::from(d) / 200.0, 2)?)?
            }
        } else {
            Currency::from_f64(touchstone_vb6::round(f64::from(mr) / 200.0, 2)?)?
        };
        damage.checked_sub(damage.checked_mul(pct)?)
    }

    /// Port of clsMonsterAttackSim `GetMaxDamage`. On a VB6 error MME's handler returns the
    /// value accumulated so far, and so does this.
    #[must_use]
    pub fn max_damage(&self) -> i64 {
        let mut acc = 0i64;
        let _ = self.max_damage_inner(&mut acc);
        acc
    }

    fn nothing_to_sim(&self) -> bool {
        let i = &self.input;
        (0..5).all(|x| {
            i.atk_min[x] <= 0
                && i.atk_max[x] <= 0
                && i.atk_hit_spell_min[x] <= 0
                && i.atk_hit_spell_max[x] <= 0
                && i.between_round_min[x] <= 0
                && i.between_round_max[x] <= 0
        })
    }

    fn max_damage_inner(&self, acc: &mut i64) -> Res<()> {
        let i = &self.input;
        if self.nothing_to_sim() {
            return Ok(());
        }
        let epr = i64::from(i.energy_per_round);
        let mut lowest_cost = 0i64;
        let mut max_cost = 0i64;
        let mut zero_energy = false;
        for a in 0..5 {
            let (ty, en) = (i.atk_type[a], i64::from(i.atk_energy[a]));
            if ty > 0 && en > 0 {
                let mut low = en;
                if ty == 2
                    && (((i.atk_resist[a] == 1 && i.user_anti_magic == 1) || i.atk_resist[a] == 2)
                        || i.atk_success[a] < 100)
                {
                    low = i64::from(touchstone_vb6::clng(f64::from(i.atk_energy[a]) / 2.0)?);
                }
                if (lowest_cost == 0 || low < lowest_cost) && low > 0 {
                    lowest_cost = low;
                }
                if max_cost == 0 || en > max_cost {
                    max_cost = en;
                }
            } else if ty > 0 && en == 0 && i.atk_chance[a] > 0 {
                lowest_cost = 0;
                zero_energy = true;
                break;
            }
        }
        let max_energy_round = if zero_energy {
            // Integer * Integer, so a large energy-per-round overflows here.
            i64::from(touchstone_vb6::mul_i16(i.energy_per_round, 2)?)
        } else {
            if lowest_cost == 0 {
                return Ok(());
            }
            let mut least_used = lowest_cost;
            while least_used + max_cost <= epr {
                least_used += lowest_cost;
            }
            let mut remaining = epr - least_used;
            if remaining < lowest_cost {
                remaining += lowest_cost - 1;
            }
            epr + remaining
        };

        let mut remaining = max_energy_round;
        let mut x = 1i64;
        while remaining >= lowest_cost && x <= 6 {
            let mut max_dmg = 0i64;
            let mut max_dpe = c0();
            let mut energy_for_attack = 0i64;
            for a in 0..5 {
                let en = i64::from(i.atk_energy[a]);
                if en <= remaining && en > 0 {
                    let ty = i.atk_type[a];
                    if (ty != 2 || i.atk_success[a] > 0) && ty > 0 {
                        let mut dmg = i64::from(i.atk_max[a]);
                        if ty == 1 && i.atk_duration[a] <= 1 {
                            dmg += i64::from(i.atk_hit_spell_max[a]);
                        }
                        let mut attempts = i64::from(to_i16(touchstone_vb6::fix(
                            remaining as f64 / en as f64,
                        ) as i64)?);
                        if attempts > 6 - x + 1 {
                            attempts = 6 - x + 1;
                        }
                        let dmg_l = to_i32(dmg * attempts)?;
                        let dpe = Currency::from_f64(f64::from(dmg_l) / remaining as f64)?;
                        if dpe > max_dpe {
                            max_dmg = dmg;
                            max_dpe = dpe;
                            energy_for_attack = en;
                        }
                    }
                }
            }
            *acc += max_dmg;
            remaining -= energy_for_attack;
            x += 1;
        }
        for a in 0..5 {
            let (ty, dur) = (i.atk_type[a], i.atk_duration[a]);
            if ty == 1 && dur > 1 {
                *acc += i64::from(i.atk_hit_spell_max[a]);
                if dur > 2 {
                    *acc += i64::from(i.atk_hit_spell_max[a]);
                }
            } else if ty == 2 && dur > 1 {
                *acc += i64::from(i.atk_max[a]);
                if dur > 2 {
                    *acc += i64::from(i.atk_max[a]);
                }
            }
        }
        let mut best = 0i64;
        for x in 0..5 {
            if i.between_round_chance[x] > 0 && i.between_round_duration[x] <= 1 {
                best = best.max(i64::from(i.between_round_max[x]));
                // nMaxDamage is only raised, never lowered, by a larger value.
            }
        }
        *acc += best;
        for x in 0..5 {
            if i.between_round_chance[x] > 0 && i.between_round_duration[x] > 1 {
                *acc += i64::from(i.between_round_max[x]);
                if i.between_round_duration[x] > 2 {
                    *acc += i64::from(i.between_round_max[x]);
                }
            }
        }
        Ok(())
    }

    /// Port of `AddToCombatLog`.
    fn add_log(&mut self, left: &str, right: &str) {
        if self.combat_log_round_count >= i64::from(self.input.combat_log_max_rounds) {
            return;
        }
        let mut right = right;
        if self.input.hide_energy_info && right.to_ascii_lowercase().contains("energy") {
            right = "";
        }
        let mut left = left.to_string();
        if !right.is_empty() && left.chars().count() > 29 {
            left = left.chars().take(26).collect::<String>() + "...";
        }
        self.combat_log.push_str("\r\n");
        self.combat_log.push_str(&left);
        if !right.is_empty() {
            let pad = 30usize.saturating_sub(left.chars().count());
            self.combat_log.push_str(&" ".repeat(pad));
            self.combat_log.push_str(right);
        }
    }

    fn reset_atk_spell(&mut self, a: usize) {
        self.atk_left[a] = c0();
        self.atk_ticks[a] = 0;
        self.atk_value[a] = 0;
    }

    fn reset_btw_spell(&mut self, a: usize) {
        self.btw_left[a] = c0();
        self.btw_ticks[a] = 0;
        self.btw_value[a] = 0;
    }

    /// The per-element resistance reduction shared by the three spell paths. `reduction` is a
    /// Sub-level variable in MME that is never cleared between attacks, so a stale value from an
    /// earlier attack is applied again when the current element's resistance is not positive.
    fn element_reduction(&self, kind: i16, dmg: Currency, reduction: &mut Currency) -> Res<()> {
        if kind == 4 || kind == 6 {
            return Ok(());
        }
        let i = &self.input;
        let res = match kind {
            0 => i.user_rcol,
            1 => i.user_rfir,
            2 => i.user_rsto,
            3 => i.user_rlit,
            5 => i.user_rwat,
            _ => return Ok(()),
        };
        if res > 0 {
            *reduction = Currency::from_f64(dmg.to_f64() * (f64::from(res) / 100.0))?;
        }
        Ok(())
    }

    fn mr_i16(&self) -> Res<i16> {
        to_i16(i64::from(self.input.user_mr))
    }

    /// Port of clsMonsterAttackSim `RunSim`. `rnd` supplies every roll; the `Randomize` call at
    /// the top of the VB6 procedure is not reproduced.
    ///
    /// # Errors
    /// [`RulesError::Unsupported`] when `greater_mud` is set; [`RulesError::Vb6`] when VB6 would
    /// have raised a run-time error (MME's handler abandons the run, and so does this, leaving
    /// the partial results in place).
    pub fn run(&mut self, rnd: &mut Rnd) -> Result<(), RulesError> {
        if self.input.greater_mud {
            return Err(RulesError::Unsupported("GreaterMUD monster simulation"));
        }
        self.run_inner(rnd)
            .map_err(|e| RulesError::Vb6(e.code() as i32))
    }

    #[allow(clippy::too_many_lines, unused_assignments)]
    fn run_inner(&mut self, rnd: &mut Rnd) -> Res<()> {
        if self.input.dynamic_calc {
            self.input.number_of_rounds = 100_000;
        }
        if self.input.number_of_rounds < 1 || self.input.energy_per_round < 1 {
            return Ok(());
        }
        self.total_attacks = 0;
        self.total_damage = c0();
        self.total_damage_phys = c0();
        self.total_damage_spell = c0();
        self.max_round_damage = 0;
        self.max_energy_per_round = 0;

        // MME writes each clamp as one-line `If x < 0 Then x = 0: If x > 100 Then x = 100`, so
        // the upper clamp lives inside the first If's Then-branch and never fires.
        for v in [
            &mut self.input.hit_min,
            &mut self.input.hit_cap,
            &mut self.input.spell_hit_cap,
            &mut self.input.dodge_softcap,
            &mut self.input.dodge_cap,
        ] {
            if *v < 0 {
                *v = 0;
            }
        }

        let mut user_defense = i64::from(self.input.user_ac);
        if self.input.mob_is_evil {
            user_defense += i64::from(self.input.user_prot_evil);
        }
        let user_defense = to_i32(user_defense)?;

        self.atk_ticks = [0; 5];
        self.atk_left = [c0(); 5];
        self.atk_value = [0; 5];
        self.atk_value_orig = [0; 5];
        self.btw_ticks = [0; 5];
        self.btw_left = [c0(); 5];
        self.btw_value = [0; 5];
        self.btw_value_orig = [0; 5];

        let mut n_round: i64 = 0;
        let mut max_round_log = String::new();

        if !self.nothing_to_sim() {
            self.combat_log = SEP.to_string();
            self.combat_log_round_count = 0;
            if self.input.combat_log_max_rounds < 0 {
                self.input.combat_log_max_rounds = 0;
            }
            let epr = i64::from(self.input.energy_per_round);
            let mut remaining: i64 = 0;
            // Sub-level locals in MME that survive across attacks and rounds.
            let mut resist_red = c0();
            let mut current_avg = c0();
            let mut dynamic_round_count: i64 = 0;
            let rounds = i64::from(self.input.number_of_rounds);
            n_round = 1;
            'rounds: while n_round <= rounds {
                let mut round_damage: i64 = 0;
                let mut sep_shown = false;

                // Extra spell-round ticks (a spell round is 3s, a combat round 5s).
                for x in 0..5 {
                    if self.atk_left[x] > c0() && self.atk_ticks[x] == 3 {
                        let v = ci(i64::from(self.atk_value[x]));
                        let orig = ci(i64::from(self.atk_value_orig[x]));
                        round_damage += i64::from(self.atk_value[x]);
                        self.total_damage = cadd(self.total_damage, v);
                        self.total_damage_spell = cadd(self.total_damage_spell, v);
                        self.stat_atk_total_damage[x] = cadd(self.stat_atk_total_damage[x], v);
                        self.stat_atk_dmg_resisted[x] =
                            cadd(self.stat_atk_dmg_resisted[x], csub(orig, v));
                        self.stat_hit_spell_atk_dmg_resisted[x] =
                            cadd(self.stat_hit_spell_atk_dmg_resisted[x], csub(orig, v));
                        self.stat_hit_spell_atk_total_damage[x] =
                            cadd(self.stat_hit_spell_atk_total_damage[x], v);
                        if !sep_shown {
                            self.add_log("", "");
                            sep_shown = true;
                        }
                        self.atk_left[x] = csub(self.atk_left[x], ci(1));
                        let left = self.atk_left[x].scaled() / 10_000;
                        if self.input.atk_type[x] == 2 {
                            self.add_log(
                                &format!(
                                    "[{}, spell tick] for {} -- {} rounds rem.",
                                    self.input.atk_name[x], self.atk_value[x], left
                                ),
                                "",
                            );
                        } else {
                            let name = self.hit_spell_name(x, x + 1);
                            self.add_log(
                                &format!(
                                    "[{}, hit spell tick] for {} -- {} rounds rem.",
                                    name, self.atk_value[x], left
                                ),
                                "",
                            );
                        }
                        if self.atk_left[x] < ci(1) {
                            self.reset_atk_spell(x);
                        }
                    }
                    if self.btw_left[x] > c0() && self.btw_ticks[x] == 3 {
                        let v = ci(i64::from(self.btw_value[x]));
                        let orig = ci(i64::from(self.btw_value_orig[x]));
                        round_damage += i64::from(self.btw_value[x]);
                        self.total_damage = cadd(self.total_damage, v);
                        self.total_damage_spell = cadd(self.total_damage_spell, v);
                        self.stat_between_round_atk_dmg_resisted[x] =
                            cadd(self.stat_between_round_atk_dmg_resisted[x], csub(orig, v));
                        self.stat_between_round_atk_total_damage[x] =
                            cadd(self.stat_between_round_atk_total_damage[x], v);
                        if !sep_shown {
                            self.add_log("", "");
                            sep_shown = true;
                        }
                        self.btw_left[x] = csub(self.btw_left[x], ci(1));
                        let left = self.btw_left[x].scaled() / 10_000;
                        self.add_log(
                            &format!(
                                "[{}, between spell tick] for {} -- {} rounds rem.",
                                self.input.between_round_name[x], self.btw_value[x], left
                            ),
                            "",
                        );
                        if self.btw_left[x] < ci(1) {
                            self.reset_btw_spell(x);
                        }
                    }
                }
                if sep_shown {
                    self.add_log("", "");
                }

                if self.input.hide_energy_info {
                    self.add_log(&format!("ROUND {n_round}"), "");
                } else {
                    self.add_log(
                        &format!(
                            "ROUND {n_round} / Energy: {remaining} + {epr} = {}",
                            remaining + epr
                        ),
                        "",
                    );
                }
                self.add_log("", "");

                remaining += epr;
                if remaining > self.max_energy_per_round {
                    self.max_energy_per_round = remaining;
                }

                // Regular attack attempts.
                'attempts: for _attempt in 1..=6 {
                    let roll_attack = i64::from(rnd.random_number(1, 100));
                    let mut last_type = 0i16;
                    let mut last_energy = 0i16;
                    for a in 0..5 {
                        let mut damage = c0();
                        let mut hit_dmg = c0();
                        let mut show_hit_spell = false;
                        let mut dur_applied = false;
                        if !(roll_attack <= i64::from(self.input.atk_chance[a])
                            && self.input.atk_type[a] > 0)
                        {
                            continue;
                        }
                        let ty = self.input.atk_type[a];
                        let energy = i64::from(self.input.atk_energy[a]);
                        last_energy = self.input.atk_energy[a];
                        last_type = ty;

                        if remaining >= energy {
                            self.stat_atk_attempted[a] = cadd(self.stat_atk_attempted[a], ci(1));
                            self.total_attacks += 1;
                            let mut hit = false;
                            let mut dodged = false;
                            let mut glanced = false;
                            let mut resisted = false;

                            // Chance to hit / cast.
                            let mut adj = ci(i64::from(self.input.atk_success[a]));
                            if ty != 2 {
                                if user_defense > 0 {
                                    if adj != c0() {
                                        let def = to_i32(
                                            i64::from(user_defense) * i64::from(user_defense),
                                        )?;
                                        let num = f64::from(def) / 100.0;
                                        let den = adj.checked_mul(adj)?.checked_div(ci(140))?;
                                        let q = touchstone_vb6::div(num, den.to_f64())?;
                                        let r = touchstone_vb6::round(1.0 - q, 2)?;
                                        adj = Currency::from_f64(r * 100.0)?;
                                    } else {
                                        adj = c0();
                                    }
                                } else {
                                    adj = ci(i64::from(self.input.hit_cap));
                                }
                                if adj < ci(i64::from(self.input.hit_min)) {
                                    adj = ci(i64::from(self.input.hit_min));
                                }
                                if adj > ci(i64::from(self.input.hit_cap)) {
                                    adj = ci(i64::from(self.input.hit_cap));
                                }
                            }

                            if self.input.dodge_before_ac && self.input.user_dodge > 0 && ty != 2 {
                                let roll = i64::from(rnd.random_number(1, 100));
                                if roll <= i64::from(self.input.user_dodge) {
                                    dodged = true;
                                    self.stat_atk_attempt_dodged_or_resisted[a] =
                                        cadd(self.stat_atk_attempt_dodged_or_resisted[a], ci(1));
                                }
                            }

                            if !self.input.dodge_before_ac || !dodged {
                                let roll = i64::from(rnd.random_number(1, 100));
                                if ci(roll) <= adj {
                                    hit = true;
                                    if ty != 2
                                        && self.input.user_dodge > 0
                                        && !self.input.dodge_before_ac
                                        && self.input.atk_success[a] >= 8
                                    {
                                        let prod = to_i32(i64::from(self.input.user_dodge) * 10)?;
                                        let q = touchstone_vb6::div(
                                            f64::from(prod),
                                            f64::from(self.input.atk_success[a]) / 8.0,
                                        )?;
                                        let mut adj_dodge = i64::from(touchstone_vb6::clng(
                                            touchstone_vb6::fix(q),
                                        )?);
                                        if adj_dodge > i64::from(self.input.dodge_cap) {
                                            adj_dodge = i64::from(self.input.dodge_cap);
                                        }
                                        let roll = i64::from(rnd.random_number(1, 100));
                                        if roll <= adj_dodge {
                                            hit = false;
                                            dodged = true;
                                            self.stat_atk_attempt_dodged_or_resisted[a] = cadd(
                                                self.stat_atk_attempt_dodged_or_resisted[a],
                                                ci(1),
                                            );
                                        }
                                    }
                                }
                            }

                            if hit {
                                damage = ci(i64::from(
                                    rnd.random_number(self.input.atk_min[a], self.input.atk_max[a]),
                                ));
                                let original = damage;
                                if ty == 2 {
                                    self.atk_value_orig[a] = cur_to_i16(damage)?;
                                    if self.input.atk_mr_dmg_resist[a] == 1 && damage > c0() {
                                        let red = round_cur(
                                            Self::resisted_damage(
                                                damage,
                                                self.mr_i16()?,
                                                self.input.user_anti_magic,
                                            ),
                                            0,
                                        );
                                        damage = red;
                                    }
                                    if self.input.atk_spell_type[a] != 4
                                        && self.input.atk_spell_type[a] != 6
                                    {
                                        self.element_reduction(
                                            self.input.atk_spell_type[a],
                                            damage,
                                            &mut resist_red,
                                        )?;
                                        if resist_red != c0() {
                                            damage = round_cur(csub(damage, resist_red), 0);
                                        }
                                    }
                                    if self.input.atk_duration[a] > 0 {
                                        if self.atk_left[a] < ci(1)
                                            || ci(i64::from(self.atk_value[a])) != damage
                                        {
                                            if Self::is_spell_resisted(
                                                rnd,
                                                self.input.atk_resist[a],
                                                self.mr_i16()?,
                                                self.input.user_anti_magic,
                                            ) {
                                                self.stat_atk_attempt_dodged_or_resisted[a] = cadd(
                                                    self.stat_atk_attempt_dodged_or_resisted[a],
                                                    ci(1),
                                                );
                                                resisted = true;
                                                hit = false;
                                                damage = c0();
                                            } else {
                                                self.reset_atk_spell(a);
                                                self.atk_left[a] =
                                                    ci(i64::from(self.input.atk_duration[a]));
                                                self.atk_value[a] = cur_to_i16(damage)?;
                                                dur_applied = true;
                                                damage = c0();
                                            }
                                        } else {
                                            // Duration continues: not really an attempt.
                                            self.stat_atk_attempted[a] =
                                                csub(self.stat_atk_attempted[a], ci(1));
                                            self.total_attacks -= 1;
                                            continue;
                                        }
                                    } else if Self::is_spell_resisted(
                                        rnd,
                                        self.input.atk_resist[a],
                                        self.mr_i16()?,
                                        self.input.user_anti_magic,
                                    ) {
                                        self.stat_atk_attempt_dodged_or_resisted[a] = cadd(
                                            self.stat_atk_attempt_dodged_or_resisted[a],
                                            ci(1),
                                        );
                                        resisted = true;
                                        hit = false;
                                        damage = c0();
                                    }
                                } else {
                                    let mut dr = ci(i64::from(self.input.user_dr));
                                    if dr > damage {
                                        dr = damage;
                                    }
                                    self.stat_atk_dmg_resisted[a] =
                                        cadd(self.stat_atk_dmg_resisted[a], dr);
                                    damage = csub(damage, dr);

                                    if self.input.atk_hit_spell_min[a] > 0
                                        || self.input.atk_hit_spell_max[a] > 0
                                        || self.input.atk_duration[a] > 0
                                    {
                                        hit_dmg = ci(i64::from(rnd.random_number(
                                            self.input.atk_hit_spell_min[a],
                                            self.input.atk_hit_spell_max[a],
                                        )));
                                        self.atk_value_orig[a] = cur_to_i16(hit_dmg)?;
                                        let dur = self.input.atk_duration[a];
                                        if self.input.atk_mr_dmg_resist[a] == 1 && hit_dmg > c0() {
                                            let red = round_cur(
                                                Self::resisted_damage(
                                                    hit_dmg,
                                                    self.mr_i16()?,
                                                    self.input.user_anti_magic,
                                                ),
                                                0,
                                            );
                                            if dur == 0 {
                                                self.stat_atk_dmg_resisted[a] = cadd(
                                                    self.stat_atk_dmg_resisted[a],
                                                    csub(hit_dmg, red),
                                                );
                                            }
                                            hit_dmg = red;
                                        }
                                        if self.input.atk_hit_spell_type[a] != 4
                                            && self.input.atk_hit_spell_type[a] != 6
                                        {
                                            self.element_reduction(
                                                self.input.atk_hit_spell_type[a],
                                                hit_dmg,
                                                &mut resist_red,
                                            )?;
                                            if resist_red != c0() {
                                                if dur == 0 {
                                                    self.stat_atk_dmg_resisted[a] = round_cur(
                                                        cadd(
                                                            self.stat_atk_dmg_resisted[a],
                                                            resist_red,
                                                        ),
                                                        0,
                                                    );
                                                }
                                                hit_dmg = round_cur(csub(hit_dmg, resist_red), 0);
                                            }
                                        }
                                        if dur > 0 {
                                            if self.atk_left[a] < ci(1)
                                                || ci(i64::from(self.atk_value[a])) != hit_dmg
                                            {
                                                if Self::is_spell_resisted(
                                                    rnd,
                                                    self.input.atk_resist[a],
                                                    self.mr_i16()?,
                                                    self.input.user_anti_magic,
                                                ) {
                                                    self.stat_atk_dmg_resisted[a] = cadd(
                                                        self.stat_atk_dmg_resisted[a],
                                                        hit_dmg,
                                                    );
                                                    show_hit_spell = true;
                                                    resisted = true;
                                                    hit_dmg = c0();
                                                } else {
                                                    self.reset_atk_spell(a);
                                                    self.atk_left[a] = ci(i64::from(dur));
                                                    self.atk_value[a] = cur_to_i16(hit_dmg)?;
                                                    dur_applied = true;
                                                    show_hit_spell = true;
                                                    hit_dmg = c0();
                                                }
                                            }
                                        } else {
                                            if Self::is_spell_resisted(
                                                rnd,
                                                self.input.atk_resist[a],
                                                self.mr_i16()?,
                                                self.input.user_anti_magic,
                                            ) {
                                                self.stat_atk_dmg_resisted[a] =
                                                    cadd(self.stat_atk_dmg_resisted[a], hit_dmg);
                                                resisted = true;
                                                hit_dmg = c0();
                                            }
                                            show_hit_spell = true;
                                        }
                                        if !show_hit_spell {
                                            hit_dmg = c0();
                                        }
                                    }
                                }

                                if damage <= c0() {
                                    if ty != 2 {
                                        glanced = true;
                                    }
                                    damage = c0();
                                }

                                if hit {
                                    if original != damage
                                        && ty == 2
                                        && self.input.atk_duration[a] < 1
                                    {
                                        self.stat_atk_dmg_resisted[a] = cadd(
                                            self.stat_atk_dmg_resisted[a],
                                            csub(original, damage),
                                        );
                                    }
                                    let sum = cadd(damage, hit_dmg);
                                    round_damage =
                                        i64::from(cur_to_long(cadd(ci(round_damage), sum))?);
                                    self.total_damage = cadd(self.total_damage, sum);
                                    self.total_damage_spell =
                                        cadd(self.total_damage_spell, hit_dmg);
                                    if ty == 2 {
                                        self.total_damage_spell =
                                            cadd(self.total_damage_spell, damage);
                                    } else {
                                        self.total_damage_phys =
                                            cadd(self.total_damage_phys, damage);
                                    }
                                    remaining -= energy;
                                    self.stat_atk_hits[a] = cadd(self.stat_atk_hits[a], ci(1));
                                    self.stat_atk_total_damage[a] =
                                        cadd(self.stat_atk_total_damage[a], sum);

                                    let right =
                                        format!("Energy used: {energy} ... Remaining: {remaining}");
                                    if ty == 2 && self.input.atk_duration[a] > 0 {
                                        self.add_log(
                                            &format!(
                                                "{} applied ({})",
                                                self.input.atk_name[a], self.atk_value_orig[a]
                                            ),
                                            &right,
                                        );
                                    } else {
                                        self.add_log(
                                            &format!(
                                                "{} for {}{}",
                                                self.input.atk_name[a],
                                                round_cur(damage, 0).scaled() / 10_000,
                                                if glanced { " (GLANCE)" } else { "" }
                                            ),
                                            &right,
                                        );
                                    }
                                    if show_hit_spell {
                                        if dur_applied {
                                            self.add_log(
                                                &format!(
                                                    " - duration spell applied ({})",
                                                    self.atk_value[a]
                                                ),
                                                "",
                                            );
                                        } else {
                                            let name = self.hit_spell_name(a, 6);
                                            let what = if resisted {
                                                "(resist)".to_string()
                                            } else {
                                                format!(
                                                    "for {}",
                                                    round_cur(hit_dmg, 0).scaled() / 10_000
                                                )
                                            };
                                            self.add_log(&format!(" -{name} {what}"), "");
                                        }
                                    }
                                }
                            }

                            if !hit {
                                if ty == 2 {
                                    if self.input.atk_duration[a] == 0 || resisted {
                                        let used = i64::from(touchstone_vb6::cint(
                                            f64::from(self.input.atk_energy[a]) / 2.0,
                                        )?);
                                        remaining -= used;
                                        self.add_log(
                                            &format!(
                                                "{} ({})",
                                                self.input.atk_name[a],
                                                if resisted { "RESIST" } else { "FAIL" }
                                            ),
                                            &format!(
                                                "Energy used: {used} ... Remaining: {remaining}"
                                            ),
                                        );
                                    }
                                } else {
                                    remaining -= energy;
                                    self.add_log(
                                        &format!(
                                            "{} ({})",
                                            self.input.atk_name[a],
                                            if dodged { "DODGE" } else { "MISS" }
                                        ),
                                        &format!(
                                            "Energy used: {energy} ... Remaining: {remaining}"
                                        ),
                                    );
                                }
                            }
                        }
                        break;
                    }
                    if last_type == 1 && remaining < i64::from(last_energy) {
                        break 'attempts;
                    }
                }

                // Between-round spell cast.
                sep_shown = false;
                let roll_attack = i64::from(rnd.random_number(1, 100));
                for x in 0..5 {
                    let mut hit = false;
                    let mut resisted = false;
                    let mut dur_applied = false;
                    let mut damage = c0();
                    if self.input.between_round_chance[x] > 0
                        && roll_attack <= i64::from(self.input.between_round_chance[x])
                    {
                        damage = ci(i64::from(rnd.random_number(
                            self.input.between_round_min[x],
                            self.input.between_round_max[x],
                        )));
                        self.btw_value_orig[x] = cur_to_i16(damage)?;
                        if self.input.between_round_resist_dmg_mr[x] == 1 && damage > c0() {
                            damage = round_cur(
                                Self::resisted_damage(
                                    damage,
                                    self.mr_i16()?,
                                    self.input.user_anti_magic,
                                ),
                                0,
                            );
                        }
                        if self.input.between_round_spell_type[x] != 4
                            && self.input.between_round_spell_type[x] != 6
                        {
                            self.element_reduction(
                                self.input.between_round_spell_type[x],
                                damage,
                                &mut resist_red,
                            )?;
                            if resist_red != c0() {
                                damage = round_cur(csub(damage, resist_red), 0);
                            }
                        }
                        if self.input.between_round_duration[x] > 0 {
                            if self.btw_left[x] < ci(1)
                                || ci(i64::from(self.btw_value[x])) != damage
                            {
                                if Self::is_spell_resisted(
                                    rnd,
                                    self.input.between_round_resist_type[x],
                                    self.mr_i16()?,
                                    self.input.user_anti_magic,
                                ) {
                                    resisted = true;
                                    damage = c0();
                                } else {
                                    self.reset_btw_spell(x);
                                    self.btw_left[x] =
                                        ci(i64::from(self.input.between_round_duration[x]));
                                    self.btw_value[x] = cur_to_i16(damage)?;
                                    dur_applied = true;
                                    damage = c0();
                                }
                            }
                        } else if Self::is_spell_resisted(
                            rnd,
                            self.input.between_round_resist_type[x],
                            self.mr_i16()?,
                            self.input.user_anti_magic,
                        ) {
                            resisted = true;
                            damage = c0();
                        } else {
                            hit = true;
                        }
                    }
                    if !sep_shown && (dur_applied || resisted || hit) {
                        self.add_log("", "");
                        sep_shown = true;
                    }
                    let name = self.input.between_round_name[x].clone();
                    if dur_applied {
                        self.add_log(
                            &format!(
                                "[between round] {name} - duration spell applied ({})",
                                self.btw_value[x]
                            ),
                            "",
                        );
                        break;
                    } else if resisted {
                        self.add_log(&format!("[between round] {name} (RESIST)"), "");
                        break;
                    } else if hit {
                        self.add_log(
                            &format!(
                                "[between round] {name} for {}",
                                round_cur(damage, 0).scaled() / 10_000
                            ),
                            "",
                        );
                        round_damage = i64::from(cur_to_long(cadd(ci(round_damage), damage))?);
                        self.total_damage = cadd(self.total_damage, damage);
                        self.total_damage_spell = cadd(self.total_damage_spell, damage);
                        self.stat_between_round_atk_total_damage[x] =
                            cadd(self.stat_between_round_atk_total_damage[x], damage);
                        break;
                    }
                }

                // All spell duration ticks.
                sep_shown = false;
                for x in 0..5 {
                    if self.atk_left[x] > c0() {
                        let v = ci(i64::from(self.atk_value[x]));
                        let orig = ci(i64::from(self.atk_value_orig[x]));
                        round_damage += i64::from(self.atk_value[x]);
                        self.total_damage = cadd(self.total_damage, v);
                        self.total_damage_spell = cadd(self.total_damage_spell, v);
                        self.stat_atk_total_damage[x] = cadd(self.stat_atk_total_damage[x], v);
                        self.stat_atk_dmg_resisted[x] =
                            cadd(self.stat_atk_dmg_resisted[x], csub(orig, v));
                        self.stat_hit_spell_atk_dmg_resisted[x] =
                            cadd(self.stat_hit_spell_atk_dmg_resisted[x], csub(orig, v));
                        self.stat_hit_spell_atk_total_damage[x] =
                            cadd(self.stat_hit_spell_atk_total_damage[x], v);
                        if !sep_shown {
                            self.add_log("", "");
                            sep_shown = true;
                        }
                        self.atk_left[x] = csub(self.atk_left[x], ci(1));
                        let left = self.atk_left[x].scaled() / 10_000;
                        if self.input.atk_type[x] == 2 {
                            self.add_log(
                                &format!(
                                    "[{}, attack spell tick] for {} -- {} rounds rem.",
                                    self.input.atk_name[x], self.atk_value[x], left
                                ),
                                "",
                            );
                        } else {
                            let name = self.hit_spell_name(x, x + 1);
                            self.add_log(
                                &format!(
                                    "[{}, hit spell tick] for {} -- {} rounds rem.",
                                    name, self.atk_value[x], left
                                ),
                                "",
                            );
                        }
                        self.atk_ticks[x] = if self.atk_ticks[x] >= 3 {
                            1
                        } else {
                            self.atk_ticks[x] + 1
                        };
                        if self.atk_left[x] < ci(1) {
                            self.reset_atk_spell(x);
                        }
                    }
                    if self.btw_left[x] > c0() {
                        let v = ci(i64::from(self.btw_value[x]));
                        let orig = ci(i64::from(self.btw_value_orig[x]));
                        round_damage += i64::from(self.btw_value[x]);
                        self.total_damage = cadd(self.total_damage, v);
                        self.total_damage_spell = cadd(self.total_damage_spell, v);
                        self.stat_between_round_atk_dmg_resisted[x] =
                            cadd(self.stat_between_round_atk_dmg_resisted[x], csub(orig, v));
                        self.stat_between_round_atk_total_damage[x] =
                            cadd(self.stat_between_round_atk_total_damage[x], v);
                        if !sep_shown {
                            self.add_log("", "");
                            sep_shown = true;
                        }
                        self.btw_left[x] = csub(self.btw_left[x], ci(1));
                        let left = self.btw_left[x].scaled() / 10_000;
                        self.add_log(
                            &format!(
                                "[{}, between spell tick] for {} -- {} rounds rem.",
                                self.input.between_round_name[x], self.btw_value[x], left
                            ),
                            "",
                        );
                        self.btw_ticks[x] = if self.btw_ticks[x] >= 3 {
                            1
                        } else {
                            self.btw_ticks[x] + 1
                        };
                        if self.btw_left[x] < ci(1) {
                            self.reset_btw_spell(x);
                        }
                    }
                }

                self.add_log("", "");
                self.add_log(
                    &format!("Damage for round: {round_damage}"),
                    &format!("Energy Remaining: {remaining}"),
                );
                self.add_log(SEP, "");
                self.combat_log_round_count += 1;

                if round_damage > self.max_round_damage {
                    self.max_round_damage = round_damage;
                    if self.input.combat_log_max_round_only {
                        max_round_log.clone_from(&self.combat_log);
                    }
                } else if self.input.combat_log_max_round_only {
                    self.combat_log.clear();
                    self.combat_log_round_count = 0;
                }

                if dynamic_round_count > 1000 && self.input.dynamic_calc {
                    let avg = round_cur(self.total_damage.checked_div(ci(n_round))?, 3);
                    if current_avg == c0() {
                        current_avg = avg;
                        if current_avg < ci(1) {
                            self.input.number_of_rounds = to_i32(n_round)?;
                            break 'rounds;
                        }
                        dynamic_round_count = 0;
                    } else {
                        let ratio = avg.checked_div(current_avg)?;
                        let diff = csub(ci(1), ratio);
                        let diff = Currency::from_scaled(diff.scaled().abs());
                        if diff < self.input.dynamic_calc_difference {
                            self.input.number_of_rounds = to_i32(n_round)?;
                            break 'rounds;
                        }
                        current_avg = avg;
                        dynamic_round_count = 0;
                    }
                }
                dynamic_round_count += 1;
                n_round += 1;
            }
        }

        // `end_sim`: after a completed For loop the counter sits one past its limit.
        if n_round > 0 {
            let nr = ci(n_round);
            self.average_damage = round_cur(self.total_damage.checked_div(nr)?, 1);
            self.average_damage_phys = round_cur(self.total_damage_phys.checked_div(nr)?, 1);
            self.average_damage_spell = round_cur(self.total_damage_spell.checked_div(nr)?, 1);
        }
        if self.input.combat_log_max_round_only {
            self.combat_log = max_round_log;
        }
        Ok(())
    }

    /// `IIf(Len(m_sAtkHitSpellName(a)) = 0, "attack " & n, name)`.
    fn hit_spell_name(&self, a: usize, n: usize) -> String {
        if self.input.atk_hit_spell_name[a].is_empty() {
            format!("attack {n}")
        } else {
            self.input.atk_hit_spell_name[a].clone()
        }
    }
}
