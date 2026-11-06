// Fixture tests for the monster_glue_char family: see docs/porting.md step 5.
//
// Expected values: fixtures/rules/monster_glue_char.json, produced by the oracle running MME's own
// SetupMonsterAttackSimWithCharStats, CalculateMonsterDamageVsChar, GetPreCalculatedMonsterDamage
// and GetMonsterAvgDmgFromDB. The World is the "table" key of the scenario file (see
// monster_glue.rs); the characters are controls copied from pipeline fixture cases, and the
// adapter test below rebuilds those characters with refresh_all.
use super::fixture::{cur, load};
use super::monster_glue::world_from;
use super::pipeline::{Inputs, world as pipeline_world};
use serde_json::Value;
use std::path::PathBuf;
use touchstone_model::World;
use touchstone_rules::Profile;
use touchstone_rules::character::PartyFilter;
use touchstone_rules::monster_glue::{
    CharSimInputs, GlueEnv, PreCalcInput, calculate_monster_damage_vs_char,
    calculate_monster_damage_vs_char_all, get_monster_avg_dmg_from_db,
    get_pre_calculated_monster_damage, setup_monster_attack_sim_with_char_stats,
};
use touchstone_rules::monster_sim::MonsterSimInput;
use touchstone_rules::pipeline::refresh_all;
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Rnd};

fn scenario() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/monster_glue_char.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn i(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}

/// `Val` of a control's text, caption or tag (a missing control reads as an empty string).
fn val_of(v: &Value) -> f64 {
    vb::val(v.as_str().unwrap_or(""))
}

/// The tag as `Option`: `None` for an empty string (error 13 on assignment to a Long).
fn tag_of(v: &Value) -> Option<f64> {
    match v.as_str() {
        Some("") | None => None,
        Some(s) => Some(vb::val(s)),
    }
}

/// A character from the scenario's `controls` dict (strings, as the generator wrote them).
fn inputs_from_controls(c: &Value) -> CharSimInputs {
    let s = |k: &str| c.get(k).cloned().unwrap_or(Value::Null);
    CharSimInputs {
        class: c.get("cls").map_or(0, i),
        ac: val_of(&s("ac")),
        dr: val_of(&s("dr")),
        mr: val_of(&s("mr")),
        dodge: val_of(&s("dodge")),
        anti_magic: c.get("am").is_some_and(|v| v.as_i64() == Some(1)),
        prot_evil: val_of(&s("pe")),
        resist_col: tag_of(&s("col")),
        resist_fir: tag_of(&s("fir")),
        resist_sto: tag_of(&s("sto")),
        resist_lit: tag_of(&s("lit")),
        resist_wat: tag_of(&s("wat")),
    }
}

/// frmMain's controls as a case sees them: the case's own, else the family setup's.
struct Forms<'a> {
    case: &'a Value,
    setup: &'a Value,
}

impl Forms<'_> {
    fn ctl(&self, name: &str) -> &Value {
        let v = &self.case[name];
        if v.is_null() { &self.setup[name] } else { v }
    }
    fn elem(&self, name: &str, idx: usize) -> &Value {
        &self.ctl(name)["$array"][idx.to_string()]
    }
    fn char_inputs(&self) -> CharSimInputs {
        let stat = |k: usize| self.elem("lblInvenCharStat", k);
        let class = {
            let e = self.elem("cmbGlobalClass", 0);
            if e["ListIndex"].as_i64().unwrap_or(-1) > 0 {
                i(&e["ItemData"]["1"])
            } else {
                0
            }
        };
        CharSimInputs {
            class,
            ac: val_of(&stat(2)["Caption"]),
            dr: val_of(&stat(3)["Caption"]),
            mr: val_of(&self.ctl("txtCharMR")["Text"]),
            dodge: val_of(&self.ctl("lblCharDodge")["Tag"]),
            anti_magic: self.ctl("chkCharAntiMagic")["Value"].as_i64() == Some(1),
            prot_evil: val_of(&stat(20)["Tag"]),
            resist_col: tag_of(&stat(28)["Tag"]),
            resist_fir: tag_of(&stat(27)["Tag"]),
            resist_sto: tag_of(&stat(25)["Tag"]),
            resist_lit: tag_of(&stat(29)["Tag"]),
            resist_wat: tag_of(&stat(26)["Tag"]),
        }
    }
    fn party(&self) -> PartyFilter {
        let mut p = PartyFilter {
            lair_mode: self.elem("optMonsterFilter", 1)["Value"].as_bool() == Some(true),
            ..PartyFilter::default()
        };
        for (k, t) in p.lair_text.iter_mut().enumerate() {
            *t = self.elem("txtMonsterLairFilter", k)["Text"]
                .as_str()
                .unwrap_or("")
                .to_string();
        }
        p
    }
    fn use_character(&self) -> bool {
        self.ctl("chkGlobalFilter")["Value"].as_i64() == Some(1)
    }
}

