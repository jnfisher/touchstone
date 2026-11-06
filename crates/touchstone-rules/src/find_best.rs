//! The EQ tab's Find Best and Next Best (FEAT-EQ). Card P4-18.
//!
//! MME's `InvenFindBest` scores every item in each slot's combo against one criterion, keeps the best per
//! slot and selects the winners; "Next Best" replays the last criterion, steps down from what is worn and
//! excludes the worn items. It drives the combos, so the port works on the worn slots as it goes: the
//! paired finger and wrist checks and the two-hander recheck read slots earlier in the same pass.
//!
//! - [`find_best`]: a pick from the Find Best menu (`mnuFindBest*_Click` after `cmdEquipButtons_Click(11)`).
//! - [`next_best`]: `cmdEquipButtons_Click(12)`, replaying [`FindBestRequest::last`].
//! - [`menu`]: the menu captions in the designer block's order.
//!
//! What survives between clicks is `nInvenLastIndex` ([`LastFind`]) and `nInvenExcludedItems`
//! ([`FindBestRequest::excluded`]); the "last find best" values of each slot are recomputed from the worn
//! items on every Next Best, so [`FindBestResult::slot_values`] is informational.
//!
//! Decisions that follow MME rather than the game:
//!
//! - Ties (same value and encumbrance ratio) go to the first entry of the list as given. MME's combos are
//!   sorted by display text, so the caller orders the lists that way.
//! - Abilities of 1000 and up (VileWard) exist only on GreaterMUD and Paramud: on the stock profile MME
//!   leaves quietly, after noting the criterion for a later Next Best. So does the port (`ran` is false).
//! - A Next Best before any Find Best meets an unallocated exclusion array: VB error 9, handled, nothing
//!   changes. `excluded: None` stands for that array.
//! - Selecting a winner never sets the character's "changed" flag: MME runs it with `bDontRefresh` on, and
//!   `cmbEquip_Click` returns before `bPromptSave`.

use std::collections::HashMap;

use touchstone_model::{Item, World};
use touchstone_vb6::{self as vb, Currency};

use crate::gear::EQUIP_SLOTS;
use crate::stats::get_enc_ratio;
use crate::{Profile, RulesError};

/// `enmFindBest`: which Find Best submenu was picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Criterion {
    #[default]
    Armour = 0,
    Attack = 1,
    Resist = 2,
    Stats = 3,
    Mystics = 4,
    Attribs = 5,
}

impl Criterion {
    #[must_use]
    pub fn from_code(code: i32) -> Option<Self> {
        Some(match code {
            0 => Self::Armour,
            1 => Self::Attack,
            2 => Self::Resist,
            3 => Self::Stats,
            4 => Self::Mystics,
            5 => Self::Attribs,
            _ => return None,
        })
    }
}

/// `nInvenLastIndex`: the menu entry the last Find Best ran, which Next Best replays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LastFind {
    pub index: i32,
    pub criterion: Criterion,
}

/// One entry of the Find Best menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuEntry {
    /// The submenu caption (`Armour`, `Attacking`, ...).
    pub group: &'static str,
    /// A nested submenu (`Mystic Attacks` under `Attacking`).
    pub subgroup: Option<&'static str>,
    pub label: &'static str,
    pub criterion: Criterion,
    /// The `Index` of the menu element, which is what `InvenFindBest` receives.
    pub index: i32,
    /// MME shows the entry only on GreaterMUD and Paramud (`mnuFindBestArmour(6).Visible = bGreaterMUD`).
    pub greater_only: bool,
}

const fn e(
    group: &'static str,
    subgroup: Option<&'static str>,
    label: &'static str,
    criterion: Criterion,
    index: i32,
) -> MenuEntry {
    MenuEntry {
        group,
        subgroup,
        label,
        criterion,
        index,
        greater_only: false,
    }
}

use Criterion::{Armour, Attack, Attribs, Mystics, Resist, Stats};

