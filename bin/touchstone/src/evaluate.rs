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
use touchstone_model::World;
use touchstone_rules::Profile;
use touchstone_rules::attack::EquippedGlobals;
use touchstone_rules::character::{
    AttackTypeMud, Character as MmeCharacter, CharacterProfile, PartyFilter,
    populate_character_profile, set_current_attack_type_config,
};
use touchstone_rules::exp_per_hour::ExpPerHourSettings;
use touchstone_rules::filters::{
    AbilityFilter, AllReachable, Answer, ArmourFilter, DamagePrompts, FilterEnv, MonsterAdd,
    MonsterExtras, MonsterFilter, MonsterFilterInputs, ProfileRequest, SpellFilter, SpellFilterEnv,
    WeaponEntry, WeaponFilter, filter_armour, filter_monsters, filter_spells, filter_sundry,
    filter_weapons,
};
use touchstone_rules::gear::Character as GlobalChar;
use touchstone_rules::lairs::{
    CharCallouts, CharLairEnv, LairCache, LairQuery, calculate_average_lairs,
};
use touchstone_rules::monster_sim::MonsterSimInput;
use touchstone_rules::pipeline::{CharacterState, PipelineOptions, refresh_all};
use touchstone_rules::rows::monster::{
    MonsterRowArgs, MonsterRowInputs, add_monster_row, backstab_attack,
};
use touchstone_rules::rows::{
    Row, RowEnv, SpellOptions, WeaponArgs, WeaponOptions, add_armour_row, add_class_row,
    add_other_item_row, add_race_row, add_shop_row, add_spell_row, add_weapon_row,
    no_character_profile, race_color_code,
};
use touchstone_rules::spell_cast::UsableEnv;
use touchstone_rules::stats::compute_avg_level_max_stats;
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Rnd};

use crate::calc::{ApiResult, Body, bad, not_found};
use crate::character::CharacterDto;
use crate::detail::nmr_ver;
use crate::settings::CharacterBlock;
use crate::state::AppState;

/// The tabs this endpoint lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Weapons,
    Armour,
    Spells,
    Monsters,
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
            "monsters" => Self::Monsters,
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
    /// The Monsters tab's filter boxes, lair/party mode and the extra filters. The Monsters list and
    /// pane read them; the other tabs ignore them.
    pub monsters: MonsterControls,
    /// The Compare tab's lists (card P4-14): list exactly these record numbers, in this order, one row per entry, with
    /// no filter. MME's `CompareAddItem` and the other adders call `AddWeapon2LV`, `AddArmour2LV`, `AddSpell2LV` and
    /// `AddMonster2LV` straight from the record, so a compare row is the explorer's row for the same record.
    /// Weapons, armour, spells and monsters read it; a record that does not exist adds no row.
    pub numbers: Option<Vec<i32>>,
}

/// Entries one compare request may carry (MME saves 101 per list; the cap only bounds the work).
const MAX_NUMBERS: usize = 5000;

/// The Monsters tab's controls (card P4-08): frmMain's filter boxes, `optMonsterFilter`, the lair/party
/// boxes and the "Monster Filters" window. `Default` is what MME starts with.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct MonsterControls {
    /// `optMonsterFilter(1)`: filter by lair (and, with a party size above one, for a party).
    pub lair: bool,
    /// `cmbMonsterRegen.ListIndex`: 0 regen at most the box, 1 at least.
    pub regen_op: i32,
    /// `txtMonsterRegen`.
    pub regen: BoxText,
    /// `txtMonsterHP`.
    pub hp: BoxText,
    /// `txtMonsterEXP`; a trailing K or M multiplies.
    pub exp: BoxText,
    /// `txtMonsterDamage`.
    pub damage: BoxText,
    /// `chkMonMagic`.
    pub magic: bool,
    /// `txtMonMagic`.
    pub magic_value: BoxText,
    /// `txtMonsterLairFilter(0..9)` and `txtMonsterDamageOUT(0..1)`.
    pub party: PartyBoxes,
    pub extras: ExtrasBody,
}

/// The lair/party frame: `txtMonsterLairFilter(0..9)` and the two damage-output boxes.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct PartyBoxes {
    /// `# Party`.
    pub size: BoxText,
    pub ac: BoxText,
    pub dr: BoxText,
    pub mr: BoxText,
    pub dodge: BoxText,
    /// Average max HP.
    pub hp: BoxText,
    /// Anti-magic players.
    pub anti_magic: BoxText,
    /// `RestHP`.
    pub rest_hp: BoxText,
    pub accuracy: BoxText,
    pub swings: BoxText,
    /// `txtMonsterDamageOUT(0)` and `(1)`.
    pub damage_out_physical: BoxText,
    pub damage_out_magic: BoxText,
}

