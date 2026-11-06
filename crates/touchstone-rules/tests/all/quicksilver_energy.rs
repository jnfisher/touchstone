//! Q-017: Alter Energy Usage (ability 87) under Quicksilver. `fixtures/quicksilver/energy.json`
//! (written by `tools/qs-vectors`, family `energy`) holds made-up characters whose bless spells,
//! race, class, worn items, weapon and a carried item carry the ability, with the engine's total
//! and the energy a normal blow costs with it. The total is a running average over the sources, so
//! the order of the sources matters; this rebuilds each character through `refresh_all` and checks
//! the total, then the swings `calculate_attack` gives. Stock and the neutral profile ignore the
//! ability, as MME does.

use std::path::PathBuf;

use serde_json::{Value, json};
use touchstone_model::World;
use touchstone_rules::Profile;
use touchstone_rules::character::Character;
use touchstone_rules::equip::{EquipOptions, equip_item};
use touchstone_rules::gear::ItemManagerRow;
use touchstone_rules::pipeline::{PipelineOptions, refresh_all};
use touchstone_rules::quicksilver::Balance;

use super::quicksilver_cells::world;
use super::quicksilver_melee::{Rig, hero, request};

static NEUTRAL: Balance = Balance::NEUTRAL;

fn load() -> (Value, Vec<Value>) {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/quicksilver/energy.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let mut doc: Value = serde_json::from_str(&text).unwrap();
    let Value::Array(cases) = doc["cases"].take() else {
        panic!("no cases array");
    };
    (doc["header"].take(), cases)
}

fn int(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}

/// The rows of a case's character in the cells fixture's layout. Worn codes and item types are the
/// engine's, which the cells already rely on.
fn tables(sources: &[(String, i32)]) -> Value {
    let find = |name: &str| sources.iter().find(|(n, _)| n == name).map(|(_, v)| *v);
    let abil = |name: &str| find(name).map_or(json!([]), |v| json!([[87, v]]));
    let spell = |n: i32, name: &str| {
        json!([
            n,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            10,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            abil(name),
            false
        ])
    };
    let item = |n: i32, name: &str, kind: i32, worn: i32| {
        json!([
            n,
            0,
            kind,
            0,
            0,
            0,
            0,
            worn,
            0,
            0,
            0,
            0,
            0,
            [],
            [],
            [],
            abil(name)
        ])
    };
    json!({
        "items": [
            item(2001, "head", 0, 2),
            item(2002, "torso", 0, 11),
            item(2003, "finger", 0, 4),
            item(2004, "weapon", 1, 0),
            item(2005, "carried", 10, 0),
        ],
        "spells": [spell(1001, "spell_a"), spell(1002, "spell_b")],
        "classes": [json!([1, 0, 0, 0, 0, 0, 0, 0, 3, abil("class")])],
        "races": [json!([
            1, [0, 0, 0, 0, 0, 0], [100, 100, 100, 100, 100, 100], 0, 0, abil("race")
        ])],
        "monsters": [],
    })
}

fn sources(c: &Value) -> Vec<(String, i32)> {
    c["inputs"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p[0].as_str().unwrap().to_string(), int(&p[1])))
        .collect()
}

/// What `refresh_all` leaves for a case under `profile`.
fn aggregate(w: &World, c: &Value, profile: Profile) -> i32 {
    let srcs = sources(c);
    let has = |n: &str| srcs.iter().any(|(s, _)| s == n);
    let mut ch = Character {
        race: Some(1),
        class: Some(1),
        level: 10,
        alignment: 1,
        ..Character::default()
    };
    // the slot order head, torso, finger, weapon is the engine's worn order
    for n in [2001, 2002, 2003, 2004] {
        ch.equipped = equip_item(w, &ch.equipped, n, &EquipOptions::default()).equipped;
    }
    ch.bless[0] = 1001;
    ch.bless[1] = 1002;
    let opts = PipelineOptions {
        item_manager: has("carried")
            .then(|| ItemManagerRow {
                text: "2005".to_string(),
                sub_items: vec![String::new(), "CARRIED".to_string()],
            })
            .into_iter()
            .collect(),
        ..PipelineOptions::default()
    };
    refresh_all(profile, w, &ch, &opts)
        .unwrap()
        .stats
        .energy_use
}

