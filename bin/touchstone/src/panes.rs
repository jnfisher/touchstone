//! `GET/POST /api/detail/{items|spells|shops|classes|races|monsters}/{n}`: what MME shows in the detail box
//! and reference list when an explorer row is clicked. Cards P4-DP and P4-08 (monsters).
//!
//! The text and rows come from `touchstone_rules::detail`; this module resolves the character and
//! global filter as `/api/evaluate` does, fills the frmMain controls those procedures read, puts
//! the reference rows in the order MME's ListView sorts leave them, and shapes the JSON.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use touchstone_model::World;
use touchstone_rules::Profile;
use touchstone_rules::ability_text::AbilityEnv;
use touchstone_rules::character::set_char_defense_description;
use touchstone_rules::detail::class_race::{pull_class_detail, pull_race_detail};
use touchstone_rules::detail::item::{
    DetailKind as ItemKind, ItemDetailEnv, ItemDetailOptions, pull_item_detail,
};
use touchstone_rules::detail::monster::{
    DamageCaches, MonsterDetailInputs, MonsterRow, pull_monster_detail,
};
use touchstone_rules::detail::shop::{ShopDetailOptions, pull_shop_detail};
use touchstone_rules::detail::spell::{SpellDetailOptions, pull_spell_detail};
use touchstone_rules::detail::{DetailKind, DetailRow};
use touchstone_rules::exp_per_hour::ExpPerHourSettings;
use touchstone_rules::lairs::{LairCache, LairInfo};
use touchstone_rules::locations::{
    LocationKind, LocationLairs, LocationList, LocationOptions, LocationRow,
};
use touchstone_vb6::val;

use crate::calc::{ApiResult, Body, bad, json, not_found};
use crate::character::CharacterDto;
use crate::detail::WorldNames;
use crate::evaluate::{
    Ctx, EvaluateBody, GlobalFilterBody, Options, prepare, run_settings, with_monster_env,
};
use crate::state::AppState;

/// The panes this endpoint serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pane {
    Items,
    Spells,
    Shops,
    Classes,
    Races,
    Monsters,
}

impl Pane {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "items" => Self::Items,
            "spells" => Self::Spells,
            "shops" => Self::Shops,
            "classes" => Self::Classes,
            "races" => Self::Races,
            "monsters" => Self::Monsters,
            _ => return None,
        })
    }
}

// ---------- request ----------

/// The request: the same character, global filter and view options as `/api/evaluate`, plus the
/// frmMain controls only the detail procedures read. Every field is optional; the GET form is the
/// empty request.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct DetailBody {
    pub character: Option<CharacterDto>,
    pub global_filter: Option<GlobalFilterBody>,
    pub options: Options,
    /// Shops: `nCharm` as the shop pane passes it. Absent: the loaded character's charm while
    /// "show charm" is on, else 0.
    pub charm: Option<i32>,
    /// Shops: `chkShopShowCharm(0)`, price with the character's charm.
    pub show_charm: bool,
    /// Shops: `bShowSell` (`chkShopShowCharm(1)`), what the shop pays.
    pub show_sell: bool,
    /// Items: `mnuShopsFirst.Checked`.
    pub shops_first: bool,
    /// `bHideRecordNumbers`.
    pub hide_record_numbers: bool,
    /// Items: the optional `nAttackTypeMUD` argument (the list row's tag); 0 is omitted.
    pub attack_type: i32,
    /// Items: the optional `bFullDetails` argument.
    pub full_details: bool,
    /// Monsters: `nLookupLimit`, how many rooms and lairs "Spawns via ..." lists before it stops with a
    /// "... plus n more" line. MME starts with 100.
    pub lookup_limit: Option<i32>,
    /// Monsters: `frmMain.bAutoCalcMonDamage`, simulate the monster's damage on the character on the spot.
    pub auto_calc_damage: bool,
    /// Monsters: `frmMain.bDontLookupMonRegen`, leave the "Spawns via ..." lookup out.
    pub dont_lookup_regen: bool,
    /// Monsters: `bMobPrintCharDamageOutFirst`, the character's damage before the monster's.
    pub print_char_damage_out_first: bool,
}

