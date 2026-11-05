//! Plain data types: `World` (items, monsters, spells, rooms, shops, classes, races, lairs,
//! textblocks, info) and the enums for MME's well-known codes. No IO.
//!
//! Field names are the MME `.mdb` column names in snake_case; `docs/model.md` maps every column
//! to its field so loaders can be written mechanically.
mod enums;
mod tables;
mod world;

pub use enums::{
    Alignment, ArmourType, ClassWeaponType, Currency, ItemType, Magery, MonsterAttackType,
    MonsterType, ShopType, SpellAttackType, SpellTargets, WeaponType, WornSlot,
};
pub use tables::{
    AbilitySlot, Class, DropSlot, Info, Item, Lair, MidSpell, Monster, MonsterAttack, Race, Room,
    Shop, ShopSlot, Spell, TbInfo,
};
pub use world::{ModelError, World, WorldParts};
