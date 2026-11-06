//! `POST /api/evaluate/{tab}`: the explorer lists, pinned to the cases of fixtures/rules/rows.json
//! (the world is rebuilt from the fixture header), plus the option and error paths.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use touchstone::state::{Loaded, SourceInfo};
use touchstone::{AppState, app};
use touchstone_model::{
    AbilitySlot, Class, DropSlot, Info, Item, MidSpell, Monster, MonsterAttack, Race, Shop,
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

#[allow(clippy::too_many_arguments)]
fn item(number: i32, name: String, limit: i32, encum: i32, item_type: i32) -> Item {
    Item {
        number,
        name,
        limit,
        encum,
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

fn monster(number: i32, name: String) -> Monster {
    Monster {
        number,
        name: Some(name),
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
        avg_dmg: 0.0,
        avg_dmg_phys: 0.0,
        avg_dmg_spell: 0.0,
        greet_txt: 0,
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
    }
}

/// The world of the rows fixture header's `data` (field lists as the generator writes them).
fn world(data: &Value) -> World {
    let rows = |k: &str| data[k].as_array().unwrap().clone();
    let mut items: Vec<Item> = rows("items")
        .iter()
        .map(|r| {
            let mut it = item(i(&r[0]), s(&r[1]), i(&r[2]), i(&r[3]), i(&r[4]));
            it.min = i(&r[5]);
            it.max = i(&r[6]);
            it.armour_class = i(&r[7]);
            it.damage_resist = i(&r[8]);
            it.weapon_type = i(&r[9]);
            it.armour_type = i(&r[10]);
            it.worn = i(&r[11]);
            it.accy = i(&r[12]);
            it.str_req = i(&r[13]);
            it.speed = i(&r[14]);
            for (k, c) in r[15].as_array().unwrap().iter().enumerate() {
                it.class_rest[k] = i(c);
            }
            it.abilities = pairs(&r[16]);
            it
        })
        .collect();
    items.extend(
        rows("sundry")
            .iter()
            .map(|r| item(i(&r[0]), s(&r[1]), i(&r[2]), i(&r[3]), i(&r[4]))),
    );
    items.sort_by_key(|r| r.number);
    let spells = rows("spells")
        .iter()
        .map(|r| Spell {
            number: i(&r[0]),
            name: Some(s(&r[1])),
            short: Some(s(&r[2])),
            req_level: i(&r[3]),
            energy_cost: i(&r[4]),
            mana_cost: i(&r[5]),
            min_base: i(&r[6]),
            max_base: i(&r[7]),
            diff: i(&r[8]),
            type_of_resists: i(&r[9]),
            targets: i(&r[10]),
            dur: i(&r[11]),
            att_type: i(&r[12]),
            magery: i(&r[13]),
            magery_lvl: i(&r[14]),
            cap: i(&r[15]),
            max_inc_lvls: i(&r[16]),
            max_inc: i(&r[17]),
            min_inc_lvls: i(&r[18]),
            min_inc: i(&r[19]),
            dur_inc_lvls: i(&r[20]),
            dur_inc: i(&r[21]),
            abilities: pairs(&r[24]),
            learnable: i(&r[22]) == 1,
            learned_from: (i(&r[23]) != 0).then(|| "x".to_string()),
            casted_by: None,
            classes: None,
        })
        .collect();
    let shops = rows("shops")
        .iter()
        .map(|r| Shop {
            number: i(&r[0]),
            name: s(&r[1]),
            shop_type: i(&r[2]),
            min_lvl: 0,
            max_lvl: 0,
            markup_pct: 0,
            class_rest: 0,
            stock: [ShopSlot::default(); 20],
            in_game: true,
            assigned_to: None,
        })
        .collect();
    let classes = rows("classes")
        .iter()
        .map(|r| Class {
            number: i(&r[0]),
            name: s(&r[1]),
            min_hits: i(&r[2]),
            max_hits: i(&r[3]),
            exp_table: i(&r[4]),
            magery_type: i(&r[5]),
            magery_lvl: i(&r[6]),
            weapon_type: i(&r[7]),
            armour_type: i(&r[8]),
            combat_lvl: i(&r[9]),
            abilities: pairs(&r[10]),
        })
        .collect();
    let races = rows("races")
        .iter()
        .map(|r| Race {
            number: i(&r[0]),
            name: s(&r[1]),
            m_str: i(&r[2]),
            m_int: i(&r[3]),
            m_wil: i(&r[4]),
            m_agl: i(&r[5]),
            m_hea: i(&r[6]),
            m_chm: i(&r[7]),
            x_str: i(&r[8]),
            x_int: i(&r[9]),
            x_wil: i(&r[10]),
            x_agl: i(&r[11]),
            x_hea: i(&r[12]),
            x_chm: i(&r[13]),
            hp_per_lvl: i(&r[14]),
            exp_table: i(&r[15]),
            base_cp: i(&r[16]),
            abilities: pairs(&r[17]),
        })
        .collect();
    let monsters = rows("monster_names")
        .iter()
        .map(|r| monster(i(&r[0]), s(&r[1])))
        .collect();
    let tbinfo = rows("textblocks")
        .iter()
        .map(|p| TbInfo {
            number: i(&p[0]),
            link_to: 0,
            action: Some(s(&p[1])),
            called_from: String::new(),
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
        monsters,
        spells,
        rooms: vec![],
        shops,
        classes,
        races,
        lairs: vec![],
        tbinfo,
    })
    .unwrap()
}

fn fixture() -> Value {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rules/rows.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn router(fx: &Value) -> axum::Router {
    let info = SourceInfo {
        kind: "mdb",
        path: "test".into(),
    };
    let loaded = Loaded::build(world(&fx["header"]["data"]), info, None, 1).unwrap();
    app(AppState::with_data_dir(
        loaded,
        tempfile::TempDir::new().unwrap().keep(),
    ))
}

async fn post(app: &axum::Router, uri: &str, body: &Value) -> (StatusCode, Value) {
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

/// The loaded character of a case, as the wire character (global filter on, as the explorer sends).
fn character(fx: &Value, c: &Value) -> Value {
    let setup = &fx["header"]["setup"]["forms"]["frmMain"];
    let forms = &c["inputs"]["forms"]["frmMain"];
    let g = &c["inputs"]["globals"];
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
    json!({
        "race": combo("cmbGlobalRace"),
        "class": combo("cmbGlobalClass"),
        "level": text("txtGlobalLevel", 0),
        "alignment": ctl("cmbGlobalAlignment")["ListIndex"].as_i64().unwrap_or(0),
        "global_filter": true,
        "stats": (0..6).map(|k| text("txtCharStats", k)).collect::<Vec<_>>(),
        "stat_adjustments": g["char_StatAdjustments"].as_array().cloned().unwrap_or_default(),
        "equipped": g["nEquippedItem"],
        "bless": (0..10).map(|k| elem("cmbCharBless", k)["ItemData"][0].as_i64().unwrap_or(0)).collect::<Vec<_>>(),
        "quests": (0..12).map(|k| elem("chkCharQuests", k)["Value"].as_i64() == Some(1)).collect::<Vec<_>>(),
        "quest_options": (0..4).map(|k| elem("cmbCharQuestOpts", k)["ListIndex"].as_i64().unwrap_or(-1)).collect::<Vec<_>>(),
        "attack": {
            "attack_type": gi("nGlobalAttackTypeMME"), "backstab": gb("bGlobalAttackBackstab"),
            "backstab_weapon": gi("nGlobalAttackBackstabWeapon"), "martial_art": gi("nGlobalAttackMA"),
            "spell_num": gi("nGlobalAttackSpellNum"), "spell_lvl": gi("nGlobalAttackSpellLVL"),
            "manual_p": gi("nGlobalAttackManualP"), "manual_m": gi("nGlobalAttackManualM"),
            "use_meditate": gb("bGlobalAttackUseMeditate"), "heal_type": gi("nGlobalAttackHealType"),
            "heal_spell_num": gi("nGlobalAttackHealSpellNum"), "heal_spell_lvl": gi("nGlobalAttackHealSpellLVL"),
            "heal_rounds": gi("nGlobalAttackHealRounds"), "heal_manual": gi("nGlobalAttackHealManual"),
        },
    })
}

/// Every record a step of the case lists must come back with the cells (and colours) the oracle
/// recorded. `post` names the pass whose rows are expected (RaceColorCode).
async fn pinned(id: &str, tab: &str, extra: Value, with_character: bool, out: &str, post: bool) {
    let fx = fixture();
    let c = case(&fx, id);
    let mut body = extra;
    if with_character {
        body["character"] = character(&fx, c);
    }
    let (status, got) = post_to(&fx, tab, &body).await;
    assert_eq!(status, StatusCode::OK, "{id}: {got}");
    let want = if post {
        &c["outputs"][format!("{out}.post")][0]["rows"]
    } else {
        &c["outputs"][out]
    };
    let keys: Vec<i64> = c["inputs"]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|st| st["as"] == out)
        .unwrap()["rows"]["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k.as_i64().unwrap())
        .collect();
    let shared = post;
    let listed: Vec<&Value> = got["rows"].as_array().unwrap().iter().collect();
    let mut checked = 0;
    let wanted: Vec<(Option<i64>, &Value)> = if shared {
        want.as_array().unwrap().iter().map(|r| (None, r)).collect()
    } else {
        keys.iter()
            .zip(want.as_array().unwrap())
            .filter(|(_, r)| !r.is_null())
            .map(|(k, r)| (Some(*k), r))
            .collect()
    };
    for (key, row) in wanted {
        let number = key.unwrap_or_else(|| row[0].as_str().unwrap().parse().unwrap());
        let Some(mine) = listed.iter().find(|r| r["number"] == number) else {
            // the fixture world has no cast-by or class lists, so SpellIsInGame drops some
            // sampled spells; every other tab lists its sample
            assert_eq!(tab, "spells", "{id}: record {number} is not listed");
            continue;
        };
        for (k, cell) in row[2].as_array().unwrap().iter().enumerate() {
            let (text, color) = match cell {
                Value::String(t) => (t.as_str(), None),
                other => (other[0].as_str().unwrap(), other[2].as_i64()),
            };
            let got_cell = &mine["cells"][k + 1];
            assert_eq!(got_cell["text"], text, "{id} record {number} cell {k}");
            // only the RaceColorCode cases record the colours MME's list pass adds
            if post || tab != "races" {
                assert_eq!(
                    got_cell["color"].as_i64(),
                    color,
                    "{id} record {number} colour {k}"
                );
            }
        }
        checked += 1;
    }
    assert!(checked > 0, "{id}: nothing was compared");
}

async fn post_to(fx: &Value, tab: &str, body: &Value) -> (StatusCode, Value) {
    post(&router(fx), &format!("/api/evaluate/{tab}"), body).await
}

#[tokio::test]
async fn no_character_lists_match_the_oracle() {
    // fixtures/rules/rows.json: AddArmour2LV, AddOtherItem2LV, AddShop2LV, AddClass2LV, AddRace2LV
    // and AddSpell2LV, with the global filter off
    pinned(
        "nochar-armour-all",
        "armour",
        json!({}),
        false,
        "rows",
        false,
    )
    .await;
    pinned(
        "nochar-sundry-all",
        "items",
        json!({}),
        false,
        "rows",
        false,
    )
    .await;
    pinned("nochar-shops-all", "shops", json!({}), false, "rows", false).await;
    pinned(
        "nochar-classes-all",
        "classes",
        json!({}),
        false,
        "rows",
        false,
    )
    .await;
    pinned("nochar-races-all", "races", json!({}), false, "rows", false).await;
    pinned("nochar-spells", "spells", json!({}), false, "rows", false).await;
}

#[tokio::test]
async fn race_colours_match_the_oracle() {
    // fixtures/rules/rows.json nochar-races-color: RaceColorCode over all 13 races
    pinned("nochar-races-color", "races", json!({}), false, "all", true).await;
}

#[tokio::test]
async fn weapons_match_the_oracle() {
    // fixtures/rules/rows.json: AddWeapon2LV with and without combat values
    pinned(
        "nochar-weapons-plain",
        "weapons",
        json!({}),
        false,
        "rows",
        false,
    )
    .await;
    pinned(
        "nochar-weapons-combat",
        "weapons",
        json!({"options": {"calc_combat": true, "slow_85": true, "attack": 5,
            "vs_ac": "40", "vs_dr": 5, "vs_dodge": "20"}}),
        false,
        "rows",
        false,
    )
    .await;
}

#[tokio::test]
async fn the_loaded_character_changes_the_lists() {
    // fixtures/rules/rows.json: the level 50 warrior's weapon, backstab and spell rows
    pinned(
        "warrior-weapons-plain",
        "weapons",
        json!({}),
        true,
        "rows",
        false,
    )
    .await;
    pinned(
        "warrior-weapons-backstab",
        "weapons",
        json!({"options": {"calc_combat": true, "attack": 4,
            "vs_ac": "10", "vs_dr": "3", "vs_dodge": "10"}}),
        true,
        "rows",
        false,
    )
    .await;
    pinned(
        "warrior-spells",
        "spells",
        json!({"options": {"level": 50}}),
        true,
        "rows",
        false,
    )
    .await;
    pinned(
        "warrior-spells-combat",
        "spells",
        json!({"options": {"level": 50, "calc_combat": true, "anti_magic": true, "vs_mr": 35}}),
        true,
        "rows",
        false,
    )
    .await;
}

#[tokio::test]
async fn columns_follow_the_list_views() {
    let fx = fixture();
    let app = router(&fx);
    let (_, w) = post(&app, "/api/evaluate/weapons", &json!({})).await;
    let headers: Vec<&str> = w["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["header"].as_str().unwrap())
        .collect();
    // frmMain SetUpFormObjects: lvWeapons.ColumnHeaders
    assert_eq!(
        headers,
        [
            "#", "Name", "Type", "Min", "Max", "Speed", "LVL", "Str", "Enc", "AC", "Acc", "BS",
            "Crits", "Limit", "Dmg/Spd", "#Swings", "xSwings", "Extra", "Dmg/Rnd", "Dmg/1st",
            "Ability"
        ]
    );
    for r in w["rows"].as_array().unwrap() {
        assert_eq!(r["cells"].as_array().unwrap().len(), headers.len());
    }
    // lvSpells column 7 reads Diff without a character and Cast with one
    let (_, sp) = post(&app, "/api/evaluate/spells", &json!({})).await;
    assert_eq!(sp["columns"][6]["header"], "Diff");
    let c = character(&fx, case(&fx, "warrior-spells"));
    let (_, sp) = post(
        &app,
        "/api/evaluate/spells",
        &json!({"character": c, "options": {"level": 50}}),
    )
    .await;
    assert_eq!(sp["columns"][6]["header"], "Cast");
    // lvArmour: AC/DR sorts by its tag (AC first), the ability column is numeric
    let (_, ar) = post(&app, "/api/evaluate/armour", &json!({})).await;
    let row = &ar["rows"][0]["cells"][6];
    assert!(row["sort"].is_number());
}

#[tokio::test]
async fn ability_filter_and_backstab_punch() {
    let fx = fixture();
    let app = router(&fx);
    // FilterArmour: only items with the ability at or above the box
    let (st, ar) = post(
        &app,
        "/api/evaluate/armour",
        &json!({"options": {"ability": 58, "ability_op": "ge", "ability_value": 1}}),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let rows = ar["rows"].as_array().unwrap();
    assert!(!rows.is_empty());
    assert!(
        rows.iter()
            .all(|r| r["cells"][11]["text"].as_str().unwrap() != "0")
    );
    // FilterWeapons: a backstab list ends with the surprise punch, record 0
    let (_, w) = post(
        &app,
        "/api/evaluate/weapons",
        &json!({"options": {"calc_combat": true, "attack": 4}}),
    )
    .await;
    let last = w["rows"].as_array().unwrap().last().unwrap();
    assert_eq!(last["number"], 0);
    assert_eq!(last["cells"][1]["text"], "Punch (Surprise)");
}

#[tokio::test]
async fn bad_requests_are_refused() {
    let fx = fixture();
    let app = router(&fx);
    let (st, _) = post(&app, "/api/evaluate/monsters", &json!({})).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    for body in [
        json!({"options": {"attack": 9, "calc_combat": true}}),
        json!({"options": {"nonsense": 1}}),
        json!({"options": {"ability": -5}}),
        json!({"options": {"level": 5000}}),
        json!({"character": {"race": 1, "class": 1, "level": 5000}}),
        json!([1, 2]),
    ] {
        let (st, _) = post(&app, "/api/evaluate/weapons", &body).await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "{body}");
    }
    let (st, msg) = post(
        &app,
        "/api/evaluate/spells",
        &json!({"character": {"level": 5}}),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(msg.as_str().unwrap().contains("pick a class and race"));
}

#[tokio::test]
async fn repeated_requests_come_from_the_cache() {
    let fx = fixture();
    let app = router(&fx);
    let body = json!({"options": {"calc_combat": true, "attack": 6}});
    let (_, a) = post(&app, "/api/evaluate/weapons", &body).await;
    let (_, b) = post(&app, "/api/evaluate/weapons", &body).await;
    assert_eq!(a, b);
}
