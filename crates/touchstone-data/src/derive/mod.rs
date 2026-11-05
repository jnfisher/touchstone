//! Fields that MME's `.mdb` gets from NMR's pre-computation and silvermere has no column for
//! (T-014). Each submodule fills one group of fields on the `WorldParts` the silvermere loader
//! has built, from the same database snapshot, so a live reload re-derives them. Formats were
//! learned from the stock `.mdb`; what is not reproduced is listed in `docs/data-gaps.md`.
mod lairs;
mod obtained_from;
mod source;
mod spells;
mod summoned_by;

use rusqlite::Connection;
use touchstone_model::WorldParts;

use crate::silvermere::Result;

/// Run every derivation, in dependency order.
pub(crate) fn run(conn: &Connection, parts: &mut WorldParts) -> Result<()> {
    let src = source::Source::read(conn)?;
    lairs::apply(&src, parts)?;
    // The cross-references read each other's results: monsters that are in the game, then the
    // items they drop, then the spells the items teach.
    summoned_by::apply(&src, parts);
    obtained_from::apply(&src, parts);
    spells::apply(parts);
    Ok(())
}
