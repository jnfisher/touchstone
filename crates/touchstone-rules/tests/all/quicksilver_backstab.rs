//! Q-009: the backstab rules (R1 damage, R3 crit) against the engine vectors in
//! `fixtures/quicksilver/backstab.json`. Written by `tools/qs-vectors`.
//!
//! `bs` cases carry the stab's [min, max] under the 1.20.0 balance (`v120`), a stock-equivalent one
//! (`neutral`) and an off-grid one (`alt`), plus the range a critical stab rolls (`v120_crit`,
//! `alt_crit`). `crit` cases are the critical range for a stab of a given [min, max].

use std::path::PathBuf;

use serde_json::Value;
use touchstone_model::{AbilitySlot, Item};
use touchstone_rules::Profile;
use touchstone_rules::attack::{
    AttackEnv, AttackRequest, CastText, CharacterProfile, EquippedGlobals, calc_bs_damage,
    calculate_attack,
};
use touchstone_rules::quicksilver::Balance;

/// The off-grid balance of the vectors' `alt` column (see the header's `balance_keys`).
static ALT: Balance = Balance {
    backstab_stealth_div: 5,
    backstab_level_pct: 150,
    backstab_crit_max_pct: 120,
    scalecap: 40,
    ..Balance::V120
};

fn load() -> (Value, Vec<Value>) {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/quicksilver/backstab.json");
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

fn pair(v: &Value) -> (i32, i32) {
    let a = v
        .as_array()
        .unwrap_or_else(|| panic!("pair expected, got {v}"));
    (int(&a[0]), int(&a[1]))
}

fn quicksilver(b: &'static Balance) -> Profile {
    Profile::Quicksilver(b)
}

#[test]
fn backstab_header_and_ids() {
    let (header, cases) = load();
    assert_eq!(header["family"], "backstab");
    assert_eq!(header["content_version"], "1.20.0");
    assert_eq!(header["case_count"].as_u64().unwrap(), cases.len() as u64);
    let ids: Vec<&str> = cases.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids sorted and unique");
    for kind in ["bs", "crit"] {
        assert!(cases.iter().any(|c| c["kind"] == kind), "no {kind} cases");
    }
    // The vectors' balances are the ones this crate carries.
    let k = &header["balance_keys"];
    let (v, n) = (Balance::V120, Balance::NEUTRAL);
    for (name, col, get) in [
        ("backstab_stealth_div", "v120", v.backstab_stealth_div),
        ("backstab_stealth_div", "neutral", n.backstab_stealth_div),
        ("backstab_stealth_div", "alt", ALT.backstab_stealth_div),
        ("backstab_level_pct", "v120", v.backstab_level_pct),
        ("backstab_level_pct", "neutral", n.backstab_level_pct),
        ("backstab_level_pct", "alt", ALT.backstab_level_pct),
        ("backstab_crit_max_pct", "v120", v.backstab_crit_max_pct),
        ("backstab_crit_max_pct", "alt", ALT.backstab_crit_max_pct),
        ("scalecap", "v120", v.scalecap),
        ("scalecap", "alt", ALT.scalecap),
    ] {
        assert_eq!(int(&k[name][col]), get as i32, "{name} {col}");
    }
    // Neutral has no level cap (the engine's neutral column used one out of reach).
    assert!(n.scalecap >= i32::MAX as u32);
}

fn bs_damage(profile: Profile, i: &Value) -> (i32, i32) {
    let f = |damage: &Value, plus: &Value| {
        calc_bs_damage(
            profile,
            int(&i["level"]) as i16,
            int(&i["stealth"]) as i16,
            int(damage) as i16,
            int(plus) as i16,
            i["class_stealth"].as_bool().unwrap(),
        )
    };
    (
        f(&i["weapon_min"], &i["bs_min"]),
        f(&i["weapon_max"], &i["bs_max"]),
    )
}

fn check_bs(profile: Profile, column: &str) {
    let (_, cases) = load();
    let mut bad = Vec::new();
    let mut n = 0;
    for c in cases.iter().filter(|c| c["kind"] == "bs") {
        n += 1;
        let (got, want) = (bs_damage(profile, &c["inputs"]), pair(&c[column]));
        if got != want {
            bad.push(format!(
                "{}: got {got:?}, vector {column} {want:?}",
                c["id"]
            ));
        }
    }
    assert!(n > 1000);
    assert!(
        bad.is_empty(),
        "{} of {n} differ; first:\n{}",
        bad.len(),
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}

/// R1 under the 1.20.0 balance and the off-grid one.
#[test]
fn backstab_damage_v120_and_alt() {
    check_bs(Profile::quicksilver_v120(), "v120");
    check_bs(quicksilver(&ALT), "alt");
}

/// Screening: the engine at stock-equivalent settings equals stock MME (CalcBSDamage), and the
/// neutral profile equals it too.
#[test]
fn backstab_damage_neutral_is_stock() {
    check_bs(Profile::Stock, "neutral");
    static NEUTRAL: Balance = Balance::NEUTRAL;
    check_bs(quicksilver(&NEUTRAL), "neutral");
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

fn weapon(min: i32, max: i32) -> Item {
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
        speed: 1200,
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

fn stab(profile: Profile, i: &Value, crit: i16) -> touchstone_rules::attack::AttackDamage {
    let items = [weapon(int(&i["weapon_min"]), int(&i["weapon_max"]))];
    let equipped = EquippedGlobals::default();
    let env = AttackEnv {
        items: &items,
        classes: &[],
        races: &[],
        equipped: &equipped,
        cast_text: &NoCasts,
        dat_ver_caption: "",
        avg_level_max_all_stats: 0.0,
        accuracy_tag: "",
    };
    let ch = CharacterProfile {
        level: int(&i["level"]),
        strength: 50,
        agility: 50,
        stealth: int(&i["stealth"]) as i16,
        class_stealth: i["class_stealth"].as_bool().unwrap(),
        plus_bs_min_damage: int(&i["bs_min"]) as i16,
        plus_bs_max_damage: int(&i["bs_max"]) as i16,
        crit,
        combat: 3,
        ..CharacterProfile::default()
    };
    let req = AttackRequest {
        attack_type: 4,
        weapon_number: 1,
        ..AttackRequest::default()
    };
    calculate_attack(profile, &env, &ch, &req, &mut String::new())
}

/// MME's banker's rounding of the midpoint, as `calculate_attack` applies it to the crit range.
fn mid(lo: i32, hi: i32) -> i32 {
    let s = lo + hi;
    if s % 2 == 0 || (s / 2) % 2 == 0 {
        s / 2
    } else {
        s / 2 + 1
    }
}

/// R1 and R3 through `calculate_attack` (attack type 4): the stab's min and max, its crit chance
/// (50, which no knee of the balances moves) and the crit range.
#[test]
fn backstab_through_calculate_attack() {
    let (_, cases) = load();
    let mut bad = Vec::new();
    for c in cases.iter().filter(|c| c["kind"] == "bs") {
        let i = &c["inputs"];
        // MME derives a class-stealth character's stealth when the box holds 0; that is the
        // stealth formula's business, not the stab's.
        if int(&i["stealth"]) == 0 && i["class_stealth"].as_bool().unwrap() {
            continue;
        }
        for (profile, col, crit_col) in [
            (Profile::quicksilver_v120(), "v120", "v120_crit"),
            (quicksilver(&ALT), "alt", "alt_crit"),
        ] {
            let got = stab(profile, i, 50);
            let (lo, hi) = pair(&c[col]);
            let (clo, chi) = pair(&c[crit_col]);
            let id = c["id"].as_str().unwrap();
            let expect = (lo, hi, 50, mid(clo, chi), chi);
            let seen = (
                got.min_dmg,
                got.max_dmg,
                i32::from(got.crit_chance),
                got.avg_crit,
                got.max_crit,
            );
            let shown = format!("Avg/Max Crit: {}/{} (50%)", mid(clo, chi), chi);
            if seen != expect || !got.attack_detail.contains(&shown) {
                bad.push(format!("{id} {col}: got {seen:?}, vector {expect:?}"));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{} differ; first:\n{}",
        bad.len(),
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}

/// Stock never crits a stab; the neutral balance reads the same. `crit` cases are for the engine's
/// crit range building block, checked here against the closed form through `ALT` and V120 numbers.
#[test]
fn stab_crit_off_in_stock_and_neutral() {
    let (_, cases) = load();
    static NEUTRAL: Balance = Balance::NEUTRAL;
    for c in cases.iter().filter(|c| c["kind"] == "bs").step_by(37) {
        for profile in [Profile::Stock, quicksilver(&NEUTRAL)] {
            let got = stab(profile, &c["inputs"], 50);
            assert_eq!(
                (got.crit_chance, got.avg_crit, got.max_crit),
                (0, 0, 0),
                "{}",
                c["id"]
            );
        }
    }
}

/// The crit-range building block: from a stab's [min, max], a critical stab rolls max up to
/// `backstab_crit_max_pct` percent of max (raised to max if that is lower).
#[test]
fn crit_range_cases() {
    let (_, cases) = load();
    let mut n = 0;
    for c in cases.iter().filter(|c| c["kind"] == "crit") {
        n += 1;
        let hi = int(&c["inputs"]["bs_max"]);
        for (b, col) in [(&Balance::V120, "v120"), (&ALT, "alt")] {
            let top = hi * b.backstab_crit_max_pct as i32 / 100;
            assert_eq!(pair(&c[col]), (hi, top.max(hi)), "{} {col}", c["id"]);
        }
        assert!(c["neutral"].is_null());
    }
    assert!(n >= 9);
}
