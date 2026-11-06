//! `Item #n(p%)` in `Items.Obtained From`: a chest item casts a spell, the spell runs a text
//! block, and the block (through `random` clauses) hands out items. NMR gives the chance that
//! one use of the chest yields the item.
//!
//! The estimate here treats every `random` clause as an independent draw and a random block as
//! picking one line by its share, which matches NMR for chests whose blocks call random blocks
//! that give items directly. For longer chains of random blocks NMR's figures are lower than
//! this estimate and the rule behind them is not known (docs/data-gaps.md).
use std::collections::{BTreeMap, HashMap};

use touchstone_model::WorldParts;

use super::script::{Blocks, LineKind, RefKind, lines, reference};
use super::scripts::spell_blocks;
use super::source::Source;

/// Item ability that casts the spell in its value.
const ABILITY_ITEM_CAST: i32 = 43;
/// `random` chains longer than this give nothing (the stock data nests four deep at most).
const MAX_DEPTH: u32 = 10;

type Chances = BTreeMap<i32, f64>;

/// Merge `other` in as an independent chance for each item.
fn either(into: &mut Chances, other: &Chances) {
    for (&item, &p) in other {
        let e = into.entry(item).or_insert(0.0);
        *e = 1.0 - (1.0 - *e) * (1.0 - p);
    }
}

fn clause_chances(blocks: &Blocks, clauses: &[&str], depth: u32) -> Chances {
    let mut out = Chances::new();
    for clause in clauses {
        if let Some((super::script::Act::GiveItem, item)) = super::script::act(clause) {
            either(&mut out, &Chances::from([(item, 1.0)]));
        } else if let Some(r) = reference(clause)
            && r.kind == RefKind::Random
        {
            either(&mut out, &block_chances(blocks, r.target, depth + 1));
        }
    }
    out
}

/// The chance of each item when the block runs.
fn block_chances(blocks: &Blocks, number: i32, depth: u32) -> Chances {
    let Some(block) = blocks.get(&number) else {
        return Chances::new();
    };
    if depth > MAX_DEPTH {
        return Chances::new();
    }
    let ls = lines(&block.text);
    let mut out = Chances::new();
    if ls.iter().any(|l| l.kind == LineKind::Random) {
        // One line is picked, by its share of the roll.
        for l in ls.iter().filter(|l| l.kind == LineKind::Random) {
            let share = l.pct.max(0) as f64 / 100.0;
            for (item, p) in clause_chances(blocks, &l.clauses, depth) {
                *out.entry(item).or_insert(0.0) += share * p;
            }
        }
    } else {
        for l in &ls {
            either(&mut out, &clause_chances(blocks, &l.clauses, depth));
        }
    }
    out
}

/// `65` → "65", `12.64` → "12.6": one decimal, none when it is zero.
fn percent(p: f64) -> String {
    let tenths = (p * 1000.0).round() as i64;
    if tenths % 10 == 0 {
        format!("{}", tenths / 10)
    } else {
        format!("{}.{}", tenths / 10, tenths % 10)
    }
}

/// `Item #n(p%)` entries for every item a chest hands out, in chest order.
pub(super) fn entries(src: &Source, parts: &WorldParts) -> HashMap<i32, Vec<String>> {
    let spells: HashMap<i32, _> = parts.spells.iter().map(|s| (s.number, s)).collect();
    let mut chests: Vec<_> = parts
        .items
        .iter()
        .filter(|i| i.obtained_from.is_some())
        .collect();
    chests.sort_by_key(|i| i.number);
    let mut out: HashMap<i32, Vec<String>> = HashMap::new();
    // Of several chests that cast one spell only the first is listed.
    let mut taken: Vec<i32> = Vec::new();
    for chest in chests {
        let mut chances = Chances::new();
        for a in chest
            .abilities
            .iter()
            .filter(|a| a.ability == ABILITY_ITEM_CAST && a.value > 0)
        {
            let Some(spell) = spells.get(&a.value) else {
                continue;
            };
            if taken.contains(&a.value) {
                continue;
            }
            taken.push(a.value);
            // Only what the `random` clauses of the spell's block hand out counts; an item the
            // block gives outright is listed as the block itself.
            for block in spell_blocks(spell) {
                let Some(b) = src.blocks.get(&block) else {
                    continue;
                };
                for l in lines(&b.text) {
                    for clause in &l.clauses {
                        if let Some(r) = reference(clause)
                            && r.kind == RefKind::Random
                        {
                            either(&mut chances, &block_chances(&src.blocks, r.target, 2));
                        }
                    }
                }
            }
        }
        // A chest that gives itself back (a key that reappears) is not listed under itself.
        for (item, p) in chances
            .into_iter()
            .filter(|&(item, p)| p > 0.0 && item != chest.number)
        {
            out.entry(item)
                .or_default()
                .push(format!("Item #{}({}%)", chest.number, percent(p)));
        }
    }
    out
}
