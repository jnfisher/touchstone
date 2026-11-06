// Fixture tests for the monster_glue family: see docs/porting.md step 5.
//
// Expected values: fixtures/rules/monster_glue.json, produced by the oracle running MME's own
// PopulateMonsterDataToAttackSim, CalculateMonsterAvgDmg and GetMonsterAttackSummary. The rows
// the World is built from are the "table" key of fixtures/scenarios/monster_glue.json (the
// oracle's input for the same cases), so they are stored once.
use super::fixture::{cur, load};
use serde_json::Value;
use std::path::PathBuf;
use touchstone_model::{AbilitySlot, Info, Item, Monster, MonsterAttack, Spell, World, WorldParts};
use touchstone_rules::Profile;
use touchstone_rules::monster_glue::{
    GlueEnv, MonsterAttackSummary, calculate_monster_avg_dmg, calculate_monster_item_bonuses,
    get_monster_attack_summary, populate_monster_data_to_attack_sim, reset_values,
};
use touchstone_rules::monster_sim::MonsterSimInput;
use touchstone_vb6::Rnd;

fn i(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}

fn pairs(flat: &Value) -> Vec<AbilitySlot> {
    flat.as_array()
        .unwrap()
        .chunks(2)
        .map(|p| AbilitySlot {
            ability: i(&p[0]),
            value: p.get(1).map_or(0, i),
        })
        .collect()
}

fn world() -> World {
    world_from("monster_glue")
}

/// The World built from a scenario's "table" key (monster_glue_char reuses the format).
pub(crate) fn world_from(family: &str) -> World {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios")
        .join(format!("{family}.json"));
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let t = &doc["table"];
    let n = |r: &Value, k: usize| r.get(k).map_or(0, i);
    let monsters = t["monsters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut attacks: [MonsterAttack; 5] = std::array::from_fn(|_| MonsterAttack {
                name: String::new(),
                att_type: 0,
                acc: 0,
                pct: 0,
                true_pct: 0.0,
                min: 0,
                max: 0,
                energy: 0,
                hit_spell: 0,
            });
            for (k, a) in r[4].as_array().unwrap().iter().enumerate() {
                attacks[k] = MonsterAttack {
                    name: a.get(0).and_then(Value::as_str).unwrap_or("").to_string(),
                    att_type: n(a, 1),
                    acc: n(a, 2),
                    pct: n(a, 3),
                    true_pct: a.get(4).and_then(Value::as_f64).unwrap_or(0.0),
                    min: n(a, 5),
                    max: n(a, 6),
                    energy: n(a, 7),
                    hit_spell: n(a, 8),
                };
            }
            let mut mid = [Default::default(); 5];
            for (k, a) in r[5].as_array().unwrap().iter().enumerate() {
                mid[k] = touchstone_model::MidSpell {
                    spell: n(a, 0),
                    pct: n(a, 1),
                    lvl: n(a, 2),
                };
            }
            let mut drops = [Default::default(); 10];
            for (k, d) in r[6].as_array().unwrap().iter().enumerate() {
                drops[k] = touchstone_model::DropSlot {
                    item: i(&d[0]),
                    pct: i(&d[1]),
                };
            }
            Monster {
                number: i(&r[0]),
                name: None,
                weapon: i(&r[3]),
                armour_class: 0,
                damage_resist: 0,
                follow_pct: 0,
                magic_res: 0,
                bs_defense: 0,
                exp: 0.0,
                exp_multi: 0.0,
                hp: 0,
                energy: i(&r[2]),
                avg_dmg: r.get(8).and_then(Value::as_f64).unwrap_or(0.0),
                avg_dmg_phys: 0.0,
                avg_dmg_spell: 0.0,
                greet_txt: 0,
                hp_regen: 0,
                charm_lvl: 0,
                r#type: 0,
                undead: 0,
                align: i(&r[1]),
                regen_time: 0,
                game_limit: 0,
                avg_lair_exp: 0.0,
                script_value: 0.0,
                r: 0,
                p: 0,
                g: 0,
                s: 0,
                c: 0,
                death_spell: i(&r[7]),
                create_spell: 0,
                attacks,
                mid_spells: mid,
                drops,
                abilities: [AbilitySlot::default(); 10],
                in_game: true,
                summoned_by: None,
            }
        })
        .collect();
    let spells = t["spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut abilities = [AbilitySlot::default(); 10];
            for (k, a) in pairs(&r[17]).into_iter().enumerate() {
                abilities[k] = a;
            }
            Spell {
                number: i(&r[0]),
                name: Some(r[1].as_str().unwrap().to_string()),
                short: None,
                req_level: i(&r[14]),
                energy_cost: i(&r[16]),
                mana_cost: 0,
                min_base: i(&r[8]),
                max_base: i(&r[9]),
                diff: 0,
                type_of_resists: i(&r[3]),
                targets: i(&r[4]),
                dur: i(&r[5]),
                att_type: i(&r[2]),
                magery: 0,
                magery_lvl: 0,
                cap: i(&r[15]),
                max_inc_lvls: i(&r[13]),
                max_inc: i(&r[12]),
                min_inc_lvls: i(&r[11]),
                min_inc: i(&r[10]),
                dur_inc_lvls: i(&r[7]),
                dur_inc: i(&r[6]),
                abilities,
                learnable: false,
                learned_from: None,
                casted_by: None,
                classes: None,
            }
        })
        .collect();
    let items = t["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut abilities = [AbilitySlot::default(); 20];
            for (k, a) in pairs(&r[2]).into_iter().enumerate() {
                abilities[k] = a;
            }
            Item {
                number: i(&r[0]),
                name: String::new(),
                limit: i(&r[1]),
                encum: 0,
                item_type: 0,
                use_count: 0,
                price: 0,
                currency: 0,
                min: 0,
                max: 0,
                armour_class: 0,
                damage_resist: 0,
                weapon_type: 0,
                armour_type: 0,
                worn: 0,
                accy: 0,
                gettable: true,
                str_req: 0,
                speed: 0,
                not_droppable: false,
                destroy_on_death: false,
                retain_after_uses: false,
                class_rest: [0; 10],
                race_rest: [0; 10],
                negate_spell: [0; 10],
                abilities,
                in_game: true,
                obtained_from: None,
                references: None,
            }
        })
        .collect();
    World::new(WorldParts {
        info: Info {
            nmr_version: String::new(),
            dat_file_version: String::new(),
            date: String::new(),
            time: String::new(),
            custom: String::new(),
            legit: false,
            disable_kai: false,
            update_url: String::new(),
        },
        items,
        monsters,
        spells,
        rooms: vec![],
        shops: vec![],
        classes: vec![],
        races: vec![],
        lairs: vec![],
        tbinfo: vec![],
    })
    .unwrap()
}

