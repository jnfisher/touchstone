//! The spell tools endpoints replaying cases of fixtures/rules/{spell_tools,chests,spell_book}.json (MME's own
//! procedures run by the oracle) through the wire. The rules are pinned in the rules crate; this checks the
//! requests and responses.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use touchstone::state::{Loaded, SourceInfo};
use touchstone::{AppState, app};
use touchstone_model::{AbilitySlot, Class, Info, Item, Spell, TbInfo, World, WorldParts};
use tower::ServiceExt;

fn int(v: &Value) -> i32 {
    i32::try_from(v.as_i64().unwrap()).unwrap()
}

fn fixture(family: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../fixtures/rules/{family}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn item(number: i32, name: &str, item_type: i32) -> Item {
    Item {
        number,
        name: name.to_string(),
        limit: 0,
        encum: 0,
        item_type,
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
    }
}

fn spell(number: i32, name: &str) -> Spell {
    Spell {
        number,
        name: Some(name.to_string()),
        short: None,
        req_level: 0,
        energy_cost: 0,
        mana_cost: 0,
        min_base: 0,
        max_base: 0,
        diff: 0,
        type_of_resists: 0,
        targets: 0,
        dur: 0,
        att_type: 0,
        magery: 0,
        magery_lvl: 0,
        cap: 0,
        max_inc_lvls: 0,
        max_inc: 0,
        min_inc_lvls: 0,
        min_inc: 0,
        dur_inc_lvls: 0,
        dur_inc: 0,
        abilities: [AbilitySlot::default(); 10],
        learnable: false,
        learned_from: None,
        casted_by: None,
        classes: None,
    }
}

fn parts(items: Vec<Item>, spells: Vec<Spell>, classes: Vec<Class>, tbinfo: Vec<TbInfo>) -> World {
    World::new(WorldParts {
        info: Info {
            nmr_version: "v1.8.3".into(),
            dat_file_version: "v1.11p".into(),
            date: String::new(),
            time: String::new(),
            custom: String::new(),
            legit: false,
            disable_kai: false,
            update_url: String::new(),
        },
        items,
        monsters: vec![],
        spells,
        rooms: vec![],
        shops: vec![],
        classes,
        races: vec![],
        lairs: vec![],
        tbinfo,
    })
    .unwrap()
}

fn serve(w: World) -> (axum::Router, TempDir) {
    let dir = TempDir::new().unwrap();
    let info = SourceInfo {
        kind: "mdb",
        path: "test".into(),
    };
    let loaded = Loaded::build(w, info, None, 1).unwrap();
    (
        app(AppState::with_data_dir(loaded, dir.path().join("data"))),
        dir,
    )
}

async fn call(app: &axum::Router, method: &str, uri: &str, body: &Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, v)
}

fn spell_tools_world(data: &Value) -> World {
    let items = data["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut it = item(int(&r[0]), r[1].as_str().unwrap(), 0);
            it.in_game = int(&r[2]) != 0;
            for (k, n) in r[3].as_array().unwrap().iter().enumerate() {
                it.negate_spell[k] = int(n);
            }
            for (k, p) in r[4].as_array().unwrap().chunks(2).enumerate() {
                it.abilities[k] = AbilitySlot {
                    ability: int(&p[0]),
                    value: int(&p[1]),
                };
            }
            it
        })
        .collect();
    let spells = data["spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let n = int(&r[0]);
            let mut s = spell(n, r[1].as_str().unwrap());
            s.learnable = int(&r[2]) != 0;
            s.min_base = int(&r[5]);
            s.max_base = int(&r[6]);
            s.min_inc = int(&r[7]);
            s.max_inc = int(&r[8]);
            s.min_inc_lvls = int(&r[9]);
            s.max_inc_lvls = int(&r[10]);
            s.dur = int(&r[11]);
            s.dur_inc = int(&r[12]);
            s.dur_inc_lvls = int(&r[13]);
            s.cap = int(&r[14]);
            s.req_level = int(&r[15]);
            for (k, p) in r[16].as_array().unwrap().chunks(2).enumerate() {
                s.abilities[k] = AbilitySlot {
                    ability: int(&p[0]),
                    value: int(&p[1]),
                };
            }
            // lengths and the position of "item #" are all the counters search reads
            let (len, pos) = (int(&r[3]) as usize, int(&r[4]) as usize);
            let text = match data["cast"].get(n.to_string()).and_then(Value::as_str) {
                Some(t) => t.to_string(),
                None => {
                    let mut t = "x".repeat(len);
                    if pos > 0 {
                        t.replace_range(pos - 1..pos + 5, "item #");
                    }
                    t
                }
            };
            s.casted_by = (!text.is_empty()).then_some(text);
            s
        })
        .collect();
    parts(items, spells, vec![], vec![])
}

