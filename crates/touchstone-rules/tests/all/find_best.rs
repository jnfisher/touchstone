// Fixture tests for Find Best / Next Best (FEAT-EQ): see docs/porting.md step 5.
// Expected values come from fixtures/rules/find_best.json (frmMain's own InvenFindBest run by the oracle).
use serde_json::Value;
use touchstone_model::{AbilitySlot, Info, Item, World, WorldParts};
use touchstone_rules::Profile;
use touchstone_rules::find_best::*;
use touchstone_rules::gear::EQUIP_SLOTS;

use crate::fixture;

const P: Profile = Profile::Stock;

fn i(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
}

fn world(data: &Value) -> World {
    let items = data["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut abilities = [AbilitySlot::default(); 20];
            for (k, p) in r[8].as_array().unwrap().iter().enumerate() {
                abilities[k] = AbilitySlot {
                    ability: i(&p[0]),
                    value: i(&p[1]),
                };
            }
            Item {
                number: i(&r[0]),
                name: String::new(),
                limit: i(&r[7]),
                encum: i(&r[3]),
                item_type: i(&r[1]),
                use_count: 0,
                price: 0,
                currency: 0,
                min: 0,
                max: 0,
                armour_class: i(&r[4]),
                damage_resist: i(&r[5]),
                weapon_type: i(&r[2]),
                armour_type: 0,
                worn: i(&r[9]),
                accy: i(&r[6]),
                gettable: true,
                str_req: 0,
                speed: 0,
                not_droppable: false,
                destroy_on_death: false,
                retain_after_uses: false,
                class_rest: [0; 10],
                race_rest: [0; 10],
                negate_spell: [0; 10],
                abilities,
                in_game: true,
                obtained_from: None,
                references: None,
            }
        })
        .collect();
    World::new(WorldParts {
        info: Info {
            nmr_version: String::new(),
            dat_file_version: String::new(),
            content_version: None,
            date: String::new(),
            time: String::new(),
            custom: String::new(),
            legit: false,
            disable_kai: false,
            update_url: String::new(),
        },
        items,
        monsters: vec![],
        spells: vec![],
        rooms: vec![],
        shops: vec![],
        classes: vec![],
        races: vec![],
        lairs: vec![],
        tbinfo: vec![],
    })
    .unwrap()
}

fn eq_of(v: &Value) -> [i32; EQUIP_SLOTS] {
    let mut out = [0; EQUIP_SLOTS];
    for (k, x) in v.as_array().unwrap().iter().enumerate() {
        out[k] = i(x);
    }
    out
}

fn menu_of(call: &str) -> Criterion {
    match call {
        "mnuFindBestArmour_Click" => Criterion::Armour,
        "mnuFindBestAttack_Click" => Criterion::Attack,
        "mnuFindBestResist_Click" => Criterion::Resist,
        "mnuFindBestStats_Click" => Criterion::Stats,
        "mnuFindBestMystics_Click" => Criterion::Mystics,
        "mnuFindBestAttrib_Click" => Criterion::Attribs,
        other => panic!("unknown menu handler {other}"),
    }
}

#[test]
fn find_best_fixture() {
    let fx = fixture::load("find_best");
    let w = world(&fx.header["data"]);
    let mut fails = vec![];
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        if let Err(e) = run_case(&w, case) {
            fails.push(format!("{id}: {e}"));
        }
    }
    assert!(
        fails.is_empty(),
        "{} of {} cases failed, first:\n{}",
        fails.len(),
        fx.cases.len(),
        fails[..fails.len().min(8)].join("\n")
    );
}

