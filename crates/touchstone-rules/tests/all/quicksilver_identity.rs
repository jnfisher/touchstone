//! Layer 1 of the Quicksilver tests (`docs/quicksilver-profile.md` part 4): with every parametric
//! rule at its stock-equivalent value, the Quicksilver profile must give the MME answer. The MME
//! fixtures are the reference, run through `Profile::Quicksilver(&Balance::NEUTRAL)`.
//!
//! Each card that makes a family parametric adds its fixture test here; a family whose formula
//! does not read the profile yet passes trivially, so the harness guards every later card.
//! Structural rules with no neutral setting (R2, R3, R10) are not covered here.

use touchstone_rules::Profile;
use touchstone_rules::quicksilver::Balance;

fn neutral() -> Profile {
    Profile::Quicksilver(&Balance::NEUTRAL)
}

/// R12 and the stat tables: stats fixtures.
#[test]
fn stats_fixtures_identical_under_neutral() {
    crate::stats::check_fixture(neutral());
}

/// R13: accuracy fixtures (dodge, hit and dodge caps).
#[test]
fn accuracy_fixtures_identical_under_neutral() {
    crate::accuracy::check_fixture(neutral());
}

/// R1, R5, R11: attack fixtures (backstab, class melee share, crit knee).
#[test]
fn attack_fixtures_identical_under_neutral() {
    crate::attack::check_fixture(neutral());
}

/// R14: the creation-point panel (RefreshCPs) prices stats above the race maximum.
#[test]
fn char_panel_fixtures_identical_under_neutral() {
    crate::char_panel::check_fixture(neutral());
}
