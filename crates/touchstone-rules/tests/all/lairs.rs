// Fixture tests for the lairs family: see docs/porting.md step 5.
use super::fixture::load;
use serde_json::Value;

use touchstone_rules::combat::DamageOutput;
use touchstone_rules::lairs::*;
use touchstone_vb6::Currency;

fn cur_long(v: i64) -> Currency {
    Currency::from_scaled(v * 10_000)
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

fn cur_of(v: &Value) -> Currency {
    Currency::from_f64(num(v)).unwrap()
}

/// Lair rows as the generator wrote them (VB field names).
fn info_from_row(row: &Value, group_index: &str, regen: i16) -> LairInfo {
    let mut i = LairInfo::default();
    for (k, v) in row.as_object().unwrap() {
        match k.as_str() {
            "sMobList" => i.mob_list = v.as_str().unwrap().to_string(),
            "nMobs" => i.mobs = cur_of(v),
            "nAvgExp" => i.avg_exp = cur_of(v),
            "nAvgDmg" => i.avg_dmg = cur_of(v),
            "nAccyMajority" => i.accy_majority = num(v) as i32,
            "nAccyMax" => i.accy_max = num(v) as i32,
            "nAvgHP" => i.avg_hp = num(v) as i32,
            "nAvgAC" => i.avg_ac = num(v) as i16,
            "nAvgDR" => i.avg_dr = num(v) as i16,
            "nAvgMR" => i.avg_mr = num(v) as i16,
            "nAvgRCOL" => i.avg_rcol = num(v) as i16,
            "nAvgRFIR" => i.avg_rfir = num(v) as i16,
            "nAvgRSTO" => i.avg_rsto = num(v) as i16,
            "nAvgRLIT" => i.avg_rlit = num(v) as i16,
            "nAvgRWAT" => i.avg_rwat = num(v) as i16,
            "nAvgDodge" => i.avg_dodge = num(v) as i16,
            "nAvgBSDefense" => i.avg_bs_defense = num(v) as i16,
            "nAvgWalk" => i.avg_walk = cur_of(v),
            "nAvgDelay" => i.avg_delay = num(v),
            "nDamageMitigated" => i.damage_mitigated = num(v) as i32,
            "nDamageOut" => i.damage_out = num(v) as i32,
            "nFirstRoundDamageOut" => i.first_round_damage_out = num(v) as i32,
            "nMinRoundDamageOut" => i.min_round_damage_out = num(v) as i32,
            "nSurpriseDamageOut" => i.surprise_damage_out = num(v) as i32,
            "nSurpriseMinDamageOut" => i.surprise_min_damage_out = num(v) as i32,
            "nSurpriseChance" => i.surprise_chance = num(v) as i16,
            "nAvgDmgLair" => i.avg_dmg_lair = cur_of(v),
            "nAvgDmgClearRound" => i.avg_dmg_clear_round = cur_of(v),
            "nAvgDmgClear" => i.avg_dmg_clear = cur_of(v),
            "nMagicLVL" => i.magic_lvl = num(v) as i16,
            "nMaxMagicLVL" => i.max_magic_lvl = num(v) as i16,
            "nSpellImmuLVL" => i.spell_immu_lvl = num(v) as i16,
            "nMaxSpellImmuLVL" => i.max_spell_immu_lvl = num(v) as i16,
            "nNumUndeads" => i.num_undeads = num(v) as i16,
            "nNumAntiMagic" => i.num_anti_magic = num(v) as i16,
            "nNumAnimals" => i.num_animals = num(v) as i16,
            "nNumLiving" => i.num_living = num(v) as i16,
            other => panic!("unknown lair field {other}"),
        }
    }
    // what the oracle's stand-in for GetLairInfo sets from its arguments
    i.group_index = group_index.to_string();
    i.max_regen = cur_long(i64::from(regen));
    i.rtk = 1.0;
    i.rtc = f64::from(regen);
    i
}

struct TestEnv<'a> {
    shared: &'a Value,
    case: &'a Value,
    damage: DamageOutput,
    calls: Vec<Vec<i64>>,
}

impl LairEnv for TestEnv<'_> {
    fn get_lair_info(&mut self, gi: &str, regen: i16) -> LairInfo {
        let row = self.case["lairs"].get(gi).or_else(|| self.shared.get(gi));
        row.map_or_else(LairInfo::default, |r| info_from_row(r, gi, regen))
    }
    fn get_damage_output(&mut self, q: &AvgDefence) -> DamageOutput {
        self.calls.push(vec![
            q.ac.into(),
            q.dr.into(),
            q.mr.into(),
            q.dodge.into(),
            q.flags.into(),
            100,
            q.spell_immu_lvl.into(),
            q.magic_lvl.into(),
        ]);
        self.damage
    }
}

