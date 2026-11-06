//! Parsed-item matching, best shop price and Item Manager rows (FAM-22, second half). Card P3-39.
//!
//! P3-38 turned pasted text into four name lists. This half looks each name up in the Items
//! table, picks the shop that sells (or buys) it best, and adds a row to the Item Manager list.
//! MME's ListView becomes `Vec<ListRow>` (the visible columns plus the two sort tags); the hidden
//! row-sequence column is UI plumbing and is not modelled. MsgBox prompts become a callback and
//! the boxes MME shows become [`Event`]s, in order.
//!
//! The record-number lookups MME does with `Seek` are `World::get_*`; the name scan of
//! `GetItemsByExactNameArr` walks `World::items` in their stored order, as MME walks the table.

use touchstone_model::{Item, World};
use touchstone_vb6::{Vb6Error, add_i32, cdbl_str, cint, clng, cstr, is_numeric, val};

use super::{
    ItemParseResult, array_count, instr_ci, instrrev, lcase, left, len, mid, mid_from,
    normalize_equipped_name, parse_name_and_qty, safe_array_has_data, squeeze_spaces, trim,
};
use crate::gear::{Character, EQUIP_SLOTS, item_is_usable_by_char};
use crate::item_value::{ItemValue, get_item_value};
use crate::textblock::{extract_map_room, obtained_from_is_gettable};
use crate::{Profile, RulesError};

type R<T> = Result<T, RulesError>;

fn vb<T>(r: Result<T, Vb6Error>) -> R<T> {
    r.map_err(|e| RulesError::Vb6(e.code() as i32))
}

/// `Err.Description` for the run-time errors the port can raise.
fn err_description(e: &RulesError) -> String {
    match e {
        RulesError::Vb6(6) => "Overflow".into(),
        RulesError::Vb6(13) => "Type mismatch".into(),
        RulesError::Vb6(11) => "Division by zero".into(),
        RulesError::Vb6(9) => "Subscript out of range".into(),
        RulesError::Vb6(5) => "Invalid procedure call or argument".into(),
        other => other.to_string(),
    }
}

fn err_number(e: &RulesError) -> u32 {
    match e {
        RulesError::Vb6(n) => *n as u32,
        RulesError::Unsupported(_) => 0,
    }
}

/// MME's `ItemMatch`: the Items columns the Item Manager rows need.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemMatch {
    pub number: i32,
    pub name: String,
    pub item_type: i32,
    pub worn: i32,
    pub weapon_type: i32,
    pub encum: i32,
    pub obtained_from: String,
    /// 0 or 1: the `Gettable` column, or an Obtained From that names an NPC or textblock.
    pub gettable: i32,
}

/// MME's `ShopToken`: one `Shop #n` entry of an Obtained From text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShopToken {
    pub shop_number: i32,
    /// `(sell)`: the shop only buys the item from the player.
    pub no_buy: bool,
    pub no_sell: bool,
}

/// The tag of a ListView sub item: the Worn column holds an Integer, the Value column a String.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubTag {
    Int(i32),
    Text(String),
}

/// One sub item (`ListSubItems(i)`), numbered from 1 in MME, from 0 here.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubItem {
    pub text: String,
    pub tag: Option<SubTag>,
}

impl SubItem {
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            tag: None,
        }
    }
}

/// One Item Manager row: `Text` is the item number; sub items are Name, Flag, QTY, Source, Enc,
/// Type, Worn, Usable, Value, Shop.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListRow {
    pub text: String,
    pub subs: Vec<SubItem>,
}

/// What MME shows or records while a procedure runs: a message box, or an error its
/// `HandleError` caught.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    MsgBox(String),
    HandledError { source: &'static str, number: u32 },
}

