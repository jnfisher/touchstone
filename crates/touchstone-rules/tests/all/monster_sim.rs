// Fixture tests for the monster_sim family: see docs/porting.md step 5.
use super::fixture::{cur, load};
use serde_json::Value;
use touchstone_rules::RulesError;
use touchstone_rules::monster_sim::{MonsterSim, MonsterSimInput};
use touchstone_vb6::{Currency, Rnd};

fn int(v: &Value) -> i64 {
    match v {
        Value::Object(o) => {
            let inner = &o["v"];
            if o["t"] == "cur" {
                // A Currency given as a decimal.
                (inner.as_f64().unwrap() * 10_000.0).round() as i64
            } else {
                inner.as_i64().unwrap()
            }
        }
        Value::Bool(b) => i64::from(*b),
        _ => v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")),
    }
}

fn arr<T: Copy + Default, F: Fn(&Value) -> T>(v: &Value, f: F) -> [T; 5] {
    let mut out = [T::default(); 5];
    for (o, x) in out.iter_mut().zip(v.as_array().unwrap()) {
        *o = f(x);
    }
    out
}

fn strs(v: &Value) -> [String; 5] {
    let mut out: [String; 5] = Default::default();
    for (o, x) in out.iter_mut().zip(v.as_array().unwrap()) {
        *o = x.as_str().unwrap().to_string();
    }
    out
}

fn build(props: &Value) -> MonsterSimInput {
    let mut i = MonsterSimInput::default();
    let Some(map) = props.as_object() else {
        return i;
    };
    for (k, v) in map {
        let a16 = || arr(v, |x| int(x) as i16);
        match k.as_str() {
            "sAtkName" => i.atk_name = strs(v),
            "nAtkType" => i.atk_type = a16(),
            "nAtkSpellType" => i.atk_spell_type = a16(),
            "nAtkEnergy" => i.atk_energy = a16(),
            "nAtkMin" => i.atk_min = a16(),
            "nAtkMax" => i.atk_max = a16(),
            "nAtkChance" => i.atk_chance = a16(),
            "nAtkSuccess" => i.atk_success = a16(),
            "nAtkHitSpellMin" => i.atk_hit_spell_min = a16(),
            "nAtkHitSpellMax" => i.atk_hit_spell_max = a16(),
            "nAtkHitSpellType" => i.atk_hit_spell_type = a16(),
            "sAtkHitSpellName" => i.atk_hit_spell_name = strs(v),
            "nAtkResist" => i.atk_resist = a16(),
            "nAtkMRdmgResist" => i.atk_mr_dmg_resist = a16(),
            "nAtkDuration" => i.atk_duration = a16(),
            "sBetweenRoundName" => i.between_round_name = strs(v),
            "nBetweenRoundMin" => i.between_round_min = a16(),
            "nBetweenRoundMax" => i.between_round_max = a16(),
            "nBetweenRoundSpellType" => i.between_round_spell_type = a16(),
            "nBetweenRoundChance" => i.between_round_chance = a16(),
            "nBetweenRoundResistType" => i.between_round_resist_type = a16(),
            "nBetweenRoundResistDmgMR" => i.between_round_resist_dmg_mr = a16(),
            "nBetweenRoundDuration" => i.between_round_duration = a16(),
            "nUserAC" => i.user_ac = int(v) as i32,
            "nUserDR" => i.user_dr = int(v) as i32,
            "nUserDodge" => i.user_dodge = int(v) as i32,
            "nUserMR" => i.user_mr = int(v) as i32,
            "nUserAntiMagic" => i.user_anti_magic = int(v) as i16,
            "bDodgeBeforeAC" => i.dodge_before_ac = int(v) != 0,
            "nUserProtEvil" => i.user_prot_evil = int(v) as i32,
            "bMobIsEvil" => i.mob_is_evil = int(v) != 0,
            "nUserVileWard" => i.user_vile_ward = int(v) as i32,
            "nUserRFIR" => i.user_rfir = int(v) as i32,
            "nUserRCOL" => i.user_rcol = int(v) as i32,
            "nUserRLIT" => i.user_rlit = int(v) as i32,
            "nUserRWAT" => i.user_rwat = int(v) as i32,
            "nUserRSTO" => i.user_rsto = int(v) as i32,
            "nEnergyPerRound" => i.energy_per_round = int(v) as i16,
            "nNumberOfRounds" => i.number_of_rounds = int(v) as i32,
            "bGreaterMUD" => i.greater_mud = int(v) != 0,
            "bDynamicCalc" => i.dynamic_calc = int(v) != 0,
            "bUseCPU" => {}
            "nDynamicCalcDifference" => {
                i.dynamic_calc_difference = Currency::from_scaled(int(v));
            }
            "nCombatLogMaxRounds" => i.combat_log_max_rounds = int(v) as i32,
            "bCombatLogMaxRoundOnly" => i.combat_log_max_round_only = int(v) != 0,
            "bHideEnergyInfo" => i.hide_energy_info = int(v) != 0,
            "HIT_MIN" => i.hit_min = int(v) as i16,
            "HIT_CAP" => i.hit_cap = int(v) as i16,
            "SPELL_HIT_CAP" => i.spell_hit_cap = int(v) as i16,
            "DODGE_SOFTCAP" => i.dodge_softcap = int(v) as i16,
            "DODGE_CAP" => i.dodge_cap = int(v) as i16,
            other => panic!("unmodelled property {other}"),
        }
    }
    i
}

