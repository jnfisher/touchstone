//! The EQ tab's rules (FEAT-EQ): which slot an item goes to, what each slot's pick list holds, how
//! equipping and unequipping move items between the twenty slots, and the warning flags. Card P4-17.
//!
//! MME keeps the equipment in twenty sorted combo boxes (`cmbEquip`) and mirrors the selection in
//! `nEquippedItem`; the rules live in procedures that fill and drive those combos. The port is stateless:
//! the equipped slots go in, the new slots come out, and the lists are computed from the item table.
//!
//! - [`list_slots`] / [`in_list`] / [`slot_list`]: `InvenAddEquip`'s mapping and the contents of
//!   `LoadInvenItems` and `FilterInvenItems` (the global filter).
//! - [`equip_item`]: `InvenEquipItem`, including the paired fingers and wrists and the unequip toggle.
//! - [`set_slot`]: picking an entry in a slot's combo (`cmbEquip_Click`).
//! - [`reconcile`]: `FilterInvenItems` and `LoadInvenItems` on already equipped items.
//! - [`clear_all`], [`clear_unheld`]: `InvenSetupEquip` and the equipment half of `InvenClear`.
//! - [`slot_flags`]: the red boxes `CalcCharacterStats` paints on the hold checkboxes.
//! - [`next_armour_worn`]: `cmdArmNextSlot_Click` of the Armour explorer.
//!
//! Decisions that follow MME rather than the game:
//!
//! - A two-handed weapon does not clear the off-hand. Both stay equipped and [`slot_flags`] marks the
//!   weapon and the off-hand red (`docs/differences.md`).
//! - `InvenEquipItem` adds the item to its slot's combo when the global filter had dropped it, so it
//!   accepts every item the mapping allows. The combos then keep that item until the next rebuild; the
//!   stateless port forgets it, so [`set_slot`] under the filter refuses an unusable item that MME's
//!   combo would still offer after such an equip.
//! - Unequipping through `InvenEquipItem` does not set the "character changed" flag (the combo is set to
//!   "(none)" after the array was already cleared, so `cmbEquip_Click` finds nothing to do);
//!   [`EquipOutcome::changed`] is false for it.
//! - The combos are `Sorted`; only the display order depends on that, so lists come back in table order.

use touchstone_model::{Item, World};

use crate::Profile;
use crate::gear::{self, EQUIP_SLOTS};

pub use crate::gear::Character as UsableBy;

/// The message `InvenEquipItem` shows for armour worn nowhere.
pub const NOWHERE_WORN_MESSAGE: &str = "Nowhere-Worn items cannot be equipped, but you can add them to the item manager and then mark them as 'carried' to have its stats count for your character.";

/// The weapon slot.
pub const WEAPON_SLOT: usize = 16;
/// The off-hand slot.
pub const OFF_HAND_SLOT: usize = 15;
/// The two finger slots.
pub const FINGER_SLOTS: [usize; 2] = [9, 10];
/// The two wrist slots (the second is used only when `bInvenUse2ndWrist` is on).
pub const WRIST_SLOTS: [usize; 2] = [6, 7];

/// `InvenAddEquip`'s main slot for an item, or `None` when it is listed nowhere: armour by `Worn`
/// (0, 17, 20 and up and negatives are not listed), weapons in the weapon slot, other types never.
/// Both finger values give 9 / 10 and the wrist gives 6; [`list_slots`] widens those to the pair.
#[must_use]
pub fn main_slot(item_type: i32, worn: i32) -> Option<usize> {
    match item_type {
        0 => match worn {
            1 => Some(19),
            2 => Some(0),
            3 => Some(8),
            4 => Some(9),
            5 => Some(13),
            6 => Some(5),
            7 => Some(3),
            8 => Some(2),
            9 => Some(12),
            10 => Some(11),
            11 => Some(4),
            12 => Some(15),
            13 => Some(10),
            14 => Some(6),
            15 => Some(1),
            16 => Some(14),
            18 => Some(17),
            19 => Some(18),
            _ => None,
        },
        1 => Some(WEAPON_SLOT),
        _ => None,
    }
}