/// What the matching code reads besides the database: MME's globals and frmMain controls.
#[derive(Debug, Clone, Copy)]
pub struct MatchEnv<'a> {
    pub world: &'a World,
    /// `bOnlyInGame`: skip Items rows whose `In Game` is 0 when matching names.
    pub only_in_game: bool,
    /// `bHideRecordNumbers`.
    pub hide_record_numbers: bool,
    /// `frmMain.txtCharStats(5).Tag`: the charm, as text.
    pub charm_tag: &'a str,
    /// The character `ItemIsUsableByChar` judges by.
    pub who: &'a Character,
    /// `nEquippedItem`.
    pub equipped: &'a [i32; EQUIP_SLOTS],
}

impl MatchEnv<'_> {
    /// `nCharm = val(frmMain.txtCharStats(5).Tag)` into an Integer.
    fn charm(&self) -> R<i16> {
        vb(cint(val(self.charm_tag)))
    }
}

fn nz_item_match(item: &Item) -> ItemMatch {
    let obtained = item.obtained_from.clone().unwrap_or_default();
    let gettable = i32::from(item.gettable || obtained_from_is_gettable(&obtained));
    ItemMatch {
        number: item.number,
        name: item.name.clone(),
        item_type: item.item_type,
        worn: item.worn,
        weapon_type: item.weapon_type,
        encum: item.encum,
        obtained_from: obtained,
        gettable,
    }
}

/// Port of modItemParse `GetItemsByExactNameArr`: every Items row whose trimmed name equals the
/// trimmed `exact_name`, ignoring case, in table order.
#[must_use]
pub fn get_items_by_exact_name(env: &MatchEnv, exact_name: &str) -> Vec<ItemMatch> {
    let want = lcase(trim(exact_name));
    env.world
        .items
        .iter()
        .filter(|it| (!env.only_in_game || it.in_game) && lcase(trim(&it.name)) == want)
        .map(nz_item_match)
        .collect()
}

/// Port of modItemParse `ExtractFirstNumber`: the first run of digits, 0 when there is none.
///
/// # Errors
/// Overflow (6) when the digits do not fit a Long; MME has no handler there.
pub fn extract_first_number(s: &str) -> R<i32> {
    let mut digits = String::new();
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else if !digits.is_empty() {
            break;
        }
    }
    if digits.is_empty() {
        return Ok(0);
    }
    vb(cdbl_str(&digits).and_then(clng))
}

/// Port of modItemParse `ExtractShopsFromObtainedFrom`: the `Shop #n` entries, in text order.
/// `(sell)` anywhere in an entry marks the shop sell-only.
///
/// # Errors
/// Overflow (6) from [`extract_first_number`].
pub fn extract_shops_from_obtained_from(obtained: &str) -> R<Vec<ShopToken>> {
    let mut out = Vec::new();
    if trim(obtained).is_empty() {
        return Ok(out);
    }
    for part in obtained.split(',') {
        let t = lcase(&squeeze_spaces(trim(part)));
        if left(&t, 4) == "shop" {
            let has_sell = instr_ci(&t, "(sell)") > 0;
            let n = extract_first_number(&t)?;
            if n > 0 {
                out.push(ShopToken {
                    shop_number: n,
                    no_buy: has_sell,
                    no_sell: false,
                });
            }
        }
    }
    Ok(out)
}

fn auto_append(base: &str, add: &str) -> String {
    if !base.is_empty() {
        if !trim(add).is_empty() {
            format!("{base}, {add}")
        } else {
            base.to_string()
        }
    } else {
        add.to_string()
    }
}

/// Port of modMMudDatabase `GetRoomName` for a map and room number.
fn get_room_name(world: &World, map: i32, room: i32, no_number: bool) -> String {
    if map == 0 || room == 0 {
        return "?".into();
    }
    match world.get_room(map, room) {
        None => format!("{map}/{room}"),
        Some(r) => {
            let mut name = r.name.clone().unwrap_or_default();
            if name.is_empty() {
                name = "(no name)".into();
            }
            if !no_number {
                name = format!("{name} ({map}/{room})");
            }
            name
        }
    }
}

