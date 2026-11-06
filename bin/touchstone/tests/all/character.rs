//! `/api/character/*` and `/api/characters`: evaluate against cases of fixtures/rules/pipeline.json
//! (rebuilt into a World from the fixture header), bless-spell filter, and the character store.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use touchstone::state::{Loaded, SourceInfo};
use touchstone::{AppState, app};
use touchstone_model::{AbilitySlot, Class, Info, Item, Race, Spell, World, WorldParts};
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

/// The rows the cases use, from the fixture header's `data` (field lists, see the generator).
fn world(data: &Value) -> World {
    let rows = |k: &str| data[k].as_array().unwrap().clone();
    let items = rows("items")
        .iter()
        .map(|r| {
            let mut class_rest = [0; 10];
            for (k, c) in r[11].as_array().unwrap().iter().enumerate() {
                class_rest[k] = i(c);
            }
            Item {
                number: i(&r[0]),
                name: String::new(),
                limit: 0,
                encum: i(&r[1]),
                item_type: i(&r[2]),
                use_count: 0,
                price: 0,
                currency: 0,
                min: 0,
                max: 0,
                armour_class: i(&r[3]),
                damage_resist: i(&r[4]),
                weapon_type: i(&r[5]),
                armour_type: i(&r[6]),
                worn: i(&r[7]),
                accy: i(&r[8]),
                gettable: true,
                str_req: i(&r[9]),
                speed: i(&r[10]),
                not_droppable: false,
                destroy_on_death: false,
                retain_after_uses: false,
                class_rest,
                race_rest: [0; 10],
                negate_spell: [0; 10],
                abilities: pairs(&r[12]),
                in_game: true,
                obtained_from: None,
                references: None,
            }
        })
        .collect();
    let spells = rows("spells")
        .iter()
        .map(|r| Spell {
            number: i(&r[0]),
            name: None,
            short: Some(format!("sp{}", i(&r[0]))),
            req_level: i(&r[1]),
            energy_cost: i(&r[2]),
            mana_cost: i(&r[3]),
            min_base: i(&r[4]),
            max_base: i(&r[5]),
            diff: i(&r[6]),
            type_of_resists: i(&r[7]),
            targets: i(&r[8]),
            dur: i(&r[9]),
            att_type: i(&r[10]),
            magery: i(&r[11]),
            magery_lvl: i(&r[12]),
            cap: i(&r[13]),
            max_inc_lvls: i(&r[14]),
            max_inc: i(&r[15]),
            min_inc_lvls: i(&r[16]),
            min_inc: i(&r[17]),
            dur_inc_lvls: i(&r[18]),
            dur_inc: i(&r[19]),
            abilities: pairs(&r[20]),
            learnable: false,
            learned_from: None,
            casted_by: None,
            classes: None,
        })
        .collect();
    let classes = rows("classes")
        .iter()
        .map(|r| Class {
            number: i(&r[0]),
            name: String::new(),
            min_hits: i(&r[1]),
            max_hits: i(&r[2]),
            exp_table: i(&r[3]),
            magery_type: i(&r[4]),
            magery_lvl: i(&r[5]),
            weapon_type: i(&r[6]),
            armour_type: i(&r[7]),
            combat_lvl: i(&r[8]),
            abilities: pairs(&r[9]),
        })
        .collect();
    let races = rows("races")
        .iter()
        .map(|r| Race {
            number: i(&r[0]),
            name: String::new(),
            m_str: i(&r[1]),
            m_int: i(&r[2]),
            m_wil: i(&r[3]),
            m_agl: i(&r[4]),
            m_hea: i(&r[5]),
            m_chm: i(&r[6]),
            x_int: 0,
            x_wil: 0,
            x_str: 0,
            x_hea: 0,
            x_agl: 0,
            x_chm: 0,
            hp_per_lvl: i(&r[7]),
            exp_table: i(&r[8]),
            base_cp: i(&r[9]),
            abilities: pairs(&r[10]),
        })
        .collect();
    World::new(WorldParts {
        info: Info {
            nmr_version: "v1.8.3".to_owned(),
            dat_file_version: "v1.11p".to_owned(),
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
        races,
        lairs: vec![],
        tbinfo: vec![],
    })
    .unwrap()
}

fn fixture() -> Value {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rules/pipeline.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

pub(crate) struct Env {
    pub dir: TempDir,
    pub app: axum::Router,
    pub fx: Value,
}

pub(crate) fn env() -> Env {
    let fx = fixture();
    let dir = TempDir::new().unwrap();
    let info = SourceInfo {
        kind: "mdb",
        path: "test".into(),
    };
    let loaded = Loaded::build(world(&fx["header"]["data"]), info, None, 1).unwrap();
    let state = AppState::with_data_dir(loaded, dir.path().join("data"));
    Env {
        dir,
        app: app(state),
        fx,
    }
}

pub(crate) async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<&Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    let b = match body {
        Some(v) => {
            req = req.header("content-type", "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    let res = app.clone().oneshot(req.body(b).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, v)
}

/// The request for a fixture case: its frmMain controls and globals as the wire character.
pub(crate) fn request(fx: &Value, id: &str) -> (Value, Value) {
    let case = fx["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap();
    let setup = &fx["header"]["setup"]["forms"]["frmMain"];
    let forms = &case["inputs"]["forms"]["frmMain"];
    let g = &case["inputs"]["globals"];
    let ctl = |name: &str| {
        if forms[name].is_null() {
            &setup[name]
        } else {
            &forms[name]
        }
    };
    let elem = |name: &str, k: usize| ctl(name)["$array"][k.to_string()].clone();
    let combo = |name: &str| {
        let e = elem(name, 0);
        if e["ListIndex"].as_i64().unwrap_or(-1) > 0 {
            e["ItemData"]["1"].as_i64().unwrap()
        } else {
            0
        }
    };
    let text = |name: &str, k: usize| {
        elem(name, k)["Text"]
            .as_str()
            .unwrap_or("0")
            .parse::<i64>()
            .unwrap_or(0)
    };
    let gi = |n: &str| g[n].as_i64().unwrap_or(0);
    let gb = |n: &str| g[n].as_bool().unwrap_or(false);
    let req = json!({
        "race": combo("cmbGlobalRace"),
        "class": combo("cmbGlobalClass"),
        "level": text("txtGlobalLevel", 0),
        "alignment": ctl("cmbGlobalAlignment")["ListIndex"].as_i64().unwrap_or(0),
        "global_filter": ctl("chkGlobalFilter")["Value"].as_i64() == Some(1),
        "stats": (0..6).map(|k| text("txtCharStats", k)).collect::<Vec<_>>(),
        "stat_adjustments": g["char_StatAdjustments"].as_array().cloned().unwrap_or_default(),
        "equipped": g["nEquippedItem"],
        "bless": (0..10).map(|k| elem("cmbCharBless", k)["ItemData"][0].as_i64().unwrap_or(0)).collect::<Vec<_>>(),
        "quests": (0..12).map(|k| elem("chkCharQuests", k)["Value"].as_i64() == Some(1)).collect::<Vec<_>>(),
        "quest_options": (0..4).map(|k| elem("cmbCharQuestOpts", k)["ListIndex"].as_i64().unwrap_or(-1)).collect::<Vec<_>>(),
        "use_add_weight": ctl("chkInvenAddWeight")["Value"].as_i64() == Some(1),
        "add_weight": ctl("txtInvenAddWeight")["Text"].as_str().unwrap_or("0").parse::<i64>().unwrap_or(0),
        "attack": {
            "attack_type": gi("nGlobalAttackTypeMME"), "backstab": gb("bGlobalAttackBackstab"),
            "backstab_weapon": gi("nGlobalAttackBackstabWeapon"), "martial_art": gi("nGlobalAttackMA"),
            "spell_num": gi("nGlobalAttackSpellNum"), "spell_lvl": gi("nGlobalAttackSpellLVL"),
            "manual_p": gi("nGlobalAttackManualP"), "manual_m": gi("nGlobalAttackManualM"),
            "use_meditate": gb("bGlobalAttackUseMeditate"), "heal_type": gi("nGlobalAttackHealType"),
            "heal_spell_num": gi("nGlobalAttackHealSpellNum"), "heal_spell_lvl": gi("nGlobalAttackHealSpellLVL"),
            "heal_rounds": gi("nGlobalAttackHealRounds"), "heal_manual": gi("nGlobalAttackHealManual"),
        },
        "options": { "hide_class_race": ctl("chkInvenHideCharStats")["Value"].as_i64() == Some(1) },
    });
    (req, case["outputs"].clone())
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap()
}

async fn pinned(id: &str) {
    let e = env();
    let (req, o) = request(&e.fx, id);
    let (status, got) = call(&e.app, "POST", "/api/character/evaluate", Some(&req)).await;
    assert_eq!(status, StatusCode::OK, "{id}: {got}");
    // fixtures/rules/pipeline.json: lblInvenCharStat captions
    for k in 0..47 {
        assert_eq!(got["stats"][k], o["stat"][k], "{id} stat {k}");
    }
    // fixtures/rules/pipeline.json: txtCharStats(i).Tag, CP box and panel captions
    for k in 0..6 {
        assert_eq!(
            got["stat_values"][k]
                .as_f64()
                .unwrap()
                .to_string()
                .trim_end_matches(".0"),
            s(&o["ctags"][k]),
            "{id} ctag {k}"
        );
    }
    assert_eq!(
        got["cps"]["avail"]
            .as_i64()
            .map(|n| n.to_string())
            .unwrap_or_default(),
        s(&o["cps_tag"]),
        "{id} cps"
    );
    assert_eq!(
        got["hp"]["caption"].as_str().unwrap_or(""),
        s(&o["hp_cap"]),
        "{id} hp"
    );
    assert_eq!(
        got["hp"]["rest_caption"].as_str().unwrap_or(""),
        s(&o["rest_cap"]),
        "{id} rest"
    );
    assert_eq!(
        got["mana"]["max_caption"].as_str().unwrap_or(""),
        s(&o["maxmana_cap"]),
        "{id} mana"
    );
    assert_eq!(
        got["mana"]["regen_caption"].as_str().unwrap_or(""),
        s(&o["mana_rate_cap"]),
        "{id} regen"
    );
    assert_eq!(
        got["mana"]["spellcasting_caption"].as_str().unwrap_or(""),
        s(&o["sc_cap"]),
        "{id} sc"
    );
    assert_eq!(
        got["picklocks_caption"].as_str().unwrap_or(""),
        s(&o["pick_cap"]),
        "{id} picks"
    );
    assert_eq!(
        got["magic_res"]
            .as_i64()
            .map(|n| n.to_string())
            .unwrap_or_default(),
        s(&o["mr"]),
        "{id} mr"
    );
    assert_eq!(
        got["encum"]["caption"].as_str().unwrap_or(""),
        s(&o["encum_cap"]),
        "{id} encum"
    );
    assert_eq!(
        got["dodge"].as_f64().unwrap().to_string(),
        s(&o["dodge_tag"]),
        "{id} dodge"
    );
    assert_eq!(
        got["bless_mana"].as_str().unwrap_or(""),
        s(&o["bless_cap"]),
        "{id} bless"
    );
}

#[tokio::test]
async fn evaluate_level_50_grid_case() {
    pinned("grid-c1-r11-l50").await;
}

#[tokio::test]
async fn evaluate_bless_case() {
    pinned("bless-07").await;
}

#[tokio::test]
async fn evaluate_all_quests_case() {
    pinned("quests-all").await;
}

#[tokio::test]
async fn evaluate_antimagic_and_hidden_class_race_cases() {
    pinned("antimagic-2").await;
    pinned("hide-3").await;
}

#[tokio::test]
async fn evaluate_needs_class_and_race() {
    let e = env();
    let (status, body) = call(
        &e.app,
        "POST",
        "/api/character/evaluate",
        Some(&json!({"level": 5})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body.as_str().unwrap().contains("pick a class and race"));
    let (status, _) = call(
        &e.app,
        "POST",
        "/api/character/evaluate",
        Some(&json!({"race": 1, "class": 1, "level": 5000})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &e.app,
        "POST",
        "/api/character/evaluate",
        Some(&json!({"race": 1, "class": 1, "bless": [0,0,0,0,0,0,0,0,0,0,0]})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn evaluate_survives_extremes() {
    let e = env();
    for (level, stat) in [(0, 0), (999, 999), (1, 999), (999, 0)] {
        let body = json!({"race": 1, "class": 1, "level": level, "stats": vec![stat; 6]});
        let (status, got) = call(&e.app, "POST", "/api/character/evaluate", Some(&body)).await;
        assert_eq!(status, StatusCode::OK, "{level}/{stat}: {got}");
    }
}

#[tokio::test]
async fn bless_spells_follow_the_filter() {
    // FilterBlessSpells: timed spells with self/party targets that are learnable; the spells in
    // the fixture world have no names, so the label is " (n)".
    let e = env();
    let (status, all) = call(&e.app, "GET", "/api/character/bless-spells?all=true", None).await;
    assert_eq!(status, StatusCode::OK);
    let all = all.as_array().unwrap();
    assert!(!all.is_empty());
    for c in all {
        assert_eq!(s(&c["label"]), format!(" ({})", c["number"]));
    }
    let (_, mine) = call(
        &e.app,
        "GET",
        "/api/character/bless-spells?class=1&level=50",
        None,
    )
    .await;
    assert!(mine.as_array().unwrap().len() <= all.len());
    let first = all[0]["number"].as_i64().unwrap();
    let (_, inc) = call(
        &e.app,
        "GET",
        &format!("/api/character/bless-spells?class=1&level=1&include={first}"),
        None,
    )
    .await;
    assert!(inc.as_array().unwrap().iter().any(|c| c["number"] == first));
    let (status, _) = call(&e.app, "GET", "/api/character/bless-spells?include=x", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn character_store_round_trip() {
    let e = env();
    let (status, list) = call(&e.app, "GET", "/api/characters", None).await;
    assert_eq!((status, list), (StatusCode::OK, json!([])));
    let body = json!({"race": 1, "class": 1, "level": 50, "stats": [60, 50, 40, 30, 20, 10], "bless": [5]});
    let (status, saved) = call(
        &e.app,
        "PUT",
        "/api/characters/Zed%20the%20Bold",
        Some(&body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["name"], "Zed the Bold");
    assert_eq!(saved["bless"].as_array().unwrap().len(), 10);
    call(&e.app, "PUT", "/api/characters/alpha", Some(&body)).await;
    let (_, got) = call(&e.app, "GET", "/api/characters/Zed%20the%20Bold", None).await;
    assert_eq!(got, saved);
    let (_, list) = call(&e.app, "GET", "/api/characters", None).await;
    assert_eq!(list[0]["name"], "alpha");
    assert_eq!(list[1]["name"], "Zed the Bold");
    assert_eq!(list[1]["level"], 50);
    // atomic write leaves no temp file behind
    let files: Vec<_> = std::fs::read_dir(e.dir.path().join("data/characters"))
        .unwrap()
        .map(|f| f.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(files.len(), 2, "{files:?}");
    let (status, _) = call(&e.app, "DELETE", "/api/characters/alpha", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&e.app, "DELETE", "/api/characters/alpha", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&e.app, "GET", "/api/characters/alpha", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn bad_names_and_bodies_are_rejected() {
    let e = env();
    let body = json!({"race": 1, "class": 1});
    for name in [
        "..",
        ".x",
        "a%2Fb",
        "a%5Cb",
        "a%3Ab",
        "a%00b",
        "x%20",
        &"y".repeat(65),
    ] {
        let (status, _) = call(
            &e.app,
            "PUT",
            &format!("/api/characters/{name}"),
            Some(&body),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{name}");
        let (status, _) = call(&e.app, "GET", &format!("/api/characters/{name}"), None).await;
        assert!(
            status == StatusCode::BAD_REQUEST || status == StatusCode::NOT_FOUND,
            "{name} {status}"
        );
    }
    let (status, _) = call(
        &e.app,
        "PUT",
        "/api/characters/ok",
        Some(&json!({"level": 5000})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(!e.dir.path().join("data/characters/ok.json").exists());
    assert!(!e.dir.path().join("data/characters/..json").exists());
}

async fn raw(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Vec<u8>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::from(body))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, headers, bytes.to_vec())
}

fn sample(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/data/mmec")
            .join(name),
    )
    .unwrap()
}

#[tokio::test]
async fn mmec_import_applies_load_rules_and_exports() {
    let e = env();
    // Head=9 is a torso item, so MME places it in the torso slot; 99 is no class in this database;
    // the weapon 48 is real, 9999 is not an item
    let file = "[PlayerInfo]\r\nName=Imp\r\nClass=1\r\nRace=2\r\nLevel=12\r\nStrength=70\r\n\
        CurrentAttackType=1\r\nWeaponVSCharAC=77\r\n[Inventory]\r\nHead=9\r\nWrist2=426\r\n\
        Finger1=433\r\nFinger2=434\r\nWeapon=48\r\nEyes=9999\r\nIM_CARRIED=48|2,9999|1,\r\n\
        [MonsterFilter]\r\nHP=123\r\n"
        .as_bytes()
        .to_vec();
    let (status, _, body) = raw(&e.app, "POST", "/api/characters/import", file.clone()).await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let dto: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(dto["name"], "Imp");
    assert_eq!(
        (dto["class"].clone(), dto["race"].clone()),
        (json!(1), json!(2))
    );
    let eq = dto["equipped"].as_array().unwrap();
    assert_eq!(
        (eq[4].clone(), eq[0].clone(), eq[6].clone()),
        (json!(9), json!(0), json!(426))
    );
    assert_eq!(
        (
            eq[9].clone(),
            eq[10].clone(),
            eq[16].clone(),
            eq[17].clone()
        ),
        (json!(433), json!(434), json!(48), json!(0))
    );
    assert_eq!(dto["attack"]["attack_type"], 1);
    assert_eq!(dto["carried"], json!([{"item": 48, "qty": 2}]));
    // same name again is refused until overwrite is asked for
    let (status, _, _) = raw(&e.app, "POST", "/api/characters/import", file.clone()).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _, _) = raw(
        &e.app,
        "POST",
        "/api/characters/import?overwrite=true",
        file,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, headers, bytes) =
        raw(&e.app, "GET", "/api/characters/Imp/export.mmec", vec![]).await;
    assert_eq!(status, StatusCode::OK);
    let disp = headers["content-disposition"].to_str().unwrap();
    assert!(
        disp.starts_with("attachment; filename=\"Imp.mmec\""),
        "{disp}"
    );
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.starts_with("[PlayerInfo]\r\nName=Imp\r\n"));
    assert!(text.contains("\r\nWeaponVSCharAC=77\r\n"));
    assert!(text.contains("\r\nTorso=9\r\n") && text.contains("\r\nWrist=426\r\n"));
    assert!(text.contains("\r\nIM_CARRIED=48|2,\r\n"));
    assert!(text.contains("[MonsterFilter]\r\nHP=123\r\n"));
    let (status, _, _) = raw(&e.app, "GET", "/api/characters/nobody/export.mmec", vec![]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn mmec_import_names_and_bad_files() {
    let e = env();
    let f = sample("empty-name.mmec");
    // no Name in the file: the uploaded file's name is used
    let (status, _, body) = raw(
        &e.app,
        "POST",
        "/api/characters/import?file=C%3A%5Cchars%5CGrim.MMEC",
        f.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["name"],
        "Grim"
    );
    // nothing to name it by
    let (status, _, _) = raw(&e.app, "POST", "/api/characters/import", f.clone()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    // an explicit bad name
    let (status, _, _) = raw(&e.app, "POST", "/api/characters/import?name=..%2Fx", f).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    // not a character file
    let (status, _, _) = raw(
        &e.app,
        "POST",
        "/api/characters/import?name=x",
        b"junk".to_vec(),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    // the non-ASCII sample keeps its cp1252 name
    let (status, _, body) = raw(
        &e.app,
        "POST",
        "/api/characters/import",
        sample("thief-backstab.mmec"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["name"],
        "Kit \u{e9}"
    );
    let (_, headers, _) = raw(
        &e.app,
        "GET",
        "/api/characters/Kit%20%C3%A9/export.mmec",
        vec![],
    )
    .await;
    let disp = headers["content-disposition"].to_str().unwrap();
    assert!(
        disp.contains("filename=\"Kit _.mmec\"") && disp.contains("UTF-8''Kit%20%C3%A9.mmec"),
        "{disp}"
    );
}
