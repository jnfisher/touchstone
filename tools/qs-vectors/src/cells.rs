//! The end-to-end cells family (Q-016): whole characters from Quicksilver's balance tool
//! (`tools/balance`, layer 3 of docs/quicksilver-profile.md part 4).
//!
//! Each cell is one race, class and level in the tool's melee kit with its standing buffs, fighting
//! the band's reference monster. The cell holds what is needed to rebuild the same character in
//! touchstone (trained stats, the wielded and worn items, the buff spells) and the tool's figures:
//! the best move's damage a round at the steady state, its weave share, blows a round, hit chance
//! and the energy a blow costs. The rows the character needs (items, spells, classes, races, the
//! target) ride in one `tables` case as numbers, because the rules crate does no IO and the
//! Quicksilver database is not present in CI.
//!
//! The figure comes from the tool's library (`sim::melee` over `sim::cell_builds`), never from its
//! printed output. Deterministic: a fixed cell list, the tool's own seed and a fixed round count.

use std::collections::BTreeMap;

use quicksilver_balance::character::Cell;
use quicksilver_balance::data::Data;
use quicksilver_balance::kits::Kits;
use quicksilver_balance::sim::{self, TargetMonster};
use serde_json::{Value, json};
use silvermere_types::config::Balance;
use silvermere_types::ids::MonsterNo;
use silvermere_types::records::{AbilitySlot, Class, Item, KnownMonster, Race, Spell};

use crate::Family;

/// Rounds the tool plays for each figure. The tables use 20,000; more rounds shrink the sampling
/// error of the comparison without changing what the tool measures.
const ROUNDS: u32 = 200_000;

/// The classes, in the tool's order, and the levels. Three levels per class cover the tool's
/// L10, L30 and L60 kits.
const CLASSES: [&str; 15] = [
    "Warrior",
    "Witchunter",
    "Paladin",
    "Cleric",
    "Priest",
    "Missionary",
    "Ninja",
    "Thief",
    "Bard",
    "Gypsy",
    "Warlock",
    "Mage",
    "Druid",
    "Ranger",
    "Mystic",
];
const LEVELS: [u16; 3] = [10, 30, 60];

/// Races to rotate through, so a race other than Human reaches every class and level band.
/// A race the kit cannot serve (the tool refuses it, naming the item) falls back to the next.
const RACES: [&str; 6] = ["Human", "Half-Orc", "Dwarf", "Nekojin", "Elf", "Human"];

fn abilities(slots: &[AbilitySlot]) -> Value {
    Value::Array(
        slots
            .iter()
            .filter(|(a, _)| a.code() != 0)
            .map(|(a, v)| json!([a.code(), v]))
            .collect(),
    )
}

fn item_row(i: &Item) -> Value {
    json!([
        i.number.0,
        i.weight,
        i.item_type.code(),
        i.ac,
        i.dr,
        i.wpntype.code(),
        i.armtype.code(),
        i.worn.code(),
        i.accuracy,
        i.strreq,
        i.speed,
        i.mindmg,
        i.maxdmg,
        i.classes.iter().map(|c| c.0).collect::<Vec<_>>(),
        i.races.iter().map(|r| r.0).collect::<Vec<_>>(),
        i.negates.iter().map(|s| s.0).collect::<Vec<_>>(),
        abilities(&i.abilities),
    ])
}

fn spell_row(s: &Spell, learnable: bool) -> Value {
    json!([
        s.number.0,
        s.level,
        s.energy,
        s.mana,
        s.minval,
        s.maxval,
        s.diff,
        s.resmode.code(),
        s.target.code(),
        s.duration,
        s.element.code(),
        s.magery.code(),
        s.magelvl,
        s.levelcap & 0xFF,
        s.maxlvls,
        s.maxinc,
        s.minlvls,
        s.mininc,
        s.durlvls,
        s.durinc,
        abilities(&s.abilities),
        learnable,
    ])
}

fn class_row(c: &Class) -> Value {
    json!([
        c.number.0,
        c.basehp,
        c.hplevel,
        c.exp,
        c.magery.code(),
        c.magelvl,
        c.weapon.code(),
        c.armour.code(),
        c.combat,
        abilities(&c.abilities),
    ])
}

fn race_row(r: &Race) -> Value {
    json!([
        r.number.0,
        [r.minstr, r.minint, r.minwil, r.minagi, r.minhea, r.mincha],
        [r.maxstr, r.maxint, r.maxwil, r.maxagi, r.maxhea, r.maxcha],
        r.hpbonus,
        r.exp,
        abilities(&r.abilities),
    ])
}

fn monster_row(m: &KnownMonster) -> Value {
    json!([
        m.number.0,
        m.ac,
        m.dr,
        m.magicres,
        m.bsdefense,
        m.maxhp,
        m.energy,
        m.undead,
        m.alignment.code(),
        abilities(&m.abilities),
    ])
}

