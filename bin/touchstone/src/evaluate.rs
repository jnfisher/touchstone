//! `POST /api/evaluate/{tab}`: the explorer lists as MME builds them. Card P4-05.
//!
//! MME fills its weapon, armour, spell, sundry, race, class and shop lists with the `Add*2LV`
//! procedures (ported in `touchstone_rules::rows`) from the `Filter*` procedures of frmMain. This
//! module is the glue: it picks the records a tab lists, builds the profile of the character (or of
//! "no character"), runs the row builders and puts the finished rows on the wire with the column
//! headers of the ListView.
//!
//! A response is `{columns, rows}`. `columns[0]` is the `#` column (the `ListItem` text), so a
//! row's `cells` line up one to one with `columns`; a row MME left half built (a VB6 error stopped
//! its procedure) is padded with empty cells.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use touchstone_model::{Item, World};
use touchstone_rules::Profile;
use touchstone_rules::attack::EquippedGlobals;
use touchstone_rules::character::AttackTypeMud;
use touchstone_rules::pipeline::{CharacterState, PipelineOptions, refresh_all};
use touchstone_rules::rows::{
    Row, RowEnv, SpellOptions, WeaponArgs, WeaponOptions, add_armour_row, add_class_row,
    add_other_item_row, add_race_row, add_shop_row, add_spell_row, add_weapon_row,
    no_character_profile, race_color_code,
};
use touchstone_rules::spell_cast::UsableEnv;
use touchstone_rules::stats::compute_avg_level_max_stats;
use touchstone_vb6 as vb;

use crate::calc::{ApiResult, Body, bad, not_found};
use crate::character::CharacterDto;
use crate::detail::nmr_ver;
use crate::state::AppState;

/// The tabs this endpoint lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Weapons,
    Armour,
    Spells,
    Items,
    Races,
    Classes,
    Shops,
}

impl Tab {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "weapons" => Self::Weapons,
            "armour" => Self::Armour,
            "spells" => Self::Spells,
            "items" => Self::Items,
            "races" => Self::Races,
            "classes" => Self::Classes,
            "shops" => Self::Shops,
            _ => return None,
        })
    }
}

// ---------- request ----------

/// A text box MME reads with `Val`: the client may send text or a number.
#[derive(Debug, Clone, Default)]
pub struct BoxText(pub String);

impl<'de> Deserialize<'de> for BoxText {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        match Value::deserialize(d)? {
            Value::String(s) => Ok(Self(s)),
            Value::Number(n) => Ok(Self(n.to_string())),
            Value::Null => Ok(Self(String::new())),
            other => Err(D::Error::custom(format!(
                "expected text or number, got {other}"
            ))),
        }
    }
}

/// `cmbWeaponAbilityOp` / `cmbArmorAbilityOp`.
#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AbilityOp {
    /// Index 0: the value is at most the box.
    Le,
    /// Index 1, MME's default: the value is at least the box.
    #[default]
    Ge,
}

/// The view toggles of the tabs, with MME's defaults. A tab ignores the ones that are not its own.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    /// `chkWeaponOptions(3)` or `chkSpellOptions(0)`: calculate combat values.
    pub calc_combat: bool,
    /// `chkWeaponOptions(4)`: the 85% speed adjustment.
    pub slow_85: bool,
    /// `cmbWeaponCombos(1)`: 5 normal, 4 backstab, 6 bash, 7 smash. Read when combat is calculated.
    pub attack: Option<i32>,
    pub vs_ac: BoxText,
    pub vs_dr: BoxText,
    pub vs_dodge: BoxText,
    /// The weapon / armour ability list: 0 any, -1 "Negates (= Spell#)", else an ability number.
    pub ability: i32,
    pub ability_op: AbilityOp,
    /// `txtWeaponAbilityVal`; MME starts with 1.
    pub ability_value: Option<BoxText>,
    /// `txtGlobalLevel(1)`, the spell level box; the character's level when absent.
    pub level: Option<i32>,
    /// `chkSpellOptions(2)`.
    pub anti_magic: bool,
    /// `txtSpellOptions(0)`: the target's magic resistance.
    pub vs_mr: BoxText,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct EvaluateBody {
    /// The loaded character; it is used when its `global_filter` is on, as `chkGlobalFilter`.
    pub character: Option<CharacterDto>,
    pub options: Options,
}

/// MME's three-digit boxes.
const MAX_LEVEL: i32 = 999;

