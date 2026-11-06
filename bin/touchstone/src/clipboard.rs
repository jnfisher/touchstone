//! `POST /api/clipboard/{character,monster,shop,lines}`: the texts MME's menus and the Char tab copy
//! (card P4-15). The builders are `touchstone_rules::clipboard`; this module gathers what MME's
//! controls would hold and shapes the answer: `{text, error}`. `text` is what the clipboard should now
//! hold (`null`: MME copies nothing) and `error` names the VB6 error that stopped the procedure.
//! The page does the actual copying.

use std::sync::Arc;

use axum::extract::State;
use serde::{Deserialize, Serialize};
use touchstone_model::World;
use touchstone_rules::Profile;
use touchstone_rules::clipboard::{
    self as clip, CharClip, Clip, EQUIP_SLOTS, LinesClip, LvColumn, LvRow, ShopClip, fix_tag,
};
use touchstone_rules::detail::shop::{ShopDetailOptions, ShopLines, pull_shop_detail};
use touchstone_rules::exp::calc_exp_needed_by_race_class;
use touchstone_rules::locations::LocationOptions;
use touchstone_rules::rows::add_shop_row;
use touchstone_vb6::cstr;

use crate::calc::{ApiResult, Body, bad, json, vb_error_text};
use crate::character::{CharacterDto, refresh};
use crate::panes::references;
use crate::state::AppState;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ClipJson {
    pub text: Option<String>,
    pub error: Option<String>,
}

impl From<Clip> for ClipJson {
    fn from(c: Clip) -> Self {
        Self {
            text: c.text,
            error: c.error.map(|n| match n {
                clip::ERR_INVALID_CALL => {
                    "Invalid procedure call or argument (run-time error 5)".to_owned()
                }
                clip::ERR_INDEX_OUT_OF_BOUNDS => {
                    "Index out of bounds (run-time error 35600)".to_owned()
                }
                n => vb_error_text(u32::try_from(n).unwrap_or(0)),
            }),
        }
    }
}

// ---------- character ----------

#[derive(Debug, Deserialize)]
pub struct CharacterRequest {
    pub character: CharacterDto,
    /// `bStatsOnly`: the stat block without the equipment.
    #[serde(default)]
    pub stats_only: bool,
}

/// `CopyChartoClip` for the evaluated character.
pub fn character_clip(
    world: &World,
    req: &CharacterRequest,
) -> Result<ClipJson, (axum::http::StatusCode, String)> {
    let (ch, st) = refresh(world, &req.character)?;
    let (Some(class), Some(race)) = (ch.class, ch.race) else {
        return Err(bad("pick a class and race"));
    };
    let exp = calc_exp_needed_by_race_class(
        Profile::Stock,
        ch.level,
        class,
        race,
        &world.classes,
        &world.races,
    )
    .map_err(|e| bad(e.to_string()))?;
    let s = &st.stats;
    let setup = st.class_setup.as_ref();
    let mut c = CharClip {
        name: req.character.name.clone(),
        cp_tag: st
            .cps
            .summary
            .as_ref()
            .map_or_else(String::new, |x| x.avail.to_string()),
        race: world
            .get_race(race)
            .map_or_else(String::new, |r| r.name.clone()),
        class: world
            .get_class(class)
            .map_or_else(String::new, |r| r.name.clone()),
        level: ch.level.to_string(),
        exp,
        max_hp_tag: st
            .hit_points
            .range
            .as_ref()
            .map_or_else(String::new, |r| cstr(r.average)),
        stats: (0..47).map(|i| s.caption(i)).collect(),
        enc_tag: st
            .encum_level
            .as_ref()
            .map_or_else(|_| String::new(), |e| e.tag.clone()),
        ..CharClip::default()
    };
    c.ac_tag = fix_tag(&c.stats[2]);
    c.dr_tag = fix_tag(&c.stats[3]);
    if setup.is_some_and(|x| x.has_magic) {
        let m = st.magic.as_ref();
        c.mana = Some((
            m.and_then(|m| m.max_mana)
                .map_or_else(String::new, |v| v.to_string()),
            m.and_then(|m| m.spellcasting)
                .map_or_else(String::new, |v| v.to_string()),
        ));
    }
    if setup.is_some_and(|x| x.has_picks) {
        c.picklocks = Some(
            st.picklocks
                .total
                .map_or_else(String::new, |v| v.to_string()),
        );
    }
    for i in 0..6 {
        c.stat_tags[i] = cstr(s.stat_with_bonus[i]);
        c.stat_bonus[i] = s.label_bonus[i];
    }
    for (slot, &n) in ch.equipped.iter().enumerate().take(EQUIP_SLOTS) {
        if n > 0
            && let Some(it) = world.get_item(n)
        {
            // the equipment combo's text
            c.equip[slot] = Some(format!("{} ({})", it.name, it.number));
        }
    }
    Ok(clip::char_to_clip(&c, req.stats_only).into())
}

pub async fn character_route(
    State(state): State<Arc<AppState>>,
    Body(req): Body<CharacterRequest>,
) -> ApiResult {
    let loaded = state.current();
    json(&character_clip(&loaded.world, &req)?)
}

// ---------- rows the page holds ----------

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct RowIn {
    pub text: String,
    pub subs: Vec<String>,
    pub selected: bool,
    pub detail: Option<String>,
    pub refs: Option<Vec<RowIn>>,
}