// ---------- response ----------

#[derive(Debug, Serialize, PartialEq)]
pub struct RowJson {
    pub label: String,
    pub value: String,
    pub kind: &'static str,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct CellJson {
    pub text: String,
    pub tag: Option<String>,
}

/// The record a reference row stands for, so the page can link to it.
#[derive(Debug, Serialize, PartialEq)]
pub struct Target {
    /// `items`, `monsters`, `spells`, `shops` or `rooms`.
    pub table: &'static str,
    /// The record number; 0 for a room, which `key` ("map/room") names.
    pub number: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct RefJson {
    pub kind: &'static str,
    /// The ListItem's text.
    pub text: String,
    pub tag: Option<String>,
    /// The sub-items.
    pub cells: Vec<CellJson>,
    pub target: Option<Target>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct CompareJson {
    /// "Compared to <equipped item>".
    pub label: String,
    pub text: String,
}

/// A sub-item of a monster pane line (MME's `ListSubItems`).
#[derive(Debug, Serialize, PartialEq)]
pub struct LineCellJson {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// The VB `ForeColor` MME gave the cell.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<i32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
}

/// A line of the monster pane: MME's `lvMonsterDetail` item with its label, colour, bold flag and cells.
#[derive(Debug, Serialize, PartialEq)]
pub struct LineJson {
    pub label: String,
    /// The ListItem's `Tag`, lower case (`item`, `spell`, `monster`, `room`, `greet_text`, ...); `plain` where MME gave none.
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<i32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    pub cells: Vec<LineCellJson>,
    /// The record the line stands for, so the page can link to it.
    pub target: Option<Target>,
}

#[derive(Debug, Serialize)]
struct PaneJson {
    kind: &'static str,
    number: i32,
    /// `DetailTB.Text`, exactly.
    text: String,
    rows: Vec<RowJson>,
    /// The reference list, in the order MME's ListView holds it after its sorts.
    references: Vec<RefJson>,
    /// Monsters: the pane's list, line by line (the other panes use `rows` and `references`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    lines: Vec<LineJson>,
    /// Items: the "Compared to ..." blocks, apart from `rows` so a page can offer them as a toggle.
    compare: Vec<CompareJson>,
    /// Shops: the charm the prices use; the label MME shows beside it.
    #[serde(skip_serializing_if = "Option::is_none")]
    charm: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    charm_label: Option<String>,
    /// VB6 error numbers that reached the procedures' handlers.
    errors: Vec<i32>,
}

fn kind_name(k: DetailKind) -> &'static str {
    match k {
        DetailKind::Ability => "ability",
        DetailKind::ShopLine => "shop_line",
        DetailKind::Fact => "fact",
        DetailKind::Cost => "cost",
        DetailKind::Reference => "reference",
        DetailKind::Message => "message",
    }
}

fn item_kind_name(k: ItemKind) -> &'static str {
    match k {
        ItemKind::Summary => "summary",
        ItemKind::WeaponDamage => "weapon_damage",
        ItemKind::StrengthNotice => "notice",
        ItemKind::Flags => "flags",
        ItemKind::Uses => "uses",
        ItemKind::Abilities => "abilities",
        ItemKind::Casts => "casts",
        ItemKind::ClassOk => "class_ok",
        ItemKind::Classes => "classes",
        ItemKind::Races => "races",
        ItemKind::Negates => "negates",
        ItemKind::Compared => "compared",
    }
}

fn row_json(r: &DetailRow) -> RowJson {
    RowJson {
        label: r.label.clone(),
        value: r.value.clone(),
        kind: kind_name(r.kind),
    }
}