/// The three damage arrays as the case left them (-1 is "not calculated").
struct Caches {
    party: Vec<Currency>,
    chr: Vec<Currency>,
    default: Vec<Currency>,
}

/// The setup's array with the case's list laid over its first elements (how the oracle assigns a
/// list to an array global that is already dimensioned).
fn currency_array(setup: &Value, case: &Value) -> Vec<Currency> {
    let cur = |x: &Value| Currency::from_f64(x.as_f64().unwrap()).unwrap();
    let mut a: Vec<Currency> = setup.as_array().unwrap().iter().map(cur).collect();
    for (slot, x) in a.iter_mut().zip(case.as_array().into_iter().flatten()) {
        *slot = cur(x);
    }
    a
}

fn check(name: &str, got: &dyn std::fmt::Debug, want: &dyn std::fmt::Debug) -> Option<String> {
    let (g, w) = (format!("{got:?}"), format!("{want:?}"));
    (g != w).then(|| format!("{name}: got {g}, want {w}"))
}

fn arr(a: &[i16; 5]) -> Vec<i64> {
    a.iter().map(|&x| i64::from(x)).collect()
}

fn ints(v: &Value) -> Vec<i64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap())
        .collect()
}

fn strs(a: &[String; 5]) -> Vec<&str> {
    a.iter().map(String::as_str).collect()
}

fn strs_v(v: &Value) -> Vec<&str> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect()
}

