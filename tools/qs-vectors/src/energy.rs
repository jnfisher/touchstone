//! The energy family (Alter Energy Usage, ability 87).
//!
//! Each case is a made-up character whose bless spells, race, class, worn items, weapon and a
//! carried item may carry ability 87. The engine's own ability fold gives the character's total
//! (`fold`, a running average that depends on the order of the sources), and the engine's fighter
//! builder gives the energy a normal blow costs with that total, at two levels (`energy`), beside
//! the cost without it (`base`). The engine floors a blow at 200; MME has no floor, so the
//! comparison against touchstone is of swings, which the cap of five swings floors the same way.

use quicksilver_balance::data::Data;
use serde_json::{Value, json};
use silvermere_engine::clear_player;
use silvermere_engine::world::blank;
use silvermere_rules::abilities::{CharacterAbilities, dynamic_stats};
use silvermere_rules::combat::{
    AlignGroup, Blow, Move, PlayerAbilities, player_fighter,
};
use silvermere_types::config::Balance;
use silvermere_types::enums::{Ability, ItemType, WornLocation};
use silvermere_types::ids::{ClassNo, ItemNo, RaceNo, SpellNo};
use silvermere_types::records::Spell;

use crate::Family;

/// The sources a case may use, in the order the engine folds them (bless spells first, then race,
/// class, the worn items in slot order, the weapon, the carried item).
const SOURCES: [&str; 9] = [
    "spell_a", "spell_b", "race", "class", "head", "torso", "finger", "weapon", "carried",
];
/// Values the ability takes: a hasten-type 85, slower and faster cases, and a 0 (an item slot of
/// 0 still averages in).
const VALUES: [i16; 6] = [85, 60, 120, 100, 70, 0];
/// The levels the blow is built at; both leave a normal blow above the floor of 200.
const LEVELS: [u16; 2] = [5, 10];

pub fn family() -> Family {
    let data = Data::content().expect("the stock records with the 1.20 content");
    let template: Spell = data.spells.values().next().expect("a spell").clone();
    let mut cases = Vec::new();
    let n = SOURCES.len();
    for mask in 1u32..(1 << n) {
        let count = mask.count_ones();
        if !(1..=4).contains(&count) {
            continue;
        }
        // values cycle with the case, so every source sees several and orders differ
        let mut vi = (mask as usize * 7) % VALUES.len();
        let mut sources: Vec<(usize, i16)> = Vec::new();
        for (k, _) in SOURCES.iter().enumerate() {
            if mask & (1 << k) != 0 {
                let v = VALUES[vi % VALUES.len()];
                // a spell slot of 0 would take the rolled value; keep spells non-zero
                let v = if k < 2 && v == 0 { 85 } else { v };
                sources.push((k, v));
                vi += 1 + k % 3;
            }
        }
        cases.push(case(&template, &sources));
    }
    cases.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    Family {
        name: "energy",
        about: "Quicksilver's Alter Energy Usage (ability 87) on a made-up character. `inputs.sources` lists the \
                sources that carry the ability as [name, value], in the engine's fold order: bless spells \
                spell_a and spell_b, then race, class, the worn items head, torso and finger, the weapon, and a \
                carried item. `fold` is the engine's running-average total ((running + value) / 2 once the \
                running value is not 0). `energy` is, for each level in `levels` with the 1200-speed weapon, \
                combat 3, Str 50 and Agl 60, the energy a normal blow costs with the total, and `base` the \
                cost without it; the engine floors a blow at 200 (never reached here).",
        keys: json!({}),
        cases,
    }
}

fn case(template: &Spell, sources: &[(usize, i16)]) -> Value {
    let find = |k: usize| sources.iter().find(|(s, _)| *s == k).map(|&(_, v)| v);
    let abil = |v: i16| (Ability::EnergyUse, v);

    let mut spells = Vec::new();
    for (idx, k) in [0usize, 1].iter().enumerate() {
        if let Some(v) = find(*k) {
            let mut s = template.clone();
            s.number = SpellNo(1001 + idx as i16);
            s.abilities = std::array::from_fn(|_| (Ability::None, 0));
            s.abilities[0] = abil(v);
            spells.push(s);
        }
    }
    let mut race = blank::clear_race(RaceNo(1));
    if let Some(v) = find(2) {
        race.abilities[0] = abil(v);
    }
    let mut class = blank::clear_class(ClassNo(1));
    class.combat = 3;
    if let Some(v) = find(3) {
        class.abilities[0] = abil(v);
    }
    let item = |number: i32, k: usize, kind: ItemType, worn: WornLocation| {
        let mut it = blank::clear_item(ItemNo(number));
        it.item_type = kind;
        it.worn = worn;
        if let Some(v) = find(k) {
            it.abilities[0] = abil(v);
        }
        it
    };
    let head = item(2001, 4, ItemType::Armour, WornLocation::Head);
    let torso = item(2002, 5, ItemType::Armour, WornLocation::Torso);
    let finger = item(2003, 6, ItemType::Armour, WornLocation::Finger);
    let weapon = item(2004, 7, ItemType::Weapon, WornLocation::Nowhere);
    let carried = item(2005, 8, ItemType::General, WornLocation::Nowhere);
    let all_items = [&head, &torso, &finger, &weapon, &carried];

    let mut plr = clear_player();
    plr.race = race.number;
    plr.class = class.number;
    for (k, s) in spells.iter().enumerate() {
        plr.spells[k] = s.number;
    }
    // worn items in touchstone's slot order: head, torso, finger
    plr.worn[0] = head.number;
    plr.worn[1] = torso.number;
    plr.worn[2] = finger.number;
    plr.weapon = weapon.number;
    plr.items = vec![carried.number];

    let lookup_spell = |n: SpellNo| spells.iter().find(|s| s.number == n);
    let lookup_item = |n: ItemNo| all_items.iter().copied().find(|i| i.number == n);
    let fold = dynamic_stats(&CharacterAbilities {
        character: &plr,
        race: Some(&race),
        class: Some(&class),
        spells: &lookup_spell,
        items: &lookup_item,
    })
    .energyuse;

    let mut energy = Vec::new();
    for level in LEVELS {
        let cost = |euse: i32| {
            let mut c = clear_player();
            c.class = class.number;
            c.level = level;
            c.str = 50;
            c.agi = 60;
            c.maxenergy = 1000;
            c.energyuse = euse as i16;
            let mut w = blank::clear_item(ItemNo(1));
            w.speed = 1200;
            w.mindmg = 2;
            w.maxdmg = 12;
            player_fighter(
                &c,
                &class,
                Some(&w),
                &[],
                PlayerAbilities::default(),
                Blow::new(Move::Attack, AlignGroup::Evil),
                &Balance::V120,
            )
            .energy
        };
        energy.push(json!({ "level": level, "base": cost(0), "with": cost(fold) }));
    }
    let id: String = sources
        .iter()
        .map(|(k, v)| format!("{}{}", SOURCES[*k], v))
        .collect::<Vec<_>>()
        .join("+");
    json!({
        "id": format!("energy/{id}"),
        "kind": "fold",
        "inputs": { "sources": sources.iter().map(|(k, v)| json!([SOURCES[*k], v])).collect::<Vec<_>>() },
        "fold": fold,
        "energy": energy,
    })
}
