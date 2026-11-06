//! Rest and meditate regeneration (rule R10), against the engine vectors in
//! `fixtures/quicksilver/regen.json` (written by `tools/qs-vectors`, Quicksilver's own regen
//! functions). The `v120` column is checked under `Profile::Quicksilver(V120)`. The `neutral` column
//! (both multiples at 1: resting and meditating add nothing) is what stock MME answers with no
//! meditation rate, so it screens the vectors against the stock functions and is also what the
//! Quicksilver profile gives at `Balance::NEUTRAL`.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::Value;
use touchstone_model::{Class, Info, World, WorldParts};
use touchstone_rules::Profile;
use touchstone_rules::char_panel::{HpInput, MagicInput, refresh_hit_points, refresh_magic};
use touchstone_rules::quicksilver::Balance;
use touchstone_rules::spell_cast::{calc_mana_recovery_rounds, calc_mana_recovery_rounds_qs};
use touchstone_rules::stats::{
    ManaRegenInput, QS_MEDITATE_TICK_SECS, QS_REST_TICK_SECS, calc_mana_regen,
    calc_meditate_mana_regen, calc_rest_mana_regen, calc_resting_rate, qs_boost_tick,
};

static NEUTRAL: Balance = Balance::NEUTRAL;

fn v120() -> Profile {
    Profile::quicksilver_v120()
}

fn neutral() -> Profile {
    Profile::Quicksilver(&NEUTRAL)
}

struct Vectors {
    header: Value,
    cases: Vec<Value>,
}

