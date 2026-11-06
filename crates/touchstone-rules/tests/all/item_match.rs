// Fixture tests for the item_match family: see docs/porting.md step 5.
// Expected values come from fixtures/rules/item_match.json (modItemParse, modListViewExt and
// modMMudDatabase run by the oracle over the stock database plus invented rows).

use crate::fixture;
use serde_json::Value;
use touchstone_model::{AbilitySlot, Class, Info, Item, Room, Shop, ShopSlot, World, WorldParts};
use touchstone_rules::RulesError;
use touchstone_rules::game_text::{
    Event, ItemMatch, ItemParseResult, ListRow, MatchEnv, SubItem, SubTag,
    evaluate_best_price_for_hit, extract_first_number, extract_shops_from_obtained_from,
    get_best_shop_num_for_item, get_items_by_exact_name, get_shop_room_names,
    lv_add_row_by_item_number, parse_action_and_qty, populate_item_manager_from_parsed,
    sum_inv_or_carried_qty_for_name,
};
use touchstone_rules::gear::{Character, EQUIP_SLOTS};
use touchstone_vb6 as vb;

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

fn opt_str(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

/// The rows the cases can reach, from the fixture header's `data`.
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
                encum: i(&r["enc"]),
                item_type: i(&r["t"]),
                use_count: 0,
                price: i(&r["price"]),
                currency: i(&r["cur"]),
                min: 0,
                max: 0,
                armour_class: 0,
                damage_resist: 0,
                weapon_type: i(&r["wt"]),
                armour_type: i(&r["at"]),
                worn: i(&r["w"]),
                accy: 0,
                gettable: i(&r["g"]) == 1,
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
                obtained_from: opt_str(&r["of"]),
                references: None,
            }
        })
        .collect();
    let shops = data["shops"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Shop {
            number: i(&r["n"]),
            name: r["name"].as_str().unwrap().to_string(),
            shop_type: 0,
            min_lvl: 0,
            max_lvl: 0,
            markup_pct: i(&r["markup"]),
            class_rest: 0,
            stock: [ShopSlot::default(); 20],
            in_game: true,
            assigned_to: opt_str(&r["asg"]),
        })
        .collect();
    let rooms = data["rooms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Room {
            map_number: i(&r["m"]),
            room_number: i(&r["r"]),
            name: opt_str(&r["name"]),
            light: 0,
            shop: 0,
            npc: 0,
            cmd: 0,
            spell: 0,
            lair: None,
            delay: 0,
            placed: None,
            n: String::new(),
            s: String::new(),
            e: String::new(),
            w: String::new(),
            ne: String::new(),
            nw: String::new(),
            se: String::new(),
            sw: String::new(),
            u: String::new(),
            d: String::new(),
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
        rooms,
        shops,
        classes,
        races: vec![],
        lairs: vec![],
        tbinfo: vec![],
    })
    .unwrap()
}

/// frmMain's controls: the setup's, with the case's replacing those of the same name.
struct Forms<'a> {
    setup: &'a Value,
    case: &'a Value,
}

impl Forms<'_> {
    fn ctl(&self, name: &str) -> &Value {
        let v = &self.case[name];
        if v.is_null() { &self.setup[name] } else { v }
    }
    fn elem(&self, name: &str, idx: usize) -> &Value {
        &self.ctl(name)["$array"][idx.to_string()]
    }
    fn character(&self) -> Character {
        let combo = self.elem("cmbGlobalClass", 0);
        let class = if combo["ListIndex"].as_i64().unwrap_or(-1) > 0 {
            i(&combo["ItemData"]["1"])
        } else {
            0
        };
        Character {
            level: vb::val(
                self.elem("txtGlobalLevel", 0)["Text"]
                    .as_str()
                    .unwrap_or(""),
            )
            .round() as i32,
            class,
            race: 0,
            alignment: self.ctl("cmbGlobalAlignment")["ListIndex"]
                .as_i64()
                .unwrap_or(0) as i32,
            min_item_level: vb::val(self.ctl("txtGlobalMinLVL")["Text"].as_str().unwrap_or(""))
                as i32,
        }
    }
    fn charm_tag(&self) -> &str {
        self.elem("txtCharStats", 5)["Tag"].as_str().unwrap_or("")
    }
    fn rows(&self) -> Vec<ListRow> {
        rows_from_listview(&self.ctl("lvItemManager")["$listview"])
    }
}