fn run_case(world: &World, setup: &Value, setup_rounds: i32, case: &Value) -> Option<String> {
    let inp = &case["inputs"];
    let out = &case["outputs"];
    let forms = Forms {
        case: &inp["forms"]["frmMain"],
        setup: &setup["forms"]["frmMain"],
    };
    let env = GlueEnv {
        profile: Profile::Stock,
        nmr_ver: inp["globals"]["nNMRVer"]
            .as_f64()
            .unwrap_or_else(|| setup["globals"]["nNMRVer"].as_f64().unwrap()),
    };
    let sim_rounds = inp["globals"]["nGlobalMonsterSimRounds"]
        .as_i64()
        .map_or(setup_rounds, |r| r as i32);
    let mut caches = Caches {
        party: currency_array(
            &setup["globals"]["nMonsterDamageVsParty"],
            &inp["globals"]["nMonsterDamageVsParty"],
        ),
        chr: currency_array(
            &setup["globals"]["nMonsterDamageVsChar"],
            &inp["globals"]["nMonsterDamageVsChar"],
        ),
        default: currency_array(
            &setup["globals"]["nMonsterDamageVsDefault"],
            &inp["globals"]["nMonsterDamageVsDefault"],
        ),
    };
    let chr = forms.char_inputs();
    let party = forms.party();
    let mut rnd = Rnd::from_state(inp["rnd"].as_u64().unwrap_or(327_680) as u32);
    let mut sim = MonsterSimInput::default();

    for st in inp["steps"].as_array().map_or(&vec![], |s| s) {
        let args: Vec<&Value> = st["args"].as_array().map_or(vec![], |a| a.iter().collect());
        let name = st["as"].as_str();
        match st["call"].as_str().unwrap() {
            "SetupMonsterAttackSimWithCharStats" => {
                // nRounds = 50, bDynamic = True, bPartyInstead = False, nPartyAntiMagic = 0
                setup_monster_attack_sim_with_char_stats(
                    &env,
                    args.first().map_or(50, |v| i(v)),
                    args.get(1).is_none_or(|v| v.as_bool().unwrap()),
                    args.get(2).is_some_and(|v| v.as_bool().unwrap()),
                    args.get(3).map_or(0, |v| i(v)),
                    &chr,
                    &party,
                    &mut sim,
                );
            }
            "CalculateMonsterDamageVsChar" => {
                let n = i(args[0]);
                let party_instead = args.get(1).is_some_and(|v| v.as_bool().unwrap());
                let r = calculate_monster_damage_vs_char(
                    world,
                    &env,
                    n,
                    party_instead,
                    sim_rounds,
                    &chr,
                    &party,
                    &mut sim,
                    &mut rnd,
                )
                .unwrap();
                if let Some(d) =
                    name.and_then(|key| check("damage", &r.damage.scaled(), &cur(&out[key])))
                {
                    return Some(d);
                }
                // the store is skipped by MME's handler when the monster is past the array's end
                if let Some(c) = r.cached {
                    let a = if party_instead {
                        &mut caches.party
                    } else {
                        &mut caches.chr
                    };
                    if let Some(slot) = usize::try_from(n).ok().and_then(|k| a.get_mut(k)) {
                        *slot = c;
                    }
                }
            }
            "GetPreCalculatedMonsterDamage" => {
                let input = PreCalcInput {
                    use_character: forms.use_character(),
                    party: &party,
                    vs_party: &caches.party,
                    vs_char: &caches.chr,
                    vs_default: &caches.default,
                };
                let n_party = args.get(2).map_or(0, |v| i(v) as i16);
                let (v, label) = get_pre_calculated_monster_damage(
                    world,
                    &env,
                    &input,
                    i(args[0]),
                    n_party,
                    sim_rounds,
                    &mut sim,
                    &mut rnd,
                )
                .unwrap();
                let key = name.unwrap();
                let d = check(
                    "cached",
                    &v.to_bits(),
                    &out[key].as_f64().unwrap().to_bits(),
                )
                .or_else(|| {
                    check(
                        "label",
                        &label.as_str(),
                        &out[format!("{key}.refs")][0].as_str().unwrap(),
                    )
                });
                if d.is_some() {
                    return d;
                }
            }
            "GetMonsterAvgDmgFromDB" => {
                let v = get_monster_avg_dmg_from_db(
                    world,
                    &env,
                    i(args[0]),
                    sim_rounds,
                    &mut sim,
                    &mut rnd,
                )
                .unwrap();
                if let Some(d) = check("avgdb", &v, &i(&out[name.unwrap()])) {
                    return Some(d);
                }
            }
            other => panic!("unknown step {other}"),
        }
    }
    for (k, v) in out.as_object().unwrap() {
        let d = match k.as_str() {
            "r" | "r1" | "r2" | "g" | "cache" | "cache.refs" | "g.refs" => None,
            "nUserAC" => check(k, &sim.user_ac, &i(v)),
            "nUserDR" => check(k, &sim.user_dr, &i(v)),
            "nUserDodge" => check(k, &sim.user_dodge, &i(v)),
            "nUserMR" => check(k, &sim.user_mr, &i(v)),
            "nUserAntiMagic" => check(k, &i32::from(sim.user_anti_magic), &i(v)),
            "nUserProtEvil" => check(k, &sim.user_prot_evil, &i(v)),
            "nUserVileWard" => check(k, &sim.user_vile_ward, &i(v)),
            "nUserRFIR" => check(k, &sim.user_rfir, &i(v)),
            "nUserRCOL" => check(k, &sim.user_rcol, &i(v)),
            "nUserRLIT" => check(k, &sim.user_rlit, &i(v)),
            "nUserRWAT" => check(k, &sim.user_rwat, &i(v)),
            "nUserRSTO" => check(k, &sim.user_rsto, &i(v)),
            "nNumberOfRounds" | "rounds" => check(k, &sim.number_of_rounds, &i(v)),
            "bDynamicCalc" => check(k, &sim.dynamic_calc, &v.as_bool().unwrap()),
            "bGreaterMUD" => check(k, &sim.greater_mud, &v.as_bool().unwrap()),
            "nDynamicCalcDifference" => check(k, &sim.dynamic_calc_difference.scaled(), &cur(v)),
            "HIT_MIN" => check(k, &sim.hit_min, &(i(v) as i16)),
            "HIT_CAP" => check(k, &sim.hit_cap, &(i(v) as i16)),
            "SPELL_HIT_CAP" => check(k, &sim.spell_hit_cap, &(i(v) as i16)),
            "DODGE_SOFTCAP" => check(k, &sim.dodge_softcap, &(i(v) as i16)),
            "DODGE_CAP" => check(k, &sim.dodge_cap, &(i(v) as i16)),
            "nCombatLogMaxRounds" => check(k, &sim.combat_log_max_rounds, &i(v)),
            "bCombatLogMaxRoundOnly" => {
                check(k, &sim.combat_log_max_round_only, &v.as_bool().unwrap())
            }
            "bMobIsEvil" => check(k, &sim.mob_is_evil, &v.as_bool().unwrap()),
            "dur" => check(k, &arr(&sim.atk_duration), &ints(v)),
            "hsn" => check(k, &strs(&sim.atk_hit_spell_name), &strs_v(v)),
            "brd" => check(k, &arr(&sim.between_round_duration), &ints(v)),
            "rnd" => check(k, &i64::from(rnd.state()), &v.as_i64().unwrap()),
            other => panic!("unmodelled output {other}"),
        };
        if d.is_some() {
            return d.map(|d| format!("{k}: {d}"));
        }
    }
    None
}

