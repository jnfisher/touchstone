//! Shop detail and the NPC command references of the map list (modMain `PullShopDetail`,
//! `AddRoomNPCCommandRefs`).

use touchstone_model::World;
use touchstone_vb6::{Currency, Vb6Error, clng, cstr, fix, format, len, val};

use super::{DetailKind, DetailRow};
use crate::Profile;
use crate::ability_text::{get_class_name, get_item_name};
use crate::exp::calc_money_required_to_train;
use crate::item_value::{ItemValue, get_item_value};
use crate::locations::{LocationArgs, LocationLairs, LocationList, LocationOptions, get_locations};
use crate::textblock::{get_textblock_link_to, get_textblock_teleport};

type Res<T> = Result<T, Vb6Error>;

fn cur(v: i64) -> Currency {
    Currency::from_scaled(v * 10_000)
}

/// `CStr(Currency)`.
fn cur_str(c: Currency) -> String {
    let s = c.scaled();
    let a = s.unsigned_abs();
    let sign = if s < 0 { "-" } else { "" };
    let (whole, frac) = (a / 10_000, a % 10_000);
    if frac == 0 {
        format!("{sign}{whole}")
    } else {
        format!(
            "{sign}{whole}.{}",
            format!("{frac:04}").trim_end_matches('0')
        )
    }
}

/// `Round(c, digits)` on a Currency (digits 0 or 2), half to even.
fn round_cur(c: Currency, digits: u32) -> Currency {
    let q = 10i64.pow(4 - digits);
    let s = c.scaled();
    let d = s.div_euclid(q);
    let r = s.rem_euclid(q);
    let d = match (r * 2).cmp(&q) {
        std::cmp::Ordering::Less => d,
        std::cmp::Ordering::Greater => d + 1,
        std::cmp::Ordering::Equal => d + (d & 1),
    };
    Currency::from_scaled(d * q)
}

/// What `PullShopDetail` reads besides the shop: its arguments and the frontend state behind
/// `GetLocations`.
#[derive(Debug, Clone)]
pub struct ShopDetailOptions {
    /// `bStartup`.
    pub startup: bool,
    /// `nCharm` (an Integer).
    pub charm: i16,
    /// `bShowSell`: show what the shop pays rather than what it charges.
    pub show_sell: bool,
    pub locations: LocationOptions,
}

/// A stocked item line (the five columns of the shop list).
#[derive(Debug, Clone, PartialEq)]
pub struct ShopStockRow {
    pub number: String,
    pub name: String,
    pub max: String,
    pub regen: String,
    pub cost: String,
    /// The cost cell's `Tag`: the copper value.
    pub copper: String,
}

/// A training line: the level and what training to it costs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainingRow {
    pub level: String,
    pub cost: String,
    pub copper: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShopLines {
    None,
    /// "Shop not found" in the list's first row.
    NotFound,
    Stock(Vec<ShopStockRow>),
    Training(Vec<TrainingRow>),
}

/// What `PullShopDetail` leaves in its controls.
#[derive(Debug, Clone)]
pub struct ShopDetail {
    pub lines: ShopLines,
    /// `DetailTB.Text`.
    pub text: String,
    /// `frmMain.lblCharmMod.Caption`.
    pub charm_label: String,
    /// The "Assigned To" list.
    pub assigned: LocationList,
}

impl ShopDetail {
    /// The same content as (label, value, kind) rows.
    #[must_use]
    pub fn detail_rows(&self) -> Vec<DetailRow> {
        let mut out = Vec::new();
        match &self.lines {
            ShopLines::None => {}
            ShopLines::NotFound => {
                out.push(DetailRow::new("", "Shop not found", DetailKind::Message))
            }
            ShopLines::Stock(rows) => {
                for r in rows {
                    out.push(DetailRow::new(&r.name, &r.cost, DetailKind::ShopLine));
                }
            }
            ShopLines::Training(rows) => {
                for r in rows {
                    out.push(DetailRow::new(
                        &format!("Level {}", r.level),
                        &r.cost,
                        DetailKind::ShopLine,
                    ));
                }
            }
        }
        if !self.charm_label.is_empty() {
            out.push(DetailRow::new("Charm", &self.charm_label, DetailKind::Cost));
        }
        if !self.text.is_empty() {
            out.push(DetailRow::new("Shop", &self.text, DetailKind::Fact));
        }
        out
    }
}

