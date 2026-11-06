//! Monster sim glue, monster side (FAM-13). Card P3-14a.
//!
//! Loads a monster's attacks, spells and item bonuses into the attack simulator's inputs
//! ([`MonsterSimInput`]) and answers the questions MME asks of a monster with the default
//! defender: average and maximum damage, and the attack summary. The character side
//! (`SetupMonsterAttackSimWithCharStats`, `CalculateMonsterDamageVsChar`) is a later card.
//!
//! MME keeps one global simulator, and `ResetValues` leaves some of its fields alone (attack and
//! between-round durations, hit-spell names, the combat-log switches). The functions here take
//! that persistent state as `&mut MonsterSimInput`, so a caller that reuses one input across
//! monsters sees what MME shows; a fresh `MonsterSimInput::default()` gives a first run.
//!
//! Where MME's `On Error` handler resumes at `out:`, the port stops where the error hit and keeps
//! what had been assigned, as MME does; no error escapes from the fill. GreaterMUD and Paramud
//! branches are not ported (the `Profile` only has `Stock`).

use touchstone_model::{Monster, Spell, World};
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Rnd, Vb6Error};

use crate::abilities::{NO_ABILITY, item_has_ability, spell_has_ability};
use crate::accuracy::{get_dodge_cap, get_hit_cap, get_hit_min, get_spell_hit_cap};
use crate::monster_sim::{MonsterSim, MonsterSimInput};
use crate::spell_cast::spell_attack_type_enum;
use crate::spells::{
    get_spell_attack_type, get_spell_duration, get_spell_max_damage, get_spell_min_damage,
    spell_does_damage,
};
use crate::{Profile, RulesError};

type Res<T> = Result<T, Vb6Error>;

/// MME's `MAJ_THRESH_PCT`: the share of melee weight one accuracy needs to count as the majority.
const MAJ_THRESH_PCT: i64 = 51;

/// The globals these procedures read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlueEnv {
    pub profile: Profile,
    /// `nNMRVer`: the data version (1.71 added monster energy, 1.8 attack names and spell resists).
    pub nmr_ver: f64,
}

/// `MonAttackSimReturn`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonAttackSimReturn {
    pub average_damage: Currency,
    pub max_damage: Currency,
}

impl Default for MonAttackSimReturn {
    fn default() -> Self {
        Self {
            average_damage: Currency::from_scaled(0),
            max_damage: Currency::from_scaled(0),
        }
    }
}

/// `MonsterAttackSummary`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MonsterAttackSummary {
    pub acc_majority: i32,
    pub acc_max: i32,
    pub atk_poison: bool,
    pub atk_fear: bool,
    pub atk_confusion: bool,
    pub spell_attack_types: String,
    pub spell_extra_types: String,
}

fn i16c(v: i32) -> Res<i16> {
    i16::try_from(v).map_err(|_| Vb6Error::Overflow)
}

/// VB6 `Trim`: spaces only.
fn trim(s: &str) -> String {
    s.trim_matches(' ').to_string()
}

/// `SpellHasAbility`: the ability's value, -1 for none (or no such spell).
fn spell_has(world: &World, number: i32, ability: i16) -> i32 {
    let slots = world.get_spell(number).map(|s| &s.abilities[..]);
    spell_has_ability(Profile::Stock, slots, number, ability)
}

/// The spell's `Name`, as `GetSpellName(n, True)` answers: "None" for 0, the number for no row.
fn spell_name_plain(world: &World, n: i32) -> String {
    if n == 0 {
        return "None".into();
    }
    match world.get_spell(n) {
        None => n.to_string(),
        Some(s) => s.name.clone().unwrap_or_default(),
    }
}