impl PartialEq for BoxText {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

fn boxed(s: &str) -> BoxText {
    BoxText(s.to_owned())
}

impl Default for PartyBoxes {
    fn default() -> Self {
        Self {
            size: boxed("1"),
            ac: boxed("60"),
            dr: boxed("20"),
            mr: boxed("50"),
            dodge: boxed("50"),
            hp: boxed("400"),
            anti_magic: boxed("0"),
            rest_hp: boxed("12"),
            accuracy: boxed("250"),
            swings: boxed("1"),
            damage_out_physical: boxed("99999"),
            damage_out_magic: boxed("99999"),
        }
    }
}

impl Default for MonsterControls {
    /// The values `RemoveFilters` writes into the boxes.
    fn default() -> Self {
        let d = MonsterFilter::default();
        Self {
            lair: false,
            regen_op: d.regen_op,
            regen: BoxText(d.regen_text),
            hp: BoxText(d.hp_text),
            exp: BoxText(d.exp_text),
            damage: BoxText(d.damage_text),
            magic: d.magic,
            magic_value: BoxText(d.magic_text),
            party: PartyBoxes::default(),
            extras: ExtrasBody::default(),
        }
    }
}

macro_rules! extras_body {
    ($($f:ident: $t:ty),* $(,)?) => {
        /// The "Monster Filters" window (`filter_Monster_*`); a field left out keeps the window's start-up value.
        #[derive(Debug, Clone, Default, Deserialize, PartialEq)]
        #[serde(default, deny_unknown_fields)]
        pub struct ExtrasBody { $(pub $f: Option<$t>,)* }

        impl ExtrasBody {
            fn apply(&self) -> MonsterExtras {
                let mut e = MonsterExtras::default();
                $(if let Some(v) = self.$f.clone() { e.$f = v; })*
                e
            }
        }
    };
}

extras_body! {
    enabled: bool, armour_class: i32, damage_resist: i32, magic_res: i32, bs_def: i32, game_limit: i32,
    avg_lair_exp: f64, atk_accuracy_maj: i32, atk_accuracy_max: i32, dodge: i32, num_lairs: i32,
    num_mobs_lte: f64, num_mobs_gte: f64, follow_gte: i32, follow_lte: i32, at_least_lairs: i32,
    abilities: [[i32; 3]; 3], is_undead: bool, non_hostile_vs_evil: bool, non_hostile_vs_ng: bool,
    reachable_only: bool, at_least_lairs_on: bool, drops_cash: bool, drops_r: bool, drops_p: bool,
    drops_g: bool, drops_s: bool, atk_no_poison: bool, atk_no_confusion: bool, atk_no_fear: bool,
    show_all: bool,
}

impl MonsterControls {
    /// `optMonsterFilter(1)` and the boxes `PopulateCharacterProfile` and `GetDamageOutput` read.
    pub(crate) fn party(&self) -> PartyFilter {
        let p = &self.party;
        PartyFilter {
            lair_mode: self.lair,
            lair_text: [
                &p.size,
                &p.ac,
                &p.dr,
                &p.mr,
                &p.dodge,
                &p.hp,
                &p.anti_magic,
                &p.rest_hp,
                &p.accuracy,
            ]
            .map(|b| b.0.clone()),
            monster_damage_text: self.damage.0.clone(),
        }
    }

    fn filter(&self) -> MonsterFilter {
        MonsterFilter {
            regen_op: self.regen_op,
            regen_text: self.regen.0.clone(),
            hp_text: self.hp.0.clone(),
            exp_text: self.exp.0.clone(),
            damage_text: self.damage.0.clone(),
            magic: self.magic,
            magic_text: self.magic_value.0.clone(),
            extras: self.extras.apply(),
        }
    }
}

/// The global filter bar (card P4-21): `chkGlobalFilter` and the boxes of its frame.
///
/// The defaults are the case `ItemIsUsableByChar` answers true for at once (level 999, any class,
/// any alignment), so a bar left alone filters nothing.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct GlobalFilterBody {
    /// Mirror the loaded character (class, race, level, alignment, and its stats for the row
    /// calculations); the class, race, level and alignment below are then ignored.
    pub use_character: bool,
    /// `cmbGlobalClass`: a class number, 0 for the first entry (any class).
    pub class: i32,
    /// `cmbGlobalRace`.
    pub race: i32,
    /// `txtGlobalLevel(0)`.
    pub level: i32,
    /// `cmbGlobalAlignment.ListIndex`: 0 any, 1 good, 2 neutral, 3 evil.
    pub alignment: i32,
    /// `txtGlobalMinLVL`.
    pub min_item_level: i32,
}

