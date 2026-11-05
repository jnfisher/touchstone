// Fixture tests for the textblock family (fixtures/rules/textblock.json, written by
// tools/oracle/gen/textblock.py and `oracle.py gen textblock`).
//
// The oracle ran MME's own procedures over the stock database. The textblock table the ports
// read is rebuilt from the fixture itself: the "tb-*" cases record each textblock's Action and
// LinkTo as the outputs of GetTextblockAction/GetTextblockLinkTo, and the "r*"/"n*" cases record
// the CMD of each room they name. Every expected value below comes from the fixture file.

use super::fixture;
use serde_json::{Value, json};
use std::collections::HashMap;
use touchstone_model::TbInfo;
use touchstone_rules::textblock::*;
use touchstone_vb6::Currency;

#[derive(Default)]
struct Db {
    tb: HashMap<i32, TbInfo>,
    rooms: HashMap<(i32, i32), i32>,
}

impl TextblockData for Db {
    fn tbinfo(&self, number: i32) -> Option<&TbInfo> {
        self.tb.get(&number)
    }
    fn room_cmd(&self, map: i32, room: i32) -> Option<i32> {
        self.rooms.get(&(map, room)).copied()
    }
}

fn i(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("not a string: {v}"))
}

fn cur_of(v: &Value) -> Currency {
    Currency::from_f64(v["v"].as_f64().expect("cur value")).unwrap()
}

fn build_db(cases: &[Value]) -> Db {
    let mut db = Db::default();
    for c in cases {
        let id = c["id"].as_str().unwrap();
        let out = &c["outputs"];
        if id.starts_with("tb-") {
            let n = i(&c["inputs"]["steps"][0]["args"][0]);
            db.tb.insert(
                n,
                TbInfo {
                    number: n,
                    link_to: i(&out["l"]),
                    action: Some(s(&out["a"]).to_string()),
                    called_from: String::new(),
                },
            );
        } else if let Some(args) = c["inputs"]["args"].as_array()
            && c["inputs"]["proc"] == "GetRoomCMDTB"
            && args.len() == 3
        {
            db.rooms
                .insert((i(&args[1]), i(&args[2])), i(&out["result"]));
        }
    }
    // The grid's own cases hold the real text; the "action" of a missing textblock is "none".
    db.tb.retain(|n, _| *n != 0);
    db
}

fn source_json(t: &RoomCmdSource) -> Value {
    json!({
        "scommands": t.commands, "slabel": t.label, "sroomlist": t.room_list,
        "sfirstroom": t.first_room, "nroomcount": t.room_count, "nchance": t.chance,
        "bcut": t.cut, "baction": t.action,
    })
}

