//! The Spells explorer's lookups, the Spell Book window and chest contents (card P4-32).
//!
//! The rules are `touchstone_rules::{spell_tools, spell_book, chests}`; this module reads the tables and
//! shapes the JSON. Lists MME shows in its Results window (counters, what casts a spell) come back as rows the
//! page links to the records.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use touchstone_rules::Profile;
use touchstone_rules::chests::{ChestResult, chest_contents, sorted_by_chance};
use touchstone_rules::locations::LocationOptions;
use touchstone_rules::spell_book::{
    BookGlobals, BookQuery, ClassPick, GlobalCopy, copy_global_char, list_spells, pick_class,
    uses_character,
};
use touchstone_rules::spell_cast::UsableEnv;
use touchstone_rules::spell_tools::{
    NegateFrom, NegateKind, WhatCasts, find_negates, negate_query, what_casts,
};
use touchstone_rules::stats::compute_avg_level_max_stats;
use touchstone_vb6::val;

use crate::calc::{ApiResult, Body, json, not_found};
use crate::detail::nmr_ver;
use crate::panes::{RefJson, StoredLairs, references};
use crate::state::AppState;

// ---------- Find counters ----------

#[derive(Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct CountersRequest {
    /// The spell row's number.
    pub spell: i32,
    /// The monster pane's row tag (`number` or `number@level`); present when the menu was opened on a pane.
    pub tag: Option<String>,
    /// What the user typed to "Cast spell at a certain level?" (`Val`); absent before the question was asked.
    pub level_answer: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct CounterRow {
    /// `items` or `spells`.
    pub table: &'static str,
    pub number: i32,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct CountersResponse {
    /// MME asks for a level before it searches: ask, then send the request again with `level_answer`.
    pub ask_level: bool,
    /// `nLevel` the search ran with.
    pub level: i32,
    pub rows: Vec<CounterRow>,
}

pub async fn counters_route(
    State(state): State<Arc<AppState>>,
    Body(req): Body<CountersRequest>,
) -> ApiResult {
    let world = &state.current().world;
    let avg = compute_avg_level_max_stats(Profile::Stock, &world.races).all_stats;
    let from = match &req.tag {
        Some(tag) => NegateFrom::Pane { tag },
        None => NegateFrom::List {
            spell: req.spell,
            answer: req.level_answer,
        },
    };
    let plan = negate_query(&world.spells, from, avg);
    if plan.ask_level {
        return json(&CountersResponse {
            ask_level: true,
            level: 0,
            rows: Vec::new(),
        });
    }
    let q = plan.query;
    let rows = find_negates(&world.items, &world.spells, &q, true)
        .into_iter()
        .map(|r| CounterRow {
            table: match r.kind {
                NegateKind::Item => "items",
                NegateKind::Spell => "spells",
            },
            number: r.number,
            text: r.text,
        })
        .collect();
    json(&CountersResponse {
        ask_level: false,
        level: q.level,
        rows,
    })
}

// ---------- What casts this spell ----------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct CastsQuery {
    pub hide_record_numbers: bool,
}

#[derive(Debug, Serialize)]
pub struct CastsResponse {
    /// `found`, `nothing` (MME's "Nothing." box) or `missing` ("Cannot find Spell.").
    pub status: &'static str,
    /// `Spell <name> is casted by:`.
    pub caption: String,
    pub rows: Vec<RefJson>,
}

pub async fn casts_route(
    State(state): State<Arc<AppState>>,
    Path(number): Path<i32>,
    Query(q): Query<CastsQuery>,
) -> ApiResult {
    let loaded = state.current();
    let world = &loaded.world;
    let mut lairs = StoredLairs(loaded.lair_cache());
    let opt = LocationOptions {
        nmr_ver: nmr_ver(&world.info.nmr_version),
        hide_record_numbers: q.hide_record_numbers,
        global_filter: false,
        charm_tag: String::new(),
        attack_type: 0,
    };
    let (status, caption, rows) = match what_casts(&world.spells, world, &mut lairs, &opt, number) {
        WhatCasts::NoSuchSpell => ("missing", String::new(), Vec::new()),
        WhatCasts::Nothing => ("nothing", String::new(), Vec::new()),
        WhatCasts::Found { caption_name, list } => {
            let (rows, _) = references(list, None);
            ("found", format!("Spell {caption_name} is casted by:"), rows)
        }
    };
    json(&CastsResponse {
        status,
        caption,
        rows,
    })
}

// ---------- chests ----------

#[derive(Debug, Serialize)]
pub struct ChestRowJson {
    pub text: String,
    /// The chance in percent, as sorted.
    pub percent: f64,
    pub item_text: String,
    pub item: i32,
}

#[derive(Debug, Serialize)]
pub struct ChestResponse {
    /// `rows`, or the reason there are none: `no_item`, `not_container`, `no_data`.
    pub status: &'static str,
    /// MME's message box text for the failures.
    pub message: Option<String>,
    pub rows: Vec<ChestRowJson>,
}

