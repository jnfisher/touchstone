// Fixture tests for the paste_char family: see docs/porting.md step 5.
// Expected values come from fixtures/rules/paste_char.json (frmMain PasteCharacter and friends, and
// modMMudFunc TestPasteChar / TestAlphaChar, run by the oracle over the stock database plus a few
// invented rows).

use crate::fixture;
use serde_json::Value;
use std::collections::VecDeque;
use touchstone_model::{
    AbilitySlot, Class, Info, Item, Race, Room, Shop, ShopSlot, Spell, World, WorldParts,
};
use touchstone_rules::character::Character;
use touchstone_rules::game_text::{ListRow, SubItem, SubTag};
use touchstone_rules::gear::EQUIP_SLOTS;
use touchstone_rules::paste_char::{
    Answers, PasteEnv, PasteEvent, PasteOutcome, PasteState, apply_pasted_stat, paste_character,
    paste_inventory_manager, paste_spells, pasted_stat_is_buffed, test_alpha_char, test_paste_char,
};
use touchstone_vb6 as vb;

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

fn world(data: &Value) -> World {
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

/// frmMain's controls as a case sees them: its own over the family-wide setup's, element by element
/// for `$array+`.
struct Forms<'a> {
    setup: &'a Value,
    case: &'a Value,
    globals: &'a Value,
}

impl Forms<'_> {
    fn ctl(&self, name: &str) -> &Value {
        let v = &self.case[name];
        if v.is_null() { &self.setup[name] } else { v }
    }

    /// One element of a control array, as a property lookup that falls back to the setup's element.
    fn prop(&self, name: &str, idx: usize, prop: &str) -> Value {
        let k = idx.to_string();
        let c = &self.case[name];
        let over = if c["$array+"].is_object() {
            &c["$array+"][&k][prop]
        } else if c["$array"].is_object() {
            return c["$array"][&k][prop].clone();
        } else {
            &Value::Null
        };
        if !over.is_null() {
            return over.clone();
        }
        self.setup[name]["$array"][&k][prop].clone()
    }

    fn text(&self, name: &str) -> String {
        self.ctl(name)["Text"].as_str().unwrap_or("").to_string()
    }

    fn global_flag(&self, name: &str) -> bool {
        self.globals[name].as_bool().unwrap_or(false)
    }
}

fn list_items(v: &Value) -> Vec<i32> {
    v.as_array()
        .map(|a| a.iter().map(i).collect())
        .unwrap_or_default()
}

fn rows_from_listview(v: &Value) -> Vec<ListRow> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|r| ListRow {
            text: r["Text"].as_str().unwrap_or("").to_string(),
            subs: r["Sub"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|x| SubItem::new(x.as_str().unwrap()))
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect()
}

fn dump_rows(rows: &[ListRow]) -> Value {
    Value::Array(
        rows.iter()
            .map(|r| {
                let subs: Vec<Value> = r
                    .subs
                    .iter()
                    .map(|s| {
                        let mut m = serde_json::Map::new();
                        m.insert("t".into(), Value::from(s.text.clone()));
                        match &s.tag {
                            None => {}
                            Some(SubTag::Int(n)) => {
                                m.insert("g".into(), Value::from(*n));
                            }
                            Some(SubTag::Text(t)) => {
                                m.insert("g".into(), Value::from(t.clone()));
                            }
                        }
                        Value::Object(m)
                    })
                    .collect();
                serde_json::json!({"text": r.text, "subs": subs})
            })
            .collect(),
    )
}

