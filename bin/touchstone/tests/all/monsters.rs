//! The Monsters list (`POST /api/evaluate/monsters`) and pane (`/api/detail/monsters/{n}`), pinned to the
//! oracle fixtures: filters_sm.json (FilterMonsters, monster cases `m-*` and `r-*`) and monster_detail.json
//! (PullMonsterDetail). The worlds are rebuilt from the fixture headers, as the rules crate's tests do.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use touchstone::state::{Loaded, SourceInfo};
use touchstone::{AppState, app};
use touchstone_model::{
    AbilitySlot, Class, DropSlot, Info, Item, Lair, MidSpell, Monster, MonsterAttack, Race, Room,
    Spell, TbInfo, World, WorldParts,
};
use tower::ServiceExt;

pub(crate) fn i(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
}

pub(crate) fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
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

/// The loaded character of a case, as the wire character.
pub(crate) fn character(fx: &Value, c: &Value) -> Value {
    let setup = &fx["header"]["setup"]["forms"]["frmMain"];
    let forms = &c["inputs"]["forms"]["frmMain"];
    let g = &c["inputs"]["globals"];
    let gs = &fx["header"]["setup"]["globals"];
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
    let global = |n: &str| if g[n].is_null() { &gs[n] } else { &g[n] };
    let gi = |n: &str| global(n).as_i64().unwrap_or(0);
    let gb = |n: &str| global(n).as_bool().unwrap_or(false);
    json!({
        "race": combo("cmbGlobalRace"),
        "class": combo("cmbGlobalClass"),
        "level": text("txtGlobalLevel", 0),
        "alignment": ctl("cmbGlobalAlignment")["ListIndex"].as_i64().unwrap_or(0),
        "global_filter": true,
        "stats": (0..6).map(|k| text("txtCharStats", k)).collect::<Vec<_>>(),
        "stat_adjustments": global("char_StatAdjustments").as_array().cloned().unwrap_or_else(|| vec![json!(0); 6]),
        "equipped": global("nEquippedItem").as_array().cloned().unwrap_or_else(|| vec![json!(0); 20]),
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

pub(crate) fn fixture(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../fixtures/rules/{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

pub(crate) fn case<'a>(fx: &'a Value, id: &str) -> &'a Value {
    fx["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no case {id}"))
}

// ---------------------------------------------------------------------------------------------- worlds

/// The pipeline fixture's classes, races, items and spells (the rows the characters use).
pub(crate) struct Base {
    pub(crate) items: Vec<Item>,
    pub(crate) spells: Vec<Spell>,
    pub(crate) classes: Vec<Class>,
    pub(crate) races: Vec<Race>,
}

pub(crate) fn base() -> Base {
    let fx = fixture("pipeline");
    let data = &fx["header"]["data"];
    let rows = |k: &str| data[k].as_array().unwrap().clone();
    let items = rows("items")
        .iter()
        .map(|r| {
            let mut class_rest = [0; 10];
            for (k, c) in r[11].as_array().unwrap().iter().enumerate() {
                class_rest[k] = i(c);
            }
            blank_item(i(&r[0]), String::new(), |it| {
                it.encum = i(&r[1]);
                it.item_type = i(&r[2]);
                it.armour_class = i(&r[3]);
                it.damage_resist = i(&r[4]);
                it.weapon_type = i(&r[5]);
                it.armour_type = i(&r[6]);
                it.worn = i(&r[7]);
                it.accy = i(&r[8]);
                it.str_req = i(&r[9]);
                it.speed = i(&r[10]);
                it.class_rest = class_rest;
                it.abilities = pairs(&r[12]);
            })
        })
        .collect();
    let spells = rows("spells")
        .iter()
        .map(|r| Spell {
            number: i(&r[0]),
            name: None,
            short: None,
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
    Base {
        items,
        spells,
        classes,
        races,
    }
}

fn blank_item(number: i32, name: String, set: impl FnOnce(&mut Item)) -> Item {
    let mut it = Item {
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
    };
    set(&mut it);
    it
}

pub(crate) fn info() -> Info {
    Info {
        nmr_version: "v1.8.3".to_owned(),
        dat_file_version: "v1.11p".to_owned(),
        date: String::new(),
        time: String::new(),
        custom: String::new(),
        legit: false,
        disable_kai: false,
        update_url: String::new(),
    }
}

pub(crate) fn blank_attack() -> MonsterAttack {
    MonsterAttack {
        name: String::new(),
        att_type: 0,
        acc: 0,
        pct: 0,
        true_pct: 0.0,
        min: 0,
        max: 0,
        energy: 0,
        hit_spell: 0,
    }
}

pub(crate) fn none_if_empty(s: String) -> Option<String> {
    (!s.is_empty()).then_some(s)
}

fn lairs_of(rows: &Value) -> Vec<Lair> {
    rows.as_array()
        .unwrap()
        .iter()
        .map(|r| Lair {
            group_index: s(&r[0]),
            mob_list: s(&r[1]),
            mobs: i(&r[2]),
            total_lairs: i(&r[3]),
            avg_delay: i(&r[4]),
            avg_walk: f(&r[5]),
            avg_exp: f(&r[6]),
            avg_dmg: f(&r[7]),
            avg_dmg_phys: 0.0,
            avg_dmg_spell: 0.0,
            avg_hp: f(&r[8]),
            avg_ac: i(&r[9]),
            avg_dr: i(&r[10]),
            avg_mr: i(&r[11]),
            avg_dodge: i(&r[12]),
        })
        .collect()
}

/// The world of filters_sm.json: the pipeline's rows plus the scenario's monsters, lairs and spells.
fn list_world() -> World {
    let fx = fixture("filters_sm");
    let data = &fx["header"]["data"];
    let b = base();
    let slots = |v: &Value| {
        let mut out = [AbilitySlot::default(); 10];
        for p in v.as_array().unwrap() {
            out[i(&p[0]) as usize] = AbilitySlot {
                ability: i(&p[1]),
                value: i(&p[2]),
            };
        }
        out
    };
    let monsters = data["monsters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut attacks: [MonsterAttack; 5] = std::array::from_fn(|_| blank_attack());
            for a in r[26].as_array().unwrap() {
                attacks[i(&a[0]) as usize] = MonsterAttack {
                    att_type: i(&a[1]),
                    acc: i(&a[2]),
                    pct: i(&a[3]),
                    true_pct: f(&a[4]),
                    hit_spell: i(&a[5]),
                    ..blank_attack()
                };
            }
            let mut mid_spells: [MidSpell; 5] = Default::default();
            for m in r[27].as_array().unwrap() {
                mid_spells[i(&m[0]) as usize] = MidSpell {
                    spell: i(&m[1]),
                    pct: i(&m[2]),
                    lvl: 0,
                };
            }
            Monster {
                number: i(&r[0]),
                name: none_if_empty(s(&r[1])),
                weapon: 0,
                armour_class: i(&r[6]),
                damage_resist: i(&r[7]),
                follow_pct: i(&r[15]),
                magic_res: i(&r[8]),
                bs_defense: i(&r[9]),
                exp: f(&r[3]),
                exp_multi: f(&r[4]),
                hp: i(&r[5]),
                energy: 0,
                avg_dmg: f(&r[13]),
                avg_dmg_phys: 0.0,
                avg_dmg_spell: 0.0,
                greet_txt: 0,
                hp_regen: i(&r[12]),
                charm_lvl: 0,
                r#type: 0,
                undead: i(&r[10]),
                align: i(&r[16]),
                regen_time: i(&r[2]),
                game_limit: i(&r[17]),
                avg_lair_exp: f(&r[11]),
                script_value: 0.0,
                r: i(&r[18]),
                p: i(&r[19]),
                g: i(&r[20]),
                s: i(&r[21]),
                c: i(&r[22]),
                death_spell: i(&r[14]),
                create_spell: 0,
                attacks,
                mid_spells,
                drops: <[DropSlot; 10]>::default(),
                abilities: slots(&r[25]),
                in_game: i(&r[23]) == 1,
                summoned_by: none_if_empty(s(&r[24])),
            }
        })
        .collect();
    // the scenario's spell rows add what the filters read to a copy of a pipeline spell
    let spells = data["spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let n = i(&r[0]);
            let mut sp = b
                .spells
                .iter()
                .find(|x| x.number == n)
                .or_else(|| b.spells.first())
                .unwrap()
                .clone();
            sp.number = n;
            // a spell with no name adds no row to the Spells list
            sp.name = Some(format!("Spell {n}"));
            sp.req_level = i(&r[1]);
            sp.magery = i(&r[2]);
            sp.magery_lvl = i(&r[3]);
            sp.learnable = i(&r[4]) > 0;
            sp.targets = i(&r[5]);
            sp.att_type = i(&r[6]);
            sp.dur = i(&r[7]);
            sp.dur_inc_lvls = i(&r[8]);
            sp.dur_inc = i(&r[9]);
            sp.learned_from = none_if_empty("x".repeat(i(&r[10]) as usize));
            sp.casted_by = none_if_empty("x".repeat(i(&r[11]) as usize));
            sp.classes = none_if_empty(s(&r[12]));
            sp.abilities = [AbilitySlot::default(); 10];
            for (k, a) in r[13].as_array().unwrap().iter().enumerate() {
                sp.abilities[k] = AbilitySlot {
                    ability: i(&a[0]),
                    value: i(&a[1]),
                };
            }
            sp
        })
        .collect();
    World::new(WorldParts {
        info: info(),
        items: b.items,
        monsters,
        spells,
        rooms: vec![],
        shops: vec![],
        classes: b.classes,
        races: b.races,
        lairs: lairs_of(&data["lairs"]),
        tbinfo: vec![],
    })
    .unwrap()
}

/// Column lookup for a row in a table's own column order.
struct Cols(std::collections::HashMap<String, usize>);

impl Cols {
    fn new(names: &Value) -> Self {
        Self(
            names
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .map(|(k, n)| (s(n), k))
                .collect(),
        )
    }
    fn at<'a>(&self, r: &'a Value, name: &str) -> &'a Value {
        &r[*self
            .0
            .get(name)
            .unwrap_or_else(|| panic!("no column {name}"))]
    }
    fn i(&self, r: &Value, name: &str) -> i32 {
        i(self.at(r, name))
    }
    fn f(&self, r: &Value, name: &str) -> f64 {
        f(self.at(r, name))
    }
    fn s(&self, r: &Value, name: &str) -> String {
        s(self.at(r, name))
    }
    fn slots<const N: usize>(&self, r: &Value) -> [AbilitySlot; N] {
        std::array::from_fn(|k| AbilitySlot {
            ability: self.i(r, &format!("Abil-{k}")),
            value: self.i(r, &format!("AbilVal-{k}")),
        })
    }
}

