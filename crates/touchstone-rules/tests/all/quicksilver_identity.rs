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

/// R12 (CalcMR) and the stat tables: stats fixtures.
#[test]
fn stats_fixtures_identical_under_neutral() {
    crate::stats::check_fixture(neutral());
}

/// R13 (CalcDodge): accuracy fixtures (dodge, hit and dodge caps).
#[test]
fn accuracy_fixtures_identical_under_neutral() {
    crate::accuracy::check_fixture(neutral());
}

/// R1, R5, R11: attack fixtures (backstab, class melee share, crit knee).
#[test]
fn attack_fixtures_identical_under_neutral() {
    crate::attack::check_fixture(neutral());
}

/// R12 and R13 in the aggregation: the second half of CalcCharacterStats (magic resistance and
/// dodge).
#[test]
fn gear2_fixtures_identical_under_neutral() {
    crate::gear::check_gear2(neutral());
}

/// R12 through RefreshAll (magic resistance) and the rest of the pipeline.
#[test]
fn pipeline_fixtures_identical_under_neutral() {
    crate::pipeline::check_fixture(neutral());
}

/// R13 for party members (dodge) and the party damage that follows.
#[test]
fn paste_party_fixtures_identical_under_neutral() {
    crate::paste_party::check_fixture(neutral());
}

/// R14: the creation-point panel (RefreshCPs) prices stats above the race maximum.
#[test]
fn char_panel_fixtures_identical_under_neutral() {
    crate::char_panel::check_fixture(neutral());
}

/// R7, R8, R9: `CalculateSpellCast` (casting-stat and class scaling, the area resist factor) over its
/// whole fixture with a caster that has a casting stat, class and level.
#[test]
fn spell_cast_fixtures_identical_under_neutral() {
    crate::spell_cast::check_fixture(neutral());
}

/// R7, R8: the character profile carries the casting stat that `CalculateSpellCast` reads.
#[test]
fn character_fixtures_identical_under_neutral() {
    crate::character::check_fixture(neutral());
}