impl Default for GlobalFilterBody {
    fn default() -> Self {
        Self {
            use_character: false,
            class: 0,
            race: 0,
            level: MAX_LEVEL,
            alignment: 0,
            min_item_level: 0,
        }
    }
}

impl GlobalFilterBody {
    fn validate(&self) -> Result<(), (StatusCode, String)> {
        let id = 0..=i32::from(i16::MAX);
        if !id.contains(&self.class) || !id.contains(&self.race) {
            return Err(bad("class and race must be record numbers"));
        }
        if !(0..=MAX_LEVEL).contains(&self.level) {
            return Err(bad(format!("level must be 0 to {MAX_LEVEL}")));
        }
        if !(0..=MAX_LEVEL).contains(&self.min_item_level) {
            return Err(bad(format!("min_item_level must be 0 to {MAX_LEVEL}")));
        }
        if !(0..=3).contains(&self.alignment) {
            return Err(bad(
                "alignment must be 0 (any), 1 (good), 2 (neutral) or 3 (evil)",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct EvaluateBody {
    /// The loaded character; it is used when the global filter says so (`use_character`), or,
    /// with no `global_filter` object, when its own `global_filter` is on, as `chkGlobalFilter`.
    pub character: Option<CharacterDto>,
    /// The global filter bar; absent means the character's own flag decides.
    pub global_filter: Option<GlobalFilterBody>,
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
        if let Some(n) = &self.numbers
            && n.len() > MAX_NUMBERS
        {
            return Err(bad(format!("numbers has more than {MAX_NUMBERS} entries")));
        }
        Ok(())
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
    /// Monsters: some record was filtered out, so MME's caption reads "*Monsters*".
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub filtered: bool,
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
        Tab::Monsters => monster_columns(1.83),
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
    /// The data version, for the Monsters columns (the set depends on it).
    nmr: f64,
    /// Some monster was filtered out: MME's "*Monsters*" caption.
    filtered: bool,
}

/// The Monsters list's `ColumnHeaders.Add` texts by data version. Most cells carry the number MME sorts by in their tag.
fn monster_columns(nmr: f64) -> Vec<Col> {
    let mut c = vec![
        col("num", "#", true),
        col("name", "Name", false),
        tagged("rgn", "Rgn"),
        tagged("exp", "Exp"),
        tagged("hp", "HP"),
        tagged("ac_dr", "AC/DR"),
        tagged("dodge", "Dodge"),
        tagged("mr", "MR"),
        tagged("acc", "Acc (Maj/Mx)"),
        tagged("damage", "Damage"),
        tagged("exp_dmg_hp", "Exp/(Dmg+HP)"),
        tagged(
            "lair_exp",
            if nmr >= 1.83 {
                "Lair Exp"
            } else {
                "Script Value"
            },
        ),
        tagged("lairs", "Lairs"),
    ];
    if nmr >= 1.83 {
        c.push(tagged("mobs", "Mobs/Spwn"));
    } else if nmr >= 1.82 {
        c.push(tagged("mobs", "#Mobs"));
    }
    c.push(tagged("magic", "Mag."));
    if nmr >= 1.83 {
        c.push(tagged("bs_def", "BS Defense"));
    }
    c.push(tagged("undead", "Undead"));
    c.push(col("spell_atk", "Spell Atk.", false));
    c
}

fn to_table(tab: Tab, built: Built) -> Table {
    let mut cols = if tab == Tab::Monsters {
        monster_columns(built.nmr)
    } else {
        columns(tab)
    };
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
                // a row MME greys (the filter's "show all") has the colour on its ListItem too
                color: r.fore_color,
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
        filtered: built.filtered,
    }
}

// ---------- the build ----------

/// What the builders read about the world and the character.
pub(crate) struct Ctx<'a> {
    pub(crate) world: &'a World,
    pub(crate) nmr: f64,
    pub(crate) avg: f64,
    /// The loaded character's state when it is used (`chkGlobalFilter` on).
    pub(crate) state: Option<CharacterState>,
    pub(crate) level_text: String,
    /// `lblInvenCharStat(10).Tag`, the accuracy a swing falls back on.
    pub(crate) accuracy_tag: String,
    /// The global filter's class, race, level, alignment and minimum item level.
    pub(crate) who: GlobalChar,
    /// `chkGlobalFilter.Value = 1`: the lists keep to what `who` can use. True too for a filter
    /// that has its own values and no character (`state` is then `None`).
    pub(crate) filter_on: bool,
    /// `nEquippedItem`.
    pub(crate) equipped_items: [i32; 20],
    /// The attack choice of the character that came with the request, used or not: MME keeps its
    /// `nGlobalAttack*` globals whatever `chkGlobalFilter` says, so "Damage vs Mob" follows them.
    pub(crate) attack: Option<touchstone_rules::character::AttackConfig>,
    /// The load's reachability, for "Reachable only" (computed when that filter asks, as `ReachIsMonsterOK` does).
    pub(crate) reach: Option<&'a crate::reach::ReachSlot>,
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

/// `FilterEnv` over the list tabs: `bOnlyInGame` is on, as the data the tabs show is the in-game data.
fn filter_env<'a>(ctx: &'a Ctx<'a>) -> FilterEnv<'a> {
    let martial = ctx.state.as_ref().map_or([0.0; 3], |st| {
        std::array::from_fn(|k| st.stats.stat_tag[37 + k].map_or(0.0, |t| vb::val(&vb::cstr(t))))
    });
    FilterEnv {
        profile: Profile::Stock,
        world: ctx.world,
        only_in_game: true,
        global_filter: ctx.filter_on,
        who: ctx.who,
        equipped: ctx.equipped_items,
        martial_skill: martial,
    }
}

fn ability_filter(opts: &Options) -> AbilityFilter {
    AbilityFilter {
        selected: opts.ability,
        at_most: opts.ability_op == AbilityOp::Le,
        // MME starts the box with 1
        value_text: opts
            .ability_value
            .as_ref()
            .map_or_else(|| "1".to_owned(), |t| t.0.clone()),
    }
}

fn weapons(ctx: &Ctx, opts: &Options) -> Built {
    let attack = if opts.calc_combat {
        opts.attack.unwrap_or(5)
    } else {
        5
    };
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
    if let Some(numbers) = &opts.numbers {
        // CompareAddItem: AddWeapon2LV with the profile PopulateCharacterProfile(tChar, False, True) gives
        let ch = CharacterState::attack_profile_of(&ctx.profile(AttackTypeMud::None, 0));
        let rows = numbers
            .iter()
            .filter_map(|n| {
                let item = ctx.world.get_item(*n)?;
                add_weapon_row(&env, Some(item), &ch, &wopts, &WeaponArgs::default())
            })
            .collect();
        return plain(rows);
    }
    let filter = WeaponFilter {
        calc_combat: opts.calc_combat,
        slow_85: opts.slow_85,
        combo: Some(attack),
        vs: wopts.extras.clone(),
        ability: ability_filter(opts),
        ..WeaponFilter::default()
    };
    let profile_for = |req: ProfileRequest| {
        let mud = AttackTypeMud::from_code(req.mud).unwrap_or(AttackTypeMud::Normal);
        ctx.profile(mud, req.weapon)
    };
    // the tabs have no dialogs: a character without backstab skill is calculated anyway
    let list = filter_weapons(
        &filter_env(ctx),
        &env,
        &filter,
        false,
        &profile_for,
        &mut |_| Answer::Yes,
    );
    let mut rows = Vec::new();
    for entry in list.entries {
        match entry {
            WeaponEntry::Built(r) => rows.push(r),
            WeaponEntry::Add(a) => {
                let ch = CharacterState::attack_profile_of(&profile_for(a.profile));
                let args = WeaponArgs {
                    ability: a.ability,
                    attack_type: a.attack_type,
                    force_calc: a.force_calc,
                };
                let item = a.item.and_then(|n| ctx.world.get_item(n));
                rows.extend(add_weapon_row(&env, item, &ch, &wopts, &args));
            }
        }
    }
    Built {
        rows,
        headers: Vec::new(),
        ability_numeric: list.numeric_ability_sort,
        nmr: 0.0,
        filtered: false,
    }
}

fn armour(ctx: &Ctx, opts: &Options) -> Built {
    if let Some(numbers) = &opts.numbers {
        // CompareAddItem and LoadCharacter: AddArmour2LV with no ability to show
        return plain(
            numbers
                .iter()
                .filter_map(|n| add_armour_row(ctx.world.get_item(*n)?, 0))
                .collect(),
        );
    }
    let filter = ArmourFilter {
        ability: ability_filter(opts),
        ..ArmourFilter::default()
    };
    let list = filter_armour(&filter_env(ctx), &filter, ctx.filter_on);
    let rows = list
        .entries
        .iter()
        .filter_map(|a| add_armour_row(ctx.world.get_item(a.item)?, a.ability))
        .collect();
    Built {
        rows,
        headers: Vec::new(),
        ability_numeric: false,
        nmr: 0.0,
        filtered: false,
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
    if let Some(numbers) = &opts.numbers {
        // the Compare tab's AddSpell2LV calls: the explorer's row for the record, no filter
        let rows = numbers
            .iter()
            .filter_map(|n| ctx.world.get_spell(*n))
            .filter_map(|s| add_spell_row(&env, s, &profile, &sopts, false))
            .map(|r| r.row)
            .collect();
        return plain(rows);
    }
    // FilterSpells with MME's default controls; the global filter (the character's, or the bar's own values) decides
    // which spells the class, level and alignment can use
    let t_char = if ctx.filter_on {
        CharacterProfile {
            class: ctx.who.class,
            level: ctx.who.level,
            align: i16::try_from(ctx.who.alignment).unwrap_or(0),
            ..CharacterProfile::default()
        }
    } else {
        CharacterProfile::default()
    };
    let header_level = opts.level.map_or_else(
        || {
            if use_character {
                level.clone()
            } else {
                ctx.who.level.to_string()
            }
        },
        |l| l.to_string(),
    );
    let list = filter_spells(
        &SpellFilterEnv {
            usable: &usable,
            use_character: ctx.filter_on,
            global_class: ctx.who.class,
            global_level: vb::val(&header_level),
        },
        &SpellFilter::default(),
        &t_char,
        false,
    );
    let rows = list
        .entries
        .iter()
        .filter_map(|n| ctx.world.get_spell(*n))
        .filter_map(|s| add_spell_row(&env, s, &profile, &sopts, false))
        .map(|r| r.row)
        .collect();
    // FilterSpells: the column reads "Cast" when the character's chance is shown
    let headers = if list.cast_header {
        vec![(6, "Cast")]
    } else {
        Vec::new()
    };
    Built {
        rows,
        headers,
        ability_numeric: false,
        nmr: 0.0,
        filtered: false,
    }
}

/// `Dim`med text of a cast; the damage numbers never need it.
struct NoCastText;

impl touchstone_rules::attack::CastText for NoCastText {
    fn spell_name(&self, _: i32) -> String {
        String::new()
    }
    fn spell_eq(&self, _: i32, _: i16) -> String {
        String::new()
    }
}

/// The monster damage arrays (`nMonsterDamageVsChar` and friends) as MME starts: nothing calculated.
fn uncalculated(world: &World) -> Vec<Currency> {
    let top = world.monsters.iter().map(|m| m.number).max().unwrap_or(0);
    vec![Currency::from_scaled(-10_000); usize::try_from(top).unwrap_or(0) + 1]
}

/// What the Monsters list and the monster pane share once the lair environment stands: the character
/// state (a placeholder without a character), the profile of the list, and the start-up lair averages.
pub(crate) struct MonsterSetup<'a> {
    pub state: &'a CharacterState,
    /// The combat context the state was built with.
    pub cctx: &'a touchstone_rules::character::Context<'a>,
    /// `PopulateCharacterProfile(tChar)`.
    pub t_char: CharacterProfile,
    /// `nMonsterPossy` and `nMonsterSpawnChance`, filled by `CalculateAverageLairs` as MME does while starting.
    pub possy: Vec<Currency>,
    pub spawn_chance: Vec<Currency>,
    pub settings: &'a ExpPerHourSettings,
    pub sim_rounds: i32,
}

/// Build the lair environment the Monsters list and pane run in and hand it to `f`. Without a character the list is
/// made for "no character" (the global filter off); the placeholder pipeline state behind it only supplies the
/// plumbing the damage environment needs. `controls` supplies the lair/party boxes.
pub(crate) fn with_monster_env<R>(
    ctx: &Ctx,
    controls: &MonsterControls,
    settings: &ExpPerHourSettings,
    sim_rounds: i32,
    cast: &dyn touchstone_rules::attack::CastText,
    f: impl FnOnce(&mut CharLairEnv<'_>, &MonsterSetup<'_>) -> R,
) -> Result<R, ApiError> {
    let world = ctx.world;
    let unprocessable = |m: String| (StatusCode::UNPROCESSABLE_ENTITY, m);
    let placeholder;
    let state = if let Some(st) = &ctx.state {
        st
    } else {
        let first = |n: Option<i32>| {
            n.ok_or_else(|| unprocessable("the data has no classes or races".into()))
        };
        let ch = MmeCharacter {
            race: Some(first(world.races.first().map(|r| r.number))?),
            class: Some(first(world.classes.first().map(|c| c.number))?),
            level: 1,
            stats: [50; 6],
            attack: ctx.attack.unwrap_or_default(),
            ..MmeCharacter::default()
        };
        let opts = PipelineOptions {
            dat_ver_caption: world.info.dat_file_version.clone(),
            nmr_ver: ctx.nmr,
            avg_level_max_all_stats: ctx.avg,
            party: controls.party(),
            ..PipelineOptions::default()
        };
        placeholder = refresh_all(Profile::Stock, world, &ch, &opts)
            .map_err(|e| unprocessable(format!("cannot evaluate the default character: {e}")))?;
        &placeholder
    };
    let party = controls.party();
    let p = &controls.party;
    let cenv = state.env(
        [
            p.damage_out_physical.0.clone(),
            p.damage_out_magic.0.clone(),
        ],
        p.swings.0.clone(),
    );
    let mut cctx = cenv.context(world);
    cctx.recovery_only = settings.recovery_only;
    let key = set_current_attack_type_config(&cctx, &state.character, "");
    let mut t_char = CharacterProfile::default();
    populate_character_profile(
        &cctx,
        &state.character,
        &mut t_char,
        false,
        false,
        AttackTypeMud::None,
        0,
        false,
    );
    let (vs_party, vs_char, vs_default) = (
        uncalculated(world),
        uncalculated(world),
        uncalculated(world),
    );
    let slots = vs_char.len();
    let (mut possy, mut spawn_chance) = (
        vec![Currency::from_scaled(0); slots],
        vec![Currency::from_scaled(0); slots],
    );
    let mut cache = LairCache::load(world, Profile::Stock, ctx.nmr);
    let mut sim = MonsterSimInput::default();
    let mut rnd = Rnd::from_state(327_680);
    let use_character = state.character.global_filter;
    // CalculateAverageLairs runs while MME starts (bStartup), when GetLairInfo answers from its cache
    {
        let mut env = CharLairEnv {
            cache: &mut cache,
            query: LairQuery {
                profile: Profile::Stock,
                startup: true,
                use_character,
                party: &party,
                global_attack_config: &key.config,
            },
            callouts: CharCallouts {
                world,
                damage: cenv.damage_env(world, &cctx, &state.character, cast),
                vs_party: &vs_party,
                vs_char: &vs_char,
                vs_default: &vs_default,
                sim_rounds,
                sim: &mut sim,
                rnd: &mut rnd,
            },
        };
        let rows: Vec<(i32, &str)> = world
            .monsters
            .iter()
            .map(|m| (m.number, m.summoned_by.as_deref().unwrap_or("")))
            .collect();
        calculate_average_lairs(&mut env, ctx.nmr, &rows, &mut possy, &mut spawn_chance);
    }
    let mut env = CharLairEnv {
        cache: &mut cache,
        query: LairQuery {
            profile: Profile::Stock,
            startup: false,
            use_character,
            party: &party,
            global_attack_config: &key.config,
        },
        callouts: CharCallouts {
            world,
            damage: cenv.damage_env(world, &cctx, &state.character, cast),
            vs_party: &vs_party,
            vs_char: &vs_char,
            vs_default: &vs_default,
            sim_rounds,
            sim: &mut sim,
            rnd: &mut rnd,
        },
    };
    let setup = MonsterSetup {
        state,
        cctx: &cctx,
        t_char,
        possy,
        spawn_chance,
        settings,
        sim_rounds,
    };
    Ok(f(&mut env, &setup))
}

/// FilterMonsters with the tab's controls, `AddMonster2LV` for every monster it lists. No question is asked (the
/// default prompt state); a Yes would be the right answer anyway, as the damage arrays here are never calculated.
fn monsters(
    ctx: &Ctx,
    controls: &MonsterControls,
    settings: &ExpPerHourSettings,
    sim_rounds: i32,
    numbers: Option<&[i32]>,
) -> Result<Built, ApiError> {
    let filter = controls.filter();
    with_monster_env(
        ctx,
        controls,
        settings,
        sim_rounds,
        &NoCastText,
        |env, s| {
            if let Some(numbers) = numbers {
                // CompareAddMonster and ReloadMonsterCompare: AddMonster2LV into the compare list with an empty
                // tChar (only lairs fill it, and the compare list never shows lairs), so the row is the plain
                // monster row: not the main list, so no lair mode, and no hp or regen.
                let row_inputs = MonsterRowInputs {
                    use_exp_multi: true,
                    is_main_list: false,
                    settings: s.settings,
                    possy: &s.possy,
                    spawn_chance: &s.spawn_chance,
                    backstab_attack: backstab_attack(s.state.attack_config()),
                };
                let t = CharacterProfile::default();
                let args = MonsterRowArgs::default();
                let rows = numbers
                    .iter()
                    .filter_map(|n| add_monster_row(env, &row_inputs, &t, *n, &args))
                    .collect();
                return Built {
                    rows,
                    headers: Vec::new(),
                    ability_numeric: false,
                    nmr: ctx.nmr,
                    filtered: false,
                };
            }
            let computed = if filter.extras.reachable_only {
                ctx.reach.map(|slot| slot.get_or_compute(ctx.world))
            } else {
                None
            };
            let filter_inputs = MonsterFilterInputs {
                only_in_game: true,
                use_exp_multi: true,
                settings: s.settings,
                possy: &s.possy,
                global_level: f64::from(ctx.who.level),
                reach: match &computed {
                    Some(r) => &**r,
                    None => &AllReachable,
                },
                prompts: DamagePrompts::default(),
            };
            let row_inputs = MonsterRowInputs {
                use_exp_multi: true,
                is_main_list: true,
                settings: s.settings,
                possy: &s.possy,
                spawn_chance: &s.spawn_chance,
                backstab_attack: backstab_attack(s.state.attack_config()),
            };
            let mut rows: Vec<Row> = Vec::new();
            let mut add = |env: &mut CharLairEnv<'_>, a: &MonsterAdd| {
                let mut t = s.t_char.clone();
                t.hp = a.hp;
                t.hp_regen = a.hp_regen;
                rows.extend(add_monster_row(env, &row_inputs, &t, a.number, &a.args));
            };
            let list = filter_monsters(
                env,
                &filter_inputs,
                &filter,
                &s.t_char,
                false,
                &mut |_| Answer::Yes,
                &mut add,
            );
            // FilterMonsters renames columns 11 and 12 (1-based) for the data version and mode
            let headers = list
                .headers
                .map_or_else(Vec::new, |(a, b)| vec![(10, a), (11, b)]);
            Built {
                rows,
                headers,
                ability_numeric: false,
                nmr: ctx.nmr,
                filtered: list.filtered && !list.cancelled,
            }
        },
    )
}

fn sundry(ctx: &Ctx, opts: &Options) -> Built {
    let list = filter_sundry(&filter_env(ctx), &ability_filter(opts));
    let rows = list
        .entries
        .iter()
        .filter_map(|n| add_other_item_row(ctx.world.get_item(*n)?))
        .collect();
    plain(rows)
}

fn plain(rows: Vec<Row>) -> Built {
    Built {
        rows,
        headers: Vec::new(),
        ability_numeric: false,
        nmr: 0.0,
        filtered: false,
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

/// Resolve a request's character and global filter into what the builders read.
///
/// # Errors
/// 422 for a character without class and race or one the pipeline cannot evaluate; 400 for a
/// character that is not well formed.
pub(crate) fn prepare<'a>(world: &'a World, req: &EvaluateBody) -> Result<Ctx<'a>, ApiError> {
    req.options.validate()?;
    if let Some(g) = &req.global_filter {
        g.validate()?;
        if g.use_character && req.character.is_none() {
            return Err(bad("use_character needs a character"));
        }
    }
    let nmr = nmr_ver(&world.info.nmr_version);
    let avg = compute_avg_level_max_stats(Profile::Stock, &world.races).all_stats;

    let mut state = None;
    let mut level_text = "0".to_owned();
    let mut who = GlobalChar::default();
    let mut equipped_items = [0; 20];
    let mut attack = None;
    let mut filter_on = false;
    if let Some(g) = req.global_filter.as_ref().filter(|g| !g.use_character) {
        // the bar's own values: the character, if one came along, stays out of it
        filter_on = true;
        who = GlobalChar {
            level: g.level,
            class: g.class,
            race: g.race,
            alignment: g.alignment,
            min_item_level: g.min_item_level,
        };
    }
    if let Some(dto) = &req.character {
        let mut ch = dto.to_character().map_err(bad)?;
        if ch.class.is_none() || ch.race.is_none() {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                "pick a class and race".into(),
            ));
        }
        attack = Some(ch.attack);
        let (use_char, min_item_level) = match &req.global_filter {
            Some(g) => (g.use_character, g.min_item_level),
            None => (dto.global_filter, dto.options.min_item_level),
        };
        if use_char {
            // The bar's "use character" is MME's `chkGlobalFilter`, whatever the saved flag says.
            ch.global_filter = true;
            let opts = PipelineOptions {
                hide_class_race: dto.options.hide_class_race,
                min_item_level,
                item_manager: dto.gear_rows(world),
                dat_ver_caption: world.info.dat_file_version.clone(),
                nmr_ver: nmr,
                avg_level_max_all_stats: avg,
                party: req.options.monsters.party(),
                ..PipelineOptions::default()
            };
            let st = refresh_all(Profile::Stock, world, &ch, &opts).map_err(|e| {
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("cannot evaluate this character: {e}"),
                )
            })?;
            state = Some(st);
            filter_on = true;
            level_text = ch.level.to_string();
            who = GlobalChar {
                level: ch.level,
                class: ch.class.unwrap_or(0),
                race: ch.race.unwrap_or(0),
                alignment: i32::from(ch.alignment),
                min_item_level,
            };
            equipped_items = ch.equipped;
        }
    }
    let accuracy_tag = state
        .as_ref()
        .and_then(|st| st.stats.stat_tag[10])
        .map_or_else(String::new, vb::cstr);
    Ok(Ctx {
        world,
        nmr,
        avg,
        state,
        level_text,
        accuracy_tag,
        who,
        filter_on,
        equipped_items,
        attack,
        reach: None,
    })
}

