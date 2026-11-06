// Fixture tests for the detail panes (class, race, shop, NPC command references, spell): see
// docs/porting.md step 5.
//
// Expected values: fixtures/rules/misc_detail.json, produced by the oracle running MME's own modMain
// procedures against the stock database (plus invented rows numbered 9001 and up). The records the
// cases can reach are under the header's "data" as {fields, rows}; the World is built from them.
use serde_json::Value;
use touchstone_model::{
    AbilitySlot, Class, Info, Item, Monster, MonsterAttack, Race, Room, Shop, ShopSlot, Spell,
    TbInfo, World, WorldParts,
};
use touchstone_rules::Profile;
use touchstone_rules::ability_text::AbilityEnv;
use touchstone_rules::detail::class_race::{pull_class_detail, pull_race_detail};
use touchstone_rules::detail::shop::{
    NpcRefKind, ShopDetailOptions, ShopLines, add_room_npc_command_refs, pull_shop_detail,
};
use touchstone_rules::lairs::LairInfo;
use touchstone_rules::locations::{LocationLairs, LocationOptions, LocationRow};
use touchstone_vb6::Currency;

use crate::fixture::load;

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

pub(crate) struct Lairs(pub(crate) serde_json::Map<String, Value>);

impl LocationLairs for Lairs {
    fn get_lair_info(&mut self, group_index: &str, max_regen: i16) -> LairInfo {
        let mut u = LairInfo::default();
        let Some(row) = self.0.get(group_index) else {
            return u;
        };
        let cur = |k: &str| {
            row.get(k).map_or(Currency::from_scaled(0), |v| {
                Currency::from_f64(v.as_f64().unwrap()).unwrap()
            })
        };
        u.group_index = group_index.to_string();
        u.mobs = cur("nMobs");
        u.avg_exp = cur("nAvgExp");
        u.avg_dmg_clear_round = cur("nAvgDmgClearRound");
        u.damage_out = row
            .get("nDamageOut")
            .map_or(0, |v| v.as_i64().unwrap() as i32);
        u.max_regen = Currency::from_scaled(i64::from(max_regen) * 10_000);
        u
    }
}

/// A row as the oracle writes it: [Text, Tag, cells], a cell its Text or [Text, Tag, null].
pub(crate) fn loc_row(r: &LocationRow) -> Value {
    let cells: Vec<Value> = r
        .cells
        .iter()
        .map(|c| match &c.tag {
            None => Value::from(c.text.clone()),
            Some(t) => serde_json::json!([c.text, t, null]),
        })
        .collect();
    serde_json::json!([r.text, r.tag, cells])
}

pub(crate) fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