#[tokio::test]
async fn counters_and_casts_agree_with_the_oracle() {
    let doc = fixture("spell_tools");
    let (app, _dir) = serve(spell_tools_world(&doc["header"]["data"]));
    let mut seen = 0;
    for case in doc["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let (proc, list) = if id.starts_with("menu-") {
            ("mnuSpellsPopUpItem_Click", true)
        } else if id.starts_with("pane-") {
            ("mnuAuxPopUpItem_Click", false)
        } else {
            continue;
        };
        let wl = &case["inputs"]["work_list"];
        let cap = &case["outputs"][format!("{proc}.cap")];
        let text = if list { &wl["text"] } else { &wl["subtag"] };
        let spell_no = int(&json!(
            text.as_str()
                .unwrap()
                .split('@')
                .next()
                .unwrap()
                .parse::<i64>()
                .unwrap_or(0)
        ));
        if spell_no <= 0 {
            continue; // the handler does nothing for a row without a number
        }
        let mut body = if list {
            json!({"spell": spell_no})
        } else {
            json!({"spell": spell_no, "tag": text})
        };
        let (status, mut v) = call(&app, "POST", "/api/spells/counters", &body).await;
        assert_eq!(status, StatusCode::OK, "{id}");
        if v["ask_level"] == true {
            // MME asked; a case without an answer cancelled
            let answer = case["inputs"]["inputbox"][0]
                .as_str()
                .map_or(0.0, |s| s.parse::<f64>().unwrap_or(0.0));
            body["level_answer"] = json!(answer);
            v = call(&app, "POST", "/api/spells/counters", &body).await.1;
        }
        assert_eq!(v["level"], cap["lv"], "{id} level");
        let got: Vec<Value> = v["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| json!([r["text"], r["number"].to_string().trim_matches('"')]))
            .collect();
        let want: Vec<Value> = cap["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| json!([r[0], r[1]]))
            .collect();
        assert_eq!(got, want, "{id} rows");
        seen += 1;
    }
    assert!(seen > 40);

    // what casts: a spell cast by a spell, one MME has nothing for, one that is not there
    let (_, v) = call(&app, "GET", "/api/spells/1097/casts", &json!({})).await;
    assert_eq!(v["status"], "found");
    assert_eq!(v["caption"], "Spell balanced word(1097) is casted by:");
    assert_eq!(v["rows"][0]["text"], "Spell: balanced word(1096)");
    assert_eq!(v["rows"][0]["target"]["table"], "spells");
    let (_, v) = call(&app, "GET", "/api/spells/2/casts", &json!({})).await;
    assert_eq!(v["status"], "nothing");
    let (_, v) = call(&app, "GET", "/api/spells/99999/casts", &json!({})).await;
    assert_eq!(v["status"], "missing");
}

#[tokio::test]
async fn chests_agree_with_the_oracle() {
    let doc = fixture("chests");
    let data = &doc["header"]["data"];
    let items = data["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut it = item(int(&r[0]), r[1].as_str().unwrap(), int(&r[2]));
            for (k, v) in r[3].as_array().unwrap().iter().enumerate() {
                if int(v) != 0 {
                    it.abilities[k] = AbilitySlot {
                        ability: 43,
                        value: int(v),
                    };
                }
            }
            it
        })
        .collect();
    let spells = data["spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut s = spell(int(&r[0]), "chest");
            s.min_base = int(&r[1]);
            s.max_base = int(&r[2]);
            for (k, p) in r[3].as_array().unwrap().chunks(2).enumerate() {
                s.abilities[k] = AbilitySlot {
                    ability: int(&p[0]),
                    value: int(&p[1]),
                };
            }
            s
        })
        .collect();
    let tb = data["tb"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| TbInfo {
            number: int(&r[0]),
            link_to: 0,
            action: Some(r[1].as_str().unwrap().to_string()),
            called_from: String::new(),
        })
        .collect();
    let (app, _dir) = serve(parts(items, spells, vec![], tb));
    for id in [
        "chest-906",
        "chest-1801",
        "chest-9309",
        "chest-9312",
        "chest-9306",
        "chest-9318",
    ] {
        let case = doc["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .unwrap();
        let n = int(&json!(
            id.rsplit('-').next().unwrap().parse::<i64>().unwrap()
        ));
        let hide = case["inputs"]["globals"]["bHideRecordNumbers"] == true;
        let (status, v) = call(
            &app,
            "GET",
            &format!("/api/items/{n}/chest?hide_record_numbers={hide}"),
            &json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{id}");
        let want = case["outputs"]["cmdSundryChests_Click.cap"]["rows"]
            .as_array()
            .unwrap();
        let events = case["events"].as_array().cloned().unwrap_or_default();
        let msg = events.iter().find(|e| e["kind"] == "msgbox");
        assert_eq!(
            v["message"].as_str(),
            msg.and_then(|e| e["text"].as_str()),
            "{id}"
        );
        // the list is sorted by chance, highest first, and holds the same rows
        let mut got: Vec<(String, String)> = v["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["text"].as_str().unwrap().into(),
                    r["item_text"].as_str().unwrap().into(),
                )
            })
            .collect();
        let pcts: Vec<f64> = v["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["percent"].as_f64().unwrap())
            .collect();
        assert!(pcts.windows(2).all(|w| w[0] >= w[1]), "{id} order");
        got.sort();
        let mut exp: Vec<(String, String)> = want
            .iter()
            .map(|r| {
                (
                    r[0].as_str().unwrap().into(),
                    r[2][0][0].as_str().unwrap().into(),
                )
            })
            .collect();
        exp.sort();
        assert_eq!(got, exp, "{id}");
        if hide {
            // the hide_record_numbers setting stands in for the query value
            let (st, _) = call(
                &app,
                "PUT",
                "/api/settings",
                &json!({"hide_record_numbers": true}),
            )
            .await;
            assert_eq!(st, StatusCode::OK, "{id}");
            let (st, set) = call(&app, "GET", &format!("/api/items/{n}/chest"), &json!({})).await;
            assert_eq!(st, StatusCode::OK, "{id}");
            assert_eq!(set, v, "{id}: setting vs query");
            let (st, _) = call(&app, "PUT", "/api/settings", &json!({})).await;
            assert_eq!(st, StatusCode::OK, "{id}");
        }
    }
}

