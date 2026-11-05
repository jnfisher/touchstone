//! The silvermere columns the derivations need and the model has no field for.
use rusqlite::Connection;

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

pub(super) struct Source {
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
        Ok(Self {
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
