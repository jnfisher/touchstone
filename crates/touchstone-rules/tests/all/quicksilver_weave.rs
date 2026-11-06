//! Q-013a: the weave's pick, budget, power and proc expectation against the engine vectors in
//! `fixtures/quicksilver/weave.json` (written by `tools/qs-vectors`, family `weave`). The spells
//! are the book in the vector header, built here as MME-shaped rows.

use std::path::PathBuf;

use serde_json::Value;
use touchstone_model::{AbilitySlot, Class, Spell};
use touchstone_rules::Profile;
use touchstone_rules::quicksilver::Balance;
use touchstone_rules::quicksilver::weave::{
    Volley, WeaveTarget, budget, expected_per_hit, expected_proc, pick, power_pct, weaver,
};
use touchstone_rules::spell_cast::{SpellCasterProfile, UsableEnv};

struct Vectors {
    header: Value,
    cases: Vec<Value>,
}

fn load() -> Vectors {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/quicksilver")
        .join("weave.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let mut doc: Value = serde_json::from_str(&text).expect("weave.json parses");
    let Value::Array(cases) = doc["cases"].take() else {
        panic!("no cases array");
    };
    Vectors {
        header: doc["header"].take(),
        cases,
    }
}

fn int(v: &Value) -> i32 {
    i32::try_from(
        v.as_i64()
            .unwrap_or_else(|| panic!("integer expected, got {v}")),
    )
    .unwrap()
}

fn book(v: &Vectors) -> Vec<Spell> {
    v.header["balance_keys"]["book"]
        .as_array()
        .expect("book in the header")
        .iter()
        .map(|s| {
            let mut abilities = [AbilitySlot::default(); 10];
            for (i, a) in s["abilities"].as_array().unwrap().iter().enumerate() {
                abilities[i] = AbilitySlot {
                    ability: int(&a[0]),
                    value: int(&a[1]),
                };
            }
            let pair = |k: &str| (int(&s[k][0]), int(&s[k][1]));
            let (min_inc, min_inc_lvls) = pair("min_inc");
            let (max_inc, max_inc_lvls) = pair("max_inc");
            Spell {
                number: int(&s["number"]),
                name: Some(format!("s{}", int(&s["number"]))),
                short: None,
                req_level: int(&s["req_level"]),
                energy_cost: int(&s["energy"]),
                mana_cost: 10,
                min_base: int(&s["min"]),
                max_base: int(&s["max"]),
                diff: 0,
                type_of_resists: int(&s["resmode"]),
                targets: int(&s["target"]),
                dur: int(&s["duration"]),
                att_type: int(&s["element"]),
                magery: int(&s["magery"]),
                magery_lvl: int(&s["magelvl"]),
                cap: int(&s["cap"]),
                max_inc_lvls,
                max_inc,
                min_inc_lvls,
                min_inc,
                dur_inc_lvls: 0,
                dur_inc: 0,
                abilities,
                learnable: true,
                learned_from: None,
                casted_by: None,
                classes: None,
            }
        })
        .collect()
}

fn class(i: &Value) -> Class {
    Class {
        number: int(&i["class"]),
        name: String::new(),
        min_hits: 0,
        max_hits: 0,
        exp_table: 0,
        magery_type: int(&i["magery"]),
        magery_lvl: int(&i["magelvl"]),
        weapon_type: 0,
        armour_type: 0,
        combat_lvl: 0,
        abilities: [AbilitySlot::default(); 10],
    }
}

fn target(t: &Value) -> WeaveTarget {
    WeaveTarget {
        mr: int(&t["mr"]),
        anti_magic: t["anti_magic"].as_bool().unwrap(),
        cold: int(&t["cold"]),
        fire: int(&t["fire"]),
        stone: int(&t["stone"]),
        lightning: int(&t["lightning"]),
        water: int(&t["water"]),
        poison: int(&t["poison"]),
        spell_immunity: int(&t["spell_immunity"]),
        undead: false,
        animal: t["animal"].as_bool().unwrap(),
        non_living: t["non_living"].as_bool().unwrap(),
    }
}

fn usable<'a>(profile: Profile, spells: &'a [Spell], classes: &'a [Class]) -> UsableEnv<'a> {
    UsableEnv {
        profile,
        spells,
        classes,
        nmr_ver: 1.8,
        only_in_game: false,
        disable_kai_autolearn: false,
    }
}

fn v120() -> Profile {
    Profile::quicksilver_v120()
}

#[test]
fn weave_header_and_ids() {
    let v = load();
    assert_eq!(v.header["family"], "weave");
    assert_eq!(v.header["content_version"], "1.20.0");
    assert_eq!(
        v.header["case_count"].as_u64().unwrap(),
        v.cases.len() as u64
    );
    let ids: Vec<&str> = v.cases.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids sorted and unique");
    let keys = &v.header["balance_keys"];
    let b = Balance::V120;
    for (key, want) in [
        ("weave_pct", b.weave_pct),
        ("weave_pct_magery1", b.weave_pct_magery1),
        ("weave_budget_per_level", b.weave_budget_per_level),
        ("weave_power_min_pct", b.weave_power_min_pct),
        ("weave_power_max_pct", b.weave_power_max_pct),
        ("weave_stat_bonus_pct", b.weave_stat_bonus_pct),
        ("scalecap", b.scalecap),
        ("spell_stat_permille", b.spell_stat_permille),
        ("spell_stat_knee", b.spell_stat_knee),
    ] {
        assert_eq!(int(&keys[key]["v120"]), want as i32, "{key}");
    }
    for kind in ["weaver", "budget", "power", "pick", "proc"] {
        assert!(v.cases.iter().any(|c| c["kind"] == kind), "no {kind} cases");
    }
}

/// weaver, budget, power and pick under Quicksilver 1.20.0.
#[test]
fn weave_rules_match_the_engine() {
    let v = load();
    let spells = book(&v);
    let profile = v120();
    let mut bad = Vec::new();
    let mut picked = 0;
    for c in &v.cases {
        let (i, id, want) = (&c["inputs"], c["id"].as_str().unwrap(), &c["v120"]);
        match c["kind"].as_str().unwrap() {
            "weaver" => {
                let got = weaver(profile, &class(i)).map_or(Value::Null, |w| {
                    serde_json::json!({ "chance_pct": w.chance_pct, "bard": w.bard, "class": w.class })
                });
                if got != *want {
                    bad.push(format!("{id}: got {got}, vector {want}"));
                }
            }
            "budget" => {
                let got = budget(profile, int(&i["level"]));
                if got != want.as_i64().unwrap() {
                    bad.push(format!("{id}: got {got}, vector {want}"));
                }
            }
            "power" => {
                let vol = Volley {
                    hits: Vec::new(),
                    twice_average: i["twice_average"].as_i64().unwrap(),
                };
                let got = power_pct(profile, i["budget"].as_i64().unwrap(), &vol);
                if i64::from(got) != want.as_i64().unwrap() {
                    bad.push(format!("{id}: got {got}, vector {want}"));
                }
            }
            "pick" => {
                let classes = [class(i)];
                let env = usable(profile, &spells, &classes);
                let t = target(&i["target"]);
                let got = pick(&env, &classes[0], int(&i["level"]) as i16, 2, Some(&t)).map_or(
                    Value::Null,
                    |p| {
                        serde_json::json!({
                            "spell": p.spell.number, "twice_average": p.volley.twice_average,
                            "power_pct": p.power_pct,
                            "hits": p.volley.hits.iter().map(|s| s.number).collect::<Vec<_>>(),
                        })
                    },
                );
                picked += usize::from(!got.is_null());
                if got != *want {
                    bad.push(format!("{id}: got {got}, vector {want}"));
                }
            }
            _ => {}
        }
    }
    assert!(picked > 100, "the pick vectors must mostly pick something");
    assert!(
        bad.is_empty(),
        "{} differ; first:\n{}",
        bad.len(),
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}

/// The expected damage of a proc equals the engine's mean over every roll, and the resist rolls
/// agree.
#[test]
fn weave_proc_expectation_matches_the_engine() {
    let v = load();
    let spells = book(&v);
    let profile = v120();
    let mut bad = Vec::new();
    let mut checked = 0;
    for c in v.cases.iter().filter(|c| c["kind"] == "proc") {
        let (i, id, want) = (&c["inputs"], c["id"].as_str().unwrap(), &c["v120"]);
        let classes = [class(i)];
        let env = usable(profile, &spells, &classes);
        let t = target(&i["target"]);
        let level = int(&i["level"]);
        let p = pick(&env, &classes[0], level as i16, 2, Some(&t)).expect("proc cases pick");
        let w = weaver(profile, &classes[0]).unwrap();
        let who = SpellCasterProfile {
            casting_stat: int(&i["stat"]),
            class: classes[0].number,
            level,
            spell_dmg_bonus: int(&i["bonus"]) as i16,
            ..SpellCasterProfile::default()
        };
        let got = expected_proc(profile, &p, &w, &who, &t);
        let (mean, rolls) = (
            want["unresisted"].as_f64().unwrap(),
            int(&want["resist_rolls"]),
        );
        checked += 1;
        if int(&want["spell"]) != p.spell.number
            || got.resist_rolls != rolls
            || (got.unresisted - mean).abs() > 1e-9 * mean.abs().max(1.0)
        {
            bad.push(format!(
                "{id}: spell {} mean {} rolls {}, vector {want}",
                p.spell.number, got.unresisted, got.resist_rolls
            ));
        }
        let per_hit = expected_per_hit(&env, &classes[0], 2, &who, &t);
        let want_hit = f64::from(w.chance_pct) / 100.0 * mean * f64::from(99 - rolls) / 99.0;
        if (per_hit - want_hit).abs() > 1e-9 * want_hit.abs().max(1.0) {
            bad.push(format!("{id}: per hit {per_hit}, want {want_hit}"));
        }
    }
    assert_eq!(checked, 504);
    assert!(
        bad.is_empty(),
        "{} differ; first:\n{}",
        bad.len(),
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}

/// Stock has no weave; the neutral balance weaves with chance 0, which is the neutral column.
#[test]
fn weave_is_off_under_stock_and_neutral() {
    let v = load();
    let spells = book(&v);
    let neutral = Profile::Quicksilver(&Balance::NEUTRAL);
    for c in &v.cases {
        let i = &c["inputs"];
        match c["kind"].as_str().unwrap() {
            "weaver" => {
                let cl = class(i);
                assert!(weaver(Profile::Stock, &cl).is_none());
                let got = weaver(neutral, &cl).map(|w| w.chance_pct);
                let want = c["neutral"]["chance_pct"].as_u64().map(|n| n as u32);
                assert_eq!(got, want, "{}", c["id"]);
            }
            "proc" => {
                let classes = [class(i)];
                let t = target(&i["target"]);
                let who = SpellCasterProfile {
                    casting_stat: int(&i["stat"]),
                    class: classes[0].number,
                    level: int(&i["level"]),
                    ..SpellCasterProfile::default()
                };
                let env = usable(Profile::Stock, &spells, &classes);
                assert_eq!(expected_per_hit(&env, &classes[0], 2, &who, &t), 0.0);
                assert!(pick(&env, &classes[0], 40, 2, None).is_none());
                let env = usable(neutral, &spells, &classes);
                assert_eq!(expected_per_hit(&env, &classes[0], 2, &who, &t), 0.0);
            }
            _ => {}
        }
    }
}

/// The vectors must reach the rules: classes whose weave share, the power clamp, the chain and
/// both damage kinds all show up.
#[test]
fn weave_vectors_reach_each_rule() {
    let v = load();
    let powers: Vec<i64> = v
        .cases
        .iter()
        .filter(|c| c["kind"] == "power")
        .map(|c| c["v120"].as_i64().unwrap())
        .collect();
    assert!(powers.contains(&25), "power clamped to the minimum");
    assert!(powers.contains(&100), "power at the maximum");
    assert!(
        powers.iter().any(|&p| p > 25 && p < 100),
        "power in between"
    );
    let picks: std::collections::BTreeSet<i64> = v
        .cases
        .iter()
        .filter(|c| c["kind"] == "pick" && !c["v120"].is_null())
        .map(|c| c["v120"]["spell"].as_i64().unwrap())
        .collect();
    assert!(
        picks.len() >= 8,
        "several different spells picked: {picks:?}"
    );
    assert!(
        v.cases
            .iter()
            .any(|c| c["kind"] == "proc" && c["v120"]["resist_rolls"].as_i64().unwrap() > 0)
    );
}