fn check(name: &str, got: &dyn std::fmt::Debug, want: &dyn std::fmt::Debug) -> Option<String> {
    let (g, w) = (format!("{got:?}"), format!("{want:?}"));
    (g != w).then(|| format!("{name}: got {g}, want {w}"))
}

fn ints(v: &Value) -> Vec<i64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap())
        .collect()
}

fn summary(got: &MonsterAttackSummary, want: &Value) -> Option<String> {
    check(
        "summary",
        &(
            got.acc_majority,
            got.acc_max,
            got.atk_poison,
            got.atk_fear,
            got.atk_confusion,
            got.spell_attack_types.as_str(),
            got.spell_extra_types.as_str(),
        ),
        &(
            i(&want["naccmajority"]),
            i(&want["naccmax"]),
            want["batkpoison"].as_bool().unwrap(),
            want["batkfear"].as_bool().unwrap(),
            want["batkconfusion"].as_bool().unwrap(),
            want["sspellattacktypes"].as_str().unwrap(),
            want["sspellextratypes"].as_str().unwrap(),
        ),
    )
}

fn arr<T: Copy + Into<i64>>(a: &[T; 5]) -> Vec<i64> {
    a.iter().map(|&x| x.into()).collect()
}

fn strs(a: &[String; 5]) -> Vec<&str> {
    a.iter().map(String::as_str).collect()
}