/// The slot combos `InvenAddEquip` adds an item to: both fingers or both wrists for those, one slot
/// for everything else, none when the item is not equippable.
#[must_use]
pub fn list_slots(item_type: i32, worn: i32) -> Vec<usize> {
    match main_slot(item_type, worn) {
        Some(9 | 10) => vec![9, 10],
        Some(6 | 7) => vec![6, 7],
        Some(s) => vec![s],
        None => vec![],
    }
}

/// What decides whether an item is offered: `bOnlyInGame`, and the global filter's character.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ListRule {
    /// `bOnlyInGame`: items marked not in game are left out.
    pub only_in_game: bool,
    /// The global filter (`FilterInvenItems`): only items this character can use, armour and weapons.
    pub usable_by: Option<UsableBy>,
}

/// Does the slot's combo hold the item under the rule?
#[must_use]
pub fn in_list(profile: Profile, world: &World, item: &Item, slot: usize, rule: &ListRule) -> bool {
    if rule.only_in_game && !item.in_game {
        return false;
    }
    if !list_slots(item.item_type, item.worn).contains(&slot) {
        return false;
    }
    match &rule.usable_by {
        // `ItemIsUsableByChar(n, True)`: the minimum item level is not applied
        Some(who) => gear::item_is_usable_by_char(
            profile,
            world,
            who,
            &[0; EQUIP_SLOTS],
            item.number,
            true,
            false,
        ),
        None => true,
    }
}

/// The item numbers in a slot's combo after `LoadInvenItems` / `FilterInvenItems`, in table order
/// (MME's combos sort by display text). The leading "(none)" entry is not included.
#[must_use]
pub fn slot_list(profile: Profile, world: &World, slot: usize, rule: &ListRule) -> Vec<i32> {
    world
        .items
        .iter()
        .filter(|i| in_list(profile, world, i, slot, rule))
        .map(|i| i.number)
        .collect()
}

/// What a rule did to the equipped slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquipOutcome {
    pub equipped: [i32; EQUIP_SLOTS],
    /// The slot that was filled or cleared, or the slot already holding the item.
    pub slot: Option<usize>,
    /// `bPromptSave` would be set: a slot's combo selection moved to a different item.
    pub changed: bool,
    /// Dialogs MME shows.
    pub messages: Vec<String>,
}

impl EquipOutcome {
    fn unchanged(equipped: &[i32; EQUIP_SLOTS]) -> Self {
        Self {
            equipped: *equipped,
            slot: None,
            changed: false,
            messages: vec![],
        }
    }
}

/// Options of [`equip_item`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EquipOptions {
    /// `bInvenUse2ndWrist`: a new wrist item goes to the second wrist when the first is taken.
    pub use_2nd_wrist: bool,
    /// `bUnequipIfEquipped`: an item already in a slot is removed instead of equipped again.
    pub unequip_if_equipped: bool,
    /// `frmMain.Enabled`: the nowhere-worn message is only shown while the form is enabled.
    pub dialogs: bool,
}

