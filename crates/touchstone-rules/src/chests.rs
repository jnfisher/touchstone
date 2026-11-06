//! Chest contents on the Sundry explorer. Card P4-32.
//!
//! Port of frmMain `cmdSundryChests_Click`: a container item (type 8) holds a `castsp` spell whose
//! textblock lists what the chest can give; `GetChestItems` ([`crate::textblock::get_chest_items`])
//! does the arithmetic. MME fills `lvOtherItemLoc` and sorts it by chance, highest first; the port
//! returns the rows in MME's insertion order and [`ChestRow::percent`] for the caller's sort.

use touchstone_model::World;
use touchstone_vb6::{Currency, val};

use crate::textblock::{ChestItems, get_chest_items};

/// One line of the chest list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChestRow {
    /// `nPercent & "%"`.
    pub text: String,
    /// `nPercent` as MME stores it in the row's Tag (and sorts by).
    pub percent: Currency,
    /// `Item: <GetItemName>`.
    pub item_text: String,
    pub item: i32,
}

/// What the button does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChestResult {
    /// `Item #n not found.`
    NoSuchItem(i32),
    /// `<name> is not a container.`
    NotContainer(String),
    /// `Failed to find chest data.`
    NoChestData,
    /// The list (empty when the chest gives nothing).
    Rows(Vec<ChestRow>),
}

/// `Round(c, 1)` on a Currency, half to even.
fn round1(c: Currency) -> Currency {
    let s = c.scaled();
    let q = 1_000;
    let d = s.div_euclid(q);
    let r = s.rem_euclid(q);
    let d = match (r * 2).cmp(&q) {
        std::cmp::Ordering::Less => d,
        std::cmp::Ordering::Greater => d + 1,
        std::cmp::Ordering::Equal => d + (d & 1),
    };
    Currency::from_scaled(d * q)
}

/// `CStr(Currency)`.
fn cur_str(c: Currency) -> String {
    let s = c.scaled();
    let sign = if s < 0 { "-" } else { "" };
    let a = s.unsigned_abs();
    let (whole, frac) = (a / 10_000, a % 10_000);
    if frac == 0 {
        format!("{sign}{whole}")
    } else {
        let f = format!("{frac:04}");
        format!("{sign}{whole}.{}", f.trim_end_matches('0'))
    }
}

/// `GetItemName(n, bHideRecordNumbers)`.
fn item_name(world: &World, n: i32, hide: bool) -> String {
    if n == 0 {
        return "None".into();
    }
    match world.get_item(n) {
        None => n.to_string(),
        Some(i) if hide => i.name.clone(),
        Some(i) => format!("{}({n})", i.name),
    }
}

/// The textblock text the chest's spell points at (the first `castsp` ability of the first `Abil`
/// that is the chest spell ability 43), lower-cased, as `sData`.
fn chest_data(world: &World, item: &touchstone_model::Item) -> Option<String> {
    for slot in item.abilities.iter().filter(|a| a.ability == 43 && a.value > 0) {
        let Some(spell) = world.get_spell(slot.value) else {
            continue;
        };
        for a in spell.abilities.iter().filter(|a| a.ability == 148) {
            let tb = if a.value == 0 {
                if spell.min_base > 0 {
                    spell.min_base
                } else {
                    spell.max_base
                }
            } else {
                a.value
            };
            if let Some(row) = world.get_tbinfo(tb) {
                let action = row.action.as_deref().unwrap_or("");
                if action != "\0" {
                    return Some(action.to_lowercase());
                }
            }
        }
    }
    None
}

/// Port of frmMain `cmdSundryChests_Click` for the selected item number.
#[must_use]
pub fn chest_contents(world: &World, item_number: i32, hide_record_numbers: bool) -> ChestResult {
    let Some(item) = world.get_item(item_number) else {
        return ChestResult::NoSuchItem(item_number);
    };
    if item.item_type != 8 {
        return ChestResult::NotContainer(item.name.clone());
    }
    let Some(data) = chest_data(world, item) else {
        return ChestResult::NoChestData;
    };
    let chars: Vec<char> = data.chars().collect();
    let mut chest = ChestItems::new();
    let mut nest = 0;
    let mut pos = 0usize;
    while let Some(at) = find_from(&chars, pos, "random ") {
        pos = at + "random ".len();
        let mut x = pos;
        while x < chars.len() && chars[x].is_ascii_digit() {
            x += 1;
        }
        if x > pos {
            let text: String = chars[pos..x].iter().collect();
            // `Long` overflow: the handler ends the procedure before any row is made
            let Ok(tb) = touchstone_vb6::clng(val(&text)) else {
                return ChestResult::Rows(Vec::new());
            };
            get_chest_items(world, &mut chest, tb, &mut nest, None);
        }
    }
    let mut rows = Vec::new();
    if chest.entries.len() > 1 {
        for e in chest.entries.iter().filter(|e| e.item > 0) {
            let Ok(hundred) = e.percent.checked_mul(Currency::from_scaled(100 * 10_000)) else {
                continue;
            };
            let p = round1(hundred);
            rows.push(ChestRow {
                text: format!("{}%", cur_str(p)),
                percent: p,
                item_text: format!("Item: {}", item_name(world, e.item, hide_record_numbers)),
                item: e.item,
            });
        }
    }
    ChestResult::Rows(rows)
}

fn find_from(h: &[char], from: usize, needle: &str) -> Option<usize> {
    let n: Vec<char> = needle.chars().collect();
    if h.len() < n.len() {
        return None;
    }
    (from..=h.len() - n.len()).find(|&i| h[i..i + n.len()] == n[..])
}

/// The rows in the order the list shows them: chance descending (a stable sort; MME's ListView
/// leaves ties in an order the oracle cannot see).
#[must_use]
pub fn sorted_by_chance(mut rows: Vec<ChestRow>) -> Vec<ChestRow> {
    rows.sort_by_key(|r| std::cmp::Reverse(r.percent));
    rows
}
