//! Q-011: the class melee share (R5) and the Thief opportunist (R4) against the engine vectors in
//! `fixtures/quicksilver/melee.json`, written by `tools/qs-vectors`.
//!
//! `share` cases give a damage range scaled by a class's percentage, `fighter` cases the range a
//! landed blow deals for a character (through `calculate_attack`), `opening` cases the opportunist
//! bonus range and its exact mean over the whole roll space. Columns: `v120`, `neutral` (class shares
//! 100 and no opportunist, which `Stock` and `Balance::NEUTRAL` must reproduce) and `alt` (off-grid).

use std::path::PathBuf;

use serde_json::Value;
use touchstone_model::{AbilitySlot, Item};
use touchstone_rules::Profile;
use touchstone_rules::attack::{
    AttackDamage, AttackEnv, AttackRequest, CastText, CharacterProfile, EquippedGlobals, THIEF,
    calculate_attack, opportunist_mean, opportunist_range,
};
use touchstone_rules::quicksilver::{Anchors, Balance, ClassMeleePct, LevelPct};

/// The vectors' `alt` balance (see the header's `about`).
static ALT: Balance = Balance {
    class_melee_pct: ClassMeleePct {
        warrior: LevelPct::Points(Anchors::new([(10, 80), (30, 135), (50, 90)])),
        thief: LevelPct::Points(Anchors::new([(5, 120), (45, 33)])),
        mage: LevelPct::One(37),
        ranger: LevelPct::Points(Anchors::new([(1, 10), (2, 11), (3, 13), (90, 250)])),
        ..ClassMeleePct::even(101)
    },
    opportunist_pct: 35,
    opportunist_level_pct: 120,
    backstab_stealth_div: 5,
    scalecap: 40,
    ..Balance::V120
};

/// The neutral balance of the vectors is the crate's: class shares 100, no opportunist, the stab keys
/// at their stock values.
static NEUTRAL: Balance = Balance::NEUTRAL;

/// 1.20.0 with the class shares at 100, so only the opportunist moves an answer.
static OPP_ONLY: Balance = Balance {
    class_melee_pct: ClassMeleePct::even(100),
    ..Balance::V120
};

/// `OPP_ONLY` with the chance at 0.
static OPP_OFF: Balance = Balance {
    opportunist_pct: 0,
    ..OPP_ONLY
};

/// 1.20.0 with no opportunist, so only the class share moves an answer.
static SHARE_ONLY: Balance = Balance {
    opportunist_pct: 0,
    ..Balance::V120
};

fn load() -> (Value, Vec<Value>) {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/quicksilver/melee.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let mut doc: Value = serde_json::from_str(&text).unwrap();
    let Value::Array(cases) = doc["cases"].take() else {
        panic!("no cases array");
    };
    (doc["header"].take(), cases)
}

fn int(v: &Value) -> i32 {
    i32::try_from(
        v.as_i64()
            .unwrap_or_else(|| panic!("integer expected, got {v}")),
    )
    .unwrap()
}

