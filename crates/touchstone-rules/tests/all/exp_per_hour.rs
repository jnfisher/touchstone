// Fixture tests for the exp_per_hour family: see docs/porting.md step 5.
use serde_json::Value;
use touchstone_rules::exp_per_hour::{
    ExpPerHourInfo, ExpPerHourInput, ExpPerHourKnobs, ExpPerHourSettings, calc_exp_per_hour,
    clamp_knob, model_a, model_b, model_c, model_d,
};
use touchstone_rules::{Profile, RulesError};
use touchstone_vb6::Currency;

use std::path::PathBuf;

// serde_json (without its `float_roundtrip` feature) can misparse a Double by one ulp, and this
// family compares Doubles exactly. So numbers are lifted out as strings before parsing and
// converted with `str::parse`, which is exact.
fn load_exact(family: &str) -> Vec<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/rules")
        .join(format!("{family}.json"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let mut out = String::with_capacity(text.len() + text.len() / 4);
    let mut in_str = false;
    let mut esc = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_str {
            out.push(c);
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
        } else if c == '"' {
            in_str = true;
            out.push(c);
        } else if c == '-' || c.is_ascii_digit() {
            out.push('"');
            out.push(c);
            while let Some(&n) = chars.peek() {
                if n.is_ascii_digit() || matches!(n, '.' | 'e' | 'E' | '+' | '-') {
                    out.push(n);
                    chars.next();
                } else {
                    break;
                }
            }
            out.push('"');
        } else {
            out.push(c);
        }
    }
    let mut doc: Value =
        serde_json::from_str(&out).unwrap_or_else(|e| panic!("parsing {}: {e}", path.display()));
    match doc["cases"].take() {
        Value::Array(cases) => cases,
        _ => panic!("{} has no cases array", path.display()),
    }
}

fn f(v: &Value) -> f64 {
    v.as_str()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("not a number: {v}"))
}

fn i(v: &Value) -> i32 {
    v.as_str()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("not an integer: {v}"))
}

/// The oracle's positional arguments for ceph_ModelD (the CalcExpPerHour order).
fn input_of(args: &[Value]) -> ExpPerHourInput {
    assert_eq!(args.len(), 25, "ceph_ModelD takes 25 arguments");
    ExpPerHourInput {
        n_exp: Currency::from_f64(f(&args[0]["v"])).expect("Currency"),
        n_regen_time: f(&args[1]),
        n_num_mobs: f(&args[2]),
        n_total_lairs: i(&args[3]),
        n_poss_spawns: i(&args[4]),
        n_rtk: f(&args[5]),
        n_char_dmg: f(&args[6]),
        n_char_hp: i(&args[7]),
        n_char_hp_regen: i(&args[8]),
        n_mob_dmg: f(&args[9]),
        n_mob_hp: i(&args[10]),
        n_mob_hp_regen: i(&args[11]),
        n_damage_threshold: i(&args[12]),
        n_spell_cost: i(&args[13]) as i16,
        n_spell_overhead: f(&args[14]),
        n_char_mana: i(&args[15]),
        n_char_mp_regen: i(&args[16]),
        n_meditate_rate: i(&args[17]),
        n_avg_walk: f(&args[18]),
        n_walk_speed: f(&args[19]),
        n_surprise_dmg: f(&args[20]),
        n_surprise_min_dmg: f(&args[21]),
        n_surprise_chance: i(&args[22]) as i16,
        n_char_first_round_dmg: f(&args[23]),
        n_min_round_dmg: f(&args[24]),
    }
}

fn knobs_of(globals: &Value) -> ExpPerHourKnobs {
    ExpPerHourKnobs {
        xp: f(&globals["nGlobal_cephXP_Knob"]),
        dmg: f(&globals["nGlobal_cephDMG_Knob"]),
        mana: f(&globals["nGlobal_cephMana_Knob"]),
        mv: f(&globals["nGlobal_cephMove_Knob"]),
    }
}