/// The designer block's menu in display order: `mnuFindBestItem` groups Armour, Attacking (with the Mystic
/// Attacks submenu last), Attributes, Resistances and Stats.
pub const MENU: &[MenuEntry] = &[
    e("Armour", None, "AC/DR Combo", Armour, 0),
    e("Armour", None, "AC", Armour, 1),
    e("Armour", None, "DR", Armour, 2),
    e("Armour", None, "Dodge", Armour, 3),
    e("Armour", None, "Prot. from Evil", Armour, 4),
    e("Armour", None, "Prot. from Good", Armour, 5),
    MenuEntry {
        greater_only: true,
        ..e("Armour", None, "VileWard", Armour, 6)
    },
    e("Attacking", None, "Accuracy", Attack, 0),
    e("Attacking", None, "BS Accuracy", Attack, 1),
    e("Attacking", None, "BS Min", Attack, 2),
    e("Attacking", None, "BS Max", Attack, 3),
    e("Attacking", None, "Crits", Attack, 4),
    e("Attacking", None, "DamageShield", Attack, 5),
    e("Attacking", None, "Max Damage", Attack, 6),
    e(
        "Attacking",
        Some("Mystic Attacks"),
        "JumpKick ACY",
        Mystics,
        0,
    ),
    e(
        "Attacking",
        Some("Mystic Attacks"),
        "JumpKick DMG",
        Mystics,
        1,
    ),
    e("Attacking", Some("Mystic Attacks"), "Kick ACY", Mystics, 2),
    e("Attacking", Some("Mystic Attacks"), "Kick DMG", Mystics, 3),
    e("Attacking", Some("Mystic Attacks"), "Punch ACY", Mystics, 4),
    e("Attacking", Some("Mystic Attacks"), "Punch DMG", Mystics, 5),
    e("Attributes", None, "Agility", Attribs, 0),
    e("Attributes", None, "Charm", Attribs, 1),
    e("Attributes", None, "Health", Attribs, 2),
    e("Attributes", None, "Intellect", Attribs, 3),
    e("Attributes", None, "Strength", Attribs, 4),
    e("Attributes", None, "Wisdom", Attribs, 5),
    e("Resistances", None, "All Elemental", Resist, 0),
    e("Resistances", None, "All Magic (MR)", Resist, 1),
    e("Resistances", None, "Cold", Resist, 2),
    e("Resistances", None, "Fire", Resist, 3),
    e("Resistances", None, "Lightning", Resist, 4),
    e("Resistances", None, "Stone", Resist, 5),
    e("Resistances", None, "Water", Resist, 6),
    e("Stats", None, "Encumbrance", Stats, 0),
    e("Stats", None, "HP", Stats, 1),
    e("Stats", None, "HP Regen", Stats, 2),
    e("Stats", None, "Illumination", Stats, 3),
    e("Stats", None, "Mana", Stats, 4),
    e("Stats", None, "Mana Regen", Stats, 5),
    e("Stats", None, "Perception", Stats, 6),
    e("Stats", None, "Picklocks", Stats, 7),
    e("Stats", None, "Spellcasting", Stats, 8),
    e("Stats", None, "Stealth", Stats, 9),
    e("Stats", None, "Thievery", Stats, 10),
    e("Stats", None, "Traps", Stats, 11),
];

/// The menu entries a profile shows (`VileWard` only on GreaterMUD and Paramud, which are not ported).
#[must_use]
pub fn menu(_profile: Profile) -> Vec<MenuEntry> {
    MENU.iter().copied().filter(|m| !m.greater_only).collect()
}

/// What `InvenFindBest` resolves a menu entry to: the ability numbers to add up, an item field added on top,
/// or the AC + DR combination.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Spec {
    abils: Vec<i32>,
    field: Field,
    ac_dr: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    None,
    Accy,
    ArmourClass,
    DamageResist,
}