fn ints(v: &Value) -> Vec<i64> {
    v.as_array()
        .unwrap_or_else(|| panic!("array expected, got {v}"))
        .iter()
        .map(|x| x.as_i64().unwrap())
        .collect()
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
fn melee_header_and_ids() {
    let (header, cases) = load();
    assert_eq!(header["family"], "melee");
    assert_eq!(header["content_version"], "1.20.0");
    assert_eq!(header["case_count"].as_u64().unwrap(), cases.len() as u64);
    let ids: Vec<&str> = cases.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids sorted and unique");
    for kind in ["share", "fighter", "opening"] {
        assert!(cases.iter().any(|c| c["kind"] == kind), "no {kind} cases");
    }
    // The vectors' balances are the ones this crate carries.
    let keys = &header["balance_keys"];
    for (key, v, n, a) in [
        (
            "opportunist_pct",
            Balance::V120.opportunist_pct,
            NEUTRAL.opportunist_pct,
            ALT.opportunist_pct,
        ),
        (
            "opportunist_level_pct",
            Balance::V120.opportunist_level_pct,
            NEUTRAL.opportunist_level_pct,
            ALT.opportunist_level_pct,
        ),
        (
            "backstab_stealth_div",
            Balance::V120.backstab_stealth_div,
            NEUTRAL.backstab_stealth_div,
            ALT.backstab_stealth_div,
        ),
        (
            "scalecap",
            Balance::V120.scalecap,
            NEUTRAL.scalecap,
            ALT.scalecap,
        ),
    ] {
        assert_eq!(int(&keys[key]["v120"]), v as i32, "{key} v120");
        assert_eq!(int(&keys[key]["neutral"]), n as i32, "{key} neutral");
        assert_eq!(int(&keys[key]["alt"]), a as i32, "{key} alt");
    }
}

/// `class_melee_pct.scale` is the engine's, in every column.
#[test]
fn share_cases_match_the_balance() {
    let (_, cases) = load();
    let mut bad = Vec::new();
    let mut n = 0;
    for c in cases.iter().filter(|c| c["kind"] == "share") {
        n += 1;
        let i = &c["inputs"];
        let (class, level) = (int(&i["class"]) as u16, int(&i["level"]) as u16);
        let (min, max) = (int(&i["min"]), int(&i["max"]));
        for (col, b) in [
            ("v120", &Balance::V120),
            ("alt", &ALT),
            ("neutral", &NEUTRAL),
        ] {
            let got = vec![
                i64::from(b.class_melee_pct.scale(class, level, min)),
                i64::from(b.class_melee_pct.scale(class, level, max)),
            ];
            if got != ints(&c[col]) {
                bad.push(format!("{} {col}: got {got:?}, vector {}", c["id"], c[col]));
            }
        }
    }
    assert!(n > 1000);
    report("share", n, &bad);
}

/// The weapon text is never asked: the test weapon has no cast ability.
struct NoCasts;

impl CastText for NoCasts {
    fn spell_name(&self, n: i32) -> String {
        panic!("cast text asked for spell {n}")
    }
    fn spell_eq(&self, n: i32, _: i16) -> String {
        panic!("cast text asked for spell {n}")
    }
}

fn weapon(min: i32, max: i32, speed: i32) -> Item {
    Item {
        number: 1,
        name: "test weapon".into(),
        limit: 0,
        encum: 0,
        item_type: 1,
        use_count: 0,
        price: 0,
        currency: 0,
        min,
        max,
        armour_class: 0,
        damage_resist: 0,
        weapon_type: 0,
        armour_type: 0,
        worn: 0,
        accy: 0,
        gettable: true,
        str_req: 0,
        speed,
        not_droppable: false,
        destroy_on_death: false,
        retain_after_uses: false,
        class_rest: [0; 10],
        race_rest: [0; 10],
        negate_spell: [0; 10],
        abilities: [AbilitySlot::default(); 20],
        in_game: true,
        obtained_from: None,
        references: None,
    }
}

struct Rig {
    items: [Item; 1],
    equipped: EquippedGlobals,
}

impl Rig {
    fn new(min: i32, max: i32) -> Self {
        Self {
            items: [weapon(min, max, 1200)],
            equipped: EquippedGlobals::default(),
        }
    }

    fn env(&self) -> AttackEnv<'_> {
        AttackEnv {
            items: &self.items,
            classes: &[],
            races: &[],
            equipped: &self.equipped,
            cast_text: &NoCasts,
            dat_ver_caption: "",
            avg_level_max_all_stats: 0.0,
            accuracy_tag: "",
        }
    }

    fn attack(&self, profile: Profile, ch: &CharacterProfile, req: &AttackRequest) -> AttackDamage {
        calculate_attack(profile, &self.env(), ch, req, &mut String::new())
    }
}

fn hero(class: i32, level: i32) -> CharacterProfile {
    CharacterProfile {
        level,
        class,
        strength: 50,
        agility: 60,
        stealth: 100,
        class_stealth: true,
        combat: 3,
        ..CharacterProfile::default()
    }
}

