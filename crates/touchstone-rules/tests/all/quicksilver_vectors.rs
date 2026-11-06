//! The engine vectors under `fixtures/quicksilver/` (written by `tools/qs-vectors`): structure,
//! header, and the screening half of part 4 layer 2. Each family's `neutral` column is
//! Quicksilver's engine run with the stock-equivalent balance, so the stock MME function must
//! agree with it; a case where it does not is outside the region MME and the engine share and is
//! moved out of the grid, not explained away. The `v120` column is checked by the card that
//! ports the rule (Q-006 for the knees, through the real `Profile` functions).

use std::path::PathBuf;

use serde_json::Value;
use touchstone_model::Magery;
use touchstone_rules::Profile;
use touchstone_rules::accuracy::calc_dodge;
use touchstone_rules::attack::crit_after_knee;
use touchstone_rules::quicksilver::Balance;
use touchstone_rules::spell_cast::{
    SpellCasterProfile, area_resist_chance, area_resist_keep, casting_stat, scale_spell_damage,
};
use touchstone_rules::stats::{calc_mr, soft_cap, train_cost, train_max};

struct Vectors {
    header: Value,
    cases: Vec<Value>,
}

/// Reads a vector family. A missing file is a failure, never a skip.
fn load(family: &str) -> Vectors {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/quicksilver")
        .join(format!("{family}.json"));
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

#[test]
fn knees_header_and_ids() {
    let v = load("knees");
    assert_eq!(v.header["family"], "knees");
    assert_eq!(v.header["content_version"], "1.20.0");
    assert_eq!(
        v.header["case_count"].as_u64().unwrap(),
        v.cases.len() as u64
    );
    let ids: Vec<&str> = v.cases.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids sorted and unique");
    for kind in ["crit", "mr", "dodge"] {
        assert!(v.cases.iter().any(|c| c["kind"] == kind), "no {kind} cases");
    }
}

/// The header's knee values are the ones touchstone's `Balance` carries, so a retune of either
/// side fails here.
#[test]
fn knees_header_matches_balance() {
    let keys = &load("knees").header["balance_keys"];
    let (v, n) = (Balance::V120, Balance::NEUTRAL);
    let pair = |k: &str| (int(&keys[k]["v120"]), int(&keys[k]["neutral"]));
    assert_eq!(pair("crit_knee"), (v.crit_knee as i32, n.crit_knee as i32));
    assert_eq!(
        pair("mr_stat_knee"),
        (v.mr_stat_knee as i32, n.mr_stat_knee as i32)
    );
    assert_eq!(
        pair("dodge_agi_knee"),
        (v.dodge_agi_knee as i32, n.dodge_agi_knee as i32)
    );
}

/// Screening: the engine at stock knees equals stock MME. Expected for `mr` and `dodge`: MME's
/// CalcMR and CalcDodge (no encumbrance, so the light-load term is off). For `crit` the stock
/// block in CalcAttack: past 40 a point counts a third.
#[test]
fn knees_neutral_column_matches_stock_mme() {
    let v = load("knees");
    let mut bad = Vec::new();
    for c in &v.cases {
        let i = &c["inputs"];
        let want = int(&c["neutral"]);
        let got = match c["kind"].as_str().unwrap() {
            "crit" => {
                let crit = int(&i["crit"]);
                if crit > 40 {
                    40 + (crit - 40) / 3
                } else {
                    crit
                }
            }
            "mr" => calc_mr(
                Profile::Stock,
                int(&i["int"]),
                int(&i["wil"]),
                int(&i["gear"]),
            ),
            "dodge" => calc_dodge(
                Profile::Stock,
                int(&i["level"]) as i16,
                int(&i["agility"]) as i16,
                int(&i["charm"]) as i16,
                f64::from(int(&i["plus_dodge"])),
                0.0,
                -1.0,
            ),
            other => panic!("unknown kind {other}"),
        };
        if got != want {
            bad.push(format!(
                "{}: stock {got}, engine at neutral {want}",
                c["id"]
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} differ; first:\n{}",
        bad.len(),
        v.cases.len(),
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}

/// The knee functions of the ported rules for one case, under `profile`. A crit above 99 is
/// MME's cap, applied after the knee, so the engine's uncapped value is compared capped.
fn knee_answer(profile: Profile, case: &Value) -> (i32, i32) {
    let i = &case["inputs"];
    match case["kind"].as_str().unwrap() {
        "crit" => {
            let got = crit_after_knee(profile, int(&i["crit"]) as i16).unwrap();
            (i32::from(got), 99)
        }
        "mr" => (
            calc_mr(profile, int(&i["int"]), int(&i["wil"]), int(&i["gear"])),
            i32::MAX,
        ),
        "dodge" => (
            calc_dodge(
                profile,
                int(&i["level"]) as i16,
                int(&i["agility"]) as i16,
                int(&i["charm"]) as i16,
                f64::from(int(&i["plus_dodge"])),
                0.0,
                -1.0,
            ),
            i32::MAX,
        ),
        other => panic!("unknown kind {other}"),
    }
}

/// Every knee case: `column` of the vector is what the ported functions answer under `profile`
/// (the second element of `knee_answer` is the cap MME puts on the engine's value).
fn check_knees(profile: Profile, column: &str) {
    let v = load("knees");
    let mut bad = Vec::new();
    for c in &v.cases {
        let (got, cap) = knee_answer(profile, c);
        let want = int(&c[column]).min(cap);
        if got != want {
            bad.push(format!("{}: got {got}, vector {column} {want}", c["id"]));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} differ; first:\n{}",
        bad.len(),
        v.cases.len(),
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn training_header_and_ids() {
    let v = load("training");
    assert_eq!(v.header["family"], "training");
    assert_eq!(v.header["content_version"], "1.20.0");
    assert_eq!(
        v.header["case_count"].as_u64().unwrap(),
        v.cases.len() as u64
    );
    let ids: Vec<&str> = v.cases.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids sorted and unique");
    for kind in ["cost", "softcap", "train_max"] {
        assert!(v.cases.iter().any(|c| c["kind"] == kind), "no {kind} cases");
    }
    let keys = &v.header["balance_keys"];
    let (b, n) = (Balance::V120, Balance::NEUTRAL);
    assert_eq!(int(&keys["softcap_floor"]["v120"]), b.softcap_floor as i32);
    assert_eq!(
        int(&keys["softcap_floor"]["neutral"]),
        n.softcap_floor as i32
    );
    assert_eq!(
        int(&keys["softcap_floor_price"]["v120"]),
        b.softcap_floor_price as i32
    );
    assert_eq!(
        int(&keys["softcap_floor_price"]["neutral"]),
        n.softcap_floor_price as i32
    );
    for (col, bal) in [("v120", b), ("neutral", n)] {
        let got: Vec<i32> = keys["softcap_band_prices"][col]
            .as_array()
            .unwrap()
            .iter()
            .map(int)
            .collect();
        let want: Vec<i32> = bal.softcap_band_prices.iter().map(|&p| p as i32).collect();
        assert_eq!(got, want, "softcap_band_prices {col}");
    }
}

/// The v120 column is touchstone under `Profile::Quicksilver(V120)`; the neutral column is
/// stock MME (the cost ladder from the race minimum, no cap), present exactly where the target
/// is at or below the race maximum. Expected values: fixtures/quicksilver/training.json.
#[test]
fn training_columns_match_touchstone() {
    let v = load("training");
    let qs = Profile::Quicksilver(&Balance::V120);
    let neutral = Profile::Quicksilver(&Balance::NEUTRAL);
    let mut bad = Vec::new();
    for c in &v.cases {
        let i = &c["inputs"];
        let id = c["id"].as_str().unwrap();
        let ((got_v, got_n), has_neutral) = match c["kind"].as_str().unwrap() {
            "cost" => {
                let (mn, mx) = (int(&i["race_min"]), int(&i["race_max"]));
                let (from, to) = (int(&i["from"]), int(&i["to"]));
                let at = |p| train_cost(p, mn, mx, from, to).unwrap();
                // stock MME and the NEUTRAL profile agree everywhere
                assert_eq!(at(Profile::Stock), at(neutral), "{id}");
                ((at(qs), at(Profile::Stock)), to <= mx)
            }
            "softcap" => {
                let x = int(&i["race_max"]);
                ((soft_cap(qs, x), soft_cap(neutral, x)), true)
            }
            "train_max" => {
                let x = int(&i["race_max"]);
                // Stock sets no limit, so only the v120 column has a touchstone answer
                ((train_max(qs, x).unwrap(), 0), false)
            }
            other => panic!("unknown kind {other}"),
        };
        if got_v != int(&c["v120"]) {
            bad.push(format!("{id}: v120 got {got_v}, want {}", c["v120"]));
        }
        if has_neutral {
            if c["neutral"].is_null() || got_n != int(&c["neutral"]) {
                bad.push(format!("{id}: neutral got {got_n}, want {}", c["neutral"]));
            }
        } else if c["kind"] == "cost" && !c["neutral"].is_null() {
            bad.push(format!("{id}: neutral must be null above the race maximum"));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} differ; first:\n{}",
        bad.len(),
        v.cases.len(),
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}

/// R11 to R13: Quicksilver with the 1.20.0 balance reproduces the engine's answers.
#[test]
fn knees_v120_column_matches_quicksilver_profile() {
    check_knees(Profile::quicksilver_v120(), "v120");
}

/// The neutral column is both stock and the neutral balance.
#[test]
fn knees_neutral_column_matches_stock_and_neutral_profiles() {
    check_knees(Profile::Stock, "neutral");
    check_knees(Profile::Quicksilver(&Balance::NEUTRAL), "neutral");
}

/// The vectors must reach the compressed region of every knee, or the V120 test proves nothing.
#[test]
fn knees_vectors_reach_past_each_knee() {
    let v = load("knees");
    let moved = |kind: &str| {
        v.cases
            .iter()
            .filter(|c| c["kind"] == kind && c["v120"] != c["neutral"])
            .count()
    };
    for kind in ["crit", "mr", "dodge"] {
        assert!(moved(kind) > 20, "{kind}: v120 and neutral rarely differ");
    }
}

#[test]
fn spells_header_and_ids() {
    let v = load("spells");
    assert_eq!(v.header["family"], "spells");
    assert_eq!(v.header["content_version"], "1.20.0");
    assert_eq!(
        v.header["case_count"].as_u64().unwrap(),
        v.cases.len() as u64
    );
    let ids: Vec<&str> = v.cases.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids sorted and unique");
    for kind in ["stat", "class", "damage", "casting", "area"] {
        assert!(v.cases.iter().any(|c| c["kind"] == kind), "no {kind} cases");
    }
    let keys = &v.header["balance_keys"];
    let (b, n) = (Balance::V120, Balance::NEUTRAL);
    for (key, bv, nv) in [
        (
            "spell_stat_permille",
            b.spell_stat_permille,
            // The engine's neutral permille is the NEUTRAL profile's: 0
            n.spell_stat_permille,
        ),
        ("spell_stat_knee", b.spell_stat_knee, n.spell_stat_knee),
        (
            "aoe_resist_cut_max_pct",
            b.aoe_resist_cut_max_pct,
            n.aoe_resist_cut_max_pct,
        ),
        (
            "aoe_resist_cut_div",
            b.aoe_resist_cut_div,
            n.aoe_resist_cut_div,
        ),
    ] {
        assert_eq!(int(&keys[key]["v120"]), bv as i32, "{key} v120");
        assert_eq!(int(&keys[key]["neutral"]), nv as i32, "{key} neutral");
    }
}

fn caster(stat: i32, class: i32, level: i32) -> SpellCasterProfile {
    SpellCasterProfile {
        casting_stat: stat,
        class,
        level,
        ..SpellCasterProfile::default()
    }
}

/// One spells case under `profile`, compared with the vector's `column`.
fn check_spells(profile: Profile, column: &str) {
    let v = load("spells");
    let mut bad = Vec::new();
    for c in &v.cases {
        let i = &c["inputs"];
        let id = c["id"].as_str().unwrap();
        let want = &c[column];
        match c["kind"].as_str().unwrap() {
            "stat" => {
                let got =
                    scale_spell_damage(profile, int(&i["amount"]), &caster(int(&i["stat"]), 0, 0));
                if got != int(want) {
                    bad.push(format!("{id}: got {got}, vector {column} {want}"));
                }
            }
            "class" => {
                let who = caster(0, int(&i["class"]), int(&i["level"]));
                let got = scale_spell_damage(profile, int(&i["amount"]), &who);
                if got != int(want) {
                    bad.push(format!("{id}: got {got}, vector {column} {want}"));
                }
            }
            "damage" => {
                let who = caster(int(&i["stat"]), int(&i["class"]), int(&i["level"]));
                let got = scale_spell_damage(profile, int(&i["amount"]), &who);
                if got != int(want) {
                    bad.push(format!("{id}: got {got}, vector {column} {want}"));
                }
            }
            "casting" => {
                let magery = Magery::from_code(int(&i["magery"])).unwrap();
                let got = casting_stat(magery, int(&i["int"]), int(&i["wil"]), int(&i["cha"]));
                if got != int(want) {
                    bad.push(format!("{id}: got {got}, vector {column} {want}"));
                }
            }
            "area" => {
                // The engine's exact sum of integer shares over the roll space, against the closed
                // form 'share x (1 - p x cut)': each resisting roll truncates its share, losing less
                // than 1, so the sum sits within `resisting_rolls` below the closed form.
                let (mr, share) = (int(&i["mr"]), int(&i["share"]));
                let rolls = f64::from(int(&want["rolls"]));
                let sum = f64::from(int(&want["sum"]));
                let closed = f64::from(share) * rolls * area_resist_keep(profile, mr);
                let resisting = int(&want["resisting_rolls"]);
                if sum > closed + 1e-6 || closed - sum > f64::from(resisting) + 1e-6 {
                    bad.push(format!(
                        "{id}: closed form {closed}, engine sum {sum}, {resisting} resisting rolls"
                    ));
                }
                // The chance is the engine's count of resisting rolls, the cut its cut. Stock never
                // resists (the cut is off), as does a neutral balance.
                let (chance, cut) = area_resist_chance(profile, mr);
                let (want_chance, want_cut) = match profile {
                    Profile::Stock => (0, 0),
                    Profile::Quicksilver(_) => (resisting, int(&want["cut_pct"])),
                };
                if chance != want_chance || (resisting > 0 && cut != want_cut) {
                    bad.push(format!(
                        "{id}: chance/cut {chance}/{cut}, vector {want_chance}/{want_cut}"
                    ));
                }
            }
            other => panic!("unknown kind {other}"),
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} differ; first:\n{}",
        bad.len(),
        v.cases.len(),
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}

/// R7, R8, R9: Quicksilver with the 1.20.0 balance reproduces the engine's answers.
#[test]
fn spells_v120_column_matches_quicksilver_profile() {
    check_spells(Profile::quicksilver_v120(), "v120");
}

/// The neutral column is the engine with the three rules off: the stock path and the NEUTRAL
/// profile both give it.
#[test]
fn spells_neutral_column_matches_stock_and_neutral_profiles() {
    check_spells(Profile::Stock, "neutral");
    check_spells(Profile::Quicksilver(&Balance::NEUTRAL), "neutral");
}

/// The vectors must reach each rule, or the V120 test proves nothing.
#[test]
fn spells_vectors_reach_each_rule() {
    let v = load("spells");
    let moved = |kind: &str| {
        v.cases
            .iter()
            .filter(|c| c["kind"] == kind && c["v120"] != c["neutral"])
            .count()
    };
    for (kind, least) in [("stat", 100), ("class", 100), ("damage", 100), ("area", 50)] {
        assert!(
            moved(kind) > least,
            "{kind}: v120 and neutral rarely differ"
        );
    }
}