fn detail_monster(c: &Cols, r: &Value) -> Monster {
    Monster {
        number: c.i(r, "Number"),
        name: none_if_empty(c.s(r, "Name")),
        weapon: c.i(r, "Weapon"),
        armour_class: c.i(r, "ArmourClass"),
        damage_resist: c.i(r, "DamageResist"),
        follow_pct: c.i(r, "Follow%"),
        magic_res: c.i(r, "MagicRes"),
        bs_defense: c.i(r, "BSDefense"),
        exp: c.f(r, "EXP"),
        exp_multi: c.f(r, "ExpMulti"),
        hp: c.i(r, "HP"),
        energy: c.i(r, "Energy"),
        avg_dmg: c.f(r, "AvgDmg"),
        avg_dmg_phys: c.f(r, "AvgDmgPhys"),
        avg_dmg_spell: c.f(r, "AvgDmgSpell"),
        greet_txt: c.i(r, "GreetTXT"),
        hp_regen: c.i(r, "HPRegen"),
        charm_lvl: c.i(r, "CharmLVL"),
        r#type: c.i(r, "Type"),
        undead: c.i(r, "Undead"),
        align: c.i(r, "Align"),
        regen_time: c.i(r, "RegenTime"),
        game_limit: c.i(r, "GameLimit"),
        avg_lair_exp: c.f(r, "AvgLairExp"),
        script_value: c.f(r, "ScriptValue"),
        r: c.i(r, "R"),
        p: c.i(r, "P"),
        g: c.i(r, "G"),
        s: c.i(r, "S"),
        c: c.i(r, "C"),
        death_spell: c.i(r, "DeathSpell"),
        create_spell: c.i(r, "CreateSpell"),
        attacks: std::array::from_fn(|k| MonsterAttack {
            name: c.s(r, &format!("AttName-{k}")),
            att_type: c.i(r, &format!("AttType-{k}")),
            acc: c.i(r, &format!("AttAcc-{k}")),
            pct: c.i(r, &format!("Att%-{k}")),
            true_pct: c.f(r, &format!("AttTrue%-{k}")),
            min: c.i(r, &format!("AttMin-{k}")),
            max: c.i(r, &format!("AttMax-{k}")),
            energy: c.i(r, &format!("AttEnergy-{k}")),
            hit_spell: c.i(r, &format!("AttHitSpell-{k}")),
        }),
        mid_spells: std::array::from_fn(|k| MidSpell {
            spell: c.i(r, &format!("MidSpell-{k}")),
            pct: c.i(r, &format!("MidSpell%-{k}")),
            lvl: c.i(r, &format!("MidSpellLVL-{k}")),
        }),
        drops: std::array::from_fn(|k| DropSlot {
            item: c.i(r, &format!("DropItem-{k}")),
            pct: c.i(r, &format!("DropItem%-{k}")),
        }),
        abilities: c.slots(r),
        in_game: true,
        summoned_by: none_if_empty(c.s(r, "Summoned By")),
    }
}

