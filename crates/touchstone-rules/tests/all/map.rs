// Fixture tests for the graphical map engine: see docs/porting.md step 5.
//
// Expected values: fixtures/rules/map.json, produced by the oracle running MME's own MapStartMapping,
// MapActivateCell and MapMapExits (frmMain and frmMap), MapGoDirection, cmdMove_Click and
// cmdMapLeadsHere_Click over a subset of the stock database plus invented rows numbered 9001 and up.
// The rows the cases can reach are under the header's "data".
use std::collections::BTreeMap;

use serde_json::{Value, json};
use touchstone_model::{Monster, Room, World, WorldParts};
use touchstone_rules::map::{
    AlsoMark, LeadsHereOptions, MapOptions, MapRequest, MapSurface, RoomKey, Walk, cmd_move,
    go_direction, layout, leads_here, legend, presets,
};

use crate::fixture::load;
use crate::misc_detail::{Table, i, world as misc_world};

fn empty() -> Value {
    json!({"fields": [], "rows": []})
}

fn world(data: &Value) -> World {
    let d = json!({
        "items": empty(), "shops": empty(), "classes": empty(), "races": empty(), "rooms": empty(),
        "spells": data["spells"], "monsters": json!({"fields": ["Number", "Name", "GreetTXT"], "rows":
            data["monsters"]["rows"].as_array().unwrap().iter().map(|r| json!([r[0], r[1], r[2]])).collect::<Vec<_>>()}),
        "tbinfo": data["tbinfo"], "lairs": {},
    });
    let w = misc_world(&d);
    let mut monsters: Vec<Monster> = w.monsters.clone();
    for (m, r) in monsters
        .iter_mut()
        .zip(data["monsters"]["rows"].as_array().unwrap())
    {
        m.in_game = i(&r[3]) != 0;
        m.summoned_by = Some(r[4].as_str().unwrap().to_string());
    }
    let parts = WorldParts {
        info: w.info.clone(),
        items: vec![],
        monsters,
        spells: w.spells.clone(),
        rooms: vec![],
        shops: vec![],
        classes: vec![],
        races: vec![],
        lairs: vec![],
        tbinfo: w.tbinfo.clone(),
    };
    let rooms = Table::new(&data["rooms"]);
    let mut parts = parts;
    for r in rooms.iter() {
        let ex = |k: &str| r.v(k).as_str().unwrap().to_string();
        parts.rooms.push(Room {
            map_number: r.i("Map Number"),
            room_number: r.i("Room Number"),
            name: Some(ex("Name")),
            light: 0,
            shop: r.i("Shop"),
            npc: r.i("NPC"),
            cmd: r.i("CMD"),
            spell: r.i("Spell"),
            lair: Some(ex("Lair")),
            delay: 0,
            placed: None,
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

fn options(case: &Value) -> MapOptions {
    let frm = &case["inputs"]["forms"]["frmMain"];
    let chk = |n: &str| frm["chkMapOptions"]["$array"][n]["Value"].as_i64() == Some(1);
    let mark = (0..4)
        .find(|k| frm["optAlsoMark"]["$array"][k.to_string()]["Value"] == json!(true))
        .unwrap_or(0);
    MapOptions {
        follow_map_changes: chk("0"),
        no_hidden: chk("1"),
        no_lairs: chk("2"),
        no_npcs: chk("3"),
        no_commands: chk("4"),
        allow_dupes: chk("9"),
        allow_overwrite: chk("10"),
        no_restricted: chk("12"),
        also_mark: match mark {
            1 => AlsoMark::Shops,
            2 => AlsoMark::Spells,
            _ => AlsoMark::None,
        },
    }
}

fn ints(v: &Value) -> Vec<i64> {
    v.as_array().unwrap().iter().map(|x| i(x) as i64).collect()
}

#[test]
fn layout_matches_the_oracle() {
    let fx = load("map");
    let world = world(&fx.header["data"]);
    let mut bad = Vec::new();
    let mut n = 0;
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        let Some(surface) = case["inputs"]["surface"].as_str() else {
            continue;
        };
        n += 1;
        let surface = MapSurface::parse(surface).unwrap();
        let args = case["inputs"]["steps"][0]["args"].as_array().unwrap();
        let start = RoomKey {
            map: i(&args[0]),
            room: i(&args[1]),
        };
        let req = MapRequest {
            surface,
            options: options(case),
            center: args.get(2).map(i),
        };
        let m = &case["outputs"]["m"];
        match layout(&world, start, &req) {
            Err(_) => {
                // expected value: the oracle drew nothing and MME showed a message box
                if !m["draws"].as_array().unwrap().is_empty() {
                    bad.push(format!("{id}: refused a start room MME drew"));
                }
                continue;
            }
            Ok(grid) => {
                if m["draws"].as_array().unwrap().is_empty() {
                    bad.push(format!("{id}: drew where MME refused"));
                    continue;
                }
                let want = ints(&m["draws"]);
                let got: Vec<i64> = grid.draws.iter().map(|d| d.packed()).collect();
                if want != got {
                    let at = want.iter().zip(&got).position(|(a, b)| a != b);
                    bad.push(format!(
                        "{id}: draws differ (oracle {} vs port {}, first difference at {:?}: {:?} vs {:?})",
                        want.len(),
                        got.len(),
                        at,
                        at.map(|k| want[k]),
                        at.map(|k| got[k])
                    ));
                    continue;
                }
                let raw = grid.raw_cells();
                let got_cells: Vec<Vec<i64>> = raw
                    .iter()
                    .filter(|c| c.map != 0 || c.room != 0 || c.state != 0 || c.back != 0xFF_FFFF)
                    .map(|c| {
                        vec![
                            c.cell as i64,
                            c.map as i64,
                            c.room as i64,
                            c.state as i64,
                            c.back as i64,
                        ]
                    })
                    .collect();
                let want_cells: Vec<Vec<i64>> =
                    m["cells"].as_array().unwrap().iter().map(ints).collect();
                if got_cells != want_cells {
                    let at = want_cells
                        .iter()
                        .zip(&got_cells)
                        .position(|(a, b)| a != b)
                        .unwrap_or(0);
                    bad.push(format!(
                        "{id}: cells differ ({} vs {}; first {:?} vs {:?})",
                        want_cells.len(),
                        got_cells.len(),
                        want_cells.get(at),
                        got_cells.get(at)
                    ));
                    continue;
                }
                let got_alts: Vec<Vec<i64>> = raw
                    .iter()
                    .filter(|c| c.alt_map != 0 || c.alt_room != 0 || c.alt_state != 0)
                    .map(|c| {
                        vec![
                            c.cell as i64,
                            c.alt_map as i64,
                            c.alt_room as i64,
                            c.alt_state as i64,
                        ]
                    })
                    .collect();
                let want_alts: Vec<Vec<i64>> =
                    m["alts"].as_array().unwrap().iter().map(ints).collect();
                if got_alts != want_alts {
                    bad.push(format!("{id}: ALT cells differ"));
                }
                if i64::from(grid.center_cell) != m["center"].as_i64().unwrap()
                    || i64::from(grid.overwrite_passes) != m["passes"].as_i64().unwrap()
                    || i64::from(grid.surface.geometry().last) != m["last"].as_i64().unwrap()
                    || i64::from(grid.surface.geometry().row_len) != m["row"].as_i64().unwrap()
                {
                    bad.push(format!("{id}: centre, passes or geometry differ"));
                }
                // the tooltip's replaced room: the last recorded one per cell
                let mut was: BTreeMap<i64, (i64, i64)> = BTreeMap::new();
                for w in m["was"].as_array().unwrap() {
                    was.insert(i(&w[0]) as i64, (i(&w[1]) as i64, i(&w[2]) as i64));
                }
                for c in &grid.cells {
                    let got = c.replaced.map(|k| (i64::from(k.map), i64::from(k.room)));
                    if got != was.get(&i64::from(c.cell)).copied() {
                        bad.push(format!("{id}: replaced room of cell {} differs", c.cell));
                        break;
                    }
                }
                // the semantic view reads the same state
                if grid.cells.iter().any(|c| c.map == 0) {
                    bad.push(format!("{id}: an empty cell was listed"));
                }
                let charted = raw.iter().filter(|c| c.map != 0).count();
                if charted != grid.cells.len() {
                    bad.push(format!("{id}: cells and raw cells disagree"));
                }
            }
        }
    }
    assert!(n >= 40, "only {n} layout cases");
    assert!(
        bad.is_empty(),
        "{} of {n} differ:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

fn walk_from_events(ev: &Value) -> Walk {
    for e in ev.as_array().unwrap() {
        match e["kind"].as_str().unwrap() {
            "call" => {
                return Walk::Go(RoomKey {
                    map: i(&e["args"][0]),
                    room: i(&e["args"][1]),
                });
            }
            "msgbox" => return Walk::Message(e["text"].as_str().unwrap().to_string()),
            _ => {}
        }
    }
    Walk::Stay
}

#[test]
fn walking_matches_the_oracle() {
    let fx = load("map");
    let world = world(&fx.header["data"]);
    let mut bad = Vec::new();
    let mut n = 0;
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        if !id.starts_with("go-") {
            continue;
        }
        for step in case["inputs"]["steps"].as_array().unwrap() {
            let name = step["as"].as_str().unwrap();
            let want = walk_from_events(&case["outputs"][format!("{name}.cap")]["ev"]);
            let got = if id == "go-main" {
                go_direction(
                    &world,
                    i(&step["args"][0]),
                    i(&step["args"][1]),
                    step["args"][2].as_str().unwrap(),
                )
            } else {
                let g = &case["inputs"]["globals"];
                cmd_move(
                    &world,
                    i(&g["WnMapStartMap"]),
                    i(&g["WnMapStartRoom"]),
                    i(&step["args"][0]) as usize,
                )
            };
            n += 1;
            if got != want {
                bad.push(format!("{id} {name}: {got:?} vs oracle {want:?}"));
            }
        }
    }
    assert!(n >= 100, "only {n} walking steps");
    assert!(bad.is_empty(), "{} differ:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn what_leads_here_matches_the_oracle() {
    let fx = load("map");
    let world = world(&fx.header["data"]);
    let mut bad = Vec::new();
    let mut n = 0;
    let mut with_rows = [false; 3];
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        if !id.starts_with("leads-") {
            continue;
        }
        n += 1;
        let g = &case["inputs"]["globals"];
        let opt = LeadsHereOptions {
            only_in_game: g["bOnlyInGame"].as_bool().unwrap(),
            avg_level_max_all_stats: 0.0,
        };
        let cap = &case["outputs"]["l.cap"];
        let want: Vec<(String, String)> = cap["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r[0].as_str().unwrap().to_string(),
                    r[1].as_str().unwrap().to_string(),
                )
            })
            .collect();
        let got = leads_here(&world, i(&g["nMapStartMap"]), i(&g["nMapStartRoom"]), &opt);
        let (rows, caption, nothing) = match &got {
            None => (vec![], String::new(), false),
            Some(l) => (
                l.rows
                    .iter()
                    .map(|r| (r.text.clone(), r.tag.clone()))
                    .collect(),
                l.caption.clone().unwrap_or_default(),
                l.nothing,
            ),
        };
        for (t, _) in &want {
            if t.starts_with("Room:") {
                with_rows[0] = true;
            } else if t.starts_with("Spell:") {
                with_rows[1] = true;
            } else if t.starts_with("Monster:") {
                with_rows[2] = true;
            }
        }
        let want_nothing = cap["ev"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["text"] == json!("Nothing."));
        if rows != want || caption != cap["caption"].as_str().unwrap() || nothing != want_nothing {
            bad.push(format!(
                "{id}: {} rows vs {}; caption {caption:?} vs {:?}; nothing {nothing} vs {want_nothing}\n  port {rows:?}\n  mme  {want:?}",
                rows.len(),
                want.len(),
                cap["caption"]
            ));
        }
    }
    assert!(n >= 8, "only {n} leads-here cases");
    assert!(
        with_rows.iter().all(|b| *b),
        "rows of every kind: {with_rows:?}"
    );
    assert!(bad.is_empty(), "{} differ:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn default_presets_are_mmes() {
    // expected value: LoadPresets' Select Case, read by the generator into the fixture's data
    let fx = load("map");
    let want = fx.header["data"]["presets"].as_array().unwrap();
    let got = presets();
    assert_eq!(got.len(), 50);
    assert_eq!(want.len(), 30);
    for (k, w) in want.iter().enumerate() {
        assert_eq!(got[k].name, w[0].as_str().unwrap());
        assert_eq!(i64::from(got[k].map), w[1].as_i64().unwrap());
        assert_eq!(i64::from(got[k].room), w[2].as_i64().unwrap());
    }
    for p in &got[30..] {
        // the Case Else of LoadPresets
        assert_eq!((p.name.as_str(), p.map, p.room), ("unset", 1, 1));
    }
}

#[test]
fn legend_names_every_line_kind() {
    let l = legend();
    assert!(l.iter().any(|e| e.id == "Hidden"));
    assert_eq!(l.iter().filter(|e| e.group == "line").count(), 9);
}

#[test]
fn the_semantic_view_reads_the_same_state() {
    use touchstone_rules::map::{CellColor, Marker, SegmentKind, UnplacedReason};
    let fx = load("map");
    let world = world(&fx.header["data"]);
    let go = |surface, start: (i32, i32), options: MapOptions| {
        layout(
            &world,
            RoomKey {
                map: start.0,
                room: start.1,
            },
            &MapRequest::new(surface, options),
        )
        .unwrap()
    };
    // invented rows: the hub (9001/1) has up and down exits, an NPC, a lair, a command and an exit to a room the data lacks
    let hub = go(
        MapSurface::Main,
        (9001, 1),
        MapOptions {
            follow_map_changes: true,
            ..MapOptions::default()
        },
    );
    let c = hub
        .cells
        .iter()
        .find(|c| c.cell == hub.center_cell)
        .unwrap();
    assert_eq!((c.map, c.room, c.key.as_str()), (9001, 1, "9001/1"));
    assert_eq!(c.color, CellColor::UpDown);
    for m in [Marker::Start, Marker::Npc, Marker::Lair, Marker::Command] {
        assert!(c.markers.contains(&m), "{m:?}");
    }
    assert!(hub.missing.contains(&RoomKey {
        map: 9001,
        room: 99
    }));
    // the labyrinth reaches rooms by several paths; without "allow dupes" MME keeps one cell for each
    let maze = go(MapSurface::Main, (1, 1917), MapOptions::default());
    assert!(
        maze.unplaced
            .iter()
            .any(|u| u.reason == UnplacedReason::Duplicate)
    );
    assert_eq!((c.x, c.y), (14, 11));
    // a corridor longer than the grid leaves it on both sides
    let cor = go(MapSurface::Main, (9003, 30), MapOptions::default());
    for dir in [2, 3] {
        assert!(
            cor.unplaced
                .iter()
                .any(|u| u.reason == UnplacedReason::OffGrid && u.dir == dir)
        );
        assert!(
            cor.segments
                .iter()
                .any(|s| s.kind == SegmentKind::Edge && s.dir == dir)
        );
    }
    // the same rooms with overwrite on replace a cell and say which room they displaced
    let over = go(
        MapSurface::Main,
        (9001, 1),
        MapOptions {
            follow_map_changes: true,
            allow_overwrite: true,
            ..MapOptions::default()
        },
    );
    assert!(over.overwrite_passes > 0);
    assert!(over.cells.iter().any(|c| c.replaced.is_some()));
    assert!(
        over.cells
            .iter()
            .any(|c| c.markers.contains(&Marker::Overwritten))
    );
}