fn charm_label(charm: i16, show_sell: bool, shop_type: i32) -> Res<String> {
    if !(charm > 0 || show_sell) {
        return Ok(String::new());
    }
    if show_sell && shop_type != 8 {
        let n = fix(f64::from(charm) / 2.0) + 25.0;
        return Ok(format!("{}% cost pre-markup", cstr(n)));
    }
    let m = 1.0 - ((fix(f64::from(charm) / 5.0) - 10.0) / 100.0);
    Ok(if m > 1.0 {
        let d = cur(1).checked_sub(Currency::from_f64(m)?)?;
        let d = if d.scaled() < 0 {
            Currency::from_scaled(-d.scaled())
        } else {
            d
        };
        format!("{}% Markup", cur_str(d.checked_mul(cur(100))?))
    } else if m < 1.0 {
        let d = cur(1).checked_sub(Currency::from_f64(m)?)?;
        format!("{}% Discount", cstr(val(&cur_str(d)) * 100.0))
    } else {
        "Retail Value".to_string()
    })
}

fn regen_text(time: i32, pct: i32, amount: i32) -> Res<String> {
    if !(time > 0 && pct > 0 && amount > 0) {
        return Ok("no regen".to_string());
    }
    let mut t = i16::try_from(time).map_err(|_| Vb6Error::Overflow)?;
    let mut s = String::new();
    if t >= 60 * 24 {
        let d = fix(f64::from(t) / 1440.0);
        s.push_str(&format!("{}d", cstr(d)));
        t -= (d as i16) * 1440;
    }
    if t >= 60 {
        let h = fix(f64::from(t) / 60.0);
        s.push_str(&format!("{}h", cstr(h)));
        t -= (h as i16) * 60;
    }
    if t > 0 {
        s.push_str(&format!("{t}m"));
    }
    Ok(format!("{pct}% for {amount} per {s}"))
}

fn training_rows(lo: i32, hi: i32, markup: i32) -> Res<Vec<TrainingRow>> {
    let mut rows = Vec::new();
    for x in lo..=hi {
        let level = i16::try_from(x).map_err(|_| Vb6Error::Overflow)?;
        let m = cur(i64::from(markup));
        let mut copper = calc_money_required_to_train(cur(i64::from(level) - 1), m)
            .unwrap_or(Currency::from_scaled(0));
        if copper.scaled() < 0 {
            copper = Currency::from_scaled(0);
        }
        copper = round_cur(copper, 0);
        let (mut reduced, mut coin) = (Currency::from_scaled(0), "Copper");
        let n = copper.scaled() / 10_000;
        if n >= 10_000_000 {
            reduced = copper.checked_div(cur(1_000_000))?;
            coin = "Runic";
        } else if n >= 100_000 {
            reduced = copper.checked_div(cur(10_000))?;
            coin = "Platinum";
        } else if n >= 1000 {
            reduced = copper.checked_div(cur(100))?;
            coin = "Gold";
        } else if n >= 100 {
            reduced = copper.checked_div(cur(10))?;
            coin = "Silver";
        }
        if reduced.scaled() > 0 {
            reduced = round_cur(reduced, 2);
        }
        let strip = |s: String| s.strip_suffix(".00").map_or(s.clone(), str::to_string);
        let s_copper = strip(format(copper.to_f64(), "##,##0.00")?);
        let cost = if reduced.scaled() == 0 {
            if copper.scaled() <= 0 {
                "Free".to_string()
            } else {
                format!("{s_copper} Copper")
            }
        } else {
            let s = strip(format(reduced.to_f64(), "##,##0.00")?);
            format!("{} copper ({s} {coin})", format(copper.to_f64(), "#,#")?)
        };
        rows.push(TrainingRow {
            level: x.to_string(),
            cost,
            copper: cur_str(copper),
        });
    }
    Ok(rows)
}

