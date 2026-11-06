//! The Item Manager's list actions (FEAT-LISTS and FEAT-EQ, card P4-19).
//!
//! MME's Item Manager is `frmMain.lvItemManager`; here it is `Vec<ListRow>` (see
//! [`crate::game_text::ListRow`]: sub item 0 is the Name, 1 the flag/Action cell, 2 QTY, 3 Source,
//! 4 Enc). A ListView selection is a `&[bool]` by row. The procedures that act on the list port
//! modListViewExt `LV_SetActionCell`, `LV_CopySelectedActionsToClipboard`, `LV_DeleteSelected` and
//! `LV_InvertSelection`; `saved_stacks` is the Item Manager half of frmMain `SaveCharacter`.
//!
//! MsgBoxes become callbacks. Anything MME leaves to the caller (`RefreshAll`, `bPromptSave`,
//! focus) is reported in the outcome. A VB6 run-time error ends a procedure the way its handler
//! does: `LV_SetActionCell` stops its loop and still reports the flags it had set, so the outcome
//! carries the error beside them.

use touchstone_model::World;
use touchstone_vb6::{cint, clng, val};

use crate::RulesError;
use crate::game_text::{ListRow, instr_ci, parse_action_and_qty, trim};
use crate::gear::ItemManagerRow;
use crate::mmec::ItemStack;
use crate::textblock::obtained_from_is_gettable;

type R<T> = Result<T, RulesError>;

/// `LV_SetActionCell`'s action indexes: the button indexes of `cmdCompareNav`.
pub mod action {
    pub const DROP_HIDE: i32 = 9;
    pub const PICKUP_USE: i32 = 10;
    pub const SELL_BUY: i32 = 11;
    pub const CARRIED: i32 = 12;
    pub const MINUS: i32 = 15;
    pub const PLUS: i32 = 16;
    pub const STASH: i32 = 17;
    pub const CLEAR: i32 = 18;
}

/// A Long expression that overflowed, or a missing sub item (`ListSubItems(i)` out of range).
const OVERFLOW: RulesError = RulesError::Vb6(6);
const OUT_OF_BOUNDS: RulesError = RulesError::Vb6(35600);

fn vb<T>(r: Result<T, touchstone_vb6::Vb6Error>) -> R<T> {
    r.map_err(|e| RulesError::Vb6(e.code() as i32))
}

fn sub_text(row: &ListRow, vb_index: usize) -> R<&str> {
    row.subs
        .get(vb_index - 1)
        .map(|s| s.text.as_str())
        .ok_or(OUT_OF_BOUNDS)
}

fn is_selected(selected: &[bool], i: usize) -> bool {
    selected.get(i).copied().unwrap_or(false)
}

fn with_qty(base: &str, qty: i32) -> String {
    if qty > 1 {
        format!("{base} x{qty}")
    } else {
        base.to_string()
    }
}

/// The weight question of the CARRIED action, word for word.
#[must_use]
pub fn carried_weight_question(item_name: &str) -> String {
    format!(
        "Subtract {item_name}'s encumbrance from the 'Additional Item Weight' field on EQ tab? e.g. Is its weight currently included in that value?\r\n\r\nIf you recently did a 'Paste Character' (different from 'Paste Items' here) where this item was in your inventory, then you probably want to answer yes.\r\n\r\nCarried items are counted in the encumbrance calculation along with equipped items. If you answer no and it's currently included in the additional weight field, it will effectively be calculated twice, putting your calculated encumbrance over what it should be."
    )
}

/// What `LV_SetActionCell` leaves for its caller.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActionOutcome {
    /// `bCarriedAddedOrRemoved`: the caller runs `RefreshAll`.
    pub carried_changed: bool,
    /// `bStashChanged`.
    pub stash_changed: bool,
    /// At least one row was selected (MME then focuses the list).
    pub any_selected: bool,
    /// The run-time error that ended the loop; the rows before it keep their changes.
    pub error: Option<RulesError>,
}

/// Port of modListViewExt `LV_SetActionCell`: applies the Action button `action_index` (see
/// [`action`]) to every selected row. `add_weight` is `Val(txtInvenAddWeight.Text)`; `ask` is the
/// "Subtract ... encumbrance?" box (true for Yes).
pub fn set_action_cell(
    world: &World,
    rows: &mut [ListRow],
    selected: &[bool],
    action_index: i32,
    add_weight: &mut i32,
    ask: &mut dyn FnMut(&str) -> bool,
) -> ActionOutcome {
    let mut out = ActionOutcome::default();
    if rows.is_empty() {
        return out;
    }
    for (i, row) in rows.iter_mut().enumerate() {
        if !is_selected(selected, i) {
            continue;
        }
        out.any_selected = true;
        if let Err(e) = action_one(world, row, action_index, add_weight, ask, &mut out) {
            out.error = Some(e);
            break;
        }
    }
    out
}