fn rows_from_listview(v: &Value) -> Vec<ListRow> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|r| ListRow {
            text: r["Text"].as_str().unwrap_or("").to_string(),
            subs: r["Sub"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|x| SubItem::new(x.as_str().unwrap()))
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect()
}

fn equipped(case: &Value) -> [i32; EQUIP_SLOTS] {
    let mut out = [0; EQUIP_SLOTS];
    if let Some(a) = case["inputs"]["globals"]["nEquippedItem"].as_array() {
        for (k, v) in a.iter().enumerate() {
            out[k] = i(v);
        }
    }
    out
}

fn flag(case: &Value, name: &str) -> bool {
    case["inputs"]["globals"][name].as_bool().unwrap_or(false)
}

fn want_error(case: &Value) -> Option<i32> {
    case["error"]["number"].as_i64().map(|n| n as i32)
}

fn step(case: &Value) -> &Value {
    &case["inputs"]["steps"][0]
}

fn args(case: &Value) -> &Vec<Value> {
    if case["inputs"]["proc"].is_string() {
        case["inputs"]["args"].as_array().unwrap()
    } else {
        step(case)["args"].as_array().unwrap()
    }
}

/// The result of a call that may have raised: either the value or the VB6 error number.
fn check_result<T: PartialEq + std::fmt::Debug>(
    case: &Value,
    got: Result<T, RulesError>,
    want: impl Fn(&Value) -> T,
) -> Result<(), String> {
    match (got, want_error(case)) {
        (Err(RulesError::Vb6(n)), Some(w)) if n == w => Ok(()),
        (Err(e), w) => Err(format!("error {e:?}, want {w:?}")),
        (Ok(_), Some(w)) => Err(format!("no error, want {w}")),
        (Ok(g), None) => {
            let w = want(&case["outputs"]["x"]);
            if g == w {
                Ok(())
            } else {
                Err(format!("got {g:?}, want {w:?}"))
            }
        }
    }
}

fn item_match_from(v: &Value) -> ItemMatch {
    ItemMatch {
        number: i(&v["number"]),
        name: v["name"].as_str().unwrap().to_string(),
        item_type: i(&v["itemtype"]),
        worn: i(&v["worn"]),
        weapon_type: i(&v["weapontype"]),
        encum: i(&v["encum"]),
        obtained_from: v["obtainedfrom"].as_str().unwrap().to_string(),
        gettable: i(&v["gettable"]),
    }
}

/// The ItemMatch a case passes as a Type argument: `{"udt": "ItemMatch", "fields": {...}}`.
fn item_match_arg(v: &Value) -> ItemMatch {
    let f = &v["fields"];
    ItemMatch {
        number: i(&f["Number"]),
        name: f["name"].as_str().unwrap().to_string(),
        item_type: i(&f["ItemType"]),
        worn: i(&f["Worn"]),
        weapon_type: i(&f["WeaponType"]),
        encum: i(&f["encum"]),
        obtained_from: f["ObtainedFrom"].as_str().unwrap().to_string(),
        gettable: i(&f["Gettable"]),
    }
}

fn parsed_arg(v: &Value) -> ItemParseResult {
    let f = &v["fields"];
    let list = |k: &str| -> Vec<String> {
        f.get(k)
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|s| s.as_str().unwrap().to_string()).collect())
            .unwrap_or_default()
    };
    ItemParseResult {
        equipped: list("sEquipped"),
        inventory: list("sInventory"),
        keys: list("sKeys"),
        ground: list("sGround"),
    }
}

/// The fixture's ListView dump: [{"text", "subs": [{"t", "g"?}]}].
fn dump_rows(rows: &[ListRow]) -> Value {
    let tag = |t: &Option<SubTag>| match t {
        None => None,
        Some(SubTag::Int(n)) => Some(Value::from(*n)),
        Some(SubTag::Text(s)) => Some(Value::from(s.clone())),
    };
    Value::Array(
        rows.iter()
            .map(|r| {
                let subs: Vec<Value> = r
                    .subs
                    .iter()
                    .map(|s| {
                        let mut m = serde_json::Map::new();
                        m.insert("t".into(), Value::from(s.text.clone()));
                        if let Some(g) = tag(&s.tag) {
                            m.insert("g".into(), g);
                        }
                        Value::Object(m)
                    })
                    .collect();
                serde_json::json!({"text": r.text, "subs": subs})
            })
            .collect(),
    )
}