fn load() -> Vectors {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/quicksilver/regen.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let mut doc: Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parsing {}: {e}", path.display()));
    let Value::Array(cases) = doc["cases"].take() else {
        panic!("{} has no cases array", path.display());
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

fn ints(v: &Value) -> Vec<i32> {
    v.as_array().unwrap().iter().map(int).collect()
}

fn of_kind<'a>(v: &'a Vectors, kind: &'a str) -> impl Iterator<Item = &'a Value> {
    v.cases.iter().filter(move |c| c["kind"] == kind)
}

fn regen_input(i: &Value) -> ManaRegenInput {
    ManaRegenInput {
        level: int(&i["level"]),
        intellect: int(&i["int"]),
        willpower: int(&i["wil"]),
        charm: int(&i["cha"]),
        magic_level: int(&i["magic_level"]),
        magic_type: int(&i["magic_type"]),
        mp_regen: int(&i["mp_regen"]),
        meditating: false,
    }
}

fn whole(c: touchstone_vb6::Currency) -> i32 {
    assert_eq!(c.scaled() % 10_000, 0, "not a whole number: {c:?}");
    i32::try_from(c.scaled() / 10_000).unwrap()
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
fn regen_header_and_ids() {
    let v = load();
    assert_eq!(v.header["family"], "regen");
    assert_eq!(v.header["content_version"], "1.20.0");
    assert_eq!(
        v.header["case_count"].as_u64().unwrap(),
        v.cases.len() as u64
    );
    let ids: Vec<&str> = v.cases.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids sorted and unique");
    for kind in ["standing", "hp", "tick", "recovery"] {
        assert!(of_kind(&v, kind).count() > 0, "no {kind} cases");
    }
}

/// The header's multiples are the ones touchstone's `Balance` carries, so a retune of either side
/// fails here.
#[test]
fn regen_header_matches_balance() {
    let keys = &load().header["balance_keys"];
    let (v, n) = (Balance::V120, Balance::NEUTRAL);
    let pair = |k: &str| (int(&keys[k]["v120"]), int(&keys[k]["neutral"]));
    assert_eq!(
        pair("rest_mana_mult"),
        (v.rest_mana_mult as i32, n.rest_mana_mult as i32)
    );
    assert_eq!(
        pair("meditate_mana_mult"),
        (v.meditate_mana_mult as i32, n.meditate_mana_mult as i32)
    );
}

/// The standing rate is MME's CalcManaRegen under both profiles and equals the engine's. The
/// engine column is the same at 1.20.0 and neutral (the rule leaves the standing tick alone).
#[test]
fn standing_rate_matches_engine_and_stock() {
    let v = load();
    let mut bad = Vec::new();
    let mut total = 0;
    for c in of_kind(&v, "standing") {
        total += 1;
        let (want, neutral_col) = (int(&c["v120"]), int(&c["neutral"]));
        assert_eq!(want, neutral_col, "{}", c["id"]);
        let input = regen_input(&c["inputs"]);
        for (name, p) in [
            ("stock", Profile::Stock),
            ("v120", v120()),
            ("neutral", neutral()),
        ] {
            let got = whole(calc_mana_regen(p, &input));
            if got != want {
                bad.push(format!("{} {name}: got {got}, engine {want}", c["id"]));
            }
        }
    }
    report("standing", total, &bad);
}

/// Resting and meditating heal at MME's resting rate (CalcRestingRate with its resting flag).
#[test]
fn rest_hp_matches_engine_and_stock() {
    let v = load();
    let mut bad = Vec::new();
    let mut total = 0;
    for c in of_kind(&v, "hp") {
        total += 1;
        let i = &c["inputs"];
        let (level, hea, rate) = (int(&i["level"]), int(&i["health"]), int(&i["hp_regen"]));
        let want = int(&c["v120"]);
        assert_eq!(want, int(&c["neutral"]), "{}", c["id"]);
        for (name, p) in [
            ("stock", Profile::Stock),
            ("v120", v120()),
            ("neutral", neutral()),
        ] {
            let got = calc_resting_rate(p, level, hea, rate, true);
            if got != want {
                bad.push(format!("{} {name}: got {got}, engine {want}", c["id"]));
            }
        }
        if rate == 0 {
            let normal = calc_resting_rate(Profile::Stock, level, hea, 0, false);
            if normal != int(&c["base"]) {
                bad.push(format!(
                    "{} base: got {normal}, engine {}",
                    c["id"], c["base"]
                ));
            }
        }
    }
    report("hp", total, &bad);
}

fn tick_params(mode: &str, b: &Balance) -> (u32, i32) {
    match mode {
        "rest" => (b.rest_mana_mult, QS_REST_TICK_SECS),
        "meditate" => (b.meditate_mana_mult, QS_MEDITATE_TICK_SECS),
        other => panic!("mode {other}"),
    }
}

/// The carry arithmetic reproduces the engine's payments tick for tick.
#[test]
fn boost_ticks_match_engine_sequences() {
    let v = load();
    let mut bad = Vec::new();
    let mut total = 0;
    for c in of_kind(&v, "tick") {
        total += 1;
        let i = &c["inputs"];
        let (mode, standing) = (i["mode"].as_str().unwrap(), int(&i["standing"]));
        for (name, b, col) in [
            ("v120", &Balance::V120, "v120"),
            ("neutral", &NEUTRAL, "neutral"),
        ] {
            let (mult, period) = tick_params(mode, b);
            let mut carry = 0;
            let got: Vec<i32> = (0..30)
                .map(|_| qs_boost_tick(standing, mult, period, &mut carry))
                .collect();
            if got != ints(&c[col]) {
                bad.push(format!(
                    "{} {name}: got {got:?}, engine {}",
                    c["id"], c[col]
                ));
            }
            if carry != 0 {
                bad.push(format!(
                    "{} {name}: carry {carry} after a full cycle",
                    c["id"]
                ));
            }
        }
    }
    report("tick", total, &bad);
}

/// The panel's tick is the engine's cycle total over 30 ticks, divided evenly: exact in a
/// Currency. Checked for every caster whose standing rate has a tick vector.
#[test]
fn average_ticks_are_the_engine_cycle_total() {
    let v = load();
    let mut sums: HashMap<(String, i32), (i32, i32)> = HashMap::new();
    for c in of_kind(&v, "tick") {
        let i = &c["inputs"];
        let key = (i["mode"].as_str().unwrap().to_string(), int(&i["standing"]));
        sums.insert(
            key,
            (
                ints(&c["v120"]).iter().sum(),
                ints(&c["neutral"]).iter().sum(),
            ),
        );
    }
    let (mut checked, mut bad) = (0, Vec::new());
    for c in of_kind(&v, "standing") {
        let input = regen_input(&c["inputs"]);
        let standing = int(&c["v120"]);
        for (mode, f) in [
            (
                "rest",
                calc_rest_mana_regen as fn(Profile, &ManaRegenInput) -> _,
            ),
            ("meditate", calc_meditate_mana_regen),
        ] {
            let Some(&(sum_v, sum_n)) = sums.get(&(mode.to_string(), standing)) else {
                continue;
            };
            checked += 1;
            for (name, p, sum) in [("v120", v120(), sum_v), ("neutral", neutral(), sum_n)] {
                let got = f(p, &input).scaled() * 30;
                if got != i64::from(sum) * 10_000 {
                    bad.push(format!("{} {mode} {name}: {got} vs {sum} x 10000", c["id"]));
                }
            }
        }
    }
    assert!(checked > 100, "only {checked} cases reached the check");
    report("average tick", checked, &bad);
}

/// The stock rules: resting gives no mana, and the meditation tick is MME's, which takes no
/// regeneration buffs.
#[test]
fn stock_ticks_are_mme_s() {
    for magic_type in 0..=5 {
        let input = ManaRegenInput {
            level: 37,
            intellect: 88,
            willpower: 61,
            charm: 45,
            magic_level: 3,
            magic_type,
            mp_regen: 25,
            meditating: false,
        };
        assert_eq!(calc_rest_mana_regen(Profile::Stock, &input).scaled(), 0);
        let stock_medi = calc_mana_regen(
            Profile::Stock,
            &ManaRegenInput {
                mp_regen: 0,
                meditating: true,
                ..input
            },
        );
        assert_eq!(calc_meditate_mana_regen(Profile::Stock, &input), stock_medi);
    }
}

/// Rounds from no mana to the target, resting or meditating, against the engine's second-by-second
/// run. Neutral is the stock function with no meditation rate.
#[test]
fn recovery_rounds_match_engine() {
    let v = load();
    let mut bad = Vec::new();
    let mut total = 0;
    for c in of_kind(&v, "recovery") {
        total += 1;
        let i = &c["inputs"];
        let meditating = i["mode"] == "meditate";
        let standing = whole(calc_mana_regen(v120(), &regen_input(&i["caster"])));
        assert_eq!(standing, int(&i["standing"]), "{}", c["id"]);
        let target = int(&i["target"]);
        // the profile form reads "meditating" from the rate; any positive rate will do
        let rate = i32::from(meditating);
        let (want_v, want_n) = (int(&c["v120"]), int(&c["neutral"]));
        let got = [
            (
                "v120",
                calc_mana_recovery_rounds(v120(), target, standing, rate, 100),
                want_v,
            ),
            (
                "v120 qs",
                calc_mana_recovery_rounds_qs(&Balance::V120, meditating, target, standing, 100),
                want_v,
            ),
            (
                "neutral",
                calc_mana_recovery_rounds(neutral(), target, standing, rate, 100),
                want_n,
            ),
            // screening: stock MME, no meditation, is the engine at multiples of 1
            (
                "stock",
                calc_mana_recovery_rounds(Profile::Stock, target, standing, 0, 100),
                want_n,
            ),
        ];
        for (name, g, w) in got {
            if g != w {
                bad.push(format!("{} {name}: got {g}, engine {w}", c["id"]));
            }
        }
    }
    report("recovery", total, &bad);
}

/// Resting is the mode when there is no meditation rate; the percentage target works as in MME.
#[test]
fn recovery_mode_and_percentage() {
    // standing 3 a 30 s tick, 100 mana: expected from the vector family's tick cases (rest 4.2 a
    // 21 s tick, meditate 6 a 15 s tick), so the minute is 3x and 5x the standing 6.
    let rest = calc_mana_recovery_rounds(v120(), 100, 3, 0, 95);
    let medi = calc_mana_recovery_rounds(v120(), 100, 3, 1, 95);
    assert_eq!(
        rest,
        calc_mana_recovery_rounds_qs(&Balance::V120, false, 100, 3, 95)
    );
    assert_eq!(
        medi,
        calc_mana_recovery_rounds_qs(&Balance::V120, true, 100, 3, 95)
    );
    assert!(
        medi < rest,
        "meditating ({medi}) must be faster than resting ({rest})"
    );
    // 95 is the same as a target of 95 mana
    assert_eq!(rest, calc_mana_recovery_rounds(v120(), 95, 3, 0, 100));
    // nothing to recover, and a rate of zero that never gets there
    assert_eq!(calc_mana_recovery_rounds(v120(), 0, 3, 1, 95), 0);
    assert_eq!(calc_mana_recovery_rounds(v120(), 100, 0, 1, 95), 999);
}

fn panel_world() -> World {
    let classes = (1..=5)
        .map(|n| Class {
            number: n,
            name: String::new(),
            min_hits: 0,
            max_hits: 0,
            exp_table: 0,
            magery_type: n,
            magery_lvl: 3,
            weapon_type: 0,
            armour_type: 0,
            combat_lvl: 0,
            abilities: Default::default(),
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
            content_version: None,
        },
        items: vec![],
        monsters: vec![],
        spells: vec![],
        rooms: vec![],
        shops: vec![],
        classes,
        races: vec![],
        lairs: vec![],
        tbinfo: vec![],
    })
    .unwrap()
}

fn magic(
    profile: Profile,
    world: &World,
    class: i32,
    level: f64,
    regen_pct: f64,
) -> touchstone_rules::char_panel::Magic {
    refresh_magic(&MagicInput {
        profile,
        world,
        level,
        class,
        intellect: 60.0,
        willpower: 60.0,
        charm: 66.0,
        mana_regen_pct: regen_pct,
        mana_bonus: 0.0,
        spellcasting_bonus: 0.0,
    })
}

/// Expected: the vector family's tick cases for a standing rate of 3 (an L1 Mage with 60
/// Intellect and magery level 3): the 1.20 rest tick averages 4.2, the meditate tick 6.
#[test]
fn magic_panel_under_quicksilver() {
    let world = panel_world();
    let m = magic(v120(), &world, 1, 1.0, 0.0);
    assert_eq!(m.mana_regen, Some(3));
    assert_eq!(m.rest_ticks, Some(4.2));
    assert_eq!(m.medi_ticks, Some(6.0));
    assert_eq!(
        m.mana_regen_caption.as_deref(),
        Some("Mana Regen: 3\r\nRest Ticks: 4.2\r\nMedi. Ticks: 6")
    );
    // the regeneration buffs count towards both ticks: +50% makes the standing rate 4 (150 x 3 / 100)
    let m = magic(v120(), &world, 1, 1.0, 50.0);
    assert_eq!(m.mana_regen, Some(4));
    assert_eq!(m.rest_ticks, Some(5.6));
    assert_eq!(m.medi_ticks, Some(8.0));
}

/// Stock keeps MME's panel byte for byte; at neutral multiples the Quicksilver tick is zero.
#[test]
fn magic_panel_stock_and_neutral() {
    let world = panel_world();
    for class in 1..=5 {
        for pct in [0.0, 50.0] {
            let s = magic(Profile::Stock, &world, class, 30.0, pct);
            assert_eq!(s.rest_ticks, None);
            let want = ManaRegenInput {
                level: 30,
                intellect: 60,
                willpower: 60,
                charm: 66,
                magic_level: 3,
                magic_type: class,
                mp_regen: 0,
                meditating: true,
            };
            assert_eq!(
                s.medi_ticks,
                Some(calc_mana_regen(Profile::Stock, &want).to_f64())
            );
            assert!(!s.mana_regen_caption.as_deref().unwrap().contains("Rest"));
            // the standing rate and the other lines do not depend on the profile
            for p in [v120(), neutral()] {
                let q = magic(p, &world, class, 30.0, pct);
                assert_eq!(q.mana_regen, s.mana_regen);
                assert_eq!(q.max_mana, s.max_mana);
                assert_eq!(q.spellcasting, s.spellcasting);
            }
            let n = magic(neutral(), &world, class, 30.0, pct);
            assert_eq!((n.rest_ticks, n.medi_ticks), (Some(0.0), Some(0.0)));
        }
    }
}

/// Rest keeps its hit points, and meditating heals as resting does: the hit point panel is the
/// stock one under Quicksilver.
#[test]
fn hit_point_panel_is_stock() {
    let world = panel_world();
    let input = HpInput {
        world: &world,
        level: 30.0,
        class: 1,
        health: 70.0,
        hp_regen: 25.0,
        hp_bonus: 0.0,
    };
    let stock = refresh_hit_points(Profile::Stock, &input);
    assert_eq!(refresh_hit_points(v120(), &input), stock);
    assert_eq!(refresh_hit_points(neutral(), &input), stock);
}