/// Fixture keys are the lower-cased VB field names.
fn expected_of(r: &Value) -> ExpPerHourInfo {
    let s = |k: &str| {
        r[k].as_str()
            .unwrap_or_else(|| panic!("no {k}"))
            .to_string()
    };
    ExpPerHourInfo {
        n_exp_per_hour: f(&r["nexpperhour"]),
        n_hitpoint_recovery: f(&r["nhitpointrecovery"]),
        n_mana_recovery: f(&r["nmanarecovery"]),
        n_time_recovering: f(&r["ntimerecovering"]),
        n_overkill: f(&r["noverkill"]),
        n_move: f(&r["nmove"]),
        n_rtc: f(&r["nrtc"]),
        n_attack_time: f(&r["nattacktime"]),
        n_slowdown_time: f(&r["nslowdowntime"]),
        n_roam_time: f(&r["nroamtime"]),
        s_hitpoint_recovery: s("shitpointrecovery"),
        s_mana_recovery: s("smanarecovery"),
        s_time_recovering: s("stimerecovering"),
        s_rtc_text: s("srtctext"),
        s_move_text: s("smovetext"),
        s_exp_all: s("sexpall"),
        s_notice: String::new(),
    }
}

// Expected values come from fixtures/rules/exphr_d.json (oracle run of ceph_ModelD).
#[test]
fn model_d_matches_oracle() {
    check_model_d(Profile::Stock);
}

