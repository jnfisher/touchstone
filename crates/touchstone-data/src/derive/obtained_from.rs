//! `Items.obtained_from`: the rooms, shops and monsters that yield an item, in NMR's order
//! (rooms, shops by number, monsters by number), then the scripts that give it (`Textblock
//! #n(p%)`, in the order they were read) and the monsters whose ask tree does (`NPC #n`).
//! `Item #n(p%)` (a chest that opens into it) is not reproduced (docs/data-gaps.md).
use std::collections::HashMap;

use touchstone_model::WorldParts;

use super::scripts::Scripts;
use super::source::{Source, clip};
use super::walk::Origin;

/// Column width of `Items.Obtained From`.
const WIDTH: usize = 1996;

pub(super) fn apply(src: &Source, parts: &mut WorldParts, sc: &Scripts) {
    let mut rooms: HashMap<i32, Vec<(i32, i32)>> = HashMap::new();
    for &(item, map, room) in &src.room_items {
        rooms.entry(item).or_default().push((map, room));
    }
    for v in rooms.values_mut() {
        v.sort_by_key(|&(map, room)| (room, map));
    }

    // Shops that are in the game, by number. A slot with no quantity is a shop that only
    // buys the item; one that never restocks it is "nogen".
    let mut shops: HashMap<i32, Vec<String>> = HashMap::new();
    let mut by_number: Vec<_> = parts.shops.iter().filter(|s| s.in_game).collect();
    by_number.sort_by_key(|s| s.number);
    for s in by_number {
        for slot in s.stock.iter().filter(|x| x.item > 0) {
            let kind = if slot.max == 0 {
                "Shop(sell)"
            } else if slot.amount > 0 && slot.pct > 0 {
                "Shop"
            } else {
                "Shop(nogen)"
            };
            let entry = format!("{kind} #{}", s.number);
            let list = shops.entry(slot.item).or_default();
            if !list.contains(&entry) {
                list.push(entry);
            }
        }
    }

    // Only monsters that are in the game (summoned somewhere) count.
    let mut monsters: HashMap<i32, Vec<String>> = HashMap::new();
    let mut by_number: Vec<_> = parts
        .monsters
        .iter()
        .filter(|m| m.summoned_by.is_some())
        .collect();
    by_number.sort_by_key(|m| m.number);
    for m in by_number {
        for d in m.drops.iter().filter(|d| d.item > 0) {
            monsters
                .entry(d.item)
                .or_default()
                .push(format!("Monster #{}({}%)", m.number, d.pct));
        }
    }

    // Items given by scripts: a block's own line, or the monster whose ask tree reached it.
    let mut texts: HashMap<i32, Vec<String>> = HashMap::new();
    let mut npcs: HashMap<i32, Vec<String>> = HashMap::new();
    for &(item, block, pct, origin) in &sc.fx.gives {
        match origin {
            Origin::Monster(m) => {
                let list = npcs.entry(item).or_default();
                let e = format!("NPC #{m}");
                if !list.contains(&e) {
                    list.push(e);
                }
            }
            _ => {
                let p = pct.map_or_else(String::new, |p| format!("({p}%)"));
                texts
                    .entry(item)
                    .or_default()
                    .push(format!("Textblock #{block}{p}"));
            }
        }
    }

    for it in &mut parts.items {
        let mut entries: Vec<String> = Vec::new();
        if let Some(v) = rooms.get(&it.number) {
            entries.extend(v.iter().map(|(map, room)| format!("Room {map}/{room}")));
        }
        entries.extend(shops.remove(&it.number).unwrap_or_default());
        entries.extend(monsters.remove(&it.number).unwrap_or_default());
        entries.extend(texts.remove(&it.number).unwrap_or_default());
        entries.extend(npcs.remove(&it.number).unwrap_or_default());
        it.obtained_from = if entries.is_empty() {
            None
        } else {
            Some(clip(entries.join(", "), WIDTH))
        };
    }
}
