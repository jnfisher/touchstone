//! Fields that MME's `.mdb` gets from NMR's pre-computation and silvermere has no column for
//! (T-014). Each submodule fills one group of fields on the `WorldParts` the silvermere loader
//! has built, from the same database snapshot, so a live reload re-derives them. Formats were
//! learned from the stock `.mdb`; what is not reproduced is listed in `docs/data-gaps.md`.
mod chest;
mod history;
mod lairs;
mod obtained_from;
mod script;
mod scripts;
mod source;
mod spells;
mod summoned_by;
mod tbinfo;
mod walk;

use rusqlite::Connection;
use touchstone_model::WorldParts;

use crate::silvermere::Result;

/// Passes after which the loop stops whatever happens; the stock game settles in under ten.
const MAX_PASSES: usize = 20;

/// Run every derivation. Scripts make monsters, items and spells part of the game, which makes
/// more scripts reachable, so the cross-references are derived again until the scripts read
/// are the same twice.
pub(crate) fn run(conn: &Connection, parts: &mut WorldParts) -> Result<()> {
    let src = source::Source::read(conn)?;
    lairs::apply(&src, parts)?;
    let rooms = summoned_by::prepare(&src, parts);
    let mut sc = scripts::Scripts::empty();
    let mut hist = history::History::default();
    let mut seen = snapshot(parts);
    for pass in 1..=MAX_PASSES {
        hist.pass = pass as u32;
        // The cross-references read each other's results: monsters that are in the game, then
        // the items they drop, then the spells the items teach.
        summoned_by::apply(&rooms, parts, &sc, &mut hist);
        obtained_from::apply(&src, parts, &sc, &mut hist);
        spells::apply(&src, parts, &sc, &mut hist);
        let next = scripts::read(&src, parts);
        tbinfo::observe(&src, parts, &next, &mut hist);
        let now = snapshot(parts);
        let done = next.same_as(&sc) && now == seen;
        sc = next;
        seen = now;
        if done {
            break;
        }
    }
    tbinfo::apply(&src, parts, &sc, &mut hist);
    Ok(())
}

/// The fields the passes feed each other, to tell when a pass changed nothing.
fn snapshot(parts: &WorldParts) -> Vec<Option<String>> {
    let mut v: Vec<Option<String>> = Vec::new();
    v.extend(parts.monsters.iter().map(|m| m.summoned_by.clone()));
    v.extend(parts.items.iter().map(|i| i.obtained_from.clone()));
    v.extend(parts.spells.iter().map(|s| s.casted_by.clone()));
    v.extend(parts.spells.iter().map(|s| s.learned_from.clone()));
    v
}
