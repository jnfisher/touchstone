// Fixture tests for the reachability family: see docs/porting.md step 5.
//
// Expected values: fixtures/rules/reachability.json, produced by the oracle running MME's own modReachability
// (ReachEnsureComputed once, then the queries) and modMain AddMonsterReachRows on a slice of the stock database.
// The slice's rows are the header "data".
use super::fixture::load;
use serde_json::{Value, json};
use touchstone_rules::reachability::*;

fn i(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
}

fn s(v: &Value) -> String {
    v.as_str()
        .unwrap_or_else(|| panic!("not a string: {v}"))
        .to_string()
}

fn input(data: &Value) -> ReachInput {
    let mut inp = ReachInput::default();
    for r in data["rooms"].as_array().unwrap() {
        let mut exits: [String; 10] = Default::default();
        for (k, e) in s(&r[7]).split('|').enumerate() {
            exits[k] = e.to_string();
        }
        inp.rooms.push(ReachRoom {
            map: i(&r[0]),
            room: i(&r[1]),
            name: s(&r[2]),
            npc: i(&r[3]),
            cmd: i(&r[4]),
            spell: i(&r[5]),
            placed: s(&r[6]),
            exits,
        });
    }
    for r in data["tbinfo"].as_array().unwrap() {
        inp.tbinfo.push(ReachTb {
            number: i(&r[0]),
            link_to: i(&r[1]),
            action: s(&r[2]),
            called_from: s(&r[3]),
        });
    }
    for r in data["spells"].as_array().unwrap() {
        let f: Vec<i32> = r[4].as_array().unwrap().iter().map(i).collect();
        inp.spells.push(ReachSpell {
            number: i(&r[0]),
            req_level: i(&r[1]),
            dur: i(&r[2]),
            casted_by: s(&r[3]),
            abilities: f
                .chunks(2)
                .map(|p| (p[0], *p.get(1).unwrap_or(&0)))
                .collect(),
        });
    }
    for r in data["monsters"].as_array().unwrap() {
        inp.monsters.push(ReachMonster {
            number: i(&r[0]),
            name: s(&r[1]),
            greet: i(&r[2]),
            in_game: i(&r[3]) != 0,
            summoned_by: s(&r[4]),
        });
    }
    for r in data["items"].as_array().unwrap() {
        inp.items.push(ReachItem {
            number: i(&r[0]),
            obtained_from: s(&r[1]),
        });
    }
    inp
}