/// Port of modMMudDatabase `GetShopRoomNames`: the shop's name with its number, or the names of
/// the rooms it is assigned to. An unknown shop gives its number as text.
#[must_use]
pub fn get_shop_room_names(world: &World, num: i32, limit: i32, no_number: bool) -> String {
    if num == 0 {
        return "None".into();
    }
    let number_text = num.to_string();
    if world.shops.is_empty() {
        return number_text;
    }
    let Some(shop) = world.get_shop(num) else {
        return number_text;
    };
    let named = |shop_name: &str| {
        if no_number {
            shop_name.to_string()
        } else {
            format!("{shop_name}({num})")
        }
    };
    let assigned = trim(shop.assigned_to.as_deref().unwrap_or("")).to_string();
    if assigned.is_empty() || instr_ci(&assigned, "Room") == 0 {
        return named(&shop.name);
    }
    let mut result = String::new();
    let mut count = 0;
    for loc in assigned.split(',') {
        if instr_ci(loc, "Room") > 0 {
            let mr = extract_map_room(loc);
            if mr.map > 0 && mr.room > 0 {
                result = auto_append(&result, &get_room_name(world, mr.map, mr.room, no_number));
                count += 1;
            }
        }
        if limit > 0 && count >= limit {
            break;
        }
    }
    if result.is_empty() {
        named(&shop.name)
    } else {
        result
    }
}

/// What `EvaluateBestPriceForHit` leaves in its outputs; `sort_value` is its result.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BestPrice {
    /// The sell value in copper of the chosen shop (0 when there is none); the sort key.
    pub sort_value: f64,
    pub shop_name: String,
    pub value_cell: String,
    /// Shops beyond the chosen one.
    pub more_shops: i32,
    pub sell_only: bool,
    /// The chosen shop's number, 0 for none.
    pub chosen_shop: i32,
}

fn item_value(
    env: &MatchEnv,
    item_number: i32,
    charm: i16,
    shop_number: i32,
    no_buy: bool,
) -> R<ItemValue> {
    get_item_value(
        Profile::Stock,
        item_number,
        env.world.get_item(item_number),
        charm,
        0,
        shop_number,
        env.world.get_shop(shop_number),
        no_buy,
    )
}

/// Port of modItemParse `EvaluateBestPriceForHit`: prefers the cheapest buy price among the shops
/// that sell the item (lowest shop number on a tie), else the sell-only shop with the lowest
/// number.
///
/// # Errors
/// A VB6 error from the shop text or the price.
pub fn evaluate_best_price_for_hit(env: &MatchEnv, hit: &ItemMatch, charm: i16) -> R<BestPrice> {
    let shops = extract_shops_from_obtained_from(&hit.obtained_from)?;
    if shops.is_empty() {
        return Ok(BestPrice {
            shop_name: "none".into(),
            ..BestPrice::default()
        });
    }
    let mut out = BestPrice {
        more_shops: shops.len() as i32 - 1,
        ..BestPrice::default()
    };

    let (mut have_buy, mut have_sell) = (false, false);
    let (mut best_buy, mut best_buy_shop) = (-1.0_f64, 0);
    let mut best_sell_shop = 0;
    for s in &shops {
        let v = item_value(env, hit.number, charm, s.shop_number, s.no_buy)?;
        if !s.no_buy
            && v.copper_buy > 0.0
            && (!have_buy
                || v.copper_buy < best_buy
                || (v.copper_buy == best_buy && s.shop_number < best_buy_shop))
        {
            have_buy = true;
            best_buy = v.copper_buy;
            best_buy_shop = s.shop_number;
        }
        // the sell candidate is the lowest shop number, whatever it pays
        if v.copper_sell > 0.0 && (!have_sell || s.shop_number < best_sell_shop) {
            have_sell = true;
            best_sell_shop = s.shop_number;
        }
    }

    if have_buy {
        out.chosen_shop = best_buy_shop;
        out.shop_name = get_shop_room_names(env.world, best_buy_shop, 0, env.hide_record_numbers);
        let v = item_value(env, hit.number, charm, best_buy_shop, false)?;
        out.value_cell = format!("{} / {}", v.friendly_buy_short, v.friendly_sell_short);
        out.sell_only = false;
        out.sort_value = v.copper_sell;
        return Ok(out);
    }
    if have_sell {
        out.chosen_shop = best_sell_shop;
        out.shop_name = get_shop_room_names(env.world, best_sell_shop, 0, env.hide_record_numbers);
        let v = item_value(env, hit.number, charm, best_sell_shop, true)?;
        out.value_cell = format!("(sell) {}", v.friendly_sell_short);
        out.sell_only = true;
        out.sort_value = v.copper_sell;
        return Ok(out);
    }
    out.shop_name = "none".into();
    Ok(out)
}