/// The Char tab a case starts from.
fn state(world: &World, forms: &Forms) -> PasteState {
    let classes: Vec<i32> = std::iter::once(0)
        .chain(world.classes.iter().map(|c| c.number))
        .collect();
    let races: Vec<i32> = std::iter::once(0)
        .chain(world.races.iter().map(|r| r.number))
        .collect();
    let class_ix = forms
        .prop("cmbGlobalClass", 0, "ListIndex")
        .as_i64()
        .unwrap_or(-1);
    let race_ix = forms
        .prop("cmbGlobalRace", 0, "ListIndex")
        .as_i64()
        .unwrap_or(-1);
    let mut stat_text: [String; 6] = Default::default();
    for (k, t) in stat_text.iter_mut().enumerate() {
        *t = forms
            .prop("txtCharStats", k, "Text")
            .as_str()
            .unwrap_or("")
            .to_string();
    }
    let mut character = Character {
        name: forms.text("txtCharName"),
        class: usize::try_from(class_ix).ok().map(|k| classes[k]),
        race: usize::try_from(race_ix).ok().map(|k| races[k]),
        level: vb::val(
            forms
                .prop("txtGlobalLevel", 0, "Text")
                .as_str()
                .unwrap_or(""),
        ) as i32,
        global_filter: forms.ctl("chkGlobalFilter")["Value"].as_i64().unwrap_or(0) != 0,
        add_weight: vb::val(&forms.text("txtInvenAddWeight")) as i32,
        use_add_weight: forms.ctl("chkInvenAddWeight")["Value"]
            .as_i64()
            .unwrap_or(0)
            != 0,
        ..Character::default()
    };
    if let Some(a) = forms.globals["nEquippedItem"].as_array() {
        for (k, v) in a.iter().enumerate() {
            character.equipped[k] = i(v);
        }
    }
    if let Some(a) = forms.globals["nLearnedSpells"].as_array() {
        for (k, v) in a.iter().enumerate() {
            character.learned.spells[k] = i(v);
        }
    }
    character.learned.class = forms.globals["nLearnedSpellClass"].as_i64().unwrap_or(0) as i16;
    let mut combos = vec![Vec::new(); EQUIP_SLOTS];
    let mut sel = [0usize; EQUIP_SLOTS];
    let mut hold = [false; EQUIP_SLOTS];
    for x in 0..EQUIP_SLOTS {
        let items = list_items(&forms.prop("cmbEquip", x, "Items"));
        combos[x] = items.iter().skip(1).copied().collect();
        sel[x] = forms.prop("cmbEquip", x, "ListIndex").as_u64().unwrap_or(0) as usize;
        hold[x] = forms.prop("chkEquipHold", x, "Value").as_i64().unwrap_or(0) != 0;
    }
    PasteState {
        character,
        stat_text,
        charm_tag: forms
            .prop("txtCharStats", 5, "Tag")
            .as_str()
            .unwrap_or("")
            .to_string(),
        charm_bonus: vb::val(forms.prop("lblLabelArray", 3, "Tag").as_str().unwrap_or("")),
        combos,
        sel,
        hold,
        unequip_missing: forms.ctl("chkUnequipMissing")["Value"]
            .as_i64()
            .unwrap_or(0)
            != 0,
        rows: rows_from_listview(&forms.ctl("lvItemManager")["$listview"]),
        char_loaded: forms.global_flag("bCharLoaded"),
        prompt_save: forms.global_flag("bPromptSave"),
        session_char_file: forms.globals["sSessionLastCharFile"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        refresh_delay: false,
    }
}

fn answers(case: &Value) -> Answers {
    Answers {
        msgbox: case["inputs"]["msgbox"]
            .as_array()
            .map(|a| a.iter().map(i).collect::<VecDeque<_>>())
            .unwrap_or_default(),
        inputbox: case["inputs"]["inputbox"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|s| s.as_str().unwrap().to_string())
                    .collect::<VecDeque<_>>()
            })
            .unwrap_or_default(),
    }
}

/// The oracle's FileExists: a name ending .mmec that does not say "missing".
fn file_exists(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".mmec") && !n.contains("missing")
}

/// The events the port models, in the oracle's shape.
fn want_events(case: &Value) -> Vec<Value> {
    case["events"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|e| {
                    e["kind"] != "handled_error"
                        || matches!(
                            e["source"].as_str().unwrap(),
                            "PasteCharacter"
                                | ""
                                | "LV_AddRowByItemNumber"
                                | "GetBestShopNumForItem"
                        )
                })
                .map(|e| {
                    let mut e = e.clone();
                    e.as_object_mut().unwrap().remove("description");
                    e
                })
                .collect()
        })
        .unwrap_or_default()
}

fn got_events(out: &PasteOutcome) -> Vec<Value> {
    out.events
        .iter()
        .filter_map(|e| match e {
            PasteEvent::MsgBox(t) => Some(serde_json::json!({"kind": "msgbox", "text": t})),
            PasteEvent::InputBox { prompt, default } => {
                Some(serde_json::json!({"kind": "inputbox", "text": prompt, "default": default}))
            }
            PasteEvent::Save { prompt, file } => {
                Some(serde_json::json!({"kind": "save", "prompt": prompt, "file": file}))
            }
            PasteEvent::ClearCharacter => None,
            PasteEvent::HandledError { source, number } => Some(
                serde_json::json!({"kind": "handled_error", "source": source, "number": number}),
            ),
        })
        .collect()
}

