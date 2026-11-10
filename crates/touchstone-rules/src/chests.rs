//! Chest contents on the Sundry explorer. Card P4-32.
//!
//! Port of frmMain `cmdSundryChests_Click`: a container item (type 8) holds a `castsp` spell whose
//! textblock lists what the chest can give; `GetChestItems` ([`crate::textblock::get_chest_items`])
//! does the arithmetic. MME fills `lvOtherItemLoc` and sorts it by chance, highest first; the port
//! returns the rows in MME's insertion order and [`ChestRow::percent`] for the caller's sort.

use touchstone_model::World;
use touchstone_vb6::{Currency, val};

use crate::game_text::extract_shops_from_obtained_from;
use crate::item_value::get_item_value;
use crate::locations::{LocationArgs, LocationLairs, LocationList, LocationOptions, get_locations};
use crate::textblock::{ChestItems, get_chest_items};
use crate::{Profile, RulesError};

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
    for slot in item
        .abilities
        .iter()
        .filter(|a| a.ability == 43 && a.value > 0)
    {
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

/// The first half of `cmdSundryChests_Click`: the chances `GetChestItems` leaves for the chest, or the
/// result the button ends with before it gets that far.
fn gather(world: &World, item_number: i32) -> Result<ChestItems, ChestResult> {
    let Some(item) = world.get_item(item_number) else {
        return Err(ChestResult::NoSuchItem(item_number));
    };
    if item.item_type != 8 {
        return Err(ChestResult::NotContainer(item.name.clone()));
    }
    let Some(data) = chest_data(world, item) else {
        return Err(ChestResult::NoChestData);
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
                return Err(ChestResult::Rows(Vec::new()));
            };
            get_chest_items(world, &mut chest, tb, &mut nest, None);
        }
    }
    Ok(chest)
}

/// Port of frmMain `cmdSundryChests_Click` for the selected item number.
#[must_use]
pub fn chest_contents(world: &World, item_number: i32, hide_record_numbers: bool) -> ChestResult {
    let chest = match gather(world, item_number) {
        Ok(c) => c,
        Err(r) => return r,
    };
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

// ---------------------------------------------------------------------------------------------
// The Chests tab. Card CH-001. A touchstone addition: MME shows one chest's chances from the Items
// explorer and nothing else, so the sale values below are not MME numbers (docs/differences.md).
// ---------------------------------------------------------------------------------------------

/// What the sell price of an item the chest can give comes to, or why it has none.
#[derive(Debug, Clone, PartialEq)]
pub struct SalePrice {
    /// What a shop pays at the charm, in copper (`ItemValue::copper_sell`); 0 when nothing buys it.
    pub copper: f64,
    /// `GetItemValue`'s wording of that price ("N Copper (M Coin)"); "(no value)" when it has none.
    pub text: String,
    /// The short form ("12G").
    pub short: String,
    /// False for "can't be sold": no shop buys the item, or its value is 0.
    pub sellable: bool,
}

/// One line of a chest's contents.
#[derive(Debug, Clone, PartialEq)]
pub struct ChestContent {
    pub item: i32,
    pub name: String,
    /// The chance as MME shows it (`Round(chance * 100, 1)`).
    pub percent: Currency,
    /// `percent` as MME words it, with its "%".
    pub percent_text: String,
    pub sale: SalePrice,
    /// The chance (unrounded) times the sale price in copper: what this line adds to the average.
    pub expected_copper: f64,
}

/// What the Chests tab shows for one chest.
#[derive(Debug, Clone)]
pub struct ChestDetail {
    pub item: i32,
    pub name: String,
    /// Best chance first.
    pub contents: Vec<ChestContent>,
    /// The average sale of one opening in copper: the sum of the lines' `expected_copper`.
    pub average_copper: f64,
    /// Where the chest comes from, as the Items pane lists it (monsters with their drop chance, and
    /// the rooms, textblocks and NPCs), in the order `GetLocations` adds them; the caller sorts by chance as
    /// the Items pane does.
    pub droppers: LocationList,
}

/// One row of the chest list.
#[derive(Debug, Clone, PartialEq)]
pub struct ChestSummary {
    pub item: i32,
    pub name: String,
    /// How many different items it can give.
    pub contents: usize,
    pub average_copper: f64,
    /// How many monsters carry the chest.
    pub dropped_by: usize,
}

/// A shop buys an item when it is one of the shop's stock lines (`Shop #n`, `Shop(sell) #n` or
/// `Shop(nogen) #n` in the item's Obtained From).
fn some_shop_buys(item: &touchstone_model::Item) -> bool {
    extract_shops_from_obtained_from(item.obtained_from.as_deref().unwrap_or(""))
        .is_ok_and(|s| !s.is_empty())
}

/// The sell price of `number` at `charm`: `GetItemValue` with no shop (a shop's markup only moves the
/// buy price), taken as the price when some shop buys it and it is worth something.
fn sale_price(
    world: &World,
    profile: Profile,
    number: i32,
    charm: i16,
) -> Result<SalePrice, RulesError> {
    let item = world.get_item(number);
    let v = get_item_value(profile, number, item, charm, 0, 0, None, true)?;
    let sellable = item.is_some_and(some_shop_buys) && v.copper_sell > 0.0;
    if !sellable {
        return Ok(SalePrice {
            copper: 0.0,
            text: "can't be sold".into(),
            short: String::new(),
            sellable: false,
        });
    }
    Ok(SalePrice {
        copper: v.copper_sell,
        text: v.friendly_sell,
        short: v.friendly_sell_short,
        sellable: true,
    })
}

/// The chest's lines with their sale price: the chance of each item (the rounded one MME shows and the
/// exact one the average uses) in MME's order, best first.
fn contents_of(
    world: &World,
    profile: Profile,
    item_number: i32,
    charm: i16,
) -> Result<Vec<ChestContent>, RulesError> {
    let Ok(chest) = gather(world, item_number) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    if chest.entries.len() > 1 {
        for e in chest.entries.iter().filter(|e| e.item > 0) {
            let Ok(hundred) = e.percent.checked_mul(Currency::from_scaled(100 * 10_000)) else {
                continue;
            };
            let p = round1(hundred);
            let sale = sale_price(world, profile, e.item, charm)?;
            let chance = e.percent.scaled() as f64 / 10_000.0;
            out.push(ChestContent {
                item: e.item,
                name: item_name(world, e.item, true),
                percent: p,
                percent_text: format!("{}%", cur_str(p)),
                expected_copper: chance * sale.copper,
                sale,
            });
        }
    }
    out.sort_by_key(|r| std::cmp::Reverse(r.percent));
    Ok(out)
}

/// The average sale of one opening: the lines' expected values added up (0, not -0, for a chest with none).
fn average_of(contents: &[ChestContent]) -> f64 {
    contents
        .iter()
        .map(|c| c.expected_copper)
        .fold(0.0, |a, b| a + b)
}

/// How many monsters carry an item: the `Monster #n` entries of its Obtained From.
fn monster_droppers(item: &touchstone_model::Item) -> usize {
    item.obtained_from
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|p| p.trim_start().to_lowercase().starts_with("monster #"))
        .count()
}