/// What `GetBestShopNumForItem` returns and leaves in its outputs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BestShop {
    /// The chosen shop number, 0 for none.
    pub shop: i32,
    pub sell_only: bool,
    pub sort_sell_copper: f64,
    pub shop_name: String,
    pub value_cell: String,
    /// The error MME's handler caught, if any (the outputs stay at their defaults).
    pub handled: Option<Event>,
}

/// The `ItemMatch` `GetBestShopNumForItem` and `LV_AddRowByItemNumber` build from a record seek.
fn item_by_number(world: &World, number: i32) -> Option<ItemMatch> {
    if number == 0 || world.items.is_empty() {
        return None;
    }
    world.get_item(number).map(nz_item_match)
}

/// Port of modItemParse `GetBestShopNumForItem`: the shop `EvaluateBestPriceForHit` picks for the
/// item with this record number. An unknown item gives shop 0 and empty outputs.
#[must_use]
pub fn get_best_shop_num_for_item(env: &MatchEnv, item_number: i32, charm: i16) -> BestShop {
    let Some(hit) = item_by_number(env.world, item_number) else {
        return BestShop::default();
    };
    match evaluate_best_price_for_hit(env, &hit, charm) {
        Ok(b) => BestShop {
            shop: b.chosen_shop,
            sell_only: b.sell_only,
            sort_sell_copper: b.sort_value,
            shop_name: b.shop_name,
            value_cell: b.value_cell,
            handled: None,
        },
        Err(e) => BestShop {
            handled: Some(Event::HandledError {
                source: "GetBestShopNumForItem",
                number: err_number(&e),
            }),
            ..BestShop::default()
        },
    }
}

fn ucase(s: &str) -> String {
    s.chars()
        .map(|c| {
            let mut it = c.to_uppercase();
            match (it.next(), it.next()) {
                (Some(u), None) => u,
                _ => c,
            }
        })
        .collect()
}

/// Port of modListViewExt `ParseActionAndQty`: splits a flag such as `CARRIED x3` or `STASHx2` into
/// the upper-cased action and the quantity; `field_qty` (at least 1) is the quantity when the
/// flag has none.
///
/// # Errors
/// Overflow (6) when the quantity does not fit a Long.
pub fn parse_action_and_qty(s_in: &str, field_qty: i32) -> R<(String, i32)> {
    let s = trim(s_in).to_string();
    let mut action = ucase(&s);
    let mut qty = field_qty.max(1);
    if s.is_empty() {
        return Ok((action, qty));
    }
    // 1) the " ... x#" form
    let pos = instrrev(&s, " ", None);
    if pos > 0 {
        let tail = mid_from(&s, pos + 1);
        if lcase(&left(&tail, 1)) == "x" && len(&tail) > 1 {
            let maybe = mid_from(&tail, 2);
            if is_numeric(&maybe) {
                qty = vb(clng(val(&maybe)))?.max(1);
                action = ucase(trim(&left(&s, pos - 1)));
                return Ok((action, qty));
            }
        }
    }
    // 2) the "...x#" form
    let chars: Vec<char> = s.chars().collect();
    let mut start_digits = 0;
    for j in (1..=chars.len()).rev() {
        let ch = chars[j - 1];
        if ch.is_ascii_digit() {
            start_digits = j;
        } else if (ch == 'x' || ch == 'X') && start_digits > 0 && j == start_digits - 1 {
            let maybe = mid_from(&s, j + 1);
            if is_numeric(&maybe) {
                qty = vb(clng(val(&maybe)))?.max(1);
                action = ucase(trim(&left(&s, j - 1)));
                return Ok((action, qty));
            }
            break;
        } else {
            break;
        }
    }
    Ok((action, qty))
}