fn run_case(world: &World, setup_nmr: f64, case: &Value) -> Option<String> {
    let inp = &case["inputs"];
    let out = &case["outputs"];
    let nmr = inp["globals"]["nNMRVer"].as_f64().unwrap_or(setup_nmr);
    let env = GlueEnv {
        profile: Profile::Stock,
        nmr_ver: nmr,
    };
    let mut rnd = Rnd::from_state(inp["rnd"].as_u64().unwrap_or(327_680) as u32);
    let mut sim = MonsterSimInput::default();

    if inp["proc"] == "GetMonsterAttackSummary" {
        let a = inp["args"].as_array().unwrap();
        let got = get_monster_attack_summary(
            world,
            &env,
            i(&a[0]),
            a[1].as_bool().unwrap(),
            a[2].as_bool().unwrap(),
        );
        return summary(&got, &out["result"]);
    }
    for st in inp["steps"].as_array().unwrap() {
        let args: Vec<&Value> = st["args"].as_array().map_or(vec![], |a| a.iter().collect());
        let name = st["as"].as_str();
        match st["call"].as_str().unwrap() {
            "sim.ResetValues" => reset_values(&mut sim),
            "PopulateMonsterDataToAttackSim" => {
                populate_monster_data_to_attack_sim(world, &env, i(args[0]), &mut sim)
            }
            "CalculateMonsterAvgDmg" => {
                // an omitted nNumRounds is 500
                let rounds = args.get(1).map_or(500, |v| i(v));
                let r =
                    calculate_monster_avg_dmg(world, &env, i(args[0]), rounds, &mut sim, &mut rnd)
                        .unwrap();
                let w = &out[name.unwrap()];
                let d = check(
                    "avg",
                    &r.average_damage.scaled(),
                    &cur(&w["naveragedamage"]),
                )
                .or_else(|| check("max", &r.max_damage.scaled(), &cur(&w["nmaxdamage"])));
                if d.is_some() {
                    return d;
                }
            }
            "GetMonsterAttackSummary" => {
                let got = get_monster_attack_summary(
                    world,
                    &env,
                    i(args[0]),
                    args[1].as_bool().unwrap(),
                    args[2].as_bool().unwrap(),
                );
                if let Some(d) = summary(&got, &out[name.unwrap()]) {
                    return Some(d);
                }
            }
            other => panic!("unknown step {other}"),
        }
    }
    for (k, v) in out.as_object().unwrap() {
        let d = match k.as_str() {
            "r" | "r1" | "r2" | "s" => None,
            "sAtkName" => check(k, &strs(&sim.atk_name), &strs_v(v)),
            "sAtkHitSpellName" | "hsn" => check(k, &strs(&sim.atk_hit_spell_name), &strs_v(v)),
            "sBetweenRoundName" => check(k, &strs(&sim.between_round_name), &strs_v(v)),
            "nAtkType" => check(k, &arr(&sim.atk_type), &ints(v)),
            "nAtkSpellType" => check(k, &arr(&sim.atk_spell_type), &ints(v)),
            "nAtkEnergy" => check(k, &arr(&sim.atk_energy), &ints(v)),
            "nAtkMin" => check(k, &arr(&sim.atk_min), &ints(v)),
            "nAtkMax" => check(k, &arr(&sim.atk_max), &ints(v)),
            "nAtkChance" => check(k, &arr(&sim.atk_chance), &ints(v)),
            "nAtkSuccess" => check(k, &arr(&sim.atk_success), &ints(v)),
            "nAtkHitSpellMin" => check(k, &arr(&sim.atk_hit_spell_min), &ints(v)),
            "nAtkHitSpellMax" => check(k, &arr(&sim.atk_hit_spell_max), &ints(v)),
            "nAtkHitSpellType" => check(k, &arr(&sim.atk_hit_spell_type), &ints(v)),
            "nAtkResist" => check(k, &arr(&sim.atk_resist), &ints(v)),
            "nAtkMRdmgResist" => check(k, &arr(&sim.atk_mr_dmg_resist), &ints(v)),
            "nAtkDuration" | "dur" => check(k, &arr(&sim.atk_duration), &ints(v)),
            "nBetweenRoundMin" => check(k, &arr(&sim.between_round_min), &ints(v)),
            "nBetweenRoundMax" => check(k, &arr(&sim.between_round_max), &ints(v)),
            "nBetweenRoundSpellType" => check(k, &arr(&sim.between_round_spell_type), &ints(v)),
            "nBetweenRoundChance" => check(k, &arr(&sim.between_round_chance), &ints(v)),
            "nBetweenRoundResistType" => check(k, &arr(&sim.between_round_resist_type), &ints(v)),
            "nBetweenRoundResistDmgMR" => {
                check(k, &arr(&sim.between_round_resist_dmg_mr), &ints(v))
            }
            "nBetweenRoundDuration" | "brd" => {
                check(k, &arr(&sim.between_round_duration), &ints(v))
            }
            "bMobIsEvil" => check(k, &sim.mob_is_evil, &v.as_bool().unwrap()),
            "nEnergyPerRound" => check(k, &i64::from(sim.energy_per_round), &v.as_i64().unwrap()),
            "rnd" => check(k, &i64::from(rnd.state()), &v.as_i64().unwrap()),
            // the run's statistics belong to the run, not to the persistent input
            "phys" | "spell" | "attacks" => None,
            other => panic!("unmodelled output {other}"),
        };
        if d.is_some() {
            return d.map(|d| format!("{k}: {d}"));
        }
    }
    None
}