impl Options {
    fn validate(&self) -> Result<(), (StatusCode, String)> {
        if let Some(a) = self.attack
            && !(4..=7).contains(&a)
        {
            return Err(bad(format!(
                "attack must be 5 (normal), 4 (backstab), 6 (bash) or 7 (smash), not {a}"
            )));
        }
        if self.ability < -1 || self.ability > i32::from(i16::MAX) {
            return Err(bad("ability must be -1, 0 or an ability number"));
        }
        if let Some(l) = self.level
            && !(0..=MAX_LEVEL).contains(&l)
        {
            return Err(bad(format!("level must be 0 to {MAX_LEVEL}")));
        }
        Ok(())
    }

    fn ability_value(&self) -> f64 {
        self.ability_value.as_ref().map_or(1.0, |t| vb::val(&t.0))
    }
}

// ---------- response ----------

#[derive(Debug, Serialize, PartialEq)]
pub struct Column {
    pub id: &'static str,
    pub header: String,
    pub numeric: bool,
}

/// What a column sorts by: the number MME's numeric sort reads, or the text.
#[derive(Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum SortKey {
    Number(serde_json::Number),
    Text(String),
}

#[derive(Debug, Serialize, PartialEq)]
pub struct CellJson {
    pub text: String,
    pub sort: SortKey,
    /// The cell's VB `ForeColor` where MME assigned one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<i32>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct RowJson {
    /// The record number (0 for the surprise punch, which has no record).
    pub number: i32,
    pub cells: Vec<CellJson>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Table {
    pub columns: Vec<Column>,
    pub rows: Vec<RowJson>,
}

fn sort_number(n: f64) -> SortKey {
    let n = if n.is_finite() { n } else { 0.0 };
    let num = if n.fract() == 0.0 && n.abs() < 1e15 {
        serde_json::Number::from(n as i64)
    } else {
        serde_json::Number::from_f64(n).unwrap_or_else(|| 0.into())
    };
    SortKey::Number(num)
}

/// `StripNumFmt` then `Val`, which is how MME's numeric list sort reads a cell.
fn numeric_value(s: &str) -> f64 {
    let t: String = s.chars().filter(|c| *c != ',' && *c != '$').collect();
    vb::val(t.trim())
}

/// One column of a tab: `by_tag` columns sort by the cell's tag (the weapon first-round damage and
/// the armour AC/DR), as `lvWeapons_ColumnClick` and `lvArmour_ColumnClick` ask.
struct Col {
    id: &'static str,
    header: &'static str,
    numeric: bool,
    by_tag: bool,
}

const fn col(id: &'static str, header: &'static str, numeric: bool) -> Col {
    Col {
        id,
        header,
        numeric,
        by_tag: false,
    }
}

const fn tagged(id: &'static str, header: &'static str) -> Col {
    Col {
        id,
        header,
        numeric: true,
        by_tag: true,
    }
}

/// The `ColumnHeaders.Add` texts of frmMain's `SetUpFormObjects`, and the sort kinds of the
/// `lv*_ColumnClick` handlers.
fn columns(tab: Tab) -> Vec<Col> {
    match tab {
        Tab::Weapons => vec![
            col("num", "#", true),
            col("name", "Name", false),
            col("type", "Type", false),
            col("min", "Min", true),
            col("max", "Max", true),
            col("speed", "Speed", true),
            col("lvl", "LVL", true),
            col("str", "Str", true),
            col("enc", "Enc", true),
            col("ac", "AC", true),
            col("acc", "Acc", true),
            col("bs", "BS", true),
            col("crits", "Crits", true),
            col("limit", "Limit", true),
            col("dmg_spd", "Dmg/Spd", true),
            col("swings", "#Swings", true),
            col("xswings", "xSwings", true),
            col("extra", "Extra", true),
            col("dmg_rnd", "Dmg/Rnd", true),
            tagged("dmg_1st", "Dmg/1st"),
            col("ability", "Ability", false),
        ],
        Tab::Armour => vec![
            col("num", "#", true),
            col("name", "Name", false),
            col("worn", "Worn", false),
            col("type", "Armr Type", false),
            col("level", "Level", true),
            col("enc", "Enc", true),
            tagged("ac_dr", "AC/DR"),
            col("acc", "Acc", true),
            col("crits", "Crits", true),
            col("limit", "Limit", true),
            col("ac_enc", "AC/Enc", true),
            col("ability", "Ability", true),
        ],
        Tab::Spells => vec![
            col("num", "#", true),
            col("name", "Name", false),
            col("short", "Short", false),
            col("magery", "Magery", false),
            col("lvl", "LVL", true),
            col("mana", "Mana", true),
            col("diff", "Diff", true),
            col("dmg", "Dmg", true),
            col("dmg_m", "Dmg/M", true),
            col("heal", "Heal", true),
            col("heal_m", "Heal/M", true),
            col("detail", "Detail", false),
        ],
        Tab::Items => vec![
            col("num", "Number", true),
            col("name", "Name", false),
            col("type", "Type", false),
            col("enc", "Enc", true),
            col("limit", "Limit", true),
        ],
        Tab::Races => vec![
            col("num", "#", true),
            col("name", "Name", false),
            col("exp", "Exp", true),
            col("hp", "HP", true),
            col("str", "Str", true),
            col("int", "Int", true),
            col("wis", "Wis", true),
            col("agi", "Agi", true),
            col("hea", "Hea", true),
            col("cha", "Cha", true),
            col("abilities", "Abilities", false),
        ],
        Tab::Classes => vec![
            col("num", "#", true),
            col("name", "Name", false),
            col("exp", "Exp", true),
            col("weapon", "Weapon", false),
            col("armour", "Armour", false),
            col("magic", "Magic", false),
            col("cmbt", "Cmbt", true),
            col("hp", "HP", true),
            col("abilities", "Abilities", false),
        ],
        Tab::Shops => vec![
            col("num", "#", true),
            col("name", "Name", false),
            col("type", "Type", false),
        ],
    }
}

/// The finished list: the rows, and whatever the build changed about the headers.
struct Built {
    rows: Vec<Row>,
    /// `(column index, header)` pairs that replace the defaults.
    headers: Vec<(usize, &'static str)>,
    /// The weapon Ability column sorts as text, unless a numeric ability is shown.
    ability_numeric: bool,
}

fn to_table(tab: Tab, built: Built) -> Table {
    let mut cols = columns(tab);
    if tab == Tab::Weapons
        && let Some(c) = cols.last_mut()
    {
        c.numeric = built.ability_numeric;
    }
    let columns_json: Vec<Column> = cols
        .iter()
        .enumerate()
        .map(|(k, c)| Column {
            id: c.id,
            header: built
                .headers
                .iter()
                .find(|(i, _)| *i == k)
                .map_or(c.header, |(_, h)| h)
                .to_owned(),
            numeric: c.numeric,
        })
        .collect();
    let rows = built
        .rows
        .iter()
        .map(|r| {
            let number = vb::val(&r.text) as i32;
            let mut cells = Vec::with_capacity(cols.len());
            cells.push(CellJson {
                text: r.text.clone(),
                sort: sort_number(f64::from(number)),
                color: None,
            });
            for (k, c) in cols.iter().enumerate().skip(1) {
                let cell = r.cells.get(k - 1);
                let text = cell.map(|c| c.text.clone()).unwrap_or_default();
                let sort = if c.numeric {
                    let source = match cell {
                        Some(cell) if c.by_tag => cell.tag.as_deref().unwrap_or(&cell.text),
                        _ => &text,
                    };
                    sort_number(numeric_value(source))
                } else {
                    SortKey::Text(text.clone())
                };
                cells.push(CellJson {
                    text,
                    sort,
                    color: cell.and_then(|c| c.fore_color),
                });
            }
            RowJson { number, cells }
        })
        .collect();
    Table {
        columns: columns_json,
        rows,
    }
}

// ---------- the build ----------

/// What the builders read about the world and the character.
struct Ctx<'a> {
    world: &'a World,
    nmr: f64,
    avg: f64,
    /// The loaded character's state when it is used (`chkGlobalFilter` on).
    state: Option<CharacterState>,
    level_text: String,
    /// `lblInvenCharStat(10).Tag`, the accuracy a swing falls back on.
    accuracy_tag: String,
}

impl Ctx<'_> {
    /// `PopulateCharacterProfile(tChar, bUseCharacter, True, mud, weapon)`.
    fn profile(
        &self,
        mud: AttackTypeMud,
        weapon: i32,
    ) -> touchstone_rules::character::CharacterProfile {
        match &self.state {
            Some(st) => st.profile_for(self.world, mud, weapon),
            None => no_character_profile(Profile::Stock, self.world, self.nmr, self.avg, mud),
        }
    }
}

