//! `TBInfo`: one row for every text block the game can reach, with the block's script text in
//! `action` when it is read as a script, and what points at it in `called_from`.
//!
//! Rooms come first (`Room m/r`, by room then map), then monsters in the game, spells in the
//! game (`Spell #n`) and the blocks that call it (`Textblock #n`, `Textblock(rndm) #n` for a
//! `random` clause, `(p%)` for a random line), cut to the column's 250 characters.
use std::collections::{BTreeMap, HashMap, HashSet};

use touchstone_model::{TbInfo, WorldParts};

use super::history::{Field, History};
use super::scripts::{Scripts, spell_blocks, spell_in_game};
use super::source::{Source, clip};

/// Column width of `TBInfo.Called From`: a longer list keeps its first 248 characters and a `+`.
const WIDTH: usize = 249;

/// The rows' numbers and what calls each of them, before ordering.
pub(super) fn entries(
    src: &Source,
    parts: &WorldParts,
    sc: &Scripts,
) -> BTreeMap<i32, Vec<String>> {
    let mut rooms: HashMap<i32, Vec<(i32, i32)>> = HashMap::new();
    for r in parts.rooms.iter().filter(|r| r.cmd > 0) {
        rooms
            .entry(r.cmd)
            .or_default()
            .push((r.map_number, r.room_number));
    }
    for v in rooms.values_mut() {
        v.sort_by_key(|&(map, room)| (room, map));
    }
    let mut monsters: HashMap<i32, Vec<i32>> = HashMap::new();
    let mut by_number: Vec<_> = parts
        .monsters
        .iter()
        .filter(|m| m.summoned_by.is_some() && m.greet_txt > 0)
        .collect();
    by_number.sort_by_key(|m| m.number);
    for m in by_number {
        monsters.entry(m.greet_txt).or_default().push(m.number);
    }
    let mut spells: HashMap<i32, Vec<i32>> = HashMap::new();
    let mut by_number: Vec<_> = parts.spells.iter().filter(|s| spell_in_game(s)).collect();
    by_number.sort_by_key(|s| s.number);
    for s in by_number {
        for b in spell_blocks(s) {
            spells.entry(b).or_default().push(s.number);
        }
    }

    // Every row: what was read or shown, and the roots that exist.
    let mut numbers: HashSet<i32> = sc
        .walk
        .scripts
        .iter()
        .chain(&sc.walk.shown)
        .copied()
        .collect();
    numbers.extend(
        rooms
            .keys()
            .chain(monsters.keys())
            .chain(spells.keys())
            .copied(),
    );
    let rows: BTreeMap<i32, ()> = numbers
        .into_iter()
        .filter(|n| src.blocks.contains_key(n) && *n != 0)
        .map(|n| (n, ()))
        .collect();

    rows.into_keys()
        .map(|number| {
            let mut entries: Vec<String> = Vec::new();
            for (map, room) in rooms.get(&number).into_iter().flatten() {
                entries.push(format!("Room {map}/{room}"));
            }
            for m in monsters.get(&number).into_iter().flatten() {
                entries.push(format!("Monster #{m}"));
            }
            for s in spells.get(&number).into_iter().flatten() {
                entries.push(format!("Spell #{s}"));
            }
            for c in sc.walk.called.get(&number).into_iter().flatten() {
                let kind = if c.random {
                    "Textblock(rndm)"
                } else {
                    "Textblock"
                };
                let pct = c.pct.map_or_else(String::new, |p| format!("({p}%)"));
                entries.push(format!("{kind} #{}{pct}", c.block));
            }
            (number, entries)
        })
        .collect()
}

/// Note this pass's entries in the history.
pub(super) fn observe(src: &Source, parts: &WorldParts, sc: &Scripts, hist: &mut History) {
    for (number, list) in entries(src, parts, sc) {
        hist.order(Field::CalledFrom, number, list);
    }
}

pub(super) fn apply(src: &Source, parts: &mut WorldParts, sc: &Scripts, hist: &mut History) {
    let rows = entries(src, parts, sc);
    parts.tbinfo = rows
        .into_iter()
        .map(|(number, list)| {
            let block = &src.blocks[&number];
            TbInfo {
                number,
                link_to: block.link,
                action: sc
                    .walk
                    .scripts
                    .contains(&number)
                    .then(|| block.text.clone()),
                called_from: clip(
                    hist.order(Field::CalledFrom, number, list).join(", "),
                    WIDTH,
                ),
            }
        })
        .collect();
}
