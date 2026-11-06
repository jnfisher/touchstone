// Fixture tests for the item detail pane (PullItemDetail with CompareArmor and CompareWeapons): see docs/porting.md step 5.
// Expected values: fixtures/rules/item_detail.json, produced by the oracle running MME's own PullItemDetail against the stock
// database. Per item: `<as>.capture[i]` holds the detail text, the reference-list rows and the events (ListView sort
// requests and handled errors) of that call. The records come from the header's "data".
use std::collections::HashMap;

use serde_json::Value;
use touchstone_model::{
    AbilitySlot, Class, Info, Item, Race, Shop, ShopSlot, Spell, TbInfo, World, WorldParts,
};
use touchstone_rules::Profile;
use touchstone_rules::detail::item::{ItemDetailEnv, ItemDetailOptions, pull_item_detail};
use touchstone_rules::lairs::LairInfo;
use touchstone_rules::locations::{LocationData, LocationLairs, LocationRow};
use touchstone_rules::pipeline::{CharacterState, refresh_all};
use touchstone_vb6::Currency;

use crate::fixture::{self, Names, Textblocks};
use crate::pipeline::Inputs;

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

struct Db {
    world: World,
    names: Names,
    textblocks: Textblocks,
    rooms: HashMap<(i32, i32), (String, i32)>,
    lairs: HashMap<String, Value>,
}

impl LocationData for Db {
    fn item(&self, n: i32) -> Option<&Item> {
        self.world.get_item(n)
    }
    fn shop(&self, n: i32) -> Option<&Shop> {
        self.world.get_shop(n)
    }
    fn monster_name(&self, n: i32) -> Option<String> {
        self.names.monsters.get(&n).cloned()
    }
    fn spell_name(&self, n: i32) -> Option<String> {
        self.names.spells.get(&n).cloned()
    }
    fn room(&self, map: i32, room: i32) -> Option<(String, i32)> {
        self.rooms.get(&(map, room)).cloned()
    }
}

struct Lairs<'a>(&'a HashMap<String, Value>);

impl LocationLairs for Lairs<'_> {
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

fn database(data: &Value) -> Db {
    let rows = |k: &str| data[k].as_array().unwrap().clone();
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
        spells: rows("spells").iter().map(spell).collect(),
        rooms: vec![],
        shops,
        classes,
        races,
        lairs: vec![],
        tbinfo: Vec::<TbInfo>::new(),
    })
    .unwrap();
    let mut names = Names::default();
    for r in &world.items {
        names.items.insert(r.number, r.name.clone());
    }
    for r in &world.spells {
        names
            .spells
            .insert(r.number, r.name.clone().unwrap_or_default());
    }
    for r in &world.classes {
        names.classes.insert(r.number, r.name.clone());
    }
    for r in rows("monster_names") {
        names.monsters.insert(i(&r[0]), s(&r[1]));
    }
    Db {
        names,
        textblocks: Textblocks::from_table(&data["textblocks"]),
        rooms: rows("rooms")
            .iter()
            .map(|r| ((i(&r[0]), i(&r[1])), (s(&r[2]), i(&r[3]))))
            .collect(),
        lairs: data["lairs"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        world,
    }
}

fn flag(inp: &Inputs, name: &str, idx: Option<usize>) -> bool {
    let v = match idx {
        Some(k) => inp.elem(name, k),
        None => inp.ctl(name),
    };
    v["Value"].as_i64() == Some(1)
}