type ApiError = (StatusCode, String);

fn ability_ok(it: &Item, opts: &Options) -> bool {
    let value = opts.ability_value();
    if opts.ability > 0 {
        return it.abilities.iter().any(|s| {
            s.ability == opts.ability
                && match opts.ability_op {
                    AbilityOp::Le => f64::from(s.value) <= value,
                    AbilityOp::Ge => f64::from(s.value) >= value,
                }
        });
    }
    if opts.ability == -1 {
        // `nFilterNegate` is a Long: a value of 0 or less filters nothing
        let spell = vb::clng(value).unwrap_or(0);
        return spell <= 0 || it.negate_spell.contains(&spell);
    }
    true
}

fn weapons(ctx: &Ctx, opts: &Options) -> Built {
    let attack = if opts.calc_combat {
        opts.attack.unwrap_or(5)
    } else {
        5
    };
    let mud = AttackTypeMud::from_code(attack).unwrap_or(AttackTypeMud::Normal);
    let profile = ctx.profile(mud, 0);
    let ch = CharacterState::attack_profile_of(&profile);
    let equipped = ctx
        .state
        .as_ref()
        .map_or_else(EquippedGlobals::default, CharacterState::equipped_globals);
    let env = row_env(ctx, &equipped);
    let wopts = WeaponOptions {
        global_filter: ctx.state.is_some(),
        calc_combat: opts.calc_combat,
        slow_85: opts.slow_85,
        combo_mode: Some(attack),
        extras: [
            opts.vs_ac.0.clone(),
            opts.vs_dr.0.clone(),
            opts.vs_dodge.0.clone(),
        ],
    };
    let args = WeaponArgs {
        ability: i16::try_from(opts.ability.max(0)).unwrap_or(0),
        attack_type: attack,
        force_calc: false,
    };
    let mut rows = Vec::new();
    for it in &ctx.world.items {
        if it.item_type == 1
            && it.in_game
            && ability_ok(it, opts)
            && let Some(row) = add_weapon_row(&env, Some(it), &ch, &wopts, &args)
        {
            rows.push(row);
        }
    }
    // a backstab list ends with the surprise punch, unless an ability narrows the list
    if attack == 4 && opts.ability == 0 {
        let punch = ctx.profile(AttackTypeMud::Surprise, -1);
        let ch = CharacterState::attack_profile_of(&punch);
        let args = WeaponArgs {
            ability: 0,
            attack_type: -4,
            force_calc: false,
        };
        rows.extend(add_weapon_row(&env, None, &ch, &wopts, &args));
    }
    Built {
        rows,
        headers: Vec::new(),
        ability_numeric: opts.ability > 0 && opts.ability != 43,
    }
}