fn same<T: PartialEq + std::fmt::Debug>(what: &str, got: T, want: T) -> Result<(), String> {
    if got == want {
        Ok(())
    } else {
        Err(format!("{what}: got {got:?}, want {want:?}"))
    }
}

fn check_state(
    world: &World,
    st: &PasteState,
    out: &PasteOutcome,
    want: &Value,
    case: &Value,
) -> Result<(), String> {
    let classes: Vec<i32> = std::iter::once(0)
        .chain(world.classes.iter().map(|c| c.number))
        .collect();
    let races: Vec<i32> = std::iter::once(0)
        .chain(world.races.iter().map(|r| r.number))
        .collect();
    let ch = &st.character;
    let o = |k: &str| &want[k];
    if !o("eq").is_null() {
        same(
            "equipped",
            ch.equipped.to_vec(),
            o("eq").as_array().unwrap().iter().map(i).collect(),
        )?;
    }
    if !o("learned").is_null() {
        let mut got = ch.learned.spells.to_vec();
        while got.last() == Some(&0) {
            got.pop();
        }
        same(
            "learned",
            got,
            o("learned").as_array().unwrap().iter().map(i).collect(),
        )?;
        same("learned class", i32::from(ch.learned.class), i(o("lclass")))?;
    }
    if !o("stats").is_null() {
        let want_stats: Vec<String> = o("stats")
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect();
        same("stat text", st.stat_text.to_vec(), want_stats)?;
        same(
            "charm tag",
            st.charm_tag.as_str(),
            o("tags")[5].as_str().unwrap(),
        )?;
    }
    if !o("lvl").is_null() {
        same(
            "level",
            ch.level.to_string(),
            o("lvl").as_str().unwrap().to_string(),
        )?;
        same("filter", i32::from(ch.global_filter), i(o("filter")))?;
        same("name", ch.name.as_str(), o("name").as_str().unwrap())?;
        same(
            "class",
            ch.class
                .and_then(|c| classes.iter().position(|&n| n == c))
                .map(|p| p as i32),
            Some(i(o("cls"))),
        )?;
        same(
            "race",
            ch.race
                .and_then(|c| races.iter().position(|&n| n == c))
                .map(|p| p as i32),
            Some(i(o("race"))),
        )?;
        same(
            "add weight",
            ch.add_weight.to_string(),
            o("addw").as_str().unwrap().to_string(),
        )?;
        same("add weight on", i32::from(ch.use_add_weight), i(o("addc")))?;
        same(
            "session file",
            st.session_char_file.as_str(),
            o("session").as_str().unwrap(),
        )?;
        same("loaded", st.char_loaded, o("loaded").as_bool().unwrap())?;
        same(
            "refresh delay",
            i32::from(st.refresh_delay),
            i32::from(i(o("delay")) != 0),
        )?;
        same(
            "sim ac",
            out.sim_ac.map(|n| n.to_string()).unwrap_or_default(),
            o("simac").as_str().unwrap().to_string(),
        )?;
        same(
            "sim dr",
            out.sim_dr.map(|n| n.to_string()).unwrap_or_default(),
            o("simdr").as_str().unwrap().to_string(),
        )?;
    }
    if !o("combos").is_null() {
        let mut got = serde_json::Map::new();
        for x in 0..EQUIP_SLOTS {
            if !st.combos[x].is_empty() || st.sel[x] != 0 {
                let mut data = vec![0];
                data.extend(st.combos[x].iter().copied());
                got.insert(
                    x.to_string(),
                    serde_json::json!({"n": data.len(), "sel": st.sel[x], "data": data}),
                );
            }
        }
        same("combos", Value::Object(got), o("combos").clone())?;
    }
    if !o("rows").is_null() {
        same("rows", dump_rows(&st.rows), o("rows").clone())?;
    }
    same("events", got_events(out), want_events(case))
}

