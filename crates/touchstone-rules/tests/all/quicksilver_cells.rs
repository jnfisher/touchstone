//! Q-016: whole characters from Quicksilver's balance tool against touchstone's character
//! pipeline (layer 3 of docs/quicksilver-profile.md part 4). `fixtures/quicksilver/cells.json`
//! (written by `tools/qs-vectors`, family `cells`) holds, for each cell, the inputs of a tool
//! character and the tool's damage a round; this rebuilds the character through `refresh_all` and
//! reads `get_damage_output` under `Profile::quicksilver_v120()`.
//!
//! The tool's figure is a steady-state mean from a simulation (a target that never dies, every
//! blow played), touchstone's an MME-shaped expectation; the differences are listed in
//! docs/differences.md ("Quicksilver: end-to-end cells"). The comparison guards against gross
//! error and wiring mistakes, not formula detail.

use std::path::PathBuf;

use serde_json::Value;
use touchstone_model::{
    AbilitySlot, Class, Info, Item, Monster, MonsterAttack, Race, Spell, World, WorldParts,
};
use touchstone_rules::Profile;
use touchstone_rules::ability_text::AbilityEnv;
use touchstone_rules::character::{AttackType, Character};
use touchstone_rules::combat::{DamageRequest, get_damage_output};
use touchstone_rules::equip::{EquipOptions, equip_item};
use touchstone_rules::pipeline::{PipelineOptions, refresh_all};

fn load() -> (Value, Vec<Value>) {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/quicksilver/cells.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let mut doc: Value = serde_json::from_str(&text).unwrap();
    let Value::Array(cases) = doc["cases"].take() else {
        panic!("no cases array");
    };
    (doc["header"].take(), cases)
}

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

fn ints<const N: usize>(v: &Value) -> [i32; N] {
    let mut out = [0; N];
    for (k, x) in v.as_array().unwrap().iter().enumerate() {
        out[k] = i(x);
    }
    out
}

fn rows<'a>(tables: &'a Value, key: &str) -> &'a Vec<Value> {
    tables[key].as_array().unwrap()
}

pub(crate) fn world(tables: &Value) -> World {
    let items = rows(tables, "items")
        .iter()
        .map(|r| Item {
            number: i(&r[0]),
            name: String::new(),
            limit: 0,
            encum: i(&r[1]),
            item_type: i(&r[2]),
            use_count: -1,
            price: 0,
            currency: 0,
            min: i(&r[11]),
            max: i(&r[12]),
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
            class_rest: ints(&r[13]),
            race_rest: ints(&r[14]),
            negate_spell: ints(&r[15]),
            abilities: pairs(&r[16]),
            in_game: true,
            obtained_from: None,
            references: None,
        })
        .collect();
    let spells = rows(tables, "spells")
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
            // the weave picks among spells a class can learn; spells an item casts are not
            learnable: r[21].as_bool().unwrap(),
            learned_from: None,
            casted_by: None,
            classes: None,
        })
        .collect();
    let classes = rows(tables, "classes")
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
    let races = rows(tables, "races")
        .iter()
        .map(|r| {
            let (lo, hi) = (ints::<6>(&r[1]), ints::<6>(&r[2]));
            // the rows list Str Int Wil Agl Hea Cha
            Race {
                number: i(&r[0]),
                name: String::new(),
                m_str: lo[0],
                m_int: lo[1],
                m_wil: lo[2],
                m_agl: lo[3],
                m_hea: lo[4],
                m_chm: lo[5],
                x_str: hi[0],
                x_int: hi[1],
                x_wil: hi[2],
                x_agl: hi[3],
                x_hea: hi[4],
                x_chm: hi[5],
                hp_per_lvl: i(&r[3]),
                exp_table: i(&r[4]),
                base_cp: 0,
                abilities: pairs(&r[5]),
            }
        })
        .collect();
    let monsters = rows(tables, "monsters")
        .iter()
        .map(|r| Monster {
            number: i(&r[0]),
            name: None,
            weapon: 0,
            armour_class: i(&r[1]),
            damage_resist: i(&r[2]),
            follow_pct: 0,
            magic_res: i(&r[3]),
            bs_defense: i(&r[4]),
            exp: 0.0,
            exp_multi: 0.0,
            hp: i(&r[5]),
            energy: i(&r[6]),
            avg_dmg: 0.0,
            avg_dmg_phys: 0.0,
            avg_dmg_spell: 0.0,
            greet_txt: 0,
            hp_regen: 0,
            charm_lvl: 0,
            r#type: 0,
            undead: i(&r[7]),
            align: i(&r[8]),
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
                name: "None".to_string(),
                att_type: 0,
                acc: 0,
                pct: 0,
                true_pct: 0.0,
                min: 0,
                max: 0,
                energy: 0,
                hit_spell: 0,
            }),
            mid_spells: Default::default(),
            drops: Default::default(),
            abilities: pairs(&r[9]),
            in_game: true,
            summoned_by: None,
        })
        .collect();
    World::new(WorldParts {
        info: Info {
            nmr_version: String::new(),
            dat_file_version: String::new(),
            content_version: Some("1.20.0".to_string()),
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
        shops: vec![],
        classes,
        races,
        lairs: vec![],
        tbinfo: vec![],
    })
    .unwrap()
}