#[test]
fn energy_header_and_shape() {
    let (header, cases) = load();
    assert_eq!(header["family"], "energy");
    assert_eq!(header["case_count"].as_u64().unwrap(), cases.len() as u64);
    assert!(cases.len() >= 200, "{} cases", cases.len());
    // an order-dependent total is exercised: some case has a total that is not any single value
    assert!(cases.iter().any(|c| {
        let vals: Vec<i32> = sources(c).iter().map(|(_, v)| *v).collect();
        vals.len() >= 3 && !vals.contains(&int(&c["fold"]))
    }));
}

/// The total over the sources, in the engine's order (bless spells, race, class, worn items, the
/// weapon, the carried item), equals the engine's fold.
#[test]
fn aggregate_matches_the_engine_fold() {
    let (_, cases) = load();
    let mut bad = Vec::new();
    for c in &cases {
        let w = world(&tables(&sources(c)));
        let got = aggregate(&w, c, Profile::quicksilver_v120());
        if got != int(&c["fold"]) {
            bad.push(format!("{}: got {got}, engine {}", c["id"], c["fold"]));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Stock and the neutral profile ignore the ability (MME has no such stat).
#[test]
fn stock_and_neutral_ignore_the_ability() {
    let (_, cases) = load();
    for c in cases.iter().step_by(7) {
        let w = world(&tables(&sources(c)));
        assert_eq!(aggregate(&w, c, Profile::Stock), 0, "{}", c["id"]);
    }
}

/// A normal blow's energy with the total is the total's percentage of its energy without it,
/// truncated (the engine's within one), in both the engine (floored at 200, which MME has not and the five-swing cap
/// reproduces) and touchstone. The two bases differ a little (MME's formula rounds on its own
/// terms), so the percentage is compared, not the absolute cost. Stock and the neutral profile
/// give the swings without the ability.
#[test]
fn swings_follow_the_energy_the_engine_charges() {
    let (_, cases) = load();
    let rig = Rig::new(2, 12);
    let mut bad = Vec::new();
    let mut moved = 0;
    for c in &cases {
        let fold = int(&c["fold"]);
        for e in c["energy"].as_array().unwrap() {
            let level = int(&e["level"]);
            let (base, with) = (int(&e["base"]), int(&e["with"]));
            // the engine: the percentage of the base, floored at 200; its later steps (the encumbrance
            // factor) truncate on their own terms, so the cost can sit one off
            let engine = if fold > 0 { base * fold / 100 } else { base };
            if (with - engine.max(200)).abs() > 1 {
                bad.push(format!(
                    "{} L{level}: vector {with}, expected {}",
                    c["id"],
                    engine.max(200)
                ));
            }
            // a negative encumbrance skips MME's encumbrance step, which the engine's cost has not
            let mut plain = hero(1, level);
            plain.encum_pct = -1;
            let mut with_ability = plain.clone();
            with_ability.energy_pct = fold;
            let req = request(5, 1);
            let q = Profile::quicksilver_v120();
            let energy_of =
                |profile: Profile, ch: &_| 1000.0 / rig.attack(profile, ch, &req).swings;
            let (tb, tw) = (energy_of(q, &plain), energy_of(q, &with_ability));
            let want = if fold > 0 {
                (tb * f64::from(fold) / 100.0).trunc()
            } else {
                tb
            };
            // swings are rounded to four places and capped at five (energy 200)
            if (tw - want.max(200.0)).abs() > 2.0 {
                bad.push(format!(
                    "{} L{level}: touchstone energy {tw:.1}, expected {want:.1}",
                    c["id"]
                ));
            }
            if with != base {
                moved += 1;
            }
            for (name, profile) in [
                ("stock", Profile::Stock),
                ("neutral", Profile::Quicksilver(&NEUTRAL)),
            ] {
                if energy_of(profile, &with_ability) != energy_of(profile, &plain) {
                    bad.push(format!(
                        "{name} {} L{level}: the ability changed the swings",
                        c["id"]
                    ));
                }
            }
        }
    }
    assert!(moved > 100, "{moved} cases move the cost");
    assert!(
        bad.is_empty(),
        "{}",
        bad.iter().take(20).cloned().collect::<Vec<_>>().join("\n")
    );
}