fn detail_spell(c: &Cols, r: &Value) -> Spell {
    Spell {
        number: c.i(r, "Number"),
        name: none_if_empty(c.s(r, "Name")),
        short: Some(c.s(r, "Short")),
        req_level: c.i(r, "ReqLevel"),
        energy_cost: c.i(r, "EnergyCost"),
        mana_cost: c.i(r, "ManaCost"),
        min_base: c.i(r, "MinBase"),
        max_base: c.i(r, "MaxBase"),
        diff: c.i(r, "Diff"),
        type_of_resists: c.i(r, "TypeOfResists"),
        targets: c.i(r, "Targets"),
        dur: c.i(r, "Dur"),
        att_type: c.i(r, "AttType"),
        magery: c.i(r, "Magery"),
        magery_lvl: c.i(r, "MageryLVL"),
        cap: c.i(r, "Cap"),
        max_inc_lvls: c.i(r, "MaxIncLVLs"),
        max_inc: c.i(r, "MaxInc"),
        min_inc_lvls: c.i(r, "MinIncLVLs"),
        min_inc: c.i(r, "MinInc"),
        dur_inc_lvls: c.i(r, "DurIncLVLs"),
        dur_inc: c.i(r, "DurInc"),
        abilities: c.slots(r),
        learnable: c.i(r, "Learnable") != 0,
        learned_from: None,
        casted_by: None,
        classes: None,
    }
}

fn detail_item(c: &Cols, r: &Value) -> Item {
    blank_item(c.i(r, "Number"), c.s(r, "Name"), |it| {
        it.limit = c.i(r, "Limit");
        it.encum = c.i(r, "Encum");
        it.item_type = c.i(r, "ItemType");
        it.use_count = c.i(r, "UseCount");
        it.price = c.i(r, "Price");
        it.currency = c.i(r, "Currency");
        it.min = c.i(r, "Min");
        it.max = c.i(r, "Max");
        it.armour_class = c.i(r, "ArmourClass");
        it.damage_resist = c.i(r, "DamageResist");
        it.weapon_type = c.i(r, "WeaponType");
        it.armour_type = c.i(r, "ArmourType");
        it.worn = c.i(r, "Worn");
        it.accy = c.i(r, "Accy");
        it.gettable = c.i(r, "Gettable") != 0;
        it.str_req = c.i(r, "StrReq");
        it.speed = c.i(r, "Speed");
        it.not_droppable = c.i(r, "Not Droppable") != 0;
        it.destroy_on_death = c.i(r, "Destroy On Death") != 0;
        it.retain_after_uses = c.i(r, "Retain After Uses") != 0;
        it.class_rest = std::array::from_fn(|k| c.i(r, &format!("ClassRest-{k}")));
        it.race_rest = std::array::from_fn(|k| c.i(r, &format!("RaceRest-{k}")));
        it.negate_spell = std::array::from_fn(|k| c.i(r, &format!("NegateSpell-{k}")));
        it.abilities = c.slots(r);
        it.in_game = c.i(r, "In Game") != 0;
        it.obtained_from = Some(c.s(r, "Obtained From"));
    })
}

