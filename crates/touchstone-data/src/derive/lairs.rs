//! The `Lairs` table and `Rooms.lair`.
//!
//! A lair room is a room of type 3 with a generation level range; the game fills it from the
//! known monsters of its group (`mongroup` = monster type) whose generation index lies in the
//! range. NMR writes that pool as `(Max n): 1,2,[group-min-max-n]` on the room and keeps one
//! `Lairs` row per distinct `group-min-max`.
use std::collections::{BTreeMap, HashMap};

use touchstone_model::{Lair, WorldParts};

use super::source::{GenRoom, Known, Source};
use crate::silvermere::Result;

/// `room_type` of a lair room (silvermere's `ROOM_LAIR`).
const ROOM_LAIR: i64 = 3;
/// The ability number of Dodge (checked against the stock `Lairs.AvgDodge`).
const ABILITY_DODGE: i32 = 34;
/// At most this many monsters of one type and generation index are listed. The stock data has
/// one pool with 18 such monsters and NMR lists the first 15 (the game's per-room monster
/// cap); no other pool in the stock data reaches the limit, so this is inferred from one case.
const PER_INDEX_CAP: usize = 15;

/// The monsters a room can generate, in NMR's order: by generation index, then number. The
/// lair text and table list at most [`PER_INDEX_CAP`] per type and index; `capped` false gives
/// the whole pool (what a monster's `summoned_by` uses).
pub(super) fn pool(known: &[Known], room: &GenRoom, capped: bool) -> Vec<i32> {
    let mut found: Vec<&Known> = known
        .iter()
        .filter(|k| {
            (room.group == -1 || k.montype == room.group)
                && (room.min == 0 || k.genindex >= room.min)
                && k.genindex <= room.max
        })
        .collect();
    found.sort_by_key(|k| (k.genindex, k.number));
    let mut per_index: HashMap<(i64, i64), usize> = HashMap::new();
    found
        .into_iter()
        .filter(|k| {
            let n = per_index.entry((k.montype, k.genindex)).or_default();
            *n += 1;
            !capped || *n <= PER_INDEX_CAP
        })
        .map(|k| k.number)
        .collect()
}

/// The pool a group lists in the table and the rooms (map, room) that belong to it.
type Group = (Vec<i32>, Vec<(i32, i32)>);

fn join(numbers: &[i32]) -> String {
    numbers
        .iter()
        .map(i32::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// Round half to even, as VB6 `CInt` does.
fn round(x: f64) -> f64 {
    x.round_ties_even()
}

pub(super) fn apply(src: &Source, parts: &mut WorldParts) -> Result<()> {
    let rooms: Vec<&GenRoom> = src
        .rooms
        .iter()
        .filter(|r| r.room_type == ROOM_LAIR && r.max > 0)
        .collect();
    let known = &src.known;
    let by_number: HashMap<i32, usize> = parts
        .monsters
        .iter()
        .enumerate()
        .map(|(i, m)| (m.number, i))
        .collect();
    let room_at: HashMap<(i32, i32), usize> = parts
        .rooms
        .iter()
        .enumerate()
        .map(|(i, r)| ((r.map_number, r.room_number), i))
        .collect();

    // group index -> (pool without regenerating uniques, lair rooms of the group)
    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    for gr in rooms {
        let listed = pool(known, gr, true);
        let key = format!("{}-{}-{}", gr.group, gr.min, gr.max);
        if let Some(&ri) = room_at.get(&(gr.map, gr.room)) {
            // The room text lists the whole pool, uniques included; the table below leaves out
            // monsters with a regeneration time.
            parts.rooms[ri].lair = Some(format!(
                "(Max {}): {},[{}-{}]",
                gr.max_monsters,
                join(&listed),
                key,
                gr.max_monsters
            ));
        }
        let entry = groups.entry(key).or_insert_with(|| {
            let regular = listed
                .iter()
                .copied()
                .filter(|n| {
                    by_number
                        .get(n)
                        .is_none_or(|&i| parts.monsters[i].regen_time == 0)
                })
                .collect();
            (regular, Vec::new())
        });
        entry.1.push((gr.map, gr.room));
    }

    let mut lairs = Vec::new();
    for (key, (mobs, at)) in groups {
        // A pool made only of uniques has no row.
        if mobs.is_empty() {
            continue;
        }
        let ms: Vec<_> = mobs
            .iter()
            .filter_map(|n| by_number.get(n).map(|&i| &parts.monsters[i]))
            .collect();
        let mean = |f: &dyn Fn(&touchstone_model::Monster) -> f64| -> f64 {
            if ms.is_empty() {
                0.0
            } else {
                ms.iter().map(|m| f(m)).sum::<f64>() / ms.len() as f64
            }
        };
        let delays: f64 = at
            .iter()
            .filter_map(|k| room_at.get(k))
            .map(|&i| f64::from(parts.rooms[i].delay))
            .sum();
        lairs.push(Lair {
            group_index: key,
            mob_list: join(&mobs),
            mobs: mobs.len() as i32,
            total_lairs: at.len() as i32,
            avg_delay: round(delays / at.len() as f64) as i32,
            // NMR's walking distance has no recoverable rule (docs/data-gaps.md).
            avg_walk: 0.0,
            avg_exp: round(mean(&|m| m.exp * m.exp_multi)),
            // The damage averages are those of the monsters, which phase-3 rules fill in;
            // they read 0 until then.
            avg_dmg: round(mean(&|m| m.avg_dmg)),
            avg_dmg_phys: round(mean(&|m| m.avg_dmg_phys)),
            avg_dmg_spell: round(mean(&|m| m.avg_dmg_spell)),
            avg_hp: round(mean(&|m| f64::from(m.hp))),
            avg_ac: round(mean(&|m| f64::from(m.armour_class))) as i32,
            avg_dr: round(mean(&|m| f64::from(m.damage_resist))) as i32,
            avg_mr: round(mean(&|m| f64::from(m.magic_res))) as i32,
            avg_dodge: round(mean(&|m| {
                m.abilities
                    .iter()
                    .filter(|a| a.ability == ABILITY_DODGE)
                    .map(|a| f64::from(a.value))
                    .sum()
            })) as i32,
        });
    }
    // The stock table is in text order with the hyphens ignored ("10-10-10" before "10-1-1").
    lairs.sort_by_key(|l| l.group_index.replace('-', ""));
    parts.lairs = lairs;
    Ok(())
}