fn text(inp: &Inputs, name: &str, idx: usize) -> String {
    inp.elem(name, idx)["Text"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

fn global<'a>(header: &'a Value, case: &'a Value, name: &str) -> &'a Value {
    let v = &case["inputs"]["globals"][name];
    if v.is_null() {
        &header["setup"]["globals"][name]
    } else {
        v
    }
}

fn options(header: &Value, case: &Value, inp: &Inputs) -> ItemDetailOptions {
    let out = &case["outputs"];
    let combo = inp.elem("cmbWeaponCombos", 1);
    let selected = combo["ListIndex"].as_i64().unwrap_or(-1);
    let mut equipped = [0; 20];
    if let Some(a) = global(header, case, "nEquippedItem").as_array() {
        for (k, e) in a.iter().enumerate() {
            equipped[k] = i(e);
        }
    }
    ItemDetailOptions {
        global_filter: flag(inp, "chkGlobalFilter", None),
        calc_combat: flag(inp, "chkWeaponOptions", Some(3)),
        slow_85: flag(inp, "chkWeaponOptions", Some(4)),
        combo_mode: (selected >= 0).then(|| i(&combo["ItemData"][selected as usize])),
        extras: [2, 3, 4].map(|k| text(inp, "txtWeaponExtras", k)),
        shops_first: inp.ctl("mnuShopsFirst")["Checked"] == true,
        equipped,
        nmr_ver: global(header, case, "nNMRVer").as_f64().unwrap(),
        hide_record_numbers: global(header, case, "bHideRecordNumbers") == true,
        quick_spell: global(header, case, "bQuickSpell") == true,
        global_level_text: s(&out["level_text"]),
        avg_level_max_all_stats: 0.0,
        dat_ver_caption: s(&header["setup"]["forms"]["frmMain"]["lblDatVer"]["Caption"]),
        accuracy_tag: s(&out["accuracy_tag"]),
        charm_tag: s(&out["charm_tag"]),
        strength_tag: s(&out["strength_tag"]),
        attack_type_mme: global(header, case, "nGlobalAttackTypeMME")
            .as_i64()
            .unwrap_or(0) as i32,
    }
}

/// A row as the oracle writes it: [Text, Tag, cells], a cell its Text or [Text, Tag, ForeColor].
fn row_json(r: &LocationRow) -> Value {
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

#[test]
fn item_detail_matches_the_oracle() {
    let fx = fixture::load("item_detail");
    let db = database(&fx.header["data"]);
    let mut fails = Vec::new();
    let mut checked = 0;
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        assert!(case.get("error").is_none(), "{id}: the oracle raised");
        let inp = Inputs::new(&fx.header, case);
        let steps = case["inputs"]["steps"].as_array().unwrap();
        let opts = options(&fx.header, case, &inp);
        let state: Option<CharacterState> = (steps[0]["call"] == "RefreshAll").then(|| {
            refresh_all(Profile::Stock, &db.world, &inp.character(), &inp.options())
                .unwrap_or_else(|e| panic!("{id}: refresh_all: {e}"))
        });
        let env = ItemDetailEnv {
            profile: Profile::Stock,
            world: &db.world,
            names: &db.names,
            textblocks: &db.textblocks,
            locations: &db,
            options: &opts,
        };
        for st in steps {
            let Some(spec) = st.get("rows") else {
                continue;
            };
            let name = st["as"].as_str().unwrap();
            let want = case["outputs"][format!("{name}.capture")]
                .as_array()
                .unwrap();
            let mode = i(&st["args"][2]);
            let full = st["args"][3] == true;
            for (key, want) in spec["keys"].as_array().unwrap().iter().zip(want) {
                let key = i(key);
                let mut lairs = Lairs(&db.lairs);
                let got = pull_item_detail(&env, &mut lairs, key, mode, full, state.as_ref());
                checked += 1;
                let rows: Vec<Value> = got.locations.rows.iter().map(row_json).collect();
                let events = want["events"].as_array().unwrap();
                let kind = |k: &'static str| events.iter().filter(move |e| e["kind"] == k);
                let want_sorts: Vec<usize> = kind("sort_listview")
                    .map(|e| e["rows"].as_u64().unwrap() as usize)
                    .collect();
                let want_refresh: Vec<(bool, usize)> = kind("lv_refresh_sort")
                    .map(|e| {
                        assert_eq!(
                            (e["column"].as_i64(), e["data_type"].as_i64()),
                            (Some(1), Some(1))
                        );
                        assert!(e["by_tag"] == true && e["ascending"] == false);
                        (
                            e["shops_first"] == true,
                            e["rows"].as_u64().unwrap() as usize,
                        )
                    })
                    .collect();
                let want_errors: Vec<i32> = kind("handled_error")
                    .filter(|e| e["source"] == "PullItemDetail")
                    .map(|e| i(&e["number"]))
                    .collect();
                let got_refresh: Vec<(bool, usize)> =
                    got.sort.iter().map(|x| (x.shops_first, x.rows)).collect();
                let ok = got.text == want["detail"].as_str().unwrap()
                    && Value::Array(rows.clone()) == want["rows"]
                    && got.locations.sorts == want_sorts
                    && got_refresh == want_refresh
                    && got.errors == want_errors;
                if !ok {
                    fails.push(format!(
                        "{id} {name} item {key}: text {:?} (want {:?}); rows {} (want {}); sorts {:?}/{:?}; refresh {:?}/{:?}; errors {:?}/{:?}\n  got rows {}\n  want rows {}",
                        got.text,
                        want["detail"],
                        rows.len(),
                        want["rows"].as_array().unwrap().len(),
                        got.locations.sorts,
                        want_sorts,
                        got_refresh,
                        want_refresh,
                        got.errors,
                        want_errors,
                        Value::Array(rows),
                        want["rows"],
                    ));
                }
            }
        }
    }
    assert!(
        fails.is_empty(),
        "{} of {checked} items differ; first:\n{}",
        fails.len(),
        fails.iter().take(6).cloned().collect::<Vec<_>>().join("\n")
    );
    assert!(checked > 900, "only {checked} items ran");
}
