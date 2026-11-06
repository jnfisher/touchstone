//! Reads the scripts the game can reach, from the state of the derived fields so far, and keeps
//! what the clauses do. [`super::run`] repeats it until nothing new becomes part of the game.
use std::collections::HashSet;

use touchstone_model::{Spell, WorldParts};

use super::source::Source;
use super::walk::{Effects, Origin, Root, Walk, effects, walk};

/// Spell ability that runs a text block when the spell is cast.
const ABILITY_TEXT_BLOCK: i32 = 148;

pub(super) struct Scripts {
    pub walk: Walk,
    pub fx: Effects,
}

impl Scripts {
    pub(super) fn empty() -> Self {
        Self {
            walk: Walk::default(),
            fx: Effects::default(),
        }
    }

    /// Whether another read found the same scripts in the same order.
    pub(super) fn same_as(&self, other: &Self) -> bool {
        self.walk.order == other.walk.order && self.fx == other.fx
    }
}

/// The text block a spell runs: the ability's value, or the spell's base value when that is 0.
pub(super) fn spell_blocks(s: &Spell) -> Vec<i32> {
    let mut out = Vec::new();
    for a in s
        .abilities
        .iter()
        .filter(|a| a.ability == ABILITY_TEXT_BLOCK)
    {
        let b = if a.value > 0 { a.value } else { s.min_base };
        if b > 0 && !out.contains(&b) {
            out.push(b);
        }
    }
    out
}

/// Spells that are part of the game: taught, or cast by something.
pub(super) fn spell_in_game(s: &Spell) -> bool {
    s.learnable || s.casted_by.is_some()
}

pub(super) fn read(src: &Source, parts: &WorldParts) -> Scripts {
    let mut first: Vec<Root> = Vec::new();
    let mut rest: Vec<Root> = Vec::new();

    for r in parts.rooms.iter().filter(|r| r.cmd > 0) {
        first.push(Root {
            block: r.cmd,
            origin: Origin::Room,
        });
    }
    let spell_by_number: std::collections::HashMap<i32, &Spell> =
        parts.spells.iter().map(|s| (s.number, s)).collect();
    let room_spells = parts
        .rooms
        .iter()
        .map(|r| r.spell)
        .chain(src.exit_spells.iter().map(|e| e.0))
        .filter(|&s| s > 0);
    for n in room_spells {
        if let Some(s) = spell_by_number.get(&n) {
            for block in spell_blocks(s) {
                first.push(Root {
                    block,
                    origin: Origin::Spell(n),
                });
            }
        }
    }

    // Monsters that are in the game before any script is read have a room, a lair or a summon
    // spell; the others come from scripts and are read with everything else. The spells any
    // monster casts are read with the first group.
    let mut cast_by_monster: HashSet<i32> = HashSet::new();
    for m in parts.monsters.iter() {
        let Some(sb) = m.summoned_by.as_deref() else {
            continue;
        };
        cast_by_monster.extend(super::spells::monster_casts(m));
        if m.greet_txt <= 0 {
            continue;
        }
        let root = Root {
            block: m.greet_txt,
            origin: Origin::Monster(m.number),
        };
        if sb.split(", ").any(|e| !e.starts_with("Textblock")) {
            first.push(root);
        } else {
            rest.push(root);
        }
    }
    for s in parts.spells.iter().filter(|s| spell_in_game(s)) {
        let roots = spell_blocks(s).into_iter().map(|block| Root {
            block,
            origin: Origin::Spell(s.number),
        });
        if cast_by_monster.contains(&s.number) {
            first.extend(roots);
        } else {
            rest.extend(roots);
        }
    }

    let known: HashSet<i32> = parts
        .spells
        .iter()
        .filter(|s| spell_in_game(s))
        .map(|s| s.number)
        .collect();
    let w = walk(&src.blocks, &[first, rest], &|n| known.contains(&n));
    let fx = effects(&src.blocks, &w.order);
    Scripts { walk: w, fx }
}
