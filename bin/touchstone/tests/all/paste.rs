//! `/api/paste/*`: the dialog's endpoints against cases of fixtures/rules/paste_char.json and
//! paste_party.json (worlds rebuilt from the fixture headers), the question loop, and the
//! Char-tab reset.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use touchstone::state::{Loaded, SourceInfo};
use touchstone::{AppState, app};
use touchstone_model::{
    AbilitySlot, Class, Info, Item, Race, Room, Shop, ShopSlot, Spell, World, WorldParts,
};
use tower::ServiceExt;

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

pub(crate) fn char_world(data: &Value) -> World {
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
            name: r["name"].as_str().unwrap().to_string(),
            min_hits: 0,
            max_hits: 0,
            exp_table: 0,
            magery_type: i(&r["mt"]),
            magery_lvl: i(&r["ml"]),
            weapon_type: i(&r["wt"]),
            armour_type: i(&r["at"]),
            combat_lvl: 0,
            abilities: pairs(&r["ab"]),
        })
        .collect();
    let races = data["races"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let m: Vec<i32> = r["mins"].as_array().unwrap().iter().map(i).collect();
            Race {
                number: i(&r["n"]),
                name: r["name"].as_str().unwrap().to_string(),
                m_str: m[0],
                m_int: m[1],
                m_wil: m[2],
                m_agl: m[3],
                m_hea: m[4],
                m_chm: m[5],
                x_int: 0,
                x_wil: 0,
                x_str: 0,
                x_hea: 0,
                x_agl: 0,
                x_chm: 0,
                hp_per_lvl: 0,
                exp_table: 0,
                base_cp: 0,
                abilities: [AbilitySlot::default(); 10],
            }
        })
        .collect();
    let spells = data["spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Spell {
            number: i(&r["n"]),
            name: opt_str(&r["name"]),
            short: (i(&r["short"]) != 0).then(|| "x".to_string()),
            req_level: i(&r["req"]),
            energy_cost: 0,
            mana_cost: 0,
            min_base: 0,
            max_base: 0,
            diff: 0,
            type_of_resists: 0,
            targets: 0,
            dur: 0,
            att_type: 0,
            magery: i(&r["magery"]),
            magery_lvl: i(&r["mlvl"]),
            cap: 0,
            max_inc_lvls: 0,
            max_inc: 0,
            min_inc_lvls: 0,
            min_inc: 0,
            dur_inc_lvls: 0,
            dur_inc: 0,
            abilities: [AbilitySlot::default(); 10],
            learnable: i(&r["learnable"]) != 0,
            learned_from: opt_str(&r["lf"]),
            casted_by: opt_str(&r["cb"]),
            classes: opt_str(&r["cls"]),
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
        spells,
        rooms,
        shops,
        classes,
        races,
        lairs: vec![],
        tbinfo: vec![],
    })
    .unwrap()
}

fn slots<const N: usize>(flat: &Value) -> [AbilitySlot; N] {
    let mut out = [AbilitySlot::default(); N];
    for (k, p) in flat.as_array().unwrap().chunks(2).enumerate().take(N) {
        out[k] = AbilitySlot {
            ability: i(&p[0]),
            value: i(&p[1]),
        };
    }
    out
}

fn item(r: &Value) -> Item {
    Item {
        number: i(&r[0]),
        name: r[1].as_str().unwrap().to_string(),
        limit: 0,
        encum: i(&r[2]),
        item_type: i(&r[11]),
        use_count: 0,
        price: 0,
        currency: 0,
        min: i(&r[6]),
        max: i(&r[7]),
        armour_class: 0,
        damage_resist: 0,
        weapon_type: i(&r[8]),
        armour_type: 0,
        worn: i(&r[10]),
        accy: i(&r[3]),
        gettable: true,
        str_req: i(&r[4]),
        speed: i(&r[5]),
        not_droppable: false,
        destroy_on_death: false,
        retain_after_uses: false,
        class_rest: [0; 10],
        race_rest: [0; 10],
        negate_spell: [0; 10],
        abilities: slots(&r[9]),
        in_game: i(&r[12]) != 0,
        obtained_from: None,
        references: None,
    }
}