fn request(attack_type: i32, weapon_number: i32) -> AttackRequest {
    AttackRequest {
        attack_type,
        weapon_number,
        ..AttackRequest::default()
    }
}

/// The blow of a `fighter` case through `calculate_attack`: its damage range.
fn fighter_range(profile: Profile, c: &Value) -> Vec<i64> {
    let i = &c["inputs"];
    let (wmin, wmax) = (int(&i["weapon_min"]), int(&i["weapon_max"]));
    let rig = Rig::new(wmin, wmax);
    let mut ch = hero(int(&i["class"]), int(&i["level"]));
    ch.plus_max_damage = int(&i["plus_max"]) as i16;
    let (attack_type, weapon) = match i["move"].as_str().unwrap() {
        "attack" => (5, 1),
        "bash" => (6, 1),
        "smash" => (7, 1),
        "punch" => (1, 0),
        "kick" => (2, 0),
        "jumpkick" => (3, 0),
        other => panic!("unknown move {other}"),
    };
    if attack_type <= 3 {
        let k = (attack_type - 1) as usize;
        ch.ma_plus_skill[k] = int(&i["skill"]) as i16;
        ch.ma_plus_damage[k] = int(&i["ma_dmg"]) as i16;
    }
    let hit = rig.attack(profile, &ch, &request(attack_type, weapon));
    vec![i64::from(hit.min_dmg), i64::from(hit.max_dmg)]
}

/// R5 end to end: the damage range of every non-stab blow under Quicksilver equals the engine's.
#[test]
fn fighter_ranges_match_quicksilver_profiles() {
    let (_, cases) = load();
    let mut bad = Vec::new();
    let mut n = 0;
    for c in cases.iter().filter(|c| c["kind"] == "fighter") {
        n += 1;
        for (col, b) in [("v120", &Balance::V120), ("alt", &ALT)] {
            let got = fighter_range(Profile::Quicksilver(b), c);
            if got != ints(&c[col]) {
                bad.push(format!("{} {col}: got {got:?}, vector {}", c["id"], c[col]));
            }
        }
    }
    assert!(n > 500);
    report("fighter", n, &bad);
}

/// The neutral column is the engine with the share off: both the stock path and the NEUTRAL profile
/// give it, which also screens the vectors against MME's own damage steps.
#[test]
fn fighter_neutral_column_matches_stock_and_neutral_profiles() {
    let (_, cases) = load();
    let mut bad = Vec::new();
    let mut n = 0;
    for c in cases.iter().filter(|c| c["kind"] == "fighter") {
        n += 1;
        for (name, profile) in [
            ("stock", Profile::Stock),
            ("neutral", Profile::Quicksilver(&NEUTRAL)),
        ] {
            let got = fighter_range(profile, c);
            if got != ints(&c["neutral"]) {
                bad.push(format!(
                    "{} {name}: got {got:?}, vector {}",
                    c["id"], c["neutral"]
                ));
            }
        }
    }
    report("fighter neutral", n, &bad);
}

/// The vectors must reach the rules, or the V120 tests prove nothing.
#[test]
fn melee_vectors_reach_each_rule() {
    let (_, cases) = load();
    let moved = |kind: &str| {
        cases
            .iter()
            .filter(|c| c["kind"] == kind && c["v120"] != c["neutral"])
            .count()
    };
    assert!(moved("share") > 800);
    assert!(moved("fighter") > 400);
    assert!(moved("opening") > 200);
}