/// The slot `InvenEquipItem` picks for an item; `Err(true)` for armour worn nowhere (a message), `Err(false)`
/// for items with no slot.
fn target_slot(
    equipped: &[i32; EQUIP_SLOTS],
    item: &Item,
    opts: &EquipOptions,
) -> Result<Slot, bool> {
    let n = item.number;
    let single = |s| Ok(Slot::Fill(s));
    match item.item_type {
        0 => match item.worn {
            0 => Err(true),
            1 => single(19),
            2 => single(0),
            3 => single(8),
            4 | 13 => {
                if equipped[9] > 0 {
                    if equipped[9] == n || equipped[10] == n {
                        Ok(Slot::Already(if equipped[9] == n { 9 } else { 10 }))
                    } else {
                        single(10)
                    }
                } else if equipped[10] > 0 {
                    if equipped[10] == n {
                        Ok(Slot::Already(10))
                    } else {
                        single(9)
                    }
                } else {
                    single(9)
                }
            }
            5 => single(13),
            6 => single(5),
            7 => single(3),
            8 => single(2),
            9 => single(12),
            10 => single(11),
            11 => single(4),
            12 => single(15),
            14 => {
                if equipped[6] > 0 {
                    if equipped[6] == n || equipped[7] == n {
                        Ok(Slot::Already(if equipped[6] == n { 6 } else { 7 }))
                    } else if opts.use_2nd_wrist {
                        single(7)
                    } else {
                        single(6)
                    }
                } else if equipped[7] > 0 {
                    if equipped[7] == n {
                        Ok(Slot::Already(7))
                    } else {
                        single(6)
                    }
                } else {
                    single(6)
                }
            }
            15 => single(1),
            16 => single(14),
            18 => single(17),
            19 => single(18),
            _ => Err(false),
        },
        1 => single(WEAPON_SLOT),
        _ => Err(false),
    }
}

enum Slot {
    Fill(usize),
    Already(usize),
}

/// Port of frmMain `InvenEquipItem`: equip an item (or take it off when `unequip_if_equipped` and it is
/// worn). Items the table does not have, item 0, and types with no slot change nothing.
#[must_use]
pub fn equip_item(
    world: &World,
    equipped: &[i32; EQUIP_SLOTS],
    item_number: i32,
    opts: &EquipOptions,
) -> EquipOutcome {
    let mut out = EquipOutcome::unchanged(equipped);
    if item_number == 0 {
        return out;
    }
    if opts.unequip_if_equipped
        && let Some(x) = equipped.iter().position(|&e| e == item_number)
    {
        out.equipped[x] = 0;
        out.slot = Some(x);
        return out;
    }
    let Some(item) = world.get_item(item_number) else {
        return out;
    };
    match target_slot(equipped, item, opts) {
        Err(true) => {
            if opts.dialogs {
                out.messages.push(NOWHERE_WORN_MESSAGE.to_string());
            }
        }
        Err(false) => {}
        Ok(Slot::Already(x)) => out.slot = Some(x),
        Ok(Slot::Fill(x)) => {
            out.slot = Some(x);
            if out.equipped[x] != item_number {
                out.equipped[x] = item_number;
                out.changed = true;
            }
        }
    }
    out
}

/// Why a pick from a slot's combo was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SetSlotError {
    #[error("there is no slot {0}")]
    NoSuchSlot(usize),
    #[error("item {0} is not in the item table")]
    NoSuchItem(i32),
    #[error("item {0} does not go in that slot")]
    NotInList(i32),
}

/// Port of picking an entry in `cmbEquip(slot)` (`cmbEquip_Click`): item 0 is the "(none)" entry; any
/// other item must be one the slot's combo holds under the rule.
pub fn set_slot(
    profile: Profile,
    world: &World,
    equipped: &[i32; EQUIP_SLOTS],
    slot: usize,
    item_number: i32,
    rule: &ListRule,
) -> Result<EquipOutcome, SetSlotError> {
    if slot >= EQUIP_SLOTS {
        return Err(SetSlotError::NoSuchSlot(slot));
    }
    if item_number != 0 {
        let item = world
            .get_item(item_number)
            .ok_or(SetSlotError::NoSuchItem(item_number))?;
        if !in_list(profile, world, item, slot, rule) {
            return Err(SetSlotError::NotInList(item_number));
        }
    }
    let mut out = EquipOutcome::unchanged(equipped);
    out.slot = Some(slot);
    if equipped[slot] != item_number {
        out.equipped[slot] = item_number;
        out.changed = true;
    }
    Ok(out)
}

/// Port of `InvenSetupEquip`: every slot empty. `changed` is whether any slot held something.
#[must_use]
pub fn clear_all(equipped: &[i32; EQUIP_SLOTS]) -> EquipOutcome {
    let mut out = EquipOutcome::unchanged(equipped);
    out.equipped = [0; EQUIP_SLOTS];
    out.changed = equipped.iter().any(|&e| e != 0);
    out
}