/// Build one tab's list for a request.
///
/// # Errors
/// As [`prepare`].
pub fn evaluate(world: &World, tab: Tab, req: &EvaluateBody) -> Result<Table, ApiError> {
    let exp = ExpPerHourSettings::default();
    evaluate_with(world, tab, req, &exp, crate::settings::SIM_ROUNDS_DEFAULT)
}

/// [`evaluate`] with the exp/hr block and the monster simulation rounds the request runs with
/// ([`run_settings`]); only the Monsters list reads them.
///
/// # Errors
/// As [`prepare`].
pub fn evaluate_with(
    world: &World,
    tab: Tab,
    req: &EvaluateBody,
    exp: &ExpPerHourSettings,
    sim_rounds: i32,
) -> Result<Table, ApiError> {
    evaluate_with_reach(world, tab, req, exp, sim_rounds, None)
}

/// [`evaluate_with`] with the load's reachability slot, which "Reachable only" fills on first use.
///
/// # Errors
/// As [`prepare`].
pub fn evaluate_with_reach(
    world: &World,
    tab: Tab,
    req: &EvaluateBody,
    exp: &ExpPerHourSettings,
    sim_rounds: i32,
    reach: Option<&crate::reach::ReachSlot>,
) -> Result<Table, ApiError> {
    let mut ctx = prepare(world, req)?;
    ctx.reach = reach;
    let built = match tab {
        Tab::Weapons => weapons(&ctx, &req.options),
        Tab::Armour => armour(&ctx, &req.options),
        Tab::Spells => spells(&ctx, &req.options),
        Tab::Monsters => monsters(
            &ctx,
            &req.options.monsters,
            exp,
            sim_rounds,
            req.options.numbers.as_deref(),
        )?,
        Tab::Items => sundry(&ctx, &req.options),
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

/// The exp/hr block and the monster simulation rounds a request runs with: `settings.json`, and the open
/// character's own block when "by character" is on. A damaged settings file reads as the defaults.
pub(crate) fn run_settings(
    state: &AppState,
    character: Option<&CharacterDto>,
) -> (ExpPerHourSettings, i32) {
    let settings = state.settings().unwrap_or_default();
    let block = match character {
        None => CharacterBlock::NoCharacter,
        Some(c) => c.exp_hr.map_or(CharacterBlock::Unset, CharacterBlock::Own),
    };
    (
        settings.effective_exp_per_hour(block),
        settings.monster_sim_rounds,
    )
}

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
    let (exp, rounds) = run_settings(&state, req.character.as_ref());
    let key = format!("{tab_name}|{raw}|{exp:?}|{rounds}");
    let load = loaded.info.load_count;
    let body = if let Some(hit) = state.evaluate_cache_get(&key, load) {
        hit
    } else {
        let job = tokio::task::spawn_blocking(move || {
            evaluate_with_reach(&loaded.world, tab, &req, &exp, rounds, Some(&loaded.reach))
                .and_then(|t| {
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
