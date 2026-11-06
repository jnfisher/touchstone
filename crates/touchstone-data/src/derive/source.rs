//! The silvermere columns the derivations need and the model has no field for.
use std::collections::HashMap;

use rusqlite::Connection;

use super::script::{Block, Blocks, has_action};
use crate::silvermere::Result;

/// A room's generation settings (silvermere `rooms`).
pub(super) struct GenRoom {
    pub map: i32,
    pub room: i32,
    pub room_type: i64,
    /// `mongroup`: the monster type the room generates from (-1 any).
    pub group: i64,
    /// `minlevel`, `maxlevel`: the generation index range (0 = open below; a room with a
    /// `maxlevel` of 0 generates from the pool of nothing).
    pub min: i64,
    pub max: i64,
    pub max_monsters: i64,
}

/// A known monster's generation class (silvermere `monsters`).
pub(super) struct Known {
    pub number: i32,
    pub montype: i64,
    pub genindex: i64,
}

/// Exit types that cast a spell when the room is entered: Cast (22, spell in the second
/// parameter) and Spell Trap (24, spell in the first).
const EXIT_CAST: i64 = 22;
const EXIT_SPELL_TRAP: i64 = 24;

pub(super) struct Source {
    /// The text blocks by number.
    pub blocks: Blocks,
    /// `(spell, map, room)` of every room exit that casts a spell, ordered by room, map.
    pub exit_spells: Vec<(i32, i32, i32)>,
    /// Ordered by map, room.
    pub rooms: Vec<GenRoom>,
    /// Ordered by number.
    pub known: Vec<Known>,
    /// `(item, map, room)` of every item placed in a room or hidden in it, each once.
    pub room_items: Vec<(i32, i32, i32)>,
}

impl Source {
    pub(super) fn read(conn: &Connection) -> Result<Self> {
        let mut stmt = conn.prepare(
            "SELECT map, room, room_type, mongroup, minlevel, maxlevel, maxmonsters FROM rooms \
             ORDER BY map, room",
        )?;
        let rooms = stmt
            .query_map([], |r| {
                Ok(GenRoom {
                    map: r.get(0)?,
                    room: r.get(1)?,
                    room_type: r.get(2)?,
                    group: r.get(3)?,
                    min: r.get(4)?,
                    max: r.get(5)?,
                    max_monsters: r.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut stmt =
            conn.prepare("SELECT number, montype, genindex FROM monsters ORDER BY number")?;
        let known = stmt
            .query_map([], |r| {
                Ok(Known {
                    number: r.get(0)?,
                    montype: r.get(1)?,
                    genindex: r.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        // Visible floor items are live state and not a source; the placed and hidden ones are.
        let mut stmt = conn.prepare(
            "SELECT DISTINCT item, map, room FROM ( \
               SELECT p.item AS item, r.map AS map, r.room AS room FROM room_placed_items p \
                 JOIN rooms r ON r.id = p.room_id \
               UNION ALL \
               SELECT f.item, r.map, r.room FROM room_floor_items f \
                 JOIN rooms r ON r.id = f.room_id WHERE f.hidden = 1) \
             WHERE item > 0",
        )?;
        let room_items = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut stmt =
            conn.prepare("SELECT number, part, link, text FROM text_blocks ORDER BY number, part")?;
        let mut blocks: Blocks = HashMap::new();
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i32>(0)?,
                r.get::<_, i32>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            let (number, link, text) = row?;
            // The link of the first part is the block's; later parts only continue the text.
            let b = blocks.entry(number).or_insert_with(|| Block {
                link,
                text: String::new(),
                acts: false,
            });
            b.text.push_str(&text);
        }
        for b in blocks.values_mut() {
            b.acts = has_action(&b.text);
        }
        let mut stmt = conn.prepare(
            "SELECT CASE e.exit_type WHEN ?1 THEN e.par2 ELSE e.par1 END, r.map, r.room \
             FROM room_exits e JOIN rooms r ON r.id = e.room_id \
             WHERE e.exit_type IN (?1, ?2) \
               AND (CASE e.exit_type WHEN ?1 THEN e.par2 ELSE e.par1 END) > 0 \
             ORDER BY r.room, r.map",
        )?;
        let exit_spells = stmt
            .query_map([EXIT_CAST, EXIT_SPELL_TRAP], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(Self {
            blocks,
            exit_spells,
            rooms,
            known,
            room_items,
        })
    }
}

/// Cut text to a column's width the way NMR does: a value over `max` characters keeps its
/// first `max - 1` and ends in `+`.
pub(super) fn clip(mut s: String, max: usize) -> String {
    if s.len() > max {
        s.truncate(max - 1);
        s.push('+');
    }
    s
}
