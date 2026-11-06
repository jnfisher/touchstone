//! `/api/rooms/...`, pinned to fixtures/rules/rooms.json (frmMain MapGetRoomLoc, MapMapExits,
//! cmdMapFindText_Click, FindRoomWithDirections, ShowRoomCmdRoomList); the world is rebuilt from
//! the fixture header. The fixture answers GetLairInfo from its own table and the damage arrays
//! from the case, the server from the lair cache and the stored average damage, so tooltips of
//! rooms whose lair group the table lists, and the `dmg-*` cases, are left out.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use touchstone::state::{Loaded, SourceInfo};
use touchstone::{AppState, app};
use touchstone_model::{
    AbilitySlot, Class, DropSlot, Info, Item, MidSpell, Monster, MonsterAttack, Race, Room, Shop,
    ShopSlot, Spell, TbInfo, World, WorldParts,
};
use tower::ServiceExt;

fn i(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
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

fn fixture() -> Value {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rules/rooms.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A table of the header's data as objects keyed by field name.
fn table(v: &Value) -> Vec<serde_json::Map<String, Value>> {
    let fields: Vec<&str> = v["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap())
        .collect();
    v["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            fields
                .iter()
                .zip(r.as_array().unwrap())
                .map(|(f, x)| ((*f).to_string(), x.clone()))
                .collect()
        })
        .collect()
}

fn world(data: &Value, nmr: f64) -> World {
    let items = table(&data["items"])
        .iter()
        .map(|r| Item {
            number: i(&r["Number"]),
            name: s(&r["Name"]),
            limit: 0,
            encum: 0,
            item_type: 0,
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
            abilities: [AbilitySlot::default(); 20],
            in_game: true,
            obtained_from: None,
            references: None,
        })
        .collect();
    let monsters = table(&data["monsters"])
        .iter()
        .map(|r| Monster {
            number: i(&r["Number"]),
            name: Some(s(&r["Name"])),
            weapon: 0,
            armour_class: 0,
            damage_resist: 0,
            follow_pct: 0,
            magic_res: 0,
            bs_defense: 0,
            exp: 0.0,
            exp_multi: 1.0,
            hp: 10,
            energy: 1000,
            avg_dmg: r["AvgDmg"].as_f64().unwrap(),
            avg_dmg_phys: 0.0,
            avg_dmg_spell: 0.0,
            greet_txt: i(&r["GreetTXT"]),
            hp_regen: 0,
            charm_lvl: 0,
            r#type: 0,
            undead: 0,
            align: 0,
            regen_time: 0,
            game_limit: 0,
            avg_lair_exp: 0.0,
            script_value: 0.0,
            r: 0,
            p: 0,
            g: 0,
            s: 0,
            c: 0,
            death_spell: 0,
            create_spell: 0,
            attacks: std::array::from_fn(|_| MonsterAttack {
                name: String::new(),
                att_type: 0,
                acc: 0,
                pct: 0,
                true_pct: 0.0,
                min: 0,
                max: 0,
                energy: 0,
                hit_spell: 0,
            }),
            mid_spells: [MidSpell::default(); 5],
            drops: [DropSlot::default(); 10],
            abilities: [AbilitySlot::default(); 10],
            in_game: true,
            summoned_by: None,
        })
        .collect();
    let spells = table(&data["spells"])
        .iter()
        .map(|r| Spell {
            number: i(&r["Number"]),
            name: Some(s(&r["Name"])),
            short: None,
            req_level: i(&r["ReqLevel"]),
            energy_cost: i(&r["EnergyCost"]),
            mana_cost: i(&r["ManaCost"]),
            min_base: i(&r["MinBase"]),
            max_base: i(&r["MaxBase"]),
            diff: i(&r["Diff"]),
            type_of_resists: i(&r["TypeOfResists"]),
            targets: i(&r["Targets"]),
            dur: i(&r["Dur"]),
            att_type: i(&r["AttType"]),
            magery: i(&r["Magery"]),
            magery_lvl: i(&r["MageryLVL"]),
            cap: i(&r["Cap"]),
            max_inc_lvls: i(&r["MaxIncLVLs"]),
            max_inc: i(&r["MaxInc"]),
            min_inc_lvls: i(&r["MinIncLVLs"]),
            min_inc: i(&r["MinInc"]),
            dur_inc_lvls: i(&r["DurIncLVLs"]),
            dur_inc: i(&r["DurInc"]),
            abilities: pairs(&r["Abilities"]),
            learnable: i(&r["Learnable"]) != 0,
            learned_from: None,
            casted_by: None,
            classes: Some(s(&r["Classes"])),
        })
        .collect();
    let shops = table(&data["shops"])
        .iter()
        .map(|r| Shop {
            number: i(&r["Number"]),
            name: s(&r["Name"]),
            shop_type: 0,
            min_lvl: 0,
            max_lvl: 0,
            markup_pct: 0,
            class_rest: 0,
            stock: [ShopSlot::default(); 20],
            in_game: true,
            assigned_to: None,
        })
        .collect();
    let classes = table(&data["classes"])
        .iter()
        .map(|r| Class {
            number: i(&r["Number"]),
            name: s(&r["Name"]),
            min_hits: i(&r["MinHits"]),
            max_hits: i(&r["MaxHits"]),
            exp_table: i(&r["ExpTable"]),
            magery_type: i(&r["MageryType"]),
            magery_lvl: i(&r["MageryLVL"]),
            weapon_type: i(&r["WeaponType"]),
            armour_type: i(&r["ArmourType"]),
            combat_lvl: i(&r["CombatLVL"]),
            abilities: pairs(&r["Abilities"]),
        })
        .collect();
    let races = table(&data["races"])
        .iter()
        .map(|r| Race {
            number: i(&r["Number"]),
            name: s(&r["Name"]),
            m_int: i(&r["mINT"]),
            m_wil: i(&r["mWIL"]),
            m_str: i(&r["mSTR"]),
            m_hea: i(&r["mHEA"]),
            m_agl: i(&r["mAGL"]),
            m_chm: i(&r["mCHM"]),
            x_int: i(&r["xINT"]),
            x_wil: i(&r["xWIL"]),
            x_str: i(&r["xSTR"]),
            x_hea: i(&r["xHEA"]),
            x_agl: i(&r["xAGL"]),
            x_chm: i(&r["xCHM"]),
            hp_per_lvl: i(&r["HPPerLVL"]),
            exp_table: i(&r["ExpTable"]),
            base_cp: i(&r["BaseCP"]),
            abilities: pairs(&r["Abilities"]),
        })
        .collect();
    let tbinfo = table(&data["tbinfo"])
        .iter()
        .map(|r| TbInfo {
            number: i(&r["Number"]),
            link_to: i(&r["LinkTo"]),
            action: Some(s(&r["Action"])),
            called_from: String::new(),
        })
        .collect();
    let rooms = table(&data["rooms"])
        .iter()
        .map(|r| Room {
            map_number: i(&r["Map Number"]),
            room_number: i(&r["Room Number"]),
            name: Some(s(&r["Name"])),
            light: i(&r["Light"]),
            shop: i(&r["Shop"]),
            npc: i(&r["NPC"]),
            cmd: i(&r["CMD"]),
            spell: i(&r["Spell"]),
            lair: Some(s(&r["Lair"])),
            delay: i(&r["Delay"]),
            placed: Some(s(&r["Placed"])),
            n: s(&r["N"]),
            s: s(&r["S"]),
            e: s(&r["E"]),
            w: s(&r["W"]),
            ne: s(&r["NE"]),
            nw: s(&r["NW"]),
            se: s(&r["SE"]),
            sw: s(&r["SW"]),
            u: s(&r["U"]),
            d: s(&r["D"]),
        })
        .collect();
    World::new(WorldParts {
        info: Info {
            nmr_version: format!("v{nmr}"),
            dat_file_version: "v1.11p".to_owned(),
            date: String::new(),
            time: String::new(),
            custom: String::new(),
            legit: false,
            disable_kai: false,
            update_url: String::new(),
        },
        items,
        monsters,
        spells,
        rooms,
        shops,
        classes,
        races,
        lairs: vec![],
        tbinfo,
    })
    .unwrap()
}

fn router(world: World) -> axum::Router {
    let info = SourceInfo {
        kind: "mdb",
        path: "test".into(),
    };
    let loaded = Loaded::build(world, info, None, 1).unwrap();
    app(AppState::with_data_dir(
        loaded,
        tempfile::TempDir::new().unwrap().keep(),
    ))
}

async fn call(app: &axum::Router, uri: &str, body: Option<&Value>) -> (StatusCode, Value) {
    let req = match body {
        Some(b) => Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(b.to_string()))
            .unwrap(),
        None => Request::get(uri).body(Body::empty()).unwrap(),
    };
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, v)
}

