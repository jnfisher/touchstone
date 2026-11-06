//! The Spell Book window. Card P4-32.
//!
//! Port of frmSpellBook `cmdListSpells_Click` (which spells are listed, and whether the loaded
//! character's profile builds the rows), `cmbClass_Click` (a class sets the magery boxes) and
//! `CopyGlobalChar` (the global filter fills the window). `AddSpell2LV` rows are
//! [`crate::rows::add_spell_row`].

use touchstone_model::{Class, Spell};

use crate::spell_cast::UsableEnv;

/// The window's controls.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BookQuery {
    /// `cmbClass.ItemData` (0 for "none").
    pub class: i32,
    /// `Val(txtLevel.Text)`.
    pub level: f64,
    /// `cmbAlignment.ListIndex`: 0 any, 1 good, 2 neutral, 3 evil.
    pub alignment: i32,
}

/// The global filter and the loaded character, as `cmdListSpells_Click` reads them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BookGlobals {
    /// `frmMain.chkGlobalFilter.Value = 1`.
    pub filter_on: bool,
    /// `Val(frmMain.txtGlobalLevel(1).Text) > 0`.
    pub global_level_set: bool,
    /// `cmbGlobalClass(0).ItemData` when its `ListIndex > 0`, else 0.
    pub global_class: i32,
    /// `nGlobalCharMagery`, `nGlobalCharMageryLVL`.
    pub char_magery: i32,
    pub char_magery_lvl: i32,
}

/// The magery boxes after `cmbClass_Click`: `(magery level index, magery index)`.
#[must_use]
pub fn class_magery_boxes(class: Option<&Class>) -> Option<(i32, i32)> {
    let c = class?;
    Some((c.magery_lvl.min(3), c.magery_type))
}

/// Whether the rows are built for the loaded character (`bDontUseCharacter` is False): the filter is
/// on with a level, the window's class is the character's class, and both magery boxes equal the
/// character's own.
#[must_use]
pub fn uses_character(g: &BookGlobals, q: &BookQuery, magery: i32, magery_level: i32) -> bool {
    g.filter_on
        && g.global_level_set
        && g.global_class > 0
        && q.class > 0
        && g.global_class == q.class
        && magery > 0
        && magery_level > 0
        && magery == g.char_magery
        && magery_level == g.char_magery_lvl
}

/// The spells `cmdListSpells_Click` lists, in table order.
#[must_use]
pub fn list_spells(env: &UsableEnv<'_>, q: &BookQuery) -> Vec<i32> {
    let level = i16::try_from(touchstone_vb6::cint(q.level).map_or(0, i32::from)).unwrap_or(0);
    let align = i16::try_from(q.alignment).unwrap_or(0);
    env.spells
        .iter()
        .filter(|s: &&Spell| env.spell_is_usable(s.number, q.class, level, align, true))
        .map(|s| s.number)
        .collect()
}