/// The world of monster_detail.json: the pipeline's rows plus the scenario's (a scenario row replaces the
/// pipeline's of the same number). `multiplier_off` makes every ExpMulti 1, which is what the cases that run
/// with `UseExpMulti` False mean (the server always runs with it on, as the stock data has the column).
fn detail_world(multiplier_off: bool) -> World {
    let fx = fixture("monster_detail");
    let table = &fx["header"]["data"]["table"];
    let b = base();
    let (mc, sc, ic) = (
        Cols::new(&table["monster_cols"]),
        Cols::new(&table["spell_cols"]),
        Cols::new(&table["item_cols"]),
    );
    let monsters = table["monsters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut m = detail_monster(&mc, r);
            if multiplier_off {
                m.exp_multi = 1.0;
            }
            m
        })
        .collect();
    let mut spells = b.spells.clone();
    for r in table["spells"].as_array().unwrap() {
        let sp = detail_spell(&sc, r);
        match spells.iter_mut().find(|x| x.number == sp.number) {
            Some(slot) => *slot = sp,
            None => spells.push(sp),
        }
    }
    let mut items = b.items.clone();
    for r in table["items"].as_array().unwrap() {
        let it = detail_item(&ic, r);
        match items.iter_mut().find(|x| x.number == it.number) {
            Some(slot) => *slot = it,
            None => items.push(it),
        }
    }
    let mut classes = b.classes.clone();
    for r in table["classes"].as_array().unwrap() {
        if let Some(c) = classes.iter_mut().find(|c| c.number == i(&r[0])) {
            c.name = s(&r[1]);
        }
    }
    let rooms = table["rooms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Room {
            map_number: i(&r[0]),
            room_number: i(&r[1]),
            name: Some(s(&r[2])),
            light: 0,
            shop: 0,
            npc: 0,
            cmd: 0,
            spell: 0,
            lair: None,
            delay: i(&r[3]),
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
    let tbinfo = table["tbinfo"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| TbInfo {
            number: i(&r[0]),
            link_to: i(&r[1]),
            action: Some(s(&r[2])),
            called_from: s(&r[3]),
        })
        .collect();
    World::new(WorldParts {
        info: info(),
        items,
        monsters,
        spells,
        rooms,
        shops: vec![],
        classes,
        races: b.races,
        lairs: lairs_of(&table["lairs"]),
        tbinfo,
    })
    .unwrap()
}

// ---------------------------------------------------------------------------------------------- requests

/// The app over a world, with the settings the oracle ran with: 1000 simulation rounds (`nGlobalMonsterSimRounds`),
/// where the server's default is MME's 500.
pub(crate) fn router(world: World) -> axum::Router {
    router_state(world).0
}

pub(crate) fn router_state(world: World) -> (axum::Router, std::sync::Arc<AppState>) {
    router_rounds(world, 1000)
}

/// [`router`] with `rounds` simulation rounds in the settings.
pub(crate) fn router_rounds(world: World, rounds: i64) -> (axum::Router, std::sync::Arc<AppState>) {
    let info = SourceInfo {
        kind: "mdb",
        path: "test".into(),
    };
    let loaded = Loaded::build(world, info, None, 1).unwrap();
    let dir = tempfile::TempDir::new().unwrap().keep();
    std::fs::write(
        dir.join("settings.json"),
        json!({"monster_sim_rounds": rounds}).to_string(),
    )
    .unwrap();
    let state = AppState::with_data_dir(loaded, dir);
    (app(state.clone()), state)
}

pub(crate) async fn call(
    app: &axum::Router,
    uri: &str,
    body: Option<&Value>,
) -> (StatusCode, Value) {
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

/// The monster controls of a filters_sm case, in the wire shape.
fn controls(fx: &Value, c: &Value) -> Value {
    let setup = &fx["header"]["setup"];
    let g = |name: &str| {
        let v = &c["inputs"]["globals"][name];
        if v.is_null() {
            setup["globals"][name].clone()
        } else {
            v.clone()
        }
    };
    let ctl = |name: &str| {
        let forms = &c["inputs"]["forms"]["frmMain"];
        if forms[name].is_null() {
            setup["forms"]["frmMain"][name].clone()
        } else {
            forms[name].clone()
        }
    };
    let text = |name: &str| ctl(name)["Text"].clone();
    let mut extras = json!({});
    let flag = |k: &str| g(&format!("filter_Monster_{k}"));
    for (wire, key) in [
        ("enabled", "bExtrasEnabled"),
        ("armour_class", "nArmourClass"),
        ("damage_resist", "nDamageResist"),
        ("magic_res", "nMagicRes"),
        ("bs_def", "nBSDef"),
        ("game_limit", "nGameLimit"),
        ("avg_lair_exp", "nAvgLairExp"),
        ("atk_accuracy_maj", "nAtkAccuracyMaj"),
        ("atk_accuracy_max", "nAtkAccuracyMax"),
        ("dodge", "nDodge"),
        ("num_lairs", "nNumLairs"),
        ("num_mobs_lte", "nNumMobsLTE"),
        ("num_mobs_gte", "nNumMobsGTE"),
        ("follow_gte", "nFollowGTE"),
        ("follow_lte", "nFollowLTE"),
        ("at_least_lairs", "nAtLeastLairs"),
        ("is_undead", "bIsUndead"),
        ("non_hostile_vs_evil", "bIsNonHostile_vEvil"),
        ("non_hostile_vs_ng", "bIsNonHostile_vNG"),
        ("at_least_lairs_on", "bAtLeastLairs"),
        ("drops_cash", "bDropsCash"),
        ("drops_r", "bDropsR"),
        ("drops_p", "bDropsP"),
        ("drops_g", "bDropsG"),
        ("drops_s", "bDropsS"),
        ("atk_no_poison", "bAtkNoPoison"),
        ("atk_no_confusion", "bAtkNoConfusion"),
        ("atk_no_fear", "bAtkNoFear"),
        ("show_all", "bShowAll"),
    ] {
        let v = flag(key);
        if !v.is_null() {
            extras[wire] = v;
        }
    }
    let mut abilities = [[0; 3]; 3];
    for st in c["inputs"]["steps"].as_array().unwrap() {
        if st["call"] == "SetAbil" {
            let a = st["args"].as_array().unwrap();
            abilities[i(&a[0]) as usize] = [i(&a[1]), i(&a[2]), i(&a[3])];
        }
    }
    extras["abilities"] = json!(abilities);
    json!({
        "regen_op": ctl("cmbMonsterRegen")["ListIndex"],
        "regen": text("txtMonsterRegen"),
        "hp": text("txtMonsterHP"),
        "exp": text("txtMonsterEXP"),
        "damage": text("txtMonsterDamage"),
        "magic": ctl("chkMonMagic")["Value"] == 1,
        "magic_value": text("txtMonMagic"),
        "extras": extras,
    })
}

/// The record numbers of the oracle's list, in order (a recorded row is "number:tag" or a row with cells).
fn want_numbers(c: &Value) -> Vec<i32> {
    c["outputs"]["mons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let t = r.as_str().map_or_else(|| s(&r[0]), str::to_owned);
            t.split(':').next().unwrap().parse().unwrap()
        })
        .collect()
}