/// Port of clsMonsterAttackSim `ResetValues`, for the input fields it touches. The durations,
/// hit-spell names and the combat-log switches are left as they are, as in MME. The run's
/// statistics live in [`MonsterSim`], which a fresh run starts with zeroed.
pub fn reset_values(sim: &mut MonsterSimInput) {
    sim.hit_min = 8;
    sim.hit_cap = 99;
    sim.spell_hit_cap = 98;
    sim.dodge_cap = 95;
    sim.dodge_softcap = 0;
    sim.user_ac = 0;
    sim.user_dr = 0;
    sim.user_dodge = 0;
    sim.user_mr = 0;
    sim.user_anti_magic = 0;
    sim.dodge_before_ac = false;
    sim.user_prot_evil = 0;
    sim.mob_is_evil = true;
    sim.user_vile_ward = 0;
    sim.user_rfir = 0;
    sim.user_rcol = 0;
    sim.user_rlit = 0;
    sim.user_rwat = 0;
    sim.user_rsto = 0;
    sim.energy_per_round = 0;
    sim.number_of_rounds = 0;
    for x in 0..5 {
        sim.atk_name[x].clear();
        sim.atk_type[x] = 0;
        sim.atk_energy[x] = 0;
        sim.atk_spell_type[x] = 4;
        sim.atk_min[x] = 0;
        sim.atk_max[x] = 0;
        sim.atk_chance[x] = 0;
        sim.atk_success[x] = 0;
        sim.atk_hit_spell_min[x] = 0;
        sim.atk_hit_spell_max[x] = 0;
        sim.atk_hit_spell_type[x] = 4;
        sim.atk_resist[x] = 0;
        sim.atk_mr_dmg_resist[x] = 0;
        sim.between_round_name[x].clear();
        sim.between_round_min[x] = 0;
        sim.between_round_max[x] = 0;
        sim.between_round_spell_type[x] = 4;
        sim.between_round_chance[x] = 0;
        sim.between_round_resist_type[x] = 0;
        sim.between_round_resist_dmg_mr[x] = 0;
    }
}

/// Port of modMMudDatabase `CalculateMonsterItemBonuses` (an Integer): the sum of `abilities`
/// over the monster's weapon and, weighted by drop percent, its drops, for items with Limit 0.
/// An overflow ends the sum early, as MME's handler does. A drop chance above 100 makes MME
/// assign to a read-only recordset field (a DAO error); the handler then ends the sum there.
#[must_use]
pub fn calculate_monster_item_bonuses(world: &World, m: &Monster, abilities: &[i16]) -> i16 {
    fn inner(world: &World, m: &Monster, abilities: &[i16], acc: &mut i16) -> Res<()> {
        let has = |item: i32, ab: i16| {
            let it = world.get_item(item).filter(|i| i.limit == 0)?;
            let t = item_has_ability(Profile::Stock, Some(&it.abilities[..]), item, ab);
            (t != NO_ABILITY).then_some(t)
        };
        if m.weapon > 0 {
            for &ab in abilities {
                if let Some(t) = has(m.weapon, ab) {
                    *acc = vb::add_i16(*acc, i16c(t)?)?;
                }
            }
        }
        for d in m.drops.iter().filter(|d| d.item > 0) {
            for &ab in abilities {
                if let Some(t) = has(d.item, ab) {
                    if d.pct > 100 {
                        return Err(Vb6Error::Overflow); // stands for DAO error 3020
                    }
                    let add = f64::from(i16c(t)?) * (f64::from(d.pct) / 100.0);
                    *acc = vb::cint(f64::from(*acc) + add)?;
                }
            }
        }
        Ok(())
    }
    let mut acc = 0;
    let _ = inner(world, m, abilities, &mut acc);
    acc
}

/// The damage abilities in the order MME tests them (1 damage, 17 damage vs MR, 8 drain) with
/// the "resisted by MR" flag each one sets.
const DAMAGE_ABILITIES: [(i16, i16); 3] = [(1, 0), (17, 1), (8, 0)];

/// Port of modMMudDatabase `PopulateMonsterDataToAttackSim` (stock engine): fills `sim` from
/// monster `number`. A missing monster leaves `sim` unchanged.
pub fn populate_monster_data_to_attack_sim(
    world: &World,
    env: &GlueEnv,
    number: i32,
    sim: &mut MonsterSimInput,
) {
    let Some(m) = world.get_monster(number) else {
        return;
    };
    let _ = populate_inner(world, env, m, sim);
}

/// Min and max of a spell used as an attack: the ability's own value when it has one, else the
/// spell's damage at `level` for a monster.
fn spell_damage(
    spells: &[Spell],
    number: i32,
    value: i32,
    level: i16,
    min: &mut i16,
    max: &mut i16,
) -> Res<()> {
    if value > 0 {
        *min = i16c(value)?;
        *max = *min;
    } else {
        *min = i16c(get_spell_min_damage(
            spells, number, level, &mut -1, true, false,
        ))?;
        *max = i16c(get_spell_max_damage(
            spells, number, level, &mut -1, true, false,
        ))?;
    }
    Ok(())
}

