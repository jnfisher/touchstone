//! MME's formulas, ported: pure functions over model types plus a `Profile`. No IO.
//!
//! Each module ports one formula family (see `docs/inventory.md` section 4 and
//! `docs/porting.md`) and is tested against the oracle's fixtures in `fixtures/rules/`.

pub mod abilities;
pub mod ability_text;
pub mod accuracy;
pub mod attack;
pub mod calculators;
pub mod char_panel;
pub mod character;
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
pub mod hit_calc;
pub mod item_manager;
pub mod item_value;
pub mod lairs;
pub mod locations;
pub mod mega_stat;
pub mod mmec;
pub mod monster_glue;
pub mod monster_sim;
pub mod paste_char;
pub mod paste_party;
pub mod pipeline;
pub mod rows;
pub mod spell_cast;
pub mod spells;
pub mod stats;
pub mod textblock;

/// Which game rules a calculation follows.
///
/// Only `Stock` is ported and fixture-tested: MME's stock (non-GreaterMUD) branches. A
/// Quicksilver profile is added later; MME's GreaterMUD and Paramud branches are not ported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Profile {
    #[default]
    Stock,
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