fn damage_from(env: &Value) -> DamageOutput {
    let mut d = DamageOutput::start();
    d.average_damage = cur_long(0);
    d.first_round_damage = cur_long(0);
    d.surprise_damage = cur_long(0);
    if let Some(m) = env["damage_out"].as_object() {
        for (k, v) in m {
            match k.as_str() {
                "nAverageDamage" => d.average_damage = cur_of(v),
                "nSurpriseDamage" => d.surprise_damage = cur_of(v),
                "nSurpriseDamageChance" => d.surprise_damage_chance = num(v) as i16,
                other => panic!("unknown damage field {other}"),
            }
        }
    }
    d
}

enum V {
    C(Currency),
    I(i64),
    F(f64),
    S(String),
}

fn fields(i: &LairInfo) -> Vec<(&'static str, V)> {
    use V::*;
    vec![
        ("sgroupindex", S(i.group_index.clone())),
        ("smoblist", S(i.mob_list.clone())),
        ("nmobs", C(i.mobs)),
        ("nmaxregen", C(i.max_regen)),
        ("navgexp", C(i.avg_exp)),
        ("navgdmg", C(i.avg_dmg)),
        ("naccymajority", I(i.accy_majority.into())),
        ("naccymax", I(i.accy_max.into())),
        ("navghp", I(i.avg_hp.into())),
        ("navgac", I(i.avg_ac.into())),
        ("navgdr", I(i.avg_dr.into())),
        ("navgmr", I(i.avg_mr.into())),
        ("navgrcol", I(i.avg_rcol.into())),
        ("navgrfir", I(i.avg_rfir.into())),
        ("navgrsto", I(i.avg_rsto.into())),
        ("navgrlit", I(i.avg_rlit.into())),
        ("navgrwat", I(i.avg_rwat.into())),
        ("navgdodge", I(i.avg_dodge.into())),
        ("navgbsdefense", I(i.avg_bs_defense.into())),
        ("ntotallairs", I(i.total_lairs.into())),
        ("navgwalk", C(i.avg_walk)),
        ("navgdelay", F(i.avg_delay)),
        ("ndamagemitigated", I(i.damage_mitigated.into())),
        ("ndamageout", I(i.damage_out.into())),
        ("nfirstrounddamageout", I(i.first_round_damage_out.into())),
        ("nminrounddamageout", I(i.min_round_damage_out.into())),
        ("nsurprisedamageout", I(i.surprise_damage_out.into())),
        ("nsurprisemindamageout", I(i.surprise_min_damage_out.into())),
        ("nsurprisechance", I(i.surprise_chance.into())),
        ("npossspawns", I(i.poss_spawns.into())),
        ("sglobalattackconfig", S(i.global_attack_config.clone())),
        ("navgdmglair", C(i.avg_dmg_lair)),
        ("navgdmgclearround", C(i.avg_dmg_clear_round)),
        ("navgdmgclear", C(i.avg_dmg_clear)),
        ("nrtk", F(i.rtk)),
        ("nrtc", F(i.rtc)),
        ("nmagiclvl", I(i.magic_lvl.into())),
        ("nmaxmagiclvl", I(i.max_magic_lvl.into())),
        ("nspellimmulvl", I(i.spell_immu_lvl.into())),
        ("nmaxspellimmulvl", I(i.max_spell_immu_lvl.into())),
        ("nnumundeads", I(i.num_undeads.into())),
        ("nnumantimagic", I(i.num_anti_magic.into())),
        ("nnumanimals", I(i.num_animals.into())),
        ("nnumliving", I(i.num_living.into())),
    ]
}

fn same(v: &V, e: &Value) -> bool {
    match v {
        V::C(c) => e["cur"].as_i64() == Some(c.scaled()),
        V::I(i) => e.as_i64() == Some(*i),
        V::F(f) => e
            .as_f64()
            .is_some_and(|x| x.to_bits() == f.to_bits() || x == *f),
        V::S(s) => e.as_str() == Some(s.as_str()),
    }
}

fn f64s(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(num).collect()
}

fn same_floats(a: &[f64], e: &Value) -> bool {
    let e = f64s(e);
    a.len() == e.len() && a.iter().zip(&e).all(|(x, y)| x == y)
}

fn input_array(arg: &Value) -> Vec<f64> {
    arg["values"]
        .as_array()
        .map_or_else(Vec::new, |v| v.iter().map(num).collect())
}

fn dict_of(arg: &Value) -> CountDict {
    CountDict {
        items: arg["dict"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (p[0].as_i64().unwrap() as i32, p[1].as_i64().unwrap() as i32))
            .collect(),
    }
}

