//! Detail panes of the spell, shop, class and race explorers and the NPC command references of the
//! map list (FAM-20, stock only). Card P3-37.
//!
//! MME fills text boxes and ListViews; each port returns what MME writes into them, exactly (the
//! `text` fields and row cells are what the oracle fixture records), plus [`DetailRow`]s, the same
//! content as (label, value, kind) triples for a UI that does not want to parse the text.

pub mod class_race;
pub mod shop;
pub mod spell;

/// What a [`DetailRow`] holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailKind {
    /// A class or race ability, as `GetAbilityStats` words it.
    Ability,
    /// A stocked item or a training level of a shop.
    ShopLine,
    /// A line of a summary (levels, markup, class restriction, spell facts).
    Fact,
    /// A cost, charm or mana figure.
    Cost,
    /// A reference row (where it is found, what it teleports to).
    Reference,
    /// "Class not found" and the like.
    Message,
}

/// One label/value pair of a detail pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailRow {
    pub label: String,
    pub value: String,
    pub kind: DetailKind,
}

impl DetailRow {
    pub(crate) fn new(label: &str, value: impl Into<String>, kind: DetailKind) -> Self {
        Self {
            label: label.to_string(),
            value: value.into(),
            kind,
        }
    }
}