fn item_type_text(code: i32) -> R<String> {
    let c = vb(cint(f64::from(code)))?;
    Ok(touchstone_model::ItemType::from_code(i32::from(c))
        .map_or_else(|| c.to_string(), |t| t.label().to_string()))
}

fn weapon_type_text(code: i32) -> R<String> {
    let c = i32::from(vb(cint(f64::from(code)))?);
    Ok(touchstone_model::WeaponType::from_code(c)
        .map_or_else(|| format!("Unknown ({c})"), |t| t.label().to_string()))
}

fn worn_text(code: i32) -> R<String> {
    let c = i32::from(vb(cint(f64::from(code)))?);
    Ok(touchstone_model::WornSlot::from_code(c)
        .map_or_else(|| format!("Unknown ({c})"), |t| t.label().to_string()))
}

/// The columns of an Item Manager row `AddOneRow` is given beyond the item itself.
#[derive(Debug, Clone, Copy)]
pub struct RowParts<'a> {
    pub section: &'a str,
    pub encum: i32,
    pub qty: i32,
    pub shop_cell: &'a str,
    pub value_cell: &'a str,
    pub sort_copper: f64,
    pub is_key: bool,
    pub flag: &'a str,
}

/// Port of modItemParse `AddOneRow`: appends one row. On an error MME has already added the row
/// and filled the columns before the failing one, and the port leaves it the same way.
///
/// # Errors
/// Overflow (6) from an Integer conversion or the quantity in the flag.
pub fn add_one_row(
    env: &MatchEnv,
    rows: &mut Vec<ListRow>,
    hit: &ItemMatch,
    p: &RowParts,
) -> R<()> {
    let (worn, worn_tag) = if p.is_key {
        ("Key".to_string(), 0)
    } else {
        match hit.item_type {
            0 => (
                worn_text(hit.worn)?,
                i32::from(vb(cint(f64::from(hit.worn)))?),
            ),
            1 => (
                weapon_type_text(hit.weapon_type)?,
                i32::from(vb(cint(f64::from(hit.weapon_type)))?),
            ),
            _ => ("Nowhere".to_string(), 0),
        }
    };
    let usable = if item_is_usable_by_char(
        Profile::Stock,
        env.world,
        env.who,
        env.equipped,
        hit.number,
        true,
        false,
    ) {
        "Yes"
    } else {
        "No"
    };
    let (flag_base, tmp_qty) = parse_action_and_qty(p.flag, p.qty)?;

    rows.push(ListRow {
        text: hit.number.to_string(),
        subs: Vec::new(),
    });
    let row = rows.last_mut().expect("just pushed");
    row.subs.push(SubItem::new(&hit.name));
    // a blank flag with QTY > 1 must not render a headless " xN"
    let flag_cell = if flag_base.is_empty() {
        String::new()
    } else if tmp_qty > 1 {
        format!("{flag_base} x{tmp_qty}")
    } else {
        flag_base
    };
    row.subs.push(SubItem::new(&flag_cell));
    row.subs.push(SubItem::new(&p.qty.to_string()));
    row.subs.push(SubItem::new(p.section));
    row.subs.push(SubItem::new(&p.encum.to_string()));
    row.subs.push(SubItem::new(&item_type_text(hit.item_type)?));
    row.subs.push(SubItem {
        text: worn,
        tag: Some(SubTag::Int(worn_tag)),
    });
    row.subs.push(SubItem::new(usable));
    let tag = if p.value_cell.is_empty() || p.sort_copper <= 0.0 {
        "0".to_string()
    } else {
        cstr(p.sort_copper)
    };
    row.subs.push(SubItem {
        text: p.value_cell.to_string(),
        tag: Some(SubTag::Text(tag)),
    });
    row.subs.push(SubItem::new(p.shop_cell));
    Ok(())
}