fn action_one(
    world: &World,
    row: &mut ListRow,
    action_index: i32,
    add_weight: &mut i32,
    ask: &mut dyn FnMut(&str) -> bool,
    out: &mut ActionOutcome,
) -> R<()> {
    while row.subs.len() < 2 {
        row.subs.push(crate::game_text::SubItem::new(""));
    }
    let qty_field = vb(clng(val(sub_text(row, 3)?)))?;
    let cur_text = trim(&row.subs[1].text).to_string();
    let (base, mut qty) = parse_action_and_qty(&cur_text, 0)?;
    if base.is_empty() && qty_field > 1 {
        qty = qty_field;
    }

    use action::*;
    let new_text = match action_index {
        DROP_HIDE => match base.as_str() {
            "DROP" => with_qty("HIDE", qty),
            "HIDE" => String::new(),
            _ => with_qty("DROP", qty),
        },
        PICKUP_USE => match base.as_str() {
            "PICKUP" => with_qty("USE", qty),
            "USE" => String::new(),
            _ => with_qty("PICKUP", qty),
        },
        SELL_BUY => match base.as_str() {
            "SELL" => with_qty("BUY", qty),
            "BUY" => String::new(),
            _ => with_qty("SELL", qty),
        },
        CARRIED => {
            if base == "CARRIED" {
                String::new()
            } else {
                with_qty("CARRIED", qty)
            }
        }
        STASH => {
            if base == "STASH" {
                String::new()
            } else {
                with_qty("STASH", qty)
            }
        }
        MINUS | PLUS => {
            if !matches!(
                base.as_str(),
                "DROP" | "HIDE" | "BUY" | "SELL" | "PICKUP" | "USE" | "STASH" | "CARRIED"
            ) {
                return Ok(());
            }
            if action_index == MINUS {
                if qty > 1 {
                    qty -= 1;
                }
                if qty <= 1 {
                    base.clone()
                } else {
                    format!("{base} x{qty}")
                }
            } else {
                qty = qty.checked_add(1).ok_or(OVERFLOW)?;
                format!("{base} x{qty}")
            }
        }
        CLEAR => String::new(),
        _ => return Ok(()),
    };

    if new_text != cur_text {
        if action_index == CARRIED || base == "CARRIED" {
            out.carried_changed = true;
        }
        if action_index == STASH || base == "STASH" {
            out.stash_changed = true;
        }
        if base != "CARRIED" && new_text.starts_with("CARRIED") {
            let n_enc = world
                .get_item(vb(clng(val(&row.text)))?)
                .map_or(0, |it| it.encum);
            // VB6 evaluates every operand of the And chain: the product can overflow and the
            // fourth sub item can be missing even when the first test already failed
            let product = n_enc.checked_mul(qty).ok_or(OVERFLOW)?;
            let fits = f64::from(*add_weight) >= f64::from(product);
            let in_inventory = instr_ci(sub_text(row, 4)?, "Inventory") > 0;
            if n_enc > 0 && fits && in_inventory {
                let name = row.subs[0].text.clone();
                if ask(&carried_weight_question(&name)) {
                    *add_weight = (f64::from(*add_weight) - f64::from(product)) as i32;
                }
            }
        }
        row.subs[1].text = new_text;
    }
    Ok(())
}

/// Port of modListViewExt `LV_CopySelectedActionsToClipboard`: the game commands for the selected
/// rows, one per unit of quantity, CRLF between; `None` when there is nothing to copy (MME then
/// leaves the clipboard alone).
///
/// # Errors
/// Overflow (6) from a quantity or item number, or out of memory (7) for a result VB6 could not
/// build; MME has no handler here, so the error reaches the caller.
pub fn copy_selected_actions(
    world: &World,
    rows: &[ListRow],
    selected: &[bool],
) -> R<Option<String>> {
    /// More commands than this is VB6's "out of memory" long before it is a clipboard.
    const MAX_LINES: usize = 1_000_000;
    let mut lines: Vec<String> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        if !is_selected(selected, i) || row.subs.len() < 2 {
            continue;
        }
        let name = trim(&row.subs[0].text).to_string();
        let (act, qty) = parse_action_and_qty(trim(&row.subs[1].text), 0)?;
        let cmd = match act.as_str() {
            "DROP" => format!("drop {name}"),
            "HIDE" => format!("hide {name}"),
            "SELL" => format!("sell {name}"),
            "BUY" => format!("buy {name}"),
            "PICKUP" => format!("get {name}"),
            "USE" => {
                let n = vb(clng(val(&row.text)))?;
                if n > 0 {
                    // GetItemType is -1 for an item that is not there
                    match world.get_item(n).map_or(-1, |it| it.item_type) {
                        0 | 1 => format!("equip {name}"),
                        8 => format!("open {name}"),
                        9 => format!("read {name}"),
                        _ => format!("use {name}"),
                    }
                } else {
                    format!("use {name}")
                }
            }
            "STASH" => format!("stash {name}"),
            _ => continue,
        };
        let n = usize::try_from(qty).unwrap_or(0);
        if lines.len().saturating_add(n) > MAX_LINES {
            return Err(RulesError::Vb6(7));
        }
        lines.extend(std::iter::repeat_n(cmd, n));
    }
    if lines.is_empty() {
        Ok(None)
    } else {
        Ok(Some(lines.join("\r\n")))
    }
}

