//! The Classes/Races builder endpoints: the text of MME's Build view and the Equip and Spellbook
//! buttons. The rules are `touchstone_rules::class_race_builder`; this module supplies the tables
//! and `gAvgLevelMaxSingleStat` and shapes the JSON.

use std::sync::Arc;

use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};
use touchstone_rules::Profile;
use touchstone_rules::RulesError;
use touchstone_rules::ability_text::AbilityEnv;
use touchstone_rules::attack::{AttackEnv, EquippedGlobals};
use touchstone_rules::class_race_builder::{self as builder, Spellbook};
use touchstone_rules::stats::compute_avg_level_max_stats;

use crate::calc::{ApiResult, Body, json, vb_error_text};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct BuildQuery {
    /// The selected class record number; 0 or absent for none.
    #[serde(default)]
    class: i32,
    /// The selected race record number; 0 or absent for none.
    #[serde(default)]
    race: i32,
}

#[derive(Serialize)]
struct BuildJson {
    /// The class and race MME used: a number with no row counts as none.
    class: i32,
    race: i32,
    /// The text box, CRLF between lines; empty when there is nothing to show. `None` when MME's
    /// handler ran (the box keeps what it showed), with `error` saying which error.
    text: Option<String>,
    lines: Vec<String>,
    /// `nLevelSteps`: the levels of the four columns' rows (the first is always 1).
    levels: [i32; 5],
    error: Option<String>,
}

pub async fn build_route(
    State(state): State<Arc<AppState>>,
    Query(q): Query<BuildQuery>,
) -> ApiResult {
    let loaded = state.current();
    let world = &loaded.world;
    let abilities = AbilityEnv::for_world(world, Profile::Stock);
    let equipped = EquippedGlobals::default();
    let avg = compute_avg_level_max_stats(Profile::Stock, &world.races);
    let env = AttackEnv {
        items: &world.items,
        classes: &world.classes,
        races: &world.races,
        equipped: &equipped,
        cast_text: &abilities,
        dat_ver_caption: &world.info.dat_file_version,
        avg_level_max_all_stats: avg.all_stats,
        accuracy_tag: "",
    };
    let class = if world.classes.iter().any(|c| c.number == q.class) {
        q.class
    } else {
        0
    };
    let race = if world.races.iter().any(|r| r.number == q.race) {
        q.race
    } else {
        0
    };
    let out = match builder::build(Profile::Stock, &env, avg.single_stat, q.class, q.race) {
        Ok(b) => BuildJson {
            class,
            race,
            text: Some(b.text),
            lines: b.lines,
            levels: b.levels,
            error: None,
        },
        Err(e) => BuildJson {
            class,
            race,
            text: None,
            lines: Vec::new(),
            levels: [0; 5],
            error: Some(match e {
                RulesError::Vb6(n) => vb_error_text(u32::try_from(n).unwrap_or(0)),
                other => other.to_string(),
            }),
        },
    };
    json(&out)
}

#[derive(Deserialize)]
pub struct Selection {
    #[serde(default)]
    class: i32,
    #[serde(default)]
    race: i32,
}

#[derive(Serialize)]
struct EquipJson {
    /// The EQ tab's class picker takes this, when set.
    class: Option<i32>,
    race: Option<i32>,
}

/// The Equip button: which selections the EQ tab's pickers take.
pub async fn equip_route(
    State(state): State<Arc<AppState>>,
    Body(sel): Body<Selection>,
) -> ApiResult {
    let loaded = state.current();
    let world = &loaded.world;
    let out = builder::equip_selection(&world.classes, &world.races, sel.class, sel.race);
    json(&EquipJson {
        class: out.class,
        race: out.race,
    })
}

#[derive(Serialize)]
struct SpellbookJson {
    /// `nothing` (no class selected), `no_magic` (MME's "No Magic" box) or `open`.
    action: &'static str,
    class: Option<i32>,
    level: Option<i32>,
    magery: Option<i32>,
    magery_level: Option<i32>,
}

/// The Spellbook button: what the spell book form is opened with.
pub async fn spellbook_route(
    State(state): State<Arc<AppState>>,
    Body(sel): Body<Selection>,
) -> ApiResult {
    let loaded = state.current();
    let out = match builder::spellbook(&loaded.world.classes, sel.class) {
        Spellbook::Nothing => SpellbookJson {
            action: "nothing",
            class: None,
            level: None,
            magery: None,
            magery_level: None,
        },
        Spellbook::NoMagic => SpellbookJson {
            action: "no_magic",
            class: None,
            level: None,
            magery: None,
            magery_level: None,
        },
        Spellbook::Open {
            class,
            level,
            magery,
            magery_level,
        } => SpellbookJson {
            action: "open",
            class: Some(class),
            level: Some(level),
            magery: Some(magery),
            magery_level: Some(magery_level),
        },
    };
    json(&out)
}