/// Port of modItemParse `AddListViewRowsForItem`: picks the shop and value for the item and adds
/// its row. An error is shown as a message box and adds nothing further.
pub fn add_listview_rows_for_item(
    env: &MatchEnv,
    rows: &mut Vec<ListRow>,
    hit: &ItemMatch,
    section: &str,
    qty: i32,
    is_key_section: bool,
    log: &mut Vec<Event>,
) {
    let run = |rows: &mut Vec<ListRow>| -> R<()> {
        let charm = env.charm()?;
        let best = evaluate_best_price_for_hit(env, hit, charm)?;
        let mut shop_name = best.shop_name.clone();
        if !shop_name.is_empty() && best.more_shops > 0 {
            shop_name = format!("{shop_name} +{} more", best.more_shops);
        }
        let sort_tag = if best.chosen_shop > 0 {
            item_value(env, hit.number, charm, best.chosen_shop, best.sell_only)?
                .copper_sell
                .max(0.0)
        } else {
            0.0
        };
        add_one_row(
            env,
            rows,
            hit,
            &RowParts {
                section,
                encum: hit.encum,
                qty,
                shop_cell: &shop_name,
                value_cell: &best.value_cell,
                sort_copper: sort_tag,
                is_key: is_key_section,
                flag: "",
            },
        )
    };
    if let Err(e) = run(rows) {
        log.push(Event::MsgBox(format!(
            "AddListViewRowsForItem error: {}",
            err_description(&e)
        )));
    }
}

/// `SafeSubText`: the text of sub item `idx` (from 1), empty when the row has fewer.
fn safe_sub_text(row: &ListRow, idx: usize) -> &str {
    row.subs.get(idx - 1).map_or("", |s| s.text.as_str())
}

/// Port of modItemParse `SumInvOrCarriedQtyForName`: how many of the item the list already holds,
/// from INVENTORY rows (the QTY column) and CARRIED flags.
///
/// # Errors
/// Overflow (6) from a quantity or the sum.
pub fn sum_inv_or_carried_qty_for_name(rows: &[ListRow], item_name: &str) -> R<i32> {
    let mut sum = 0;
    let want = lcase(item_name);
    for row in rows {
        if lcase(safe_sub_text(row, 1)) != want {
            continue;
        }
        let src = ucase(safe_sub_text(row, 4));
        if left(&src, 9) == "INVENTORY" {
            // one physical stack per row: the QTY column; a CARRIED flag on the row is not added again
            let q = vb(clng(val(safe_sub_text(row, 3))))?;
            if q > 0 {
                sum = vb(add_i32(sum, q))?;
            }
        } else {
            let src = ucase(safe_sub_text(row, 2));
            if left(&src, 7) == "CARRIED" {
                let q = if lcase(&mid(&src, 8, 2)) == " x" {
                    vb(clng(val(&mid(&src, 10, 999))))?
                } else {
                    1
                };
                if q > 0 {
                    sum = vb(add_i32(sum, q))?;
                }
            }
        }
    }
    Ok(sum)
}

/// Which list of a paste `AddSectionItems` is adding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Equipped,
    Keys,
    Ground,
    Inventory,
}

