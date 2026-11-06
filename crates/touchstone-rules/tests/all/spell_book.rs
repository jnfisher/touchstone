// Fixture tests for the spell_book family (card P4-32).
// Expected values: fixtures/rules/spell_book.json, produced by the oracle running MME's own frmSpellBook
// cmdListSpells_Click, cmbClass_Click and CopyGlobalChar against the stock database. The rows' cells are the
// rows family's; here the spells listed, the profile choice and the sort are checked.
use crate::fixture::load;
use serde_json::Value;
use touchstone_model::{AbilitySlot, Class, Spell};
use touchstone_rules::Profile;
use touchstone_rules::spell_book::*;
use touchstone_rules::spell_cast::UsableEnv;
use touchstone_vb6::val;

fn int(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
}

fn spells(data: &Value) -> Vec<Spell> {
    data["spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut abilities = [AbilitySlot::default(); 10];
            for (k, p) in r[9].as_array().unwrap().chunks(2).enumerate() {
                abilities[k] = AbilitySlot {
                    ability: int(&p[0]),
                    value: int(&p[1]),
                };
            }
            // Only the lengths of Learned From and Casted By matter to SpellIsUsable.
            let text = |len: i32| (len > 0).then(|| "x".repeat(len as usize));
            let classes = r[8].as_str().unwrap();
            Spell {
                number: int(&r[0]),
                name: Some(r[1].as_str().unwrap().to_string()),
                short: None,
                req_level: int(&r[7]),
                energy_cost: 0,
                mana_cost: 0,
                min_base: 0,
                max_base: 0,
                diff: 0,
                type_of_resists: 0,
                targets: 0,
                dur: 0,
                att_type: 0,
                magery: int(&r[5]),
                magery_lvl: int(&r[6]),
                cap: 0,
                max_inc_lvls: 0,
                max_inc: 0,
                min_inc_lvls: 0,
                min_inc: 0,
                dur_inc_lvls: 0,
                dur_inc: 0,
                abilities,
                learnable: int(&r[2]) != 0,
                learned_from: text(int(&r[3])),
                casted_by: text(int(&r[4])),
                classes: (!classes.is_empty()).then(|| classes.to_string()),
            }
        })
        .collect()
}

fn classes(data: &Value) -> Vec<Class> {
    data["classes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Class {
            number: int(&r[0]),
            name: String::new(),
            min_hits: 0,
            max_hits: 0,
            exp_table: 0,
            magery_type: int(&r[1]),
            magery_lvl: int(&r[2]),
            weapon_type: 0,
            armour_type: 0,
            combat_lvl: 0,
            abilities: [AbilitySlot::default(); 10],
        })
        .collect()
}

/// A combo given to the oracle as ItemData values and a selection.
fn selected(c: &Value) -> (Vec<i32>, i32) {
    let items = c["Items"].as_array().unwrap().iter().map(int).collect();
    (items, int(&c["ListIndex"]))
}

fn cap<'a>(case: &'a Value, call: &str) -> &'a Value {
    &case["outputs"][format!("{call}.cap")]
}

