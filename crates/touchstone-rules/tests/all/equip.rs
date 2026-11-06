// Fixture tests for the EQ tab family (FEAT-EQ): see docs/porting.md step 5.
// Expected values come from fixtures/rules/equip.json (frmMain's own procedures run by the oracle).
use serde_json::Value;
use std::collections::HashSet;
use touchstone_model::{AbilitySlot, Class, Info, Item, World, WorldParts};
use touchstone_rules::Profile;
use touchstone_rules::equip::*;
use touchstone_rules::gear::EQUIP_SLOTS;

use crate::fixture;

const P: Profile = Profile::Stock;

fn i(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
}

fn pairs<const N: usize>(v: &Value) -> [AbilitySlot; N] {
    let mut out = [AbilitySlot::default(); N];
    for (k, p) in v.as_array().unwrap().iter().enumerate() {
        out[k] = AbilitySlot {
            ability: i(&p[0]),
            value: i(&p[1]),
        };
    }
    out
}

fn world(data: &Value) -> World {
    let items = data["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut class_rest = [0; 10];
            for (k, c) in r["cr"].as_array().unwrap().iter().enumerate() {
                class_rest[k] = i(c);
            }
            Item {
                number: i(&r["n"]),
                name: r["name"].as_str().unwrap().to_string(),
                limit: 0,
                encum: 0,
                item_type: i(&r["t"]),
                use_count: 0,
                price: 0,
                currency: 0,
                min: 0,
                max: 0,
                armour_class: 0,
                damage_resist: 0,
                weapon_type: i(&r["wt"]),
                armour_type: i(&r["at"]),
                worn: i(&r["w"]),
                accy: 0,
                gettable: true,
                str_req: 0,
                speed: 0,
                not_droppable: false,
                destroy_on_death: false,
                retain_after_uses: false,
                class_rest,
                race_rest: [0; 10],
                negate_spell: [0; 10],
                abilities: pairs(&r["ab"]),
                in_game: i(&r["ig"]) != 0,
                obtained_from: None,
                references: None,
            }
        })
        .collect();
    let classes = data["classes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Class {
            number: i(&r["n"]),
            name: String::new(),
            min_hits: 0,
            max_hits: 0,
            exp_table: 0,
            magery_type: 0,
            magery_lvl: 0,
            weapon_type: i(&r["wt"]),
            armour_type: i(&r["at"]),
            combat_lvl: 0,
            abilities: pairs(&r["ab"]),
        })
        .collect();
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
        items,
        monsters: vec![],
        spells: vec![],
        rooms: vec![],
        shops: vec![],
        classes,
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

fn global_bool(case: &Value, name: &str) -> bool {
    case["inputs"]["globals"][name].as_bool().unwrap_or(false)
}

/// The global-filter character from the case's frmMain controls (the setup's where the case sets none).
fn character(header: &Value, case: &Value) -> UsableBy {
    let ctl = |name: &str| {
        let v = &case["inputs"]["forms"]["frmMain"][name];
        if v.is_null() {
            header["setup"]["forms"]["frmMain"][name].clone()
        } else {
            v.clone()
        }
    };
    let cls = ctl("cmbGlobalClass");
    let e = &cls["$array"]["0"];
    let class = if e["ListIndex"].as_i64().unwrap_or(-1) > 0 {
        i(&e["ItemData"]["1"])
    } else {
        0
    };
    let text = |v: &Value| touchstone_vb6::val(v.as_str().unwrap_or("")) as i32;
    UsableBy {
        level: text(&ctl("txtGlobalLevel")["$array"]["0"]["Text"]),
        class,
        race: 0,
        alignment: ctl("cmbGlobalAlignment")["ListIndex"].as_i64().unwrap_or(0) as i32,
        min_item_level: text(&ctl("txtGlobalMinLVL")["Text"]),
    }
}

