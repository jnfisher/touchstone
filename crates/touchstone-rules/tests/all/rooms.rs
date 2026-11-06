// Fixture tests for the Rooms tab (text half): see docs/porting.md step 5.
//
// Expected values: fixtures/rules/rooms.json, produced by the oracle running MME's own frmMain
// procedures (MapGetRoomLoc, MapMapExits, cmdMapFindText_Click, FindRoomWithDirections,
// ShowRoomCmdRoomList) over a subset of the stock database plus invented rows numbered 9001 and up.
// The rows the cases can reach are under the header's "data".
use serde_json::{Value, json};
use touchstone_model::{World, WorldParts};
use touchstone_rules::lairs::LairInfo;
use touchstone_rules::locations::LocationLairs;
use touchstone_rules::rooms::{
    FindTextEvent, FindTextState, RoomEnv, RoomOptions, find_exits, find_text_step,
    room_cmd_room_list, room_refs, room_tooltip,
};
use touchstone_vb6::{Currency, clng, val};

use crate::fixture::load;
use crate::misc_detail::{Table, i, world as misc_world};

fn world(data: &Value) -> World {
    // the shared reader wants whole item and shop rows; the rooms data keeps only names
    let mut d = data.clone();
    let item_f = [
        "Number",
        "Name",
        "ItemType",
        "Price",
        "Currency",
        "Encum",
        "ArmourClass",
        "DamageResist",
        "WeaponType",
        "ArmourType",
        "Worn",
        "Accy",
        "StrReq",
        "Speed",
        "ClassRest",
        "Abilities",
        "Obtained From",
    ];
    let items: Vec<Value> = data["items"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| json!([r[0], r[1], 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, [], [], ""]))
        .collect();
    d["items"] = json!({"fields": item_f, "rows": items});
    let shop_f = [
        "Number",
        "ShopType",
        "MinLVL",
        "MaxLVL",
        "Markup%",
        "ClassRest",
        "Stock",
        "Assigned To",
    ];
    let shops: Vec<Value> = data["shops"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| json!([r[0], 0, 0, 0, 0, 0, [], ""]))
        .collect();
    d["shops"] = json!({"fields": shop_f, "rows": shops});
    let w = misc_world(&d);
    let mut parts = WorldParts {
        info: w.info.clone(),
        items: w.items.clone(),
        monsters: w.monsters.clone(),
        spells: w.spells.clone(),
        rooms: Vec::new(),
        shops: w.shops.clone(),
        classes: w.classes.clone(),
        races: w.races.clone(),
        lairs: vec![],
        tbinfo: w.tbinfo.clone(),
    };
    for (s, r) in parts
        .shops
        .iter_mut()
        .zip(data["shops"]["rows"].as_array().unwrap())
    {
        s.name = r[1].as_str().unwrap().to_string();
    }
    let mons = Table::new(&data["monsters"]);
    for (m, r) in parts.monsters.iter_mut().zip(mons.iter()) {
        m.avg_dmg = r.v("AvgDmg").as_f64().unwrap();
    }
    let rooms = Table::new(&data["rooms"]);
    for r in rooms.iter() {
        let ex = |k: &str| r.v(k).as_str().unwrap().to_string();
        let opt = |k: &str| Some(ex(k));
        parts.rooms.push(touchstone_model::Room {
            map_number: r.i("Map Number"),
            room_number: r.i("Room Number"),
            name: opt("Name"),
            light: r.i("Light"),
            shop: r.i("Shop"),
            npc: r.i("NPC"),
            cmd: r.i("CMD"),
            spell: r.i("Spell"),
            lair: opt("Lair"),
            delay: r.i("Delay"),
            placed: opt("Placed"),
            n: ex("N"),
            s: ex("S"),
            e: ex("E"),
            w: ex("W"),
            ne: ex("NE"),
            nw: ex("NW"),
            se: ex("SE"),
            sw: ex("SW"),
            u: ex("U"),
            d: ex("D"),
        });
    }
    World::new(parts).unwrap()
}

