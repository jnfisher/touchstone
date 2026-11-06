// Fixture tests for the spell_tools family (card P4-32).
// Expected values: fixtures/rules/spell_tools.json, produced by the oracle running MME's own FindNegates,
// the list/pane menu code that builds its arguments, and LookUpSpellCast against the stock database
// (plus invented rows numbered 9001 and up).
use crate::fixture::load;
use serde_json::Value;
use std::collections::HashMap;
use touchstone_model::{AbilitySlot, Item, Shop, Spell};
use touchstone_rules::lairs::LairInfo;
use touchstone_rules::locations::{LocationData, LocationLairs, LocationOptions};
use touchstone_rules::spell_tools::*;
use touchstone_vb6::val;

fn int(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
}

fn blank_item(number: i32, name: &str) -> Item {
    Item {
        number,
        name: name.to_string(),
        limit: 0,
        encum: 0,
        item_type: 0,
        use_count: 0,
        price: 0,
        currency: 0,
        min: 0,
        max: 0,
        armour_class: 0,
        damage_resist: 0,
        weapon_type: 0,
        armour_type: 0,
        worn: 0,
        accy: 0,
        gettable: false,
        str_req: 0,
        speed: 0,
        not_droppable: false,
        destroy_on_death: false,
        retain_after_uses: false,
        class_rest: [0; 10],
        race_rest: [0; 10],
        negate_spell: [0; 10],
        abilities: [AbilitySlot::default(); 20],
        in_game: false,
        obtained_from: None,
        references: None,
    }
}

fn items(data: &Value) -> Vec<Item> {
    data["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut it = blank_item(int(&r[0]), r[1].as_str().unwrap());
            it.in_game = int(&r[2]) != 0;
            for (k, n) in r[3].as_array().unwrap().iter().enumerate() {
                it.negate_spell[k] = int(n);
            }
            for (k, p) in r[4].as_array().unwrap().chunks(2).enumerate() {
                it.abilities[k] = AbilitySlot {
                    ability: int(&p[0]),
                    value: int(&p[1]),
                };
            }
            it
        })
        .collect()
}

fn spells(data: &Value) -> Vec<Spell> {
    let cast = &data["cast"];
    data["spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let n = int(&r[0]);
            // Only the length of Casted By and where "item #" sits in it matter, unless the fixture
            // holds the text (the what-casts cases).
            let casted_by = match cast.get(n.to_string()).and_then(Value::as_str) {
                Some(t) => t.to_string(),
                None => {
                    let (len, pos) = (int(&r[3]) as usize, int(&r[4]) as usize);
                    let mut s = "x".repeat(len);
                    if pos > 0 {
                        s.replace_range(pos - 1..pos - 1 + 6, "item #");
                    }
                    s
                }
            };
            let mut abilities = [AbilitySlot::default(); 10];
            for (k, p) in r[16].as_array().unwrap().chunks(2).enumerate() {
                abilities[k] = AbilitySlot {
                    ability: int(&p[0]),
                    value: int(&p[1]),
                };
            }
            Spell {
                number: n,
                name: Some(r[1].as_str().unwrap().to_string()),
                short: None,
                req_level: int(&r[15]),
                energy_cost: 0,
                mana_cost: 0,
                min_base: int(&r[5]),
                max_base: int(&r[6]),
                diff: 0,
                type_of_resists: 0,
                targets: 0,
                dur: int(&r[11]),
                att_type: 0,
                magery: 0,
                magery_lvl: 0,
                cap: int(&r[14]),
                max_inc_lvls: int(&r[10]),
                max_inc: int(&r[8]),
                min_inc_lvls: int(&r[9]),
                min_inc: int(&r[7]),
                dur_inc_lvls: int(&r[13]),
                dur_inc: int(&r[12]),
                abilities,
                learnable: int(&r[2]) != 0,
                learned_from: None,
                casted_by: (!casted_by.is_empty()).then_some(casted_by),
                classes: None,
            }
        })
        .collect()
}

struct Names {
    items: HashMap<i32, Item>,
    mons: HashMap<i32, String>,
    spells: HashMap<i32, String>,
}

impl LocationData for Names {
    fn item(&self, n: i32) -> Option<&Item> {
        self.items.get(&n)
    }
    fn shop(&self, _: i32) -> Option<&Shop> {
        None
    }
    fn monster_name(&self, n: i32) -> Option<String> {
        self.mons.get(&n).cloned()
    }
    fn spell_name(&self, n: i32) -> Option<String> {
        self.spells.get(&n).cloned()
    }
    fn room(&self, _: i32, _: i32) -> Option<(String, i32)> {
        None
    }
}

struct NoLairs;

impl LocationLairs for NoLairs {
    fn get_lair_info(&mut self, _: &str, _: i16) -> LairInfo {
        LairInfo::default()
    }
}

fn names(data: &Value) -> Names {
    let pairs = |k: &str| -> Vec<(i32, Option<String>)> {
        data["names"][k]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (int(&p[0]), p[1].as_str().map(String::from)))
            .collect()
    };
    Names {
        items: pairs("items")
            .into_iter()
            .filter_map(|(n, name)| name.map(|nm| (n, blank_item(n, &nm))))
            .collect(),
        mons: pairs("mons")
            .into_iter()
            .filter_map(|(n, name)| name.map(|nm| (n, nm)))
            .collect(),
        spells: pairs("spells")
            .into_iter()
            .filter_map(|(n, name)| name.map(|nm| (n, nm)))
            .collect(),
    }
}