fn setup_or_case<'a>(fx: &'a Value, case: &'a Value, name: &str) -> &'a Value {
    case["inputs"]["forms"]["frmMain"]
        .get(name)
        .unwrap_or(&fx["header"]["setup"]["forms"]["frmMain"][name])
}

fn nmr_of(fx: &Value, case: &Value) -> f64 {
    case["inputs"]["globals"]
        .get("nNMRVer")
        .unwrap_or(&fx["header"]["setup"]["globals"]["nNMRVer"])
        .as_f64()
        .unwrap()
}

fn query_of(fx: &Value, case: &Value) -> String {
    let flag = |idx: &str| {
        setup_or_case(fx, case, "chkMapOptions")["$array"][idx]["Value"]
            .as_i64()
            .unwrap_or(0)
            == 1
    };
    let hide = case["inputs"]["globals"]
        .get("bHideRecordNumbers")
        .unwrap_or(&fx["header"]["setup"]["globals"]["bHideRecordNumbers"])
        .as_bool()
        .unwrap();
    let illu = setup_or_case(fx, case, "lblInvenCharStat")["$array"]["23"]["Caption"]
        .as_str()
        .map_or(0, |c| c.parse::<i32>().unwrap());
    format!(
        "hide_numbers={hide}&hide_npc={}&hide_lairs={}&hide_cmds={}&all_exits={}&illu={illu}",
        flag("3"),
        flag("2"),
        flag("4"),
        flag("6")
    )
}

