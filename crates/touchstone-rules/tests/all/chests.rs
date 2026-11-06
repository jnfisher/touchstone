// Fixture tests for the chests family (card P4-32).
// Expected values: fixtures/rules/chests.json, produced by the oracle running MME's own
// cmdSundryChests_Click (and GetChestItems) against the stock database plus invented chests.
use crate::fixture::load;
use serde_json::Value;
use touchstone_model::{AbilitySlot, Info, Item, Spell, TbInfo, World, WorldParts};
use touchstone_rules::chests::*;

fn int(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
}

fn item(r: &Value) -> Item {
    let mut abilities = [AbilitySlot::default(); 20];
    for (k, v) in r[3].as_array().unwrap().iter().enumerate() {
        if int(v) != 0 {
            abilities[k] = AbilitySlot {
                ability: 43,
                value: int(v),
            };
        }
    }
    Item {
        number: int(&r[0]),
        name: r[1].as_str().unwrap().to_string(),
        limit: 0,
        encum: 0,
        item_type: int(&r[2]),
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
        abilities,
        in_game: false,
        obtained_from: None,
        references: None,
    }
}

fn spell(r: &Value) -> Spell {
    let mut abilities = [AbilitySlot::default(); 10];
    for (k, p) in r[3].as_array().unwrap().chunks(2).enumerate() {
        abilities[k] = AbilitySlot {
            ability: int(&p[0]),
            value: int(&p[1]),
        };
    }
    Spell {
        number: int(&r[0]),
        name: None,
        short: None,
        req_level: 0,
        energy_cost: 0,
        mana_cost: 0,
        min_base: int(&r[1]),
        max_base: int(&r[2]),
        diff: 0,
        type_of_resists: 0,
        targets: 0,
        dur: 0,
        att_type: 0,
        magery: 0,
        magery_lvl: 0,
        cap: 0,
        max_inc_lvls: 0,
        max_inc: 0,
        min_inc_lvls: 0,
        min_inc: 0,
        dur_inc_lvls: 0,
        dur_inc: 0,
        abilities,
        learnable: false,
        learned_from: None,
        casted_by: None,
        classes: None,
    }
}

fn world(data: &Value) -> World {
    let list = |k: &str| data[k].as_array().unwrap().clone();
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
        },
        items: list("items").iter().map(item).collect(),
        monsters: Vec::new(),
        spells: list("spells").iter().map(spell).collect(),
        rooms: Vec::new(),
        shops: Vec::new(),
        classes: Vec::new(),
        races: Vec::new(),
        lairs: Vec::new(),
        tbinfo: list("tb")
            .iter()
            .map(|r| TbInfo {
                number: int(&r[0]),
                link_to: 0,
                action: Some(r[1].as_str().unwrap().to_string()),
                called_from: String::new(),
            })
            .collect(),
    })
    .unwrap()
}

#[test]
fn chests() {
    let fx = load("chests");
    let w = world(&fx.header["data"]);
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        let lv = &case["inputs"]["forms"]["frmMain"]["lvOtherItems"]["$cliplv"]["rows"][0];
        if !lv["Sel"].as_bool().unwrap() {
            continue; // nothing selected: MME leaves quietly
        }
        let n: i32 = lv["Text"].as_str().unwrap().parse().unwrap();
        let hide = case["inputs"]["globals"]["bHideRecordNumbers"]
            .as_bool()
            .unwrap_or(false);
        let events = case["events"].as_array().cloned().unwrap_or_default();
        let msg = events
            .iter()
            .find(|e| e["kind"] == "msgbox")
            .map(|e| e["text"].as_str().unwrap().to_string());
        // expected: lvOtherItemLoc's rows [percent text, tag, [[item text, item number, null]]]
        let want: Vec<(String, String, String, String)> =
            case["outputs"]["cmdSundryChests_Click.cap"]["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    (
                        r[0].as_str().unwrap().to_string(),
                        r[1].as_str().unwrap().to_string(),
                        r[2][0][0].as_str().unwrap().to_string(),
                        r[2][0][1].as_str().unwrap().to_string(),
                    )
                })
                .collect();
        match chest_contents(&w, n, hide) {
            ChestResult::NoSuchItem(k) => {
                assert_eq!(msg, Some(format!("Item #{k} not found.")), "{id}");
            }
            ChestResult::NotContainer(name) => {
                assert_eq!(msg, Some(format!("{name} is not a container.")), "{id}");
            }
            ChestResult::NoChestData => {
                assert_eq!(msg, Some("Failed to find chest data.".to_string()), "{id}");
            }
            ChestResult::Rows(rows) => {
                assert_eq!(msg, None, "{id}");
                let got: Vec<_> = rows
                    .iter()
                    .map(|r| {
                        (
                            r.text.clone(),
                            r.text.trim_end_matches('%').to_string(),
                            r.item_text.clone(),
                            r.item.to_string(),
                        )
                    })
                    .collect();
                assert_eq!(got, want, "{id}");
                let sort = events.iter().any(|e| e["kind"] == "sort_listview_by_tag");
                assert_eq!(sort, !rows.is_empty(), "{id} sort request");
            }
        }
    }
}
