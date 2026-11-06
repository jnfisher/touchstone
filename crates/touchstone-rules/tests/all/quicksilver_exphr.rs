//! Exp/hr under Quicksilver (Q-014): HP and mana recover in one phase (rule R10).
//!
//! The engine prints no exp/hr figure, so `fixtures/quicksilver/exphr_d.json` holds cross-checks built
//! from its regeneration ticks (see the family's `about`). Model D's recovery helpers are checked
//! against them; the models, the dispatch and the Stock and neutral behaviour are checked directly.

use std::path::PathBuf;

use serde_json::Value;
use touchstone_rules::exp_per_hour::{
    ExpPerHourInput, ExpPerHourKnobs, ExpPerHourSettings, QS_MODEL_NOTICE, calc_exp_per_hour,
    model_a, model_b, model_c, model_d, qs_recovery_mana_per_sec, qs_recovery_secs,
};
use touchstone_rules::quicksilver::Balance;
use touchstone_rules::{Profile, RulesError};
use touchstone_vb6::Currency;

static NEUTRAL: Balance = Balance::NEUTRAL;

fn v120() -> Profile {
    Profile::quicksilver_v120()
}

fn neutral() -> Profile {
    Profile::Quicksilver(&NEUTRAL)
}

fn load() -> (Value, Vec<Value>) {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/quicksilver/exphr_d.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let mut doc: Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parsing {}: {e}", path.display()));
    let Value::Array(cases) = doc["cases"].take() else {
        panic!("no cases array");
    };
    (doc["header"].take(), cases)
}

fn num(v: &Value) -> f64 {
    v.as_f64()
        .unwrap_or_else(|| panic!("number expected, got {v}"))
}

fn of_kind<'a>(cases: &'a [Value], kind: &'a str) -> impl Iterator<Item = &'a Value> {
    cases.iter().filter(move |c| c["kind"] == kind)
}

