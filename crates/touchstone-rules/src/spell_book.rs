//! The Spell Book window. Card P4-32.
//!
//! Port of frmSpellBook `cmdListSpells_Click` (which spells are listed, whether the loaded character's
//! profile builds the rows, and how the list is sorted afterwards), `cmbClass_Click` (a class sets the
//! magery boxes) and `CopyGlobalChar` (the global filter fills the window). The rows are
//! [`crate::rows::add_spell_row`]'s, with the loaded character's profile or the "no character" one.

use touchstone_model::{Class, Spell};
use touchstone_vb6 as vb;

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

/// Whether the rows are built for the loaded character (`bDontUseCharacter` is False): the filter is
/// on with a level, the window's class is the character's class, and both magery boxes (1 and up)
/// equal the character's own.
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

/// What `cmdListSpells_Click` lists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BookList {
    /// The spells that get a row, in table order.
    pub spells: Vec<i32>,
    /// A VB6 error ended the procedure at the first spell (a level that is not an Integer): the list
    /// is empty and is not sorted.
    pub aborted: bool,
    /// The sort that follows (`LV_RefreshSort(lv, 5, number, False, ascending)`): the level column,
    /// ascending only when the global filter is off.
    pub sort_ascending: bool,
}

/// The spells `cmdListSpells_Click` lists for the window's class, level and alignment.
#[must_use]
pub fn list_spells(env: &UsableEnv<'_>, q: &BookQuery, filter_on: bool) -> BookList {
    let mut out = BookList {
        sort_ascending: !filter_on,
        ..BookList::default()
    };
    if env.spells.is_empty() {
        return out;
    }
    // `val(txtLevel.Text)` goes into an Integer parameter: CInt, which overflows past 32767
    let Ok(level) = vb::cint(q.level) else {
        out.aborted = true;
        return out;
    };
    let align = i16::try_from(q.alignment).unwrap_or(0);
    out.spells = env
        .spells
        .iter()
        .filter(|s: &&Spell| env.spell_is_usable(s.number, q.class, level, align, true))
        .filter(|s| has_row(s))
        .map(|s| s.number)
        .collect();
    out
}

/// `AddSpell2LV` adds no row for a spell without a name, one whose name starts with "1", or "sdf".
fn has_row(s: &Spell) -> bool {
    let name = s.name.as_deref().unwrap_or("");
    !(name.is_empty()
        || name.starts_with('1')
        || name.chars().take(3).collect::<String>().to_lowercase() == "sdf")
}

/// What picking a class does to the magery boxes (`cmbClass_Click`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassPick {
    /// "none" (class 0): nothing changes.
    Nothing,
    /// `Class not found.`
    NotFound,
    /// The new `cmbSpellMageryLevel` and `cmbSpellMagery` indexes; a box that has too few entries
    /// stays as it was (`None`).
    Boxes {
        magery_level: Option<i32>,
        magery: Option<i32>,
    },
}

/// Port of `cmbClass_Click`. `level_count` and `magery_count` are the boxes' `ListCount`s (4 and 6).
#[must_use]
pub fn pick_class(classes: &[Class], class: i32, level_count: i32, magery_count: i32) -> ClassPick {
    if class < 1 {
        return ClassPick::Nothing;
    }
    let Some(c) = classes.iter().find(|c| c.number == class) else {
        return ClassPick::NotFound;
    };
    let lvl = c.magery_lvl.min(3);
    ClassPick::Boxes {
        magery_level: (lvl < level_count).then_some(lvl),
        magery: (c.magery_type < magery_count).then_some(c.magery_type),
    }
}

/// `CopyGlobalChar`'s inputs.
#[derive(Debug, Clone)]
pub struct GlobalCopy<'a> {
    /// `chkGlobalFilter.Value = 1`.
    pub filter_on: bool,
    /// `cmbGlobalAlignment.ListIndex`.
    pub alignment: i32,
    /// `txtGlobalLevel(1).Text`.
    pub level_text: &'a str,
    /// `cmbGlobalClass(1).ListIndex`.
    pub class_index: i32,
    /// `cmbGlobalClass(1).ItemData(cmbGlobalClass(0).ListIndex)`: MME indexes the second box's
    /// list with the first box's selection.
    pub class_item_data: i32,
    /// The window's class box, `ItemData` per entry ("none" first).
    pub book_classes: &'a [i32],
}

/// What `CopyGlobalChar` sets in the window; `None` leaves the control as it is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GlobalCopied {
    pub alignment: Option<i32>,
    pub level: Option<String>,
    pub class_index: Option<usize>,
}

/// Port of frmSpellBook `CopyGlobalChar`.
#[must_use]
pub fn copy_global_char(g: &GlobalCopy<'_>) -> GlobalCopied {
    let mut out = GlobalCopied::default();
    if !g.filter_on {
        return out;
    }
    if g.alignment > 0 {
        out.alignment = Some(g.alignment);
    }
    let level = vb::val(g.level_text);
    if level > 0.0 {
        out.level = Some(if level.fract() == 0.0 {
            format!("{level:.0}")
        } else {
            level.to_string()
        });
    }
    if g.class_index > 0 {
        out.class_index = g.book_classes.iter().position(|&c| c == g.class_item_data);
    }
    out
}
