// Fixture tests for the exp family: see docs/porting.md step 5.
use touchstone_model::{AbilitySlot, Class, Race};
use touchstone_rules::exp;
use touchstone_rules::{Profile, RulesError};
use touchstone_vb6::Currency;

use crate::fixture::{cur, load};
use serde_json::Value;

pub(crate) fn class(number: i32, exp_table: i32) -> Class {
    Class {
        number,
        name: String::new(),
        min_hits: 0,
        max_hits: 0,
        exp_table,
        magery_type: 0,
        magery_lvl: 0,
        weapon_type: 0,
        armour_type: 0,
        combat_lvl: 0,
        abilities: [AbilitySlot::default(); 10],
    }
}

pub(crate) fn race(number: i32, exp_table: i32) -> Race {
    Race {
        number,
        name: String::new(),
        m_int: 0,
        m_wil: 0,
        m_str: 0,
        m_hea: 0,
        m_agl: 0,
        m_chm: 0,
        x_int: 0,
        x_wil: 0,
        x_str: 0,
        x_hea: 0,
        x_agl: 0,
        x_chm: 0,
        hp_per_lvl: 0,
        exp_table,
        base_cp: 0,
        abilities: [AbilitySlot::default(); 10],
    }
}

fn arg(case: &Value, i: usize) -> i64 {
    case["inputs"]["args"][i]
        .as_i64()
        .unwrap_or_else(|| panic!("arg {i} not an integer in {}", case["inputs"]))
}

/// What MME produced: an escaped error, a handled error (the procedure then returns 0), or a value.
fn expected_error(case: &Value) -> Option<i32> {
    if let Some(n) = case["error"]["number"].as_i64() {
        return Some(n as i32);
    }
    case["events"][0]["number"].as_i64().map(|n| n as i32)
}

fn run(case: &Value) -> Result<ActualValue, RulesError> {
    let inp = &case["inputs"];
    let a = |i| arg(case, i) as i32;
    match inp["proc"].as_str().unwrap() {
        "CalcExpNeeded_STOCK" => exp::calc_exp_needed_stock(a(0), a(1)).map(ActualValue::Cur),
        "CalcMoneyRequiredToTrain" => {
            let c = |i| Currency::from_scaled(arg(case, i) * 10_000);
            exp::calc_money_required_to_train(c(0), c(1)).map(ActualValue::Cur)
        }
        "CalcExpNeededByRaceClass" => {
            // ExpTable values come from the case's meta; no meta entry means the row is not found.
            let (cn, rn) = (a(1), a(2));
            let classes: Vec<Class> = inp["meta"]["class_exp"]
                .as_i64()
                .map(|e| vec![class(cn, e as i32)])
                .unwrap_or_default();
            let races: Vec<Race> = inp["meta"]["race_exp"]
                .as_i64()
                .map(|e| vec![race(rn, e as i32)])
                .unwrap_or_default();
            exp::calc_exp_needed_by_race_class(Profile::Stock, a(0), cn, rn, &classes, &races)
                .map(ActualValue::F64)
        }
        "GetMaxLevel" => {
            let v = inp["globals"]["gAvgLevelMaxAllStats"].as_f64().unwrap();
            exp::get_max_level(v).map(ActualValue::Long)
        }
        p => panic!("unknown proc {p}"),
    }
}

#[derive(Debug, PartialEq)]
enum ActualValue {
    Cur(Currency),
    F64(f64),
    Long(i32),
}

#[test]
fn fixture_cases() {
    let fx = load("exp");
    let mut failures = Vec::new();
    let mut counts = [0usize; 4];
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap_or("?");
        let result = run(case);
        let ok = match (expected_error(case), &result) {
            // A handled error leaves the function value at 0 and an event; an escaped one has no result.
            (Some(n), Err(RulesError::Vb6(m))) => n == *m,
            (Some(_), _) | (None, Err(_)) => false,
            (None, Ok(v)) => {
                let want = &case["outputs"]["result"];
                match v {
                    ActualValue::Cur(c) => c.scaled() == cur(want),
                    ActualValue::F64(f) => want.as_f64() == Some(*f),
                    ActualValue::Long(n) => want.as_i64() == Some(i64::from(*n)),
                }
            }
        };
        counts[match case["inputs"]["proc"].as_str().unwrap() {
            "CalcExpNeeded_STOCK" => 0,
            "CalcMoneyRequiredToTrain" => 1,
            "CalcExpNeededByRaceClass" => 2,
            _ => 3,
        }] += 1;
        if !ok && failures.len() < 10 {
            failures.push(format!(
                "{id}: inputs {} -> got {result:?}, want {} (error {:?})",
                case["inputs"],
                case["outputs"],
                expected_error(case)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "mismatches (counts {counts:?}):\n{}",
        failures.join("\n")
    );
    assert!(
        counts.iter().all(|&c| c > 0),
        "every procedure has cases: {counts:?}"
    );
}