fn run_case(w: &World, case: &Value) -> Result<(), String> {
    let out = &case["outputs"];
    let frm = &case["inputs"]["forms"]["frmMain"];
    let globals = &case["inputs"]["globals"];
    // the combos as the oracle left them: FindBest never changes them
    let lists: Vec<Vec<i32>> = (0..EQUIP_SLOTS)
        .map(|s| {
            out["lists"][s.to_string()]["data"]
                .as_array()
                .unwrap()
                .iter()
                .skip(1)
                .map(i)
                .collect()
        })
        .collect();
    let mut hold = [false; EQUIP_SLOTS];
    for (k, h) in hold.iter_mut().enumerate() {
        *h = frm["chkEquipHold"]["$array"][k.to_string()]["Value"].as_i64() == Some(1);
    }
    let mut req = FindBestRequest {
        slot_lists: &lists,
        equipped: [0; EQUIP_SLOTS],
        hold,
        no_limited: frm["chkInvenNoLimited"]["Value"].as_i64() == Some(1),
        use_2nd_wrist: globals["bInvenUse2ndWrist"].as_bool().unwrap_or(false),
        excluded: None,
        last: LastFind::default(),
    };
    let mut messages = vec![];
    let mut errors = vec![];
    for st in case["inputs"]["steps"].as_array().unwrap() {
        if let Some(name) = st["select"].as_str() {
            assert_eq!(name, "cmbEquip");
            if out[st["as"].as_str().unwrap()].as_bool() == Some(true) {
                req.equipped[st["index"].as_u64().unwrap() as usize] = i(&st["item"]);
            }
            continue;
        }
        let call = st["call"].as_str().unwrap();
        let res = match call {
            "InvenSetupEquip" | "LoadInvenItems" | "FilterInvenItems" => {
                // every case sets up and filters before it wears anything
                assert!(req.equipped.iter().all(|&e| e == 0));
                continue;
            }
            "InvenAddEquip" => continue,
            "InvenEquipItem" => {
                // the oracle's list entries are all valid slots; wear it in the one list that holds it
                let n = i(&st["args"][0]);
                let o = touchstone_rules::equip::equip_item(
                    w,
                    &req.equipped,
                    n,
                    &touchstone_rules::equip::EquipOptions {
                        use_2nd_wrist: req.use_2nd_wrist,
                        ..Default::default()
                    },
                );
                req.equipped = o.equipped;
                continue;
            }
            "cmdEquipButtons_Click" if i(&st["args"][0]) == 11 => continue,
            "cmdEquipButtons_Click" => next_best(P, w, &req),
            menu => find_best(P, w, &req, menu_of(menu), i(&st["args"][0])),
        }
        .map_err(|e| format!("{call}: {e}"))?;
        // MME: the capture taken right after the step
        let cap = &out[format!("{}.cap", st["as"].as_str().unwrap())];
        if eq_of(&cap["eq"]) != res.equipped {
            return Err(format!("equipped {} vs {:?}", cap["eq"], res.equipped));
        }
        let last = cap["last"].as_array().unwrap();
        if (i(&last[0]), i(&last[1])) != (res.last.index, res.last.criterion as i32) {
            return Err(format!("last {last:?} vs {:?}", res.last));
        }
        let want_excl: Vec<i32> = cap["excl"].as_array().unwrap().iter().map(i).collect();
        let got_excl: Vec<i32> = match &res.excluded {
            None => vec![],
            Some(l) => std::iter::once(0).chain(l.iter().copied()).collect(),
        };
        if want_excl != got_excl {
            return Err(format!("excluded {want_excl:?} vs {got_excl:?}"));
        }
        messages.extend(res.messages.clone());
        errors.extend(res.error);
        req.equipped = res.equipped;
        req.excluded = res.excluded;
        req.last = res.last;
    }
    let events = case["events"].as_array().cloned().unwrap_or_default();
    let want_msgs: Vec<&str> = events
        .iter()
        .filter(|e| e["kind"] == "msgbox")
        .map(|e| e["text"].as_str().unwrap())
        .collect();
    if want_msgs != messages {
        return Err(format!("messages {want_msgs:?} vs {messages:?}"));
    }
    let want_errs: Vec<i32> = events
        .iter()
        .filter(|e| e["kind"] == "handled_error")
        .map(|e| i(&e["number"]))
        .collect();
    if want_errs != errors {
        return Err(format!("errors {want_errs:?} vs {errors:?}"));
    }
    Ok(())
}

#[test]
fn menu_lists_every_entry_in_designer_order() {
    let m = menu(P);
    assert_eq!(m.len(), 44);
    assert_eq!(m[0].label, "AC/DR Combo");
    assert!(m.iter().all(|e| !e.greater_only));
    assert!(MENU.iter().any(|e| e.label == "VileWard" && e.greater_only));
}