#[test]
fn equip_fixture() {
    let fx = fixture::load("equip");
    let w = world(&fx.header["data"]);
    let mut fails = vec![];
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        if let Err(e) = run_case(&fx.header, &w, case) {
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

fn run_case(header: &Value, w: &World, case: &Value) -> Result<(), String> {
    let out = &case["outputs"];
    let mut eq = [0; EQUIP_SLOTS];
    let only_in_game = global_bool(case, "bOnlyInGame");
    let who = character(header, case);
    let opts = EquipOptions {
        use_2nd_wrist: global_bool(case, "bInvenUse2ndWrist"),
        unequip_if_equipped: false,
        dialogs: case["inputs"]["forms"]["frmMain"]["Enabled"]
            .as_bool()
            .unwrap_or(true),
    };
    // Items the combos hold beyond a full load: InvenAddEquip calls and equips.
    let mut added: HashSet<i32> = HashSet::new();
    let mut loaded = false;
    let mut rule = ListRule {
        only_in_game,
        usable_by: None,
    };
    let mut prompt = false;
    let mut messages: Vec<String> = vec![];
    let mut pure_lists = false;
    let mut flags: Option<[bool; EQUIP_SLOTS]> = None;
    let mut next: Option<NextWorn> = None;
    let mut exact_prompt = true;
    for st in case["inputs"]["steps"].as_array().unwrap() {
        pure_lists = false;
        if let Some(name) = st["select"].as_str() {
            assert_eq!(name, "cmbEquip");
            let slot = st["index"].as_u64().unwrap() as usize;
            let item = i(&st["item"]);
            let found = out[st["as"].as_str().unwrap()].as_bool().unwrap();
            let r = set_slot(
                P,
                w,
                &eq,
                slot,
                item,
                &ListRule {
                    only_in_game: false,
                    usable_by: None,
                },
            );
            // MME's combo holds only what was loaded or added; the port accepts what the mapping allows.
            let listed = item == 0
                || w.get_item(item).is_some_and(|it| {
                    list_slots(it.item_type, it.worn).contains(&slot)
                        && (added.contains(&item) || (loaded && (!only_in_game || it.in_game)))
                });
            if found != listed {
                return Err(format!(
                    "select {item} in {slot}: oracle found={found}, port listed={listed}"
                ));
            }
            if found {
                let o = r.map_err(|e| format!("select {item} in {slot}: {e}"))?;
                eq = o.equipped;
                prompt |= o.changed;
            } else if let Ok(o) = &r {
                // allowed by the mapping but never added: nothing to compare
                let _ = o;
            }
            continue;
        }
        match st["call"].as_str().unwrap() {
            "InvenSetupEquip" => {
                let o = clear_all(&eq);
                eq = o.equipped;
                prompt |= o.changed;
                added.clear();
                loaded = false;
            }
            "LoadInvenItems" => {
                let o = clear_all(&eq);
                eq = o.equipped;
                prompt |= o.changed;
                added.clear();
                loaded = true;
                rule.usable_by = None;
                pure_lists = true;
            }
            "FilterInvenItems" => {
                let remove = st["args"].as_array().is_some_and(|a| !a.is_empty());
                rule.usable_by = if remove { None } else { Some(who) };
                let o = reconcile(P, w, &eq, &rule);
                eq = o.equipped;
                prompt |= o.changed;
                added.clear();
                loaded = true;
                pure_lists = true;
            }
            "InvenAddEquip" => {
                added.insert(i(&st["args"][0]));
            }
            "InvenEquipItem" => {
                let o = equip_item(
                    w,
                    &eq,
                    i(&st["args"][0]),
                    &EquipOptions {
                        unequip_if_equipped: st["args"][2].as_bool().unwrap(),
                        ..opts
                    },
                );
                eq = o.equipped;
                prompt |= o.changed;
                messages.extend(o.messages);
                if o.slot.is_some() && o.changed {
                    added.insert(i(&st["args"][0]));
                }
                if let Some(it) = w.get_item(i(&st["args"][0]))
                    && o.slot.is_some()
                {
                    added.insert(it.number);
                }
            }
            "InvenClear" => {
                let mut hold = [false; EQUIP_SLOTS];
                let arr = &case["inputs"]["forms"]["frmMain"]["chkEquipHold"]["$array"];
                for (k, h) in hold.iter_mut().enumerate() {
                    *h = arr[k.to_string()]["Value"].as_i64().unwrap_or(0) == 1;
                }
                let o = clear_unheld(&eq, &hold);
                eq = o.equipped;
                exact_prompt = false;
            }
            "CalcCharacterStats" => {
                flags = Some(slot_flags(P, w, Some(&who), &eq));
            }
            "cmdArmNextSlot_Click" => {
                let frm = &case["inputs"]["forms"]["frmMain"];
                let count = frm["cmbArmourWorn"]["List"].as_array().unwrap().len() as i32;
                let start = i(&frm["cmbArmourWorn"]["ListIndex"]);
                let empty = frm["lblEmpty"]["Tag"].as_str().unwrap().as_bytes().to_vec();
                next = Some(next_armour_worn(count, start, |ix| {
                    empty.get(ix as usize) == Some(&b'1')
                }));
            }
            other => return Err(format!("unknown step {other}")),
        }
    }
    if let Some(n) = next {
        if i(&out["index"]) != n.index {
            return Err(format!("index {} vs {}", out["index"], n.index));
        }
        let expect_err = case["events"]
            .as_array()
            .is_some_and(|e| e.iter().any(|x| x["number"].as_i64() == Some(380)));
        if expect_err != n.error.is_some() {
            return Err(format!("error {expect_err} vs {:?}", n.error));
        }
        return Ok(());
    }
    if let Some(f) = flags {
        let want: Vec<bool> = out["flags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| i(x) != 0)
            .collect();
        if want != f.to_vec() {
            return Err(format!("flags {want:?} vs {f:?}"));
        }
    }
    if eq_of(&out["eq"]) != eq {
        return Err(format!("equipped {} vs {eq:?}", out["eq"]));
    }
    if exact_prompt && !loaded && out["prompt"].is_boolean() {
        let want = out["prompt"].as_bool().unwrap();
        if want != prompt {
            return Err(format!("bPromptSave {want} vs {prompt}"));
        }
    }
    let want_msgs: Vec<String> = case["events"]
        .as_array()
        .map(|e| {
            e.iter()
                .filter(|x| x["kind"] == "msgbox")
                .map(|x| x["text"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default();
    if want_msgs != messages {
        return Err(format!("messages {want_msgs:?} vs {messages:?}"));
    }
    if pure_lists {
        for slot in 0..EQUIP_SLOTS {
            let mut want: Vec<i32> = out["lists"][slot.to_string()]["data"]
                .as_array()
                .unwrap()
                .iter()
                .skip(1)
                .map(i)
                .collect();
            let mut got = slot_list(P, w, slot, &rule);
            want.sort_unstable();
            got.sort_unstable();
            if want != got {
                return Err(format!(
                    "list of slot {slot}: {} vs {} entries",
                    want.len(),
                    got.len()
                ));
            }
        }
    }
    Ok(())
}