fn armour(ctx: &Ctx, opts: &Options) -> Built {
    let ability = i16::try_from(opts.ability.max(0)).unwrap_or(0);
    let rows = ctx
        .world
        .items
        .iter()
        .filter(|it| it.item_type == 0 && it.in_game && ability_ok(it, opts))
        .filter_map(|it| add_armour_row(it, ability))
        .collect();
    Built {
        rows,
        headers: Vec::new(),
        ability_numeric: false,
    }
}

fn spells(ctx: &Ctx, opts: &Options) -> Built {
    let profile = ctx.profile(AttackTypeMud::None, 0);
    let equipped = EquippedGlobals::default();
    let env = row_env(ctx, &equipped);
    let use_character = ctx.state.is_some();
    let level = opts
        .level
        .map_or_else(|| ctx.level_text.clone(), |l| l.to_string());
    let sopts = SpellOptions {
        global_filter: use_character,
        global_level_text: level.clone(),
        calc_combat: opts.calc_combat,
        anti_magic: opts.anti_magic,
        vs_mr_text: opts.vs_mr.0.clone(),
    };
    let usable = UsableEnv {
        profile: Profile::Stock,
        spells: &ctx.world.spells,
        classes: &ctx.world.classes,
        nmr_ver: ctx.nmr,
        only_in_game: true,
        disable_kai_autolearn: ctx.world.info.disable_kai,
    };
    let rows = ctx
        .world
        .spells
        .iter()
        .filter(|s| usable.spell_is_in_game(s.number))
        .filter_map(|s| add_spell_row(&env, s, &profile, &sopts, false))
        .map(|r| r.row)
        .collect();
    // FilterSpells: the column reads "Cast" when the character's chance is shown
    let headers = if use_character && vb::val(&level) > 0.0 {
        vec![(6, "Cast")]
    } else {
        Vec::new()
    };
    Built {
        rows,
        headers,
        ability_numeric: false,
    }
}

fn sundry(ctx: &Ctx) -> Built {
    let rows = ctx
        .world
        .items
        .iter()
        .filter(|it| ((it.item_type == 0 && it.worn == 0) || it.item_type > 1) && it.in_game)
        .filter_map(add_other_item_row)
        .collect();
    plain(rows)
}