fn add_section_items_inner(
    env: &MatchEnv,
    rows: &mut Vec<ListRow>,
    items: &[String],
    section_name: &str,
    kind: Section,
    log: &mut Vec<Event>,
) -> R<()> {
    if items.is_empty() {
        return Ok(());
    }
    let charm = env.charm()?;
    let mut best_h = 0_usize;
    for raw in items {
        let (base_name, qty) = if kind == Section::Equipped {
            (normalize_equipped_name(raw), 1)
        } else {
            let (n, q) = vb(parse_name_and_qty(raw))?;
            if n.is_empty() {
                continue;
            }
            (n, q)
        };
        let hits = get_items_by_exact_name(env, &base_name);
        if hits.is_empty() {
            continue;
        }
        // the single best row for this name, skipping what cannot be got. MME's `Or` does not
        // short-circuit, so `hits(bestH)` is read even when nothing is best yet; `bestH` keeps its
        // value from the previous name, which can be past the end of this name's hits (error 9).
        let mut best: Option<(usize, f64)> = None;
        for (h, hit) in hits.iter().enumerate() {
            if hit.gettable == 0 {
                continue;
            }
            let sort_val = evaluate_best_price_for_hit(env, hit, charm)?.sort_value;
            if sort_val >= 0.0 {
                let Some(stale) = hits.get(best_h) else {
                    return Err(RulesError::Vb6(9));
                };
                let better = match best {
                    None => true,
                    Some((_, bs)) => sort_val < bs || (sort_val == bs && hit.number < stale.number),
                };
                if better {
                    best = Some((h, sort_val));
                    best_h = h;
                }
            }
        }
        let Some((bh, _)) = best else { continue };
        let hit = &hits[bh];

        let is_key = kind == Section::Keys;
        if kind == Section::Inventory {
            let have = sum_inv_or_carried_qty_for_name(rows, &hit.name)?;
            if have >= qty {
                continue;
            }
            if have > 0 {
                add_listview_rows_for_item(env, rows, hit, section_name, qty - have, is_key, log);
                continue;
            }
        }
        add_listview_rows_for_item(env, rows, hit, section_name, qty, is_key, log);
    }
    Ok(())
}

/// Port of modItemParse `AddSectionItems`: adds a row for each name in `items` that matches a
/// gettable Items row. An error is shown as a message box and stops the section.
pub fn add_section_items(
    env: &MatchEnv,
    rows: &mut Vec<ListRow>,
    items: &[String],
    section_name: &str,
    kind: Section,
    log: &mut Vec<Event>,
) {
    if let Err(e) = add_section_items_inner(env, rows, items, section_name, kind, log) {
        log.push(Event::MsgBox(format!(
            "AddSectionItems error: {}",
            err_description(&e)
        )));
    }
}

/// The answers to the three questions `PopulateItemManagerFromParsed` can ask, in the order it
/// asks them. `prompt` is the question text; `default_yes` is MME's default button.
pub type Ask<'a> = &'a mut dyn FnMut(&str, bool) -> bool;

