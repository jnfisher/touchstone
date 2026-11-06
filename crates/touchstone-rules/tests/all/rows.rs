// Fixture tests for the list-row builders (AddWeapon2LV, AddArmour2LV, AddOtherItem2LV, AddSpell2LV, AddRace2LV,
// AddShop2LV, AddClass2LV, RaceColorCode, SetArmourACDRSortTags, ArmourACDRTagsAreByDR): see docs/porting.md step 5.
// Expected rows come from fixtures/rules/rows.json (modMain's own procedures run by the oracle into a recording ListView).
use std::collections::HashMap;

use serde_json::{Value, json};
use touchstone_model::{
    AbilitySlot, Class, Info, Item, Race, Shop, ShopSlot, Spell, TbInfo, World, WorldParts,
};
use touchstone_rules::Profile;
use touchstone_rules::attack::EquippedGlobals;
use touchstone_rules::character::{AttackTypeMud, CharacterProfile, PartyFilter};
use touchstone_rules::pipeline::{CharacterState, refresh_all};
use touchstone_rules::rows::{
    Row, RowEnv, SpellOptions, WeaponArgs, WeaponOptions, add_armour_row, add_class_row,
    add_other_item_row, add_race_row, add_shop_row, add_spell_row, add_weapon_row,
    armour_ac_dr_tags_are_by_dr, no_character_profile, party_profile, race_color_code,
    set_armour_ac_dr_sort_tags,
};
use touchstone_vb6 as vb;

use crate::fixture::{self, Names, Textblocks};
use crate::pipeline::Inputs;

pub(crate) fn i(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
}

pub(crate) fn s(v: &Value) -> String {
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

fn item(r: &Value) -> Item {
    // Number, Name, Limit, Encum, ItemType, Min, Max, ArmourClass, DamageResist, WeaponType, ArmourType, Worn, Accy,
    // StrReq, Speed, ClassRest, Abilities
    let mut class_rest = [0; 10];
    for (k, c) in r[15].as_array().unwrap().iter().enumerate() {
        class_rest[k] = i(c);
    }
    Item {
        number: i(&r[0]),
        name: s(&r[1]),
        limit: i(&r[2]),
        encum: i(&r[3]),
        item_type: i(&r[4]),
        use_count: 0,
        price: 0,
        currency: 0,
        min: i(&r[5]),
        max: i(&r[6]),
        armour_class: i(&r[7]),
        damage_resist: i(&r[8]),
        weapon_type: i(&r[9]),
        armour_type: i(&r[10]),
        worn: i(&r[11]),
        accy: i(&r[12]),
        gettable: true,
        str_req: i(&r[13]),
        speed: i(&r[14]),
        not_droppable: false,
        destroy_on_death: false,
        retain_after_uses: false,
        class_rest,
        race_rest: [0; 10],
        negate_spell: [0; 10],
        abilities: pairs(&r[16]),
        in_game: true,
        obtained_from: None,
        references: None,
    }
}

/// The other items only matter for their list row (and their name).
fn sundry(r: &Value) -> Item {
    Item {
        number: i(&r[0]),
        name: s(&r[1]),
        limit: i(&r[2]),
        encum: i(&r[3]),
        item_type: i(&r[4]),
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

pub(crate) fn spell(r: &Value) -> Spell {
    // Number, Name, Short, ReqLevel, EnergyCost, ManaCost, MinBase, MaxBase, Diff, TypeOfResists, Targets, Dur, AttType,
    // Magery, MageryLVL, Cap, MaxIncLVLs, MaxInc, MinIncLVLs, MinInc, DurIncLVLs, DurInc, Learnable, LearnedFrom, Abilities
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
        abilities: pairs(&r[24]),
        learnable: i(&r[22]) == 1,
        learned_from: (i(&r[23]) != 0).then(|| "x".to_string()),
        casted_by: None,
        classes: None,
    }
}

pub(crate) fn class(r: &Value) -> Class {
    Class {
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
    }
}

pub(crate) fn race(r: &Value) -> Race {
    // Number, Name, mSTR, mINT, mWIL, mAGL, mHEA, mCHM, xSTR, xINT, xWIL, xAGL, xHEA, xCHM, HPPerLVL, ExpTable, BaseCP
    Race {
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
    }
}

/// Everything the cases look records up in, from the fixture header's `data`.
struct Db {
    world: World,
    names: Names,
    textblocks: Textblocks,
}

fn database(data: &Value) -> Db {
    let rows = |k: &str| data[k].as_array().unwrap().clone();
    let mut items: Vec<Item> = rows("items").iter().map(item).collect();
    items.extend(rows("sundry").iter().map(sundry));
    items.sort_by_key(|r| r.number);
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
        classes: rows("classes").iter().map(class).collect(),
        races: rows("races").iter().map(race).collect(),
        lairs: vec![],
        tbinfo: Vec::<TbInfo>::new(),
    })
    .unwrap();
    // names: the database's own, plus the monsters the spell text can mention
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
        world,
        names,
        textblocks: Textblocks::from_table(&data["textblocks"]),
    }
}