#[tokio::test]
async fn room_references_and_tooltips_match_the_oracle() {
    let fx = fixture();
    let data = &fx["header"]["data"];
    let lair_groups: Vec<&str> = data["lairs"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let mut apps: Vec<(f64, axum::Router)> = Vec::new();
    let (mut refs_checked, mut tips_checked) = (0, 0);
    let mut bad = Vec::new();
    for case in fx["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        if id.starts_with("dmg-") || id == "room-missing" {
            continue;
        }
        let steps = case["inputs"]["steps"].as_array().unwrap();
        if !steps.iter().any(|st| st["call"] == "MapGetRoomLoc") {
            continue;
        }
        let nmr = nmr_of(&fx, case);
        if !apps.iter().any(|(n, _)| *n == nmr) {
            apps.push((nmr, router(world(data, nmr))));
        }
        let app = &apps.iter().find(|(n, _)| *n == nmr).unwrap().1;
        let query = query_of(&fx, case);
        for st in steps.iter().filter(|st| st["call"] == "MapGetRoomLoc") {
            let (m, r) = (i(&st["args"][0]), i(&st["args"][1]));
            let tag = st["as"].as_str().unwrap();
            let (status, got) = call(app, &format!("/api/rooms/{m}/{r}?{query}"), None).await;
            assert_eq!(status, StatusCode::OK, "{id} {m}/{r}");
            // the fixture row is [text, tag, cells]
            let want = &case["outputs"][format!("{tag}.cap")]["refs"];
            let have: Vec<Value> = got["refs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| json!([x["text"], x["tag"], []]))
                .collect();
            refs_checked += 1;
            if want != &Value::Array(have.clone()) {
                bad.push(format!(
                    "{id} {m}/{r} refs\n  got  {}\n  want {want}",
                    Value::Array(have)
                ));
            }
            let room = data["rooms"]["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|x| i(&x[0]) == m && i(&x[1]) == r)
                .unwrap();
            let lair = room[8].as_str().unwrap();
            let group = lair
                .rsplit(',')
                .next()
                .map(|g| g.trim_matches(|c| c == '[' || c == ']'))
                .and_then(|g| g.rsplit_once('-').map(|x| x.0))
                .unwrap_or("");
            if lair_groups.contains(&group) {
                continue;
            }
            let want_tip = &case["outputs"][format!("{}.cap", tag.replacen('r', "rt", 1))]["tip"];
            tips_checked += 1;
            if &got["tooltip"] != want_tip {
                bad.push(format!(
                    "{id} {m}/{r} tooltip\n  got  {}\n  want {want_tip}",
                    got["tooltip"]
                ));
            }
        }
    }
    assert!(
        refs_checked > 150 && tips_checked > 100,
        "{refs_checked} {tips_checked}"
    );
    assert!(
        bad.is_empty(),
        "{} differ; first:\n{}",
        bad.len(),
        bad.iter().take(5).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[tokio::test]
async fn a_room_row_links_and_exits_are_listed() {
    let fx = fixture();
    let app = router(world(&fx["header"]["data"], 1.83));
    // refs-npc: 1/11 has an NPC, an item and a lair; 9001/1 has exits of many kinds
    let (_, v) = call(&app, "/api/rooms/9001/1", None).await;
    assert_eq!(v["title"], "Rooms -- Test Hall (9001/1)  ");
    let kinds: Vec<&str> = v["exits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert_eq!(&kinds[..3], ["normal", "toll", "door"]);
    let npc = &v["refs"][0];
    assert_eq!(npc["target"]["table"], "monsters");
    assert_eq!(npc["target"]["number"], 9001);
    let (status, v) = call(&app, "/api/rooms/9999/1", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["found"], false);
    assert_eq!(v["message"], "Room 9999/1 was not found.");
}

#[tokio::test]
async fn find_text_matches_the_oracle_chain() {
    let fx = fixture();
    let app = router(world(&fx["header"]["data"], 1.83));
    let mut n = 0;
    for case in fx["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        if !id.starts_with("text-") || case["inputs"]["steps"][0]["args"][0] != 0 {
            continue;
        }
        let q = case["inputs"]["inputbox"][0].as_str().unwrap();
        if q.is_empty() || id == "text-restart" || id == "text-new-after-hit" || id == "text-end" {
            continue;
        }
        let chain: Vec<(i32, i32)> = case["events"]
            .as_array()
            .unwrap()
            .iter()
            .take_while(|e| e["kind"] != "msgbox")
            .filter(|e| e["kind"] == "call")
            .map(|e| (i(&e["args"][0]), i(&e["args"][1])))
            .collect();
        let (status, got) = call(
            &app,
            &format!(
                "/api/rooms/find-text?q={}&limit=1000",
                q.replace(' ', "%20")
            ),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{id}");
        let have: Vec<(i32, i32)> = got["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| (i(&m["map"]), i(&m["room"])))
            .collect();
        assert_eq!(&have[..chain.len().min(have.len())], &chain[..], "{id}");
        if chain.is_empty() {
            assert_eq!(got["message"], "Name not found.", "{id}");
        }
        n += 1;
    }
    assert!(n >= 4, "{n}");
    let (_, v) = call(&app, "/api/rooms/find-text?q=a&limit=3", None).await;
    assert_eq!(v["matches"].as_array().unwrap().len(), 3);
    assert_eq!(v["truncated"], true);
    let (_, v) = call(&app, "/api/rooms/find-text?q=a&limit=3&after=1/142", None).await;
    assert!(i(&v["matches"][0]["room"]) > 142 || i(&v["matches"][0]["map"]) > 1);
    let (status, _) = call(&app, "/api/rooms/find-text?q=a&limit=0", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn find_exits_and_room_command_lists_match_the_oracle() {
    let fx = fixture();
    let app = router(world(&fx["header"]["data"], 1.83));
    let (mut exits, mut lists) = (0, 0);
    for case in fx["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        if id.starts_with("exits-") {
            let f = &case["inputs"]["forms"];
            if f["frmMain"]["txtRoomFindOk"]["Text"] != "1" {
                continue;
            }
            let dirs = ["N", "S", "E", "W", "NE", "NW", "SE", "SW", "U", "D"];
            let on: Vec<&str> = dirs
                .iter()
                .enumerate()
                .filter(|(k, _)| {
                    f["frmPopUpOptions"]["cmdRoomFindDir"]["$array"][k.to_string()]["Tag"] == "1"
                })
                .map(|(_, d)| *d)
                .collect();
            let body = json!({
                "name": f["frmPopUpOptions"]["txtRoomName"]["Text"],
                "exact": f["frmPopUpOptions"]["optRoomFindMatch"]["$array"]["1"]["Value"],
                "exits": on,
            });
            let (status, got) = call(&app, "/api/rooms/find-exits", Some(&body)).await;
            assert_eq!(status, StatusCode::OK, "{id}");
            let want = &case["outputs"]["f.cap"];
            let rows: Vec<Value> = got["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| json!([r["text"], r["tag"], []]))
                .collect();
            assert_eq!(want["rows"], Value::Array(rows), "{id}");
            assert_eq!(want["caption"], got["caption"], "{id}");
            let msgs: Vec<&Value> = case["events"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter(|e| e["kind"] == "msgbox")
                        .map(|e| &e["text"])
                        .collect()
                })
                .unwrap_or_default();
            let have: Vec<&Value> = got["messages"].as_array().unwrap().iter().collect();
            assert_eq!(msgs, have, "{id}");
            exits += 1;
        } else if id.starts_with("cmdrooms-") && id != "cmdrooms-empty" {
            let st = &case["inputs"]["steps"][0];
            let body =
                json!({"list": st["args"][0], "row_text": st["args"][1], "percent": st["args"][2]});
            let (status, got) = call(&app, "/api/rooms/cmd-rooms", Some(&body)).await;
            assert_eq!(status, StatusCode::OK, "{id}");
            let want = &case["outputs"]["l.cap"];
            let rows: Vec<Value> = got["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| json!([r["text"], r["tag"], []]))
                .collect();
            assert_eq!(want["rows"], Value::Array(rows), "{id}");
            assert_eq!(want["caption"], got["caption"], "{id}");
            lists += 1;
        }
    }
    assert!(exits >= 15 && lists >= 2, "{exits} {lists}");
    let (status, _) = call(
        &app,
        "/api/rooms/find-exits",
        Some(&json!({"name": "abc", "exits": ["Q"]})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(&app, "/api/rooms/cmd-rooms", Some(&json!({"list": ""}))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