fn populate_inner(world: &World, env: &GlueEnv, m: &Monster, sim: &mut MonsterSimInput) -> Res<()> {
    let nmr = env.nmr_ver;
    let spells = &world.spells;
    sim.energy_per_round = if nmr >= 1.71 { i16c(m.energy)? } else { 1000 };

    // 4 is the MaxDamage ability; 22, 105 and 106 are the accuracy abilities.
    let dmg_bonus = calculate_monster_item_bonuses(world, m, &[4]);
    let acc_bonus = calculate_monster_item_bonuses(world, m, &[22, 105, 106]);

    sim.mob_is_evil = matches!(m.align, 1 | 2 | 5 | 6);

    for (x, a) in m.attacks.iter().enumerate() {
        if !(a.att_type > 0 && a.att_type < 4) {
            continue;
        }
        let mut name = String::new();
        if nmr >= 1.8 {
            name = a.name.clone();
        } else {
            if a.att_type == 2 && a.acc > 0 {
                name = spell_name_plain(world, a.acc);
            }
            if name.is_empty() || name == "None" {
                name = format!("Attack {}", x + 1);
            }
        }
        sim.atk_name[x] = trim(&name);
        sim.atk_type[x] = i16c(a.att_type)?;
        sim.atk_energy[x] = i16c(a.energy)?;
        sim.atk_chance[x] = i16c(a.pct)?;

        if a.att_type == 2 {
            let Some(sp) = world.get_spell(a.acc) else {
                continue;
            };
            let lvl = i16c(a.max)?;
            // An area spell in a plain attack slot that uses ability 1 is never cast by the game.
            if sp.targets == 12
                && get_spell_duration(spells, a.acc, lvl, true) == 0
                && spell_has(world, a.acc, 1) > -1
            {
                sim.atk_duration[x] = 0;
                sim.atk_min[x] = 0;
                sim.atk_max[x] = 0;
                sim.atk_energy[x] = 0;
                continue;
            }
            if nmr >= 1.8 {
                sim.atk_resist[x] = i16c(sp.type_of_resists)?;
            }
            sim.atk_spell_type[x] = i16c(sp.att_type)?;
            sim.atk_duration[x] = i16c(get_spell_duration(spells, a.acc, lvl, true))?;
            sim.atk_min[x] = 0;
            sim.atk_max[x] = 0;
            for (ab, mr) in DAMAGE_ABILITIES {
                let t = spell_has(world, a.acc, ab);
                if t >= 0 {
                    sim.atk_mr_dmg_resist[x] = mr;
                    let (mut lo, mut hi) = (sim.atk_min[x], sim.atk_max[x]);
                    let r = spell_damage(spells, a.acc, t, lvl, &mut lo, &mut hi);
                    sim.atk_min[x] = lo;
                    sim.atk_max[x] = hi;
                    r?;
                }
            }
            sim.atk_success[x] = i16c(a.min)?;
        } else {
            sim.atk_min[x] = vb::add_i16(i16c(a.min)?, dmg_bonus)?;
            sim.atk_max[x] = vb::add_i16(i16c(a.max)?, dmg_bonus)?;
            sim.atk_success[x] = vb::add_i16(i16c(a.acc)?, acc_bonus)?;
            if a.hit_spell > 0 {
                let Some(sp) = world.get_spell(a.hit_spell) else {
                    continue;
                };
                if nmr >= 1.8 {
                    sim.atk_resist[x] = i16c(sp.type_of_resists)?;
                }
                sim.atk_hit_spell_name[x] = sp.name.clone().unwrap_or_default();
                sim.atk_hit_spell_type[x] = i16c(sp.att_type)?;
                sim.atk_duration[x] = i16c(get_spell_duration(spells, a.hit_spell, 0, false))?;
                // Ability 1 wins over 17; the damage is the spell's own at level 0.
                let h1 = spell_has(world, a.hit_spell, 1) >= 0;
                let h17 = spell_has(world, a.hit_spell, 17) >= 0;
                if h1 || h17 {
                    sim.atk_mr_dmg_resist[x] = i16::from(!h1);
                    sim.atk_hit_spell_min[x] = i16c(get_spell_min_damage(
                        spells,
                        a.hit_spell,
                        0,
                        &mut 0,
                        false,
                        false,
                    ))?;
                    sim.atk_hit_spell_max[x] = i16c(get_spell_max_damage(
                        spells,
                        a.hit_spell,
                        0,
                        &mut 0,
                        false,
                        false,
                    ))?;
                } else {
                    sim.atk_hit_spell_min[x] = 0;
                    sim.atk_hit_spell_max[x] = 0;
                }
            }
        }
    }

    for (x, ms) in m.mid_spells.iter().enumerate() {
        if ms.spell <= 0 {
            continue;
        }
        let Some(sp) = world.get_spell(ms.spell) else {
            continue;
        };
        sim.between_round_name[x] = sp.name.clone().unwrap_or_default();
        sim.between_round_spell_type[x] = i16c(sp.att_type)?;
        if nmr >= 1.8 {
            sim.between_round_resist_type[x] = i16c(sp.type_of_resists)?;
        }
        sim.between_round_chance[x] = i16c(ms.pct)?;
        let lvl = i16c(ms.lvl)?;
        sim.between_round_duration[x] = i16c(get_spell_duration(spells, ms.spell, lvl, true))?;
        for (ab, mr) in DAMAGE_ABILITIES {
            let t = spell_has(world, ms.spell, ab);
            if t >= 0 {
                sim.between_round_resist_dmg_mr[x] = mr;
                let (mut lo, mut hi) = (sim.between_round_min[x], sim.between_round_max[x]);
                let r = spell_damage(spells, ms.spell, t, lvl, &mut lo, &mut hi);
                sim.between_round_min[x] = lo;
                sim.between_round_max[x] = hi;
                r?;
            }
        }
    }

    // Attacks that share a name get their slot number appended.
    for x in 0..5 {
        if !sim.atk_name[x].is_empty() {
            for y in 0..5 {
                if y != x && sim.atk_name[x] == sim.atk_name[y] {
                    sim.atk_name[x] = format!("{}{}", trim(&sim.atk_name[x]), x + 1);
                    sim.atk_name[y] = format!("{}{}", trim(&sim.atk_name[y]), y + 1);
                }
            }
        }
    }
    Ok(())
}

