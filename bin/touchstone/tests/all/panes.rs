//! `/api/detail/{kind}/{n}`, pinned to fixtures/rules/item_detail.json (PullItemDetail) and
//! misc_detail.json (spell, shop, class and race detail); the world is rebuilt from the fixture
//! headers. Reference rows are compared as a set: the fixtures record the ListView before its
//! sorts, the endpoint answers after them (order is unit-tested in `panes.rs`).

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

fn room(map: i32, number: i32, name: String, delay: i32) -> Room {
    let e = || String::new();
    Room {
        map_number: map,
        room_number: number,
        name: Some(name),
        light: 0,
        shop: 0,
        npc: 0,
        cmd: 0,
        spell: 0,
        lair: None,
        delay,
        placed: None,
        n: e(),
        s: e(),
        e: e(),
        w: e(),
        ne: e(),
        nw: e(),
        se: e(),
        sw: e(),
        u: e(),
        d: e(),
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

fn nonzero<const N: usize>(v: &Value) -> [i32; N] {
    let mut out = [0; N];
    for (k, x) in v.as_array().unwrap().iter().enumerate() {
        out[k] = i(x);
    }
    out
}

fn blank_item(number: i32, name: String) -> Item {
    Item {
        number,
        name,
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
    }
}

/// Fields: Number, Name, Limit, Encum, ItemType, UseCount, Price, Currency, Min, Max, ArmourClass, DamageResist, WeaponType,
/// ArmourType, Worn, Accy, Gettable, StrReq, Speed, Not Droppable, Destroy On Death, Retain After Uses, Obtained From,
/// References, ClassRest, RaceRest, NegateSpell, Abilities. The short row (items only a reference text reaches) is
/// Number, Name, ItemType, Price, Currency, Obtained From.
fn item(r: &Value) -> Item {
    let mut it = blank_item(i(&r[0]), s(&r[1]));
    if r.as_array().unwrap().len() == 6 {
        it.item_type = i(&r[2]);
        it.price = i(&r[3]);
        it.currency = i(&r[4]);
        it.obtained_from = Some(s(&r[5]));
        return it;
    }
    it.limit = i(&r[2]);
    it.encum = i(&r[3]);
    it.item_type = i(&r[4]);
    it.use_count = i(&r[5]);
    it.price = i(&r[6]);
    it.currency = i(&r[7]);
    it.min = i(&r[8]);
    it.max = i(&r[9]);
    it.armour_class = i(&r[10]);
    it.damage_resist = i(&r[11]);
    it.weapon_type = i(&r[12]);
    it.armour_type = i(&r[13]);
    it.worn = i(&r[14]);
    it.accy = i(&r[15]);
    it.gettable = i(&r[16]) != 0;
    it.str_req = i(&r[17]);
    it.speed = i(&r[18]);
    it.not_droppable = i(&r[19]) != 0;
    it.destroy_on_death = i(&r[20]) != 0;
    it.retain_after_uses = i(&r[21]) != 0;
    it.obtained_from = Some(s(&r[22]));
    it.references = Some(s(&r[23]));
    it.class_rest = nonzero(&r[24]);
    it.race_rest = nonzero(&r[25]);
    it.negate_spell = nonzero(&r[26]);
    it.abilities = pairs(&r[27]);
    it
}

fn spell(r: &Value) -> Spell {
    // Number, Name, Short, ReqLevel, EnergyCost, ManaCost, MinBase, MaxBase, Diff, TypeOfResists, Targets, Dur, AttType,
    // Magery, MageryLVL, Cap, MaxIncLVLs, MaxInc, MinIncLVLs, MinInc, DurIncLVLs, DurInc, Learnable, Abilities
    Spell {
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
        abilities: pairs(&r[23]),
        learnable: i(&r[22]) == 1,
        learned_from: None,
        casted_by: None,
        classes: None,
    }
}

fn fixture_world(data: &Value, dat_ver: &str) -> World {
    let rows = |k: &str| data[k].as_array().unwrap().clone();
    let monsters: Vec<Monster> = rows("monster_names")
        .iter()
        .map(|r| monster(i(&r[0]), s(&r[1])))
        .collect();
    let rooms: Vec<Room> = rows("rooms")
        .iter()
        .map(|r| room(i(&r[0]), i(&r[1]), s(&r[2]), i(&r[3])))
        .collect();
    let tbinfo: Vec<TbInfo> = rows("textblocks")
        .iter()
        .map(|p| TbInfo {
            number: i(&p[0]),
            link_to: 0,
            action: Some(s(&p[1])),
            called_from: String::new(),
        })
        .collect();
    let mut items: Vec<Item> = rows("items").iter().map(item).collect();
    items.sort_by_key(|r| r.number);
    let shops = rows("shops")
        .iter()
        .map(|r| {
            let mut stock = [ShopSlot::default(); 20];
            for (slot, st) in stock.iter_mut().zip(r[4].as_array().unwrap()) {
                *slot = ShopSlot {
                    item: i(&st[0]),
                    max: i(&st[1]),
                    time: i(&st[2]),
                    amount: i(&st[3]),
                    pct: i(&st[4]),
                };
            }
            Shop {
                number: i(&r[0]),
                name: String::new(),
                shop_type: i(&r[1]),
                min_lvl: 0,
                max_lvl: 0,
                markup_pct: i(&r[2]),
                class_rest: 0,
                stock,
                in_game: true,
                assigned_to: Some(s(&r[3])),
            }
        })
        .collect();
    let classes: Vec<Class> = rows("classes")
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
    let races: Vec<Race> = rows("races")
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
    let world = World::new(WorldParts {
        info: Info {
            nmr_version: "v1.8.3".to_owned(),
            dat_file_version: dat_ver.to_owned(),
            date: String::new(),
            time: String::new(),
            custom: String::new(),
            legit: false,
            disable_kai: false,
            update_url: String::new(),
        },
        items,
        monsters,
        spells: rows("spells").iter().map(spell).collect(),
        rooms,
        shops,
        classes,
        races,
        lairs: vec![],
        tbinfo,
    })
    .unwrap();
    world
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

fn fixture(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../fixtures/rules/{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
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

fn case<'a>(fx: &'a Value, id: &str) -> &'a Value {
    fx["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no case {id}"))
}

/// A reference row as (text, tag, cells) strings, for set comparison. Lair rows are left out: the
/// fixture answers `GetLairInfo` from its own table, the server from the lair cache of the world.
fn flat_fixture(r: &Value) -> Option<String> {
    let cells: Vec<String> = r[2]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| match c {
            Value::String(s) => format!("{s}|"),
            other => format!(
                "{}|{}",
                other[0].as_str().unwrap(),
                other[1].as_str().unwrap_or("")
            ),
        })
        .collect();
    let s = format!(
        "{}|{}|{}",
        r[0].as_str().unwrap_or(""),
        r[1].as_str().unwrap_or(""),
        cells.join(";")
    );
    (!s.contains("Lair")).then_some(s)
}

fn flat_got(r: &Value) -> Option<String> {
    let cells: Vec<String> = r["cells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            format!(
                "{}|{}",
                c["text"].as_str().unwrap(),
                c["tag"].as_str().unwrap_or("")
            )
        })
        .collect();
    let s = format!(
        "{}|{}|{}",
        r["text"].as_str().unwrap(),
        r["tag"].as_str().unwrap_or(""),
        cells.join(";")
    );
    (!s.contains("Lair")).then_some(s)
}