fn class(r: &Value) -> Class {
    Class {
        number: i(&r[0]),
        name: r[1].as_str().unwrap().to_string(),
        min_hits: 0,
        max_hits: 0,
        exp_table: 0,
        magery_type: i(&r[3]),
        magery_lvl: i(&r[4]),
        weapon_type: 0,
        armour_type: 0,
        combat_lvl: i(&r[2]),
        abilities: slots(&r[5]),
    }
}

fn race(r: &Value) -> Race {
    Race {
        number: i(&r[0]),
        name: r[1].as_str().unwrap().to_string(),
        m_int: 0,
        m_wil: 0,
        m_str: 0,
        m_hea: 0,
        m_agl: 0,
        m_chm: 0,
        x_int: 0,
        x_wil: 0,
        x_str: 0,
        x_hea: 0,
        x_agl: 0,
        x_chm: 0,
        hp_per_lvl: 0,
        exp_table: 0,
        base_cp: 0,
        abilities: slots(&r[2]),
    }
}

/// Spell rows are written in the order of the data's `spell_keys`.
fn spell(r: &Value, keys: &[&str]) -> Spell {
    let at = |k: &str| &r[keys.iter().position(|x| *x == k).unwrap()];
    let text = |k: &str| at(k).as_str().unwrap().to_string();
    Spell {
        number: i(at("Number")),
        name: Some(text("Name")),
        short: Some(text("Short")),
        req_level: i(at("ReqLevel")),
        energy_cost: i(at("EnergyCost")),
        mana_cost: i(at("ManaCost")),
        min_base: i(at("MinBase")),
        max_base: i(at("MaxBase")),
        diff: i(at("Diff")),
        type_of_resists: i(at("TypeOfResists")),
        targets: i(at("Targets")),
        dur: i(at("Dur")),
        att_type: i(at("AttType")),
        magery: i(at("Magery")),
        magery_lvl: i(at("MageryLVL")),
        cap: i(at("Cap")),
        max_inc_lvls: i(at("MaxIncLVLs")),
        max_inc: i(at("MaxInc")),
        min_inc_lvls: i(at("MinIncLVLs")),
        min_inc: i(at("MinInc")),
        dur_inc_lvls: i(at("DurIncLVLs")),
        dur_inc: i(at("DurInc")),
        abilities: slots(&r[keys.len()]),
        learnable: i(at("Learnable")) != 0,
        learned_from: Some(text("Learned From")),
        casted_by: Some(text("Casted By")),
        classes: Some(text("Classes")),
    }
}

