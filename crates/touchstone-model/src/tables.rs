//! One struct per MME table. Integer columns are `i32` (every observed range fits `i16`, but
//! later data versions may not), 0/1 flag columns are `bool`, nullable text is `Option<String>`.
//! Numbered column series become fixed arrays of small structs; slot order is the column number.
use serde::Serialize;

use crate::enums::{
    Alignment, ArmourType, ClassWeaponType, Currency, ItemType, Magery, MonsterAttackType,
    MonsterType, ShopType, SpellAttackType, SpellTargets, WeaponType, WornSlot,
};

/// One `Abil-n` / `AbilVal-n` column pair. Unused slots are zero (but a few spell rows carry a
/// stray value beside ability 0, so the value is kept as read).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct AbilitySlot {
    pub ability: i32,
    pub value: i32,
}

/// `Classes`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Class {
    pub number: i32,
    pub name: String,
    pub min_hits: i32,
    pub max_hits: i32,
    pub exp_table: i32,
    pub magery_type: i32,
    pub magery_lvl: i32,
    pub weapon_type: i32,
    pub armour_type: i32,
    pub combat_lvl: i32,
    /// `Abil-0..9`, `AbilVal-0..9`.
    pub abilities: [AbilitySlot; 10],
}

impl Class {
    pub fn magery(&self) -> Option<Magery> {
        Magery::from_code(self.magery_type)
    }
    pub fn weapon_type_enum(&self) -> Option<ClassWeaponType> {
        ClassWeaponType::from_code(self.weapon_type)
    }
    pub fn armour_type_enum(&self) -> Option<ArmourType> {
        ArmourType::from_code(self.armour_type)
    }
}

/// `Info` (a single row).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Info {
    pub nmr_version: String,
    pub dat_file_version: String,
    pub date: String,
    pub time: String,
    pub custom: String,
    pub legit: bool,
    pub disable_kai: bool,
    pub update_url: String,
}

/// `Items`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Item {
    pub number: i32,
    pub name: String,
    pub limit: i32,
    pub encum: i32,
    pub item_type: i32,
    pub use_count: i32,
    pub price: i32,
    pub currency: i32,
    pub min: i32,
    pub max: i32,
    pub armour_class: i32,
    pub damage_resist: i32,
    pub weapon_type: i32,
    pub armour_type: i32,
    pub worn: i32,
    pub accy: i32,
    pub gettable: bool,
    pub str_req: i32,
    pub speed: i32,
    pub not_droppable: bool,
    pub destroy_on_death: bool,
    pub retain_after_uses: bool,
    /// `ClassRest-0..9`: class numbers (0 = unused slot).
    pub class_rest: [i32; 10],
    /// `RaceRest-0..9`: race numbers (0 = unused slot).
    pub race_rest: [i32; 10],
    /// `NegateSpell-0..9`: spell numbers (0 = unused slot).
    pub negate_spell: [i32; 10],
    /// `Abil-0..19`, `AbilVal-0..19`.
    pub abilities: [AbilitySlot; 20],
    pub in_game: bool,
    pub obtained_from: Option<String>,
    pub references: Option<String>,
}

impl Item {
    pub fn item_type_enum(&self) -> Option<ItemType> {
        ItemType::from_code(self.item_type)
    }
    pub fn currency_enum(&self) -> Option<Currency> {
        Currency::from_code(self.currency)
    }
    pub fn weapon_type_enum(&self) -> Option<WeaponType> {
        WeaponType::from_code(self.weapon_type)
    }
    pub fn armour_type_enum(&self) -> Option<ArmourType> {
        ArmourType::from_code(self.armour_type)
    }
    pub fn worn_slot(&self) -> Option<WornSlot> {
        WornSlot::from_code(self.worn)
    }
}

/// `Lairs`. Keyed by `group_index` (no numeric key).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Lair {
    pub group_index: String,
    pub mob_list: String,
    pub mobs: i32,
    pub total_lairs: i32,
    pub avg_delay: i32,
    pub avg_walk: f64,
    pub avg_exp: f64,
    pub avg_dmg: f64,
    pub avg_dmg_phys: f64,
    pub avg_dmg_spell: f64,
    pub avg_hp: f64,
    pub avg_ac: i32,
    pub avg_dr: i32,
    pub avg_mr: i32,
    pub avg_dodge: i32,
}

/// One `AttName-n` .. `AttHitSpell-n` column group of `Monsters`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MonsterAttack {
    pub name: String,
    pub att_type: i32,
    pub acc: i32,
    /// `Att%-n`: the attack's weight in the monster's attack list.
    pub pct: i32,
    /// `AttTrue%-n`
    pub true_pct: f64,
    pub min: i32,
    pub max: i32,
    pub energy: i32,
    pub hit_spell: i32,
}

impl MonsterAttack {
    pub fn att_type_enum(&self) -> Option<MonsterAttackType> {
        MonsterAttackType::from_code(self.att_type)
    }
}

/// One `MidSpell-n`, `MidSpell%-n`, `MidSpellLVL-n` column group of `Monsters`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct MidSpell {
    pub spell: i32,
    pub pct: i32,
    pub lvl: i32,
}

/// One `DropItem-n`, `DropItem%-n` column pair of `Monsters`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct DropSlot {
    pub item: i32,
    pub pct: i32,
}