/// Port of modMain `PullShopDetail`: the shop's stock list (or training cost table), its summary
/// line, the charm label and its "Assigned To" references. A shop that does not exist gives the
/// "Shop not found" text.
///
/// The column headers and the charm check boxes' `Enabled` flags are display and are not ported.
#[must_use]
pub fn pull_shop_detail(
    profile: Profile,
    world: &World,
    lairs: &mut dyn LocationLairs,
    number: i32,
    opts: &ShopDetailOptions,
) -> ShopDetail {
    let mut out = ShopDetail {
        lines: ShopLines::None,
        text: String::new(),
        charm_label: String::new(),
        assigned: LocationList::default(),
    };
    if opts.startup {
        return out;
    }
    let Some(shop) = world.get_shop(number) else {
        out.text = "Shop not found".to_string();
        out.lines = ShopLines::NotFound;
        return out;
    };
    // MME computes the label first; an error there ends the procedure with the controls untouched.
    let Ok(label) = charm_label(opts.charm, opts.show_sell, shop.shop_type) else {
        return out;
    };
    if shop.shop_type == 8 {
        out.charm_label = String::new();
        let Ok(rows) = training_rows(shop.min_lvl, shop.max_lvl, shop.markup_pct) else {
            return out;
        };
        out.lines = ShopLines::Training(rows);
    } else {
        out.charm_label = label;
        let mut rows = Vec::new();
        for s in shop.stock.iter().filter(|s| s.item != 0) {
            let value = get_item_value(
                profile,
                s.item,
                world.get_item(s.item),
                opts.charm,
                i16::try_from(shop.markup_pct).unwrap_or(0),
                0,
                None,
                false,
            )
            .unwrap_or_else(|_| ItemValue::default());
            let (cost, copper) = if opts.show_sell {
                (value.friendly_sell.clone(), value.copper_sell)
            } else {
                (value.friendly_buy.clone(), value.copper_buy)
            };
            rows.push(ShopStockRow {
                number: s.item.to_string(),
                name: get_item_name(world, s.item, true),
                max: s.max.to_string(),
                regen: regen_text(s.time, s.pct, s.amount).unwrap_or_default(),
                cost,
                copper: cstr(copper),
            });
        }
        out.lines = ShopLines::Stock(rows);
    }
    let mut text = format!(
        "Levels: {} to {} -- Markup: {}%",
        shop.min_lvl, shop.max_lvl, shop.markup_pct
    );
    if shop.class_rest != 0 {
        text.push_str(&format!(
            " -- Class: {}",
            get_class_name(world, shop.class_rest)
        ));
    }
    out.text = text;
    get_locations(
        world,
        lairs,
        &opts.locations,
        &LocationArgs {
            loc: shop.assigned_to.as_deref().unwrap_or(""),
            ..LocationArgs::default()
        },
        &mut out.assigned,
    );
    out
}

// ---- NPC command references -----------------------------------------------------------------------

/// What a row added by [`add_room_npc_command_refs`] opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcRefKind {
    /// "Teleport: (NPC) command --> room": navigates the map.
    Teleport,
    /// "Greet: a, b, c": opens the monster's greet command tree.
    Greet,
}

/// A row of the map reference list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcRef {
    pub kind: NpcRefKind,
    pub text: String,
    /// `"map/room"` for a teleport, the greet textblock number for the greet row.
    pub tag: String,
}

fn instr(start: i32, hay: &[char], needle: char) -> i32 {
    if start < 1 || start as usize > hay.len() {
        return 0;
    }
    hay[start as usize - 1..]
        .iter()
        .position(|&c| c == needle)
        .map_or(0, |i| i as i32 + start)
}

fn mid_c(s: &[char], start: i32, length: i32) -> String {
    if start < 1 || length < 0 {
        return String::new();
    }
    s.iter()
        .skip(start as usize - 1)
        .take(length as usize)
        .collect()
}