fn spec(criterion: Criterion, index: i32) -> Spec {
    let abil = |a: &[i32]| Spec {
        abils: a.to_vec(),
        field: Field::None,
        ac_dr: false,
    };
    // the default `InvenFindBestAbils()`: one 0 entry, which scores nothing
    let none = abil(&[0]);
    match (criterion, index) {
        (Armour, 0) => Spec {
            ac_dr: true,
            ..none
        },
        (Armour, 1) => Spec {
            field: Field::ArmourClass,
            ..none
        },
        (Armour, 2) => Spec {
            field: Field::DamageResist,
            ..none
        },
        (Armour, 3) => abil(&[34]),
        (Armour, 4) => abil(&[24]),
        (Armour, 5) => abil(&[25]),
        (Armour, 6) => abil(&[1113]),
        (Attack, 0) => Spec {
            field: Field::Accy,
            ..abil(&[22, 105, 106])
        },
        (Attack, 1) => abil(&[116]),
        (Attack, 2) => abil(&[117]),
        (Attack, 3) => abil(&[118]),
        (Attack, 4) => abil(&[58]),
        (Attack, 5) => abil(&[72]),
        (Attack, 6) => abil(&[4]),
        (Resist, 0) => abil(&[3, 5, 66, 65, 147]),
        (Resist, 1) => abil(&[36]),
        (Resist, 2) => abil(&[3]),
        (Resist, 3) => abil(&[5]),
        (Resist, 4) => abil(&[66]),
        (Resist, 5) => abil(&[65]),
        (Resist, 6) => abil(&[147]),
        (Stats, 0) => abil(&[96]),
        (Stats, 1) => abil(&[88]),
        (Stats, 2) => abil(&[123]),
        (Stats, 3) => abil(&[13, 14]),
        (Stats, 4) => abil(&[69]),
        (Stats, 5) => abil(&[145]),
        (Stats, 6) => abil(&[77]),
        (Stats, 7) => abil(&[37, 180]),
        (Stats, 8) => abil(&[70]),
        (Stats, 9) => abil(&[27]),
        (Stats, 10) => abil(&[39]),
        (Stats, 11) => abil(&[40, 41, 179]),
        (Mystics, 0) => abil(&[91]),
        (Mystics, 1) => abil(&[94]),
        (Mystics, 2) => abil(&[90]),
        (Mystics, 3) => abil(&[93]),
        (Mystics, 4) => abil(&[89]),
        (Mystics, 5) => abil(&[92]),
        (Attribs, 0) => abil(&[48]),
        (Attribs, 1) => abil(&[49]),
        (Attribs, 2) => abil(&[47]),
        (Attribs, 3) => abil(&[44]),
        (Attribs, 4) => abil(&[46]),
        (Attribs, 5) => abil(&[45]),
        _ => none,
    }
}

/// The best candidate so far for one slot (`TypeEquipWinner`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Winner {
    pub number: i32,
    pub value: i32,
    pub enc_ratio: Currency,
    /// `ListIdx`: the entry's index in the slot's combo, where entry 0 is "(none)".
    pub list_index: usize,
}

/// The inputs of a Find Best or Next Best.
#[derive(Debug, Clone)]
pub struct FindBestRequest<'a> {
    /// The item numbers each slot's combo holds, without the "(none)" entry, in combo order.
    pub slot_lists: &'a [Vec<i32>],
    /// `nEquippedItem`.
    pub equipped: [i32; EQUIP_SLOTS],
    /// `chkEquipHold`: held slots are left alone.
    pub hold: [bool; EQUIP_SLOTS],
    /// `chkInvenNoLimited`: skip unique items (`Limit` above 0).
    pub no_limited: bool,
    /// `bInvenUse2ndWrist`.
    pub use_2nd_wrist: bool,
    /// `nInvenExcludedItems` without its placeholder entry; `None` before the first Find Best.
    pub excluded: Option<Vec<i32>>,
    /// `nInvenLastIndex`.
    pub last: LastFind,
}

/// What a Find Best or Next Best did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindBestResult {
    /// `nEquippedItem` afterwards.
    pub equipped: [i32; EQUIP_SLOTS],
    /// `tBest`: the winner per slot as last chosen (a slot the recheck emptied has none).
    pub winners: [Option<Winner>; EQUIP_SLOTS],
    /// `LastFindBest`: the criterion value each slot ended the pass with.
    pub slot_values: [i32; EQUIP_SLOTS],
    /// `nInvenExcludedItems` afterwards.
    pub excluded: Option<Vec<i32>>,
    /// `nInvenLastIndex` afterwards.
    pub last: LastFind,
    /// False when MME left at once (an ability only GreaterMUD has).
    pub ran: bool,
    /// Some slot has a winner.
    pub found: bool,
    /// The dialogs MME shows ("Nothing found.").
    pub messages: Vec<String>,
    /// The VB error MME's handler ran for (9 when Next Best meets the unallocated exclusion array).
    pub error: Option<i32>,
}

/// The message `InvenFindBest` shows when no slot got a winner.
pub const NOTHING_FOUND: &str = "Nothing found.";

fn vb6(e: vb::Vb6Error) -> RulesError {
    RulesError::Vb6(i32::try_from(e.code()).unwrap_or(0))
}

/// `TypeFindBestItem`: what one pass needs to know about an item.
#[derive(Debug, Clone, Copy)]
struct Scored {
    value: i32,
    enc_ratio: Currency,
    limited: bool,
    two_handed: bool,
}

