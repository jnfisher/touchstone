//! Ability effects and lookups (FAM-03). Card P3-03.
//!
//! Lookups take the table row's ability slots (`Option`: `None` is a failed Seek) instead of a
//! recordset; callers find the row with e.g.
//! `races.iter().find(|r| r.number == n).map(|r| &r.abilities[..])`.
//! The display text (`GetAbilityName`, `GetAbilityStats`) belongs to P3-07 and is not here.

use crate::Profile;
use touchstone_model::AbilitySlot;

/// What the lookups return for "does not have" (races, classes, items).
pub const NO_ABILITY: i32 = -31337;
/// What `spell_has_ability` returns for "does not have".
pub const NO_SPELL_ABILITY: i32 = -1;

/// Ability numbers that feed character stats on the stock engine, ascending.
const EFFECTS_CHAR_STATS: [i16; 94] = [
    2, 3, 4, 5, 7, 9, 10, 11, 13, 14, 15, 16, 18, 21, 22, 24, 25, 27, 29, 30, 31, 32, 34, 35, 36,
    37, 38, 39, 40, 41, 44, 45, 46, 47, 48, 49, 51, 53, 57, 58, 62, 63, 65, 66, 67, 68, 69, 70, 71,
    72, 74, 75, 76, 77, 79, 87, 88, 89, 90, 91, 92, 93, 94, 96, 99, 102, 103, 104, 105, 106, 107,
    116, 117, 118, 122, 123, 124, 139, 142, 145, 147, 150, 159, 165, 166, 170, 171, 172, 173, 178,
    179, 180, 186, 187,
];

/// Port of modMMudFunc `AbilityEffectsCharStats`: does this ability number change the
/// character's stats (stock engine)? The message-only abilities (101, 115, 120, 137, 144, 155)
/// count as "no", as they do when MME's `bForceAll` is off.
pub fn ability_effects_char_stats(profile: Profile, ability: i16) -> bool {
    match profile {
        Profile::Stock => EFFECTS_CHAR_STATS.binary_search(&ability).is_ok(),
    }
}

/// Port of modMain `GetAbilityStatSlot`, the `nEquip` half: the aggregate-stat slot an ability
/// number adds into, or -1 for none. The `sText` half (via `GetAbilityStats`) is P3-07's.
pub fn get_ability_stat_slot(ability: i16) -> i32 {
    match ability {
        2 | 10 => 2,
        3 => 28,
        4 => 11,
        5 => 27,
        7 => 3,
        13 | 14 => 23,
        22 | 105 | 106 => 10,
        24 => 20,
        25 => 32,
        27 => 19,
        29 => 37,
        30 => 38,
        34 => 8,
        35 => 39,
        36 => 24,
        37 | 180 => 22,
        40 | 179 => 21,
        44 => 104,
        45 => 124,
        46 => 101,
        47 => 123,
        48 => 102,
        49 => 103,
        58 => 7,
        65 => 25,
        66 => 29,
        67 => 31,
        69 => 6,
        70 => 9,
        77 => 18,
        88 => 5,
        89 => 40,
        90 => 41,
        91 => 42,
        92 => 34,
        93 => 35,
        94 => 36,
        96 => 4,
        116 => 13,
        117 => 14,
        118 => 15,
        123 => 16,
        142 => 12,
        145 => 17,
        147 => 26,
        165 => 33,
        _ => -1,
    }
}

/// Value of the first slot holding `ability`; the unused slots (ability 0) match ability 0.
fn first_value(slots: &[AbilitySlot], ability: i16) -> Option<i32> {
    slots
        .iter()
        .find(|s| s.ability == i32::from(ability))
        .map(|s| s.value)
}

/// Port of modMMudDatabase `RaceHasAbility`. `slots` is the row with Number = `race`, if any.
/// Returns the ability's value, or [`NO_ABILITY`].
pub fn race_has_ability(
    profile: Profile,
    slots: Option<&[AbilitySlot]>,
    race: i32,
    ability: i16,
) -> i32 {
    simple_lookup(profile, slots, race, ability, NO_ABILITY)
}

/// Port of modMMudDatabase `ClassHasAbility`. Returns the value, or [`NO_ABILITY`].
pub fn class_has_ability(
    profile: Profile,
    slots: Option<&[AbilitySlot]>,
    class: i32,
    ability: i16,
) -> i32 {
    simple_lookup(profile, slots, class, ability, NO_ABILITY)
}

/// Port of modMMudDatabase `SpellHasAbility`. Returns the value, or [`NO_SPELL_ABILITY`] (-1).
pub fn spell_has_ability(
    profile: Profile,
    slots: Option<&[AbilitySlot]>,
    spell: i32,
    ability: i16,
) -> i32 {
    simple_lookup(profile, slots, spell, ability, NO_SPELL_ABILITY)
}

fn simple_lookup(
    profile: Profile,
    slots: Option<&[AbilitySlot]>,
    number: i32,
    ability: i16,
    none: i32,
) -> i32 {
    match profile {
        Profile::Stock => {
            if ability <= 0 || number <= 0 {
                return none;
            }
            slots.and_then(|s| first_value(s, ability)).unwrap_or(none)
        }
    }
}

/// Port of modMMudDatabase `ItemHasAbility`. `ability` -1 asks for the first ability that
/// affects character stats and returns its number. Otherwise the first matching slot's value is
/// returned (ability 0 matches the first unused slot, as in MME), or [`NO_ABILITY`].
pub fn item_has_ability(
    profile: Profile,
    slots: Option<&[AbilitySlot]>,
    item: i32,
    ability: i16,
) -> i32 {
    match profile {
        Profile::Stock => {
            if ability < -1 || item < 1 {
                return NO_ABILITY;
            }
            let Some(slots) = slots else {
                return NO_ABILITY;
            };
            if ability == -1 {
                for s in slots {
                    // The Integer parameter of AbilityEffectsCharStats: an out-of-range
                    // stored number is a run-time error, which MME swallows and returns "none".
                    let Ok(n) = i16::try_from(s.ability) else {
                        return NO_ABILITY;
                    };
                    if n > 0 && ability_effects_char_stats(profile, n) {
                        return s.ability;
                    }
                }
                NO_ABILITY
            } else {
                first_value(slots, ability).unwrap_or(NO_ABILITY)
            }
        }
    }
}