/// The events the port models: message boxes and the two handled errors of the Item Manager code.
fn want_events(case: &Value) -> Vec<Event> {
    case["events"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|e| match e["kind"].as_str().unwrap() {
                    "msgbox" => Some(Event::MsgBox(e["text"].as_str().unwrap().to_string())),
                    "handled_error" => match e["source"].as_str().unwrap() {
                        "LV_AddRowByItemNumber" => Some(Event::HandledError {
                            source: "LV_AddRowByItemNumber",
                            number: e["number"].as_u64().unwrap() as u32,
                        }),
                        "GetBestShopNumForItem" => Some(Event::HandledError {
                            source: "GetBestShopNumForItem",
                            number: e["number"].as_u64().unwrap() as u32,
                        }),
                        // GetShopMarkup and ExtractMapRoom swallow their own errors inside the ported code
                        _ => None,
                    },
                    other => panic!("event kind {other}"),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn run(world: &World, header: &Value, case: &Value) -> Result<(), String> {
    let inputs = &case["inputs"];
    let forms = Forms {
        setup: &header["setup"]["forms"]["frmMain"],
        case: &inputs["forms"]["frmMain"],
    };
    let who = forms.character();
    let eq = equipped(case);
    let env = MatchEnv {
        world,
        only_in_game: flag(case, "bOnlyInGame"),
        hide_record_numbers: flag(case, "bHideRecordNumbers"),
        charm_tag: forms.charm_tag(),
        who: &who,
        equipped: &eq,
    };
    let call = inputs["proc"]
        .as_str()
        .or_else(|| step(case)["call"].as_str())
        .unwrap();
    let a = args(case);
    let out = &case["outputs"];
    match call {
        "ExtractFirstNumber" => {
            check_result(case, extract_first_number(a[0].as_str().unwrap()), |_| 0).or_else(|e| {
                // the proc form stores its value as outputs.result
                let w = i(&out["result"]);
                match extract_first_number(a[0].as_str().unwrap()) {
                    Ok(g) if g == w && want_error(case).is_none() => Ok(()),
                    _ => Err(e),
                }
            })
        }
        "ExtractShopsFromObtainedFrom" => {
            let got = extract_shops_from_obtained_from(a[0].as_str().unwrap());
            match (got, want_error(case)) {
                (Err(RulesError::Vb6(n)), Some(w)) if n == w => Ok(()),
                (Err(e), w) => Err(format!("error {e:?}, want {w:?}")),
                (Ok(_), Some(w)) => Err(format!("no error, want {w}")),
                (Ok(g), None) => {
                    let want = out["x.refs"][0].as_array().unwrap();
                    let same = i(&out["x"]) as usize == g.len()
                        && want.len() == g.len()
                        && g.iter().zip(want).all(|(s, w)| {
                            s.shop_number == i(&w["shopnumber"])
                                && s.no_buy == w["bnobuy"].as_bool().unwrap()
                                && s.no_sell == w["bnosell"].as_bool().unwrap()
                        });
                    if same {
                        Ok(())
                    } else {
                        Err(format!("got {g:?}, want {} {want:?}", out["x"]))
                    }
                }
            }
        }
        "ParseActionAndQty" => {
            let q = i(&a[3]);
            match (
                parse_action_and_qty(a[0].as_str().unwrap(), q),
                want_error(case),
            ) {
                (Err(RulesError::Vb6(n)), Some(w)) if n == w => Ok(()),
                (Err(e), w) => Err(format!("error {e:?}, want {w:?}")),
                (Ok(_), Some(w)) => Err(format!("no error, want {w}")),
                (Ok((act, qty)), None) => {
                    let r = &out["x.refs"];
                    if r[0].as_str() == Some(act.as_str()) && i(&r[1]) == qty {
                        Ok(())
                    } else {
                        Err(format!("got {act:?} {qty}, want {r}"))
                    }
                }
            }
        }
        "GetItemsByExactNameArr" => {
            let got = get_items_by_exact_name(&env, a[0].as_str().unwrap());
            let want: Vec<ItemMatch> = out["x.refs"][0]
                .as_array()
                .unwrap()
                .iter()
                .map(item_match_from)
                .collect();
            if got == want && i(&out["x"]) as usize == got.len() {
                Ok(())
            } else {
                Err(format!("got {got:?}\n  want {want:?}"))
            }
        }
        "EvaluateBestPriceForHit" => {
            let hit = item_match_arg(&a[0]);
            let got = evaluate_best_price_for_hit(&env, &hit, i(&a[1]) as i16);
            match (got, want_error(case)) {
                (Err(RulesError::Vb6(n)), Some(w)) if n == w => Ok(()),
                (Err(e), w) => Err(format!("error {e:?}, want {w:?}")),
                (Ok(_), Some(w)) => Err(format!("no error, want {w}")),
                (Ok(g), None) => {
                    let r = &out["x.refs"];
                    if out["x"].as_f64() == Some(g.sort_value)
                        && r[0].as_str() == Some(g.shop_name.as_str())
                        && r[1].as_str() == Some(g.value_cell.as_str())
                        && i(&r[2]) == g.more_shops
                        && r[3].as_bool() == Some(g.sell_only)
                        && i(&r[4]) == g.chosen_shop
                    {
                        Ok(())
                    } else {
                        Err(format!("got {g:?}, want {} {r}", out["x"]))
                    }
                }
            }
        }
        "GetBestShopNumForItem" => {
            let g = get_best_shop_num_for_item(&env, i(&a[0]), i(&a[1]) as i16);
            let r = &out["x.refs"];
            let ok = i(&out["x"]) == g.shop
                && r[0].as_bool() == Some(g.sell_only)
                && r[1].as_f64() == Some(g.sort_sell_copper)
                && r[2].as_str() == Some(g.shop_name.as_str())
                && r[3].as_str() == Some(g.value_cell.as_str());
            let want_ev: Vec<Event> = want_events(case);
            let got_ev: Vec<Event> = g.handled.iter().cloned().collect();
            if ok && want_ev == got_ev {
                Ok(())
            } else {
                Err(format!("got {g:?}, want {} {r} {want_ev:?}", out["x"]))
            }
        }
        "SumInvOrCarriedQtyForName" => {
            let rows = forms.rows();
            check_result(
                case,
                sum_inv_or_carried_qty_for_name(&rows, a[1].as_str().unwrap()),
                i,
            )
        }
        "GetShopRoomNames" => {
            let got = get_shop_room_names(world, i(&a[0]), i(&a[1]), a[2].as_bool().unwrap());
            if out["x"].as_str() == Some(got.as_str()) {
                Ok(())
            } else {
                Err(format!("got {got:?}, want {}", out["x"]))
            }
        }
        "PopulateItemManagerFromParsed" => {
            let mut rows = forms.rows();
            let items = parsed_arg(&a[0]);
            let mut answers: Vec<i64> = inputs["msgbox"]
                .as_array()
                .map(|l| l.iter().map(|x| x.as_i64().unwrap()).collect())
                .unwrap_or_default();
            let mut ask = |_q: &str, default_yes: bool| {
                if answers.is_empty() {
                    default_yes
                } else {
                    answers.remove(0) == 6
                }
            };
            let mut log = Vec::new();
            populate_item_manager_from_parsed(
                &env,
                &mut rows,
                &items,
                a[2].as_bool().unwrap(),
                &mut ask,
                &mut log,
            );
            compare_rows(case, &rows, &log)
        }
        "LV_AddRowByItemNumber" => {
            let mut rows = forms.rows();
            let mut log = Vec::new();
            lv_add_row_by_item_number(
                &env,
                &mut rows,
                i(&a[0]),
                a[1].as_str().unwrap(),
                a[2].as_str().unwrap(),
                i(&a[3]),
                i(&a[4]),
                &mut log,
            );
            compare_rows(case, &rows, &log)
        }
        other => Err(format!("unknown call {other}")),
    }
}

fn compare_rows(case: &Value, rows: &[ListRow], log: &[Event]) -> Result<(), String> {
    let got = dump_rows(rows);
    let want = &case["outputs"]["rows"];
    let want_ev = want_events(case);
    if &got == want && log == want_ev.as_slice() {
        Ok(())
    } else {
        Err(format!(
            "rows got {got}\n  want {want}\n  events got {log:?}\n  want {want_ev:?}"
        ))
    }
}

#[test]
fn item_match_fixture() {
    let fx = fixture::load("item_match");
    let w = world(&fx.header["data"]);
    let mut failures = Vec::new();
    for case in &fx.cases {
        if let Err(msg) = run(&w, &fx.header, case) {
            let id = case["id"].as_str().unwrap();
            failures.push(format!("{id}: {msg}\n  inputs {}", case["inputs"]));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases failed; first:\n{}",
        failures.len(),
        fx.cases.len(),
        failures
            .iter()
            .take(6)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
