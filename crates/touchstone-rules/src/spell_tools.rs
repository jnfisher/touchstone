//! The Spells explorer's two lookups: "Find counters (negates/dispells)" and "What casts this spell?".
//! Card P4-32.
//!
//! MME fills its Results window; the port returns the rows. [`negate_query`] ports the array building
//! that the spells list menu and the monster/aux pane menu do before they call `FindNegates`;
//! [`find_negates`] ports `FindNegates` itself (the progress bar and window are UI and skipped).
//! [`what_casts`] ports `LookUpSpellCast` over [`crate::locations::get_locations`].

use touchstone_model::{Item, Spell};
use touchstone_vb6 as vb;

use crate::locations::{
    LocationArgs, LocationData, LocationLairs, LocationList, LocationOptions, get_locations,
};
use crate::spells::{MinMaxRequest, get_current_spell_min_max};

/// The three values `FindNegates(nSpells(), nAbils(), nLevel)` takes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NegateQuery {
    /// `nSpells()`: the spell itself, then the spells it ends with `endcast`.
    pub spells: Vec<i32>,
    /// `nAbils()`: a leading 0, then every ability the spell has.
    pub abilities: Vec<i32>,
    /// `nLevel`, 0 when the spell is not cast at a level.
    pub level: i32,
}

/// What [`negate_query`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegatePlan {
    pub query: NegateQuery,
    /// The spells list menu asks "Cast spell at a certain level?" here (the spell's damage is 0 to 0
    /// but it grows with the caster's level) and the caller did not give an answer.
    pub ask_level: bool,
}

/// Where the spell comes from.
#[derive(Debug, Clone, Copy)]
pub enum NegateFrom<'a> {
    /// The Spells/Spell Book/Compare list menu (`mnuSpellsPopUpItem_Click`): the selected row's
    /// number and, once the level prompt was answered, the typed text's `Val` (`None` before the
    /// prompt, 0 for cancel).
    List { spell: i32, answer: Option<f64> },
    /// The monster/aux pane menu (`mnuAuxPopUpItem_Click`): the row's tag, `number` or `number@level`.
    Pane { tag: &'a str },
}

fn find(spells: &[Spell], n: i32) -> Option<&Spell> {
    spells.iter().find(|s| s.number == n)
}

/// `Val(tag)` for a spell row's tag, in the pane menu.
fn pane_tag(tag: &str) -> (f64, f64) {
    let chars: Vec<char> = tag.chars().collect();
    let at = chars.iter().position(|&c| c == '@').map_or(0, |p| p + 1);
    if at > 1 {
        let spell: String = chars[..at - 1].iter().collect();
        let level: String = chars[at..].iter().collect();
        (vb::val(&spell), vb::val(&level))
    } else {
        (vb::val(tag), 0.0)
    }
}

/// Ports the code in `mnuSpellsPopUpItem_Click` (case "find negate") and `mnuAuxPopUpItem_Click` that builds
/// `nSpells`, `nAbils` and `nLevel`. A spell that is not in the table leaves them as the row's own number
/// alone (MME's handler resumes at the call). `avg_level_max_all_stats` is `gAvgLevelMaxAllStats`.
#[must_use]
pub fn negate_query(
    spells: &[Spell],
    from: NegateFrom<'_>,
    avg_level_max_all_stats: f64,
) -> NegatePlan {
    let mut q = NegateQuery {
        spells: vec![0],
        abilities: vec![0],
        level: 0,
    };
    let mut ask_level = false;
    let (number, mut level, mut answer) = match from {
        NegateFrom::List { spell, answer } => (f64::from(spell), 0, answer),
        NegateFrom::Pane { tag } => {
            let (n, l) = pane_tag(tag);
            // `nLevel As Integer`: a level that does not fit is the error the outer handler gets
            let l = vb::cint(l).map_or(0, i32::from);
            (n, l, None)
        }
    };
    // `nSpells(0) = Val(...)` into a Long
    q.spells[0] = vb::clng(number).unwrap_or(0);
    let is_list = matches!(from, NegateFrom::List { .. });
    let Some(spell) = find(spells, q.spells[0]) else {
        q.level = level;
        return NegatePlan {
            query: q,
            ask_level,
        };
    };
    let range = |level: i32| {
        let req = MinMaxRequest {
            use_level: level > 0,
            level: i16::try_from(level).unwrap_or(0),
            no_header: true,
            ..MinMaxRequest::default()
        };
        get_current_spell_min_max(Some(spell), avg_level_max_all_stats, &req)
    };
    let mut mm = range(level);
    if is_list
        && mm.n_min.scaled() == 0
        && mm.n_max.scaled() == 0
        && (spell.min_inc != 0 || spell.max_inc != 0)
    {
        match answer.take() {
            None => ask_level = true,
            Some(v) if v > 0.0 => {
                // `nLevel = Val(sTemp)` into an Integer: an overflow ends the block at the error handler
                if let Ok(l) = vb::cint(v) {
                    level = i32::from(l).min(9999);
                    mm = range(level);
                }
            }
            Some(_) => {}
        }
    }
    for slot in spell.abilities.iter().take(10) {
        if slot.ability == 0 {
            continue;
        }
        q.abilities.push(slot.ability);
        if slot.ability == 151 {
            // endcast
            if slot.value > 0 {
                q.spells.push(slot.value);
            } else if let (Some(lo), Some(hi)) = (cur_int(mm.n_min), cur_int(mm.n_max)) {
                // `For y = nMin To nMax` with an Integer counter
                for y in lo..=hi {
                    q.spells.push(y);
                }
            }
        }
    }
    q.level = level;
    NegatePlan {
        query: q,
        ask_level,
    }
}

