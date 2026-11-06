//! Loads the oracle's fixtures (`fixtures/rules/<family>.json`, written by
//! `tools/oracle/oracle.py gen`). The format is documented in `tools/oracle/README.md`.

#![allow(dead_code)] // each family's tests use a different subset

use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use touchstone_model::{AbilitySlot, Spell, TbInfo};
use touchstone_rules::ability_text::NameLookup;
use touchstone_rules::textblock::TextblockData;

pub struct Fixture {
    pub header: Value,
    pub cases: Vec<Value>,
}

/// Reads a fixture family. Panics with the path if it is missing or malformed: a missing
/// fixture is a test failure, never a skip.
pub fn load(family: &str) -> Fixture {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/rules")
        .join(format!("{family}.json"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let mut doc: Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parsing {}: {e}", path.display()));
    let header = doc["header"].take();
    let cases = match doc["cases"].take() {
        Value::Array(cases) => cases,
        _ => panic!("{} has no cases array", path.display()),
    };
    assert!(!cases.is_empty(), "{} has no cases", path.display());
    Fixture { header, cases }
}

/// A Currency value as the oracle writes it: `{"cur": units of 1/10000, "s": display}`.
pub fn cur(v: &Value) -> i64 {
    v["cur"]
        .as_i64()
        .unwrap_or_else(|| panic!("not a Currency value: {v}"))
}

/// A Single value as the oracle writes it: `{"single": x, "s": shortest decimal}`.
pub fn single(v: &Value) -> f32 {
    v["single"]
        .as_f64()
        .unwrap_or_else(|| panic!("not a Single value: {v}")) as f32
}

/// Shared by the families whose fixtures carry spell, name and textblock tables (a "table" case).
fn i(v: &Value) -> i64 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}"))
}

/// The case "table": every stock Spells row as [fields.., learnable, learned_from?, [ability, value, ..]].
pub fn spell_table(case: &Value) -> Vec<Spell> {
    case["inputs"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let f: Vec<i32> = r.as_array().unwrap()[..18]
                .iter()
                .map(|x| i(x) as i32)
                .collect();
            let mut abilities = [AbilitySlot::default(); 10];
            for (k, p) in r[18].as_array().unwrap().chunks(2).enumerate() {
                abilities[k] = AbilitySlot {
                    ability: i(&p[0]) as i32,
                    value: i(&p[1]) as i32,
                };
            }
            Spell {
                number: f[0],
                name: None,
                short: None,
                req_level: f[1],
                energy_cost: f[2],
                mana_cost: 0,
                min_base: f[3],
                max_base: f[4],
                diff: 0,
                type_of_resists: 0,
                targets: f[5],
                dur: f[6],
                att_type: f[7],
                magery: f[8],
                magery_lvl: 0,
                cap: f[9],
                max_inc_lvls: f[10],
                max_inc: f[11],
                min_inc_lvls: f[12],
                min_inc: f[13],
                dur_inc_lvls: f[14],
                dur_inc: f[15],
                abilities,
                learnable: f[16] != 0,
                learned_from: (f[17] != 0).then(|| "x".to_string()),
                casted_by: None,
                classes: None,
            }
        })
        .collect()
}

#[derive(Default)]
pub struct Names {
    pub spells: HashMap<i32, String>,
    pub monsters: HashMap<i32, String>,
    pub items: HashMap<i32, String>,
    pub classes: HashMap<i32, String>,
}

impl NameLookup for Names {
    fn spell_name(&self, n: i32) -> Option<String> {
        self.spells.get(&n).cloned()
    }
    fn monster_name(&self, n: i32) -> Option<String> {
        self.monsters.get(&n).cloned()
    }
    fn item_name(&self, n: i32) -> Option<String> {
        self.items.get(&n).cloned()
    }
    fn class_name(&self, n: i32) -> Option<String> {
        self.classes.get(&n).cloned()
    }
}

pub fn name_map(v: &Value) -> HashMap<i32, String> {
    v.as_array()
        .unwrap()
        .iter()
        // The converted database holds an empty name as NULL; MME's column holds "".
        .map(|p| (i(&p[0]) as i32, p[1].as_str().unwrap_or("").to_string()))
        .collect()
}

#[derive(Default)]
pub struct Textblocks(pub HashMap<i32, TbInfo>);

impl TextblockData for Textblocks {
    fn tbinfo(&self, number: i32) -> Option<&TbInfo> {
        self.0.get(&number)
    }
    fn room_cmd(&self, _map: i32, _room: i32) -> Option<i32> {
        None
    }
}

impl Names {
    /// The `names` object of a fixture's "table" case.
    pub fn from_table(names: &Value) -> Names {
        Names {
            spells: name_map(&names["spells"]),
            monsters: name_map(&names["monsters"]),
            items: name_map(&names["items"]),
            classes: name_map(&names["classes"]),
        }
    }
}

impl Textblocks {
    /// The `textblocks` array of a fixture's "table" case: `[number, action]` pairs.
    pub fn from_table(rows: &Value) -> Textblocks {
        Textblocks(
            rows.as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    let n = i(&p[0]) as i32;
                    (
                        n,
                        TbInfo {
                            number: n,
                            link_to: 0,
                            action: Some(p[1].as_str().unwrap().to_string()),
                            called_from: String::new(),
                        },
                    )
                })
                .collect(),
        )
    }
}
