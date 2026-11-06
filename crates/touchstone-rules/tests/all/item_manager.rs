// Fixture tests for the item_manager family: see docs/porting.md step 5.
// Expected values come from fixtures/rules/item_manager.json (modListViewExt LV_SetActionCell,
// LV_CopySelectedActionsToClipboard, LV_DeleteSelected, LV_InvertSelection run by the oracle over
// the stock Items table), plus hand cases for SaveCharacter's list loop below.

use crate::fixture;
use serde_json::Value;
use touchstone_model::{Info, Item, World, WorldParts};
use touchstone_rules::RulesError;
use touchstone_rules::game_text::{ListRow, SubItem};
use touchstone_rules::item_manager::{
    copy_selected_actions, delete_selected, invert_selection, saved_stacks, set_action_cell,
};

fn i(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
}

fn world(data: &Value) -> World {
    let items = data["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Item {
            number: i(&r["n"]),
            name: r["name"].as_str().unwrap().to_string(),
            limit: 0,
            encum: i(&r["encum"]),
            item_type: i(&r["type"]),
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
            gettable: true,
            str_req: 0,
            speed: 0,
            not_droppable: false,
            destroy_on_death: false,
            retain_after_uses: false,
            class_rest: [0; 10],
            race_rest: [0; 10],
            negate_spell: [0; 10],
            abilities: Default::default(),
            in_game: true,
            obtained_from: None,
            references: None,
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

fn list(v: &Value) -> (Vec<ListRow>, Vec<bool>) {
    let mut sel = Vec::new();
    let rows = v["$listview"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            sel.push(r["Sel"].as_bool().unwrap_or(false));
            ListRow {
                text: r["Text"].as_str().unwrap().to_string(),
                subs: r["Sub"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|x| SubItem::new(x.as_str().unwrap()))
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        })
        .collect();
    (rows, sel)
}

/// The oracle's dump of the ListView: [{text, subs: [{t}], sel?}].
fn dump(rows: &[ListRow], sel: &[bool]) -> Value {
    Value::Array(
        rows.iter()
            .enumerate()
            .map(|(k, r)| {
                let mut o = serde_json::json!({
                    "text": r.text,
                    "subs": r.subs.iter().map(|s| serde_json::json!({"t": s.text})).collect::<Vec<_>>(),
                });
                if sel.get(k).copied().unwrap_or(false) {
                    o["sel"] = Value::Bool(true);
                }
                o
            })
            .collect(),
    )
}

fn run(w: &World, case: &Value) -> Result<(), String> {
    let inp = &case["inputs"];
    let (mut rows, mut sel) = list(&inp["forms"]["frmMain"]["lvItemManager"]);
    let mut weight: i32 = inp["forms"]["frmMain"]["txtInvenAddWeight"]["Text"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let step = &inp["steps"][0];
    let proc = step["call"].as_str().unwrap();
    let mut answers: Vec<i64> = inp["msgbox"]
        .as_array()
        .map(|a| a.iter().map(|x| x.as_i64().unwrap()).collect())
        .unwrap_or_default();
    answers.reverse();
    let mut asked: Vec<String> = Vec::new();
    let mut clip = String::new();
    let mut error: Option<i32> = None;
    // the default button: Yes for the weight question, No for the delete confirmation
    let mut ask = |text: &str, default: bool| -> bool {
        asked.push(text.to_string());
        answers.pop().map_or(default, |a| a == 6)
    };
    match proc {
        "LV_SetActionCell" => {
            let action = i(&step["args"][1]);
            let out = set_action_cell(w, &mut rows, &sel, action, &mut weight, &mut |t| {
                ask(t, true)
            });
            if let Some(RulesError::Vb6(n)) = out.error {
                error = Some(n);
            }
        }
        "LV_CopySelectedActionsToClipboard" => match copy_selected_actions(w, &rows, &sel) {
            Ok(Some(t)) => clip = t,
            Ok(None) => {}
            Err(RulesError::Vb6(n)) => error = Some(n),
            Err(e) => return Err(format!("{e:?}")),
        },
        "LV_DeleteSelected" => {
            let out = delete_selected(&mut rows, &sel, &mut || {
                ask("Are you sure you want to delete the selected rows?", false)
            });
            if !out.declined {
                sel = vec![false; rows.len()];
                if let Some(n) = out.next_selected {
                    sel[n] = true;
                }
            }
        }
        "LV_InvertSelection" => invert_selection(&mut sel, rows.len()),
        other => return Err(format!("unknown call {other}")),
    }
    let out = &case["outputs"];
    // an ActionOutcome error ends the loop quietly (MME's handler), so only copy reports one
    let want_err = case["error"]["number"].as_i64().map(|n| n as i32);
    let want_err = if proc == "LV_SetActionCell" {
        None
    } else {
        want_err
    };
    if error != want_err && proc != "LV_SetActionCell" {
        return Err(format!("error got {error:?}, want {want_err:?}"));
    }
    let got = dump(&rows, &sel);
    if got != out["lv"] {
        return Err(format!("rows got {got}\n  want {}", out["lv"]));
    }
    if out["weight"].as_str().unwrap() != weight.to_string() {
        return Err(format!("weight got {weight}, want {}", out["weight"]));
    }
    if out["clip"].as_str().unwrap() != clip {
        return Err(format!("clipboard got {clip:?}, want {}", out["clip"]));
    }
    let want_asked: Vec<&str> = case["events"]
        .as_array()
        .map(|a| a.iter().map(|e| e["text"].as_str().unwrap()).collect())
        .unwrap_or_default();
    if asked != want_asked {
        return Err(format!("boxes got {asked:?}, want {want_asked:?}"));
    }
    Ok(())
}

#[test]
fn item_manager_fixture() {
    let fx = fixture::load("item_manager");
    let w = world(&fx.header["data"]);
    let mut bad = Vec::new();
    for case in &fx.cases {
        if let Err(e) = run(&w, case) {
            bad.push(format!("{}: {e}\n  inputs {}", case["id"], case["inputs"]));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} cases differ; first:\n{}",
        bad.len(),
        fx.cases.len(),
        bad.iter().take(3).cloned().collect::<Vec<_>>().join("\n")
    );
}

fn row(item: &str, flag: &str) -> ListRow {
    ListRow {
        text: item.to_string(),
        subs: vec![SubItem::new("name"), SubItem::new(flag), SubItem::new("1")],
    }
}

// SaveCharacter's IM_CARRIED / IM_STASH loops (frmMain SaveCharacter): the flag decides, a
// trailing " xN" is the quantity, 50 rows at most.
#[test]
fn saved_stacks_follow_save_character() {
    let rows = [
        row("5", "CARRIED"),
        row("6", "carried x3"),
        row("7", "STASH x2"),
        row("8", "DROP"),
        row("9", "CARRIED x0"),
        row("10", "CARRIED X4"),
        row("11", "stash"),
    ];
    let (c, s) = saved_stacks(&rows).unwrap();
    let pairs = |v: &[touchstone_rules::mmec::ItemStack]| -> Vec<(i32, i32)> {
        v.iter().map(|x| (x.item, x.qty)).collect()
    };
    // " X4" matches the case-insensitive InStr but not the binary Split: quantity 1
    assert_eq!(pairs(&c), vec![(5, 1), (6, 3), (9, 1), (10, 1)]);
    assert_eq!(pairs(&s), vec![(7, 2), (11, 1)]);
}

// SaveCharacter: "If x > 50 Then Exit For" after the 50th row of a list.
#[test]
fn saved_stacks_stop_at_fifty() {
    let rows: Vec<ListRow> = (1..=60).map(|n| row(&n.to_string(), "CARRIED")).collect();
    let (c, s) = saved_stacks(&rows).unwrap();
    assert_eq!(c.len(), 50);
    assert_eq!(c[49].item, 50);
    assert!(s.is_empty());
}

// SaveCharacter keeps y as an Integer: a quantity past 32767 is run-time error 6.
#[test]
fn saved_stacks_overflow_is_error_6() {
    let rows = [row("5", "CARRIED x40000")];
    assert_eq!(saved_stacks(&rows).unwrap_err(), RulesError::Vb6(6));
}