/// FNV-1a 64 of the values joined by 0x1f, as the oracle's `%digest` read writes it.
fn digest<T: ToString>(vals: impl Iterator<Item = T>) -> String {
    let joined: Vec<String> = vals.map(|v| v.to_string()).collect();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in joined.join("\x1f").bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn cell_json(text: &str, tag: &Option<String>, color: Option<i32>, bold: bool) -> Value {
    if tag.is_none() && color.is_none() && !bold {
        json!(text)
    } else {
        json!([text, tag, color, bold])
    }
}

/// A row in the shape the oracle's recording ListView dumps it for the detail panes.
fn row_json(r: &touchstone_rules::detail::monster::MonsterRow) -> Value {
    if r.tag.is_none() && r.fore_color.is_none() && !r.bold && r.cells.is_empty() {
        return json!(r.label);
    }
    let cells: Vec<Value> = r
        .cells
        .iter()
        .map(|c| cell_json(&c.text, &c.tag, c.fore_color, c.bold))
        .collect();
    json!([r.label, r.tag, r.fore_color, r.bold, cells])
}

fn status_code(st: ReachStatus) -> i32 {
    st as i32
}

/// The ByRef values a route call leaves, in the order of the oracle's argument list.
fn route_refs(r: &Option<Route>) -> Value {
    match r {
        None => json!([0, 0, "", [0], [0], [0], [""], 0, 0, 0, 0, ""]),
        Some(r) => json!([
            r.source_map,
            r.source_room,
            r.source_name,
            r.steps.iter().map(|x| x.map).collect::<Vec<_>>(),
            r.steps.iter().map(|x| x.room).collect::<Vec<_>>(),
            r.steps.iter().map(|x| x.flags).collect::<Vec<_>>(),
            r.steps.iter().map(|x| x.label.clone()).collect::<Vec<_>>(),
            r.n_steps,
            r.coin_steps,
            r.alt_steps,
            r.alt_coin_steps,
            r.alt_source_name,
        ]),
    }
}

#[test]
fn reachability_matches_the_oracle() {
    let fx = load("reachability");
    let inp = input(&fx.header["data"]);
    let reach = compute(&inp);
    let names = |m: i32, r: i32| {
        inp.rooms
            .iter()
            .find(|x| x.map == m && x.room == r)
            .map(|x| x.name.clone())
    };
    let mut failures = Vec::new();
    let mut check = |id: &str, what: &str, got: Value, want: &Value| {
        if &got != want && failures.len() < 12 {
            failures.push(format!("{id} {what}: got {got}, want {want}"));
        }
    };
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        assert!(case.get("error").is_none(), "{id}: the oracle raised");
        assert!(
            case.get("events").is_none(),
            "{id}: MME's handler fired: {}",
            case["events"]
        );
        let out = &case["outputs"];
        let step = &case["inputs"]["steps"][0];
        let lead = |k: usize| step["each"][k].as_array().unwrap();
        if id == "build" {
            let st = reach.stats();
            check(id, "rooms", json!(st.rooms), &out["rooms"]);
            check(id, "edges", json!(st.edges), &out["edges"]);
            check(id, "groups", json!(st.random_groups), &out["groups"]);
            check(id, "dropped", json!(st.scatter_dropped), &out["dropped"]);
            check(id, "bands", json!(st.level_bands), &out["bands"]);
            check(
                id,
                "bp_levels",
                json!(reach.breakpoints()),
                &out["bp_levels"],
            );
            let e = reach.edge_columns();
            let want = &out["digest"];
            check(id, "eFrom", json!(digest(e.from.iter())), &want["eFrom"]);
            check(id, "eTo", json!(digest(e.to.iter())), &want["eTo"]);
            check(id, "eMin", json!(digest(e.min.iter())), &want["eMin"]);
            check(id, "eMax", json!(digest(e.max.iter())), &want["eMax"]);
            check(id, "eGroup", json!(digest(e.group.iter())), &want["eGroup"]);
            check(id, "eKind", json!(digest(e.kind.iter())), &want["eKind"]);
            check(id, "eDir", json!(digest(e.dir.iter())), &want["eDir"]);
            check(id, "eLabel", json!(digest(e.label.iter())), &want["eLabel"]);
            check(id, "eCoin", json!(digest(e.coin.iter())), &want["eCoin"]);
            check(id, "eWait", json!(digest(e.wait.iter())), &want["eWait"]);
            let nums = out["mon_num"].as_array().unwrap();
            for (k, n) in nums.iter().enumerate() {
                let n = i(n);
                check(
                    id,
                    &format!("status of monster {n}"),
                    json!(reach.monster_status_code(n)),
                    &out["mon_status"][k],
                );
                check(
                    id,
                    &format!("bands of monster {n}"),
                    json!(reach.bands_text(n)),
                    &out["band_text"][k],
                );
            }
            continue;
        }
        if id.starts_with("ok_") {
            for (k, v) in out["ok"].as_array().unwrap().iter().enumerate() {
                let a = lead(k);
                check(
                    id,
                    &format!("monster {}", a[0]),
                    json!(reach.is_monster_ok(i(&a[0]), i(&a[1]))),
                    v,
                );
            }
        } else if id.starts_with("info_") {
            for (k, v) in out["info"].as_array().unwrap().iter().enumerate() {
                let a = lead(k);
                let r = reach.monster_info(i(&a[0]), i(&a[1]));
                check(
                    id,
                    &format!("status of {}", a[0]),
                    json!(status_code(r.status)),
                    v,
                );
                let refs = json!([
                    r.gate_map,
                    r.gate_room,
                    r.dest_map,
                    r.dest_room,
                    r.min_lvl,
                    r.max_lvl,
                    r.random_group,
                    r.reachable_at
                ]);
                check(id, &format!("info of {}", a[0]), refs, &out["info.refs"][k]);
            }
        } else if id.starts_with("room_status_") {
            for (k, v) in out["status"].as_array().unwrap().iter().enumerate() {
                let a = lead(k);
                check(
                    id,
                    &format!("room {}/{}", a[0], a[1]),
                    json!(status_code(reach.room_status(i(&a[0]), i(&a[1]), i(&a[2])))),
                    v,
                );
            }
        } else if id.starts_with("route_") {
            for (k, v) in out["route"].as_array().unwrap().iter().enumerate() {
                let a = lead(k);
                let r = reach.route(
                    i(&a[0]),
                    i(&a[1]),
                    a[2].as_bool().unwrap(),
                    a[3].as_bool().unwrap(),
                    a[4].as_bool().unwrap(),
                );
                check(id, &format!("found for {}", a[0]), json!(r.is_some()), v);
                check(
                    id,
                    &format!("route of {}", a[0]),
                    route_refs(&r),
                    &out["route.refs"][k],
                );
            }
        } else if id.starts_with("room_route_") {
            for (k, v) in out["route"].as_array().unwrap().iter().enumerate() {
                let a = lead(k);
                let r = reach.room_route(
                    i(&a[0]),
                    i(&a[1]),
                    i(&a[2]),
                    a[3].as_bool().unwrap(),
                    a[4].as_bool().unwrap(),
                );
                check(
                    id,
                    &format!("found for {}/{}", a[0], a[1]),
                    json!(r.is_some()),
                    v,
                );
                check(
                    id,
                    &format!("route to {}/{}", a[0], a[1]),
                    route_refs(&r),
                    &out["route.refs"][k],
                );
            }
        } else if id.starts_with("rows_") {
            let level: i32 =
                case["inputs"]["forms"]["frmMain"]["txtGlobalLevel"]["$array"]["0"]["Text"]
                    .as_str()
                    .unwrap()
                    .parse()
                    .unwrap();
            let g = &case["inputs"]["globals"];
            let opts = ReachRowOptions {
                level,
                hide_record_numbers: g["bHideRecordNumbers"].as_bool().unwrap(),
                waypoint_interval: i(&g["nRouteWaypointInterval"]),
            };
            let keys = step["rows"]["keys"].as_array().unwrap();
            for (k, mon) in keys.iter().enumerate() {
                let rows: Vec<Value> = monster_reach_rows(&reach, i(mon), &opts, &names)
                    .iter()
                    .map(row_json)
                    .collect();
                check(
                    id,
                    &format!("rows of monster {mon}"),
                    json!(rows),
                    &out["rows"][k],
                );
            }
        } else {
            panic!("unknown case {id}");
        }
    }
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