fn report(what: &str, total: usize, bad: &[String]) {
    assert!(
        bad.is_empty(),
        "{what}: {} of {total} differ; first:\n{}",
        bad.len(),
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn exphr_header_and_ids() {
    let (h, cases) = load();
    assert_eq!(h["family"], "exphr_d");
    assert_eq!(h["content_version"], "1.20.0");
    assert_eq!(h["case_count"].as_u64().unwrap(), cases.len() as u64);
    let ids: Vec<&str> = cases.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids sorted and unique");
    assert!(of_kind(&cases, "cycle").count() > 0 && of_kind(&cases, "recovery").count() > 0);
    let v = Balance::V120;
    let n = Balance::NEUTRAL;
    assert_eq!(
        h["balance_keys"]["rest_mana_mult"]["v120"],
        v.rest_mana_mult
    );
    assert_eq!(
        h["balance_keys"]["meditate_mana_mult"]["v120"],
        v.meditate_mana_mult
    );
    assert_eq!(
        h["balance_keys"]["rest_mana_mult"]["neutral"],
        n.rest_mana_mult
    );
    assert_eq!(
        h["balance_keys"]["meditate_mana_mult"]["neutral"],
        n.meditate_mana_mult
    );
}

/// The 210-second cycle (30, 21 and 15 second ticks all divide it): mana is exact, so the rate
/// Model D uses times 210 is the engine's mana. The `neutral` column (multiples of 1) is the
/// standing rate alone, which is what stock Model D's passive mana is, so it screens the vector.
#[test]
fn cycle_mana_matches_engine() {
    let (_, cases) = load();
    let mut bad = Vec::new();
    for c in of_kind(&cases, "cycle") {
        let i = &c["inputs"];
        let standing = i32::try_from(i["standing"].as_i64().unwrap()).unwrap();
        let meditating = i["mode"] == "meditate";
        let id = c["id"].as_str().unwrap();
        for (column, balance) in [("v120", &Balance::V120), ("neutral", &Balance::NEUTRAL)] {
            let want = num(&c[column]["mana"]);
            let got = qs_recovery_mana_per_sec(balance, meditating, standing) * 210.0;
            if (got - want).abs() > 1e-6 {
                bad.push(format!("{id} {column}: want {want}, got {got}"));
            }
        }
        // stock Model D's passive mana in the same 210 seconds
        let stock = f64::from(standing) / 30.0 * 210.0;
        if (stock - num(&c["neutral"]["mana"])).abs() > 1e-6 {
            bad.push(format!(
                "{id}: neutral column is not the stock passive rate"
            ));
        }
    }
    report("cycle mana", of_kind(&cases, "cycle").count(), &bad);
}

/// Hit points: the engine heals on a 21-second rest tick; MME's Model D rests on a 20-second tick
/// (parity, docs/differences.md). So the vector pins the engine's own pieces, and the model's rate
/// is MME's composition of the same pieces.
#[test]
fn cycle_hit_points_follow_the_engine_pieces() {
    let (_, cases) = load();
    for c in of_kind(&cases, "cycle") {
        let i = &c["inputs"];
        let (rest, slow) = (num(&i["rest_hp"]), num(&i["slow_hp"]));
        // 10 rest ticks and 7 slow ticks in 210 seconds
        assert!(
            (num(&c["v120"]["hp"]) - (10.0 * rest + 7.0 * slow)).abs() < 1e-9,
            "{}",
            c["id"]
        );
        assert_eq!(c["v120"]["hp"], c["neutral"]["hp"], "{}", c["id"]);
    }
}

/// Seconds to restore both bars, against the engine's second-by-second run. The model is a rate,
/// the engine a set of ticks, so the allowance is one slow tick plus one rest tick (51 seconds) and
/// the 20- versus 21-second rest period (5 percent).
#[test]
fn recovery_matches_engine_within_the_tick_allowance() {
    let (_, cases) = load();
    let mut bad = Vec::new();
    let mut hp_bound = 0;
    let mut mp_bound = 0;
    for c in of_kind(&cases, "recovery") {
        let i = &c["inputs"];
        let standing = i32::try_from(i["standing"].as_i64().unwrap()).unwrap();
        let meditating = i["mode"] == "meditate";
        let rest_hp = num(&i["rest_hp"]);
        // MME's resting-form rate: the rest tick on a 20-second period plus the passive third
        let hp_ps = rest_hp / 20.0 + (rest_hp / 3.0) / 30.0;
        let id = c["id"].as_str().unwrap();
        for (column, balance) in [("v120", &Balance::V120), ("neutral", &Balance::NEUTRAL)] {
            let mp_ps = qs_recovery_mana_per_sec(balance, meditating, standing);
            let (hp, mp, phase) =
                qs_recovery_secs(num(&i["hp_missing"]), hp_ps, num(&i["mp_missing"]), mp_ps);
            if column == "v120" {
                if hp >= mp {
                    hp_bound += 1;
                } else {
                    mp_bound += 1;
                }
            }
            let want = num(&c[column]);
            if want > phase * 1.05 + 51.0 || want < phase - 51.0 - phase * 0.05 {
                bad.push(format!("{id} {column}: engine {want}, model {phase}"));
            }
        }
    }
    report("recovery", of_kind(&cases, "recovery").count(), &bad);
    assert!(
        hp_bound > 100 && mp_bound > 100,
        "both bars should limit sometimes"
    );
}

#[test]
fn recovery_helpers_edge_cases() {
    // nothing missing, or no rate: no time
    assert_eq!(qs_recovery_secs(0.0, 1.0, 0.0, 1.0), (0.0, 0.0, 0.0));
    assert_eq!(qs_recovery_secs(10.0, 0.0, 10.0, 0.0), (0.0, 0.0, 0.0));
    // the phase is the longer time, not the sum
    assert_eq!(qs_recovery_secs(10.0, 1.0, 40.0, 2.0), (10.0, 20.0, 20.0));
    // a negative standing rate pays nothing
    assert_eq!(qs_recovery_mana_per_sec(&Balance::V120, false, -5), 0.0);
    assert_eq!(qs_recovery_mana_per_sec(&Balance::V120, false, 30), 3.0);
    assert_eq!(qs_recovery_mana_per_sec(&Balance::V120, true, 30), 5.0);
}

fn lair_input() -> ExpPerHourInput {
    ExpPerHourInput {
        n_exp: Currency::from_scaled(500_000_000),
        n_regen_time: 10.0,
        n_num_mobs: 4.0,
        n_total_lairs: 5,
        n_poss_spawns: 5,
        n_rtk: 5.0,
        n_char_dmg: 400.0,
        n_char_hp: 600,
        n_char_hp_regen: 18,
        n_mob_dmg: 60.0,
        n_mob_hp: 2000,
        n_mob_hp_regen: 0,
        n_damage_threshold: 5,
        n_spell_cost: 3,
        n_spell_overhead: 0.0,
        n_char_mana: 100,
        n_char_mp_regen: 4,
        n_meditate_rate: 0,
        n_avg_walk: 20.0,
        n_walk_speed: 1.25,
        ..ExpPerHourInput::default()
    }
}

fn knobs() -> ExpPerHourKnobs {
    ExpPerHourKnobs::default()
}

#[test]
fn models_a_to_c_are_unsupported_under_quicksilver() {
    let input = lair_input();
    for result in [
        model_a(&input, &knobs(), v120()),
        model_b(&input, &knobs(), v120()),
        model_c(&input, &knobs(), v120()),
    ] {
        assert!(
            matches!(result, Err(RulesError::Unsupported(_))),
            "{result:?}"
        );
    }
    // the neutral balance reads as off: they run as stock
    for (neutral_run, stock_run) in [
        (
            model_a(&input, &knobs(), neutral()),
            model_a(&input, &knobs(), Profile::Stock),
        ),
        (
            model_b(&input, &knobs(), neutral()),
            model_b(&input, &knobs(), Profile::Stock),
        ),
        (
            model_c(&input, &knobs(), neutral()),
            model_c(&input, &knobs(), Profile::Stock),
        ),
    ] {
        assert_eq!(neutral_run, stock_run);
    }
}

#[test]
fn dispatch_runs_model_d_alone_and_says_so() {
    let input = lair_input();
    let all = ExpPerHourSettings::default();
    let only_d = ExpPerHourSettings {
        model_a: false,
        model_b: false,
        model_c: false,
        ..all
    };
    let qs = calc_exp_per_hour(&input, &all, v120()).expect("no error, only a notice");
    let alone = calc_exp_per_hour(&input, &only_d, v120()).unwrap();
    assert_eq!(qs.s_notice, QS_MODEL_NOTICE);
    assert!(
        alone.s_notice.is_empty(),
        "Model D chosen: nothing to explain"
    );
    // the same figures either way: Model D alone, and its own record
    let mut qs_figures = qs.clone();
    qs_figures.s_notice.clear();
    assert_eq!(qs_figures, alone);
    let d = model_d(&input, &knobs(), v120()).unwrap();
    assert_eq!(alone.n_exp_per_hour, d.n_exp_per_hour);
    assert!(alone.n_exp_per_hour > 0.0);

    // only A to C ticked: Model D still runs, with the notice
    let only_abc = ExpPerHourSettings {
        model_d: false,
        ..all
    };
    let abc = calc_exp_per_hour(&input, &only_abc, v120()).unwrap();
    assert_eq!(abc.s_notice, QS_MODEL_NOTICE);
    assert_eq!(abc.n_exp_per_hour, d.n_exp_per_hour);

    // no model ticked: the empty record, as in stock
    let none = ExpPerHourSettings {
        model_a: false,
        model_b: false,
        model_c: false,
        model_d: false,
        ..all
    };
    assert_eq!(
        calc_exp_per_hour(&input, &none, v120()).unwrap(),
        Default::default()
    );

    // Stock and neutral: the four models, no notice
    for p in [Profile::Stock, neutral()] {
        let r = calc_exp_per_hour(&input, &all, p).unwrap();
        assert!(r.s_notice.is_empty());
    }
    assert_eq!(
        calc_exp_per_hour(&input, &all, neutral()).unwrap(),
        calc_exp_per_hour(&input, &all, Profile::Stock).unwrap()
    );
}

/// Hit points alone: with no mana to recover, the Quicksilver phase is the stock rest.
#[test]
fn model_d_hit_point_only_is_stock() {
    let input = ExpPerHourInput {
        n_char_mana: 0,
        n_spell_cost: 0,
        ..lair_input()
    };
    let stock = model_d(&input, &knobs(), Profile::Stock).unwrap();
    let qs = model_d(&input, &knobs(), v120()).unwrap();
    assert!(stock.n_hitpoint_recovery > 0.0);
    assert_eq!(qs, stock);
}

/// Both bars drain: resting restores mana at the same time, so the recovery share is below the
/// stock one (HP rest, then mana at the passive rate), and the two shares add up to the phase.
#[test]
fn model_d_recovers_both_bars_together() {
    let input = lair_input();
    let stock = model_d(&input, &knobs(), Profile::Stock).unwrap();
    let qs = model_d(&input, &knobs(), v120()).unwrap();
    assert!(
        stock.n_hitpoint_recovery > 0.0 && stock.n_mana_recovery > 0.0,
        "{stock:?}"
    );
    assert!(
        qs.n_time_recovering < stock.n_time_recovering,
        "{qs:?} vs {stock:?}"
    );
    assert!(qs.n_exp_per_hour > stock.n_exp_per_hour);
    let sum = qs.n_hitpoint_recovery + qs.n_mana_recovery;
    assert!((sum - qs.n_time_recovering).abs() < 1e-9, "{qs:?}");
    // meditating (a meditation rate) restores more mana than resting: never slower
    let meditating = ExpPerHourInput {
        n_meditate_rate: 4,
        ..input
    };
    let med = model_d(&meditating, &knobs(), v120()).unwrap();
    assert!(med.n_time_recovering <= qs.n_time_recovering + 1e-9);
}

/// Mana alone (the damage threshold swallows every blow): the phase is the mana time, resting at
/// the rest multiple, so it is faster than stock's passive-only mana recovery.
#[test]
fn model_d_mana_only_uses_the_rest_multiple() {
    let input = ExpPerHourInput {
        n_damage_threshold: 100_000,
        ..lair_input()
    };
    let stock = model_d(&input, &knobs(), Profile::Stock).unwrap();
    let qs = model_d(&input, &knobs(), v120()).unwrap();
    assert_eq!(qs.n_hitpoint_recovery, 0.0);
    assert!(
        stock.n_mana_recovery > qs.n_mana_recovery,
        "{stock:?} {qs:?}"
    );
    assert!(qs.n_mana_recovery > 0.0);
}