fn run(world: &World, header: &Value, case: &Value) -> Result<(), String> {
    let inputs = &case["inputs"];
    let out_v = &case["outputs"];
    if let Some(p) = inputs["proc"].as_str() {
        let a = inputs["args"].as_array().unwrap();
        let ch = a[0].as_str().unwrap().chars().next().unwrap_or('\0');
        return match p {
            "TestPasteChar" => same(
                "result",
                test_paste_char(ch),
                out_v["result"].as_bool().unwrap(),
            ),
            "TestAlphaChar" => same(
                "result",
                test_alpha_char(ch),
                out_v["result"].as_bool().unwrap(),
            ),
            "PastedStatIsBuffed" => same(
                "result",
                pasted_stat_is_buffed(a[0].as_str().unwrap(), a[1].as_str().unwrap()),
                out_v["result"].as_bool().unwrap(),
            ),
            other => Err(format!("unknown proc {other}")),
        };
    }
    let step = &inputs["steps"][0];
    let a = step["args"].as_array();
    let setup_forms = &header["setup"]["forms"]["frmMain"];
    let case_forms = &inputs["forms"]["frmMain"];
    let forms = Forms {
        setup: setup_forms,
        case: case_forms,
        globals: &inputs["globals"],
    };
    let mut st = state(world, &forms);
    let loaded = inputs["loaded_forms"]
        .as_array()
        .is_some_and(|l| l.iter().any(|n| n == "frmMonsterAttackSim"));
    let nmr = header["setup"]["globals"]["nNMRVer"]
        .as_f64()
        .unwrap_or(0.0);
    let env = PasteEnv {
        world,
        only_in_game: forms.global_flag("bOnlyInGame"),
        second_wrist: forms.global_flag("bInvenUse2ndWrist"),
        hide_record_numbers: forms.global_flag("bHideRecordNumbers"),
        nmr_ver: nmr,
        disable_kai_autolearn: false,
        min_item_level: vb::val(&forms.text("txtGlobalMinLVL")) as i32,
        alignment: forms.ctl("cmbGlobalAlignment")["ListIndex"]
            .as_i64()
            .unwrap_or(0) as i32,
        attack_sim_loaded: loaded,
        auto_save: forms.global_flag("bAutoSave"),
        last_char_file: "0",
        file_exists: &file_exists,
    };
    let mut ask = answers(case);
    let mut out = PasteOutcome::default();
    match step["call"].as_str().unwrap() {
        "PasteCharacter" => {
            out = paste_character(&env, &mut st, &forms.text("txtClip"), &mut ask);
        }
        "PasteSpells" => {
            let text = a.unwrap()[0]["value"].as_str().unwrap();
            paste_spells(&env, &mut st, &mut out, text);
        }
        "PasteInventoryManager" => {
            let args = a.unwrap();
            if let Some(t) = args[0].as_str() {
                paste_inventory_manager(&env, &mut st, &mut out, &mut ask, t, true);
            } else {
                let t = forms.text("txtClip");
                paste_inventory_manager(&env, &mut st, &mut out, &mut ask, &t, false);
            }
        }
        "ApplyPastedStat" => {
            let args = a.unwrap();
            let mut manual = args[6]["value"].as_str().unwrap().to_string();
            apply_pasted_stat(
                &mut st,
                &mut out,
                &mut ask,
                i(&args[0]) as usize,
                args[1].as_str().unwrap(),
                i(&args[2]),
                args[3].as_bool().unwrap(),
                i(&args[4]),
                args[5].as_str().unwrap(),
                &mut manual,
            );
            same(
                "manual",
                manual.as_str(),
                out_v["x.refs"][0].as_str().unwrap(),
            )?;
        }
        other => return Err(format!("unknown call {other}")),
    }
    check_state(world, &st, &out, out_v, case)
}

#[test]
fn paste_char_fixture() {
    let fx = fixture::load("paste_char");
    let w = world(&fx.header["data"]);
    let mut failures = Vec::new();
    for case in &fx.cases {
        if let Err(msg) = run(&w, &fx.header, case) {
            let id = case["id"].as_str().unwrap();
            let inputs = case["inputs"].to_string();
            let short: String = inputs.chars().take(1500).collect();
            failures.push(format!("{id}: {msg}\n  inputs {short}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases failed; first:\n{}",
        failures.len(),
        fx.cases.len(),
        failures
            .iter()
            .take(4)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