#[test]
fn spell_book() {
    let fx = load("spell_book");
    let spells = spells(&fx.header["data"]);
    let classes = classes(&fx.header["data"]);
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        let g = &case["inputs"]["globals"];
        let f = &case["inputs"]["forms"]["frmMain"];
        let (book_classes, class_idx) = selected(&f["cmbClass"]);
        let class = book_classes[class_idx as usize];
        let events = case["events"].as_array().cloned().unwrap_or_default();
        let call = case["inputs"]["steps"][0]["call"].as_str().unwrap();
        let ix = |name: &str| int(&f[name]["ListIndex"]);
        match call {
            "cmdListSpells_Click" => {
                let env = UsableEnv {
                    profile: Profile::Stock,
                    spells: &spells,
                    classes: &classes,
                    nmr_ver: g["nNMRVer"].as_f64().unwrap(),
                    only_in_game: g["bOnlyInGame"].as_bool().unwrap(),
                    disable_kai_autolearn: g["bDisableKaiAutolearn"].as_bool().unwrap_or(false),
                };
                let q = BookQuery {
                    class,
                    level: val(f["txtLevel"]["Text"].as_str().unwrap()),
                    alignment: ix("cmbAlignment"),
                };
                let filter_on = int(&f["chkGlobalFilter"]["Value"]) == 1;
                let (g0_items, g0_idx) = selected(&f["cmbGlobalClass"]["$array"]["0"]);
                let globals = BookGlobals {
                    filter_on,
                    global_level_set: val(f["txtGlobalLevel"]["$array"]["1"]["Text"]
                        .as_str()
                        .unwrap())
                        > 0.0,
                    global_class: if g0_idx > 0 {
                        g0_items[g0_idx as usize]
                    } else {
                        0
                    },
                    char_magery: g.get("nGlobalCharMagery").map_or(0, int),
                    char_magery_lvl: g.get("nGlobalCharMageryLVL").map_or(0, int),
                };
                let list = list_spells(&env, &q, filter_on);
                let want: Vec<i32> = cap(case, call)["rows"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| r.as_str().unwrap().parse().unwrap())
                    .collect();
                assert_eq!(list.spells, want, "{id}");
                let has_sort = events.iter().any(|e| e["kind"] == "lv_refresh_sort");
                assert_eq!(has_sort, !list.aborted, "{id} sort");
                if let Some(e) = events.iter().find(|e| e["kind"] == "lv_refresh_sort") {
                    assert_eq!(
                        e["ascending"].as_bool().unwrap(),
                        list.sort_ascending,
                        "{id} order"
                    );
                    assert_eq!(int(&e["column"]), 5, "{id}");
                }
                if !list.aborted {
                    let uses = uses_character(
                        &globals,
                        &q,
                        ix("cmbSpellMagery"),
                        ix("cmbSpellMageryLevel"),
                    );
                    assert_eq!(
                        uses,
                        cap(case, call)["char"].as_bool().unwrap(),
                        "{id} character"
                    );
                }
            }
            "cmbClass_Click" => {
                let pick = pick_class(
                    &classes,
                    class,
                    int(&f["cmbSpellMageryLevel"]["ListCount"]),
                    int(&f["cmbSpellMagery"]["ListCount"]),
                );
                let c = cap(case, call);
                // a box that was not set keeps the index the case started with
                let (lvl, mg) = match pick {
                    ClassPick::Boxes {
                        magery_level,
                        magery,
                    } => (
                        magery_level.unwrap_or(ix("cmbSpellMageryLevel")),
                        magery.unwrap_or(ix("cmbSpellMagery")),
                    ),
                    ClassPick::Nothing => (ix("cmbSpellMageryLevel"), ix("cmbSpellMagery")),
                    ClassPick::NotFound => panic!("{id}: class not found"),
                };
                assert_eq!((lvl, mg), (int(&c["mlvl"]), int(&c["m"])), "{id}");
            }
            "CopyGlobalChar" => {
                let (g1_items, g1_idx) = selected(&f["cmbGlobalClass"]["$array"]["1"]);
                let (_, g0_idx) = selected(&f["cmbGlobalClass"]["$array"]["0"]);
                let level_text = f["txtGlobalLevel"]["$array"]["1"]["Text"].as_str().unwrap();
                let out = copy_global_char(&GlobalCopy {
                    filter_on: int(&f["chkGlobalFilter"]["Value"]) == 1,
                    alignment: ix("cmbGlobalAlignment"),
                    level_text,
                    class_index: g1_idx,
                    class_item_data: g1_items[g0_idx as usize],
                    book_classes: &book_classes,
                });
                let c = cap(case, call);
                let al = out.alignment.unwrap_or(ix("cmbAlignment"));
                let lvl = out
                    .level
                    .unwrap_or_else(|| f["txtLevel"]["Text"].as_str().unwrap().to_string());
                let cls = out.class_index.map_or(class_idx, |i| i as i32);
                assert_eq!(
                    (al, lvl.as_str(), cls),
                    (int(&c["al"]), c["lvl"].as_str().unwrap(), int(&c["cls"])),
                    "{id}"
                );
            }
            other => panic!("unknown call {other}"),
        }
    }
}
