//! MME's formulas, ported: pure functions over model types plus a `Profile`. No IO.
//!
//! Each module ports one formula family (see `docs/inventory.md` section 4 and
//! `docs/porting.md`) and is tested against the oracle's fixtures in `fixtures/rules/`.

pub mod abilities;
pub mod ability_text;
pub mod accuracy;
pub mod acquire;
pub mod attack;
pub mod calculators;
pub mod char_panel;
pub mod character;
pub mod chests;
pub mod class_race_builder;
pub mod clipboard;
pub mod combat;
pub mod detail;
pub mod equip;
pub mod exp;
pub mod exp_per_hour;
pub mod filters;
pub mod find_best;
pub mod game_text;
pub mod gear;
pub mod gear_plan;
pub mod hit_calc;
pub mod item_manager;
pub mod item_value;
pub mod lairs;
pub mod locations;
pub mod map;
pub mod mega_stat;
pub mod mmec;
pub mod monster_glue;
pub mod monster_sim;
pub mod paste_char;
pub mod paste_party;
pub mod pipeline;
pub mod quicksilver;
pub mod reachability;
pub mod rooms;
pub mod rows;
pub mod spell_book;
pub mod spell_cast;
pub mod spell_tools;
pub mod spells;
pub mod stats;
pub mod textblock;
pub mod weapon_sim;

/// Which game rules a calculation follows.
///
/// `Stock` is MME's stock (non-GreaterMUD) branches, ported and fixture-tested. `Quicksilver` is
/// stock plus John's server's 1.20 changes, parameterised by a [`quicksilver::Balance`]; until the
/// Q-006..Q-014 cards port each rule it answers exactly as `Stock` does. MME's GreaterMUD and
/// Paramud branches are not ported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Profile {
    #[default]
    Stock,
    /// Quicksilver, with the balance numbers to use (a `static`, or one leaked at startup from a
    /// config override).
    Quicksilver(&'static quicksilver::Balance),
}

impl Profile {
    /// Quicksilver with the frozen 1.20.0 balance.
    #[must_use]
    pub fn quicksilver_v120() -> Self {
        Self::Quicksilver(&quicksilver::V120_BALANCE)
    }

    /// The balance of a Quicksilver profile, `None` for `Stock`.
    #[must_use]
    pub fn balance(self) -> Option<&'static quicksilver::Balance> {
        match self {
            Self::Stock => None,
            Self::Quicksilver(b) => Some(b),
        }
    }
}

/// Why a rule could not produce an answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RulesError {
    /// The branch MME would take for these inputs is not ported (GreaterMUD, Paramud).
    #[error("not ported: {0}")]
    Unsupported(&'static str),
    /// VB6 raised a run-time error where MME would have (overflow, division by zero, ...).
    #[error("VB6 run-time error {0}")]
    Vb6(i32),
}
