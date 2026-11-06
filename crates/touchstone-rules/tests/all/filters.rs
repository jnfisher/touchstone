// Fixture tests for the list filters (FilterWeapons, FilterArmour, FilterSundry, FilterInvenItems, FilterAll): see
// docs/porting.md step 5. Expected lists come from fixtures/rules/filters.json (frmMain's own procedures run by the
// oracle into recording ListViews); the spells, classes, races and names the weapon rows need are the rows fixture's data.
use serde_json::{Value, json};
use touchstone_model::{AbilitySlot, Info, Item, Shop, Spell, TbInfo, World, WorldParts};
use touchstone_rules::Profile;
use touchstone_rules::attack::EquippedGlobals;
use touchstone_rules::character::{AttackTypeMud, CharacterProfile};
use touchstone_rules::filters::{
    AbilityFilter, Answer, ArmourFilter, Dialog, FilterEnv, InvenLists, ListFilter, ProfileRequest,
    WeaponEntry, WeaponFilter, filter_all, filter_armour, filter_inven_items, filter_sundry,
    filter_weapons,
};
use touchstone_rules::gear::Character;
use touchstone_rules::pipeline::{CharacterState, refresh_all};
use touchstone_rules::rows::{
    Row, RowEnv, WeaponArgs, WeaponOptions, add_armour_row, add_other_item_row, add_weapon_row,
    no_character_profile,
};
use touchstone_vb6 as vb;

use crate::fixture::{self, Names, Textblocks};
use crate::pipeline::Inputs;
use crate::rows::{class, i, race, s, spell};

fn item(r: &Value) -> Item {
    // Number, Name, Limit, ItemType, WeaponType, ArmourType, Worn, StrReq, Speed, In Game, ClassRest, NegateSpell,
    // Abilities, WeaponExtra [Encum, Min, Max, ArmourClass, DamageResist, Accy]
    let mut class_rest = [0; 10];
    for (k, c) in r[10].as_array().unwrap().iter().enumerate() {
        class_rest[k] = i(c);
    }
    let mut negate_spell = [0; 10];
    for (k, c) in r[11].as_array().unwrap().iter().enumerate() {
        negate_spell[k] = i(c);
    }
    let mut abilities = [AbilitySlot::default(); 20];
    for (k, p) in r[12].as_array().unwrap().iter().enumerate() {
        abilities[k] = AbilitySlot {
            ability: i(&p[0]),
            value: i(&p[1]),
        };
    }
    let extra = |k: usize| r[13].as_array().map_or(0, |e| i(&e[k]));
    Item {
        number: i(&r[0]),
        name: s(&r[1]),
        limit: i(&r[2]),
        encum: extra(0),
        item_type: i(&r[3]),
        use_count: 0,
        price: 0,
        currency: 0,
        min: extra(1),
        max: extra(2),
        armour_class: extra(3),
        damage_resist: extra(4),
        weapon_type: i(&r[4]),
        armour_type: i(&r[5]),
        worn: i(&r[6]),
        accy: extra(5),
        gettable: true,
        str_req: i(&r[7]),
        speed: i(&r[8]),
        not_droppable: false,
        destroy_on_death: false,
        retain_after_uses: false,
        class_rest,
        race_rest: [0; 10],
        negate_spell,
        abilities,
        in_game: i(&r[9]) != 0,
        obtained_from: None,
        references: None,
    }
}

struct Db {
    world: World,
    names: Names,
    textblocks: Textblocks,
}