/// Runs one call and returns (result, refs) as the oracle encodes them.
fn run(db: &Db, call: &str, args: &[Value]) -> (Value, Option<Value>) {
    let n = |k: usize| i(&args[k]);
    match call {
        "GetTextblockAction" => (json!(get_textblock_action(db, n(0))), None),
        "GetTextblockLinkTo" => (json!(get_textblock_link_to(db, n(0))), None),
        "GetTextblockCMDS" => (json!(get_textblock_cmds(db, n(0), n(1) as i16)), None),
        "GetTextblockTrigger" => (json!(get_textblock_trigger(db, n(0), n(1))), None),
        "GetTextblockCMDLine" => (
            json!(get_textblock_cmd_line(db, s(&args[0]), s(&args[1]), n(2))),
            None,
        ),
        "GetTextblockCMDText" => (
            json!(get_textblock_cmd_text(db, s(&args[0]), s(&args[1]), n(2))),
            None,
        ),
        "GetTextblockTeleport" => {
            let r = get_textblock_teleport(db, n(0));
            let (room, map) = r.unwrap_or((0, 0));
            (json!(r.is_some()), Some(json!([room, map])))
        }
        "TextBlockHasTeleport" => (
            json!(textblock_has_teleport(
                db,
                n(0),
                n(1),
                n(2),
                args[3].as_bool().unwrap(),
                n(4) as i16
            )),
            None,
        ),
        "GetChestItems" => {
            let mut chest = ChestItems::new();
            let mut nest = args[2]["value"].as_i64().unwrap() as i32;
            let pm = args.get(3).map(cur_of);
            get_chest_items(db, &mut chest, n(1), &mut nest, pm);
            let mut flat = Vec::new();
            // the oracle's array is Currency throughout, so item numbers are scaled by 10000
            flat.extend(
                chest
                    .entries
                    .iter()
                    .map(|e| json!(i64::from(e.item) * 10_000)),
            );
            flat.extend(chest.entries.iter().map(|e| json!(e.percent.scaled())));
            flat.extend(chest.entries.iter().map(|e| json!(e.fail.scaled())));
            (Value::Null, Some(json!([flat, nest])))
        }
        "GetRoomCMDTB" => {
            let (mr, m, r) = if args.len() == 1 {
                (s(&args[0]), 0, 0)
            } else {
                (s(&args[0]), n(1), n(2))
            };
            (json!(get_room_cmd_tb(db, mr, m, r)), None)
        }
        "ExtractMapRoom" => {
            let e = extract_map_room(s(&args[0]));
            (
                json!({"map": e.map, "room": e.room, "exittype": e.exit_type}),
                None,
            )
        }
        "ExtractTextCommand" => (json!(extract_text_command(s(&args[0]))), None),
        "SplitRoomCmdSources" => {
            let (rest, list) = split_room_cmd_sources(s(&args[0]));
            let arr: Vec<Value> = list.iter().map(source_json).collect();
            (json!(rest), Some(json!([arr, list.len()])))
        }
        "ObtainedFromIsGettable" => (json!(obtained_from_is_gettable(s(&args[0]))), None),
        "ExpandRoomCmdRooms" => {
            let v = expand_room_cmd_rooms(s(&args[0]));
            (json!(v.len()), Some(json!([v])))
        }
        other => panic!("fixture calls {other}, which the test does not know"),
    }
}

/// A Currency in the oracle's output is `{"cur": scaled, "s": ..}`; the port yields scaled ints.
fn normalise(v: &Value) -> Value {
    match v {
        Value::Object(m) if m.contains_key("cur") => m["cur"].clone(),
        Value::Array(a) => Value::Array(a.iter().map(normalise).collect()),
        other => other.clone(),
    }
}

#[test]
fn textblock_fixture() {
    let fx = fixture::load("textblock");
    assert_eq!(fx.header["family"], "textblock");
    let db = build_db(&fx.cases);
    assert!(db.tb.len() > 500 && db.rooms.len() > 100);

    let mut failures = Vec::new();
    let mut steps_run = 0;
    for c in &fx.cases {
        let id = c["id"].as_str().unwrap();
        let inputs = &c["inputs"];
        let steps: Vec<Value> = match inputs["steps"].as_array() {
            Some(a) => a.clone(),
            None => vec![json!({"call": inputs["proc"], "args": inputs["args"], "as": "result"})],
        };
        for st in &steps {
            let call = s(&st["call"]);
            let args = st["args"].as_array().unwrap();
            let name = s(&st["as"]);
            let (got, refs) = run(&db, call, args);
            steps_run += 1;
            let out = &c["outputs"];
            let want = &out[name];
            // GetChestItems is a Sub and Expand/Split report through refs only
            let mut bad = !(want.is_null() && got.is_null()) && normalise(want) != got;
            if let Some(r) = refs {
                let want_refs = normalise(&out[format!("{name}.refs")]);
                bad |= want_refs != r;
            }
            if bad {
                let mut msg = format!(
                    "{id} {call}({}): want {want} {} got {got}",
                    serde_json::to_string(args).unwrap(),
                    out[format!("{name}.refs")],
                );
                msg.truncate(400);
                failures.push(msg);
            }
        }
    }
    assert!(steps_run > 3000, "ran only {steps_run} steps");
    assert!(
        failures.is_empty(),
        "{} of {steps_run} steps differ, first:\n{}",
        failures.len(),
        failures
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
