//! Which items a planning tool offers a character: MME's `ItemIsUsableByChar`, with the switches
//! the Weapon Simulator and the Gear Planner share for looking past a restriction.

use serde::Deserialize;
use touchstone_model::{Item, World};
use touchstone_rules::Profile;
use touchstone_rules::gear::{Character as GlobalChar, item_is_usable_by_char};

const fn yes() -> bool {
    true
}

/// The switches of a request, flattened into its body.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Allow {
    /// Keep items above the character's level (to plan ahead).
    #[serde(default)]
    pub ignore_level: bool,
    #[serde(default)]
    pub ignore_alignment: bool,
    /// Limited items are the realm's rare ones; on by default, as a best-of list wants them.
    #[serde(default = "yes")]
    pub include_limited: bool,
    /// Skip items nothing in the data yields (no room, shop, monster, chest or script): leftovers
    /// such as test items, which a live database lists like any other row. On by default.
    #[serde(default = "yes")]
    pub only_obtainable: bool,
}

/// Why an item is offered although the character cannot use it today.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Verdict {
    pub needs_level: bool,
    pub wrong_alignment: bool,
}

/// The level an item asks for (ability 135), 0 for none.
pub fn level_req(item: &Item) -> i32 {
    item.abilities
        .iter()
        .find(|s| s.ability == 135)
        .map_or(0, |s| s.value)
}

/// Something in the data yields the item.
pub fn obtainable(item: &Item) -> bool {
    item.obtained_from.as_deref().is_some_and(|s| !s.is_empty())
}

/// The character the rule judges by; the global filter's minimum item level is not applied.
pub fn who(ch: &touchstone_rules::character::Character) -> GlobalChar {
    GlobalChar {
        level: ch.level,
        class: ch.class.unwrap_or(0),
        race: ch.race.unwrap_or(0),
        alignment: i32::from(ch.alignment),
        min_item_level: 0,
    }
}

/// May the tool offer `item`? `keep` is an item the character already holds or the user asked
/// for by name: the limited and obtainable switches do not drop it, and one already held is kept
/// whatever the rule says.
pub fn offered(
    profile: Profile,
    world: &World,
    who: &GlobalChar,
    item: &Item,
    allow: &Allow,
    held: bool,
    asked_for: bool,
) -> Option<Verdict> {
    if !item.in_game {
        return None;
    }
    let keep = held || asked_for;
    if !allow.include_limited && item.limit != 0 && !keep {
        return None;
    }
    if allow.only_obtainable && !obtainable(item) && !keep {
        return None;
    }
    let usable = |level: i32, skip_align: bool| {
        item_is_usable_by_char(
            profile,
            world,
            &GlobalChar { level, ..*who },
            &[0; 20],
            item.number,
            true,
            skip_align,
        )
    };
    if usable(who.level, false) || held {
        return Some(Verdict::default());
    }
    // a level above the character's own only ever lifts the minimum-level rule
    let level = if allow.ignore_level {
        who.level.max(level_req(item))
    } else {
        who.level
    };
    if !usable(level, allow.ignore_alignment) {
        return None;
    }
    Some(Verdict {
        needs_level: !usable(who.level, true),
        wrong_alignment: !usable(level, false),
    })
}
