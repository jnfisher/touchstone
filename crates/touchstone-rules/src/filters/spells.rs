//! `FilterSpells`: which spells the Spells list shows. Card P3-30.
//!
//! The port takes the tab's controls as a [`SpellFilter`] (its `Default` is what `ResetFilterOptions`
//! leaves in them) and returns the spells `AddSpell2LV` is called for, in table order. The row each call
//! builds is [`crate::rows::add_spell_row`] with the profile the caller made for the list.

use touchstone_model::Spell;
use touchstone_vb6 as vb;

use crate::character::CharacterProfile;
use crate::spell_cast::UsableEnv;

/// The Spells tab's controls.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpellFilter {
    /// `cmbSpellMagery.ListIndex`: 0 any, else the magery number the spell must have.
    pub magery: i32,
    /// `cmbSpellMageryLevel.ListIndex`: 0 any, else the least `MageryLVL` the spell may have.
    pub magery_level: i32,
    /// `chkSpellOptions(1)`: only spells a character can learn.
    pub learnable_only: bool,
    /// `cmbSpellAttackType.ListIndex`: 0 any, else the spell's `AttType` plus one.
    pub attack_type: i32,
    /// `cmbSpellTarget.ListIndex`: 0 any, 1 self, 2 user, 3 monster, 4 party, 5 room.
    pub target: i32,
    /// `cmbSpellContainsAbil.ItemData(ListIndex)`: 0 or less for any, else an ability number.
    pub contains_ability: i32,
}

/// What the filter reads besides its own controls.
pub struct SpellFilterEnv<'a> {
    /// The tables and the data version (`SpellIsInGame`, `SpellIsUsable`).
    pub usable: &'a UsableEnv<'a>,
    /// `chkGlobalFilter.Value = 1`.
    pub use_character: bool,
    /// `cmbGlobalClass(0).ItemData(ListIndex)`: the global filter's class, 0 for "any class".
    pub global_class: i32,
    /// `Val(txtGlobalLevel(1).Text)`, which decides whether column 7 reads "Cast".
    pub global_level: f64,
}

/// The finished Spells list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpellList {
    /// The spells `AddSpell2LV` is called for.
    pub entries: Vec<i32>,
    /// MME left the list as it was (no spells).
    pub cancelled: bool,
    /// A VB6 error ended the procedure; `entries` is what was built.
    pub aborted: bool,
    /// Some spell was filtered out (the "*Spells*" caption).
    pub filtered: bool,
    /// Column 7 reads "Cast" (the character's chance) instead of "Diff".
    pub cast_header: bool,
}

fn has_ability(s: &Spell, ability: i32) -> bool {
    s.abilities.iter().any(|a| a.ability == ability)
}

/// Does the Targets box let a spell with this `Targets` through?
fn target_ok(choice: i32, targets: i32) -> bool {
    match choice {
        1 => matches!(targets, 1 | 2),
        2 => matches!(targets, 0 | 2 | 8),
        3 => matches!(targets, 4 | 6 | 8),
        4 => matches!(targets, 5 | 10 | 13),
        5 => matches!(targets, 11 | 12 | 9 | 3),
        // 0, and any index the box does not have, filter nothing
        _ => true,
    }
}

/// Port of frmMain `FilterSpells`. `remove_filter` is `bREMOVE_FILTER` (every in-game spell is listed).
/// `t_char` is the profile `PopulateCharacterProfile(tChar, False, True, , , bREMOVE_FILTER)` made.
#[must_use]
pub fn filter_spells(
    env: &SpellFilterEnv,
    f: &SpellFilter,
    t_char: &CharacterProfile,
    remove_filter: bool,
) -> SpellList {
    let mut out = SpellList::default();
    if env.usable.spells.is_empty() {
        out.cancelled = true;
        return out;
    }
    out.cast_header = !remove_filter && env.use_character && env.global_level > 0.0;
    if spell_steps(env, f, t_char, remove_filter, &mut out).is_err() {
        out.aborted = true;
    }
    out
}

fn spell_steps(
    env: &SpellFilterEnv,
    f: &SpellFilter,
    t_char: &CharacterProfile,
    remove_filter: bool,
    out: &mut SpellList,
) -> Result<(), vb::Vb6Error> {
    let u = env.usable;
    let use_character = env.use_character;
    for s in u.spells {
        if u.only_in_game && !u.spell_is_in_game(s.number) {
            continue; // skip_dont_mark_filtered
        }
        if remove_filter {
            out.entries.push(s.number);
            continue;
        }
        if !passes(env, f, t_char, s, use_character)? {
            out.filtered = true;
            continue;
        }
        out.entries.push(s.number);
    }
    Ok(())
}

fn passes(
    env: &SpellFilterEnv,
    f: &SpellFilter,
    t_char: &CharacterProfile,
    s: &Spell,
    use_character: bool,
) -> Result<bool, vb::Vb6Error> {
    let u = env.usable;
    let mut skip_magery_check = false;
    if f.magery > 0 && f.magery != s.magery {
        // a learnable spell without a magery of its own (data 1.7 on) is still offered to the class
        // that can learn it
        if s.learnable && s.magery == 0 && use_character && u.nmr_ver >= 1.7 {
            let classes = s.classes.as_deref().unwrap_or("");
            let wanted = format!("({})", env.global_class);
            if env.global_class == 0
                || classes == "(*)"
                || classes.to_lowercase().contains(&wanted.to_lowercase())
            {
                skip_magery_check = true;
            } else {
                return Ok(false);
            }
        } else {
            return Ok(false);
        }
    }
    if !skip_magery_check {
        if f.magery_level != 0 && f.magery_level < s.magery_lvl {
            return Ok(false);
        }
        if f.learnable_only && !s.learnable && (f.magery != 5 || u.disable_kai_autolearn) {
            return Ok(false);
        }
    }
    if f.attack_type != 0 && s.att_type != f.attack_type - 1 {
        return Ok(false);
    }
    if f.target != 0 && !target_ok(f.target, s.targets) {
        return Ok(false);
    }
    if use_character {
        let level = i16::try_from(t_char.level).map_err(|_| vb::Vb6Error::Overflow)?;
        if !u.spell_is_usable(
            s.number,
            t_char.class,
            level,
            t_char.align,
            f.learnable_only,
        ) {
            return Ok(false);
        }
    }
    if f.contains_ability > 0 && !has_ability(s, f.contains_ability) {
        return Ok(false);
    }
    Ok(true)
}