impl From<RowIn> for LvRow {
    fn from(r: RowIn) -> Self {
        Self {
            text: r.text,
            subs: r.subs,
            selected: r.selected,
            detail: r.detail,
            refs: r.refs.map(|v| v.into_iter().map(Into::into).collect()),
        }
    }
}

// ---------- monster ----------

#[derive(Debug, Deserialize)]
pub struct MonsterRequest {
    /// The monster pane's lines: label and cells.
    pub lines: Vec<RowIn>,
    #[serde(default)]
    pub name_only: bool,
}

pub async fn monster_route(Body(req): Body<MonsterRequest>) -> ApiResult {
    let lines: Vec<LvRow> = req.lines.into_iter().map(Into::into).collect();
    json(&ClipJson::from(clip::monster_to_clip(
        &lines,
        req.name_only,
    )))
}

// ---------- shop ----------

#[derive(Debug, Deserialize)]
pub struct ShopRequest {
    pub number: i32,
    /// `nCharm` and `bShowSell` as the Shops tab's boxes hold them.
    #[serde(default)]
    pub charm: i16,
    #[serde(default)]
    pub show_sell: bool,
    #[serde(default)]
    pub name_only: bool,
}

/// `CopyShopToClipboard` for a shop: its list row, the detail text, assigned rooms and stock or training table.
pub fn shop_clip(
    state: &AppState,
    req: &ShopRequest,
) -> Result<ClipJson, (axum::http::StatusCode, String)> {
    let loaded = state.current();
    let world = &loaded.world;
    let shop = world
        .get_shop(req.number)
        .ok_or_else(|| bad(format!("no shop {}", req.number)))?;
    let opts = ShopDetailOptions {
        startup: false,
        charm: req.charm,
        show_sell: req.show_sell,
        locations: LocationOptions {
            nmr_ver: 0.0,
            hide_record_numbers: false,
            global_filter: false,
            charm_tag: req.charm.to_string(),
            attack_type: 0,
        },
    };
    let mut lairs = crate::panes::StoredLairs(loaded.lair_cache());
    let d = pull_shop_detail(Profile::Stock, world, &mut lairs, req.number, &opts);
    let (refs, _) = references(d.assigned, None);
    // lvShops holds only shops AddShop2LV lists; any other is still copied from its own fields
    let row = add_shop_row(shop);
    let selected = row.map(|r| LvRow {
        text: r.text,
        subs: r.cells.into_iter().map(|c| c.text).collect(),
        selected: true,
        ..LvRow::default()
    });
    let (detail_columns, detail_rows): (Vec<&str>, Vec<LvRow>) = match &d.lines {
        ShopLines::Training(rows) => (
            vec!["LVL", "Cost"],
            rows.iter()
                .map(|r| LvRow {
                    text: r.level.clone(),
                    subs: vec![r.cost.clone()],
                    ..LvRow::default()
                })
                .collect(),
        ),
        ShopLines::Stock(rows) => (
            vec!["#", "Name", "Max", "Regen", "Cost"],
            rows.iter()
                .map(|r| LvRow {
                    text: r.number.clone(),
                    subs: vec![
                        r.name.clone(),
                        r.max.clone(),
                        r.regen.clone(),
                        r.cost.clone(),
                    ],
                    ..LvRow::default()
                })
                .collect(),
        ),
        _ => (Vec::new(), Vec::new()),
    };
    let c = ShopClip {
        selected,
        shop_columns: vec!["#".into(), "Name".into(), "Type".into()],
        detail: d.text,
        loc_header: "Assigned To".into(),
        locs: refs.into_iter().map(|r| r.text).collect(),
        detail_columns: detail_columns.into_iter().map(String::from).collect(),
        detail_rows,
    };
    Ok(clip::shop_to_clip(&c, req.name_only).into())
}

pub async fn shop_route(
    State(state): State<Arc<AppState>>,
    Body(req): Body<ShopRequest>,
) -> ApiResult {
    json(&shop_clip(&state, &req)?)
}

// ---------- list lines ----------

#[derive(Debug, Deserialize)]
pub struct ColumnIn {
    pub text: String,
    #[serde(default = "wide")]
    pub width: i32,
}

fn wide() -> i32 {
    1000
}

#[derive(Debug, Deserialize)]
pub struct LinesRequest {
    /// MME's list name (`lvWeapons`, `lvSpellLoc`, ...): it decides how name-only copies read the cells.
    pub list: String,
    pub columns: Vec<ColumnIn>,
    pub rows: Vec<RowIn>,
    pub detail: Option<String>,
    pub locations: Option<Vec<RowIn>>,
    #[serde(default = "no_column")]
    pub exclude_column: i32,
    #[serde(default)]
    pub name_only: bool,
}

fn no_column() -> i32 {
    -1
}

pub async fn lines_route(Body(req): Body<LinesRequest>) -> ApiResult {
    let c = LinesClip {
        list: req.list,
        columns: req
            .columns
            .into_iter()
            .map(|c| LvColumn {
                text: c.text,
                width: c.width,
            })
            .collect(),
        rows: req.rows.into_iter().map(Into::into).collect(),
        detail: req.detail,
        locations: req
            .locations
            .map(|v| v.into_iter().map(Into::into).collect()),
        exclude_column: req.exclude_column,
    };
    json(&ClipJson::from(clip::lines_to_clip(&c, req.name_only)))
}
