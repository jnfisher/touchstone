//! `Monsters.summoned_by`: where a monster comes from.
//!
//! Three kinds of entry are reproduced, in NMR's order: `Room m/r` (the room's specific
//! monster), the rooms whose generation range covers the monster (`Group: m/r`, or
//! `[group-min-max][max]Group(lair): m/r` for a lair room) and `Spell #n` (a summon spell).
//! Then `Textblock #n(p%)` for scripts that summon the monster, in the order they were read
//! (`p` is the line's own share when the line is a random line).
use std::collections::HashMap;

use touchstone_model::WorldParts;

use super::history::{Field, History};
use super::lairs::pool;
use super::scripts::Scripts;
use super::source::{Source, clip};

/// Column width of `Monsters.Summoned By`.
const WIDTH: usize = 20000;
/// Spell ability that summons a monster.
const ABILITY_SUMMON: i32 = 12;
const ROOM_LAIR: i64 = 3;
/// A base range wider than this is no list of monsters (guards a corrupt spell row).
const MAX_SUMMON_RANGE: i32 = 100;

pub(super) fn apply(src: &Source, parts: &mut WorldParts, sc: &Scripts, hist: &mut History) {
    let mut specific: HashMap<i32, Vec<(i32, i32)>> = HashMap::new();
    for r in &parts.rooms {
        if r.npc > 0 {
            specific
                .entry(r.npc)
                .or_default()
                .push((r.map_number, r.room_number));
        }
    }
    for v in specific.values_mut() {
        v.sort_by_key(|&(map, room)| (room, map));
    }

    // Entries of each monster in room order (room number, then map), as the .mdb lists them.
    let mut rooms: Vec<_> = src.rooms.iter().filter(|r| r.max > 0).collect();
    rooms.sort_by_key(|r| (r.room, r.map));
    let mut pools: HashMap<(i64, i64, i64), Vec<i32>> = HashMap::new();
    let mut groups: HashMap<i32, Vec<String>> = HashMap::new();
    for r in rooms {
        let members = pools
            .entry((r.group, r.min, r.max))
            .or_insert_with(|| pool(&src.known, r, false));
        let entry = if r.room_type == ROOM_LAIR {
            format!(
                "[{}-{}-{}][{}]Group(lair): {}/{}",
                r.group, r.min, r.max, r.max_monsters, r.map, r.room
            )
        } else {
            format!("Group: {}/{}", r.map, r.room)
        };
        for &m in members.iter() {
            groups.entry(m).or_default().push(entry.clone());
        }
    }

    let mut spells: HashMap<i32, Vec<i32>> = HashMap::new();
    // Only spells that are part of the game summon anything.
    let mut by_number: Vec<_> = parts
        .spells
        .iter()
        .filter(|s| super::scripts::spell_in_game(s))
        .collect();
    by_number.sort_by_key(|s| s.number);
    for s in by_number {
        for a in s.abilities.iter().filter(|a| a.ability == ABILITY_SUMMON) {
            // The monster is the ability's value, or the spell's base values when that is 0: a
            // base range summons any monster in it.
            let range = if a.value > 0 {
                a.value..=a.value
            } else if s.min_base > 0 && s.max_base >= s.min_base {
                s.min_base..=s.max_base.min(s.min_base + MAX_SUMMON_RANGE)
            } else {
                1..=0
            };
            for monster in range {
                let list = spells.entry(monster).or_default();
                if !list.contains(&s.number) {
                    list.push(s.number);
                }
            }
        }
    }

    let mut texts: HashMap<i32, Vec<String>> = HashMap::new();
    for &(monster, block, pct) in &sc.fx.summons {
        let p = pct.map_or_else(String::new, |p| format!("({p}%)"));
        texts
            .entry(monster)
            .or_default()
            .push(format!("Textblock #{block}{p}"));
    }

    for m in &mut parts.monsters {
        let mut kinds: Vec<String> = Vec::new();
        if let Some(v) = specific.get(&m.number) {
            kinds.push(
                v.iter()
                    .map(|(map, room)| format!("Room {map}/{room}"))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        if let Some(v) = groups.get(&m.number) {
            kinds.push(v.join(","));
        }
        if let Some(v) = spells.get(&m.number) {
            kinds.extend(v.iter().map(|n| format!("Spell #{n}")));
        }
        if let Some(v) = texts.get(&m.number) {
            kinds.extend(v.iter().cloned());
        }
        let kinds = hist.order(Field::SummonedBy, m.number, kinds);
        m.summoned_by = if kinds.is_empty() {
            None
        } else {
            Some(clip(kinds.join(", "), WIDTH))
        };
    }
}