/// A Currency as an Integer `For` bound; `None` where VB6 raises Overflow.
fn cur_int(c: vb::Currency) -> Option<i32> {
    let rounded = {
        let s = c.scaled();
        let d = s.div_euclid(10_000);
        let r = s.rem_euclid(10_000);
        match (r * 2).cmp(&10_000) {
            std::cmp::Ordering::Less => d,
            std::cmp::Ordering::Greater => d + 1,
            std::cmp::Ordering::Equal => d + (d & 1),
        }
    };
    i16::try_from(rounded).ok().map(i32::from)
}

/// What a result row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegateKind {
    Item,
    Spell,
}

/// One row of the counters list: `Item: name (number)` or `Spell: name (number)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegateRow {
    pub kind: NegateKind,
    pub number: i32,
    pub text: String,
}

fn hit_ability(
    ability: i32,
    value: i32,
    q: &NegateQuery,
    level_hit: impl Fn(i32) -> bool,
) -> bool {
    match ability {
        73 | 124 => value > 0 && q.abilities.contains(&value),
        122 | 153 => value > 0 && q.spells.contains(&value),
        139 => level_hit(value),
        _ => false,
    }
}

/// Port of frmMain `FindNegates`: the items and spells that negate, dispel, remove or kill the spells and
/// abilities in `q`, or whose spell immunity is beaten by the level. `only_in_game` is `bOnlyInGame`.
/// Items and spells are visited in number order, as MME's primary-key walk does.
#[must_use]
pub fn find_negates(
    items: &[Item],
    spells: &[Spell],
    q: &NegateQuery,
    only_in_game: bool,
) -> Vec<NegateRow> {
    let mut out = Vec::new();
    if q.spells.first() == Some(&0) && q.abilities.first() == Some(&0) {
        return out;
    }
    let level = q.level;
    let mut order: Vec<&Item> = items.iter().collect();
    order.sort_by_key(|i| i.number);
    for item in order {
        if only_in_game && !item.in_game {
            continue;
        }
        let negated = item
            .negate_spell
            .iter()
            .any(|&s| s > 0 && q.spells.contains(&s));
        let hit = negated
            || item
                .abilities
                .iter()
                .filter(|a| a.ability > 0)
                .any(|a| hit_ability(a.ability, a.value, q, |v| v > 0 && level > 0 && level < v));
        if hit {
            out.push(NegateRow {
                kind: NegateKind::Item,
                number: item.number,
                text: format!(
                    "Item: {} ({})",
                    item.name, item.number
                ),
            });
        }
    }
    let mut order: Vec<&Spell> = spells.iter().collect();
    order.sort_by_key(|s| s.number);
    for spell in order {
        let casted_by = spell.casted_by.as_deref().unwrap_or("");
        if only_in_game && vb::len(casted_by) < 3 && !spell.learnable {
            continue;
        }
        let item_cast = || {
            // InStr(1, "Casted By", "Item #", vbTextCompare) > 1
            casted_by
                .to_lowercase()
                .find("item #")
                .is_some_and(|p| casted_by.to_lowercase()[..p].chars().count() >= 1)
        };
        let hit = spell
            .abilities
            .iter()
            .filter(|a| a.ability > 0)
            .any(|a| {
                hit_ability(a.ability, a.value, q, |v| {
                    v > 0 && level > 0 && level < v && (spell.learnable || item_cast())
                })
            });
        if hit {
            out.push(NegateRow {
                kind: NegateKind::Spell,
                number: spell.number,
                text: format!(
                    "Spell: {} ({})",
                    spell.name.as_deref().unwrap_or(""),
                    spell.number
                ),
            });
        }
    }
    out
}

/// The "what casts this spell" result.
#[derive(Debug, Clone)]
pub enum WhatCasts {
    /// `Cannot find Spell.`
    NoSuchSpell,
    /// `Nothing.` (the `Casted By` text is shorter than 5 characters, or lists nothing).
    Nothing,
    /// The window's rows and its caption, `Spell <name> is casted by:`.
    Found {
        caption_name: String,
        list: LocationList,
    },
}

/// Port of frmMain `LookUpSpellCast` for the selected spell number: the rows `GetLocations` makes from the
/// spell's `Casted By` text. `hide_record_numbers` is `bHideRecordNumbers` (`GetSpellName`'s second argument).
pub fn what_casts(
    spells: &[Spell],
    data: &dyn LocationData,
    lairs: &mut dyn LocationLairs,
    opt: &LocationOptions,
    spell_number: i32,
) -> WhatCasts {
    let Some(spell) = find(spells, spell_number) else {
        return WhatCasts::NoSuchSpell;
    };
    let casted_by = spell.casted_by.as_deref().unwrap_or("");
    if vb::len(casted_by) < 5 {
        return WhatCasts::Nothing;
    }
    let mut list = LocationList::default();
    get_locations(
        data,
        lairs,
        opt,
        &LocationArgs {
            loc: casted_by,
            ..LocationArgs::default()
        },
        &mut list,
    );
    if list.rows.is_empty() {
        return WhatCasts::Nothing;
    }
    let name = spell.name.clone().unwrap_or_default();
    let caption_name = if spell_number == 0 {
        "None".to_string()
    } else if opt.hide_record_numbers {
        name
    } else {
        format!("{name}({spell_number})")
    };
    WhatCasts::Found { caption_name, list }
}
