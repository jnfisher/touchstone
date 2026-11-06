// Fixture tests for the accuracy family: see docs/porting.md step 5.
// Expected values come from fixtures/rules/accuracy.json (MME's own code run by the oracle).
use serde_json::Value;
use touchstone_model::{AbilitySlot, Class};
use touchstone_rules::Profile;
use touchstone_rules::accuracy::*;

use crate::fixture;

/// The Classes row the case carries (only the columns FAM-06 reads), or `None` for a failed seek.
fn class_row(inputs: &Value) -> Option<Class> {
    let row = &inputs["class"];
    if row.is_null() {
        return None;
    }
    let n = |k: &str| row[k].as_i64().unwrap_or_else(|| panic!("{k} in {row}")) as i32;
    Some(Class {
        number: n("number"),
        name: String::new(),
        min_hits: 0,
        max_hits: 0,
        exp_table: 0,
        magery_type: 0,
        magery_lvl: 0,
        weapon_type: 0,
        armour_type: n("armour_type"),
        combat_lvl: n("combat_lvl"),
        abilities: [AbilitySlot::default(); 10],
    })
}

/// Argument `i` as an integer; an omitted Optional argument is 0.
fn int(a: &[Value], i: usize) -> i64 {
    a.get(i).map_or(0, |v| {
        v.as_i64().unwrap_or_else(|| panic!("not an int: {v}"))
    })
}

fn flag(a: &[Value], i: usize) -> bool {
    a.get(i).is_some_and(|v| v.as_bool().expect("bool"))
}

/// A Double argument with MME's default for an omitted one.
fn dbl(a: &[Value], i: usize, default: f64) -> f64 {
    a.get(i).map_or(default, |v| v.as_f64().expect("number"))
}

/// Runs one case; returns a description of the mismatch, if any.
pub(crate) fn run(profile: Profile, case: &Value) -> Result<(), String> {
    let inp = &case["inputs"];
    let out = &case["outputs"];
    assert!(
        case.get("error").is_none(),
        "no FAM-06 case ends in an escaped error"
    );
    let (proc, args): (&str, Vec<Value>) = if let Some(p) = inp["proc"].as_str() {
        (p, inp["args"].as_array().cloned().unwrap_or_default())
    } else {
        let s = &inp["steps"][0];
        (
            s["call"].as_str().unwrap(),
            s["args"].as_array().cloned().unwrap_or_default(),
        )
    };
    let p = profile;
    let a = &args[..];
    let r = &out["result"];
    let want_i = || r.as_i64().expect("integer result") as i32;

    macro_rules! eq {
        ($got:expr, $want:expr) => {{
            let (g, w) = ($got, $want);
            if g == w {
                Ok(())
            } else {
                Err(format!("got {g:?}, want {w:?}"))
            }
        }};
    }

    match proc {
        "CalculateAccuracy" => {
            let row = class_row(inp);
            let text_in = a.get(9).map_or("", |t| t["value"].as_str().expect("text"));
            let input = AccuracyInput {
                class: int(a, 0) as i16,
                class_row: row.as_ref(),
                level: int(a, 1) as i16,
                strength: int(a, 2) as i16,
                agility: int(a, 3) as i16,
                accuracy_worn: int(a, 6) as i16,
                plus_accuracy: int(a, 7) as i16,
                encum_pct: int(a, 8) as i16,
            };
            let got = calculate_accuracy(p, &input, text_in);
            // With no ByRef argument the oracle cannot see the text MME built locally.
            match out["result.refs"][0].as_str() {
                Some(want_text) => eq!((got.value, got.text.as_str()), (want_i(), want_text)),
                None => eq!(got.value, want_i()),
            }
        }
        "CalculateBackstabAccuracy" => {
            let input = BackstabAccuracyInput {
                stealth: int(a, 0) as i16,
                agility: int(a, 1) as i16,
                plus_bs_accuracy: int(a, 2) as i16,
                class_stealth: flag(a, 3),
                plus_normal_accuracy: int(a, 4) as i16,
            };
            eq!(calculate_backstab_accuracy(p, &input), want_i())
        }
        "CalculateAttackDefense" => {
            let input = AttackDefenseInput {
                accy: int(a, 0) as i32,
                ac: int(a, 1) as i32,
                dodge: int(a, 2) as i32,
                bs_defense: int(a, 3) as i32,
                prot_evil: int(a, 4) as i32,
                prot_good: int(a, 5) as i32,
                perception: int(a, 6) as i32,
                vile_ward: int(a, 7) as i32,
                shadow: flag(a, 9),
                see_hidden: flag(a, 10),
                backstab: flag(a, 11),
                vs_player: flag(a, 12),
                class: int(a, 13) as i32,
            };
            let got = calculate_attack_defense(p, &input);
            let want = r.as_array().expect("array result");
            eq!(
                (i64::from(got.hit_chance), i64::from(got.dodge_chance)),
                (want[0].as_i64().unwrap(), want[1].as_i64().unwrap())
            )
        }
        "CalcDodgeVSAccuracy" => eq!(
            calc_dodge_vs_accuracy(p, int(a, 0) as i32, int(a, 1) as i32, int(a, 2) as i32),
            want_i()
        ),
        "CalcDodge" => eq!(
            calc_dodge(
                int(a, 0) as i16,
                int(a, 1) as i16,
                int(a, 2) as i16,
                dbl(a, 3, 0.0),
                dbl(a, 4, 0.0),
                dbl(a, 5, -1.0),
            ),
            want_i()
        ),
        "DodgeMaxAccForPercent" => eq!(
            dodge_max_acc_for_percent(p, int(a, 0) as i32, int(a, 1) as i32, int(a, 2) as i32),
            want_i()
        ),
        "GetDodgeCap" => eq!(get_dodge_cap(p, int(a, 0) as i32, flag(a, 1)), want_i()),
        "GetHitMin" => eq!(get_hit_min(p, int(a, 0) as i32), want_i()),
        "GetHitCap" => eq!(get_hit_cap(p), want_i()),
        "GetSpellHitCap" => eq!(get_spell_hit_cap(p), want_i()),
        "GMUD_IsLightArmourClass" => eq!(
            gmud_is_light_armour_class(int(a, 0) as i32, class_row(inp).as_ref()),
            r.as_bool().expect("bool result")
        ),
        "GMUD_DiminishingReturns" => eq!(
            gmud_diminishing_returns(dbl(a, 0, 0.0), dbl(a, 1, 0.0)),
            r.as_f64().expect("double result")
        ),
        other => Err(format!("no port wired for {other}")),
    }
}

/// Runs every accuracy fixture case under `profile`; the layer-1 identity harness reuses it.
pub(crate) fn check_fixture(profile: Profile) {
    let fx = fixture::load("accuracy");
    let mut bad = Vec::new();
    for case in &fx.cases {
        if let Err(msg) = run(profile, case) {
            bad.push(format!(
                "{}: {msg}\n   inputs {}",
                case["id"], case["inputs"]
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} cases differ; first:\n{}",
        bad.len(),
        fx.cases.len(),
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn accuracy_fixture_cases() {
    check_fixture(Profile::Stock);
}