/// Every chest item (`ItemIsChest`: item type 8) in item order, with how many items it can give, the
/// average sale of one opening at `charm` and how many monsters drop it.
///
/// # Errors
/// [`RulesError`] from the price of an item.
pub fn chest_summary(
    world: &World,
    profile: Profile,
    charm: i16,
) -> Result<Vec<ChestSummary>, RulesError> {
    let mut out = Vec::new();
    for item in world.items.iter().filter(|i| i.item_type == 8) {
        let contents = contents_of(world, profile, item.number, charm)?;
        out.push(ChestSummary {
            item: item.number,
            name: item.name.clone(),
            contents: contents.len(),
            average_copper: average_of(&contents),
            dropped_by: monster_droppers(item),
        });
    }
    Ok(out)
}

/// One chest's contents and droppers; `None` when `item_number` is not a chest item.
///
/// # Errors
/// [`RulesError`] from the price of an item.
pub fn chest_detail(
    world: &World,
    profile: Profile,
    lairs: &mut dyn LocationLairs,
    opt: &LocationOptions,
    item_number: i32,
    charm: i16,
) -> Result<Option<ChestDetail>, RulesError> {
    let Some(item) = world.get_item(item_number).filter(|i| i.item_type == 8) else {
        return Ok(None);
    };
    let contents = contents_of(world, profile, item_number, charm)?;
    let mut droppers = LocationList::default();
    // the Items pane's reference list for the chest
    get_locations(
        world,
        lairs,
        opt,
        &LocationArgs {
            loc: item.obtained_from.as_deref().unwrap_or(""),
            dont_clear: true,
            aux_value: item_number,
            percent_column: true,
            ..LocationArgs::default()
        },
        &mut droppers,
    );
    Ok(Some(ChestDetail {
        item: item_number,
        name: item.name.clone(),
        average_copper: average_of(&contents),
        contents,
        droppers,
    }))
}