fn database(own: &Value, rows_data: &Value) -> Db {
    let rows = |k: &str| rows_data[k].as_array().unwrap().clone();
    let mut items: Vec<Item> = own["items"].as_array().unwrap().iter().map(item).collect();
    items.sort_by_key(|r| r.number);
    let world = World::new(WorldParts {
        info: Info {
            nmr_version: String::new(),
            dat_file_version: String::new(),
            content_version: None,
            date: String::new(),
            time: String::new(),
            custom: String::new(),
            legit: false,
            disable_kai: false,
            update_url: String::new(),
        },
        items,
        monsters: vec![],
        spells: rows("spells").iter().map(spell).collect::<Vec<Spell>>(),
        rooms: vec![],
        shops: Vec::<Shop>::new(),
        classes: rows("classes").iter().map(class).collect(),
        races: rows("races").iter().map(race).collect(),
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
        world,
        names,
        textblocks: Textblocks::from_table(&rows_data["textblocks"]),
    }
}

// ---------------------------------------------------------------------------------------------
// the frmMain controls a case sets

fn flag(v: &Value) -> bool {
    v["Value"].as_i64() == Some(1)
}

fn text(v: &Value) -> String {
    v["Text"].as_str().unwrap_or("").to_string()
}

/// `combo.ItemData(combo.ListIndex)`; the scenario writes ItemData as a list or as an object keyed by index.
fn selected_data(v: &Value) -> Option<i32> {
    let k = v["ListIndex"].as_i64()?;
    if k < 0 {
        return None;
    }
    let d = &v["ItemData"];
    let x = if d.is_array() {
        &d[k as usize]
    } else {
        &d[k.to_string()]
    };
    Some(x.as_i64().unwrap_or(0) as i32)
}

fn ability_filter(inp: &Inputs, list: &str, op: &str, val: &str) -> AbilityFilter {
    AbilityFilter {
        selected: selected_data(inp.ctl(list)).unwrap_or(0),
        at_most: inp.ctl(op)["ListIndex"].as_i64() == Some(0),
        value_text: text(inp.ctl(val)),
    }
}

fn list_filter(inp: &Inputs) -> ListFilter {
    let mut w = WeaponFilter::default();
    for k in 0..4 {
        w.handed[k] = flag(inp.elem("chkHanded", k));
    }
    w.non_magic = flag(inp.elem("chkWeaponOptions", 0));
    w.backstab_only = flag(inp.elem("chkWeaponOptions", 1));
    w.include_limited = flag(inp.elem("chkWeaponOptions", 2));
    w.calc_combat = flag(inp.elem("chkWeaponOptions", 3));
    w.slow_85 = flag(inp.elem("chkWeaponOptions", 4));
    w.max_speed_text = text(inp.ctl("txtWeaponSpeed"));
    w.max_str_text = text(inp.elem("txtWeaponExtras", 0));
    w.min_dmg_text = text(inp.elem("txtWeaponExtras", 1));
    for k in 0..3 {
        w.vs[k] = text(inp.elem("txtWeaponExtras", k + 2));
    }
    w.magic_level = inp.elem("cmbWeaponCombos", 0)["ListIndex"]
        .as_i64()
        .unwrap() as i32;
    w.combo = selected_data(inp.elem("cmbWeaponCombos", 1));
    w.ability = ability_filter(
        inp,
        "cmbWeaponAbilityList",
        "cmbWeaponAbilityOp",
        "txtWeaponAbilityVal",
    );
    let mut a = ArmourFilter::default();
    for k in 0..7 {
        a.types[k] = flag(inp.elem("chkArmourType", k));
    }
    a.no_limit = flag(inp.ctl("chkArmourNoLimit"));
    a.non_magic = flag(inp.ctl("chkArmourNonMagic"));
    a.worn = selected_data(inp.ctl("cmbArmourWorn")).unwrap_or(0);
    a.ability = ability_filter(
        inp,
        "cmbArmorAbilityList",
        "cmbArmorAbilityOp",
        "txtArmorAbilityVal",
    );
    ListFilter {
        weapons: w,
        armour: a,
        sundry: ability_filter(
            inp,
            "cmbSundryAbilityList",
            "cmbSundryAbilityOp",
            "txtSundryAbilityVal",
        ),
    }
}

fn who(inp: &Inputs) -> Character {
    let class = &inp.elem("cmbGlobalClass", 0);
    let k = class["ListIndex"].as_i64().unwrap_or(-1);
    Character {
        level: vb::val(&text(inp.elem("txtGlobalLevel", 0))) as i32,
        class: if k >= 1 {
            selected_data(class).unwrap_or(0)
        } else {
            0
        },
        race: {
            let r = inp.elem("cmbGlobalRace", 0);
            if r["ListIndex"].as_i64().unwrap_or(-1) >= 1 {
                selected_data(r).unwrap_or(0)
            } else {
                0
            }
        },
        alignment: inp.ctl("cmbGlobalAlignment")["ListIndex"]
            .as_i64()
            .unwrap_or(0) as i32,
        min_item_level: vb::val(&text(inp.ctl("txtGlobalMinLVL"))) as i32,
    }
}

// ---------------------------------------------------------------------------------------------
// lists as the oracle writes them

fn tagged(r: &Row) -> String {
    match &r.tag {
        None => r.text.clone(),
        Some(t) => format!("{}:{t}", r.text),
    }
}

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

fn weapon_options(f: &WeaponFilter, global: bool) -> WeaponOptions {
    WeaponOptions {
        global_filter: global,
        calc_combat: f.calc_combat,
        slow_85: f.slow_85,
        combo_mode: f.combo,
        extras: f.vs.clone(),
    }
}

#[derive(Default)]
struct Got {
    weapons: Option<(Vec<String>, Vec<Value>, &'static str)>,
    armour: Option<(Vec<String>, &'static str)>,
    sundry: Option<(Vec<String>, &'static str)>,
    inven: Option<InvenLists>,
    inven_caption: &'static str,
    prompt_save: bool,
}

fn run_case(header: &Value, case: &Value, db: &Db, fails: &mut Vec<String>) -> usize {
    let id = case["id"].as_str().unwrap();
    assert!(
        case.get("error").is_none(),
        "{id}: the oracle raised {}",
        case["error"]
    );
    let inp = Inputs::new(header, case);
    let steps = case["inputs"]["steps"].as_array().unwrap();
    let out = &case["outputs"];
    let global = flag(inp.ctl("chkGlobalFilter"));
    let only_in_game = inp.gb("bOnlyInGame");
    let state: Option<CharacterState> = (steps[0]["call"] == "RefreshAll").then(|| {
        refresh_all(Profile::Stock, &db.world, &inp.character(), &inp.options())
            .unwrap_or_else(|e| panic!("{id}: refresh_all: {e}"))
    });
    let mut equipped = [0; 20];
    if let Some(a) = inp.g("nEquippedItem").as_array() {
        for (k, e) in a.iter().enumerate() {
            equipped[k] = i(e);
        }
    }
    let martial_skill = state.as_ref().map_or([0.0; 3], |st| {
        std::array::from_fn(|k| st.stats.stat_tag[37 + k].unwrap_or(0.0))
    });
    let env = FilterEnv {
        profile: Profile::Stock,
        world: &db.world,
        only_in_game,
        global_filter: global,
        who: who(&inp),
        equipped,
        martial_skill,
    };
    let globals_equipped = state
        .as_ref()
        .map_or_else(EquippedGlobals::default, CharacterState::equipped_globals);
    let accuracy_tag = state
        .as_ref()
        .and_then(|st| st.stats.stat_tag[10])
        .map_or_else(String::new, vb::cstr);
    if let Some(want) = out["accuracy_tag"].as_str()
        && want != accuracy_tag
    {
        fails.push(format!(
            "{id}: accuracy tag {accuracy_tag:?}, want {want:?}"
        ));
    }
    let dat_ver = header["setup"]["forms"]["frmMain"]["lblDatVer"]["Caption"]
        .as_str()
        .unwrap_or("v1.11p")
        .to_string();
    let level_text = text(inp.elem("txtGlobalLevel", 1));
    let mut rows = RowEnv::for_world(&db.world, Profile::Stock, &globals_equipped);
    rows.names = &db.names;
    rows.textblocks = &db.textblocks;
    rows.dat_ver_caption = &dat_ver;
    rows.accuracy_tag = &accuracy_tag;
    rows.global_level_text = global.then_some(level_text.as_str());

    let mut filter = list_filter(&inp);
    // RefreshAll's SetupClass ticks the non-magic boxes of an anti-magic class (a UI refresh, not list semantics): the
    // fixture reads the boxes back
    if let Some(v) = out["nonmagic_w"].as_i64() {
        filter.weapons.non_magic = v == 1;
    }
    if let Some(v) = out["nonmagic_a"].as_i64() {
        filter.armour.non_magic = v == 1;
    }
    // PopulateCharacterProfile also looks at chkGlobalFilter itself, so a loaded character is used whenever it is on
    let profile_for = |req: ProfileRequest| -> CharacterProfile {
        let mud = AttackTypeMud::from_code(req.mud).unwrap_or(AttackTypeMud::Normal);
        match &state {
            Some(st) if req.use_character || global => st.profile_for(&db.world, mud, req.weapon),
            _ => no_character_profile(Profile::Stock, &db.world, 1.83, 0.0, mud),
        }
    };
    let mut answers: Vec<i64> = case["inputs"]["msgbox"]
        .as_array()
        .map(|a| a.iter().map(|x| x.as_i64().unwrap()).collect())
        .unwrap_or_default();
    answers.reverse();
    let mut asked = Vec::new();
    let mut ask = |d: Dialog| -> Answer {
        asked.push(d);
        match answers.pop() {
            Some(7) => Answer::No,
            Some(2) => Answer::Cancel,
            _ => Answer::Yes,
        }
    };

    let weapon_rows = |list: &touchstone_rules::filters::WeaponList| -> (Vec<String>, Vec<Value>) {
        let wopts = weapon_options(&filter.weapons, global);
        let mut names = Vec::new();
        let mut full = Vec::new();
        for e in &list.entries {
            let row = match e {
                WeaponEntry::Built(r) => Some(r.clone()),
                WeaponEntry::Add(a) => {
                    let ch = CharacterState::attack_profile_of(&profile_for(a.profile));
                    let args = WeaponArgs {
                        ability: a.ability,
                        attack_type: a.attack_type,
                        force_calc: a.force_calc,
                    };
                    let it = a.item.and_then(|n| db.world.get_item(n));
                    add_weapon_row(&rows, it, &ch, &wopts, &args)
                }
            };
            if let Some(r) = row {
                names.push(tagged(&r));
                full.push(row_json(&r));
            }
        }
        (names, full)
    };
    let caption =
        |filtered: bool, on: &'static str, off: &'static str| if filtered { on } else { off };

    let mut got = Got::default();
    for st in steps {
        let args = st["args"].as_array().cloned().unwrap_or_default();
        let flag0 = args.first().is_some_and(|a| a == true);
        match st["call"].as_str().unwrap() {
            "RefreshAll" => {}
            "FilterWeapons" => {
                let list =
                    filter_weapons(&env, &rows, &filter.weapons, flag0, &profile_for, &mut ask);
                if !list.cancelled {
                    let (names, full) = weapon_rows(&list);
                    got.weapons =
                        Some((names, full, caption(list.filtered, "*Weapons*", "Weapons")));
                }
            }
            "FilterArmour" => {
                let list = filter_armour(&env, &filter.armour, flag0);
                let names = list
                    .entries
                    .iter()
                    .filter_map(|a| add_armour_row(db.world.get_item(a.item).unwrap(), a.ability))
                    .map(|r| tagged(&r))
                    .collect();
                got.armour = Some((names, caption(list.filtered, "*Armour*", "Armour")));
            }
            "FilterSundry" => {
                let list = filter_sundry(&env, &filter.sundry);
                let names = list
                    .entries
                    .iter()
                    .filter_map(|n| add_other_item_row(db.world.get_item(*n).unwrap()))
                    .map(|r| tagged(&r))
                    .collect();
                got.sundry = Some((names, caption(list.filtered, "*Sundry*", "Sundry")));
            }
            "FilterInvenItems" => {
                if let Some(l) = filter_inven_items(&env, flag0) {
                    got.prompt_save |= l.equipped_removed;
                    got.inven_caption = caption(l.filtered, "*EQ*", "EQ");
                    got.inven = Some(l);
                }
            }
            "FilterAll" => {
                let all = filter_all(&env, &rows, flag0, &profile_for, &mut ask);
                if !all.weapons.cancelled {
                    let (names, full) = weapon_rows(&all.weapons);
                    got.weapons = Some((
                        names,
                        full,
                        caption(all.weapons.filtered, "*Weapons*", "Weapons"),
                    ));
                }
                let names = all
                    .armour
                    .entries
                    .iter()
                    .filter_map(|a| add_armour_row(db.world.get_item(a.item).unwrap(), a.ability))
                    .map(|r| tagged(&r))
                    .collect();
                got.armour = Some((names, caption(all.armour.filtered, "*Armour*", "Armour")));
                if let Some(l) = all.inven {
                    got.prompt_save |= l.equipped_removed;
                    got.inven_caption = caption(l.filtered, "*EQ*", "EQ");
                    got.inven = Some(l);
                }
            }
            other => panic!("{id}: unknown step {other}"),
        }
    }

    // MME lists records in the table's physical order, which the loaded world does not keep (it is sorted by number):
    // lists are compared in record-number order, a record's own rows (attack kinds) in the order they were added
    let sorted = |v: &Value| -> Value {
        let mut items: Vec<Value> = v.as_array().cloned().unwrap_or_default();
        let key = |x: &Value| -> i64 {
            let t = match x {
                Value::String(t) => t.as_str(),
                Value::Array(a) => a[0].as_str().unwrap_or(""),
                _ => "",
            };
            t.split(':').next().unwrap().parse().unwrap_or(0)
        };
        items.sort_by_key(|x| key(x));
        Value::Array(items)
    };
    let mut check = |name: &str, g: Value| {
        let (g, want) = if name == "equip" {
            let norm = |v: &Value| -> Value {
                Value::Array(
                    v.as_array()
                        .unwrap()
                        .iter()
                        .map(|slot| {
                            let mut n: Vec<Value> = slot.as_array().unwrap().clone();
                            n.sort_by_key(|x| x.as_i64().unwrap());
                            Value::Array(n)
                        })
                        .collect(),
                )
            };
            (norm(&g), norm(&out[name]))
        } else if g.is_array() && out[name].is_array() && name != "nav" {
            (sorted(&g), sorted(&out[name]))
        } else {
            (g, out[name].clone())
        };
        if out.get(name).is_some() && want != g {
            let (a, b) = (g.as_array(), want.as_array());
            let at = match (a, b) {
                (Some(a), Some(b)) => a
                    .iter()
                    .zip(b)
                    .position(|(x, y)| x != y)
                    .or_else(|| (a.len() != b.len()).then_some(a.len().min(b.len()))),
                _ => None,
            };
            fails.push(format!(
                "{id} {name}: differs (got {} entries, want {}; first difference at {at:?}: got {:?}, want {:?})",
                a.map_or(0, Vec::len),
                b.map_or(0, Vec::len),
                at.and_then(|k| a.and_then(|a| a.get(k))),
                at.and_then(|k| b.and_then(|b| b.get(k))),
            ));
        }
    };
    let mut checked = 0;
    if out.get("weapons").is_some() {
        let (names, full, nav) = got.weapons.clone().unwrap_or_default();
        checked += names.len();
        check("weapons", json!(names));
        check("nav", json!(nav));
        if out.get("full").is_some() {
            check("full", Value::Array(full));
        }
    }
    if out.get("armour").is_some() {
        let (names, nav) = got.armour.clone().unwrap_or_default();
        checked += names.len();
        check("armour", json!(names));
        check("nav_armour", json!(nav));
    }
    if out.get("sundry").is_some() {
        let (names, nav) = got.sundry.clone().unwrap_or_default();
        checked += names.len();
        check("sundry", json!(names));
        check("nav_sundry", json!(nav));
    }
    if out.get("equip").is_some() {
        // entry 0 of every combo is "(none)"; a filter that did nothing leaves the combos as the scenario set them
        let slots: Vec<Value> = match &got.inven {
            Some(l) => l
                .slots
                .iter()
                .map(|s| {
                    let mut v = vec![0];
                    v.extend(s);
                    json!(v)
                })
                .collect(),
            None => (0..20).map(|_| json!([])).collect(),
        };
        checked += 1;
        check("equip", Value::Array(slots));
        check("nav_eq", json!(got.inven_caption));
        check("prompt_save", json!(got.prompt_save));
    }
    checked
}

#[test]
fn filters_match_the_oracle() {
    let fx = fixture::load("filters");
    let rows_fx = fixture::load("rows");
    let db = database(&fx.header["data"], &rows_fx.header["data"]);
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
    assert!(checked > 30_000, "only {checked} records ran");
}

#[test]
fn defaults_are_resetfilteroptions() {
    // frmMain ResetFilterOptions: every check box on, the speed box 999999, strength 999, damage 0, ability "Any" with "at least 1"
    let f = ListFilter::default();
    assert_eq!(f.weapons.max_speed_text, "999999");
    assert_eq!(f.weapons.max_str_text, "999");
    assert_eq!(f.weapons.handed, [true; 4]);
    assert!(f.weapons.include_limited && !f.weapons.calc_combat);
    assert_eq!(f.armour.types, [true; 7]);
    assert_eq!(f.sundry.value_text, "1");
    assert!(!f.sundry.at_most);
}
