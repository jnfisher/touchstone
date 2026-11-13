//! Where to get an item: its Obtained From line, read into monsters to kill, chests to open,
//! shops to buy from and the places and people that hand it out.
//!
//! touchstone's own reading of a column MME only prints (`docs/differences.md`, "Gear planner").
//! The entries are NMR's: `Room m/r`, `Shop #n`, `Shop(nogen) #n` (stocked, never restocked),
//! `Shop(sell) #n` (the shop only buys it, so it is no source), `Monster #n(p%)`,
//! `Textblock #b(p%)` (a script gives it), `Item #n(p%)` (a chest opens into it) and `NPC #m`.

use serde::Serialize;
use touchstone_model::World;

/// One entry of an Obtained From line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Entry {
    Room {
        map: i32,
        room: i32,
    },
    Shop {
        number: i32,
        restocks: bool,
    },
    /// The shop buys the item and never sells it.
    ShopBuys {
        number: i32,
    },
    Monster {
        number: i32,
        pct: f64,
    },
    Script {
        number: i32,
        pct: Option<f64>,
    },
    Chest {
        item: i32,
        pct: f64,
    },
    Npc {
        number: i32,
    },
}

/// `name #n(p%)` or `name #n`: the number and the percentage, if any.
fn numbered(rest: &str) -> Option<(i32, Option<f64>)> {
    let rest = rest.trim().strip_prefix('#')?;
    let (num, pct) = match rest.split_once('(') {
        Some((n, p)) => (n, Some(p.trim_end_matches(')').trim_end_matches('%'))),
        None => (rest, None),
    };
    let number = num.trim().parse().ok()?;
    let pct = match pct {
        Some(p) => Some(p.trim().parse().ok()?),
        None => None,
    };
    Some((number, pct))
}

/// The entries of an Obtained From line. An entry cut short by the column's length limit (it
/// ends in `+`) and anything unknown is left out.
#[must_use]
pub fn parse(obtained_from: &str) -> Vec<Entry> {
    obtained_from
        .split(',')
        .filter_map(|raw| {
            let e = raw.trim();
            if let Some(rest) = e.strip_prefix("Room ") {
                let (map, room) = rest.split_once('/')?;
                return Some(Entry::Room {
                    map: map.trim().parse().ok()?,
                    room: room.trim().parse().ok()?,
                });
            }
            let (kind, rest) = e.split_once(' ')?;
            let (number, pct) = numbered(rest)?;
            Some(match kind {
                "Shop" => Entry::Shop {
                    number,
                    restocks: true,
                },
                "Shop(nogen)" => Entry::Shop {
                    number,
                    restocks: false,
                },
                "Shop(sell)" => Entry::ShopBuys { number },
                "Monster" => Entry::Monster { number, pct: pct? },
                "Textblock" => Entry::Script { number, pct },
                "Item" => Entry::Chest {
                    item: number,
                    pct: pct?,
                },
                "NPC" => Entry::Npc { number },
                _ => return None,
            })
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// Kill the monster; it drops the item.
    Monster,
    /// Kill the monster for a chest that can hold the item.
    Chest,
    Shop,
    Room,
    Npc,
    Script,
}

/// One way to get an item.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Source {
    pub kind: SourceKind,
    /// The monster, shop, NPC or script number; the room number for a room.
    pub number: i32,
    /// The room's map; 0 otherwise.
    pub map: i32,
    pub name: String,
    /// Chance in percent of getting the item from one kill (or one run of the script); for a
    /// chest, the chest's drop chance times the item's chance inside it.
    pub pct: Option<f64>,
    /// `100 / pct`: the kills to expect.
    pub kills: Option<f64>,
    /// For a chest: the chest item, its name and the item's chance inside it.
    pub chest: Option<i32>,
    pub chest_name: Option<String>,
    pub chest_pct: Option<f64>,
    /// A shop that never restocks the item.
    pub one_time: bool,
}

/// The ways to get one item, best first.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Acquisition {
    pub item: i32,
    pub name: String,
    pub sources: Vec<Source>,
    /// Sources left out by the cap.
    pub more: usize,
    /// A chest holds the item but nothing in the data yields that chest.
    pub orphan_chests: usize,
}