/// GetLairInfo from the lair shim's table (data.lairs).
struct Env<'a> {
    lairs: &'a serde_json::Map<String, Value>,
    use_char: bool,
    party: i32,
    vs_party: Vec<i64>,
    vs_char: Vec<i64>,
    vs_default: Vec<i64>,
    avg: &'a dyn Fn(i32) -> f64,
}

impl LocationLairs for Env<'_> {
    fn get_lair_info(&mut self, group_index: &str, max_regen: i16) -> LairInfo {
        let mut u = LairInfo::default();
        let Some(row) = self.lairs.get(group_index) else {
            return u;
        };
        let cur = |k: &str| {
            row.get(k).map_or(Currency::from_scaled(0), |v| {
                Currency::from_f64(v.as_f64().unwrap()).unwrap()
            })
        };
        u.group_index = group_index.to_string();
        u.mob_list = row
            .get("sMobList")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        u.mobs = cur("nMobs");
        u.avg_exp = cur("nAvgExp");
        u.avg_hp = row.get("nAvgHP").map_or(0, |v| v.as_f64().unwrap() as i32);
        u.avg_dmg_clear_round = cur("nAvgDmgClearRound");
        u.avg_dmg_clear = cur("nAvgDmgClear");
        u.max_regen = Currency::from_scaled(i64::from(max_regen) * 10_000);
        u
    }
}

impl RoomEnv for Env<'_> {
    // GetPreCalculatedMonsterDamage over the case's arrays
    fn monster_damage(&self, number: i32) -> (f64, String) {
        let party = self.party.max(1);
        if number < 1 {
            let l = if party > 1 {
                "vs Party"
            } else if self.use_char {
                "vs Char"
            } else {
                "(default)"
            };
            return (0.0, l.into());
        }
        let at = |a: &Vec<i64>| a.get(number as usize).copied().unwrap_or(-1);
        if party > 1 && at(&self.vs_party) >= 0 {
            (at(&self.vs_party) as f64, "vs Party".into())
        } else if party == 1 && self.use_char && at(&self.vs_char) >= 0 {
            (at(&self.vs_char) as f64, "vs Char".into())
        } else if at(&self.vs_default) >= 0 {
            (at(&self.vs_default) as f64, "(default)".into())
        } else {
            ((self.avg)(number), "(default)".into())
        }
    }
}

/// A control property as the case sees it: the case's own control replaces the family-wide one.
fn ctl<'a>(fx: &'a Value, case: &'a Value, form: &str, name: &str) -> Option<&'a Value> {
    case["inputs"]["forms"][form]
        .get(name)
        .or_else(|| fx["header"]["setup"]["forms"][form].get(name))
}

fn array_prop(c: Option<&Value>, idx: &str, prop: &str) -> Option<Value> {
    c.and_then(|c| c["$array"].get(idx))
        .and_then(|e| e.get(prop))
        .cloned()
}

fn global<'a>(setup: &'a Value, case: &'a Value, name: &str) -> &'a Value {
    case["inputs"]["globals"]
        .get(name)
        .unwrap_or(&setup["globals"][name])
}

fn ints(v: &Value) -> Vec<i64> {
    v.as_array()
        .map(|a| a.iter().map(|x| x.as_i64().unwrap()).collect())
        .unwrap_or_default()
}

fn row_json(text: &str, tag: &str) -> Value {
    json!([text, tag, []])
}