fn location_kind_name(k: LocationKind) -> &'static str {
    match k {
        LocationKind::Room => "room",
        LocationKind::Monster => "monster",
        LocationKind::Textblock => "textblock",
        LocationKind::Item => "item",
        LocationKind::Spell => "spell",
        LocationKind::Lair => "lair",
        LocationKind::Group => "group",
        LocationKind::Npc => "npc",
        LocationKind::RoomCmd => "room_cmd",
        LocationKind::More => "more",
        LocationKind::NoLimit => "no_limit",
    }
}

// ---------- reference order ----------

/// `SortListView(lv, 1, ldtstring, True)` as the ListView control does it: by the first column's
/// text, ignoring case. Rows with equal text keep their order.
fn string_key(r: &LocationRow) -> String {
    r.text.to_lowercase()
}

/// Apply every `SortListView` request `GetLocations` made, in order: each sorts the rows the list
/// held at that moment (`n`), later rows are not there yet.
fn apply_sorts(rows: &mut [LocationRow], sorts: &[usize]) {
    for &n in sorts {
        let n = n.min(rows.len());
        rows[..n].sort_by_cached_key(string_key);
    }
}

/// `LV_RowIsShop`: the first sub-item's text (the row's own when that is blank) starts with "shop".
fn is_shop_row(r: &LocationRow) -> bool {
    let first = r.cells.first().map_or("", |c| c.text.as_str());
    let s = if first.trim().is_empty() {
        r.text.as_str()
    } else {
        first
    };
    s.trim_start()
        .chars()
        .take(4)
        .collect::<String>()
        .eq_ignore_ascii_case("shop")
}

/// `LV_RefreshSort(lv, 1, ldtnumber, True, False)` or, with the Shops First menu, the
/// `LV_RefreshSort_ShopsFirst` variant: by the ListItem's tag as a number, highest first, shop rows
/// ahead of the others in the second. Equal keys keep their order.
fn refresh_sort(rows: &mut [LocationRow], shops_first: bool) {
    let key = |r: &LocationRow| val(r.tag.as_deref().unwrap_or(""));
    rows.sort_by(|a, b| {
        let by_value = key(b).total_cmp(&key(a));
        if shops_first {
            is_shop_row(b).cmp(&is_shop_row(a)).then(by_value)
        } else {
            by_value
        }
    });
}

// ---------- reference rows ----------