fn kills(pct: f64) -> Option<f64> {
    (pct > 0.0).then(|| 100.0 / pct)
}

fn plain(kind: SourceKind, number: i32, name: String) -> Source {
    Source {
        kind,
        number,
        map: 0,
        name,
        pct: None,
        kills: None,
        chest: None,
        chest_name: None,
        chest_pct: None,
        one_time: false,
    }
}

/// How to get `item`: its own entries, and for a chest that holds it the monsters that drop the
/// chest (one level down; a chest inside a chest is not followed). At most `cap` sources of each kind (drops and chests count together).
#[must_use]
pub fn acquisition(world: &World, item: i32, cap: usize) -> Option<Acquisition> {
    let it = world.get_item(item)?;
    let monster_name = |n: i32| {
        world
            .get_monster(n)
            .and_then(|m| m.name.clone())
            .unwrap_or_else(|| format!("monster {n}"))
    };
    let mut sources = Vec::new();
    let mut orphan_chests = 0;
    for entry in parse(it.obtained_from.as_deref().unwrap_or("")) {
        match entry {
            Entry::Monster { number, pct } => sources.push(Source {
                pct: Some(pct),
                kills: kills(pct),
                ..plain(SourceKind::Monster, number, monster_name(number))
            }),
            Entry::Chest {
                item: chest,
                pct: inside,
            } => {
                let Some(c) = world.get_item(chest) else {
                    continue;
                };
                let before = sources.len();
                for from in parse(c.obtained_from.as_deref().unwrap_or("")) {
                    if let Entry::Monster { number, pct } = from {
                        let both = pct * inside / 100.0;
                        sources.push(Source {
                            pct: Some(both),
                            kills: kills(both),
                            chest: Some(chest),
                            chest_name: Some(c.name.clone()),
                            chest_pct: Some(inside),
                            ..plain(SourceKind::Chest, number, monster_name(number))
                        });
                    }
                }
                if sources.len() == before {
                    orphan_chests += 1;
                }
            }
            Entry::Shop { number, restocks } => {
                let name = world
                    .get_shop(number)
                    .map_or_else(|| format!("shop {number}"), |s| s.name.clone());
                sources.push(Source {
                    one_time: !restocks,
                    ..plain(SourceKind::Shop, number, name)
                });
            }
            Entry::Room { map, room } => {
                let name = world
                    .get_room(map, room)
                    .and_then(|r| r.name.clone())
                    .unwrap_or_default();
                sources.push(Source {
                    map,
                    ..plain(SourceKind::Room, room, name)
                });
            }
            Entry::Npc { number } => {
                sources.push(plain(SourceKind::Npc, number, monster_name(number)));
            }
            Entry::Script { number, pct } => sources.push(Source {
                pct,
                ..plain(SourceKind::Script, number, String::new())
            }),
            Entry::ShopBuys { .. } => {}
        }
    }
    // the surest way first: shops, places and people, then drops by chance, scripts last
    let rank = |s: &Source| match s.kind {
        SourceKind::Shop => 0,
        SourceKind::Room => 1,
        SourceKind::Npc => 2,
        SourceKind::Monster | SourceKind::Chest => 3,
        SourceKind::Script => 4,
    };
    sources.sort_by(|a, b| {
        rank(a)
            .cmp(&rank(b))
            .then(b.pct.unwrap_or(0.0).total_cmp(&a.pct.unwrap_or(0.0)))
            .then(a.number.cmp(&b.number))
    });
    // the cap is per kind, so thirty rooms do not push the drops off the list
    let total = sources.len();
    let mut seen = [0usize; 5];
    sources.retain(|s| {
        let k = rank(s);
        seen[k] += 1;
        seen[k] <= cap
    });
    let more = total - sources.len();
    Some(Acquisition {
        item,
        name: it.name.clone(),
        sources,
        more,
        orphan_chests,
    })
}