// Expected values: fixtures/rules/monster_glue_char.json (MME's own procedures, run by the oracle).
#[test]
fn monster_glue_char_fixture() {
    let fx = load("monster_glue_char");
    let world = world_from("monster_glue_char");
    let setup = &fx.header["setup"];
    let setup_rounds = setup["globals"]["nGlobalMonsterSimRounds"]
        .as_i64()
        .unwrap() as i32;
    let failures: Vec<String> = fx
        .cases
        .iter()
        .filter_map(|c| {
            run_case(&world, setup, setup_rounds, c).map(|d| format!("{}: {d}", c["id"]))
        })
        .collect();
    eprintln!(
        "monster_glue_char: {} passed, {} failed",
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
    assert!(fx.cases.len() >= 600);
}

// The pipeline characters the scenario copies controls from: the adapter reproduces them.
// Expected values: the scenario's "controls" (taken from fixtures/rules/pipeline.json outputs).
#[test]
fn adapter_reproduces_the_pipeline_controls() {
    let scn = scenario();
    let pipe = load("pipeline");
    let world = pipeline_world(&pipe.header["data"]);
    for (label, pid) in scn["characters"].as_object().unwrap() {
        let case = pipe
            .cases
            .iter()
            .find(|c| c["id"].as_str() == pid.as_str())
            .unwrap_or_else(|| panic!("no pipeline case {pid}"));
        let inputs = Inputs::new(&pipe.header, case);
        let st = refresh_all(
            Profile::Stock,
            &world,
            &inputs.character(),
            &inputs.options(),
        )
        .unwrap();
        assert_eq!(
            st.sim_inputs(),
            inputs_from_controls(&scn["controls"][label]),
            "{label} ({pid})"
        );
    }
}

fn fresh() -> (MonsterSimInput, Rnd) {
    (MonsterSimInput::default(), Rnd::from_state(327_680))
}

// CalculateMonsterDamageVsCharALL walks the Monsters table with a progress-bar form the oracle
// has no shim for, so it is checked against the per-monster function it calls: same order, one
// shared simulator and generator, 0 for a monster with no attack at all (hand-derived from the
// procedure's loop).
#[test]
fn all_is_the_per_monster_function_in_table_order() {
    let scn = scenario();
    let world = world_from("monster_glue_char");
    let env = GlueEnv {
        profile: Profile::Stock,
        nmr_ver: 1.83,
    };
    let chr = inputs_from_controls(&scn["controls"]["antimagic"]);
    let party = PartyFilter::default();
    for party_instead in [false, true] {
        let (mut sim, mut rnd) = fresh();
        let all = calculate_monster_damage_vs_char_all(
            &world,
            &env,
            party_instead,
            100,
            &chr,
            &party,
            &mut sim,
            &mut rnd,
        )
        .unwrap();
        assert_eq!(all.len(), world.monsters.len());
        let (mut sim, mut rnd) = fresh();
        let mut zeros = 0;
        for (m, (number, stored)) in world.monsters.iter().zip(&all) {
            assert_eq!(m.number, *number);
            let has_attack = m.attacks.iter().any(|a| a.att_type > 0 && a.att_type < 4)
                || m.mid_spells.iter().any(|s| s.spell > 0);
            if has_attack {
                let r = calculate_monster_damage_vs_char(
                    &world,
                    &env,
                    m.number,
                    party_instead,
                    100,
                    &chr,
                    &party,
                    &mut sim,
                    &mut rnd,
                )
                .unwrap();
                assert_eq!(*stored, r.cached, "monster {number}");
            } else {
                zeros += 1;
                assert_eq!(*stored, Some(Currency::from_scaled(0)), "monster {number}");
            }
        }
        assert!(zeros > 0, "the table has monsters without attacks");
    }
}

// The progress variant (P4-30) is the same walk: same values, and one call per monster in table order with the running
// count and the table size (where MME advances its progress bar).
#[test]
fn all_with_progress_reports_each_monster_and_changes_nothing() {
    use touchstone_rules::monster_glue::calculate_monster_damage_vs_char_all_with;
    let scn = scenario();
    let world = world_from("monster_glue_char");
    let env = GlueEnv {
        profile: Profile::Stock,
        nmr_ver: 1.83,
    };
    let chr = inputs_from_controls(&scn["controls"]["caster"]);
    let party = PartyFilter::default();
    let (mut sim, mut rnd) = fresh();
    let plain = calculate_monster_damage_vs_char_all(
        &world, &env, false, 100, &chr, &party, &mut sim, &mut rnd,
    )
    .unwrap();
    let (mut sim, mut rnd) = fresh();
    let mut seen = Vec::new();
    let with = calculate_monster_damage_vs_char_all_with(
        &world,
        &env,
        false,
        100,
        &chr,
        &party,
        &mut sim,
        &mut rnd,
        &mut |done, total| {
            seen.push((done, total));
            true
        },
    )
    .unwrap();
    assert_eq!(plain, with);
    let n = world.monsters.len();
    assert_eq!(seen, (1..=n).map(|k| (k, n)).collect::<Vec<_>>());

    // F-001: a progress call that answers false stops the walk after that monster; what was done is the plain walk's
    // beginning (the caller that asked to stop throws it away)
    let (mut sim, mut rnd) = fresh();
    let stopped = calculate_monster_damage_vs_char_all_with(
        &world,
        &env,
        false,
        100,
        &chr,
        &party,
        &mut sim,
        &mut rnd,
        &mut |done, _| done < 3,
    )
    .unwrap();
    assert_eq!(stopped.len(), 3);
    assert_eq!(stopped[..], plain[..3]);
}

// Hand-derived from CalculateMonsterDamageVsChar and GetMonsterAvgDmgFromDB: monster 0 and below
// do nothing, store nothing and roll nothing.
#[test]
fn monster_zero_stores_nothing() {
    let world = world_from("monster_glue_char");
    let env = GlueEnv {
        profile: Profile::Stock,
        nmr_ver: 1.83,
    };
    let (mut sim, mut rnd) = fresh();
    let r = calculate_monster_damage_vs_char(
        &world,
        &env,
        0,
        false,
        100,
        &CharSimInputs::default(),
        &PartyFilter::default(),
        &mut sim,
        &mut rnd,
    )
    .unwrap();
    assert_eq!((r.damage.scaled(), r.cached), (0, None));
    assert_eq!(rnd.state(), 327_680);
    assert_eq!(
        get_monster_avg_dmg_from_db(&world, &env, 0, 100, &mut sim, &mut rnd).unwrap(),
        0
    );
}