#[tokio::test]
async fn spell_book_agrees_with_the_oracle() {
    let doc = fixture("spell_book");
    let data = &doc["header"]["data"];
    let spells = data["spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut s = spell(int(&r[0]), r[1].as_str().unwrap());
            s.learnable = int(&r[2]) != 0;
            s.learned_from = (int(&r[3]) > 0).then(|| "x".repeat(int(&r[3]) as usize));
            s.casted_by = (int(&r[4]) > 0).then(|| "x".repeat(int(&r[4]) as usize));
            s.magery = int(&r[5]);
            s.magery_lvl = int(&r[6]);
            s.req_level = int(&r[7]);
            let c = r[8].as_str().unwrap();
            s.classes = (!c.is_empty()).then(|| c.to_string());
            for (k, p) in r[9].as_array().unwrap().chunks(2).enumerate() {
                s.abilities[k] = AbilitySlot {
                    ability: int(&p[0]),
                    value: int(&p[1]),
                };
            }
            s
        })
        .collect();
    let classes: Vec<Class> = data["classes"]
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
        .collect();
    let (app, _dir) = serve(parts(vec![], spells, classes, vec![]));
    for case in doc["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let f = &case["inputs"]["forms"]["frmMain"];
        let g = &case["inputs"]["globals"];
        let items: Vec<i32> = f["cmbClass"]["Items"]
            .as_array()
            .unwrap()
            .iter()
            .map(int)
            .collect();
        let class = items[int(&f["cmbClass"]["ListIndex"]) as usize];
        match case["inputs"]["steps"][0]["call"].as_str().unwrap() {
            "cmdListSpells_Click" => {
                // a class below 1 and the other server-side switches (data version, Kai autolearn, in-game)
                // are the world's: only cases on the server's own settings are replayed here
                if g["nNMRVer"] != 1.83
                    || g["bOnlyInGame"] != true
                    || g["bDisableKaiAutolearn"] == true
                {
                    continue;
                }
                let g0 = &f["cmbGlobalClass"]["$array"]["0"];
                let g0_items: Vec<i32> = g0["Items"].as_array().unwrap().iter().map(int).collect();
                let body = json!({
                    "class": class,
                    "level": f["txtLevel"]["Text"],
                    "alignment": f["cmbAlignment"]["ListIndex"],
                    "magery": f["cmbSpellMagery"]["ListIndex"],
                    "magery_level": f["cmbSpellMageryLevel"]["ListIndex"],
                    "filter_on": f["chkGlobalFilter"]["Value"] == 1,
                    "global_level": f["txtGlobalLevel"]["$array"]["1"]["Text"],
                    "global_class": g0_items[int(&g0["ListIndex"]) as usize],
                    "char_magery": g.get("nGlobalCharMagery").map_or(0, int),
                    "char_magery_level": g.get("nGlobalCharMageryLVL").map_or(0, int),
                });
                let (status, v) = call(&app, "POST", "/api/spell-book/list", &body).await;
                assert_eq!(status, StatusCode::OK, "{id}");
                let cap = &case["outputs"]["cmdListSpells_Click.cap"];
                let want: Vec<i32> = cap["rows"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| r.as_str().unwrap().parse().unwrap())
                    .collect();
                let got: Vec<i32> = v["spells"].as_array().unwrap().iter().map(int).collect();
                assert_eq!(got, want, "{id}");
                if v["aborted"] != true {
                    assert_eq!(v["uses_character"], cap["char"], "{id}");
                }
            }
            "cmbClass_Click" if class > 0 => {
                let (_, v) = call(
                    &app,
                    "POST",
                    "/api/spell-book/class",
                    &json!({"class": class}),
                )
                .await;
                let cap = &case["outputs"]["cmbClass_Click.cap"];
                if f["cmbSpellMagery"]["ListCount"] == 6 {
                    assert_eq!(v["magery_level"], cap["mlvl"], "{id}");
                    assert_eq!(v["magery"], cap["m"], "{id}");
                }
            }
            _ => {}
        }
    }
}