pub async fn chest_route(
    State(state): State<Arc<AppState>>,
    Path(number): Path<i32>,
    Query(q): Query<CastsQuery>,
) -> ApiResult {
    let world = &state.current().world;
    let (status, message, rows) = match chest_contents(world, number, q.hide_record_numbers) {
        ChestResult::NoSuchItem(n) => {
            ("no_item", Some(format!("Item #{n} not found.")), Vec::new())
        }
        ChestResult::NotContainer(name) => (
            "not_container",
            Some(format!("{name} is not a container.")),
            Vec::new(),
        ),
        ChestResult::NoChestData => (
            "no_data",
            Some("Failed to find chest data.".to_owned()),
            Vec::new(),
        ),
        ChestResult::Rows(rows) => (
            "rows",
            None,
            sorted_by_chance(rows)
                .into_iter()
                .map(|r| ChestRowJson {
                    text: r.text,
                    percent: r.percent.scaled() as f64 / 10_000.0,
                    item_text: r.item_text,
                    item: r.item,
                })
                .collect(),
        ),
    };
    json(&ChestResponse {
        status,
        message,
        rows,
    })
}

// ---------- the Spell Book ----------

#[derive(Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct BookRequest {
    /// `cmbClass.ItemData` (0 for none).
    pub class: i32,
    /// `txtLevel.Text`.
    pub level: crate::calc::Text,
    /// `cmbAlignment.ListIndex`: 0 any, 1 good, 2 neutral, 3 evil.
    pub alignment: i32,
    /// `cmbSpellMagery.ListIndex` and `cmbSpellMageryLevel.ListIndex`.
    pub magery: i32,
    pub magery_level: i32,
    /// The global filter bar: on, its level box text, the character's class and magery.
    pub filter_on: bool,
    pub global_level: crate::calc::Text,
    pub global_class: i32,
    pub char_magery: i32,
    pub char_magery_level: i32,
    /// `bDisableKaiAutolearn`.
    pub disable_kai_autolearn: bool,
}

#[derive(Debug, Serialize)]
pub struct BookResponse {
    /// The spells to list, in table order; the page asks `/api/evaluate/spells` for their rows.
    pub spells: Vec<i32>,
    /// The rows are built for the loaded character.
    pub uses_character: bool,
    /// A VB6 error ended the listing (a level past 32767).
    pub aborted: bool,
    /// The level column sorts ascending (the global filter is off).
    pub sort_ascending: bool,
}

pub async fn book_route(
    State(state): State<Arc<AppState>>,
    Body(req): Body<BookRequest>,
) -> ApiResult {
    let world = &state.current().world;
    let env = UsableEnv {
        profile: Profile::Stock,
        spells: &world.spells,
        classes: &world.classes,
        nmr_ver: nmr_ver(&world.info.nmr_version),
        only_in_game: true,
        disable_kai_autolearn: req.disable_kai_autolearn || world.info.disable_kai,
    };
    let q = BookQuery {
        class: req.class,
        level: val(&req.level.0),
        alignment: req.alignment,
    };
    let list = list_spells(&env, &q, req.filter_on);
    let globals = BookGlobals {
        filter_on: req.filter_on,
        global_level_set: val(&req.global_level.0) > 0.0,
        global_class: req.global_class,
        char_magery: req.char_magery,
        char_magery_lvl: req.char_magery_level,
    };
    json(&BookResponse {
        uses_character: !list.aborted && uses_character(&globals, &q, req.magery, req.magery_level),
        spells: list.spells,
        aborted: list.aborted,
        sort_ascending: list.sort_ascending,
    })
}

#[derive(Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct ClassBody {
    pub class: i32,
}

#[derive(Debug, Serialize)]
pub struct ClassPickResponse {
    pub magery_level: Option<i32>,
    pub magery: Option<i32>,
}

/// `cmbClass_Click`: the magery boxes a class sets (the boxes hold 4 and 6 entries).
pub async fn class_route(
    State(state): State<Arc<AppState>>,
    Body(req): Body<ClassBody>,
) -> ApiResult {
    let world = &state.current().world;
    match pick_class(&world.classes, req.class, 4, 6) {
        ClassPick::Nothing => json(&ClassPickResponse {
            magery_level: None,
            magery: None,
        }),
        ClassPick::NotFound => Err(not_found("Class not found.")),
        ClassPick::Boxes {
            magery_level,
            magery,
        } => json(&ClassPickResponse {
            magery_level,
            magery,
        }),
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct CopyBody {
    pub filter_on: bool,
    pub alignment: i32,
    pub level_text: crate::calc::Text,
    /// The global bar's class: `cmbGlobalClass(1).ListIndex > 0` and the class number it stands for.
    pub class_index: i32,
    pub class: i32,
}

#[derive(Debug, Serialize)]
pub struct CopyResponse {
    pub alignment: Option<i32>,
    pub level: Option<String>,
    /// The book's class (None leaves its box).
    pub class: Option<i32>,
}

/// `CopyGlobalChar`: what the global filter fills into the window.
pub async fn copy_route(
    State(state): State<Arc<AppState>>,
    Body(req): Body<CopyBody>,
) -> ApiResult {
    let world = &state.current().world;
    // the book's class box: "none", then the classes in table order
    let mut book: Vec<i32> = vec![0];
    book.extend(world.classes.iter().map(|c| c.number));
    let out = copy_global_char(&GlobalCopy {
        filter_on: req.filter_on,
        alignment: req.alignment,
        level_text: &req.level_text.0,
        class_index: req.class_index,
        class_item_data: req.class,
        book_classes: &book,
    });
    json(&CopyResponse {
        alignment: out.alignment,
        level: out.level,
        class: out.class_index.map(|i| book[i]),
    })
}