/// R4 building blocks: the bonus range and the exact mean over the engine's whole roll space.
#[test]
fn opening_cases_match_the_range_and_mean() {
    let (_, cases) = load();
    let mut bad = Vec::new();
    let mut n = 0;
    for c in cases.iter().filter(|c| c["kind"] == "opening") {
        n += 1;
        let i = &c["inputs"];
        let thief = int(&i["class"]) == THIEF;
        for (col, b) in [
            ("v120", &Balance::V120),
            ("alt", &ALT),
            ("neutral", &NEUTRAL),
        ] {
            let v = ints(&c[col]);
            let range = opportunist_range(
                b,
                int(&i["level"]),
                int(&i["stealth"]),
                int(&i["bs_min"]),
                int(&i["bs_max"]),
            );
            if (i64::from(range.0), i64::from(range.1)) != (v[0], v[1]) {
                bad.push(format!("{} {col}: range {range:?}, vector {v:?}", c["id"]));
                continue;
            }
            // The engine's sum over every (percentile, damage) roll is chance x mean x rolls.
            let want = if thief {
                f64::from(b.opportunist_pct.min(100)) / 100.0
                    * opportunist_mean(range, int(&i["dr"]))
            } else {
                0.0
            };
            let engine = v[2] as f64 / v[3] as f64;
            if (want - engine).abs() > 1e-9 {
                bad.push(format!("{} {col}: mean {want}, engine {engine}", c["id"]));
            }
        }
    }
    assert!(n > 400);
    report("opening", n, &bad);
}

/// R4 end to end: the Thief's expected bonus per landed blow enters `round_total` and
/// `first_round_damage` as the swings times the chance times the mean, and nothing else moves.
#[test]
fn opportunist_enters_the_round_through_calculate_attack() {
    let (_, cases) = load();
    let profile = Profile::Quicksilver(&OPP_ONLY);
    let off = Profile::Quicksilver(&OPP_OFF);
    let mut checked = 0;
    let mut bad = Vec::new();
    for c in cases
        .iter()
        // a stealth of 0 is MME's "not given": it works the stealth out from the class instead
        .filter(|c| {
            c["kind"] == "opening"
                && int(&c["inputs"]["class"]) == THIEF
                && int(&c["inputs"]["stealth"]) > 0
        })
        .step_by(5)
    {
        let i = &c["inputs"];
        let rig = Rig::new(20, 30);
        let mut ch = hero(THIEF, int(&i["level"]));
        ch.stealth = int(&i["stealth"]) as i16;
        ch.plus_bs_min_damage = int(&i["bs_min"]) as i16;
        ch.plus_bs_max_damage = int(&i["bs_max"]) as i16;
        let dr = int(&i["dr"]);
        for attack_type in [5, 6, 7] {
            let req = AttackRequest {
                vs_dr: dr,
                ..request(attack_type, 1)
            };
            let on = rig.attack(profile, &ch, &req);
            let base = rig.attack(off, &ch, &req);
            let v = ints(&c["v120"]);
            // The engine: chance x mean over all rolls; touchstone also needs the blow to deal damage.
            let engine_per_proc = v[2] as f64 / v[3] as f64;
            let landed = (20..=30).filter(|d| *d > dr).count() as f64 / 11.0;
            let expect = engine_per_proc * landed;
            // bash and smash are slower or quicker; only the blows' rate differs
            let swings = base.swings;
            let diff = f64::from(on.round_total - base.round_total);
            if (diff - expect * swings).abs() > 1.0 {
                bad.push(format!(
                    "{} type {attack_type}: round diff {diff}, expected {}",
                    c["id"],
                    expect * swings
                ));
            }
            let whole = swings.trunc();
            let first = f64::from(on.first_round_damage - base.first_round_damage);
            if (first - expect * whole).abs() > 1.0 {
                bad.push(format!(
                    "{} type {attack_type}: first round diff {first}, expected {}",
                    c["id"],
                    expect * whole
                ));
            }
            assert_eq!(on.min_round_damage, base.min_round_damage, "{}", c["id"]);
            assert_eq!(on.round_physical, base.round_physical, "{}", c["id"]);
            assert_eq!(on.opportunist_pct, 10);
            assert_eq!(base.opportunist_pct, 0);
            checked += 1;
        }
    }
    assert!(checked > 60, "{checked}");
    report("opportunist round", checked, &bad);
}