fn strs_v(v: &Value) -> Vec<&str> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect()
}

// Expected values: fixtures/rules/monster_glue.json (MME's own procedures, run by the oracle).
#[test]
fn monster_glue_fixture() {
    let fx = load("monster_glue");
    let world = world();
    let setup_nmr = fx.header["setup"]["globals"]["nNMRVer"].as_f64().unwrap();
    let failures: Vec<String> = fx
        .cases
        .iter()
        .filter_map(|c| run_case(&world, setup_nmr, c).map(|d| format!("{}: {d}", c["id"])))
        .collect();
    eprintln!(
        "monster_glue: {} passed, {} failed",
        fx.cases.len() - failures.len(),
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "{} failures, first:\n{}",
        failures.len(),
        failures
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(fx.cases.len() >= 1500);
}

// Hand-derived from PopulateMonsterDataToAttackSim and CalculateMonsterItemBonuses (the stock
// data has no monster whose attack names a missing spell or whose drop chance exceeds 100, so the
// fixture cannot reach those branches).
#[test]
fn missing_spell_and_overlong_drop_chance() {
    let mut w = world();
    let idx = w
        .monsters
        .iter()
        .position(|m| m.attacks[0].att_type == 1)
        .unwrap();
    // an item with ability 4 (+3 max damage), no limit, as a 150% drop
    w.items.push(Item {
        abilities: {
            let mut a = [AbilitySlot::default(); 20];
            a[0] = AbilitySlot {
                ability: 4,
                value: 3,
            };
            a
        },
        ..w.items[0].clone()
    });
    let mut parts = WorldParts {
        info: w.info.clone(),
        items: w.items.clone(),
        monsters: w.monsters.clone(),
        spells: w.spells.clone(),
        rooms: vec![],
        shops: vec![],
        classes: vec![],
        races: vec![],
        lairs: vec![],
        tbinfo: vec![],
    };
    let new_item = parts.items.len() as i32 - 1;
    parts.items[new_item as usize].number = 99_999;
    parts.items[new_item as usize].limit = 0;
    parts.monsters[idx].weapon = 0;
    parts.monsters[idx].drops = Default::default();
    parts.monsters[idx].drops[0] = touchstone_model::DropSlot {
        item: 99_999,
        pct: 150,
    };
    parts.monsters[idx].attacks[1].att_type = 2;
    parts.monsters[idx].attacks[1].acc = 31_999; // no such spell
    parts.monsters[idx].attacks[1].energy = 500;
    let w = World::new(parts).unwrap();
    let m = &w.monsters[idx];
    // over 100 percent: the sum stops before adding the drop
    assert_eq!(calculate_monster_item_bonuses(&w, m, &[4]), 0);
    let env = GlueEnv {
        profile: Profile::Stock,
        nmr_ver: 1.83,
    };
    let mut sim = MonsterSimInput::default();
    reset_values(&mut sim);
    populate_monster_data_to_attack_sim(&w, &env, m.number, &mut sim);
    // the slot keeps its type, energy and chance but nothing else
    assert_eq!(sim.atk_type[1], 2);
    assert_eq!(sim.atk_energy[1], 500);
    assert_eq!(
        (sim.atk_min[1], sim.atk_max[1], sim.atk_success[1]),
        (0, 0, 0)
    );
}
