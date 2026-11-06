// Fixture tests for the Classes/Races builder: see docs/porting.md step 5.
// Expected values come from fixtures/rules/class_race_builder.json (frmMain's RefreshClassRaceBuilder
// cut out of the form and run by the oracle; the spellbook cases run GetClassMagery and GetClassMageryLVL).
use serde_json::Value;
use touchstone_model::{AbilitySlot, Class, Race};
use touchstone_rules::Profile;
use touchstone_rules::attack::{AttackEnv, CastText, EquippedGlobals};
use touchstone_rules::class_race_builder::*;

use crate::fixture;

fn i(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
}

fn pairs(v: &Value) -> [AbilitySlot; 10] {
    let mut out = [AbilitySlot::default(); 10];
    for (k, p) in v.as_array().unwrap().iter().enumerate() {
        out[k] = AbilitySlot {
            ability: i(&p[0]),
            value: i(&p[1]),
        };
    }
    out
}

struct NoCasts;

impl CastText for NoCasts {
    fn spell_name(&self, n: i32) -> String {
        panic!("cast text asked for spell {n}")
    }
    fn spell_eq(&self, n: i32, _: i16) -> String {
        panic!("cast text asked for spell {n}")
    }
}

fn rows(data: &Value) -> (Vec<Class>, Vec<Race>) {
    let classes = data["classes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Class {
            number: i(&r["n"]),
            name: r["name"].as_str().unwrap().to_string(),
            min_hits: i(&r["min"]),
            max_hits: i(&r["max"]),
            exp_table: i(&r["exp"]),
            magery_type: i(&r["mt"]),
            magery_lvl: i(&r["ml"]),
            weapon_type: i(&r["wt"]),
            armour_type: i(&r["at"]),
            combat_lvl: i(&r["combat"]),
            abilities: pairs(&r["ab"]),
        })
        .collect();
    let races = data["races"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Race {
            number: i(&r["n"]),
            name: r["name"].as_str().unwrap().to_string(),
            m_int: i(&r["stats"][0]),
            m_wil: i(&r["stats"][1]),
            m_str: i(&r["stats"][2]),
            m_hea: i(&r["stats"][3]),
            m_agl: i(&r["stats"][4]),
            m_chm: i(&r["stats"][5]),
            x_int: i(&r["stats"][6]),
            x_wil: i(&r["stats"][7]),
            x_str: i(&r["stats"][8]),
            x_hea: i(&r["stats"][9]),
            x_agl: i(&r["stats"][10]),
            x_chm: i(&r["stats"][11]),
            hp_per_lvl: i(&r["hp"]),
            exp_table: i(&r["exp"]),
            base_cp: i(&r["cp"]),
            abilities: pairs(&r["ab"]),
        })
        .collect();
    (classes, races)
}

/// The selection a case set: the Tag of txtClassRaceDetail(0) and (1), empty for none.
fn selected(case: &Value, idx: &str) -> i32 {
    let tag = case["inputs"]["forms"]["frmMain"]["txtClassRaceDetail"]["$array"][idx]["Tag"]
        .as_str()
        .unwrap_or("");
    touchstone_vb6::val(tag) as i32
}

#[test]
fn builder_text_matches_mme() {
    let fx = fixture::load("class_race_builder");
    let (classes, races) = rows(&fx.header["data"]);
    let equipped = EquippedGlobals::default();
    let mut failures = Vec::new();
    let mut ran = 0;
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        if id.starts_with("spellbook-") {
            continue;
        }
        let single = case["outputs"]["avg"].as_f64().unwrap();
        let env = AttackEnv {
            items: &[],
            classes: &classes,
            races: &races,
            equipped: &equipped,
            cast_text: &NoCasts,
            dat_ver_caption: "v1.11p",
            avg_level_max_all_stats: 0.0,
            accuracy_tag: "",
        };
        let got = build(
            Profile::Stock,
            &env,
            single,
            selected(case, "0"),
            selected(case, "1"),
        );
        // A handled error leaves the box as it was: the case's starting text.
        let start = case["inputs"]["forms"]["frmMain"]["txtClassRaceDetail"]["$array"]["2"]["Text"]
            .as_str()
            .unwrap_or("");
        let got = got.map_or_else(|_| start.to_string(), |b| b.text);
        let want = case["outputs"]["text"].as_str().unwrap();
        ran += 1;
        if got != want {
            failures.push(format!("{id}: got {got:?}, want {want:?}"));
        }
    }
    assert!(ran >= 50, "only {ran} builder cases");
    assert!(
        failures.is_empty(),
        "{} of {ran} failed, first: {:#?}",
        failures.len(),
        &failures[..failures.len().min(3)]
    );
}

#[test]
fn spellbook_seed_matches_mme_magery() {
    let fx = fixture::load("class_race_builder");
    let (classes, _) = rows(&fx.header["data"]);
    let mut ran = 0;
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        let Some(n) = id.strip_prefix("spellbook-") else {
            continue;
        };
        let class: i32 = n.parse().unwrap();
        // GetClassMagery and GetClassMageryLVL as the oracle ran them
        let magery = i(&case["outputs"]["magery"]);
        let level = i(&case["outputs"]["level"]);
        let want = if magery == 0 {
            Spellbook::NoMagic
        } else {
            Spellbook::Open {
                class,
                level: 999,
                magery,
                magery_level: level,
            }
        };
        assert_eq!(spellbook(&classes, class), want, "{id}");
        ran += 1;
    }
    assert_eq!(ran, 16);
    assert_eq!(spellbook(&classes, 0), Spellbook::Nothing);
}

#[test]
fn equip_takes_only_rows_that_exist() {
    let fx = fixture::load("class_race_builder");
    let (classes, races) = rows(&fx.header["data"]);
    assert_eq!(
        equip_selection(&classes, &races, 3, 5),
        EquipSelection {
            class: Some(3),
            race: Some(5)
        }
    );
    assert_eq!(
        equip_selection(&classes, &races, 99, 0),
        EquipSelection {
            class: None,
            race: None
        }
    );
}