/// The record a row stands for. A percent-column or two-column row keeps the number in its first
/// sub-item's tag; a single-column row in its own tag.
fn target_of(r: &LocationRow) -> Option<Target> {
    let tag = r
        .cells
        .first()
        .and_then(|c| c.tag.as_deref())
        .or(r.tag.as_deref())?;
    let number = || tag.trim().parse::<i32>().ok().filter(|n| *n > 0);
    let table = |table: &'static str| {
        number().map(|number| Target {
            table,
            number,
            key: None,
        })
    };
    match r.kind {
        LocationKind::Monster | LocationKind::Npc => table("monsters"),
        LocationKind::Item => table("items"),
        LocationKind::Spell => table("spells"),
        LocationKind::Room => {
            // A shop's rooms are rows of kind Room headed "Shop"; their tag is the shop.
            if is_shop_row(r) {
                table("shops")
            } else if tag.contains('/') {
                Some(Target {
                    table: "rooms",
                    number: 0,
                    key: Some(tag.to_owned()),
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

fn ref_json(r: &LocationRow) -> RefJson {
    RefJson {
        kind: location_kind_name(r.kind),
        text: r.text.clone(),
        tag: r.tag.clone(),
        cells: r
            .cells
            .iter()
            .map(|c| CellJson {
                text: c.text.clone(),
                tag: c.tag.clone(),
            })
            .collect(),
        target: target_of(r),
    }
}

fn references(list: LocationList, refresh: Option<bool>) -> (Vec<RefJson>, Vec<i32>) {
    let LocationList {
        mut rows,
        sorts,
        errors,
    } = list;
    apply_sorts(&mut rows, &sorts);
    if let Some(shops_first) = refresh {
        refresh_sort(&mut rows, shops_first);
    }
    (rows.iter().map(ref_json).collect(), errors)
}

/// `GetLairInfo` while no monster calculation has run: the lair cache as loaded.
struct StoredLairs<'a>(&'a LairCache);

impl LocationLairs for StoredLairs<'_> {
    fn get_lair_info(&mut self, group_index: &str, max_regen: i16) -> LairInfo {
        self.0.get_lair_info_startup(group_index, max_regen)
    }
}

// ---------- the panes ----------

struct Shared<'a> {
    ctx: &'a Ctx<'a>,
    body: &'a DetailBody,
    charm_tag: String,
    strength_tag: String,
    attack_type_mme: i32,
}

impl Shared<'_> {
    fn location_options(&self, charm_tag: &str) -> LocationOptions {
        LocationOptions {
            nmr_ver: self.ctx.nmr,
            hide_record_numbers: self.body.hide_record_numbers,
            global_filter: self.ctx.state.is_some(),
            charm_tag: charm_tag.to_owned(),
            attack_type: self.attack_type_mme,
        }
    }
}

fn empty(kind: &'static str, number: i32) -> PaneJson {
    PaneJson {
        kind,
        number,
        text: String::new(),
        rows: Vec::new(),
        references: Vec::new(),
        lines: Vec::new(),
        compare: Vec::new(),
        charm: None,
        charm_label: None,
        errors: Vec::new(),
    }
}

fn item_pane(world: &World, lairs: &mut StoredLairs, sh: &Shared, number: i32) -> PaneJson {
    let o = &sh.body.options;
    let ctx = sh.ctx;
    // The same combo reading as the weapon list: normal unless combat is calculated.
    let attack = if o.calc_combat {
        o.attack.unwrap_or(5)
    } else {
        5
    };
    let options = ItemDetailOptions {
        global_filter: ctx.state.is_some(),
        calc_combat: o.calc_combat,
        slow_85: o.slow_85,
        combo_mode: Some(attack),
        extras: [o.vs_ac.0.clone(), o.vs_dr.0.clone(), o.vs_dodge.0.clone()],
        shops_first: sh.body.shops_first,
        equipped: ctx.equipped_items,
        nmr_ver: ctx.nmr,
        hide_record_numbers: sh.body.hide_record_numbers,
        quick_spell: false,
        global_level_text: ctx.level_text.clone(),
        avg_level_max_all_stats: ctx.avg,
        dat_ver_caption: world.info.dat_file_version.clone(),
        accuracy_tag: ctx.accuracy_tag.clone(),
        charm_tag: sh.charm_tag.clone(),
        strength_tag: sh.strength_tag.clone(),
        attack_type_mme: sh.attack_type_mme,
    };
    let names = WorldNames(world);
    let env = ItemDetailEnv {
        profile: Profile::Stock,
        world,
        names: &names,
        textblocks: world,
        locations: world,
        options: &options,
    };
    let d = pull_item_detail(
        &env,
        lairs,
        number,
        sh.body.attack_type,
        sh.body.full_details,
        ctx.state.as_ref(),
    );
    let mut out = empty("items", number);
    out.text = d.text;
    for r in &d.rows {
        if r.kind == ItemKind::Compared {
            out.compare.push(CompareJson {
                label: r.label.clone(),
                text: r.value.clone(),
            });
        } else {
            out.rows.push(RowJson {
                label: r.label.clone(),
                value: r.value.clone(),
                kind: item_kind_name(r.kind),
            });
        }
    }
    let mut errors = d.errors;
    let (refs, more) = references(d.locations, d.sort.map(|s| s.shops_first));
    out.references = refs;
    errors.extend(more);
    out.errors = errors;
    out
}

fn spell_pane(world: &World, lairs: &mut StoredLairs, sh: &Shared, number: i32) -> PaneJson {
    let o = &sh.body.options;
    let ctx = sh.ctx;
    let level_box = o.level.map_or_else(|| val(&ctx.level_text), f64::from);
    let opts = SpellDetailOptions {
        startup: false,
        combat_option: o.calc_combat,
        combat_value: val(&o.vs_mr.0),
        anti_magic: o.anti_magic,
        level_box,
        nmr_ver: ctx.nmr,
        only_in_game: true,
        disable_kai_autolearn: world.info.disable_kai,
        hide_record_numbers: sh.body.hide_record_numbers,
        avg_level_max_all_stats: ctx.avg,
        locations: sh.location_options(&sh.charm_tag),
    };
    let d = pull_spell_detail(
        Profile::Stock,
        world,
        lairs,
        ctx.state.as_ref(),
        number,
        &opts,
    );
    let mut out = empty("spells", number);
    out.text = d.text;
    out.rows = d.rows.iter().map(row_json).collect();
    let (refs, errors) = references(d.locations, None);
    out.references = refs;
    out.errors = errors;
    out
}

fn shop_pane(world: &World, lairs: &mut StoredLairs, sh: &Shared, number: i32) -> PaneJson {
    let body = sh.body;
    let charm = body.charm.unwrap_or_else(|| {
        if body.show_charm || body.show_sell {
            touchstone_vb6::cint(val(&sh.charm_tag)).map_or(0, i32::from)
        } else {
            0
        }
    });
    // `nCharm` is an Integer; the client's charm box holds three digits.
    let charm16 = i16::try_from(charm).unwrap_or(0);
    let opts = ShopDetailOptions {
        startup: false,
        charm: charm16,
        show_sell: body.show_sell,
        locations: sh.location_options(&sh.charm_tag),
    };
    let d = pull_shop_detail(Profile::Stock, world, lairs, number, &opts);
    let mut out = empty("shops", number);
    out.rows = d.detail_rows().iter().map(row_json).collect();
    out.text = d.text;
    out.charm = Some(i32::from(charm16));
    out.charm_label = Some(d.charm_label);
    let (refs, errors) = references(d.assigned, None);
    out.references = refs;
    out.errors = errors;
    out
}

fn class_race_pane(world: &World, ctx: &Ctx, pane: Pane, number: i32) -> PaneJson {
    let names = WorldNames(world);
    let env = AbilityEnv {
        profile: Profile::Stock,
        names: &names,
        textblocks: world,
        spells: &world.spells,
        hide_record_numbers: false,
        quick_spell: false,
        global_level_text: None,
        avg_level_max_all_stats: ctx.avg,
        nest: std::cell::Cell::new(0),
    };
    let (kind, d) = if pane == Pane::Classes {
        ("classes", pull_class_detail(&env, world, number, false))
    } else {
        ("races", pull_race_detail(&env, world, number, false))
    };
    let mut out = empty(kind, number);
    out.text = d.text;
    out.rows = d.rows.iter().map(row_json).collect();
    out
}

// ---------- monsters ----------

/// The record a monster pane line stands for: the line's tag names the table, the first cell's tag the record.
fn line_target(world: &World, row: &MonsterRow) -> Option<Target> {
    let kind = row.tag.as_deref()?.to_ascii_lowercase();
    let tag = row.cells.first()?.tag.as_deref()?;
    // a spell cell's tag can carry the level after an `@`
    let number = || {
        let digits: String = tag
            .trim()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        digits.parse::<i32>().ok().filter(|n| *n > 0)
    };
    let table = |table: &'static str| {
        number().map(|number| Target {
            table,
            number,
            key: None,
        })
    };
    match kind.as_str() {
        "item" => table("items"),
        "spell" => table("spells"),
        "monster" => table("monsters").filter(|t| world.get_monster(t.number).is_some()),
        "room" if tag.contains('/') => Some(Target {
            table: "rooms",
            number: 0,
            key: Some(tag.trim().to_owned()),
        }),
        "room"
            if row
                .label
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("shop") =>
        {
            table("shops")
        }
        _ => None,
    }
}