fn room_name(world: &World, map: i32, room: i32) -> String {
    if map == 0 || room == 0 {
        return "?".to_string();
    }
    match world.get_room(map, room) {
        None => format!("{map}/{room}"),
        Some(r) => {
            let n = r.name.clone().unwrap_or_default();
            let n = if n.is_empty() {
                "(no name)".to_string()
            } else {
                n
            };
            format!("{n} ({map}/{room})")
        }
    }
}

/// Port of modMain `AddRoomNPCCommandRefs`: the rows the room's NPC adds to the map list. A greet
/// command that leads to a teleport (directly or one `LinkTo` deeper) gets its own row; the others
/// share one "Greet:" row. Returns the added rows (MME appends them to the list it is given).
#[must_use]
pub fn add_room_npc_command_refs(world: &World, npc: i32, current_map: i32) -> Vec<NpcRef> {
    let mut rows = Vec::new();
    let _ = npc_inner(world, npc, current_map, &mut rows);
    rows
}

#[allow(clippy::too_many_lines)]
fn npc_inner(world: &World, npc: i32, current_map: i32, rows: &mut Vec<NpcRef>) -> Res<()> {
    if npc <= 0 {
        return Ok(());
    }
    let Some(mon) = world.get_monster(npc) else {
        return Ok(());
    };
    let greet_tb = mon.greet_txt;
    if greet_tb <= 0 {
        return Ok(());
    }
    let Some(tb) = world.get_tbinfo(greet_tb) else {
        return Ok(());
    };
    let action = tb.action.clone().unwrap_or_default();
    let lower = action.to_ascii_lowercase();
    // The same filter PullMonsterDetail uses for greets that are only placeholders.
    if ["heh:", "nothing:", "yada:", "hehe:", "shit:"]
        .iter()
        .any(|p| lower.starts_with(p))
    {
        return Ok(());
    }
    let data: Vec<char> = action.chars().collect();
    let total = len(&action);
    let first = instr(1, &data, ':');
    if first == 0 {
        return Ok(());
    }
    let mut command = mid_c(&data, 1, first - 1);
    let mut pos = first + 1;
    let mut keys = String::new();
    let mut greets = String::new();
    while pos < total {
        command = command.replace('*', "").replace('|', " OR ");
        let mut x2 = instr(pos, &data, '\n');
        if x2 == 0 {
            x2 = total + 1;
        }
        let line = mid_c(&data, pos, x2 - pos);
        if !line.is_empty() && val(&line) >= 1.0 && !command.is_empty() {
            let sub = clng(val(&line))?;
            let mut tele = get_textblock_teleport(world, sub);
            if tele.is_none() {
                let link = get_textblock_link_to(world, sub);
                if link > 0 {
                    tele = get_textblock_teleport(world, link);
                }
            }
            match tele {
                Some((room, map)) if room > 0 => {
                    let map = if map == 0 { current_map } else { map };
                    let key = format!("<{map}/{room}>");
                    if !keys.contains(&key) {
                        keys.push_str(&key);
                        rows.push(NpcRef {
                            kind: NpcRefKind::Teleport,
                            text: format!(
                                "Teleport: (NPC) {command} --> {}",
                                room_name(world, map, room)
                            ),
                            tag: format!("{map}/{room}"),
                        });
                    }
                }
                _ => {
                    if greets.is_empty() {
                        greets = command.clone();
                    } else {
                        greets.push_str(&format!(", {command}"));
                    }
                }
            }
        }
        // next_line: skip to the next command; an empty command name repeats the skip
        loop {
            pos = instr(pos, &data, '\n') + 1;
            if pos == 1 {
                break;
            }
            let mut c = instr(pos, &data, ':');
            if c == 0 {
                c = total + 1;
            }
            if c == pos {
                continue;
            }
            command = mid_c(&data, pos, c - pos);
            pos = c + 1;
            break;
        }
        if pos == 1 {
            break;
        }
    }
    if !greets.is_empty() {
        rows.push(NpcRef {
            kind: NpcRefKind::Greet,
            text: format!("Greet: {greets}"),
            tag: greet_tb.to_string(),
        });
    }
    Ok(())
}