fn plain(rows: Vec<Row>) -> Built {
    Built {
        rows,
        headers: Vec::new(),
        ability_numeric: false,
    }
}

fn row_env<'a>(ctx: &'a Ctx, equipped: &'a EquippedGlobals) -> RowEnv<'a> {
    let mut env = RowEnv::for_world(ctx.world, Profile::Stock, equipped);
    env.dat_ver_caption = &ctx.world.info.dat_file_version;
    env.avg_level_max_all_stats = ctx.avg;
    env.accuracy_tag = &ctx.accuracy_tag;
    env.global_level_text = ctx.state.is_some().then_some(ctx.level_text.as_str());
    env
}

/// Build one tab's list for a request.
///
/// # Errors
/// 422 for a character without class and race or one the pipeline cannot evaluate; 400 for a
/// character that is not well formed.
pub fn evaluate(world: &World, tab: Tab, req: &EvaluateBody) -> Result<Table, ApiError> {
    req.options.validate()?;
    let nmr = nmr_ver(&world.info.nmr_version);
    let avg = compute_avg_level_max_stats(Profile::Stock, &world.races).all_stats;

    let mut state = None;
    let mut level_text = "0".to_owned();
    if let Some(dto) = &req.character {
        let ch = dto.to_character().map_err(bad)?;
        if ch.class.is_none() || ch.race.is_none() {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                "pick a class and race".into(),
            ));
        }
        if dto.global_filter {
            let opts = PipelineOptions {
                hide_class_race: dto.options.hide_class_race,
                min_item_level: dto.options.min_item_level,
                dat_ver_caption: world.info.dat_file_version.clone(),
                nmr_ver: nmr,
                avg_level_max_all_stats: avg,
                ..PipelineOptions::default()
            };
            let st = refresh_all(Profile::Stock, world, &ch, &opts).map_err(|e| {
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("cannot evaluate this character: {e}"),
                )
            })?;
            state = Some(st);
            level_text = ch.level.to_string();
        }
    }
    let accuracy_tag = state
        .as_ref()
        .and_then(|st| st.stats.stat_tag[10])
        .map_or_else(String::new, vb::cstr);
    let ctx = Ctx {
        world,
        nmr,
        avg,
        state,
        level_text,
        accuracy_tag,
    };
    let built = match tab {
        Tab::Weapons => weapons(&ctx, &req.options),
        Tab::Armour => armour(&ctx, &req.options),
        Tab::Spells => spells(&ctx, &req.options),
        Tab::Items => sundry(&ctx),
        Tab::Races => {
            let equipped = EquippedGlobals::default();
            let env = row_env(&ctx, &equipped);
            let mut rows: Vec<Row> = world
                .races
                .iter()
                .filter_map(|r| add_race_row(&env, r))
                .collect();
            race_color_code(world, &mut rows);
            plain(rows)
        }
        Tab::Classes => {
            let equipped = EquippedGlobals::default();
            let env = row_env(&ctx, &equipped);
            plain(
                world
                    .classes
                    .iter()
                    .filter_map(|c| add_class_row(&env, c))
                    .collect(),
            )
        }
        Tab::Shops => plain(
            world
                .shops
                .iter()
                .filter(|s| s.in_game)
                .filter_map(add_shop_row)
                .collect(),
        ),
    };
    Ok(to_table(tab, built))
}

// ---------- the route ----------

pub async fn evaluate_route(
    State(state): State<Arc<AppState>>,
    Path(tab_name): Path<String>,
    Body(raw): Body<Value>,
) -> ApiResult {
    let Some(tab) = Tab::parse(&tab_name) else {
        return Err(not_found(format!("no list called {tab_name:?}")));
    };
    let req: EvaluateBody = serde_json::from_value(raw.clone()).map_err(|e| bad(e.to_string()))?;
    req.options.validate()?;
    let loaded = state.current();
    // serde_json keeps object keys sorted, so equal requests give equal keys
    let key = format!("{tab_name}|{raw}");
    let load = loaded.info.load_count;
    let body = if let Some(hit) = state.evaluate_cache_get(&key, load) {
        hit
    } else {
        let job = tokio::task::spawn_blocking(move || {
            evaluate(&loaded.world, tab, &req).and_then(|t| {
                serde_json::to_vec(&t)
                    .map(Bytes::from)
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
            })
        });
        let bytes = job
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))??;
        state.evaluate_cache_put(key, load, bytes.clone());
        bytes
    };
    Ok((
        [
            (CACHE_CONTROL, "no-cache"),
            (CONTENT_TYPE, "application/json"),
        ],
        body,
    )
        .into_response())
}