fn line_json(world: &World, r: &MonsterRow) -> LineJson {
    LineJson {
        label: r.label.clone(),
        kind: r
            .tag
            .as_deref()
            .map_or_else(|| "plain".to_owned(), str::to_ascii_lowercase),
        color: r.fore_color,
        bold: r.bold,
        cells: r
            .cells
            .iter()
            .map(|c| LineCellJson {
                text: c.text.clone(),
                tag: c.tag.clone(),
                color: c.fore_color,
                bold: c.bold,
            })
            .collect(),
        target: line_target(world, r),
    }
}

/// `PullMonsterDetail` for the current character and the lair/party boxes. The damage arrays are the ones MME
/// starts with (nothing calculated), rebuilt per request: no endpoint calculates monster damage, so nothing could
/// have written them.
fn monster_pane(
    world: &World,
    sh: &Shared,
    number: i32,
    exp: &ExpPerHourSettings,
    sim_rounds: i32,
    loaded_reach: Option<std::sync::Arc<touchstone_rules::reachability::Reach>>,
    waypoint_interval: i32,
) -> Result<PaneJson, (StatusCode, String)> {
    let ctx = sh.ctx;
    let controls = &sh.body.options.monsters;
    let mut ab = AbilityEnv::for_world(world, Profile::Stock);
    ab.hide_record_numbers = sh.body.hide_record_numbers;
    ab.avg_level_max_all_stats = ctx.avg;
    if ctx.state.is_some() {
        ab.global_level_text = Some(&ctx.level_text);
    }
    let computed = loaded_reach;
    let names = |m: i32, r: i32| {
        world
            .get_room(m, r)
            .map(|x| x.name.clone().unwrap_or_default())
    };
    let pane_reach = touchstone_rules::reachability::PaneReach {
        reach: computed.as_deref(),
        opts: touchstone_rules::reachability::ReachRowOptions {
            level: if ctx.filter_on {
                ctx.who.level
            } else {
                touchstone_vb6::val(&ctx.level_text) as i32
            },
            hide_record_numbers: sh.body.hide_record_numbers,
            waypoint_interval,
        },
        names: &names,
    };
    let rows = with_monster_env(ctx, controls, exp, sim_rounds, &ab, |env, s| {
        let defense_desc = set_char_defense_description(s.cctx, &s.state.character, "")
            .map_or_else(String::new, |d| d.description);
        let sim_chr = s.state.sim_inputs();
        let charm = touchstone_vb6::cstr(s.state.stats.stat_with_bonus[5]);
        let inputs = MonsterDetailInputs {
            use_exp_multi: true,
            auto_calc_mon_damage: sh.body.auto_calc_damage,
            dont_lookup_mon_regen: sh.body.dont_lookup_regen,
            print_char_damage_out_first: sh.body.print_char_damage_out_first,
            hide_record_numbers: sh.body.hide_record_numbers,
            sim_rounds: s.sim_rounds,
            settings: s.settings,
            possy: &s.possy,
            spawn_chance: &s.spawn_chance,
            vs_char_defense_config: "",
            char_defense_description: &defense_desc,
            party_damage_calculated: false,
            dont_prompt_calc_party: false,
            bless_caption: s.state.bless_mana.unwrap_or(0.0),
            charm_tag: &charm,
            sim_chr: &sim_chr,
            reach: &pane_reach,
        };
        let mut caches = DamageCaches {
            vs_party: env.callouts.vs_party.to_vec(),
            vs_char: env.callouts.vs_char.to_vec(),
        };
        let limit = sh
            .body
            .lookup_limit
            .and_then(|l| i16::try_from(l).ok())
            .unwrap_or(100);
        pull_monster_detail(env, &ab, &inputs, &mut caches, number, limit)
    })?;
    let mut out = empty("monsters", number);
    out.lines = rows.iter().map(|r| line_json(world, r)).collect();
    Ok(out)
}

