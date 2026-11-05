// Fixture tests for the ability text family (FAM-03 text half): see docs/porting.md step 5.
use crate::fixture;
use serde_json::Value;
use std::cell::Cell;
use std::collections::HashMap;
use touchstone_model::{AbilitySlot, Spell, TbInfo};
use touchstone_rules::Profile;
use touchstone_rules::ability_text::*;
use touchstone_rules::spells::PullArgs;
use touchstone_rules::textblock::TextblockData;

fn i(v: &Value) -> i64 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}"))
}

/// The case "table": every stock Spells row as [fields.., learnable, learned_from?, [ability, value, ..]].
fn spell_table(case: &Value) -> Vec<Spell> {
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
struct Names {
    spells: HashMap<i32, String>,
    monsters: HashMap<i32, String>,
    items: HashMap<i32, String>,
    classes: HashMap<i32, String>,
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

fn name_map(v: &Value) -> HashMap<i32, String> {
    v.as_array()
        .unwrap()
        .iter()
        // The converted database holds an empty name as NULL; MME's column holds "".
        .map(|p| (i(&p[0]) as i32, p[1].as_str().unwrap_or("").to_string()))
        .collect()
}

#[derive(Default)]
struct Textblocks(HashMap<i32, TbInfo>);

impl TextblockData for Textblocks {
    fn tbinfo(&self, number: i32) -> Option<&TbInfo> {
        self.0.get(&number)
    }
    fn room_cmd(&self, _map: i32, _room: i32) -> Option<i32> {
        None
    }
}

fn is_missing(v: &Value) -> bool {
    v["t"] == "missing"
}

fn flag(args: &[Value], k: usize) -> bool {
    args.get(k)
        .filter(|v| !is_missing(v))
        .is_some_and(|v| v.as_bool().unwrap())
}

#[test]
fn ability_text_fixture() {
    let fx = fixture::load("ability_text");
    let table = &fx.cases.iter().find(|c| c["id"] == "table").unwrap()["inputs"];
    let spells = spell_table(&fx.cases[0]);
    assert!(spells.len() > 1000);
    let names = Names {
        spells: name_map(&table["names"]["spells"]),
        monsters: name_map(&table["names"]["monsters"]),
        items: name_map(&table["names"]["items"]),
        classes: name_map(&table["names"]["classes"]),
    };
    let tbs = Textblocks(
        table["textblocks"]
            .as_array()
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
    );

    let mut fails = Vec::new();
    let mut ran = 0;
    for c in &fx.cases {
        let id = c["id"].as_str().unwrap();
        if id == "table" {
            continue;
        }
        let inp = &c["inputs"];
        let a = inp["args"].as_array().unwrap();
        let proc = inp["proc"].as_str().unwrap();
        let want = &c["outputs"]["result"];
        assert!(c.get("error").is_none(), "{id}: oracle error");
        let frm = &inp["forms"]["frmMain"];
        let env = AbilityEnv {
            profile: Profile::Stock,
            names: &names,
            textblocks: &tbs,
            spells: &spells,
            hide_record_numbers: inp["globals"]["bHideRecordNumbers"] == true,
            quick_spell: inp["globals"]["bQuickSpell"] == true,
            global_level_text: (frm["chkGlobalFilter"]["Value"] == 1).then(|| {
                frm["txtGlobalLevel"]["$array"]["0"]["Text"]
                    .as_str()
                    .unwrap_or("")
            }),
            avg_level_max_all_stats: 0.0,
            nest: Cell::new(inp["globals"]["nSpellNest"].as_i64().unwrap_or(0) as i32),
        };
        ran += 1;
        let got: Value = match proc {
            "GetAbilityName" => {
                get_ability_name(Profile::Stock, i(&a[0]) as i16, flag(a, 1)).into()
            }
            "GetAbilityList" => get_ability_list(Profile::Stock, env.hide_record_numbers).into(),
            "GetAbilityStats" => env
                .get_ability_stats(
                    i(&a[0]) as i16,
                    i(&a[1]) as i16,
                    a.get(3)
                        .is_none_or(|v| is_missing(v) || v.as_bool().unwrap()),
                    flag(a, 4),
                )
                .into(),
            "GetAbilDiffText" => env
                .get_abil_diff_text(
                    i(&a[0]) as i32,
                    i(&a[1]) as i32,
                    i(&a[2]) as i32,
                    i(&a[4]) as i16,
                    flag(a, 5),
                )
                .into(),
            other => panic!("{id}: unknown proc {other}"),
        };
        if got != *want {
            fails.push(format!("{id}: got {got}, want {want}; inputs {inp}"));
        }
    }
    assert!(ran > 5000, "only {ran} cases ran");
    assert!(
        fails.is_empty(),
        "{} of {ran} cases failed, first: {:#?}",
        fails.len(),
        &fails[..fails.len().min(8)]
    );
}

// An empty database: every name lookup answers with the number, PullSpellEQ with its placeholder.
#[test]
fn empty_tables() {
    let names = Names::default();
    let tbs = Textblocks::default();
    let env = AbilityEnv {
        profile: Profile::Stock,
        names: &names,
        textblocks: &tbs,
        spells: &[],
        hide_record_numbers: false,
        quick_spell: false,
        global_level_text: None,
        avg_level_max_all_stats: 0.0,
        nest: Cell::new(0),
    };
    assert_eq!(env.get_ability_stats(42, 7, true, false), "LearnSp (7)");
    assert_eq!(env.get_ability_stats(59, 0, true, false), "ClassOk");
    assert_eq!(env.get_ability_stats(59, 3, true, false), "ClassOk 3");
    assert_eq!(
        env.get_ability_stats(43, 7, true, false),
        "CastsSp [7, (No Spell Records)]"
    );
    assert_eq!(
        env.pull_spell_eq(&PullArgs::default()),
        "(No Spell Records)"
    );
    assert_eq!(env.nest.get(), 0);
}