/// Port of modItemParse `PopulateItemManagerFromParsed`: asks whether to import equipped items and
/// keys (when the paste has any) and whether to clear the non-flagged rows first, then adds the
/// sections in the order equipped, keys, ground, inventory.
pub fn populate_item_manager_from_parsed(
    env: &MatchEnv,
    rows: &mut Vec<ListRow>,
    items: &ItemParseResult,
    extra_inventory_only: bool,
    ask: Ask,
    log: &mut Vec<Event>,
) {
    let (do_equipped, do_keys, have_equipped, have_keys, have_inv, have_ground);
    if extra_inventory_only {
        have_inv = safe_array_has_data(&items.inventory);
        (do_equipped, do_keys, have_equipped, have_keys, have_ground) =
            (false, false, false, false, false);
    } else {
        have_equipped = safe_array_has_data(&items.equipped);
        have_keys = safe_array_has_data(&items.keys);
        have_inv = safe_array_has_data(&items.inventory);
        have_ground = safe_array_has_data(&items.ground);
        let mut de = true;
        let mut dk = true;
        if have_equipped {
            let q = "Import EQUIPPED items into Item Manager?";
            log.push(Event::MsgBox(q.into()));
            if !ask(q, true) {
                de = false;
            }
        }
        if have_keys {
            let q = "Import KEYS (from inventory) into Item Manager?";
            log.push(Event::MsgBox(q.into()));
            if !ask(q, true) {
                dk = false;
            }
        }
        (do_equipped, do_keys) = (de, dk);
    }

    let mut total = 0;
    if do_equipped {
        total += array_count(&items.equipped);
    }
    if do_keys {
        total += array_count(&items.keys);
    }
    if have_ground {
        total += array_count(&items.ground);
    }
    if have_inv {
        total += array_count(&items.inventory);
    }

    if total > 0 && !rows.is_empty() && !extra_inventory_only {
        let q = "Clear the Item Manager of NON-FLAGGED items first?";
        log.push(Event::MsgBox(q.into()));
        if ask(q, false) {
            // keep only the rows that carry a flag
            let mut x = rows.len();
            while x >= 1 {
                let row = &rows[x - 1];
                if row.subs.len() >= 2 {
                    if trim(&row.subs[1].text).is_empty() {
                        rows.remove(x - 1);
                    }
                } else {
                    rows.remove(x - 1);
                }
                x -= 1;
            }
        }
    }

    if do_equipped && have_equipped {
        add_section_items(
            env,
            rows,
            &items.equipped,
            "Equipped",
            Section::Equipped,
            log,
        );
    }
    if do_keys && have_keys {
        add_section_items(env, rows, &items.keys, "Inventory", Section::Keys, log);
    }
    if have_ground {
        add_section_items(env, rows, &items.ground, "Ground", Section::Ground, log);
    }
    if have_inv {
        add_section_items(
            env,
            rows,
            &items.inventory,
            "Inventory",
            Section::Inventory,
            log,
        );
    }
}

/// Port of modItemParse `LV_AddRowByItemNumber`: adds a row for one item by record number,
/// optionally showing a given shop. Items that cannot be got are skipped. `source` is the Source
/// column; `qty` is clamped to 9999 (a negative one becomes 1). An error is reported through
/// `HandleError` as a [`Event::HandledError`].
pub fn lv_add_row_by_item_number(
    env: &MatchEnv,
    rows: &mut Vec<ListRow>,
    item_number: i32,
    source: &str,
    flag: &str,
    qty: i32,
    force_shop: i32,
    log: &mut Vec<Event>,
) {
    let Some(hit) = item_by_number(env.world, item_number) else {
        return;
    };
    let qty = if qty < 0 { 1 } else { qty.min(9999) };
    if hit.gettable == 0 {
        return;
    }
    let mut run = |rows: &mut Vec<ListRow>| -> R<()> {
        let charm = env.charm()?;
        let best = get_best_shop_num_for_item(env, item_number, charm);
        if let Some(Event::HandledError { number, .. }) = best.handled {
            // the inner handler already reported it; the outputs are at their defaults
            log.push(Event::HandledError {
                source: "GetBestShopNumForItem",
                number,
            });
        }
        let mut chosen = best.shop;
        let mut shop_name = best.shop_name.clone();
        if force_shop > 0 && chosen != force_shop {
            chosen = force_shop;
            shop_name = get_shop_room_names(env.world, force_shop, 0, env.hide_record_numbers);
        }
        let sort_tag = if chosen > 0 {
            item_value(env, hit.number, charm, chosen, best.sell_only)?
                .copper_sell
                .max(0.0)
        } else {
            0.0
        };
        add_one_row(
            env,
            rows,
            &hit,
            &RowParts {
                section: source,
                encum: hit.encum,
                qty,
                shop_cell: &shop_name,
                value_cell: &best.value_cell,
                sort_copper: sort_tag,
                is_key: hit.item_type == 7,
                flag,
            },
        )
    };
    if let Err(e) = run(rows) {
        log.push(Event::HandledError {
            source: "LV_AddRowByItemNumber",
            number: err_number(&e),
        });
    }
}