fn got_numbers(list: &Value) -> Vec<i32> {
    list["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| i(&r["number"]))
        .collect()
}

// Expected: fixtures/rules/filters_sm.json monster cases (frmMain FilterMonsters, run by the oracle). The cases
// use no scripted reachability, so the list is the one the server builds from the boxes alone. m-damage-150 is left
// out: its limit falls between simulated damages, and the server runs CalculateAverageLairs first (as MME does while
// starting) where the oracle does not, so the two draw different random numbers.
#[tokio::test]
async fn filter_boxes_list_the_monsters_the_oracle_lists() {
    let fx = fixture("filters_sm");
    let app = router(list_world());
    let ids = [
        "m-default",
        "m-regen-0-0",
        "m-regen-0-10",
        "m-regen-1-10",
        "m-regen-0-abc",
        "m-hp-20",
        "m-hp-500",
        "m-damage-5",
        "m-magic-1",
        "m-magic-10",
        "m-magic-off-text",
        "m-blank-boxes",
        "m-x-nArmourClass-20",
        "m-x-nMagicRes-20",
        "m-x-nBSDef-20",
        "m-x-nGameLimit-5",
        "m-x-nDodge-20",
        "m-x-nNumLairs-3",
        "m-x-nNumMobsGTE-3",
        "m-x-nFollowLTE-20",
        "m-x-bIsUndead-True",
        "m-x-bIsNonHostile_vEvil-True",
        "m-x-bDropsCash-True",
        "m-x-bAtkNoPoison-True",
        "m-x-disabled",
        "m-abil-28-1-1",
        "m-abil-34-1-10",
        "m-abil-three",
        "m-atleast-lairs",
    ];
    let mut bad = Vec::new();
    for id in ids {
        let c = case(&fx, id);
        // the loaded character comes along, unused (the global filter is off): its attack choice still reaches the list
        let body = json!({
            "character": character(&fx, c),
            "global_filter": {"use_character": false},
            "options": {"monsters": controls(&fx, c)},
        });
        let (st, got) = call(&app, "/api/evaluate/monsters", Some(&body)).await;
        assert_eq!(st, StatusCode::OK, "{id}: {got}");
        if got_numbers(&got) != want_numbers(c) {
            bad.push(id);
        }
    }
    assert!(bad.is_empty(), "differ: {bad:?}");
}

// Expected: fixtures/rules/filters_sm.json cases m-hp-100 and m-x-bIsUndead-True: the caption reads "*Monsters*"
// once a record is filtered out (cmdNav(8)), and m-default leaves it plain.
#[tokio::test]
async fn caption_follows_whether_a_monster_was_filtered_out() {
    let fx = fixture("filters_sm");
    let app = router(list_world());
    for id in ["m-default", "m-hp-100", "m-x-bIsUndead-True"] {
        let c = case(&fx, id);
        let body = json!({"options": {"monsters": controls(&fx, c)}});
        let (_, got) = call(&app, "/api/evaluate/monsters", Some(&body)).await;
        let want = c["outputs"]["nav"].as_str().unwrap() == "*Monsters*";
        assert_eq!(got["filtered"] == true, want, "{id}");
    }
}

// Expected: fixtures/rules/filters_sm.json cases m-x-reach-showall's sibling m-abil-showall and m-showall-ac:
// with "show all" the filtered records are listed too, greyed (the row's ForeColor is 12632256).
#[tokio::test]
async fn show_all_lists_every_monster_and_greys_the_ones_that_fail() {
    let fx = fixture("filters_sm");
    let app = router(list_world());
    for id in ["m-abil-showall", "m-showall-ac"] {
        let c = case(&fx, id);
        let body = json!({"options": {"monsters": controls(&fx, c)}});
        let (_, got) = call(&app, "/api/evaluate/monsters", Some(&body)).await;
        assert_eq!(got_numbers(&got), want_numbers(c), "{id}");
        let grey = got["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["cells"][1]["color"] == 12_632_256)
            .count();
        assert!(grey > 0, "{id}: no greyed row");
    }
}

#[tokio::test]
async fn bad_monster_controls_are_400() {
    let app = router(list_world());
    for body in [
        json!({"options": {"monsters": {"nonsense": 1}}}),
        json!({"options": {"monsters": {"extras": {"armour_class": "many"}}}}),
        json!({"options": {"monsters": {"party": {"size": [1]}}}}),
    ] {
        let (st, got) = call(&app, "/api/evaluate/monsters", Some(&body)).await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "{body}: {got}");
    }
}

// ---------------------------------------------------------------------------------------------- the pane

/// A fixture row as the pane's line JSON (without the link target).
fn want_line(r: &Value) -> Value {
    if let Some(label) = r.as_str() {
        return json!({"label": label, "kind": "plain", "cells": []});
    }
    let cell = |c: &Value| {
        if let Some(t) = c.as_str() {
            return json!({"text": t});
        }
        let mut o = json!({"text": c[0]});
        if !c[1].is_null() {
            o["tag"] = c[1].clone();
        }
        if !c[2].is_null() {
            o["color"] = c[2].clone();
        }
        if c[3] == true {
            o["bold"] = json!(true);
        }
        o
    };
    let mut o = json!({
        "label": r[0],
        "kind": r[1].as_str().map_or_else(|| "plain".to_owned(), str::to_ascii_lowercase),
        "cells": r[4].as_array().unwrap().iter().map(cell).collect::<Vec<_>>(),
    });
    if !r[2].is_null() {
        o["color"] = r[2].clone();
    }
    if r[3] == true {
        o["bold"] = json!(true);
    }
    o
}

fn got_line(l: &Value) -> Value {
    let mut l = l.clone();
    l.as_object_mut().unwrap().remove("target");
    l
}

// Expected: fixtures/rules/monster_detail.json case nochar (PullMonsterDetail with no character: 50 monsters).
// Only the start-up globals the server runs with differ from the case's (UseExpMulti, the party flags), and no
// monster of the case reads them.
#[tokio::test]
async fn monster_panes_match_the_oracle_without_a_character() {
    let fx = fixture("monster_detail");
    let app = router(detail_world(true));
    let c = case(&fx, "nochar");
    let step = c["inputs"]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["call"] == "PullMonsterDetail")
        .unwrap();
    // the loaded character comes along but the global filter is off: only its attack choice reaches the pane
    let limit = step["args"][2].clone();
    let body = json!({
        "character": character(&fx, c),
        "global_filter": {"use_character": false},
        "lookup_limit": limit,
    });
    let want = c["outputs"]["rows"].as_array().unwrap();
    let mut bad = Vec::new();
    for (key, want) in step["rows"]["keys"].as_array().unwrap().iter().zip(want) {
        let n = key.as_i64().unwrap();
        let (st, got) = call(&app, &format!("/api/detail/monsters/{n}"), Some(&body)).await;
        assert_eq!(st, StatusCode::OK, "monster {n}: {got}");
        let got: Vec<Value> = without_reach_row(got["lines"].as_array().unwrap())
            .into_iter()
            .map(got_line)
            .collect();
        let want: Vec<Value> = want.as_array().unwrap().iter().map(want_line).collect();
        if got != want {
            let at = got.iter().zip(&want).position(|(g, w)| g != w);
            bad.push(format!(
                "monster {n}: {} lines (want {}), first difference {at:?}\n  got  {:?}\n  want {:?}",
                got.len(),
                want.len(),
                at.map(|k| &got[k]),
                at.map(|k| &want[k])
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} panes differ:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// What the case's globals say MME had calculated before the pane was built: the two damage arrays, the stored defense
/// description (the oracle's RefreshAll leaves its own in `sGlobalCharDefenseDescription`, so the stale note shows in both)
/// and the party flags, put in the store a calculation fills.
fn seed_damage(fx: &Value, c: &Value, state: &AppState, body: &Value) {
    use touchstone::evaluate::PartyBoxes;
    use touchstone::monster_damage::party_boxes_key;
    use touchstone_vb6::Currency;
    let g = |name: &str| {
        let v = &c["inputs"]["globals"][name];
        if v.is_null() {
            fx["header"]["setup"]["globals"][name].clone()
        } else {
            v.clone()
        }
    };
    let array = |name: &str| -> Vec<Currency> {
        g(name)
            .as_array()
            .unwrap()
            .iter()
            .map(|x| Currency::from_f64(x.as_f64().unwrap()).unwrap())
            .collect()
    };
    let loaded = state.current();
    let party: PartyBoxes =
        serde_json::from_value(body["options"]["monsters"]["party"].clone()).unwrap();
    let config = g("sMonsterDamageVsCharDefenseConfig")
        .as_str()
        .unwrap()
        .to_owned();
    let (vs_char, vs_party, vs_default) = (
        array("nMonsterDamageVsChar"),
        array("nMonsterDamageVsParty"),
        array("nMonsterDamageVsDefault"),
    );
    let key = party_boxes_key(&party);
    state
        .monster_damage()
        .update(loaded.info.load_count, &loaded.world, |s| {
            for (to, from) in [
                (&mut s.vs_char, &vs_char),
                (&mut s.vs_party, &vs_party),
                (&mut s.vs_default, &vs_default),
            ] {
                for (slot, v) in to.iter_mut().zip(from) {
                    *slot = *v;
                }
            }
            s.char_config = config;
            s.party_calculated = g("bMonsterDamageVsPartyCalculated") == true;
            if g("bDontPromptCalcPartyMonsterDamage") == true {
                s.party_asked = Some(key);
            }
        });
}

async fn character_case(id: &str) {
    let fx = fixture("monster_detail");
    let world = detail_world(true);
    // the case ran with nGlobalMonsterSimRounds 500
    let (app, state) = router_rounds(world, 500);
    let c = case(&fx, id);
    let step = c["inputs"]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["call"] == "PullMonsterDetail")
        .unwrap();
    let flag = |name: &str| {
        let v = &c["inputs"]["forms"]["frmMain"][name];
        let v = if v.is_null() {
            &fx["header"]["setup"]["forms"]["frmMain"][name]
        } else {
            v
        };
        v.as_bool().unwrap_or(false)
    };
    let ctl = |name: &str| {
        let v = &c["inputs"]["forms"]["frmMain"][name];
        if v.is_null() {
            fx["header"]["setup"]["forms"]["frmMain"][name].clone()
        } else {
            v.clone()
        }
    };
    let text = |name: &str, k: usize| ctl(name)["$array"][k.to_string()]["Text"].clone();
    let global_flag = |name: &str| {
        let v = &c["inputs"]["globals"][name];
        let v = if v.is_null() {
            &fx["header"]["setup"]["globals"][name]
        } else {
            v
        };
        v.as_bool().unwrap_or(false)
    };
    let body = json!({
        "character": character(&fx, c),
        "global_filter": {"use_character": ctl("chkGlobalFilter")["Value"] == 1},
        "lookup_limit": step["args"][2],
        "auto_calc_damage": flag("bAutoCalcMonDamage"),
        "dont_lookup_regen": flag("bDontLookupMonRegen"),
        "print_char_damage_out_first": global_flag("bMobPrintCharDamageOutFirst"),
        "options": {"monsters": {
            "lair": ctl("optMonsterFilter")["$array"]["1"]["Value"] == true,
            "damage": ctl("txtMonsterDamage")["Text"],
            "party": {
                "size": text("txtMonsterLairFilter", 0), "ac": text("txtMonsterLairFilter", 1),
                "dr": text("txtMonsterLairFilter", 2), "mr": text("txtMonsterLairFilter", 3),
                "dodge": text("txtMonsterLairFilter", 4), "hp": text("txtMonsterLairFilter", 5),
                "anti_magic": text("txtMonsterLairFilter", 6), "rest_hp": text("txtMonsterLairFilter", 7),
                "accuracy": text("txtMonsterLairFilter", 8), "swings": text("txtMonsterLairFilter", 9),
                "damage_out_physical": text("txtMonsterDamageOUT", 0),
                "damage_out_magic": text("txtMonsterDamageOUT", 1),
            },
        }},
    });
    seed_damage(&fx, c, &state, &body);
    let want = c["outputs"]["rows"].as_array().unwrap();
    let mut bad = Vec::new();
    for (key, want) in step["rows"]["keys"].as_array().unwrap().iter().zip(want) {
        let n = key.as_i64().unwrap();
        let (st, got) = call(&app, &format!("/api/detail/monsters/{n}"), Some(&body)).await;
        assert_eq!(st, StatusCode::OK, "monster {n}: {got}");
        let got: Vec<Value> = without_reach_row(got["lines"].as_array().unwrap())
            .into_iter()
            .map(got_line)
            .collect();
        let want: Vec<Value> = want.as_array().unwrap().iter().map(want_line).collect();
        if got != want {
            let at = got.iter().zip(&want).position(|(g, w)| g != w);
            bad.push(format!(
                "monster {n}: {} lines (want {}), first difference {at:?}\n  got  {:?}\n  want {:?}",
                got.len(),
                want.len(),
                at.map(|k| &got[k]),
                at.map(|k| &want[k])
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{id}: {} panes differ:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

// Expected: fixtures/rules/monster_detail.json cases warrior-notcalc, warrior-stale, mage, rogue, mystic, warrior-lair-first,
// mage-lair, nochar-lair and the party cases (PullMonsterDetail with the global filter on or off, solo and by lair). The
// cases set the damage arrays and flags MME had calculated beforehand; the test puts them in the store a calculation fills
// (`seed_damage`), so the "vs party defenses" wording and the monsters with calculated damage are covered too.
#[tokio::test]
async fn monster_panes_match_the_oracle_with_a_character() {
    for id in [
        "warrior-notcalc",
        "mage",
        "rogue",
        "mystic",
        "warrior-lair-first",
        "mage-lair",
        "nochar-lair",
        "warrior-stale",
        "warrior-party2",
        "warrior-party4",
        "warrior-party6",
        "mage-party3",
        "nochar-party3",
        "warrior-party-first",
        "warrior-party-incomplete",
        "warrior-party-complete",
    ] {
        character_case(id).await;
    }
}

#[tokio::test]
async fn reference_lines_link_to_their_records() {
    let app = router(detail_world(false));
    let (st, got) = call(&app, "/api/detail/monsters/19", None).await;
    assert_eq!(st, StatusCode::OK);
    let lines = got["lines"].as_array().unwrap();
    let weapon = lines.iter().find(|l| l["label"] == "Weapon").unwrap();
    assert_eq!(weapon["target"], json!({"table": "items", "number": 72}));
    let drop = lines
        .iter()
        .find(|l| l["kind"] == "item" && l["label"] == "Item Drops")
        .unwrap();
    assert_eq!(drop["target"]["table"], "items");
    // a room of "Spawns via ..." links by its map/room key
    let (_, other) = call(&app, "/api/detail/monsters/62", None).await;
    let room = other["lines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["kind"] == "room")
        .expect("a room line");
    assert_eq!(room["target"]["table"], "rooms");
    assert!(room["target"]["key"].as_str().unwrap().contains('/'));
}

#[tokio::test]
async fn unknown_monsters_are_404() {
    let app = router(detail_world(false));
    let (st, _) = call(&app, "/api/detail/monsters/99999", None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

/// The oracle stubs AddMonsterReachRows (the reachability module is its own family); the server shows its
/// "double-click to compute" row and the blank row after it until reachability is computed.
fn without_reach_row(lines: &[Value]) -> Vec<&Value> {
    let mut out = Vec::new();
    let mut skip = 0;
    for l in lines {
        if skip > 0 {
            skip -= 1;
        } else if l["kind"] == "reach_compute" {
            skip = 1;
        } else {
            out.push(l);
        }
    }
    out
}

/// The spell controls (and global filter) of a filters_sm `sp-*` case, in the wire shape.
fn spell_request(fx: &Value, c: &Value) -> Value {
    let setup = &fx["header"]["setup"]["forms"]["frmMain"];
    let forms = &c["inputs"]["forms"]["frmMain"];
    let ctl = |name: &str| {
        if forms[name].is_null() {
            setup[name].clone()
        } else {
            forms[name].clone()
        }
    };
    let list_index = |name: &str| ctl(name)["ListIndex"].as_i64().unwrap_or(0);
    let item_data = |name: &str, idx: &Value| {
        let c = ctl(name);
        let k = c["ListIndex"].as_i64().unwrap_or(0);
        idx.as_array()
            .unwrap_or_else(|| c["ItemData"].as_array().unwrap())
            .get(usize::try_from(k).unwrap())
            .and_then(Value::as_i64)
            .unwrap_or(0)
    };
    let class_box = &ctl("cmbGlobalClass")["$array"]["0"];
    let class = class_box["ItemData"]
        .as_array()
        .and_then(|d| d.get(class_box["ListIndex"].as_u64().unwrap_or(0) as usize))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let level = |k: &str| {
        ctl("txtGlobalLevel")["$array"][k]["Text"]
            .as_str()
            .unwrap_or("999")
            .to_owned()
    };
    let on = ctl("chkGlobalFilter")["Value"].as_i64() == Some(1);
    // chkGlobalFilter off: no bar, whatever values the boxes hold
    let bar = on.then(|| {
        json!({
            "use_character": false,
            "class": class,
            "level": level("0").parse::<i64>().unwrap(),
            "alignment": list_index("cmbGlobalAlignment"),
        })
    });
    json!({
        "global_filter": bar,
        "options": {
            "level": level("1").parse::<i64>().unwrap(),
            "spells": {
                "magery": list_index("cmbSpellMagery"),
                "magery_level": list_index("cmbSpellMageryLevel"),
                "learnable_only": ctl("chkSpellOptions")["$array"]["1"]["Value"].as_i64() == Some(1),
                "attack_type": list_index("cmbSpellAttackType"),
                "target": list_index("cmbSpellTarget"),
                "contains_ability": item_data("cmbSpellContainsAbil", &Value::Null),
            },
        },
    })
}

// Expected: fixtures/rules/filters_sm.json spell cases (frmMain FilterSpells, run by the oracle): the magery, magery
// level, learnable-only, attack-type, target and contains-ability boxes alone and together, and the global filter's
// class, level and alignment. Left out: the cases that call FilterSpells with its remove-filter argument (the Spell
// Book's call, pinned by spell_tools), the cases that list spells outside the game or turn Kai auto-learn off (the
// endpoint always lists in-game spells of the loaded database), and the ones that read a whole loaded character.
#[tokio::test]
async fn spell_boxes_list_the_spells_the_oracle_lists() {
    let fx = fixture("filters_sm");
    let app = router(list_world());
    let (mut checked, mut bad) = (0, Vec::new());
    for c in fx["cases"].as_array().unwrap() {
        let id = c["id"].as_str().unwrap();
        let remove = !c["inputs"]["steps"][0]["args"]
            .as_array()
            .is_none_or(Vec::is_empty);
        let from_character = !c["inputs"]["forms"]["frmMain"]["cmbEquip"].is_null();
        if !id.starts_with("sp-")
            || remove
            || from_character
            || matches!(id, "sp-default-all" | "sp-learn-kai-off")
        {
            continue;
        }
        let (st, got) = call(&app, "/api/evaluate/spells", Some(&spell_request(&fx, c))).await;
        assert_eq!(st, StatusCode::OK, "{id}: {got}");
        let mut want: Vec<i32> = c["outputs"]["spells"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().parse().unwrap())
            .collect();
        let mut have = got_numbers(&got);
        want.sort_unstable();
        have.sort_unstable();
        let caption = got["filtered"].as_bool().unwrap_or(false);
        if have != want || caption != (c["outputs"]["nav"] == "*Spells*") {
            bad.push(id.to_owned());
        }
        checked += 1;
    }
    eprintln!("{checked} spell cases through the endpoint");
    assert!(checked >= 100, "only {checked} cases ran");
    assert!(bad.is_empty(), "differ: {bad:?}");
}

#[tokio::test]
async fn bad_spell_controls_are_400() {
    let app = router(list_world());
    for body in [
        json!({"options": {"spells": {"magery": -1}}}),
        json!({"options": {"spells": {"target": 70000}}}),
        json!({"options": {"spells": {"nope": 1}}}),
    ] {
        let (st, got) = call(&app, "/api/evaluate/spells", Some(&body)).await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "{body}: {got}");
    }
}
