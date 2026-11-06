//! `Spells.learned_from`, `casted_by`, `classes` and `learnable`.
//!
//! Learned from: scroll-like items with the teach-spell ability, then `learnspell` clauses of
//! scripts (`NPC #n` when a monster's ask tree reaches them, else `Textblock #n`). Cast by:
//! rooms (an occupant spell or a casting exit), monsters (mid spells, hit spells, spell attacks,
//! death and create spells), spells that cast it, `cast` clauses of scripts and items that cast
//! it, in that order.
use std::collections::{BTreeSet, HashMap};

use touchstone_model::{Monster, WorldParts};

use super::history::{Field, History};
use super::scripts::Scripts;
use super::source::{Source, clip};
use super::walk::Origin;

/// Item ability that teaches the spell in its value.
const ABILITY_TEACH: i32 = 42;
/// Spell ability that casts the spell in its value.
const ABILITY_CAST: i32 = 151;
/// Item ability that casts the spell in its value.
const ABILITY_ITEM_CAST: i32 = 43;
/// Column width of `Spells.Casted By`.
const WIDTH: usize = 250;

/// The spells a monster casts: death, create, mid spells, hit spells and spell attacks.
pub(super) fn monster_casts(m: &Monster) -> Vec<i32> {
    let mut cast = vec![m.death_spell, m.create_spell];
    cast.extend(m.mid_spells.iter().map(|s| s.spell));
    for a in &m.attacks {
        cast.push(a.hit_spell);
        if a.att_type == 2 {
            cast.push(a.acc);
        }
    }
    cast.retain(|&s| s > 0);
    cast
}

fn percent(p: Option<i64>) -> String {
    p.map_or_else(String::new, |p| format!("({p}%)"))
}

pub(super) fn apply(src: &Source, parts: &mut WorldParts, sc: &Scripts, hist: &mut History) {
    // Only items that can be obtained teach anything.
    let mut teachers: HashMap<i32, Vec<(i32, [i32; 10])>> = HashMap::new();
    let mut items: Vec<_> = parts
        .items
        .iter()
        .filter(|i| i.obtained_from.is_some())
        .collect();
    items.sort_by_key(|i| i.number);
    for i in items {
        for a in i.abilities.iter().filter(|a| a.ability == ABILITY_TEACH) {
            teachers
                .entry(a.value)
                .or_default()
                .push((i.number, i.class_rest));
        }
    }

    let mut monsters: HashMap<i32, BTreeSet<i32>> = HashMap::new();
    for m in parts.monsters.iter().filter(|m| m.summoned_by.is_some()) {
        for s in monster_casts(m) {
            monsters.entry(s).or_default().insert(m.number);
        }
    }

    let mut rooms: HashMap<i32, Vec<(i32, i32)>> = HashMap::new();
    for r in &parts.rooms {
        if r.spell > 0 {
            rooms
                .entry(r.spell)
                .or_default()
                .push((r.map_number, r.room_number));
        }
    }
    for &(spell, map, room) in &src.exit_spells {
        rooms.entry(spell).or_default().push((map, room));
    }
    for v in rooms.values_mut() {
        v.sort_by_key(|&(map, room)| (room, map));
        v.dedup();
    }

    // Items that can be obtained and cast the spell when used.
    let mut item_casts: HashMap<i32, BTreeSet<i32>> = HashMap::new();
    for i in parts.items.iter().filter(|i| i.obtained_from.is_some()) {
        for a in i
            .abilities
            .iter()
            .filter(|a| a.ability == ABILITY_ITEM_CAST)
        {
            item_casts.entry(a.value).or_default().insert(i.number);
        }
    }

    // Script teaching: `(NPC or block, classes of the line)` in the order the scripts were read.
    let mut taught_by_script: HashMap<i32, Vec<(String, Vec<i32>)>> = HashMap::new();
    for (spell, block, origin, classes) in &sc.fx.learns {
        let who = match origin {
            Origin::Monster(m) => format!("NPC #{m}"),
            _ => format!("Textblock #{block}"),
        };
        let v = taught_by_script.entry(*spell).or_default();
        match v.iter_mut().find(|(w, _)| *w == who) {
            Some((_, c)) => c.extend(classes.iter().copied()),
            None => v.push((who, classes.clone())),
        }
    }

    let mut script_casts: HashMap<i32, Vec<String>> = HashMap::new();
    for &(spell, block, pct) in &sc.fx.casts {
        script_casts
            .entry(spell)
            .or_default()
            .push(format!("Textblock #{block}{}", percent(pct)));
    }

    let mut casters: HashMap<i32, BTreeSet<i32>> = HashMap::new();
    for s in &parts.spells {
        for a in s.abilities.iter().filter(|a| a.ability == ABILITY_CAST) {
            if a.value > 0 {
                casters.entry(a.value).or_default().insert(s.number);
            }
        }
    }

    for s in &mut parts.spells {
        let by_item = teachers.get(&s.number);
        let by_script = taught_by_script.get(&s.number);
        s.learnable = by_item.is_some() || by_script.is_some();
        let mut from: Vec<String> = by_item
            .into_iter()
            .flatten()
            .map(|(n, _)| format!("Item #{n}"))
            .collect();
        from.extend(by_script.into_iter().flatten().map(|(w, _)| w.clone()));
        let from = hist.order(Field::LearnedFrom, s.number, from);
        s.learned_from = if from.is_empty() {
            None
        } else {
            Some(from.join(", "))
        };
        // "(*)" when any teacher is open to every class, else the classes they name.
        let mut sets: Vec<Vec<i32>> = by_item
            .into_iter()
            .flatten()
            .map(|(_, c)| c.iter().copied().filter(|&x| x > 0).collect())
            .collect();
        sets.extend(by_script.into_iter().flatten().map(|(_, c)| c.clone()));
        s.classes = if sets.is_empty() {
            None
        } else if sets.iter().any(Vec::is_empty) {
            Some("(*)".to_string())
        } else {
            let all: BTreeSet<i32> = sets.iter().flatten().copied().collect();
            let list: Vec<String> = all.iter().map(i32::to_string).collect();
            Some(format!("({})", list.join(",")))
        };

        let mut entries: Vec<String> = Vec::new();
        entries.extend(
            rooms
                .get(&s.number)
                .into_iter()
                .flatten()
                .map(|(map, room)| format!("Room {map}/{room}")),
        );
        entries.extend(
            monsters
                .get(&s.number)
                .into_iter()
                .flatten()
                .map(|m| format!("Monster #{m}")),
        );
        entries.extend(
            casters
                .get(&s.number)
                .into_iter()
                .flatten()
                .map(|n| format!("Spell #{n}")),
        );
        entries.extend(script_casts.get(&s.number).into_iter().flatten().cloned());
        entries.extend(
            item_casts
                .get(&s.number)
                .into_iter()
                .flatten()
                .map(|n| format!("Item #{n}")),
        );
        let entries = hist.order(Field::CastedBy, s.number, entries);
        s.casted_by = if entries.is_empty() {
            None
        } else {
            Some(clip(entries.join(", "), WIDTH))
        };
    }
}