/// Every key of the case's PullItemDetail step must come back with the recorded text and rows.
async fn pinned_items(id: &str, with_character: bool) -> usize {
    let fx = fixture("item_detail");
    let dat = fx["header"]["setup"]["forms"]["frmMain"]["lblDatVer"]["Caption"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    let app = router(fixture_world(&fx["header"]["data"], &dat));
    let c = case(&fx, id);
    let step = c["inputs"]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["call"] == "PullItemDetail")
        .unwrap();
    let want = c["outputs"]["rows.capture"].as_array().unwrap();
    let mut body = json!({
        "attack_type": step["args"][2],
        "full_details": step["args"][3],
    });
    if with_character {
        body["character"] = character(&fx, c);
        body["global_filter"] = json!({"use_character": true, "min_item_level": 0});
    }
    let mut checked = 0;
    for (key, want) in step["rows"]["keys"].as_array().unwrap().iter().zip(want) {
        let n = key.as_i64().unwrap();
        let (status, got) = call(&app, &format!("/api/detail/items/{n}"), Some(&body)).await;
        assert_eq!(status, StatusCode::OK, "{id} item {n}: {got}");
        let want_text = want["detail"].as_str().unwrap();
        assert_eq!(got["text"], want_text, "{id} item {n}");
        // the compare blocks are the "Compared to" pieces of the text
        for cmp in got["compare"].as_array().unwrap() {
            let piece = format!(
                "{}: {}",
                cmp["label"].as_str().unwrap(),
                cmp["text"].as_str().unwrap()
            );
            assert!(want_text.contains(&piece), "{id} item {n}: {piece}");
        }
        let mut got_rows: Vec<String> = got["references"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(flat_got)
            .collect();
        let mut want_rows: Vec<String> = want["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(flat_fixture)
            .collect();
        got_rows.sort();
        want_rows.sort();
        assert_eq!(got_rows, want_rows, "{id} item {n}");
        checked += 1;
    }
    checked
}

// Expected: fixtures/rules/item_detail.json cases nochar-plain and nochar-full (PullItemDetail).
#[tokio::test]
async fn item_panes_match_the_oracle_without_a_character() {
    let a = pinned_items("nochar-plain", false).await;
    let b = pinned_items("nochar-full", false).await;
    assert!(a > 30 && b > 30, "{a} {b}");
}

// Expected: fixtures/rules/item_detail.json case warrior-full (a level-50 warrior's PullItemDetail).
#[tokio::test]
async fn item_panes_match_the_oracle_with_a_character() {
    assert!(pinned_items("warrior-full", true).await > 30);
}

// Expected: fixtures/rules/item_detail.json compare-0 sets nEquippedItem; the same slots on a
// character give the "Compared to" blocks apart from the rows.
#[tokio::test]
async fn compare_blocks_come_apart_from_the_rows() {
    let fx = fixture("item_detail");
    let c = case(&fx, "warrior-full");
    let dat = fx["header"]["setup"]["forms"]["frmMain"]["lblDatVer"]["Caption"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    let app = router(fixture_world(&fx["header"]["data"], &dat));
    let step = c["inputs"]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["call"] == "PullItemDetail")
        .unwrap();
    let body = json!({"character": character(&fx, c), "global_filter": {"use_character": true}});
    let mut compared = 0;
    for key in step["rows"]["keys"].as_array().unwrap() {
        let (_, got) = call(&app, &format!("/api/detail/items/{key}"), Some(&body)).await;
        if !got["compare"].as_array().unwrap().is_empty() {
            compared += 1;
            assert!(
                got["rows"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|r| r["kind"] != "compared")
            );
        }
    }
    assert!(compared > 0, "no item compared to the equipped ones");
}

#[tokio::test]
async fn unknown_records_and_panes_are_404() {
    let fx = fixture("item_detail");
    let app = router(fixture_world(&fx["header"]["data"], ""));
    for uri in [
        "/api/detail/items/99999",
        "/api/detail/spells/99999",
        "/api/detail/shops/99999",
        "/api/detail/nope/1",
    ] {
        let (status, _) = call(&app, uri, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
    let (status, _) = call(&app, "/api/detail/items/1", Some(&json!({"charm": 99999}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

fn misc_app(fx: &Value) -> axum::Router {
    router(misc::world(&fx["header"]["data"]))
}

// Expected: fixtures/rules/misc_detail.json cases classN, raceN (PullClassDetail, PullRaceDetail).
#[tokio::test]
async fn class_and_race_panes_match_the_oracle() {
    let fx = fixture("misc_detail");
    let app = misc_app(&fx);
    let mut n = 0;
    for c in fx["cases"].as_array().unwrap() {
        let id = c["id"].as_str().unwrap();
        let (Some(kind), true) = (
            id.strip_prefix("class-")
                .map(|_| "classes")
                .or(id.strip_prefix("race-").map(|_| "races")),
            id.split('-')
                .nth(1)
                .is_some_and(|x| x.parse::<u32>().is_ok())
                && id.split('-').count() == 2,
        ) else {
            continue;
        };
        let num = c["inputs"]["steps"][0]["args"][0].as_i64().unwrap();
        let (status, got) = call(&app, &format!("/api/detail/{kind}/{num}"), None).await;
        assert_eq!(status, StatusCode::OK, "{id}");
        assert_eq!(got["text"], c["outputs"]["text"], "{id}");
        n += 1;
    }
    assert!(n >= 28, "{n}");
}

// Expected: fixtures/rules/misc_detail.json cases shop-N-0 and shop-N-1 (PullShopDetail): the
// summary text, the charm label and the assigned-to rows.
#[tokio::test]
async fn shop_panes_match_the_oracle() {
    let fx = fixture("misc_detail");
    let app = misc_app(&fx);
    let mut n = 0;
    for c in fx["cases"].as_array().unwrap() {
        let id = c["id"].as_str().unwrap();
        let parts: Vec<&str> = id.split('-').collect();
        if parts.len() != 3 || parts[0] != "shop" || parts[1].parse::<u32>().is_err() {
            continue;
        }
        let args = &c["inputs"]["steps"][0]["args"];
        let hide = c["inputs"]["globals"]["bHideRecordNumbers"] == true;
        let body = json!({"charm": args[4], "show_sell": args[5], "hide_record_numbers": hide});
        let (status, got) =
            call(&app, &format!("/api/detail/shops/{}", args[0]), Some(&body)).await;
        assert_eq!(status, StatusCode::OK, "{id}: {got}");
        assert_eq!(got["text"], c["outputs"]["text"], "{id}");
        assert_eq!(got["charm_label"], c["outputs"]["charm"], "{id}");
        let want: Vec<String> = c["outputs"]["loc"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(flat_fixture)
            .collect();
        let mut got_rows: Vec<String> = got["references"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(flat_got)
            .collect();
        let mut want = want;
        got_rows.sort();
        want.sort();
        assert_eq!(got_rows, want, "{id}");
        n += 1;
    }
    assert!(n > 100, "{n}");
}

// Expected: fixtures/rules/misc_detail.json cases spellN (PullSpellDetail, no character).
#[tokio::test]
async fn spell_panes_match_the_oracle() {
    let fx = fixture("misc_detail");
    let app = misc_app(&fx);
    let mut n = 0;
    for c in fx["cases"].as_array().unwrap() {
        let id = c["id"].as_str().unwrap();
        let parts: Vec<&str> = id.split('-').collect();
        if parts.len() != 2 || parts[0] != "spell" || parts[1].parse::<u32>().is_err() {
            continue;
        }
        let num = c["inputs"]["steps"][0]["args"][0].as_i64().unwrap();
        let hide = c["inputs"]["globals"]["bHideRecordNumbers"] == true;
        let body = json!({"hide_record_numbers": hide});
        let (status, got) = call(&app, &format!("/api/detail/spells/{num}"), Some(&body)).await;
        assert_eq!(status, StatusCode::OK, "{id}: {got}");
        assert_eq!(got["text"], c["outputs"]["text"], "{id}");
        let mut got_rows: Vec<String> = got["references"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(flat_got)
            .collect();
        let mut want: Vec<String> = c["outputs"]["loc"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(flat_fixture)
            .collect();
        got_rows.sort();
        want.sort();
        assert_eq!(got_rows, want, "{id}");
        n += 1;
    }
    assert!(n > 20, "{n}");
}

/// The world of misc_detail.json's header data (copied from the rules test that pins it).
mod misc {
    #![allow(clippy::needless_range_loop)]
    use serde_json::Value;
    use touchstone_model::{
        AbilitySlot, Class, Info, Item, Monster, MonsterAttack, Race, Room, Shop, ShopSlot, Spell,
        TbInfo, World, WorldParts,
    };

    pub(crate) fn i(v: &Value) -> i32 {
        v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
    }

    /// A table of the header's data: rows addressed by field name.
    pub(crate) struct Table<'a> {
        fields: Vec<&'a str>,
        rows: &'a Vec<Value>,
    }

    pub(crate) struct Row<'a> {
        t: &'a Table<'a>,
        r: &'a Value,
    }

    impl<'a> Table<'a> {
        pub(crate) fn new(v: &'a Value) -> Self {
            Table {
                fields: v["fields"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|f| f.as_str().unwrap())
                    .collect(),
                rows: v["rows"].as_array().unwrap(),
            }
        }
        pub(crate) fn iter(&'a self) -> impl Iterator<Item = Row<'a>> {
            self.rows.iter().map(move |r| Row { t: self, r })
        }
    }

    impl Row<'_> {
        pub(crate) fn v(&self, f: &str) -> &Value {
            let k = self
                .t
                .fields
                .iter()
                .position(|x| *x == f)
                .unwrap_or_else(|| panic!("no field {f}"));
            &self.r[k]
        }
        pub(crate) fn i(&self, f: &str) -> i32 {
            i(self.v(f))
        }
        pub(crate) fn s(&self, f: &str) -> String {
            self.v(f).as_str().unwrap().to_string()
        }
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

    fn monster(number: i32, name: &str, greet: i32) -> Monster {
        let attack = || MonsterAttack {
            name: String::new(),
            att_type: 0,
            acc: 0,
            pct: 0,
            true_pct: 0.0,
            min: 0,
            max: 0,
            energy: 0,
            hit_spell: 0,
        };
        Monster {
            number,
            name: Some(name.to_string()),
            weapon: 0,
            armour_class: 0,
            damage_resist: 0,
            follow_pct: 0,
            magic_res: 0,
            bs_defense: 0,
            exp: 0.0,
            exp_multi: 0.0,
            hp: 0,
            energy: 0,
            avg_dmg: 0.0,
            avg_dmg_phys: 0.0,
            avg_dmg_spell: 0.0,
            greet_txt: greet,
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
            attacks: [attack(), attack(), attack(), attack(), attack()],
            mid_spells: Default::default(),
            drops: Default::default(),
            abilities: [AbilitySlot::default(); 10],
            in_game: true,
            summoned_by: None,
        }
    }

    /// The World the cases can reach, from the fixture header's `data`.
    pub(crate) fn world(data: &Value) -> World {
        let items = Table::new(&data["items"])
            .iter()
            .map(|r| {
                let mut class_rest = [0; 10];
                for (k, c) in r.v("ClassRest").as_array().unwrap().iter().enumerate() {
                    class_rest[k] = i(c);
                }
                Item {
                    number: r.i("Number"),
                    name: r.s("Name"),
                    limit: 0,
                    encum: r.i("Encum"),
                    item_type: r.i("ItemType"),
                    use_count: 0,
                    price: r.i("Price"),
                    currency: r.i("Currency"),
                    min: 0,
                    max: 0,
                    armour_class: r.i("ArmourClass"),
                    damage_resist: r.i("DamageResist"),
                    weapon_type: r.i("WeaponType"),
                    armour_type: r.i("ArmourType"),
                    worn: r.i("Worn"),
                    accy: r.i("Accy"),
                    gettable: true,
                    str_req: r.i("StrReq"),
                    speed: r.i("Speed"),
                    not_droppable: false,
                    destroy_on_death: false,
                    retain_after_uses: false,
                    class_rest,
                    race_rest: [0; 10],
                    negate_spell: [0; 10],
                    abilities: pairs(r.v("Abilities")),
                    in_game: true,
                    obtained_from: Some(r.s("Obtained From")),
                    references: None,
                }
            })
            .collect();
        let spells = Table::new(&data["spells"])
            .iter()
            .map(|r| Spell {
                number: r.i("Number"),
                name: Some(r.s("Name")),
                short: None,
                req_level: r.i("ReqLevel"),
                energy_cost: r.i("EnergyCost"),
                mana_cost: r.i("ManaCost"),
                min_base: r.i("MinBase"),
                max_base: r.i("MaxBase"),
                diff: r.i("Diff"),
                type_of_resists: r.i("TypeOfResists"),
                targets: r.i("Targets"),
                dur: r.i("Dur"),
                att_type: r.i("AttType"),
                magery: r.i("Magery"),
                magery_lvl: r.i("MageryLVL"),
                cap: r.i("Cap"),
                max_inc_lvls: r.i("MaxIncLVLs"),
                max_inc: r.i("MaxInc"),
                min_inc_lvls: r.i("MinIncLVLs"),
                min_inc: r.i("MinInc"),
                dur_inc_lvls: r.i("DurIncLVLs"),
                dur_inc: r.i("DurInc"),
                abilities: pairs(r.v("Abilities")),
                learnable: r.i("Learnable") != 0,
                learned_from: Some(r.s("Learned From")),
                casted_by: Some(r.s("Casted By")),
                classes: Some(r.s("Classes")),
            })
            .collect();
        let shops = Table::new(&data["shops"])
            .iter()
            .map(|r| {
                let mut stock = [ShopSlot::default(); 20];
                for (slot, s) in stock.iter_mut().zip(r.v("Stock").as_array().unwrap()) {
                    *slot = ShopSlot {
                        item: i(&s[0]),
                        max: i(&s[1]),
                        time: i(&s[2]),
                        amount: i(&s[3]),
                        pct: i(&s[4]),
                    };
                }
                Shop {
                    number: r.i("Number"),
                    name: String::new(),
                    shop_type: r.i("ShopType"),
                    min_lvl: r.i("MinLVL"),
                    max_lvl: r.i("MaxLVL"),
                    markup_pct: r.i("Markup%"),
                    class_rest: r.i("ClassRest"),
                    stock,
                    in_game: true,
                    assigned_to: Some(r.s("Assigned To")),
                }
            })
            .collect();
        let classes = Table::new(&data["classes"])
            .iter()
            .map(|r| Class {
                number: r.i("Number"),
                name: r.s("Name"),
                min_hits: r.i("MinHits"),
                max_hits: r.i("MaxHits"),
                exp_table: r.i("ExpTable"),
                magery_type: r.i("MageryType"),
                magery_lvl: r.i("MageryLVL"),
                weapon_type: r.i("WeaponType"),
                armour_type: r.i("ArmourType"),
                combat_lvl: r.i("CombatLVL"),
                abilities: pairs(r.v("Abilities")),
            })
            .collect();
        let races = Table::new(&data["races"])
            .iter()
            .map(|r| Race {
                number: r.i("Number"),
                name: r.s("Name"),
                m_int: r.i("mINT"),
                m_wil: r.i("mWIL"),
                m_str: r.i("mSTR"),
                m_hea: r.i("mHEA"),
                m_agl: r.i("mAGL"),
                m_chm: r.i("mCHM"),
                x_int: r.i("xINT"),
                x_wil: r.i("xWIL"),
                x_str: r.i("xSTR"),
                x_hea: r.i("xHEA"),
                x_agl: r.i("xAGL"),
                x_chm: r.i("xCHM"),
                hp_per_lvl: r.i("HPPerLVL"),
                exp_table: r.i("ExpTable"),
                base_cp: r.i("BaseCP"),
                abilities: pairs(r.v("Abilities")),
            })
            .collect();
        let monsters = Table::new(&data["monsters"])
            .iter()
            .map(|r| monster(r.i("Number"), &r.s("Name"), r.i("GreetTXT")))
            .collect();
        let tbinfo = Table::new(&data["tbinfo"])
            .iter()
            .map(|r| TbInfo {
                number: r.i("Number"),
                link_to: r.i("LinkTo"),
                action: Some(r.s("Action")),
                called_from: String::new(),
            })
            .collect();
        let rooms = Table::new(&data["rooms"])
            .iter()
            .map(|r| Room {
                map_number: r.i("Map Number"),
                room_number: r.i("Room Number"),
                name: Some(r.s("Name")),
                light: 0,
                shop: 0,
                npc: 0,
                cmd: 0,
                spell: 0,
                lair: None,
                delay: r.i("Delay"),
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
        World::new(WorldParts {
            info: Info {
                nmr_version: "v1.8.3".to_owned(),
                dat_file_version: String::new(),
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
}