/// Only a Thief, only under a profile with a chance, never on a stab, a manual figure or the
/// max-level mock.
#[test]
fn opportunist_needs_a_thief_a_chance_and_a_normal_blow() {
    let rig = Rig::new(20, 30);
    let v120 = Profile::quicksilver_v120();
    let thief = hero(THIEF, 30);
    let normal = request(5, 1);
    let hit = rig.attack(v120, &thief, &normal);
    assert_eq!(hit.opportunist_pct, 10);
    assert!(hit.opportunist_avg > 0);
    assert!(hit.attack_detail.contains("Opportunist: 10% for "));

    for class in (1..=15).filter(|c| *c != THIEF) {
        let h = rig.attack(v120, &hero(class, 30), &normal);
        assert_eq!(
            (h.opportunist_pct, h.opportunist_avg),
            (0, 0),
            "class {class}"
        );
        assert!(!h.attack_detail.contains("Opportunist"));
    }
    for profile in [Profile::Stock, Profile::Quicksilver(&NEUTRAL)] {
        let h = rig.attack(profile, &thief, &normal);
        assert_eq!((h.opportunist_pct, h.opportunist_avg), (0, 0));
    }
    // a stab has its own damage and never an opening
    let stab = rig.attack(v120, &thief, &request(4, 1));
    assert_eq!(stab.opportunist_pct, 0);
    // a manual figure has no class
    let manual = AttackRequest {
        specify_damage: 50.0,
        ..normal
    };
    let m = rig.attack(v120, &thief, &manual);
    assert_eq!(m.opportunist_pct, 0);
    assert_eq!((m.min_dmg, m.max_dmg), (50, 50));
    // the max-level mock (level 0) has no class
    let mock = rig.attack(v120, &hero(THIEF, 0), &normal);
    assert_eq!(mock.opportunist_pct, 0);
}

/// R5 end to end for the class share on a plain blow, and the cases that must stay whole: the stab,
/// a manual figure, a profile with no class and the max-level mock.
#[test]
fn class_share_applies_to_every_blow_but_the_stab() {
    let rig = Rig::new(20, 30);
    let v120 = Profile::quicksilver_v120();
    let normal = request(5, 1);
    let at = |class, level, profile, req: &AttackRequest| {
        let h = rig.attack(profile, &hero(class, level), req);
        (h.min_dmg, h.max_dmg)
    };
    // a Witchunter deals 114%, a Cleric 70% of the weapon's range
    assert_eq!(at(2, 30, v120, &normal), (22, 34));
    assert_eq!(at(4, 30, v120, &normal), (14, 21));
    // a Mystic's share runs from 50 to 77 percent between level 40 and 60
    assert_eq!(at(15, 40, v120, &normal), (10, 15));
    assert_eq!(at(15, 50, v120, &normal), (12, 19));
    assert_eq!(at(15, 60, v120, &normal), (15, 23));
    // other levels and Stock leave the weapon alone
    assert_eq!(at(1, 30, v120, &normal), (20, 30));
    assert_eq!(at(2, 30, Profile::Stock, &normal), (20, 30));
    assert_eq!(at(2, 30, Profile::Quicksilver(&NEUTRAL), &normal), (20, 30));
    // no class, or a class beyond the stock fifteen
    assert_eq!(at(0, 30, v120, &normal), (20, 30));
    assert_eq!(at(40, 30, v120, &normal), (20, 30));
    // the share is the only difference between 1.20.0 and the same balance without it
    let only = Profile::Quicksilver(&SHARE_ONLY);
    assert_eq!(at(2, 30, only, &normal), (22, 34));
    // a stab keeps its own figures
    let stab = request(4, 1);
    assert_eq!(at(4, 30, v120, &stab), at(1, 30, v120, &stab));
    // a manual figure is not scaled
    let manual = AttackRequest {
        specify_damage: 100.0,
        ..normal
    };
    assert_eq!(at(4, 30, v120, &manual), (100, 100));
    // bash triples what the share leaves: 14..21 at 110% then x3
    let (lo, hi) = at(4, 30, v120, &request(6, 1));
    assert_eq!((lo, hi), (fix(14.0 * 1.1) * 3, fix(21.0 * 1.1) * 3));
}

fn fix(x: f64) -> i32 {
    x as i32
}