/// The whole fixture under `profile` (the identity harness runs it under the neutral Quicksilver balance).
pub(crate) fn check_model_d(profile: Profile) {
    let cases = load_exact("exphr_d");
    assert!(
        cases.len() >= 500,
        "scenario should have at least 500 cases"
    );
    let mut failures = Vec::new();
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        let inputs = &case["inputs"];
        let input = input_of(inputs["args"].as_array().unwrap());
        let knobs = knobs_of(&inputs["globals"]);
        let got = model_d(&input, &knobs, profile);
        if let Some(err) = case.get("error") {
            let want = RulesError::Vb6(i(&err["number"]));
            if got != Err(want.clone()) {
                failures.push(format!(
                    "{id}: want {want:?}, got {got:?}\n  inputs {inputs}"
                ));
            }
            continue;
        }
        let want = expected_of(&case["outputs"]["result"]);
        match got {
            Ok(g) if g == want => {}
            other => failures.push(format!(
                "{id}: want {want:?}\n  got {other:?}\n  inputs {inputs}"
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ; first:\n{}",
        failures.len(),
        cases.len(),
        failures
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Like `input_of`, but `None` when an `Integer` argument (nSpellCost, nSurpriseChance) is out of
/// range: VB6 raises error 6 when the call converts it.
fn input_of_checked(args: &[Value]) -> Option<ExpPerHourInput> {
    i16::try_from(i(&args[13])).ok()?;
    i16::try_from(i(&args[22])).ok()?;
    Some(input_of(args))
}

// Expected values come from fixtures/rules/exphr_c.json (oracle run of ceph_ModelC).
#[test]
fn model_c_matches_oracle() {
    check_model_c(Profile::Stock);
}

/// The whole fixture under `profile` (the identity harness runs it under the neutral Quicksilver balance).
pub(crate) fn check_model_c(profile: Profile) {
    let cases = load_exact("exphr_c");
    assert!(
        cases.len() >= 500,
        "scenario should have at least 500 cases"
    );
    let mut failures = Vec::new();
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        let inputs = &case["inputs"];
        let args = inputs["args"].as_array().unwrap();
        let knobs = knobs_of(&inputs["globals"]);
        let got = match input_of_checked(args) {
            Some(input) => model_c(&input, &knobs, profile),
            None => Err(RulesError::Vb6(6)),
        };
        if let Some(err) = case.get("error") {
            let want = RulesError::Vb6(i(&err["number"]));
            if got != Err(want.clone()) {
                failures.push(format!(
                    "{id}: want {want:?}, got {got:?}\n  inputs {inputs}"
                ));
            }
            continue;
        }
        let want = expected_of(&case["outputs"]["result"]);
        match got {
            Ok(g) if g == want => {}
            other => failures.push(format!(
                "{id}: want {want:?}\n  got {other:?}\n  inputs {inputs}"
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ; first:\n{}",
        failures.len(),
        cases.len(),
        failures
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The oracle's positional arguments for ceph_ModelA: the first 21 of the CalcExpPerHour order.
/// The remaining fields keep their defaults because Model A never reads them.
fn input_of_a(args: &[Value]) -> ExpPerHourInput {
    assert_eq!(args.len(), 21, "ceph_ModelA takes 21 arguments");
    ExpPerHourInput {
        n_exp: Currency::from_f64(f(&args[0]["v"])).expect("Currency"),
        n_regen_time: f(&args[1]),
        n_num_mobs: f(&args[2]),
        n_total_lairs: i(&args[3]),
        n_poss_spawns: i(&args[4]),
        n_rtk: f(&args[5]),
        n_char_dmg: f(&args[6]),
        n_char_hp: i(&args[7]),
        n_char_hp_regen: i(&args[8]),
        n_mob_dmg: f(&args[9]),
        n_mob_hp: i(&args[10]),
        n_mob_hp_regen: i(&args[11]),
        n_damage_threshold: i(&args[12]),
        n_spell_cost: i(&args[13]) as i16,
        n_spell_overhead: f(&args[14]),
        n_char_mana: i(&args[15]),
        n_char_mp_regen: i(&args[16]),
        n_meditate_rate: i(&args[17]),
        n_avg_walk: f(&args[18]),
        n_walk_speed: f(&args[19]),
        n_surprise_dmg: f(&args[20]),
        ..ExpPerHourInput::default()
    }
}

// Expected values come from fixtures/rules/exphr_a.json (oracle run of ceph_ModelA). Model A
// swallows its own VB6 errors, so an error case is an empty record, which the fixture also holds.
#[test]
fn model_a_matches_oracle() {
    check_model_a(Profile::Stock);
}

/// The whole fixture under `profile` (the identity harness runs it under the neutral Quicksilver balance).
pub(crate) fn check_model_a(profile: Profile) {
    let cases = load_exact("exphr_a");
    assert!(
        cases.len() >= 500,
        "scenario should have at least 500 cases"
    );
    let mut failures = Vec::new();
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        let inputs = &case["inputs"];
        assert!(case.get("error").is_none(), "{id}: error escaped Model A");
        let input = input_of_a(inputs["args"].as_array().unwrap());
        let knobs = knobs_of(&inputs["globals"]);
        let got = model_a(&input, &knobs, profile);
        let want = expected_of(&case["outputs"]["result"]);
        match got {
            Ok(g) if g == want => {}
            other => failures.push(format!(
                "{id}: want {want:?}\n  got {other:?}\n  inputs {inputs}"
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ; first:\n{}",
        failures.len(),
        cases.len(),
        failures
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The oracle's 21 positional arguments for ceph_ModelB. Model B takes none of the surprise
/// minimum, chance, first-round or minimum-round values, so those fields stay zero.
/// `None` when the `Integer` argument nSpellCost is out of range (VB6 error 6 at the call).
fn input_of_b(args: &[Value]) -> Option<ExpPerHourInput> {
    assert_eq!(args.len(), 21, "ceph_ModelB takes 21 arguments");
    i16::try_from(i(&args[13])).ok()?;
    let zero = Value::String("0".to_string());
    let mut full = args.to_vec();
    full.extend(std::iter::repeat_n(zero, 4));
    let mut input = input_of(&full);
    input.n_surprise_min_dmg = 0.0;
    Some(input)
}

// Expected values come from fixtures/rules/exphr_b.json (oracle run of ceph_ModelB).
#[test]
fn model_b_matches_oracle() {
    check_model_b(Profile::Stock);
}

/// The whole fixture under `profile` (the identity harness runs it under the neutral Quicksilver balance).
pub(crate) fn check_model_b(profile: Profile) {
    let cases = load_exact("exphr_b");
    assert!(
        cases.len() >= 500,
        "scenario should have at least 500 cases"
    );
    let mut failures = Vec::new();
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        let inputs = &case["inputs"];
        let args = inputs["args"].as_array().unwrap();
        let knobs = knobs_of(&inputs["globals"]);
        let got = match input_of_b(args) {
            Some(input) => model_b(&input, &knobs, profile),
            None => Err(RulesError::Vb6(6)),
        };
        if let Some(err) = case.get("error") {
            let want = RulesError::Vb6(i(&err["number"]));
            if got != Err(want.clone()) {
                failures.push(format!(
                    "{id}: want {want:?}, got {got:?}\n  inputs {inputs}"
                ));
            }
            continue;
        }
        let want = expected_of(&case["outputs"]["result"]);
        match got {
            Ok(g) if g == want => {}
            other => failures.push(format!(
                "{id}: want {want:?}\n  got {other:?}\n  inputs {inputs}"
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ; first:\n{}",
        failures.len(),
        cases.len(),
        failures
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

fn settings_of(globals: &Value) -> ExpPerHourSettings {
    let b = |k: &str| {
        globals[k]
            .as_bool()
            .unwrap_or_else(|| panic!("no flag {k}"))
    };
    ExpPerHourSettings {
        model_a: b("bGlobal_cephModelA"),
        model_b: b("bGlobal_cephModelB"),
        model_c: b("bGlobal_cephModelC"),
        model_d: b("bGlobal_cephModelD"),
        recovery_only: b("bGlobal_cephRecoveryOnly"),
        show_all: b("bGlobal_cephShowAll"),
        knobs: knobs_of(globals),
    }
}

// Expected values come from fixtures/rules/exp_dispatch.json (oracle run of CalcExpPerHour).
#[test]
fn calc_exp_per_hour_matches_oracle() {
    check_dispatch(Profile::Stock);
}

/// The whole fixture under `profile` (the identity harness runs it under the neutral Quicksilver balance).
pub(crate) fn check_dispatch(profile: Profile) {
    let cases = load_exact("exp_dispatch");
    assert!(
        cases.len() >= 400,
        "scenario should have at least 400 cases"
    );
    let mut failures = Vec::new();
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        let inputs = &case["inputs"];
        let args = inputs["args"].as_array().unwrap();
        let settings = settings_of(&inputs["globals"]);
        // Integer arguments (nSpellCost, nSurpriseChance) are converted at the call: error 6.
        let got = match input_of_checked(args) {
            Some(input) => calc_exp_per_hour(&input, &settings, profile),
            None => Err(RulesError::Vb6(6)),
        };
        if let Some(err) = case.get("error") {
            let want = RulesError::Vb6(i(&err["number"]));
            if got != Err(want.clone()) {
                failures.push(format!(
                    "{id}: want {want:?}, got {got:?}\n  inputs {inputs}"
                ));
            }
            continue;
        }
        let want = expected_of(&case["outputs"]["result"]);
        match got {
            Ok(g) if g == want => {}
            other => failures.push(format!(
                "{id}: want {want:?}\n  got {other:?}\n  inputs {inputs}"
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ; first:\n{}",
        failures.len(),
        cases.len(),
        failures
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// MME: Form_Load (A, B, C on, D off), then LoadExpPerHourKnobs with absent keys (all four on),
// and the settings dialog's none-ticked fallback (A, B, C).
#[test]
fn exp_per_hour_settings_defaults() {
    let d = ExpPerHourSettings::default();
    assert!(d.model_a && d.model_b && d.model_c && d.model_d);
    assert!(!d.recovery_only && !d.show_all);
    assert_eq!(d.knobs, ExpPerHourKnobs::default());
    let f = ExpPerHourSettings::at_form_load();
    assert!(f.model_a && f.model_b && f.model_c && !f.model_d);
    let none = ExpPerHourSettings {
        model_a: false,
        model_b: false,
        model_c: false,
        model_d: false,
        ..d
    };
    assert_eq!(none.normalized(), f);
    let only_d = ExpPerHourSettings {
        model_a: false,
        model_b: false,
        model_c: false,
        ..d
    };
    assert_eq!(only_d.normalized(), only_d);
}

// MME: LoadExpPerHourKnobs rounds each knob to 2 decimals and clamps it to 0.01..10.
#[test]
fn exp_per_hour_knob_clamp() {
    assert_eq!(clamp_knob(0.0), 0.01);
    assert_eq!(clamp_knob(0.004), 0.01);
    assert_eq!(clamp_knob(1.234), 1.23);
    assert_eq!(clamp_knob(10.4), 10.0);
    assert_eq!(clamp_knob(25.0), 10.0);
    assert_eq!(clamp_knob(-3.0), 0.01);
}
