//! `Spells.learned_from`, `casted_by`, `classes` and `learnable`.
//!
//! Learned from: scroll-like items with the teach-spell ability. Cast by: monsters (mid
//! spells, hit spells, spell attacks, death and create spells), rooms with an occupant spell,
//! and spells that cast it. Entries that come from text-block scripts (`NPC #n`,
//! `Textblock #n(p%)`) are not reproduced (docs/data-gaps.md).
use std::collections::{BTreeSet, HashMap};

use touchstone_model::WorldParts;

use super::source::clip;

/// Item ability that teaches the spell in its value.
const ABILITY_TEACH: i32 = 42;
/// Spell ability that casts the spell in its value.
const ABILITY_CAST: i32 = 151;
/// Column width of `Spells.Casted By`.
const WIDTH: usize = 250;

pub(super) fn apply(parts: &mut WorldParts) {
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
        let mut cast = vec![m.death_spell, m.create_spell];
        cast.extend(m.mid_spells.iter().map(|s| s.spell));
        for a in &m.attacks {
            cast.push(a.hit_spell);
            if a.att_type == 2 {
                cast.push(a.acc);
            }
        }
        for s in cast.into_iter().filter(|&s| s > 0) {
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
    for v in rooms.values_mut() {
        v.sort_by_key(|&(map, room)| (room, map));
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
        let taught = teachers.get(&s.number);
        s.learnable = taught.is_some();
        s.learned_from = taught.map(|v| {
            v.iter()
                .map(|(n, _)| format!("Item #{n}"))
                .collect::<Vec<_>>()
                .join(", ")
        });
        // "(*)" when any teaching item is open to every class, else the classes it names.
        s.classes = taught.map(|v| {
            if v.iter().any(|(_, c)| c.iter().all(|&x| x == 0)) {
                "(*)".to_string()
            } else {
                let all: BTreeSet<i32> = v
                    .iter()
                    .flat_map(|(_, c)| c.iter().copied().filter(|&x| x > 0))
                    .collect();
                let list: Vec<String> = all.iter().map(i32::to_string).collect();
                format!("({})", list.join(","))
            }
        });

        let mut entries: Vec<String> = Vec::new();
        entries.extend(
            monsters
                .get(&s.number)
                .into_iter()
                .flatten()
                .map(|m| format!("Monster #{m}")),
        );
        entries.extend(
            rooms
                .get(&s.number)
                .into_iter()
                .flatten()
                .map(|(map, room)| format!("Room {map}/{room}")),
        );
        entries.extend(
            casters
                .get(&s.number)
                .into_iter()
                .flatten()
                .map(|n| format!("Spell #{n}")),
        );
        s.casted_by = if entries.is_empty() {
            None
        } else {
            Some(clip(entries.join(", "), WIDTH))
        };
    }
}
