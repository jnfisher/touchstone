//! The Quicksilver profile: John's MajorMUD server (1.11p plus the "1.20" rule changes).
//!
//! Quicksilver's tuning numbers are a config table, not data, so touchstone carries its own copy
//! ([`Balance`]) and picks one per content version. Design: `docs/quicksilver-profile.md`.

mod balance;
mod v120;
pub mod weave;

pub use balance::{
    Anchors, Balance, BalanceError, ClassMeleePct, ClassSpellPct, ClassWeavePct, LevelPct,
    MAX_ANCHORS, RankWeights,
};
pub use v120::V120_BALANCE;