/// Port of modMMudDatabase `CalculateMonsterAvgDmg`: average damage over `rounds` simulated
/// rounds (MME's default is 500) against MME's default defender (no armour, 50 MR), and the
/// maximum damage. `sim` is MME's one global simulator; `rnd` is the generator the run rolls
/// with (MME's `Randomize` is not ported, the caller seeds it). `rounds` of 0 skips the run.
pub fn calculate_monster_avg_dmg(
    world: &World,
    env: &GlueEnv,
    number: i32,
    rounds: i32,
    sim: &mut MonsterSimInput,
    rnd: &mut Rnd,
) -> Result<MonAttackSimReturn, RulesError> {
    reset_values(sim);
    sim.combat_log_max_rounds = 0;
    sim.number_of_rounds = rounds;
    sim.user_mr = 50;
    sim.greater_mud = false;
    sim.dynamic_calc = false;
    sim.dynamic_calc_difference = Currency::from_scaled(10);
    let p = env.profile;
    // The caps are small constants, so the narrowing is exact.
    sim.hit_min = get_hit_min(p, 0) as i16;
    sim.hit_cap = get_hit_cap(p) as i16;
    sim.spell_hit_cap = get_spell_hit_cap(p) as i16;
    sim.dodge_softcap = get_dodge_cap(p, 0, true) as i16;
    sim.dodge_cap = get_dodge_cap(p, 0, false) as i16;

    populate_monster_data_to_attack_sim(world, env, number, sim);

    let mut ret = MonAttackSimReturn::default();
    if sim.number_of_rounds > 0 {
        let mut run = MonsterSim::new(sim.clone());
        match run.run(rnd) {
            Ok(()) => ret.average_damage = run.average_damage,
            Err(RulesError::Vb6(_)) => return Ok(ret),
            Err(e) => return Err(e),
        }
    }
    let max = MonsterSim::new(sim.clone()).max_damage();
    ret.max_damage = Currency::from_scaled(max * 10_000);
    Ok(ret)
}

/// Port of modMMudDatabase `GetMonsterAttackSummary`: the accuracy the monster's melee attacks
/// hit with (majority and maximum), whether its spells can poison, scare or confuse, and the
/// attack types of its damaging spells. An error inside gives the empty summary, as in MME.
#[must_use]
pub fn get_monster_attack_summary(
    world: &World,
    env: &GlueEnv,
    number: i32,
    spell_attack_types: bool,
    spell_special_attacks: bool,
) -> MonsterAttackSummary {
    if world.monsters.is_empty() {
        return MonsterAttackSummary::default();
    }
    let Some(m) = world.get_monster(number) else {
        return MonsterAttackSummary::default();
    };
    summary_inner(world, env, m, spell_attack_types, spell_special_attacks).unwrap_or_default()
}