fn build(
    loaded: &crate::state::Loaded,
    pane: Pane,
    number: i32,
    body: DetailBody,
    exp: &ExpPerHourSettings,
    sim_rounds: i32,
    waypoint_interval: i32,
) -> Result<PaneJson, (StatusCode, String)> {
    let world = &loaded.world;
    let eval = EvaluateBody {
        character: body.character.clone(),
        global_filter: body.global_filter.clone(),
        options: body.options.clone(),
    };
    let ctx = prepare(world, &eval)?;
    let (charm_tag, strength_tag, attack_type_mme) = match &ctx.state {
        Some(st) => (
            touchstone_vb6::cstr(st.stats.stat_with_bonus[5]),
            touchstone_vb6::cstr(st.stats.stat_with_bonus[0]),
            st.character.attack.attack_type as i32,
        ),
        // The boxes MME starts with.
        None => ("0".to_owned(), "0".to_owned(), 0),
    };
    let sh = Shared {
        ctx: &ctx,
        body: &body,
        charm_tag,
        strength_tag,
        attack_type_mme,
    };
    let mut lairs = StoredLairs(loaded.lair_cache());
    let missing = |what: &str| not_found(format!("no {what} {number}"));
    Ok(match pane {
        Pane::Items => {
            world.get_item(number).ok_or_else(|| missing("item"))?;
            item_pane(world, &mut lairs, &sh, number)
        }
        Pane::Spells => {
            world.get_spell(number).ok_or_else(|| missing("spell"))?;
            spell_pane(world, &mut lairs, &sh, number)
        }
        Pane::Shops => {
            world.get_shop(number).ok_or_else(|| missing("shop"))?;
            shop_pane(world, &mut lairs, &sh, number)
        }
        Pane::Classes => {
            world.get_class(number).ok_or_else(|| missing("class"))?;
            class_race_pane(world, &ctx, pane, number)
        }
        Pane::Races => {
            world.get_race(number).ok_or_else(|| missing("race"))?;
            class_race_pane(world, &ctx, pane, number)
        }
        Pane::Monsters => {
            world
                .get_monster(number)
                .ok_or_else(|| missing("monster"))?;
            monster_pane(
                world,
                &sh,
                number,
                exp,
                sim_rounds,
                loaded.reach.get(),
                waypoint_interval,
            )?
        }
    })
}