/// `InvenFindBestItemValue`: the abilities of the criterion added up, plus the item field it names; or AC + DR.
fn item_value(item: &Item, spec: &Spec) -> Result<i32, RulesError> {
    if spec.ac_dr {
        return vb::add_i32(item.armour_class, item.damage_resist).map_err(vb6);
    }
    let mut total = 0;
    if spec.abils[0] > 0 {
        for slot in &item.abilities {
            if slot.ability > 0 && spec.abils.contains(&slot.ability) {
                total = vb::add_i32(total, slot.value).map_err(vb6)?;
            }
        }
    }
    let field = match spec.field {
        Field::None => return Ok(total),
        Field::Accy => item.accy,
        Field::ArmourClass => item.armour_class,
        Field::DamageResist => item.damage_resist,
    };
    vb::add_i32(total, field).map_err(vb6)
}

/// `InvenFindBestEval`: score an item once per pass; `None` for the "(none)" row and for an item the table lacks.
fn eval(
    world: &World,
    spec: &Spec,
    cache: &mut HashMap<i32, Option<Scored>>,
    number: i32,
) -> Result<Option<Scored>, RulesError> {
    if number < 1 {
        return Ok(None);
    }
    if let Some(done) = cache.get(&number) {
        return Ok(*done);
    }
    let scored = match world.get_item(number) {
        None => None,
        Some(item) => Some(Scored {
            value: item_value(item, spec)?,
            enc_ratio: get_enc_ratio(item.encum, item.armour_class, item.damage_resist)?,
            limited: item.limit > 0,
            two_handed: item.item_type == 1 && matches!(item.weapon_type, 1 | 3),
        }),
    };
    cache.insert(number, scored);
    Ok(scored)
}

/// The slot sharing a list with `slot` (wrists 6/7, fingers 9/10).
fn pair_of(slot: usize) -> Option<usize> {
    match slot {
        6 => Some(7),
        7 => Some(6),
        9 => Some(10),
        10 => Some(9),
        _ => None,
    }
}

/// `InvenFindBestDupeOK`: may `item` win `slot` without taking it from its pair?
fn dupe_ok(
    slot: usize,
    item: i32,
    best: &[Winner; EQUIP_SLOTS],
    req: &FindBestRequest,
    equipped: &[i32; EQUIP_SLOTS],
) -> bool {
    if slot == 7 && !req.use_2nd_wrist {
        return false;
    }
    let Some(pair) = pair_of(slot) else {
        return true;
    };
    if best[pair].number == item {
        return false;
    }
    if equipped[pair] == item
        && (pair < slot || req.hold[pair] || (pair == 7 && !req.use_2nd_wrist))
    {
        return false;
    }
    true
}

fn empty_winner() -> Winner {
    Winner {
        number: 0,
        value: 0,
        enc_ratio: Currency::from_scaled(0),
        list_index: 0,
    }
}

/// A Find Best from the menu: `InvenFindBest(index, criterion)` with `bInvenNextBest` off, which forgets the
/// exclusions.
///
/// # Errors
/// `Vb6(6)` when an item's score overflows a Long.
pub fn find_best(
    profile: Profile,
    world: &World,
    req: &FindBestRequest,
    criterion: Criterion,
    index: i32,
) -> Result<FindBestResult, RulesError> {
    run(profile, world, req, criterion, index, false)
}

/// Next Best: `InvenFindBest(nInvenLastIndex)` with `bInvenNextBest` on.
///
/// # Errors
/// `Vb6(6)` when an item's score overflows a Long.
pub fn next_best(
    profile: Profile,
    world: &World,
    req: &FindBestRequest,
) -> Result<FindBestResult, RulesError> {
    run(
        profile,
        world,
        req,
        req.last.criterion,
        req.last.index,
        true,
    )
}