/// `Monsters`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Monster {
    pub number: i32,
    pub name: Option<String>,
    pub weapon: i32,
    pub armour_class: i32,
    pub damage_resist: i32,
    pub follow_pct: i32,
    pub magic_res: i32,
    pub bs_defense: i32,
    pub exp: f64,
    pub exp_multi: f64,
    pub hp: i32,
    pub energy: i32,
    pub avg_dmg: f64,
    pub avg_dmg_phys: f64,
    pub avg_dmg_spell: f64,
    pub greet_txt: i32,
    pub hp_regen: i32,
    pub charm_lvl: i32,
    /// Column `Type`.
    pub r#type: i32,
    pub undead: i32,
    pub align: i32,
    pub regen_time: i32,
    pub game_limit: i32,
    pub avg_lair_exp: f64,
    pub script_value: f64,
    /// Coin drops: `R` runic, `P` platinum, `G` gold, `S` silver, `C` copper.
    pub r: i32,
    pub p: i32,
    pub g: i32,
    pub s: i32,
    pub c: i32,
    pub death_spell: i32,
    pub create_spell: i32,
    /// `AttName-0..4` and the seven columns that follow each.
    pub attacks: [MonsterAttack; 5],
    /// `MidSpell-0..4`, `MidSpell%-0..4`, `MidSpellLVL-0..4`.
    pub mid_spells: [MidSpell; 5],
    /// `DropItem-0..9`, `DropItem%-0..9`.
    pub drops: [DropSlot; 10],
    /// `Abil-0..9`, `AbilVal-0..9`.
    pub abilities: [AbilitySlot; 10],
    pub in_game: bool,
    pub summoned_by: Option<String>,
}

impl Monster {
    pub fn monster_type(&self) -> Option<MonsterType> {
        MonsterType::from_code(self.r#type)
    }
    pub fn alignment(&self) -> Option<Alignment> {
        Alignment::from_code(self.align)
    }
}

/// `Races`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Race {
    pub number: i32,
    pub name: String,
    pub m_int: i32,
    pub m_wil: i32,
    pub m_str: i32,
    pub m_hea: i32,
    pub m_agl: i32,
    pub m_chm: i32,
    pub x_int: i32,
    pub x_wil: i32,
    pub x_str: i32,
    pub x_hea: i32,
    pub x_agl: i32,
    pub x_chm: i32,
    pub hp_per_lvl: i32,
    pub exp_table: i32,
    pub base_cp: i32,
    /// `Abil-0..9`, `AbilVal-0..9`.
    pub abilities: [AbilitySlot; 10],
}

/// `Rooms`, keyed by `(map_number, room_number)`. Exit columns hold MME's text as stored
/// (`0` for none, `map/room`, optionally followed by a door note).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Room {
    pub map_number: i32,
    pub room_number: i32,
    pub name: Option<String>,
    pub light: i32,
    pub shop: i32,
    pub npc: i32,
    pub cmd: i32,
    pub spell: i32,
    pub lair: Option<String>,
    pub delay: i32,
    pub placed: Option<String>,
    pub n: String,
    pub s: String,
    pub e: String,
    pub w: String,
    pub ne: String,
    pub nw: String,
    pub se: String,
    pub sw: String,
    pub u: String,
    pub d: String,
}

/// One `Item-n`, `Max-n`, `Time-n`, `Amount-n`, `%-n` column group of `Shops`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ShopSlot {
    pub item: i32,
    pub max: i32,
    pub time: i32,
    pub amount: i32,
    pub pct: i32,
}

/// `Shops`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Shop {
    pub number: i32,
    pub name: String,
    pub shop_type: i32,
    pub min_lvl: i32,
    pub max_lvl: i32,
    pub markup_pct: i32,
    pub class_rest: i32,
    /// `Item-0..19` and the four columns that follow each.
    pub stock: [ShopSlot; 20],
    pub in_game: bool,
    pub assigned_to: Option<String>,
}

impl Shop {
    pub fn shop_type_enum(&self) -> Option<ShopType> {
        ShopType::from_code(self.shop_type)
    }
}

/// `Spells`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Spell {
    pub number: i32,
    pub name: Option<String>,
    pub short: Option<String>,
    pub req_level: i32,
    pub energy_cost: i32,
    pub mana_cost: i32,
    pub min_base: i32,
    pub max_base: i32,
    pub diff: i32,
    pub type_of_resists: i32,
    pub targets: i32,
    pub dur: i32,
    pub att_type: i32,
    pub magery: i32,
    pub magery_lvl: i32,
    pub cap: i32,
    pub max_inc_lvls: i32,
    pub max_inc: i32,
    pub min_inc_lvls: i32,
    pub min_inc: i32,
    pub dur_inc_lvls: i32,
    pub dur_inc: i32,
    /// `Abil-0..9`, `AbilVal-0..9`.
    pub abilities: [AbilitySlot; 10],
    pub learnable: bool,
    pub learned_from: Option<String>,
    pub casted_by: Option<String>,
    pub classes: Option<String>,
}

impl Spell {
    pub fn targets_enum(&self) -> Option<SpellTargets> {
        SpellTargets::from_code(self.targets)
    }
    pub fn attack_type(&self) -> Option<SpellAttackType> {
        SpellAttackType::from_code(self.att_type)
    }
    pub fn magery_enum(&self) -> Option<Magery> {
        Magery::from_code(self.magery)
    }
}

/// `TBInfo` (textblock links).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TbInfo {
    pub number: i32,
    pub link_to: i32,
    pub action: Option<String>,
    pub called_from: String,
}