async fn run(state: Arc<AppState>, kind: String, number: i32, body: DetailBody) -> ApiResult {
    let Some(pane) = Pane::parse(&kind) else {
        return Err(not_found(format!("no detail pane called {kind:?}")));
    };
    let loaded = state.current();
    let (exp, sim_rounds) = run_settings(&state, body.character.as_ref());
    let waypoint_interval = state.settings().map_or(25, |s| s.route_waypoint_interval);
    // The lair cache is built on first use and the item pane can be long: keep it off the reactor.
    let out = tokio::task::spawn_blocking(move || {
        build(
            &loaded,
            pane,
            number,
            body,
            &exp,
            sim_rounds,
            waypoint_interval,
        )
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))??;
    json(&out)
}

pub async fn get_route(
    State(state): State<Arc<AppState>>,
    Path((kind, number)): Path<(String, i32)>,
) -> ApiResult {
    run(state, kind, number, DetailBody::default()).await
}

pub async fn post_route(
    State(state): State<Arc<AppState>>,
    Path((kind, number)): Path<(String, i32)>,
    Body(body): Body<DetailBody>,
) -> ApiResult {
    if body
        .charm
        .is_some_and(|c| !(0..=i32::from(i16::MAX)).contains(&c))
    {
        return Err(bad("charm must be between 0 and 32767"));
    }
    if body
        .lookup_limit
        .is_some_and(|c| !(0..=i32::from(i16::MAX)).contains(&c))
    {
        return Err(bad("lookup_limit must be between 0 and 32767"));
    }
    run(state, kind, number, body).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use touchstone_rules::locations::LocationCell;

    fn row(
        kind: LocationKind,
        text: &str,
        tag: Option<&str>,
        cell: Option<(&str, &str)>,
    ) -> LocationRow {
        LocationRow {
            kind,
            text: text.to_owned(),
            tag: tag.map(str::to_owned),
            cells: cell
                .map(|(t, g)| {
                    vec![LocationCell {
                        text: t.to_owned(),
                        tag: Some(g.to_owned()),
                    }]
                })
                .unwrap_or_default(),
        }
    }

    fn texts(rows: &[LocationRow]) -> Vec<&str> {
        rows.iter().map(|r| r.text.as_str()).collect()
    }

    #[test]
    fn sorts_apply_to_the_rows_held_at_the_time() {
        let mut rows = vec![
            row(LocationKind::Monster, "Monster: b", Some("2"), None),
            row(LocationKind::Monster, "monster: A", Some("1"), None),
            row(LocationKind::Room, "Room: z", Some("1/1"), None),
            row(LocationKind::Room, "Room: c", Some("1/2"), None),
        ];
        // two rows were there at the first request, all four at the second
        apply_sorts(&mut rows, &[2]);
        assert_eq!(
            texts(&rows),
            ["monster: A", "Monster: b", "Room: z", "Room: c"]
        );
        apply_sorts(&mut rows, &[2, 4]);
        assert_eq!(
            texts(&rows),
            ["monster: A", "Monster: b", "Room: c", "Room: z"]
        );
    }

    #[test]
    fn refresh_sort_is_by_tag_descending() {
        let mut rows = vec![
            row(
                LocationKind::Monster,
                "5%",
                Some("5"),
                Some(("Monster: a(1)", "1")),
            ),
            row(
                LocationKind::Monster,
                "50%",
                Some("50"),
                Some(("Monster: b(2)", "2")),
            ),
            row(
                LocationKind::Room,
                "0.5%",
                Some("0.5"),
                Some(("Shop: s (1/2)", "9")),
            ),
            row(
                LocationKind::Monster,
                "5%",
                Some("5"),
                Some(("Monster: c(3)", "3")),
            ),
        ];
        refresh_sort(&mut rows, false);
        let got: Vec<&str> = rows.iter().map(|r| r.cells[0].text.as_str()).collect();
        assert_eq!(
            got,
            [
                "Monster: b(2)",
                "Monster: a(1)",
                "Monster: c(3)",
                "Shop: s (1/2)"
            ]
        );
        refresh_sort(&mut rows, true);
        let got: Vec<&str> = rows.iter().map(|r| r.cells[0].text.as_str()).collect();
        assert_eq!(
            got,
            [
                "Shop: s (1/2)",
                "Monster: b(2)",
                "Monster: a(1)",
                "Monster: c(3)"
            ]
        );
    }

    #[test]
    fn targets_follow_the_row_kind() {
        let m = row(
            LocationKind::Monster,
            "10%",
            Some("10"),
            Some(("Monster: x(25)", "25")),
        );
        assert_eq!(
            target_of(&m),
            Some(Target {
                table: "monsters",
                number: 25,
                key: None
            })
        );
        let shop = row(
            LocationKind::Room,
            "",
            Some("78"),
            Some(("Shop: Armour (2/2567)", "78")),
        );
        assert_eq!(
            target_of(&shop).map(|t| (t.table, t.number)),
            Some(("shops", 78))
        );
        let room = row(
            LocationKind::Room,
            "Room: Alley (1/405)",
            Some("1/405"),
            None,
        );
        assert_eq!(
            target_of(&room),
            Some(Target {
                table: "rooms",
                number: 0,
                key: Some("1/405".into())
            })
        );
        let tb = row(LocationKind::Textblock, "Textblock 9", Some("9"), None);
        assert_eq!(target_of(&tb), None);
        let zero = row(LocationKind::Item, "Item: x", Some("0"), None);
        assert_eq!(target_of(&zero), None);
    }
}