/// Port of the equipment half of `InvenClear`: every slot whose hold box is not ticked is emptied.
#[must_use]
pub fn clear_unheld(equipped: &[i32; EQUIP_SLOTS], hold: &[bool; EQUIP_SLOTS]) -> EquipOutcome {
    let mut out = EquipOutcome::unchanged(equipped);
    for x in 0..EQUIP_SLOTS {
        if !hold[x] && out.equipped[x] != 0 {
            out.equipped[x] = 0;
            out.changed = true;
        }
    }
    out
}

/// Port of `FilterInvenItems` (a rule with `usable_by`) and of `FilterInvenItems(True)` / `LoadInvenItems`
/// (a rule without): the combos are rebuilt and each slot keeps its item only if the new list holds it.
/// `changed` is true when anything was equipped, because every re-selection or loss marks the character.
#[must_use]
pub fn reconcile(
    profile: Profile,
    world: &World,
    equipped: &[i32; EQUIP_SLOTS],
    rule: &ListRule,
) -> EquipOutcome {
    let mut out = EquipOutcome::unchanged(equipped);
    for x in 0..EQUIP_SLOTS {
        let keep = equipped[x] > 0
            && world
                .get_item(equipped[x])
                .is_some_and(|i| in_list(profile, world, i, x, rule));
        out.equipped[x] = if keep { equipped[x] } else { 0 };
        if equipped[x] > 0 {
            out.changed = true;
        }
    }
    out
}

/// The "red" hold boxes `CalcCharacterStats` sets: an item the character cannot use (the minimum item
/// level does not count), and weapon plus off-hand together when the weapon is two-handed (weapon type
/// 1 or 3). With no class picked MME leaves before the loop and nothing is flagged.
#[must_use]
pub fn slot_flags(
    profile: Profile,
    world: &World,
    who: Option<&UsableBy>,
    equipped: &[i32; EQUIP_SLOTS],
) -> [bool; EQUIP_SLOTS] {
    let mut flags = [false; EQUIP_SLOTS];
    let Some(who) = who else {
        return flags;
    };
    for (x, &n) in equipped.iter().enumerate() {
        if n < 1 {
            continue;
        }
        let Some(item) = world.get_item(n) else {
            continue;
        };
        if x == WEAPON_SLOT && matches!(item.weapon_type, 1 | 3) && equipped[OFF_HAND_SLOT] > 0 {
            flags[OFF_HAND_SLOT] = true;
            flags[WEAPON_SLOT] = true;
        }
        if !gear::item_is_usable_by_char(profile, world, who, equipped, n, true, false) {
            flags[x] = true;
        }
    }
    flags
}

/// Where `cmdArmNextSlot_Click` leaves the Armour explorer's worn filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NextWorn {
    /// `cmbArmourWorn.ListIndex` afterwards.
    pub index: i32,
    /// The VB error MME's handler ran for (380 when the list has fewer than two entries).
    pub error: Option<i32>,
}

/// Port of `cmdArmNextSlot_Click`: step the worn filter to the next entry (wrapping to entry 1, the
/// first real slot after "Anywhere"), and keep stepping while the armour list for that slot is empty,
/// at most `count - 1` times in all. `empty_at(index)` says whether the filter finds no armour at
/// that index.
pub fn next_armour_worn(count: i32, start: i32, mut empty_at: impl FnMut(i32) -> bool) -> NextWorn {
    let mut index = start;
    let mut tries = 0;
    loop {
        let next = if index + 1 >= count { 1 } else { index + 1 };
        if next >= count {
            // ListIndex = 1 on a list of 0 or 1 entries: error 380 "Invalid property value"
            return NextWorn {
                index,
                error: Some(380),
            };
        }
        index = next;
        tries += 1;
        if empty_at(index) && tries < count - 1 {
            continue;
        }
        return NextWorn { index, error: None };
    }
}
