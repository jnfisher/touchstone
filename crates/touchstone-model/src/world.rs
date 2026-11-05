//! `World`: every table as a `Vec` plus key -> index maps.
use std::collections::HashMap;
use std::hash::Hash;

use serde::Serialize;

use crate::tables::{Class, Info, Item, Lair, Monster, Race, Room, Shop, Spell, TbInfo};

/// Building a [`World`] failed.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ModelError {
    #[error("duplicate key {key} in table {table}")]
    DuplicateKey { table: &'static str, key: String },
}

/// The tables as read by a loader, before indexing.
#[derive(Debug, Clone)]
pub struct WorldParts {
    pub info: Info,
    pub items: Vec<Item>,
    pub monsters: Vec<Monster>,
    pub spells: Vec<Spell>,
    pub rooms: Vec<Room>,
    pub shops: Vec<Shop>,
    pub classes: Vec<Class>,
    pub races: Vec<Race>,
    pub lairs: Vec<Lair>,
    pub tbinfo: Vec<TbInfo>,
}

/// All game data. Lookups are by MME record number (the table's `Number` column), not by
/// position. Row order of each `Vec` is the loader's order.
#[derive(Debug, Clone, Serialize)]
pub struct World {
    pub info: Info,
    pub items: Vec<Item>,
    pub monsters: Vec<Monster>,
    pub spells: Vec<Spell>,
    pub rooms: Vec<Room>,
    pub shops: Vec<Shop>,
    pub classes: Vec<Class>,
    pub races: Vec<Race>,
    pub lairs: Vec<Lair>,
    pub tbinfo: Vec<TbInfo>,
    #[serde(skip)]
    index: Index,
}

#[derive(Debug, Clone, Default)]
struct Index {
    items: HashMap<i32, usize>,
    monsters: HashMap<i32, usize>,
    spells: HashMap<i32, usize>,
    rooms: HashMap<(i32, i32), usize>,
    shops: HashMap<i32, usize>,
    classes: HashMap<i32, usize>,
    races: HashMap<i32, usize>,
    lairs: HashMap<String, usize>,
    tbinfo: HashMap<i32, usize>,
}

fn index_by<T, K: Hash + Eq + std::fmt::Debug>(
    table: &'static str,
    rows: &[T],
    key: impl Fn(&T) -> K,
) -> Result<HashMap<K, usize>, ModelError> {
    let mut map = HashMap::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let k = key(row);
        let shown = format!("{k:?}");
        if map.insert(k, i).is_some() {
            return Err(ModelError::DuplicateKey { table, key: shown });
        }
    }
    Ok(map)
}

impl World {
    /// Index the tables. Fails if a table repeats a key.
    pub fn new(parts: WorldParts) -> Result<Self, ModelError> {
        let index = Index {
            items: index_by("Items", &parts.items, |r| r.number)?,
            monsters: index_by("Monsters", &parts.monsters, |r| r.number)?,
            spells: index_by("Spells", &parts.spells, |r| r.number)?,
            rooms: index_by("Rooms", &parts.rooms, |r| (r.map_number, r.room_number))?,
            shops: index_by("Shops", &parts.shops, |r| r.number)?,
            classes: index_by("Classes", &parts.classes, |r| r.number)?,
            races: index_by("Races", &parts.races, |r| r.number)?,
            lairs: index_by("Lairs", &parts.lairs, |r| r.group_index.clone())?,
            tbinfo: index_by("TBInfo", &parts.tbinfo, |r| r.number)?,
        };
        Ok(Self {
            info: parts.info,
            items: parts.items,
            monsters: parts.monsters,
            spells: parts.spells,
            rooms: parts.rooms,
            shops: parts.shops,
            classes: parts.classes,
            races: parts.races,
            lairs: parts.lairs,
            tbinfo: parts.tbinfo,
            index,
        })
    }

    pub fn get_item(&self, number: i32) -> Option<&Item> {
        self.index.items.get(&number).map(|&i| &self.items[i])
    }

    pub fn get_monster(&self, number: i32) -> Option<&Monster> {
        self.index.monsters.get(&number).map(|&i| &self.monsters[i])
    }

    pub fn get_spell(&self, number: i32) -> Option<&Spell> {
        self.index.spells.get(&number).map(|&i| &self.spells[i])
    }

    pub fn get_room(&self, map_number: i32, room_number: i32) -> Option<&Room> {
        self.index
            .rooms
            .get(&(map_number, room_number))
            .map(|&i| &self.rooms[i])
    }

    pub fn get_shop(&self, number: i32) -> Option<&Shop> {
        self.index.shops.get(&number).map(|&i| &self.shops[i])
    }

    pub fn get_class(&self, number: i32) -> Option<&Class> {
        self.index.classes.get(&number).map(|&i| &self.classes[i])
    }

    pub fn get_race(&self, number: i32) -> Option<&Race> {
        self.index.races.get(&number).map(|&i| &self.races[i])
    }

    /// Lairs are keyed by their `GroupIndex` text.
    pub fn get_lair(&self, group_index: &str) -> Option<&Lair> {
        self.index.lairs.get(group_index).map(|&i| &self.lairs[i])
    }

    pub fn get_tbinfo(&self, number: i32) -> Option<&TbInfo> {
        self.index.tbinfo.get(&number).map(|&i| &self.tbinfo[i])
    }
}