fn summary_inner(
    world: &World,
    env: &GlueEnv,
    m: &Monster,
    want_types: bool,
    want_special: bool,
) -> Res<MonsterAttackSummary> {
    let mut ret = MonsterAttackSummary::default();
    let mut extra = String::new();
    let mut types = String::new();
    // The fear, poison and confusion abilities.
    let special = |ret: &mut MonsterAttackSummary, spell: i32| {
        if !ret.atk_fear && spell_has(world, spell, 60) >= 0 {
            ret.atk_fear = true;
        }
        if !ret.atk_poison && spell_has(world, spell, 19) >= 0 {
            ret.atk_poison = true;
        }
        if !ret.atk_confusion && spell_has(world, spell, 71) >= 0 {
            ret.atk_confusion = true;
        }
    };
    let type_letter = |spell: i32| {
        let t = i16c(get_spell_attack_type(&world.spells, spell))?;
        Ok::<String, Vb6Error>(spell_attack_type_enum(t, true))
    };

    if want_special && m.death_spell > 0 {
        special(&mut ret, m.death_spell);
    }

    // nPercent is one Integer shared by both loops; the between-round loop subtracts the last
    // value from each chance, which is MME's arithmetic.
    let mut pct: i16 = 0;
    if want_types || want_special {
        for ms in &m.mid_spells {
            if ms.spell != 0 {
                pct = i16c(ms.pct - i32::from(pct))?;
                if pct > 0 {
                    if want_types && spell_does_damage(&world.spells, ms.spell, true) {
                        extra.push_str(&type_letter(ms.spell)?);
                    }
                    if want_special {
                        special(&mut ret, ms.spell);
                    }
                }
                if pct < 0 {
                    pct = 0;
                }
            }
        }
    }

    let mut melee_total: i64 = 0;
    let mut prev_cum: i32 = 0;
    let mut max_acc: i64 = 0;
    let mut uniq: Vec<(i64, i64)> = Vec::new();
    for a in &m.attacks {
        if !(a.att_type > 0 && a.att_type <= 3 && a.pct > 0) {
            continue;
        }
        if env.nmr_ver >= 1.8 {
            pct = i16c(vb::clng(vb::round(a.true_pct, 0)?)?)?;
        } else {
            let cum = a.pct;
            pct = i16c(cum - prev_cum)?;
            prev_cum = cum;
        }
        if pct < 0 {
            pct = 0;
        }
        match a.att_type {
            1 | 3 => {
                let acc = i64::from(a.acc);
                match uniq.iter_mut().find(|u| u.0 == acc) {
                    Some(u) => u.1 += i64::from(pct),
                    None => uniq.push((acc, i64::from(pct))),
                }
                melee_total += i64::from(pct);
                max_acc = max_acc.max(acc);
                if want_special && a.hit_spell > 0 {
                    special(&mut ret, a.hit_spell);
                }
            }
            2 => {
                if want_types {
                    types.push_str(&type_letter(a.acc)?);
                }
                if want_special {
                    if a.acc > 0 {
                        special(&mut ret, a.acc);
                    }
                    if a.hit_spell > 0 {
                        special(&mut ret, a.hit_spell);
                    }
                }
            }
            _ => {}
        }
        if want_types && a.hit_spell > 0 && spell_does_damage(&world.spells, a.hit_spell, true) {
            extra.push_str(&type_letter(a.hit_spell)?);
        }
    }

    if melee_total < 1 {
        melee_total = 100;
    }
    // The dominant accuracy: the largest share, ties to the higher accuracy.
    let (mut dom_pct, mut dom_acc, mut found) = (0i64, 0i64, false);
    for &(acc, p) in &uniq {
        if p > dom_pct || (p == dom_pct && acc > dom_acc) {
            dom_pct = p;
            dom_acc = acc;
            found = true;
        }
    }
    let majority = found && dom_pct * 100 >= MAJ_THRESH_PCT * melee_total;
    let (maj, max) = if majority {
        if (max_acc - dom_acc).abs() > 2 {
            (dom_acc, max_acc)
        } else {
            (dom_acc, dom_acc)
        }
    } else {
        (max_acc, max_acc)
    };
    ret.acc_majority = i32::try_from(maj).map_err(|_| Vb6Error::Overflow)?;
    ret.acc_max = i32::try_from(max).map_err(|_| Vb6Error::Overflow)?;
    if want_types {
        ret.spell_attack_types = types;
        ret.spell_extra_types = extra;
    }
    Ok(ret)
}