// ---------------------------------------------------------------------------------------------
// the frmMain controls a case sets

fn flag(inp: &Inputs, name: &str, idx: Option<usize>) -> bool {
    let v = match idx {
        Some(k) => inp.elem(name, k),
        None => inp.ctl(name),
    };
    // a CheckBox holds 0 or 1, an OptionButton True or False
    v["Value"].as_i64() == Some(1) || v["Value"] == true
}

fn text(inp: &Inputs, name: &str, idx: usize) -> String {
    inp.elem(name, idx)["Text"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

fn weapon_options(inp: &Inputs) -> WeaponOptions {
    let combo = inp.elem("cmbWeaponCombos", 1);
    let selected = combo["ListIndex"].as_i64().unwrap_or(-1);
    WeaponOptions {
        global_filter: flag(inp, "chkGlobalFilter", None),
        calc_combat: flag(inp, "chkWeaponOptions", Some(3)),
        slow_85: flag(inp, "chkWeaponOptions", Some(4)),
        combo_mode: (selected >= 0).then(|| i(&combo["ItemData"][selected as usize])),
        extras: [2, 3, 4].map(|k| text(inp, "txtWeaponExtras", k)),
    }
}

fn spell_options(inp: &Inputs) -> SpellOptions {
    SpellOptions {
        global_filter: flag(inp, "chkGlobalFilter", None),
        global_level_text: text(inp, "txtGlobalLevel", 1),
        calc_combat: flag(inp, "chkSpellOptions", Some(0)),
        anti_magic: flag(inp, "chkSpellOptions", Some(2)),
        vs_mr_text: text(inp, "txtSpellOptions", 0),
    }
}

// ---------------------------------------------------------------------------------------------
// rows as the oracle writes them

fn row_json(r: &Row) -> Value {
    let cells: Vec<Value> = r
        .cells
        .iter()
        .map(|c| {
            if c.tag.is_none() && c.fore_color.is_none() {
                json!(c.text)
            } else {
                json!([c.text, c.tag, c.fore_color])
            }
        })
        .collect();
    json!([r.text, r.tag, cells])
}

fn rows_json(rows: &[Row]) -> Value {
    Value::Array(rows.iter().map(row_json).collect())
}

/// The profile fields a row depends on most, checked against what the oracle's PopulateCharacterProfile left.
fn check_profile(name: &str, got: &CharacterProfile, want: &Value) -> Result<(), String> {
    let checks: [(&str, f64, &Value); 8] = [
        ("nlevel", f64::from(got.level), &want["nlevel"]),
        ("naccuracy", got.accuracy, &want["naccuracy"]),
        ("nstr", f64::from(got.str_), &want["nstr"]),
        ("nagi", f64::from(got.agi), &want["nagi"]),
        ("ncrit", f64::from(got.crit), &want["ncrit"]),
        ("nstealth", f64::from(got.stealth), &want["nstealth"]),
        (
            "nplusbsaccy",
            f64::from(got.plus_bs_accy),
            &want["nplusbsaccy"],
        ),
        (
            "nspelldmgbonus",
            f64::from(got.spell_dmg_bonus),
            &want["nspelldmgbonus"],
        ),
    ];
    for (field, g, w) in checks {
        if (g - w.as_f64().unwrap_or(f64::NAN)).abs() > 1e-9 {
            return Err(format!("profile {name}.{field}: got {g}, want {w}"));
        }
    }
    if got.is_loaded_character != (want["bisloadedcharacter"] == true) {
        return Err(format!("profile {name}.bisloadedcharacter differs"));
    }
    Ok(())
}

fn run_case(header: &Value, case: &Value, db: &Db, fails: &mut Vec<String>) -> usize {
    let id = case["id"].as_str().unwrap();
    let inp = Inputs::new(header, case);
    let steps = case["inputs"]["steps"].as_array().unwrap();
    let out = &case["outputs"];
    assert!(
        case.get("error").is_none(),
        "{id}: the oracle raised {}",
        case["error"]
    );
    let dat_ver = header["setup"]["forms"]["frmMain"]["lblDatVer"]["Caption"]
        .as_str()
        .unwrap()
        .to_string();
    let global_filter = flag(&inp, "chkGlobalFilter", None);

    // a case that starts with RefreshAll has a loaded character
    let state: Option<CharacterState> = (steps[0]["call"] == "RefreshAll").then(|| {
        refresh_all(Profile::Stock, &db.world, &inp.character(), &inp.options())
            .unwrap_or_else(|e| panic!("{id}: refresh_all: {e}"))
    });
    let equipped = state
        .as_ref()
        .map_or_else(EquippedGlobals::default, CharacterState::equipped_globals);
    let accuracy_tag = out["accuracy_tag"].as_str().unwrap_or("").to_string();
    let level_text = text(&inp, "txtGlobalLevel", 0);
    let mut env = RowEnv::for_world(&db.world, Profile::Stock, &equipped);
    env.names = &db.names;
    env.textblocks = &db.textblocks;
    env.dat_ver_caption = &dat_ver;
    env.accuracy_tag = &accuracy_tag;
    env.global_level_text = global_filter.then_some(level_text.as_str());
    let weapon_opts = weapon_options(&inp);
    let spell_opts = spell_options(&inp);

    let mut profiles: HashMap<String, CharacterProfile> = HashMap::new();
    let mut checked = 0;
    for st in steps {
        let call = st["call"].as_str().unwrap();
        let name = st["as"].as_str().unwrap_or("");
        match (call, st.get("rows")) {
            ("RefreshAll", _) => {}
            ("PopulateCharacterProfile", _) => {
                let a = st["args"].as_array().unwrap();
                let mud = AttackTypeMud::from_code(i(&a[3])).unwrap();
                let (use_char, no_char) = (a[1] == true, a[5] == true);
                // the lair controls: a party of two or more (and the party not forced off) is not a loaded character
                let lair = |k: usize| text(&inp, "txtMonsterLairFilter", k);
                let party = PartyFilter {
                    lair_mode: flag(&inp, "optMonsterFilter", Some(1)),
                    lair_text: std::array::from_fn(lair),
                    monster_damage_text: inp.ctl("txtMonsterDamage")["Text"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                };
                let in_party =
                    a[2] != true && party.lair_mode && vb::val(&party.lair_text[0]) > 1.0;
                let profile = match &state {
                    _ if in_party && global_filter && !no_char => {
                        party_profile(Profile::Stock, &db.world, 1.83, 0.0, &party, mud)
                    }
                    Some(state) if !no_char && (use_char || global_filter) => {
                        state.profile_for(&db.world, mud, i(&a[4]))
                    }
                    _ => no_character_profile(Profile::Stock, &db.world, 1.83, 0.0, mud),
                };
                if let Err(e) = check_profile(name, &profile, &out[format!("{name}.refs")][0]) {
                    fails.push(format!("{id}: {e}"));
                }
                profiles.insert(name.to_string(), profile);
            }
            (_, Some(spec)) => {
                let keys: Vec<i32> = spec["keys"].as_array().unwrap().iter().map(i).collect();
                let args = st["args"].as_array().unwrap();
                let profile_name = args
                    .get(1)
                    .and_then(|a| a["saved"].as_str())
                    .map(str::to_string);
                let ch = profile_name.as_deref().map(|n| &profiles[n]);
                let shared = spec["lv"] == "shared";
                let mut rows: Vec<Row> = Vec::new();
                let mut got_each: Vec<Value> = Vec::new();
                let mut got_bless: Vec<Value> = Vec::new();
                for key in &keys {
                    let mut built: Option<Row> = None;
                    let mut bless = Value::Null;
                    match call {
                        "AddWeapon2LV" => {
                            let ch = CharacterState::attack_profile_of(ch.unwrap());
                            let w = WeaponArgs {
                                ability: i(&args[3]) as i16,
                                attack_type: i(&args[4]),
                                force_calc: args[6] == true,
                            };
                            // the surprise punch needs a record under the cursor but reads none of it
                            built = add_weapon_row(
                                &env,
                                db.world.get_item(*key),
                                &ch,
                                &weapon_opts,
                                &w,
                            );
                        }
                        "AddSpell2LV" => {
                            let sp = db.world.get_spell(*key).unwrap();
                            if let Some(r) =
                                add_spell_row(&env, sp, ch.unwrap(), &spell_opts, args[2] == true)
                            {
                                bless = r.bless_entry.map_or(Value::Null, |(t, d)| json!([t, d]));
                                built = Some(r.row);
                            }
                        }
                        "AddArmour2LV" => {
                            built = add_armour_row(
                                db.world.get_item(*key).unwrap(),
                                i(&args[2]) as i16,
                            );
                        }
                        "AddOtherItem2LV" => {
                            built = add_other_item_row(db.world.get_item(*key).unwrap())
                        }
                        "AddShop2LV" => built = add_shop_row(db.world.get_shop(*key).unwrap()),
                        "AddClass2LV" => {
                            built = add_class_row(&env, db.world.get_class(*key).unwrap())
                        }
                        "AddRace2LV" => {
                            built = add_race_row(&env, db.world.get_race(*key).unwrap())
                        }
                        other => panic!("{id}: unknown row procedure {other}"),
                    }
                    if shared {
                        rows.extend(built);
                    } else {
                        got_each.push(built.as_ref().map_or(Value::Null, row_json));
                    }
                    got_bless.push(bless);
                }
                let mut want_post = Vec::new();
                for p in st
                    .get("post")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let result = match p["call"].as_str().unwrap() {
                        "SetArmourACDRSortTags" => {
                            set_armour_ac_dr_sort_tags(&mut rows, p["args"][1] == true);
                            Value::Null
                        }
                        "ArmourACDRTagsAreByDR" => json!(armour_ac_dr_tags_are_by_dr(&rows)),
                        "RaceColorCode" => {
                            race_color_code(&db.world, &mut rows);
                            Value::Null
                        }
                        other => panic!("{id}: unknown pass {other}"),
                    };
                    want_post.push(json!({"result": result, "rows": rows_json(&rows)}));
                }
                // fixtures/rules/rows.json: `<as>` holds the rows, `<as>.post` the passes' results, `<as>.watch` the bless entries
                let got = if shared {
                    rows_json(&rows)
                } else {
                    Value::Array(got_each)
                };
                if got != out[name] {
                    let at = got
                        .as_array()
                        .unwrap()
                        .iter()
                        .zip(out[name].as_array().unwrap())
                        .position(|(g, w)| g != w);
                    fails.push(format!(
                        "{id} {name}: rows differ at {at:?} (key {:?}): got {}, want {}",
                        at.map(|k| keys.get(k)),
                        at.map_or_else(|| got.to_string(), |k| got[k].to_string()),
                        at.map_or_else(|| out[name].to_string(), |k| out[name][k].to_string()),
                    ));
                }
                if !want_post.is_empty()
                    && Value::Array(want_post.clone()) != out[format!("{name}.post")]
                {
                    fails.push(format!("{id} {name}.post differs"));
                }
                if spec.get("watch").is_some()
                    && Value::Array(got_bless) != out[format!("{name}.watch")]
                {
                    fails.push(format!("{id} {name}.watch (bless entries) differs"));
                }
                checked += keys.len();
            }
            other => panic!("{id}: unexpected step {other:?}"),
        }
    }
    checked
}

#[test]
fn rows_match_the_oracle() {
    let fx = fixture::load("rows");
    let db = database(&fx.header["data"]);
    let mut fails = Vec::new();
    let mut checked = 0;
    for case in &fx.cases {
        checked += run_case(&fx.header, case, &db, &mut fails);
    }
    assert!(
        fails.is_empty(),
        "{} of {} cases failed ({checked} records):\n{}",
        fails.len(),
        fx.cases.len(),
        fails
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(checked > 4000, "only {checked} records ran");
}

#[test]
fn skipped_records_add_no_row() {
    let world_row = |name: &str| {
        let mut it = sundry(&json!([1, name, 0, 0, 0]));
        it.name = name.to_string();
        it
    };
    // modMain AddOtherItem2LV and AddShop2LV skip an empty name; AddShop2LV also the sdf placeholders
    assert!(add_other_item_row(&world_row("")).is_none());
    assert!(add_other_item_row(&world_row("rock")).is_some());
    let shop = |name: &str| Shop {
        number: 1,
        name: name.to_string(),
        shop_type: 0,
        min_lvl: 0,
        max_lvl: 0,
        markup_pct: 0,
        class_rest: 0,
        stock: [ShopSlot::default(); 20],
        in_game: true,
        assigned_to: None,
    };
    for skipped in ["", "SDF test", "sdfx", "Leave this blank"] {
        assert!(add_shop_row(&shop(skipped)).is_none(), "{skipped:?}");
    }
    assert!(add_shop_row(&shop("Sdl shop")).is_some());
    let _ = vb::val("0");
}