/// What touchstone says for a cell: the average damage a round and the swings.
fn touchstone(world: &World, c: &Value, profile: Profile) -> (f64, f64) {
    let inp = &c["inputs"];
    let mut ch = Character {
        race: Some(i(&inp["race"])),
        class: Some(i(&inp["class"])),
        level: i(&inp["level"]),
        alignment: 1,
        stats: ints(&inp["stats"]),
        ..Character::default()
    };
    let opts = EquipOptions::default();
    let mut gear: Vec<i32> = inp["worn"].as_array().unwrap().iter().map(i).collect();
    gear.push(i(&inp["weapon"]));
    for n in gear {
        ch.equipped = equip_item(world, &ch.equipped, n, &opts).equipped;
    }
    for (k, b) in inp["buffs"].as_array().unwrap().iter().enumerate() {
        ch.bless[k] = i(b);
    }
    match c["tool"]["move"].as_str().unwrap() {
        "Bash" => ch.attack.attack_type = AttackType::PhysBash,
        "Smash" => ch.attack.attack_type = AttackType::PhysSmash,
        "Punch" | "Kick" | "JumpKick" => {
            ch.attack.attack_type = AttackType::MartialArts;
            ch.attack.martial_art = match c["tool"]["move"].as_str().unwrap() {
                "Punch" => 1,
                "Kick" => 2,
                _ => 3,
            };
        }
        _ => ch.attack.attack_type = AttackType::PhysAttack,
    }
    let state = refresh_all(profile, world, &ch, &PipelineOptions::default()).unwrap();
    let cenv = state.env([String::new(), String::new()], String::new());
    let mut ctx = cenv.context(world);
    ctx.profile = profile;
    let ab = AbilityEnv::for_world(world, profile);
    let out = get_damage_output(
        &cenv.damage_env(world, &ctx, &state.character, &ab),
        &DamageRequest {
            single_monster: i(&inp["target"]),
            force_character: true,
            ..DamageRequest::default()
        },
    );
    (out.average_damage.to_f64(), out.swings)
}

/// The spells of a cell's standing buffs that carry "Alter Energy Usage" (ability 87).
fn has_energy_buff(tables: &Value, c: &Value) -> bool {
    c["inputs"]["buffs"].as_array().unwrap().iter().any(|b| {
        rows(tables, "spells")
            .iter()
            .find(|r| i(&r[0]) == i(b))
            .is_some_and(|r| r[20].as_array().unwrap().iter().any(|a| i(&a[0]) == 87))
    })
}