#[allow(clippy::too_many_lines)] // one procedure in MME; the phases read in the same order
fn run(
    profile: Profile,
    world: &World,
    req: &FindBestRequest,
    criterion: Criterion,
    index: i32,
    next: bool,
) -> Result<FindBestResult, RulesError> {
    let Profile::Stock = profile;
    let mut res = FindBestResult {
        equipped: req.equipped,
        winners: [None; EQUIP_SLOTS],
        slot_values: [0; EQUIP_SLOTS],
        excluded: req.excluded.clone(),
        last: LastFind { index, criterion },
        ran: false,
        found: false,
        messages: vec![],
        error: None,
    };
    let sp = spec(criterion, index);
    // abilities of 1000 and up exist only on GreaterMUD and Paramud
    if sp.abils.iter().any(|&a| a >= 1000) {
        return Ok(res);
    }
    res.ran = true;
    if world.items.is_empty() {
        return Ok(res);
    }

    let mut equipped = req.equipped;
    let mut cache: HashMap<i32, Option<Scored>> = HashMap::new();
    let mut last_value = [0i32; EQUIP_SLOTS];
    let mut excluded: Option<Vec<i32>>;

    if next {
        excluded = req.excluded.clone();
        for x in 0..EQUIP_SLOTS {
            let n = equipped[x];
            if n > 0
                && let Some(scored) = eval(world, &sp, &mut cache, n)?
            {
                last_value[x] = scored.value;
                // InvenFindBestExclude asks the unallocated array for its upper bound
                let Some(list) = excluded.as_mut() else {
                    res.error = Some(9);
                    return Ok(res);
                };
                if !list.contains(&n) {
                    list.push(n);
                }
            }
        }
    } else {
        excluded = Some(vec![]);
    }
    let is_excluded = |n: i32| excluded.as_ref().is_some_and(|l| l.contains(&n));

    let mut best = [empty_winner(); EQUIP_SLOTS];
    let mut no_two_handed = false;
    let mut start = 0;
    'recheck: loop {
        for x in start..EQUIP_SLOTS {
            let list = req.slot_lists.get(x).map_or(&[][..], Vec::as_slice);
            if list.is_empty() || req.hold[x] {
                continue;
            }
            for (y, &n) in list.iter().enumerate() {
                let Some(scored) = eval(world, &sp, &mut cache, n)? else {
                    continue;
                };
                if req.no_limited && scored.limited {
                    continue;
                }
                if x == 16 && no_two_handed && scored.two_handed {
                    continue;
                }
                let value = scored.value;
                if value <= 0 {
                    continue; // never equip something that does nothing for us
                }
                if next && (value > last_value[x] || is_excluded(n)) {
                    continue;
                }
                if (value > best[x].value
                    || (value == best[x].value && scored.enc_ratio > best[x].enc_ratio))
                    && dupe_ok(x, n, &best, req, &equipped)
                {
                    best[x] = Winner {
                        number: n,
                        value,
                        enc_ratio: scored.enc_ratio,
                        list_index: y + 1,
                    };
                }
            }

            if best[x].number > 0 {
                let mut prev = equipped[x];
                equipped[x] = best[x].number;
                last_value[x] = best[x].value;
                // a wrist or finger winner can be the item its pair still wears: hand the pair our old item
                if let Some(pair) = pair_of(x)
                    && pair > x
                    && !req.hold[pair]
                    && equipped[pair] == best[x].number
                {
                    if prev == best[x].number {
                        prev = 0;
                    }
                    let in_list =
                        prev > 0 && req.slot_lists.get(pair).is_some_and(|l| l.contains(&prev));
                    equipped[pair] = if in_list { prev } else { 0 };
                }
            }
        }

        // two-handed weapon against the off-hand
        if equipped[15] > 0
            && equipped[16] > 0
            && (best[15].number > 0 || best[16].number > 0)
            && let Some(weapon) = eval(world, &sp, &mut cache, equipped[16])?
            && weapon.two_handed
        {
            let (hold_off, hold_weapon) = (req.hold[15], req.hold[16]);
            let redo = if !hold_off && !hold_weapon {
                if best[16].number > 0 && best[15].value >= best[16].value {
                    true
                } else {
                    equipped[15] = 0;
                    false
                }
            } else if hold_off && !hold_weapon {
                best[16].number > 0
            } else {
                if !hold_off && hold_weapon {
                    equipped[15] = 0;
                }
                false
            };
            if redo {
                equipped[16] = 0;
                best[16] = empty_winner();
                no_two_handed = true;
                start = 16;
                continue 'recheck;
            }
        }
        break;
    }

    res.found = best.iter().any(|b| b.number > 0);
    if !res.found {
        res.messages.push(NOTHING_FOUND.to_string());
    }
    for (x, b) in best.iter().enumerate() {
        res.winners[x] = (b.number > 0).then_some(*b);
    }
    res.equipped = equipped;
    res.slot_values = last_value;
    res.excluded = excluded;
    Ok(res)
}