/// What `LV_DeleteSelected` leaves for its caller.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeleteOutcome {
    /// More than one row was selected and the confirmation was not Yes: nothing changed.
    pub declined: bool,
    pub removed: usize,
    pub carried_changed: bool,
    pub stash_changed: bool,
    /// The row MME selects afterwards (0-based, in the shortened list); `None` when none remain.
    pub next_selected: Option<usize>,
}

/// Port of modListViewExt `LV_DeleteSelected`. `confirm` is "Are you sure you want to delete the
/// selected rows?" (asked only for more than one row; its default button is No).
pub fn delete_selected(
    rows: &mut Vec<ListRow>,
    selected: &[bool],
    confirm: &mut dyn FnMut() -> bool,
) -> DeleteOutcome {
    let mut out = DeleteOutcome::default();
    let original = rows.len();
    let picked: Vec<usize> = (0..original)
        .filter(|&i| is_selected(selected, i))
        .collect();
    let Some(&highest) = picked.last() else {
        return out;
    };
    if picked.len() > 1 && !confirm() {
        out.declined = true;
        return out;
    }
    // the row that was just after the highest selected one, once the selected rows are gone
    let target = (highest + 1) as i64 - picked.len() as i64 + 1;
    for &i in picked.iter().rev() {
        if let Some(flag) = rows[i].subs.get(1) {
            if instr_ci(&flag.text, "CARRIED") > 0 {
                out.carried_changed = true;
            }
            if instr_ci(&flag.text, "STASH") > 0 {
                out.stash_changed = true;
            }
        }
        rows.remove(i);
    }
    out.removed = picked.len();
    if !rows.is_empty() {
        let count = rows.len() as i64;
        out.next_selected = Some((target.clamp(1, count) - 1) as usize);
    }
    out
}

/// Port of modListViewExt `LV_InvertSelection` over `count` rows.
pub fn invert_selection(selected: &mut Vec<bool>, count: usize) {
    selected.resize(count, false);
    for s in selected.iter_mut() {
        *s = !*s;
    }
}

/// `SaveCharacter` writes at most this many rows per list.
pub const MAX_SAVED: usize = 50;

fn saved_list(rows: &[ListRow], word: &str) -> R<Vec<ItemStack>> {
    let mut out = Vec::new();
    for row in rows {
        if row.subs.len() < 3 {
            continue;
        }
        let flag = &row.subs[1].text;
        if instr_ci(flag, word) == 0 {
            continue;
        }
        let mut y = 1;
        if instr_ci(flag, " x") > 0 {
            // Split is a binary compare where the InStr above is not
            let parts: Vec<&str> = flag.split(" x").collect();
            if parts.len() >= 2 {
                y = i32::from(vb(cint(val(parts[1])))?);
            }
        }
        if y < 1 {
            y = 1;
        }
        let item = vb(clng(val(&row.text)))?;
        out.push(ItemStack { item, qty: y });
        if out.len() >= MAX_SAVED {
            break;
        }
    }
    Ok(out)
}

/// The Item Manager half of frmMain `SaveCharacter`: the `IM_CARRIED` and `IM_STASH` lists, rows
/// whose flag mentions CARRIED / STASH with the quantity of a trailing ` xN`, 50 at most each.
///
/// # Errors
/// Overflow (6) when a quantity does not fit an Integer.
pub fn saved_stacks(rows: &[ListRow]) -> R<(Vec<ItemStack>, Vec<ItemStack>)> {
    Ok((saved_list(rows, "CARRIED")?, saved_list(rows, "STASH")?))
}

/// The rows as `CalcCharacterStats` reads them: the item number and the sub item texts.
#[must_use]
pub fn gear_rows(rows: &[ListRow]) -> Vec<ItemManagerRow> {
    rows.iter()
        .map(|r| ItemManagerRow {
            text: r.text.clone(),
            sub_items: r.subs.iter().map(|s| s.text.clone()).collect(),
        })
        .collect()
}

/// The two Item Manager rows `CalcCharacterStats` sees for saved stacks that have no list yet
/// (a character loaded but not opened in the Item Manager): item number, name, `CARRIED xN`.
#[must_use]
pub fn gear_rows_for_carried(world: &World, carried: &[ItemStack]) -> Vec<ItemManagerRow> {
    carried
        .iter()
        .filter_map(|s| {
            let item = world.get_item(s.item)?;
            let flag = with_qty("CARRIED", s.qty);
            Some(ItemManagerRow {
                text: s.item.to_string(),
                sub_items: vec![item.name.clone(), flag, s.qty.to_string()],
            })
        })
        .collect()
}

/// `ItemIsGetable`: may the item be put in the Item Manager by hand?
#[must_use]
pub fn item_is_getable(world: &World, item: i32) -> bool {
    world.get_item(item).is_some_and(|it| {
        it.gettable || obtained_from_is_gettable(it.obtained_from.as_deref().unwrap_or_default())
    })
}
