// Fixture tests for the abilities family: see docs/porting.md step 5.
use crate::fixture;
use serde_json::Value;
use touchstone_model::AbilitySlot;
use touchstone_rules::Profile;
use touchstone_rules::abilities::*;

fn slots(row: &Value) -> Vec<AbilitySlot> {
    row["abilities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| AbilitySlot {
            ability: p[0].as_i64().unwrap() as i32,
            value: p[1].as_i64().unwrap() as i32,
        })
        .collect()
}

#[test]
fn abilities_fixture() {
    let fx = fixture::load("abilities");
    let mut fails = Vec::new();
    let mut ran = 0;
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        let inputs = &case["inputs"];
        let out = &case["outputs"];
        // Whole-case outputs for the pure procedures; steps for the row lookups.
        match inputs["proc"].as_str() {
            Some("AbilityEffectsCharStats") => {
                let n = inputs["args"][0].as_i64().unwrap() as i16;
                let got = ability_effects_char_stats(Profile::Stock, n);
                ran += 1;
                if Some(got) != out["result"].as_bool() {
                    fails.push(format!(
                        "{id}: effects({n}) = {got}, want {}",
                        out["result"]
                    ));
                }
            }
            Some("GetAbilityStatSlot") => {
                let n = inputs["args"][0].as_i64().unwrap() as i16;
                let got = get_ability_stat_slot(n);
                ran += 1;
                if Some(i64::from(got)) != out["result"]["nequip"].as_i64() {
                    fails.push(format!(
                        "{id}: slot({n}) = {got}, want {}",
                        out["result"]["nequip"]
                    ));
                }
            }
            _ => {
                let row = &inputs["row"];
                let kind = row["kind"].as_str().unwrap();
                let rn = row["number"].as_i64().unwrap();
                let rs = slots(row);
                for st in inputs["steps"].as_array().unwrap() {
                    let num = st["args"][0].as_i64().unwrap();
                    let ab = st["args"][1].as_i64().unwrap() as i16;
                    // A Seek on another record number finds a different row (or none).
                    let s = (num == rn).then_some(rs.as_slice());
                    let n = num as i32;
                    let got = match kind {
                        "race" => race_has_ability(Profile::Stock, s, n, ab),
                        "class" => class_has_ability(Profile::Stock, s, n, ab),
                        "spell" => spell_has_ability(Profile::Stock, s, n, ab),
                        "item" => item_has_ability(Profile::Stock, s, n, ab),
                        k => panic!("{id}: kind {k}"),
                    };
                    ran += 1;
                    let key = st["as"].as_str().unwrap();
                    if Some(i64::from(got)) != out[key].as_i64() {
                        fails.push(format!(
                            "{id} {key} {}({num},{ab}) = {got}, want {}",
                            st["call"], out[key]
                        ));
                    }
                }
            }
        }
    }
    assert!(ran > 30000, "only {ran} checks ran");
    assert!(
        fails.is_empty(),
        "{} of {ran} checks failed, first: {:#?}",
        fails.len(),
        &fails[..fails.len().min(10)]
    );
}