fn cur_arr(v: &Value) -> Vec<i64> {
    v.as_array().unwrap().iter().map(cur).collect()
}

fn scaled(a: &[Currency; 5]) -> Vec<i64> {
    a.iter().map(|c| c.scaled()).collect()
}

/// Compares one output; returns a description of the first difference.
fn check(name: &str, got: &dyn std::fmt::Debug, want: &dyn std::fmt::Debug) -> Option<String> {
    let (g, w) = (format!("{got:?}"), format!("{want:?}"));
    (g != w).then(|| format!("{name}: got {g}, want {w}"))
}

fn run_case(case: &Value) -> Result<Option<String>, &'static str> {
    let inp = &case["inputs"];
    let out = &case["outputs"];
    let props = &inp["objects"]["sim"]["props"];
    let steps = inp["steps"].as_array().unwrap();
    let mut rnd = Rnd::from_state(inp["rnd"].as_u64().unwrap_or(327_680) as u32);
    let mut sim = MonsterSim::new(build(props));

    for step in steps {
        let call = step["call"].as_str().unwrap();
        let args: Vec<&Value> = step["args"]
            .as_array()
            .map(|a| a.iter().collect())
            .unwrap_or_default();
        let name = step["as"].as_str();
        match call {
            "sim.RunSim" => match sim.run(&mut rnd) {
                Err(RulesError::Unsupported(_)) => return Err("unsupported"),
                Err(e) => panic!("{}: {e}", case["id"]),
                Ok(()) => {}
            },
            "sim.GetMaxDamage" => {
                let got = sim.max_damage();
                if let Some(d) = check(name.unwrap(), &got, &out[name.unwrap()].as_i64().unwrap()) {
                    return Ok(Some(d));
                }
            }
            "sim.CalcResistedDamage" => {
                let got = MonsterSim::resisted_damage(
                    Currency::from_f64(args[0].as_f64().unwrap()).unwrap(),
                    int(args[1]) as i16,
                    int(args[2]) as i16,
                );
                if let Some(d) = check("result", &got.scaled(), &cur(&out["result"])) {
                    return Ok(Some(d));
                }
            }
            "sim.IsSpellResisted" => {
                let got = MonsterSim::is_spell_resisted(
                    &mut rnd,
                    int(args[0]) as i16,
                    int(args[1]) as i16,
                    int(args[2]) as i16,
                );
                if let Some(d) = check("result", &got, &out["result"].as_bool().unwrap()) {
                    return Ok(Some(d));
                }
            }
            // The GreaterMUD diminishing-returns helper is not ported.
            "sim.Apply_GMUD_DiminishingReturns" => return Err("unsupported"),
            other => panic!("unknown step {other}"),
        }
    }

    let i = &sim.input;
    let mut checks: Vec<Option<String>> = Vec::new();
    for (k, v) in out.as_object().unwrap() {
        let c = match k.as_str() {
            "max_damage" | "maxdmg" | "result" => continue, // checked in the step
            "rnd_after" => check(k, &i64::from(rnd.state()), &v.as_i64().unwrap()),
            "atk_total" => check(k, &scaled(&sim.stat_atk_total_damage), &cur_arr(v)),
            "attempted" => check(k, &scaled(&sim.stat_atk_attempted), &cur_arr(v)),
            "attempts" => check(k, &sim.total_attacks, &v.as_i64().unwrap()),
            "avg" => check(k, &sim.average_damage.scaled(), &cur(v)),
            "avg_phys" => check(k, &sim.average_damage_phys.scaled(), &cur(v)),
            "avg_spell" => check(k, &sim.average_damage_spell.scaled(), &cur(v)),
            "br_resisted" => check(
                k,
                &scaled(&sim.stat_between_round_atk_dmg_resisted),
                &cur_arr(v),
            ),
            "br_total" => check(
                k,
                &scaled(&sim.stat_between_round_atk_total_damage),
                &cur_arr(v),
            ),
            "dodged" => check(
                k,
                &scaled(&sim.stat_atk_attempt_dodged_or_resisted),
                &cur_arr(v),
            ),
            "hits" => check(k, &scaled(&sim.stat_atk_hits), &cur_arr(v)),
            "hs_resisted" => check(
                k,
                &scaled(&sim.stat_hit_spell_atk_dmg_resisted),
                &cur_arr(v),
            ),
            "hs_total" => check(
                k,
                &scaled(&sim.stat_hit_spell_atk_total_damage),
                &cur_arr(v),
            ),
            "log" => check(k, &sim.combat_log, &v.as_str().unwrap()),
            "max_energy" => check(k, &sim.max_energy_per_round, &v.as_i64().unwrap()),
            "maxround" => check(k, &sim.max_round_damage, &v.as_i64().unwrap()),
            "resisted" => check(k, &scaled(&sim.stat_atk_dmg_resisted), &cur_arr(v)),
            "rounds" => check(k, &i64::from(i.number_of_rounds), &v.as_i64().unwrap()),
            "total" => check(k, &sim.total_damage.scaled(), &cur(v)),
            "energy" => check(k, &i64::from(i.energy_per_round), &v.as_i64().unwrap()),
            "hit_min" => check(k, &i64::from(i.hit_min), &v.as_i64().unwrap()),
            "hit_cap" => check(k, &i64::from(i.hit_cap), &v.as_i64().unwrap()),
            "spell_cap" => check(k, &i64::from(i.spell_hit_cap), &v.as_i64().unwrap()),
            "dodge_cap" => check(k, &i64::from(i.dodge_cap), &v.as_i64().unwrap()),
            "dodge_soft" => check(k, &i64::from(i.dodge_softcap), &v.as_i64().unwrap()),
            "evil" => check(k, &i.mob_is_evil, &v.as_bool().unwrap()),
            "spell_type0" => check(k, &i64::from(i.atk_spell_type[0]), &v.as_i64().unwrap()),
            other => panic!("unmodelled output {other}"),
        };
        checks.push(c);
    }
    Ok(checks.into_iter().flatten().next())
}

// Expected values: fixtures/rules/monster_sim.json, produced by the oracle running MME's own
// clsMonsterAttackSim. GreaterMUD cases (bGreaterMUD = True, and the GreaterMUD-only
// diminishing-returns helper) assert that the port declines them.
#[test]
fn monster_sim_fixture() {
    let fx = load("monster_sim");
    let (mut passed, mut unsupported) = (0usize, 0usize);
    let mut failures = Vec::new();
    for case in &fx.cases {
        match run_case(case) {
            Ok(None) => passed += 1,
            Ok(Some(d)) => failures.push(format!("{}: {d}", case["id"])),
            Err(_) => unsupported += 1,
        }
    }
    eprintln!(
        "monster_sim: {passed} passed, {unsupported} unsupported, {} failed",
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "{} failures, first:\n{}",
        failures.len(),
        failures
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(passed + unsupported, fx.cases.len());
    assert!(passed >= 300);
}

#[test]
fn greater_mud_is_unsupported() {
    let mut sim = MonsterSim::new(MonsterSimInput {
        greater_mud: true,
        ..Default::default()
    });
    assert!(matches!(
        sim.run(&mut Rnd::new()),
        Err(RulesError::Unsupported(_))
    ));
}