#[test]
fn rooms_match_the_oracle() {
    let fx_loaded = load("rooms");
    let fx = json!({"header": fx_loaded.header});
    let data = &fx["header"]["data"];
    let w = world(data);
    let lairs = data["lairs"].as_object().unwrap();
    let setup = &fx["header"]["setup"];
    let avg = |n: i32| {
        w.get_monster(n)
            .map_or(0.0, |m| f64::from(clng(m.avg_dmg).unwrap_or(0)))
    };
    let mut bad = Vec::new();
    let mut checked = 0;
    for case in &fx_loaded.cases {
        let id = case["id"].as_str().unwrap();
        let case_v = json!({"inputs": case["inputs"]});
        let opt_box = |i: &str| {
            let c = ctl(&fx, &case_v, "frmMain", "chkMapOptions");
            array_prop(c, i, "Value")
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                != 0
        };
        let illu = array_prop(
            ctl(&fx, &case_v, "frmMain", "lblInvenCharStat"),
            "23",
            "Caption",
        )
        .map_or(0, |v| clng(val(v.as_str().unwrap())).unwrap());
        let opts = RoomOptions {
            nmr_ver: global(setup, &case_v, "nNMRVer").as_f64().unwrap(),
            hide_record_numbers: global(setup, &case_v, "bHideRecordNumbers")
                .as_bool()
                .unwrap(),
            hide_npcs: opt_box("3"),
            hide_lairs: opt_box("2"),
            hide_commands: opt_box("4"),
            show_all_exits: opt_box("6"),
            illumination: illu,
        };
        let lair_mode = array_prop(
            ctl(&fx, &case_v, "frmMain", "optMonsterFilter"),
            "1",
            "Value",
        )
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
        let party_text = array_prop(
            ctl(&fx, &case_v, "frmMain", "txtMonsterLairFilter"),
            "0",
            "Text",
        )
        .map_or(1.0, |v| val(v.as_str().unwrap()));
        let use_char = ctl(&fx, &case_v, "frmMain", "chkGlobalFilter")
            .and_then(|c| c["Value"].as_i64())
            .unwrap_or(0)
            == 1;
        let mut env = Env {
            lairs,
            use_char,
            party: if lair_mode { party_text as i32 } else { 0 },
            vs_party: ints(global(setup, &case_v, "nMonsterDamageVsParty")),
            vs_char: ints(global(setup, &case_v, "nMonsterDamageVsChar")),
            vs_default: ints(global(setup, &case_v, "nMonsterDamageVsDefault")),
            avg: &avg,
        };
        let out = &case["outputs"];
        let mut state = FindTextState::default();
        let mut answers = case["inputs"]["inputbox"]
            .as_array()
            .map(|a| a.iter().map(|x| x.as_str().unwrap().to_string()).collect())
            .unwrap_or_else(Vec::<String>::new)
            .into_iter();
        let mut want_events: Vec<Value> = Vec::new();
        let mut got_events: Vec<Value> = Vec::new();
        let mut want_errors: Vec<i64> = Vec::new();
        let mut got_errors: Vec<i64> = Vec::new();
        for st in case["inputs"]["steps"].as_array().unwrap() {
            let call = st["call"].as_str().unwrap();
            let args = st["args"].as_array().cloned().unwrap_or_default();
            let tag = st["as"].as_str().unwrap_or("");
            checked += 1;
            match call {
                "RoomsClearRefs" => {}
                "MapGetRoomLoc" => {
                    let (m, r) = (i(&args[0]), i(&args[1]));
                    let got = room_refs(&w, &opts, &env, m, r);
                    if let Some(e) = got.error {
                        got_errors.push(i64::from(e));
                    }
                    if let Some(msg) = &got.message {
                        got_events.push(json!({"kind": "msgbox", "text": msg}));
                    }
                    let rows: Vec<Value> = got
                        .rows
                        .iter()
                        .map(|x| row_json(&x.text, &x.tag.text()))
                        .collect();
                    let want = &out[format!("{tag}.cap")]["refs"];
                    if want != &Value::Array(rows.clone()) {
                        bad.push(format!(
                            "{id} {tag} room {m}/{r}: refs\n  got  {}\n  want {want}",
                            Value::Array(rows)
                        ));
                    }
                }
                "MapMapExits" => {
                    let (r, m) = (i(&args[1]), i(&args[2]));
                    let got = room_tooltip(&w, &opts, &mut env, m, r);
                    let want = out[format!("{tag}.cap")]["tip"].as_str();
                    if got.as_deref() != want {
                        bad.push(format!(
                            "{id} {tag} room {m}/{r}: tooltip\n  got  {got:?}\n  want {want:?}"
                        ));
                    }
                }
                "cmdMapFindText_Click" => {
                    let index = i(&args[0]) as u8;
                    let will_ask = index == 0 || state.last.is_none();
                    let answer = if will_ask {
                        answers.next().unwrap_or_default()
                    } else {
                        String::new()
                    };
                    for ev in find_text_step(&w, &mut state, index, &answer) {
                        got_events.push(match ev {
                            FindTextEvent::Hit(m, r) => {
                                json!({"kind": "call", "name": "MapStartMapping", "args": [m, r]})
                            }
                            FindTextEvent::Message(t) => json!({"kind": "msgbox", "text": t}),
                        });
                    }
                }
                "FindRoomWithDirections" => {
                    let pop = |n: &str| ctl(&fx, &case_v, "frmPopUpOptions", n).cloned();
                    let name = pop("txtRoomName").unwrap()["Text"]
                        .as_str()
                        .unwrap()
                        .to_string();
                    let exact = array_prop(pop("optRoomFindMatch").as_ref(), "1", "Value")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let mut mask = 0;
                    for d in 0..10 {
                        let t = array_prop(pop("cmdRoomFindDir").as_ref(), &d.to_string(), "Tag");
                        if t.and_then(|t| t.as_str().map(|s| s == "1"))
                            .unwrap_or(false)
                        {
                            mask |= 1 << d;
                        }
                    }
                    let ok = ctl(&fx, &case_v, "frmMain", "txtRoomFindOk").unwrap()["Text"] == "1";
                    let got = if ok {
                        find_exits(&w, &name, exact, mask)
                    } else {
                        Default::default()
                    };
                    for m in &got.messages {
                        got_events.push(json!({"kind": "msgbox", "text": m}));
                    }
                    let rows: Vec<Value> =
                        got.rows.iter().map(|x| row_json(&x.text, &x.tag)).collect();
                    let want = &out[format!("{tag}.cap")];
                    if want["rows"] != Value::Array(rows.clone())
                        || want["caption"] != got.caption.as_str()
                    {
                        bad.push(format!(
                            "{id}: exits\n  got  {} {}\n  want {} {}",
                            got.caption,
                            Value::Array(rows),
                            want["caption"],
                            want["rows"]
                        ));
                    }
                }
                "ShowRoomCmdRoomList" => {
                    let got = room_cmd_room_list(
                        &w,
                        opts.hide_record_numbers,
                        args[0].as_str().unwrap(),
                        args[1].as_str().unwrap(),
                        args[2].as_str().unwrap(),
                    );
                    let rows: Vec<Value> =
                        got.rows.iter().map(|x| row_json(&x.text, &x.tag)).collect();
                    let want = &out[format!("{tag}.cap")];
                    if want["rows"] != Value::Array(rows.clone())
                        || want["caption"] != got.caption.as_str()
                    {
                        bad.push(format!(
                            "{id}: cmd rooms\n  got  {} {}\n  want {} {}",
                            got.caption,
                            Value::Array(rows),
                            want["caption"],
                            want["rows"]
                        ));
                    }
                }
                other => panic!("{id}: unknown call {other}"),
            }
        }
        // events: message boxes, the finder's jumps and the errors MapGetRoomLoc's handler took
        for e in case["events"].as_array().cloned().unwrap_or_default() {
            match e["kind"].as_str().unwrap() {
                "msgbox" => want_events.push(json!({"kind": "msgbox", "text": e["text"]})),
                "call" => {
                    want_events.push(json!({"kind": "call", "name": e["name"], "args": e["args"]}))
                }
                "handled_error" => want_errors.push(e["number"].as_i64().unwrap()),
                _ => {}
            }
        }
        if want_events != got_events {
            bad.push(format!(
                "{id}: events\n  got  {got_events:?}\n  want {want_events:?}"
            ));
        }
        if want_errors != got_errors {
            bad.push(format!(
                "{id}: errors got {got_errors:?} want {want_errors:?}"
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} checks differ; first:\n{}",
        bad.len(),
        checked,
        bad.iter().take(6).cloned().collect::<Vec<_>>().join("\n")
    );
}