fn longs(v: &Value) -> Vec<i32> {
    v.as_array().unwrap().iter().map(int).collect()
}

fn row_texts(v: &Value) -> Vec<(String, String)> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|r| {
            assert!(r[2].as_array().unwrap().is_empty());
            (
                r[0].as_str().unwrap().to_string(),
                r[1].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn found(rows: &[NegateRow]) -> Vec<(String, String)> {
    rows.iter()
        .map(|r| (r.text.clone(), r.number.to_string()))
        .collect()
}

#[test]
fn spell_tools() {
    let fx = load("spell_tools");
    let items = items(&fx.header["data"]);
    let spells = spells(&fx.header["data"]);
    let mut seen = 0;
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        let inp = &case["inputs"];
        let only_in_game = inp["globals"]["bOnlyInGame"].as_bool().unwrap_or(false);
        let avg = inp["globals"]["gAvgLevelMaxAllStats"]
            .as_f64()
            .unwrap_or(0.0);
        seen += 1;
        if id.starts_with("neg-") {
            // expected: FindNegates' result rows
            let st = &inp["steps"][0];
            let q = NegateQuery {
                spells: longs(&st["args"][0]["values"]),
                abilities: longs(&st["args"][1]["values"]),
                level: int(&st["args"][2]),
            };
            let got = found(&find_negates(&items, &spells, &q, only_in_game));
            let want = row_texts(&case["outputs"]["FindNegates.cap"]["rows"]);
            assert_eq!(got, want, "{id}");
        } else if id.starts_with("menu-") || id.starts_with("pane-") {
            let list = id.starts_with("menu-");
            let (proc, wl) = if list {
                ("mnuSpellsPopUpItem_Click", &inp["work_list"])
            } else {
                ("mnuAuxPopUpItem_Click", &inp["work_list"])
            };
            let cap = &case["outputs"][format!("{proc}.cap")];
            let text = if list { &wl["text"] } else { &wl["subtag"] };
            if val(text.as_str().unwrap()) <= 0.0 {
                // the handler does nothing for a row without a spell number
                assert!(cap["sp"].as_array().unwrap().is_empty(), "{id}");
                continue;
            }
            let answer = inp["inputbox"]
                .as_array()
                .and_then(|a| a.first())
                .map(|a| val(a.as_str().unwrap()));
            let spell_no = val(text.as_str().unwrap().split('@').next().unwrap()) as i32;
            let from = if list {
                NegateFrom::List {
                    spell: spell_no,
                    answer,
                }
            } else {
                NegateFrom::Pane {
                    tag: text.as_str().unwrap(),
                }
            };
            let mut plan = negate_query(&spells, from, avg);
            if plan.ask_level {
                // MME asks, and a missing answer is Cancel
                let from = NegateFrom::List {
                    spell: spell_no,
                    answer: Some(answer.unwrap_or(0.0)),
                };
                plan = negate_query(&spells, from, avg);
            }
            let q = plan.query;
            assert_eq!(q.spells, longs(&cap["sp"]), "{id} spells");
            assert_eq!(q.abilities, longs(&cap["ab"]), "{id} abilities");
            assert_eq!(q.level, int(&cap["lv"]), "{id} level");
            let got = found(&find_negates(&items, &spells, &q, true));
            assert_eq!(got, row_texts(&cap["rows"]), "{id} rows");
        } else if id.starts_with("cast-") {
            let st = &inp["steps"][0];
            let row = &st["args"][0]["cliplv"]["rows"][0];
            if !row["Sel"].as_bool().unwrap_or(false) {
                continue; // nothing selected: MME leaves quietly
            }
            let n = int(&Value::from(
                row["Text"].as_str().unwrap().parse::<i64>().unwrap(),
            ));
            let nm = names(&fx.header["data"]);
            let opt = LocationOptions {
                nmr_ver: 0.0,
                hide_record_numbers: inp["globals"]["bHideRecordNumbers"]
                    .as_bool()
                    .unwrap_or(false),
                global_filter: false,
                charm_tag: String::new(),
                attack_type: 0,
            };
            let out = what_casts(&spells, &nm, &mut NoLairs, &opt, n);
            let events = case["events"].as_array().cloned().unwrap_or_default();
            let msgs: Vec<&str> = events
                .iter()
                .filter(|e| e["kind"] == "msgbox")
                .map(|e| e["text"].as_str().unwrap())
                .collect();
            match out {
                WhatCasts::NoSuchSpell => assert_eq!(msgs, ["Cannot find Spell."], "{id}"),
                WhatCasts::Nothing => assert_eq!(msgs, ["Nothing."], "{id}"),
                WhatCasts::Found { caption_name, list } => {
                    assert!(msgs.is_empty(), "{id}");
                    let cap = &case["outputs"]["LookUpSpellCast.cap"];
                    assert_eq!(
                        format!("Spell {caption_name} is casted by:"),
                        cap["caption"].as_str().unwrap(),
                        "{id}"
                    );
                    let got: Vec<(String, String)> = list
                        .rows
                        .iter()
                        .map(|r| (r.text.clone(), r.tag.clone().unwrap_or_default()))
                        .collect();
                    assert_eq!(got, row_texts(&cap["rows"]), "{id}");
                    let sorts: Vec<usize> = events
                        .iter()
                        .filter(|e| e["kind"] == "sort_listview")
                        .map(|e| e["rows"].as_u64().unwrap() as usize)
                        .collect();
                    assert_eq!(list.sorts, sorts, "{id} sorts");
                }
            }
        } else {
            panic!("unknown case {id}");
        }
    }
    assert!(seen >= 90);
}