/// The frontend state `GetLocations` reads, from a case's controls and globals.
pub(crate) fn location_options(setup: &Value, case: &Value) -> LocationOptions {
    let frm = &case["inputs"]["forms"]["frmMain"];
    let glob = |k: &str| -> &Value {
        let v = &case["inputs"]["globals"][k];
        if v.is_null() { &setup["globals"][k] } else { v }
    };
    LocationOptions {
        nmr_ver: glob("nNMRVer").as_f64().unwrap_or(1.83),
        hide_record_numbers: glob("bHideRecordNumbers").as_bool().unwrap_or(false),
        global_filter: frm["chkGlobalFilter"]["Value"].as_i64() == Some(1),
        charm_tag: frm["txtCharStats"]["$array"]["5"]["Tag"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        attack_type: glob("nGlobalAttackTypeMME").as_i64().unwrap_or(0) as i32,
    }
}

pub(crate) fn global_bool(setup: &Value, case: &Value, k: &str) -> bool {
    let v = &case["inputs"]["globals"][k];
    let v = if v.is_null() { &setup["globals"][k] } else { v };
    v.as_bool().unwrap_or(false)
}

fn lair_rows(data: &Value) -> Lairs {
    Lairs(data["lairs"].as_object().unwrap().clone())
}

#[test]
fn class_and_race_detail_match_the_oracle() {
    let fx = load("misc_detail");
    let world = world(&fx.header["data"]);
    let env = AbilityEnv::for_world(&world, Profile::Stock);
    let mut bad = Vec::new();
    let mut n = 0;
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        let step = &case["inputs"]["steps"][0];
        let call = step["call"].as_str().unwrap_or("");
        if call != "PullClassDetail" && call != "PullRaceDetail" {
            continue;
        }
        n += 1;
        let num = i(&step["args"][0]);
        let startup = global_bool(&fx.header["setup"], case, "bStartup");
        let mut env = AbilityEnv::for_world(&world, Profile::Stock);
        env.hide_record_numbers = global_bool(&fx.header["setup"], case, "bHideRecordNumbers");
        let got = if call == "PullClassDetail" {
            pull_class_detail(&env, &world, num, startup)
        } else {
            pull_race_detail(&env, &world, num, startup)
        };
        // expected: the text box after the call
        let want = text(&case["outputs"]["text"]);
        if got.text != want {
            bad.push(format!("{id}: got {:?}, want {want:?}", got.text));
        }
    }
    let _ = env;
    assert!(n > 30);
    assert!(
        bad.is_empty(),
        "{} mismatches:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

#[test]
fn shop_detail_matches_the_oracle() {
    let fx = load("misc_detail");
    let setup = &fx.header["setup"];
    let world = world(&fx.header["data"]);
    let mut lairs = lair_rows(&fx.header["data"]);
    let mut bad = Vec::new();
    let mut n = 0;
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        let step = &case["inputs"]["steps"][0];
        if step["call"].as_str() != Some("PullShopDetail") {
            continue;
        }
        n += 1;
        let a = &step["args"];
        let opts = ShopDetailOptions {
            startup: global_bool(setup, case, "bStartup"),
            charm: a[4].as_i64().unwrap() as i16,
            show_sell: a[5].as_bool().unwrap(),
            locations: location_options(setup, case),
        };
        let got = pull_shop_detail(Profile::Stock, &world, &mut lairs, i(&a[0]), &opts);
        let out = &case["outputs"];
        let rows: Vec<Value> = match &got.lines {
            ShopLines::None => vec![],
            ShopLines::NotFound => vec![serde_json::json!(["Shop not found", null, []])],
            ShopLines::Stock(rs) => rs
                .iter()
                .map(|r| {
                    serde_json::json!([
                        r.number,
                        null,
                        [r.name, r.max, r.regen, [r.cost, r.copper, null]]
                    ])
                })
                .collect(),
            ShopLines::Training(rs) => rs
                .iter()
                .map(|r| serde_json::json!([r.level, null, [[r.cost, r.copper, null]]]))
                .collect(),
        };
        let loc: Vec<Value> = got.assigned.rows.iter().map(loc_row).collect();
        let mut why = Vec::new();
        if got.text != text(&out["text"]) {
            why.push(format!("text {:?} vs {:?}", got.text, out["text"]));
        }
        if got.charm_label != text(&out["charm"]) {
            why.push(format!("charm {:?} vs {:?}", got.charm_label, out["charm"]));
        }
        if Value::Array(rows.clone()) != out["rows"] {
            why.push(format!("rows {} vs {}", Value::Array(rows), out["rows"]));
        }
        if Value::Array(loc.clone()) != out["loc"] {
            why.push(format!("loc {} vs {}", Value::Array(loc), out["loc"]));
        }
        if !why.is_empty() {
            bad.push(format!("{id}: {}", why.join("; ")));
        }
    }
    assert!(n > 150);
    assert!(
        bad.is_empty(),
        "{} mismatches:\n{}",
        bad.len(),
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn npc_command_refs_match_the_oracle() {
    let fx = load("misc_detail");
    let world = world(&fx.header["data"]);
    let mut bad = Vec::new();
    let mut n = 0;
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        let step = &case["inputs"]["steps"][0];
        if step["call"].as_str() != Some("AddRoomNPCCommandRefs") {
            continue;
        }
        n += 1;
        let a = &step["args"];
        let got = add_room_npc_command_refs(&world, i(&a[1]), i(&a[2]));
        let mut want: Vec<Value> = Vec::new();
        for r in case["inputs"]["forms"]["frmMain"]["lvMap"]["$recordlv"]
            .as_array()
            .map_or(&vec![], |v| v)
        {
            want.push(serde_json::json!([r["Text"], r["Tag"], []]));
        }
        for r in &got {
            let _ = matches!(r.kind, NpcRefKind::Teleport);
            want.push(serde_json::json!([r.text, r.tag, []]));
        }
        if Value::Array(want.clone()) != case["outputs"]["rows"] {
            bad.push(format!(
                "{id}: got {}, want {}",
                Value::Array(want),
                case["outputs"]["rows"]
            ));
        }
    }
    assert!(n > 100);
    assert!(
        bad.is_empty(),
        "{} mismatches:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

fn global_f64(setup: &Value, case: &Value, k: &str, default: f64) -> f64 {
    let v = &case["inputs"]["globals"][k];
    let v = if v.is_null() { &setup["globals"][k] } else { v };
    v.as_f64().unwrap_or(default)
}

// Expected values: fixtures/rules/misc_detail.json (modMain PullSpellDetail run by the oracle, for no
// character and for two characters RefreshAll'd from frmMain.frm).
#[test]
fn spell_detail_matches_the_oracle() {
    use touchstone_rules::detail::spell::{SpellDetailOptions, pull_spell_detail};
    use touchstone_rules::pipeline::refresh_all;

    let fx = load("misc_detail");
    let setup = &fx.header["setup"];
    let world = world(&fx.header["data"]);
    let mut lairs = lair_rows(&fx.header["data"]);
    let mut bad = Vec::new();
    let mut n = 0;
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        let steps = case["inputs"]["steps"].as_array().unwrap();
        if !steps
            .iter()
            .any(|s| s["call"].as_str() == Some("PullSpellDetail"))
        {
            continue;
        }
        let inputs = crate::pipeline::Inputs::new(&fx.header, case);
        let state = if steps[0]["call"].as_str() == Some("RefreshAll") {
            Some(
                refresh_all(
                    Profile::Stock,
                    &world,
                    &inputs.character(),
                    &inputs.options(),
                )
                .unwrap_or_else(|e| panic!("{id}: {e}")),
            )
        } else {
            None
        };
        let opts = SpellDetailOptions {
            startup: global_bool(setup, case, "bStartup"),
            combat_option: inputs.elem("chkSpellOptions", 0)["Value"].as_i64() == Some(1),
            combat_value: touchstone_vb6::val(
                inputs.elem("txtSpellOptions", 0)["Text"]
                    .as_str()
                    .unwrap_or(""),
            ),
            anti_magic: inputs.elem("chkSpellOptions", 2)["Value"].as_i64() == Some(1),
            level_box: touchstone_vb6::val(
                inputs.elem("txtGlobalLevel", 1)["Text"]
                    .as_str()
                    .unwrap_or(""),
            ),
            nmr_ver: global_f64(setup, case, "nNMRVer", 1.83),
            only_in_game: global_bool(setup, case, "bOnlyInGame"),
            disable_kai_autolearn: global_bool(setup, case, "bDisableKaiAutolearn"),
            hide_record_numbers: global_bool(setup, case, "bHideRecordNumbers"),
            avg_level_max_all_stats: global_f64(setup, case, "gAvgLevelMaxAllStats", 0.0),
            locations: location_options(setup, case),
        };
        for st in steps
            .iter()
            .filter(|s| s["call"].as_str() == Some("PullSpellDetail"))
        {
            n += 1;
            let num = i(&st["args"][0]);
            let got = pull_spell_detail(
                Profile::Stock,
                &world,
                &mut lairs,
                state.as_ref(),
                num,
                &opts,
            );
            let want = match st["as"].as_str() {
                Some(a) => &case["outputs"][format!("{a}.cap")],
                None => &case["outputs"],
            };
            let loc = Value::Array(got.locations.rows.iter().map(loc_row).collect());
            let mut why = Vec::new();
            if got.text != text(&want["text"]) {
                why.push(format!("text {:?} vs {:?}", got.text, want["text"]));
            }
            if loc != want["loc"] {
                why.push(format!("loc {loc} vs {}", want["loc"]));
            }
            if !why.is_empty() {
                bad.push(format!("{id} spell {num}: {}", why.join("; ")));
            }
        }
    }
    assert!(n > 250, "{n}");
    assert!(
        bad.is_empty(),
        "{} of {n} mismatches:\n{}",
        bad.len(),
        bad.iter().take(10).cloned().collect::<Vec<_>>().join("\n")
    );
}