#[test]
fn every_fixture_case_matches() {
    let fx = load("lairs");
    let shared = &fx.header["data"]["lairs"];
    let mut bad = Vec::new();
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        let inp = &case["inputs"];
        let out = &case["outputs"];
        let nmr = inp["globals"]["nNMRVer"].as_f64().unwrap_or(0.0);
        let args = inp["args"].as_array().cloned().unwrap_or_default();
        let env_json = &inp["lair_env"];
        let mut env = TestEnv {
            shared,
            case: env_json,
            damage: damage_from(env_json),
            calls: Vec::new(),
        };
        let ok = if let Some(p) = inp["proc"].as_str() {
            match p {
                "CountLairsByMaxRegen" => {
                    let n = count_lairs_by_max_regen(
                        args[0].as_str().unwrap(),
                        nmr,
                        num(&args[1]),
                        num(&args[2]),
                    );
                    out["result"].as_i64() == Some(n.into())
                }
                "GetLairAveragesFromLocs" => {
                    let loc = args[0].as_str().unwrap();
                    let sb = match env_json["summoned_by"].as_str().unwrap() {
                        "$loc" => loc,
                        s => s,
                    };
                    let cfg = inp["globals"]["sGlobalAttackConfig"].as_str().unwrap_or("");
                    let r = get_lair_averages_from_locs(
                        &mut env,
                        loc,
                        &LairLocsInput {
                            nmr_ver: nmr,
                            summoned_by: sb,
                            global_attack_config: cfg,
                        },
                    );
                    let want: Vec<Vec<i64>> = case["events"]
                        .as_array()
                        .map(|e| {
                            e.iter()
                                .filter(|x| x["kind"] == "damage_output")
                                .map(|x| {
                                    x["args"]
                                        .as_array()
                                        .unwrap()
                                        .iter()
                                        .map(|a| a.as_i64().unwrap())
                                        .collect()
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    fields(&r).iter().all(|(k, v)| same(v, &out["result"][*k])) && want == env.calls
                }
                "QuickSort" => {
                    let mut a = input_array(&args[0]);
                    quick_sort(
                        &mut a,
                        args[1].as_i64().unwrap() as i32,
                        args[2].as_i64().unwrap() as i32,
                    );
                    out["result.refs"]
                        .as_array()
                        .is_none_or(|r| same_floats(&a, &r[0]))
                        || a.is_empty()
                }
                "GetMedian" => same_f(get_median(&input_array(&args[0])), out),
                "GetMedianAbsDev" => same_f(
                    get_median_abs_dev(&input_array(&args[0]), num(&args[1])),
                    out,
                ),
                "GetStdDev" => same_f(get_std_dev(&input_array(&args[0])), out),
                "RemoveOutliers" => {
                    let mut a = input_array(&args[0]);
                    remove_outliers(&mut a);
                    a.is_empty() || same_floats(&a, &out["result.refs"][0])
                }
                "DICT_ModeFromCounts" => {
                    dict_of(&args[0]).mode(args[1].as_i64().unwrap() as i32)
                        == out["result"].as_i64().unwrap() as i32
                }
                "DICT_BumpCount" | "DICT_AddToCount" => {
                    let mut d = dict_of(&args[0]);
                    let key = args[1].as_i64().unwrap() as i32;
                    let r = if p == "DICT_BumpCount" {
                        d.bump_count(key)
                    } else {
                        d.add_to_count(key, args[2].as_i64().unwrap() as i32)
                    };
                    let want: Vec<(i32, i32)> = out["result.dicts"][0]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| (p[0].as_i64().unwrap() as i32, p[1].as_i64().unwrap() as i32))
                        .collect();
                    r.is_ok() && d.items == want
                }
                other => panic!("unknown proc {other}"),
            }
        } else {
            // CalculateAverageLairs
            let mut possy = vec![
                Currency::from_scaled(0);
                inp["globals"]["nMonsterPossy"].as_array().unwrap().len()
            ];
            let mut chance = possy.clone();
            let rows: Vec<(i32, &str)> = env_json["monsters"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    (
                        r["Number"].as_i64().unwrap() as i32,
                        r["Summoned By"].as_str().unwrap(),
                    )
                })
                .collect();
            calculate_average_lairs(&mut env, nmr, &rows, &mut possy, &mut chance);
            let eq = |a: &[Currency], e: &Value| {
                let e = e.as_array().unwrap();
                a.len() == e.len()
                    && a.iter()
                        .zip(e)
                        .all(|(c, v)| v["cur"].as_i64() == Some(c.scaled()))
            };
            eq(&possy, &out["possy"]) && eq(&chance, &out["chance"])
        };
        if !ok {
            bad.push(id.to_string());
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} cases differ, first: {:?}",
        bad.len(),
        fx.cases.len(),
        &bad[..bad.len().min(10)]
    );
}

fn same_f(x: f64, out: &Value) -> bool {
    out["result"].as_f64().is_some_and(|e| e == x)
}

#[test]
fn quick_sort_keeps_equal_values_in_range() {
    let mut a = [3.0, 1.0, 2.0, 1.0, 3.0];
    quick_sort(&mut a, 0, 4);
    assert_eq!(a, [1.0, 1.0, 2.0, 3.0, 3.0]);
}