#[test]
fn cells_header_and_shape() {
    let (header, cases) = load();
    assert_eq!(header["family"], "cells");
    assert_eq!(header["content_version"], "1.20.0");
    assert_eq!(header["case_count"].as_u64().unwrap(), cases.len() as u64);
    let ids: Vec<&str> = cases.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids sorted and unique");
    let cells = cases.iter().filter(|c| c["kind"] == "cell").count();
    assert!((30..=60).contains(&cells), "{cells} cells");
    // every class of the stock set is present at three levels
    let classes: std::collections::BTreeSet<i32> = cases
        .iter()
        .filter(|c| c["kind"] == "cell")
        .map(|c| i(&c["inputs"]["class"]))
        .collect();
    assert_eq!(classes.len(), 15);
    let size = std::fs::metadata(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/quicksilver/cells.json"),
    )
    .unwrap()
    .len();
    assert!(size <= 200 * 1024, "cells.json is {size} bytes");
}

/// Touchstone's damage a round against the balance tool's, cell by cell.
///
/// The tool plays 200,000 rounds of a target that never dies; touchstone gives MME's expectation.
/// Most cells agree within 7% (or 3 points on a small figure). The cells that do not each have a
/// named cause, and a wider allowance only for that cause (docs/differences.md, "Quicksilver:
/// end-to-end cells"):
/// - the tool caps a blow at the round's 1000 energy, so a very slow blow still lands once a round
///   where MME's swings fall below one (allowance 17%);
/// - a standing buff with Alter Energy Usage speeds the tool's blows; Quicksilver's profile now
///   applies it (Q-017), so those cells need no allowance and must stay within the default 7%;
/// - a weapon that casts spells on hit: the tool plays every cast, MME prices them from the spell
///   text (allowance 12%).
#[test]
fn cells_agree_with_the_balance_tool() {
    let (_, cases) = load();
    let tables = cases.iter().find(|c| c["kind"] == "tables").unwrap();
    let w = world(tables);
    let mut ratios = Vec::new();
    let mut bad = Vec::new();
    let (mut cannot_hit, mut capped, mut energy, mut cast) = (0, 0, 0, 0);
    for c in cases.iter().filter(|c| c["kind"] == "cell") {
        let id = c["id"].as_str().unwrap();
        let t = &c["tool"];
        let tool = t["dpr"].as_f64().unwrap();
        let (got, swings) = touchstone(&w, c, Profile::quicksilver_v120());
        if tool == 0.0 {
            // a weapon that cannot hurt the target: MME's marker for "cannot be hit" is -9998
            cannot_hit += 1;
            if got != -9998.0 {
                bad.push(format!("{id}: tool cannot hit, touchstone {got}"));
            }
            continue;
        }
        let ratio = got / tool;
        ratios.push(ratio);
        let mut allow: f64 = 0.07;
        if t["energy"].as_i64() == Some(1000) && swings < 1.0 {
            capped += 1;
            allow = allow.max(0.17);
        }
        if has_energy_buff(tables, c) {
            energy += 1;
        }
        if t["weapon_dpr"].as_f64().unwrap() > 5.0 {
            cast += 1;
            allow = allow.max(0.12);
        }
        if (got - tool).abs() > (allow * tool).max(3.0) {
            bad.push(format!(
                "{id}: tool {tool:.1}, touchstone {got:.1} (ratio {ratio:.2}, allowance {allow})"
            ));
        }
    }
    assert!(bad.is_empty(), "outside tolerance:\n{}", bad.join("\n"));
    ratios.sort_by(f64::total_cmp);
    let median = ratios[ratios.len() / 2];
    assert!((0.97..=1.02).contains(&median), "median ratio {median}");
    assert!(
        cannot_hit >= 2 && capped >= 3 && energy >= 5 && cast >= 1,
        "the explained groups are exercised: {cannot_hit} {capped} {energy} {cast}"
    );
}
