//! The order NMR lists a record's entries in follows the pass in which each one came into the
//! game: what a room, a lair or a drop gives comes first, what only a script reaches comes
//! later. The loop in [`super::run`] derives the cross-references again and again until nothing
//! changes, so each entry is stamped with the pass that first produced it and a list is sorted
//! by that stamp (stable, so the order inside one pass is the derivation's own).
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum Field {
    SummonedBy,
    ObtainedFrom,
    CastedBy,
    LearnedFrom,
    CalledFrom,
}

#[derive(Default)]
pub(super) struct History {
    pub pass: u32,
    seen: HashMap<(Field, i32), HashMap<String, u32>>,
}

impl History {
    /// Stamp the entries not seen before with the current pass and return all of them sorted
    /// by stamp.
    pub(super) fn order(&mut self, field: Field, key: i32, entries: Vec<String>) -> Vec<String> {
        let stamps = self.seen.entry((field, key)).or_default();
        for e in &entries {
            if !stamps.contains_key(e) {
                stamps.insert(e.clone(), self.pass);
            }
        }
        let mut stamped: Vec<(u32, String)> =
            entries.into_iter().map(|e| (stamps[&e], e)).collect();
        stamped.sort_by_key(|(p, _)| *p);
        stamped.into_iter().map(|(_, e)| e).collect()
    }
}