pub fn family() -> Family {
    let data = Data::content().expect("the stock records with the 1.20 content");
    let kits = Kits::default_kits().expect("the tool's kits");
    let balance = Balance::V120;

    let mut items: BTreeMap<i32, Value> = BTreeMap::new();
    let mut spells: BTreeMap<i32, Value> = BTreeMap::new();
    let mut classes: BTreeMap<u16, Value> = BTreeMap::new();
    let mut races: BTreeMap<u16, Value> = BTreeMap::new();
    let mut monsters: BTreeMap<i32, Value> = BTreeMap::new();
    let mut cases = Vec::new();

    for (ci, class) in CLASSES.iter().enumerate() {
        for (li, &level) in LEVELS.iter().enumerate() {
            let mut built = None;
            for k in 0..RACES.len() {
                let race = RACES[(ci + li + k) % RACES.len()];
                let cell = Cell {
                    race,
                    class,
                    level,
                    kit: None,
                };
                if let Ok(pair) = sim::cell_builds(&data, &kits, &cell, &balance) {
                    built = Some((race, pair.0));
                    break;
                }
            }
            let (race_name, b) = built.unwrap_or_else(|| panic!("no race fits {class} L{level}"));

            let target = kits.target_for(level).expect("a target for the level");
            let known = data
                .monsters
                .get(&MonsterNo(target.monster))
                .expect("the target monster");
            let t = TargetMonster::new(&data, known);
            let best = sim::melee(&b, &t, &data, &balance, ROUNDS, sim::seed(&[1]));

            // the rows the character needs
            let mut worn: Vec<i32> = Vec::new();
            for i in b.worn.iter().copied().chain(b.weapon) {
                items.insert(i.number.0, item_row(i));
                if b.weapon.is_none_or(|w| w.number != i.number) {
                    worn.push(i.number.0);
                }
                // a spell an item casts (ability 43, "Cast spell") deals damage in the round
                for (a, v) in &i.abilities {
                    if a.code() == 43 {
                        if let Some(s) = data.spells.get(&silvermere_types::ids::SpellNo(*v)) {
                            spells.entry(i32::from(s.number.0)).or_insert_with(|| spell_row(s, false));
                        }
                    }
                }
            }
            let mut buffs = Vec::new();
            for x in &b.buffs {
                spells.entry(i32::from(x.spell.number.0)).or_insert_with(|| spell_row(x.spell, false));
                buffs.push(x.spell.number.0);
            }
            // the weave picks from every spell the class can cast at the level
            for s in quicksilver_balance::character::known_spells(
                &data,
                b.class,
                level,
                b.character.evil,
            ) {
                spells.insert(i32::from(s.number.0), spell_row(s, true));
            }
            classes.insert(b.class.number.0, class_row(b.class));
            races.insert(b.race.number.0, race_row(b.race));
            monsters.insert(known.number.0, monster_row(known));

            let c = &b.character;
            let striker = quicksilver_balance::fight::Striker::new(&b, &t, &data, &balance);
            cases.push(json!({
                "id": format!("cell-{:02}-{}-{}-L{:02}", ci, class, race_name, level),
                "kind": "cell",
                "inputs": {
                    "class": b.class.number.0,
                    "class_name": class,
                    "race": b.race.number.0,
                    "race_name": race_name,
                    "level": level,
                    "kit_level": b.kit_level,
                    "stats": [b.stats.str, b.stats.int, b.stats.wil, b.stats.agi, b.stats.hea, b.stats.cha],
                    "weapon": b.weapon.map_or(0, |w| w.number.0),
                    "worn": worn,
                    "buffs": buffs,
                    "target": known.number.0,
                },
                "tool": {
                    "move": format!("{:?}", best.mv),
                    "dpr": best.dpr,
                    "weave_dpr": best.weave_dpr,
                    "weapon_dpr": best.weapon_dpr,
                    "blows_per_round": best.blows_per_round,
                    "hit_pct": best.hit_pct,
                    "energy": best.energy,
                    "av": striker.fighter(best.mv, best.mv).av,
                    "hp": c.maxhp,
                    "mana": c.maxmana,
                    "stealth": c.stealth,
                    "crit_bonus": c.critbonus,
                    "dmg_bonus": c.dmgbonus,
                    "av_bonus": c.avbonus,
                },
            }));
        }
    }

    cases.push(json!({
        "id": "tables",
        "kind": "tables",
        "classes": classes.values().collect::<Vec<_>>(),
        "items": items.values().collect::<Vec<_>>(),
        "monsters": monsters.values().collect::<Vec<_>>(),
        "races": races.values().collect::<Vec<_>>(),
        "spells": spells.values().collect::<Vec<_>>(),
    }));

    Family {
        name: "cells",
        about: "Whole characters from the balance tool (tools/balance): one race, class and level in the \
                tool's melee kit with its standing buffs against the band's reference monster. A `cell` \
                case has `inputs` (class, race, level, trained stats in the order Str Int Wil Agl Hea Cha, \
                the wielded weapon and worn item numbers, the buff spell numbers, the target monster \
                number) and `tool`: the best move's damage a round at the steady state (`dpr`, with \
                `weave_dpr` the part the weave dealt and `weapon_dpr` the part the weapon's spells \
                dealt), blows a round, hit chance in percent, the energy a blow costs, the attack value, and \
                the tool's hit points, mana, stealth and bonuses for the cross-checks. The `tables` case \
                holds the rows those numbers name. Item row: number, weight, type, ac, dr, weapon type, \
                armour type, worn, accuracy, strength required, speed, min, max, classes, races, negated \
                spells, abilities as [ability, value]. Spell row: number, level, energy, mana, min, max, \
                diff, resist mode, target, duration, element, magery, magery level, cap, max levels, max \
                increment, min levels, min increment, duration levels, duration increment, abilities, whether a class can learn it. \
                Class row: number, base hp, hp per level, exp, magery, magery level, weapon class, armour \
                type, combat, abilities. Race row: number, minimum stats, maximum stats (Str Int Wil Agl \
                Hea Cha), hp bonus, exp, abilities. Monster row: number, ac, dr, magic resistance, \
                backstab defence, hp, energy, undead, alignment, abilities.",
        keys: json!({}),
        cases,
    }
}