pub(crate) fn fixture(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../fixtures/rules/{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn party_world(data: &Value, nmr: &str) -> World {
    let keys: Vec<&str> = data["spell_keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k.as_str().unwrap())
        .collect();
    World::new(WorldParts {
        info: Info {
            nmr_version: nmr.to_owned(),
            dat_file_version: String::new(),
            date: String::new(),
            time: String::new(),
            custom: String::new(),
            legit: false,
            disable_kai: false,
            update_url: String::new(),
        },
        items: data["items"].as_array().unwrap().iter().map(item).collect(),
        monsters: vec![],
        spells: data["spells"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| spell(r, &keys))
            .collect(),
        rooms: vec![],
        shops: vec![],
        classes: data["classes"]
            .as_array()
            .unwrap()
            .iter()
            .map(class)
            .collect(),
        races: data["races"].as_array().unwrap().iter().map(race).collect(),
        lairs: vec![],
        tbinfo: vec![],
    })
    .unwrap()
}

pub(crate) fn app_for(world: World) -> (TempDir, axum::Router) {
    let dir = TempDir::new().unwrap();
    let info = SourceInfo {
        kind: "mdb",
        path: "test".into(),
    };
    let loaded = Loaded::build(world, info, None, 1).unwrap();
    let state = AppState::with_data_dir(loaded, dir.path().join("data"));
    (dir, app(state))
}

pub(crate) async fn post(app: &axum::Router, uri: &str, body: &Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
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

fn case<'a>(fx: &'a Value, id: &str) -> &'a Value {
    fx["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no case {id}"))
}

/// The wire character for a paste_char case: only what the endpoint reads (equipment, text).
fn char_request(fx: &Value, id: &str) -> (Value, Vec<i64>) {
    let c = case(fx, id);
    let clip = c["inputs"]["forms"]["frmMain"]["txtClip"]["Text"]
        .as_str()
        .unwrap()
        .to_owned();
    let eq = match &c["inputs"]["globals"]["nEquippedItem"] {
        Value::Null => json!([]),
        v => v.clone(),
    };
    let want = c["outputs"]["eq"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    (
        json!({
            "character": { "equipped": eq, "level": 1, "stats": [0,0,0,0,0,0] },
            "text": clip,
            "char_loaded": false,
            // the oracle's defaults for these two globals
            "only_in_game": c["inputs"]["globals"]["bOnlyInGame"].as_bool().unwrap_or(false),
            "second_wrist": c["inputs"]["globals"]["bInvenUse2ndWrist"].as_bool().unwrap_or(false),
        }),
        want,
    )
}

/// The fixture case's Item Manager and equip holds, in the request's shape.
fn add_form_state(req: &mut Value, fx: &Value, id: &str) {
    let f = &case(fx, id)["inputs"]["forms"]["frmMain"];
    if let Some(rows) = f["lvItemManager"]["$listview"].as_array() {
        req["rows"] = Value::Array(
            rows.iter()
                .map(|r| {
                    json!({
                        "text": r["Text"],
                        "subs": r["Sub"].as_array().unwrap().iter()
                            .map(|t| json!({"text": t})).collect::<Vec<_>>(),
                    })
                })
                .collect(),
        );
    }
    let mut hold = vec![false; 20];
    if let Some(h) = f["chkEquipHold"]["$array"].as_object() {
        for (k, v) in h {
            hold[k.parse::<usize>().unwrap()] = v["Value"].as_i64() == Some(1);
        }
    }
    req["hold"] = json!(hold);
    req["unequip_missing"] = json!(f["chkUnequipMissing"]["Value"].as_i64() == Some(1));
}

fn class_race_numbers(fx: &Value, id: &str) -> (i64, i64) {
    let c = case(fx, id);
    let o = &c["outputs"];
    let nth = |list: &str, ix: &Value| {
        let k = ix.as_i64().unwrap() as usize;
        if k == 0 {
            0
        } else {
            fx["header"]["data"][list][k - 1]["n"].as_i64().unwrap()
        }
    };
    (nth("classes", &o["cls"]), nth("races", &o["race"]))
}

#[tokio::test]
async fn paste_character_matches_fixture_cases() {
    let fx = fixture("paste_char");
    let (_d, app) = app_for(char_world(&fx["header"]["data"]));
    // fixtures/rules/paste_char.json: no boxes, no loaded character, the Item Manager empty.
    for id in ["f282", "f284", "f286", "f289", "f291", "f294"] {
        let (mut req, want_eq) = char_request(&fx, id);
        add_form_state(&mut req, &fx, id);
        let (status, got) = post(&app, "/api/paste/character", &req).await;
        assert_eq!(status, StatusCode::OK, "{id}: {got}");
        assert!(got["needs_answer"].is_null(), "{id}");
        let eq: Vec<i64> = got["character"]["equipped"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap())
            .collect();
        assert_eq!(eq, want_eq, "{id} equipped");
        let o = &case(&fx, id)["outputs"];
        assert_eq!(
            got["character"]["level"].to_string(),
            o["lvl"].as_str().unwrap(),
            "{id} level"
        );
        let (cls, race) = class_race_numbers(&fx, id);
        assert_eq!(
            got["character"]["class"].as_i64().unwrap(),
            cls,
            "{id} class"
        );
        assert_eq!(
            got["character"]["race"].as_i64().unwrap(),
            race,
            "{id} race"
        );
    }
}

#[tokio::test]
async fn character_reports_a_message_box_and_a_missing_item() {
    let fx = fixture("paste_char");
    let (_d, app) = app_for(char_world(&fx["header"]["data"]));
    // f292 (paste_char.json): two rows flagged CARRIED are not in the pasted inventory, so MME
    // shows one "Discrepancy" box
    let (mut req, _) = char_request(&fx, "f292");
    add_form_state(&mut req, &fx, "f292");
    let (status, got) = post(&app, "/api/paste/character", &req).await;
    assert_eq!(status, StatusCode::OK, "{got}");
    let want = case(&fx, "f292")["events"][0]["text"].as_str().unwrap();
    let prompts = got["prompts"].as_array().unwrap();
    assert_eq!(prompts.len(), 1, "{got}");
    assert_eq!(prompts[0]["kind"], "info");
    assert_eq!(prompts[0]["text"], want);
    // the same box is not listed twice
    assert_eq!(got["events"], json!([]));
}

#[tokio::test]
async fn short_paste_does_nothing() {
    let fx = fixture("paste_char");
    let (_d, app) = app_for(char_world(&fx["header"]["data"]));
    let req = json!({ "character": { "level": 7 }, "text": "stat" });
    let (status, got) = post(&app, "/api/paste/character", &req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(got["character"]["level"], 7);
    assert_eq!(got["prompts"], json!([]));
}

#[tokio::test]
async fn different_class_asks_then_resets_the_tab() {
    let fx = fixture("paste_char");
    let (_d, app) = app_for(char_world(&fx["header"]["data"]));
    // f283 (paste_char.json): a loaded Bard (class index 9 after the paste) with a save and a box
    let c = case(&fx, "f283");
    let (mut req, _) = char_request(&fx, "f283");
    // pretend the stored character is a Warrior (class 1) so the class differs
    req["character"]["class"] = json!(1);
    req["character"]["race"] = json!(1);
    req["character"]["name"] = json!("Old");
    req["char_loaded"] = json!(true);
    req["character"]["learned_spells"] = json!([1, 2]);
    req["character"]["quests"] = json!([true]);
    let _ = c;
    let (status, first) = post(&app, "/api/paste/character", &req).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let q = &first["needs_answer"];
    assert!(
        q["text"].as_str().unwrap().contains("different class"),
        "{first}"
    );
    assert_eq!(q["kind"], "yes_no_cancel");
    // Cancel leaves the character alone
    req["answers"] = json!([2]);
    let (_, cancelled) = post(&app, "/api/paste/character", &req).await;
    assert!(cancelled["needs_answer"].is_null());
    assert_eq!(cancelled["character"]["class"], 1);
    // Yes resets the tab: the old quests and spells are gone and the events say so
    req["answers"] = json!([6]);
    let (_, yes) = post(&app, "/api/paste/character", &req).await;
    assert!(yes["needs_answer"].is_null(), "{yes}");
    assert!(
        yes["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "clear_character"),
        "{yes}"
    );
    assert_eq!(yes["character"]["quests"][0], false);
    assert_eq!(yes["character"]["learned_spells"][0], 0);
    assert_eq!(yes["character"]["level"], 56);
}

#[tokio::test]
async fn bad_requests_are_rejected() {
    let fx = fixture("paste_char");
    let (_d, app) = app_for(char_world(&fx["header"]["data"]));
    let (status, _) = post(&app, "/api/paste/character", &json!({ "text": 5 })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, body) = post(
        &app,
        "/api/paste/character",
        &json!({ "character": { "alignment": 9 }, "text": "x" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

fn party_request(fx: &Value, id: &str) -> Value {
    let t = case(fx, id)["inputs"]["forms"]["frmPasteChar"]["txtText"]["Text"]
        .as_str()
        .unwrap()
        .to_owned();
    json!({ "text": t })
}

#[tokio::test]
async fn party_no_data_asks_then_follows_the_answer() {
    let fx = fixture("paste_party");
    let (_d, app) = app_for(party_world(&fx["header"]["data"], "v1.83"));
    // fixtures/rules/paste_party.json: nomatch-total0 asks "Continue to party screen anyway?"
    let req = party_request(&fx, "nomatch-total0");
    let (status, first) = post(&app, "/api/paste/party", &req).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["needs_answer"]["kind"], "no_data");
    let mut yes = req.clone();
    yes["no_data_continue"] = json!(true);
    let (_, done) = post(&app, "/api/paste/party", &yes).await;
    assert!(done["needs_answer"].is_null());
    assert_eq!(done["status"], "done");
    let mut no = req.clone();
    no["no_data_continue"] = json!(false);
    let (_, done) = post(&app, "/api/paste/party", &no).await;
    assert_eq!(done["status"], "no_data_exit");
}

#[tokio::test]
async fn party_member_rows_and_attack_prompt() {
    let fx = fixture("paste_party");
    // fixtures/rules/paste_party.json: one-01 (nNMRVer 1.83, default attack accepted) gives AC 78
    let (_d, app) = app_for(party_world(&fx["header"]["data"], "v1.83"));
    let mut req = party_request(&fx, "one-01");
    let (_, first) = post(&app, "/api/paste/party", &req).await;
    assert_eq!(first["needs_answer"]["kind"], "attack", "{first}");
    assert_eq!(first["needs_answer"]["index"], 0);
    req["attack_answers"] = json!([null]);
    let (_, done) = post(&app, "/api/paste/party", &req).await;
    assert!(done["needs_answer"].is_null(), "{done}");
    let o = &case(&fx, "one-01")["outputs"];
    assert_eq!(done["form"]["ac"][1], o["txtPastePartyAC"][1]);
    assert_eq!(done["form"]["ac"][0], o["txtPastePartyAC"][0]);
    assert_eq!(done["form"]["party_total"], o["partyTotal"]);
}

#[tokio::test]
async fn party_edits_recount_the_averages() {
    let fx = fixture("paste_party");
    let (_d, app) = app_for(party_world(&fx["header"]["data"], "v1.83"));
    let form = json!({
        "ac": ["", "40", "60", "", "", "", ""],
        "name": ["", "A", "B", "", "", "", ""],
    });
    let (status, got) = post(&app, "/api/paste/party", &json!({ "form": form })).await;
    assert_eq!(status, StatusCode::OK, "{got}");
    assert_eq!(got["form"]["ac"][0], "50");
    assert_eq!(got["form"]["party_total"], "2");
}

#[tokio::test]
async fn party_continue_builds_the_filter() {
    let fx = fixture("paste_party");
    let (_d, app) = app_for(party_world(&fx["header"]["data"], "v1.83"));
    let one = json!({ "form": { "party_total": "1" } });
    let (_, got) = post(&app, "/api/paste/party/continue", &one).await;
    assert_eq!(got["status"], "info");
    // cmdContinue_Click: regen 12 -> Round(12/6)=2, heals 5 -> 7 (all given, so no question)
    let form = json!({
        "party_total": "3",
        "ac": ["50", "", "", "", "", "", ""],
        "regen_hp": ["12", "", "", "", "", "", ""],
        "heals": ["5", "", "", "", "", "", ""],
        "dmg": ["80", "", "", "", "", "", ""],
    });
    let req = json!({ "form": form, "current_damage": 99999 });
    let (_, got) = post(&app, "/api/paste/party/continue", &req).await;
    assert_eq!(got["status"], "applied", "{got}");
    assert_eq!(got["filter"]["damage"], "7");
    assert_eq!(got["filter"]["ac"], "50");
    assert_eq!(got["filter"]["party_size"], "3");
    assert_eq!(got["filter"]["damage_out_phys"], "80");
    // only one of regen/heals given and a different value on the filter: the question comes first
    let form = json!({
        "party_total": "2",
        "regen_hp": ["12", "", "", "", "", "", ""],
    });
    let req = json!({ "form": form, "current_damage": 10 });
    let (_, ask) = post(&app, "/api/paste/party/continue", &req).await;
    assert_eq!(ask["status"], "needs_answer");
    assert!(
        ask["question"]["text"]
            .as_str()
            .unwrap()
            .contains("[HEALS]")
    );
    let mut cancel = req.clone();
    cancel["answer"] = json!(2);
    let (_, got) = post(&app, "/api/paste/party/continue", &cancel).await;
    assert_eq!(got["status"], "canceled");
    let mut no = req.clone();
    no["answer"] = json!(7);
    let (_, got) = post(&app, "/api/paste/party/continue", &no).await;
    assert!(got["filter"]["damage"].is_null());
}

#[test]
fn value_sort_is_descending_and_stable() {
    use touchstone::paste::sort_by_value_desc;
    use touchstone_rules::game_text::{ListRow, SubItem, SubTag};
    let row = |n: &str, v: &str| {
        let mut subs = vec![SubItem::new(""); 8];
        subs.push(SubItem {
            text: String::new(),
            tag: Some(SubTag::Text(v.to_owned())),
        });
        ListRow {
            text: n.to_owned(),
            subs,
        }
    };
    let mut rows = vec![row("1", "5"), row("2", "100"), row("3", "5"), row("4", "0")];
    sort_by_value_desc(&mut rows);
    let order: Vec<&str> = rows.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(order, ["2", "1", "3", "4"]);
}
