//! The Character tab's endpoints: evaluating a character with `pipeline::refresh_all`, the bless
//! spell list of `FilterBlessSpells`, and the saved characters in the data directory.
//!
//! The wire structs live here rather than in `touchstone-rules`: its `Character` and state hold
//! 47- and 201-long arrays, which serde does not cover, and the page wants MME's formatted
//! captions beside the numbers. Wire format is snake_case like the rest of the API.

use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use touchstone_model::World;
use touchstone_rules::Profile;
use touchstone_rules::char_panel::ExpRequired;
use touchstone_rules::character::{AttackConfig, AttackType, Character, LearnedSpells};
use touchstone_rules::gear::STAT_COUNT;
use touchstone_rules::mmec::{self, ExtraEntry, ItemStack, Mmec};
use touchstone_rules::pipeline::{CharacterState, PipelineOptions, refresh_all};
use touchstone_rules::spell_cast::UsableEnv;
use touchstone_rules::spells::spell_is_bless_spell;
use touchstone_rules::stats::compute_avg_level_max_stats;
use touchstone_vb6::cstr;

use crate::calc::{ApiResult, Body, bad, json, not_found, vb_error_text};
use crate::detail::nmr_ver;
use crate::settings::ExpHrBlock;
use crate::state::AppState;

/// Longest saved-character name; also bounds the file name.
pub const MAX_NAME_LEN: usize = 64;
/// MME's level and stat boxes hold three digits.
const MAX_BOX: i32 = 999;
const EQUIP_SLOTS: usize = 20;
const BLESS_SLOTS: usize = 10;
const QUESTS: usize = 12;
const QUEST_OPTS: usize = 4;
const LEARNED: usize = 100;

fn unprocessable(msg: impl Into<String>) -> (StatusCode, String) {
    (StatusCode::UNPROCESSABLE_ENTITY, msg.into())
}

// ---------- the character on the wire ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AttackDto {
    /// `eAttackTypeMME` 0..7.
    pub attack_type: i32,
    pub backstab: bool,
    pub backstab_weapon: i32,
    /// 1 punch, 2 kick, 3 jumpkick.
    pub martial_art: i16,
    pub spell_num: i32,
    pub spell_lvl: i16,
    pub manual_p: i32,
    pub manual_m: i32,
    pub use_meditate: bool,
    /// 0 infinite, 1 base, 2 spell, 3 spell at a level, 4 manual.
    pub heal_type: i16,
    pub heal_spell_num: i32,
    pub heal_spell_lvl: i16,
    pub heal_rounds: i16,
    pub heal_manual: i32,
}

impl Default for AttackDto {
    /// The attack and heal choice MME starts with.
    fn default() -> Self {
        Self {
            attack_type: 0,
            backstab: false,
            backstab_weapon: 0,
            martial_art: 0,
            spell_num: 0,
            spell_lvl: 0,
            manual_p: 0,
            manual_m: 0,
            use_meditate: false,
            heal_type: 0,
            heal_spell_num: 0,
            heal_spell_lvl: 5,
            heal_rounds: 30,
            heal_manual: 0,
        }
    }
}

/// A character as the page edits it and the data directory stores it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct CharacterDto {
    pub name: String,
    /// A race or class number (0 is "(none)"); `null` means nothing is picked yet.
    pub race: Option<i32>,
    pub class: Option<i32>,
    pub level: i32,
    /// 0 any, 1 good, 2 neutral, 3 evil.
    pub alignment: i16,
    pub global_filter: bool,
    /// STR INT WIL AGL HEA CHM.
    pub stats: Vec<i32>,
    pub stat_adjustments: Vec<i32>,
    /// 20 slots; 16 is the weapon.
    pub equipped: Vec<i32>,
    pub bless: Vec<i32>,
    pub quests: Vec<bool>,
    /// 2nd alignment, 6th alignment, Dread Wraith, Renfry.
    pub quest_options: Vec<i16>,
    pub mr_text: String,
    pub anti_magic: bool,
    pub add_weight: i32,
    pub use_add_weight: bool,
    pub attack: AttackDto,
    pub learned_spells: Vec<i32>,
    pub learned_class: i16,
    /// Item Manager rows from an imported `.mmec` (`IM_CARRIED`, `IM_STASH`).
    pub carried: Vec<StackDto>,
    pub stash: Vec<StackDto>,
    /// `.mmec` keys touchstone does not model, written back on export.
    pub extra: Vec<ExtraDto>,
    /// This character's own exp/hr models and knobs (MME's per-character `ceph*` keys); used when
    /// "by character" is on in the settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exp_hr: Option<ExpHrBlock>,
    /// View toggles that are not part of a saved character.
    #[serde(skip_serializing)]
    pub options: EvalOptions,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct StackDto {
    pub item: i32,
    pub qty: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtraDto {
    pub section: String,
    pub key: String,
    pub value: String,
}

/// Most `.mmec` keys a character keeps beyond the modelled ones (a real file has about 80).
const MAX_EXTRA: usize = 500;
/// `SaveCharacter` writes at most this many Item Manager rows per list.
const MAX_STACKS: usize = 50;

#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct EvalOptions {
    pub hide_class_race: bool,
    pub min_item_level: i32,
}

/// `v` padded with defaults to `N`; a longer list is a client mistake, not something to cut.
fn fixed<T: Copy + Default, const N: usize>(v: &[T], what: &str) -> Result<[T; N], String> {
    if v.len() > N {
        return Err(format!("{what} has {} entries, at most {N}", v.len()));
    }
    let mut out = [T::default(); N];
    out[..v.len()].copy_from_slice(v);
    Ok(out)
}

fn in_box(n: i32, what: &str) -> Result<i32, String> {
    if (0..=MAX_BOX).contains(&n) {
        Ok(n)
    } else {
        Err(format!("{what} must be 0 to {MAX_BOX}"))
    }
}

impl CharacterDto {
    /// The rules crate's character, or why the request is not one.
    pub fn to_character(&self) -> Result<Character, String> {
        let stats: [i32; 6] = fixed(&self.stats, "stats")?;
        for s in stats {
            in_box(s, "a stat")?;
        }
        if self.stat_adjustments.len() > STAT_COUNT {
            return Err(format!(
                "stat_adjustments has more than {STAT_COUNT} entries"
            ));
        }
        let quest_options: [i16; QUEST_OPTS] = fixed(&self.quest_options, "quest_options")?;
        if quest_options.iter().any(|o| !(-1..=9).contains(o)) {
            return Err("quest_options entries must be -1 to 9".into());
        }
        if !(0..=3).contains(&self.alignment) {
            return Err("alignment must be 0 (any) to 3 (evil)".into());
        }
        let a = &self.attack;
        let attack_type = AttackType::from_code(a.attack_type)
            .ok_or_else(|| format!("attack_type {} is not 0 to 7", a.attack_type))?;
        if !(0..=4).contains(&a.heal_type) {
            return Err("heal_type must be 0 to 4".into());
        }
        let learned: [i32; LEARNED] = fixed(&self.learned_spells, "learned_spells")?;
        Ok(Character {
            name: self.name.clone(),
            race: self.race,
            class: self.class,
            level: in_box(self.level, "level")?,
            alignment: self.alignment,
            global_filter: self.global_filter,
            stats,
            stat_adjustments: self.stat_adjustments.clone(),
            equipped: fixed(&self.equipped, "equipped")?,
            bless: fixed(&self.bless, "bless")?,
            quests: fixed(&self.quests, "quests")?,
            quest_options,
            mr_text: self.mr_text.clone(),
            anti_magic: self.anti_magic,
            add_weight: in_box(self.add_weight, "add_weight")?,
            use_add_weight: self.use_add_weight,
            attack: AttackConfig {
                attack_type,
                backstab: a.backstab,
                backstab_weapon: a.backstab_weapon,
                martial_art: a.martial_art,
                spell_num: a.spell_num,
                spell_lvl: a.spell_lvl,
                manual_p: a.manual_p,
                manual_m: a.manual_m,
                use_meditate: a.use_meditate,
                heal_type: a.heal_type,
                heal_spell_num: a.heal_spell_num,
                heal_spell_lvl: a.heal_spell_lvl,
                heal_rounds: a.heal_rounds,
                heal_manual: a.heal_manual,
                ..AttackConfig::default()
            },
            learned: LearnedSpells {
                spells: learned,
                class: self.learned_class,
            },
        })
    }

    /// The lists at full length without the validation, for a character built here.
    pub(crate) fn padded(mut self) -> Self {
        self.stats.resize(6, 0);
        self.equipped.resize(EQUIP_SLOTS, 0);
        self.bless.resize(BLESS_SLOTS, 0);
        self.quests.resize(QUESTS, false);
        self.quest_options.resize(QUEST_OPTS, 0);
        self.learned_spells.resize(LEARNED, 0);
        self
    }

    /// The same character with every list at full length, as it is stored.
    fn normalized(mut self) -> Result<Self, String> {
        self.to_character()?;
        if self.extra.len() > MAX_EXTRA {
            return Err(format!("extra has more than {MAX_EXTRA} entries"));
        }
        if self.carried.len() > MAX_STACKS || self.stash.len() > MAX_STACKS {
            return Err(format!("carried and stash hold at most {MAX_STACKS} rows"));
        }
        self.exp_hr = self.exp_hr.map(ExpHrBlock::normalized);
        self.stats.resize(6, 0);
        self.equipped.resize(EQUIP_SLOTS, 0);
        self.bless.resize(BLESS_SLOTS, 0);
        self.quests.resize(QUESTS, false);
        self.quest_options.resize(QUEST_OPTS, 0);
        self.learned_spells.resize(LEARNED, 0);
        Ok(self)
    }
}

impl CharacterDto {
    /// The Item Manager rows `CalcCharacterStats` sees: one CARRIED row per saved stack.
    pub fn gear_rows(&self, world: &World) -> Vec<touchstone_rules::gear::ItemManagerRow> {
        let stacks: Vec<ItemStack> = self
            .carried
            .iter()
            .map(|s| ItemStack {
                item: s.item,
                qty: s.qty,
            })
            .collect();
        touchstone_rules::item_manager::gear_rows_for_carried(world, &stacks)
    }

    /// Makes `carried` and `stash` what `SaveCharacter` would write for the Item Manager rows.
    pub fn set_saved_from_rows(
        &mut self,
        rows: &[touchstone_rules::game_text::ListRow],
    ) -> Result<(), String> {
        let (carried, stash) = touchstone_rules::item_manager::saved_stacks(rows)
            .map_err(|e| format!("cannot save the Item Manager: {e}"))?;
        let conv = |v: Vec<ItemStack>| -> Vec<StackDto> {
            v.into_iter()
                .map(|s| StackDto {
                    item: s.item,
                    qty: s.qty,
                })
                .collect()
        };
        self.carried = conv(carried);
        self.stash = conv(stash);
        Ok(())
    }

    /// The file `SaveCharacter` would write for this character.
    pub fn to_mmec(&self) -> Result<Mmec, String> {
        let stacks = |v: &[StackDto]| {
            v.iter()
                .map(|s| ItemStack {
                    item: s.item,
                    qty: s.qty,
                })
                .collect()
        };
        Ok(Mmec {
            character: self.to_character()?,
            carried: stacks(&self.carried),
            stash: stacks(&self.stash),
            extra: self
                .extra
                .iter()
                .map(|e| ExtraEntry {
                    section: e.section.clone(),
                    key: e.key.clone(),
                    value: e.value.clone(),
                })
                .collect(),
        })
    }

    /// A stored character from a loaded file (the name is the caller's to set).
    pub fn from_mmec(m: &Mmec) -> Self {
        let c = &m.character;
        let a = &c.attack;
        let stacks = |v: &[ItemStack]| {
            v.iter()
                .map(|s| StackDto {
                    item: s.item,
                    qty: s.qty,
                })
                .collect()
        };
        Self {
            name: c.name.clone(),
            race: c.race,
            class: c.class,
            level: c.level,
            alignment: c.alignment,
            global_filter: c.global_filter,
            stats: c.stats.to_vec(),
            stat_adjustments: c.stat_adjustments.clone(),
            equipped: c.equipped.to_vec(),
            bless: c.bless.to_vec(),
            quests: c.quests.to_vec(),
            quest_options: c.quest_options.to_vec(),
            mr_text: c.mr_text.clone(),
            anti_magic: c.anti_magic,
            add_weight: c.add_weight,
            use_add_weight: c.use_add_weight,
            attack: AttackDto {
                attack_type: a.attack_type as i32,
                backstab: a.backstab,
                backstab_weapon: a.backstab_weapon,
                martial_art: a.martial_art,
                spell_num: a.spell_num,
                spell_lvl: a.spell_lvl,
                manual_p: a.manual_p,
                manual_m: a.manual_m,
                use_meditate: a.use_meditate,
                heal_type: a.heal_type,
                heal_spell_num: a.heal_spell_num,
                heal_spell_lvl: a.heal_spell_lvl,
                heal_rounds: a.heal_rounds,
                heal_manual: a.heal_manual,
            },
            learned_spells: c.learned.spells.to_vec(),
            learned_class: c.learned.class,
            carried: stacks(&m.carried),
            stash: stacks(&m.stash),
            extra: m
                .extra
                .iter()
                .map(|e| ExtraDto {
                    section: e.section.clone(),
                    key: e.key.clone(),
                    value: e.value.clone(),
                })
                .collect(),
            exp_hr: None,
            options: EvalOptions::default(),
        }
    }
}

// ---------- .mmec import and export ----------

/// Where `InvenEquipItem` puts an item: by its own worn type, not by the key it was saved under.
/// `use_2nd_wrist` is MME's `Use2ndWrist` setting (on by default).
fn equip_slot(item: &touchstone_model::Item, eq: &[i32; 20], use_2nd_wrist: bool) -> Option<usize> {
    let n = item.number;
    match item.item_type {
        1 => Some(16),
        0 => match item.worn {
            1 => Some(19),
            2 => Some(0),
            3 => Some(8),
            4 | 13 => {
                if eq[9] > 0 {
                    if eq[9] == n || eq[10] == n {
                        None
                    } else {
                        Some(10)
                    }
                } else if eq[10] > 0 {
                    if eq[10] == n { None } else { Some(9) }
                } else {
                    Some(9)
                }
            }
            5 => Some(13),
            6 => Some(5),
            7 => Some(3),
            8 => Some(2),
            9 => Some(12),
            10 => Some(11),
            11 => Some(4),
            12 => Some(15),
            14 => {
                if eq[6] > 0 {
                    if eq[6] == n || eq[7] == n {
                        None
                    } else if use_2nd_wrist {
                        Some(7)
                    } else {
                        Some(6)
                    }
                } else if eq[7] > 0 {
                    if eq[7] == n { None } else { Some(6) }
                } else {
                    Some(6)
                }
            }
            15 => Some(1),
            16 => Some(14),
            18 => Some(17),
            19 => Some(18),
            _ => None,
        },
        _ => None,
    }
}

/// The steps of `LoadCharacter` that read game data, applied to a freshly parsed file.
pub fn apply_world(profile: Profile, world: &World, m: &mut Mmec) {
    let c = &mut m.character;
    if c.class
        .is_some_and(|n| n != 0 && world.get_class(n).is_none())
    {
        c.class = None;
    }
    if c.race
        .is_some_and(|n| n != 0 && world.get_race(n).is_none())
    {
        c.race = None;
    }

    let saved = c.equipped;
    c.equipped = [0; 20];
    for slot in mmec::LOAD_ORDER {
        if let Some(item) = world.get_item(saved[slot])
            && let Some(to) = equip_slot(item, &c.equipped, true)
        {
            c.equipped[to] = item.number;
        }
    }
    mmec::fix_attack_for_weapon(c);
    if c.attack.backstab_weapon > 0
        && world
            .get_item(c.attack.backstab_weapon)
            .is_none_or(|i| i.item_type != 1)
    {
        c.attack.backstab_weapon = 0;
    }

    for n in &mut c.bless {
        if *n > 0 && !spell_is_bless_spell(&world.spells, *n) {
            *n = 0;
        }
    }

    c.learned.class = 0;
    match c.class {
        None => c.learned.spells = [0; LEARNED],
        Some(class) if class > 0 => {
            let env = usable_env(profile, world);
            for n in &mut c.learned.spells {
                if *n > 0 {
                    if env.spell_is_usable(*n, class, 0, 0, false) {
                        c.learned.class = i16::try_from(class).unwrap_or(0);
                    } else {
                        *n = 0;
                    }
                }
            }
        }
        Some(_) => {}
    }

    // LV_AddRowByItemNumber: unknown items are skipped, a quantity is 1..9999
    for list in [&mut m.carried, &mut m.stash] {
        list.retain(|s| s.item != 0 && world.get_item(s.item).is_some());
        for s in list.iter_mut() {
            s.qty = if s.qty < 0 { 1 } else { s.qty.min(9999) };
        }
    }
}

pub(crate) fn usable_env(profile: Profile, world: &World) -> UsableEnv<'_> {
    UsableEnv {
        profile,
        spells: &world.spells,
        classes: &world.classes,
        nmr_ver: nmr_ver(&world.info.nmr_version),
        only_in_game: true,
        disable_kai_autolearn: world.info.disable_kai,
    }
}

/// The text before `.mmec` in an uploaded file's name (any directory part dropped).
fn stem(file: &str) -> &str {
    let base = file.rsplit(['/', '\\']).next().unwrap_or(file);
    let lower = base.to_ascii_lowercase();
    if lower.ends_with(".mmec") {
        &base[..base.len() - 5]
    } else {
        base
    }
}

#[derive(Deserialize)]
pub struct ImportQuery {
    /// Save under this name, else the file's PlayerInfo Name, else its file name.
    name: Option<String>,
    /// The uploaded file's name.
    file: Option<String>,
    #[serde(default)]
    overwrite: bool,
}

/// `POST /api/characters/import`: the body is the `.mmec` file.
pub async fn import(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ImportQuery>,
    body: axum::body::Bytes,
) -> ApiResult {
    let mut m = mmec::parse(&body).map_err(|e| unprocessable(e.to_string()))?;
    let loaded = state.current();
    apply_world(loaded.profile, &loaded.world, &mut m);
    let mut dto = CharacterDto::from_mmec(&m);
    let from_file = q.file.as_deref().map(stem).unwrap_or("").trim().to_owned();
    let name = if let Some(n) = q.name.as_deref().filter(|n| !n.is_empty()) {
        validate_name(n).map_err(unprocessable)?;
        n.to_owned()
    } else {
        let own = dto.name.trim().to_owned();
        if validate_name(&own).is_ok() {
            own
        } else {
            validate_name(&from_file).map_err(|e| {
                unprocessable(format!(
                    "the file has no usable character name and its file name is no better: {e}"
                ))
            })?;
            from_file
        }
    };
    let dir = dir(&state);
    if !q.overwrite
        && tokio::fs::try_exists(file_for(&dir, &name))
            .await
            .unwrap_or(false)
    {
        return Err((
            StatusCode::CONFLICT,
            format!("a character named {name:?} already exists"),
        ));
    }
    dto.name.clone_from(&name);
    let dto = dto.normalized().map_err(bad)?;
    store(&dir, &name, &dto).await?;
    json(&dto)
}

fn percent_encode(s: &str) -> String {
    use std::fmt::Write;
    s.bytes().fold(String::new(), |mut o, b| {
        if b.is_ascii_alphanumeric() || b"-_.".contains(&b) {
            o.push(char::from(b));
        } else {
            let _ = write!(o, "%{b:02X}");
        }
        o
    })
}

/// `GET /api/characters/{name}/export.mmec`
pub async fn export(State(state): State<Arc<AppState>>, Path(name): Path<String>) -> ApiResult {
    validate_name(&name).map_err(bad)?;
    let dto = read_dto(&state, &name).await?;
    let bytes = mmec::write(&dto.to_mmec().map_err(bad)?);
    let ascii: String = name
        .chars()
        .map(|c| if c.is_ascii() { c } else { '_' })
        .collect();
    let disposition = format!(
        "attachment; filename=\"{ascii}.mmec\"; filename*=UTF-8''{}.mmec",
        percent_encode(&name)
    );
    Ok((
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/octet-stream".to_owned(),
            ),
            (axum::http::header::CONTENT_DISPOSITION, disposition),
            (axum::http::header::CACHE_CONTROL, "no-cache".to_owned()),
        ],
        bytes,
    )
        .into_response())
}

// ---------- evaluate ----------

#[derive(Serialize)]
struct CpJson {
    stat_costs: [i32; 6],
    total_cost: i32,
    used: Option<i32>,
    avail: Option<i32>,
    level_required: Option<i32>,
    /// "none" (no line), "a_lot", or "value" with `exp`.
    exp_kind: &'static str,
    exp: Option<f64>,
    error: Option<i32>,
    error_text: Option<String>,
}

#[derive(Serialize)]
struct HpJson {
    caption: Option<String>,
    min: Option<i32>,
    max: Option<i32>,
    average: Option<f64>,
    rest_caption: Option<String>,
    rest_normal: Option<i32>,
    rest_resting: Option<i32>,
    regen: f64,
    error_text: Option<String>,
}

#[derive(Serialize)]
struct ManaJson {
    max_caption: Option<String>,
    max_mana: Option<i32>,
    regen_caption: Option<String>,
    mana_regen: Option<i32>,
    medi_ticks: Option<f64>,
    spellcasting_caption: Option<String>,
    spellcasting: Option<i32>,
    /// `txtCharManaRegen` after `CalcCharacterStats`.
    regen: f64,
    error_text: Option<String>,
}

#[derive(Serialize)]
struct EncumJson {
    current: String,
    max: String,
    word: Option<&'static str>,
    caption: Option<String>,
    tag: Option<String>,
}

#[derive(Serialize)]
struct SetupJson {
    quest_options: [i32; 4],
    magery_lvl: i32,
    magery: i32,
    has_picks: bool,
    anti_magic: bool,
    has_magic: bool,
}

#[derive(Serialize)]
struct HealJson {
    value: i32,
    cost: f64,
    rounds: i16,
    spell_lvl: i16,
}

#[derive(Serialize)]
pub struct StateJson {
    setup: Option<SetupJson>,
    quest_options: [i32; 4],
    cps: CpJson,
    /// The six stats with bless and gear applied.
    stat_values: [f64; 6],
    /// "Strength:" or "Str (+3):" per stat.
    stat_labels: Vec<Option<String>>,
    /// The bless bonus alone, per stat.
    stat_bonus: [f64; 6],
    hp: HpJson,
    mana: ManaJson,
    magic_res: Option<i32>,
    anti_magic: bool,
    picklocks_caption: Option<String>,
    picklocks: Option<i32>,
    dodge: f64,
    dodge_caption: String,
    dodge_has_summary: bool,
    encum: EncumJson,
    /// Bless mana per six rounds.
    bless_mana: Option<String>,
    /// `lblInvenCharStat(0..46)` as MME shows them.
    stats: Vec<String>,
    heal: HealJson,
    /// `GetCurrentAttackName`: the Char tab's attack button caption.
    attack_name: String,
    /// The run-time error that cut `CalcCharacterStats` short, if any.
    stats_error_text: Option<String>,
    /// What the calculator windows read from the main form and the character globals.
    calc: CalcJson,
}

/// The `Tag`s of `lblInvenCharStat` (0 while empty) and the `nGlobalChar*` / `nGlobalAttack*` values
/// that frmHitCalc, frmSwingCalc and frmBSCalc read, so the page does not recompute them.
#[derive(Serialize)]
struct CalcJson {
    stat_tags: Vec<f64>,
    shadow_ac: i32,
    vile_ward: i32,
    accy_abilities: i32,
    accy_other: i32,
    accy_items: i32,
    /// `nGlobalAttackAccyAdj`.
    attack_accy_adj: i32,
    /// `nGlobalCharWeaponNumber(0)`, `nGlobalCharWeaponAccy(0)`, `nGlobalCharWeaponBSaccy(0)`.
    weapon_number: i32,
    weapon_accy: i32,
    weapon_bs_accy: i32,
}

fn error_text(code: Option<i32>) -> Option<String> {
    code.map(|c| vb_error_text(u32::try_from(c).unwrap_or(0)))
}

fn state_json(
    profile: Profile,
    world: &World,
    st: &CharacterState,
    ch: &Character,
    hide_record_numbers: bool,
) -> StateJson {
    let s = &st.stats;
    let cps = &st.cps;
    let (exp_kind, exp) = match cps.summary.as_ref().map(|c| c.exp) {
        Some(ExpRequired::ALot) => ("a_lot", None),
        Some(ExpRequired::Value(v)) => ("value", Some(v)),
        _ => ("none", None),
    };
    let hp = &st.hit_points;
    let range = hp.range.as_ref();
    let mg = st.magic.as_ref();
    let (encum_word, encum_caption, encum_tag) = match &st.encum_level {
        Ok(e) => (Some(e.word), Some(e.caption.clone()), Some(e.tag.clone())),
        Err(_) => (None, None, None),
    };
    let anti_magic = st
        .class_setup
        .as_ref()
        .map_or(ch.anti_magic, |c| c.anti_magic);
    StateJson {
        setup: st.class_setup.as_ref().map(|c| SetupJson {
            quest_options: c.quest_options,
            magery_lvl: c.magery_lvl,
            magery: c.magery,
            has_picks: c.has_picks,
            anti_magic: c.anti_magic,
            has_magic: c.has_magic,
        }),
        quest_options: st.quest_options,
        cps: CpJson {
            stat_costs: cps.stat_costs,
            total_cost: cps.total_cost,
            used: cps.summary.as_ref().map(|c| c.used),
            avail: cps.summary.as_ref().map(|c| c.avail),
            level_required: cps.summary.as_ref().map(|c| c.level_required),
            exp_kind,
            exp,
            error: cps.error,
            error_text: error_text(cps.error),
        },
        stat_values: s.stat_with_bonus,
        stat_labels: st.stat_labels.labels.to_vec(),
        stat_bonus: s.label_bonus,
        hp: HpJson {
            caption: hp.max_hp_caption.clone(),
            min: range.map(|r| r.min),
            max: range.map(|r| r.max),
            average: range.map(|r| r.average),
            rest_caption: hp.rest_caption.clone(),
            rest_normal: hp.rest_normal,
            rest_resting: hp.rest_resting,
            regen: s.hp_regen,
            error_text: error_text(hp.error),
        },
        mana: ManaJson {
            max_caption: mg.and_then(|m| m.max_mana_caption.clone()),
            max_mana: mg.and_then(|m| m.max_mana),
            regen_caption: mg.and_then(|m| m.mana_regen_caption.clone()),
            mana_regen: mg.and_then(|m| m.mana_regen),
            medi_ticks: mg.and_then(|m| m.medi_ticks),
            spellcasting_caption: mg.and_then(|m| m.spellcasting_caption.clone()),
            spellcasting: mg.and_then(|m| m.spellcasting),
            regen: s.mana_regen,
            error_text: mg.and_then(|m| error_text(m.error)),
        },
        magic_res: st.magic_res.ok(),
        anti_magic,
        picklocks_caption: st.picklocks.caption.clone(),
        picklocks: st.picklocks.total,
        dodge: st.dodge.value,
        dodge_caption: st.dodge.caption_head.clone(),
        dodge_has_summary: st.dodge.has_summary,
        encum: EncumJson {
            current: s.caption(0),
            max: s.caption(1),
            word: encum_word,
            caption: encum_caption,
            tag: encum_tag,
        },
        bless_mana: st.bless_mana.map(cstr),
        stats: (0..STAT_COUNT).map(|i| s.caption(i)).collect(),
        heal: HealJson {
            value: st.attack_config().heal_value,
            cost: st.attack_config().heal_cost,
            rounds: st.attack_config().heal_rounds,
            spell_lvl: st.attack_config().heal_spell_lvl,
        },
        attack_name: crate::combat_setup::attack_names(
            profile,
            world,
            &st.character,
            st,
            false,
            hide_record_numbers,
        )
        .0,
        stats_error_text: error_text(s.error),
        calc: CalcJson {
            stat_tags: s.stat_tag.iter().map(|t| t.unwrap_or(0.0)).collect(),
            shadow_ac: s.shadow_ac,
            vile_ward: s.vile_ward,
            accy_abilities: s.accy_abils,
            accy_other: s.accy_other,
            accy_items: s.accy_items,
            attack_accy_adj: s.attack_accy_adj,
            weapon_number: s.weapons[0].number,
            weapon_accy: s.weapons[0].accy,
            weapon_bs_accy: s.weapons[0].bs_accy,
        },
    }
}

/// `RefreshAll` over the loaded world, returning the character and the state it left.
pub(crate) fn refresh(
    profile: Profile,
    world: &World,
    dto: &CharacterDto,
    hide_record_numbers: bool,
) -> Result<(Character, CharacterState), (StatusCode, String)> {
    let ch = dto.to_character().map_err(bad)?;
    if ch.class.is_none() || ch.race.is_none() {
        return Err(unprocessable("pick a class and race"));
    }
    let opts = PipelineOptions {
        hide_class_race: dto.options.hide_class_race,
        min_item_level: dto.options.min_item_level,
        item_manager: dto.gear_rows(world),
        dat_ver_caption: world.info.dat_file_version.clone(),
        nmr_ver: nmr_ver(&world.info.nmr_version),
        avg_level_max_all_stats: compute_avg_level_max_stats(profile, &world.races).all_stats,
        hide_record_numbers,
        ..PipelineOptions::default()
    };
    let st = refresh_all(profile, world, &ch, &opts)
        .map_err(|e| unprocessable(format!("cannot evaluate this character: {e}")))?;
    Ok((ch, st))
}

/// `RefreshAll` over the loaded world. Split out so tests can call it without HTTP.
pub fn evaluate(
    profile: Profile,
    world: &World,
    dto: &CharacterDto,
    hide_record_numbers: bool,
) -> Result<StateJson, (StatusCode, String)> {
    let (ch, st) = refresh(profile, world, dto, hide_record_numbers)?;
    Ok(state_json(profile, world, &st, &ch, hide_record_numbers))
}

pub async fn evaluate_route(
    State(state): State<Arc<AppState>>,
    Body(dto): Body<CharacterDto>,
) -> ApiResult {
    let loaded = state.current();
    let profile = loaded.profile;
    json(&evaluate(
        profile,
        &loaded.world,
        &dto,
        state.hide_record_numbers(),
    )?)
}

// ---------- bless spells ----------

#[derive(Deserialize)]
pub struct BlessQuery {
    class: Option<i32>,
    level: Option<i32>,
    alignment: Option<i32>,
    /// `FilterBlessSpells(bUNfilter:=True)`: every bless spell, whoever can use it.
    #[serde(default)]
    all: bool,
    /// Comma-separated spell numbers a slot already holds; `EquipBlessSpell` adds them when they
    /// are bless spells the filter left out.
    include: Option<String>,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct BlessChoice {
    pub number: i32,
    /// MME's combo text, "Name (number)".
    pub label: String,
}

/// Port of frmMain `FilterBlessSpells`, with `bOnlyInGame` on as MME defaults.
pub fn bless_spells(
    profile: Profile,
    world: &World,
    class: i32,
    level: i32,
    alignment: i32,
    all: bool,
    include: &[i32],
) -> Vec<BlessChoice> {
    let env = UsableEnv {
        profile,
        spells: &world.spells,
        classes: &world.classes,
        nmr_ver: nmr_ver(&world.info.nmr_version),
        only_in_game: true,
        disable_kai_autolearn: world.info.disable_kai,
    };
    let level = i16::try_from(level.clamp(0, i32::from(i16::MAX))).unwrap_or(0);
    let align = i16::try_from(alignment.clamp(0, 3)).unwrap_or(0);
    let label = |s: &touchstone_model::Spell| BlessChoice {
        number: s.number,
        label: format!("{} ({})", s.name.as_deref().unwrap_or(""), s.number),
    };
    let mut out = Vec::new();
    for s in &world.spells {
        if !env.spell_is_in_game(s.number) {
            continue;
        }
        // the first block of the procedure is SpellIsBlessSpell's rule
        let timed = s.dur != 0 || (s.dur_inc != 0 && s.dur_inc_lvls != 0);
        let taught = s.learnable
            || s.learned_from.as_deref().is_some_and(|f| !f.is_empty())
            || (s.magery == 5 && s.req_level > 0);
        if taught && !(matches!(s.targets, 1 | 2 | 5 | 13) && timed) {
            continue;
        }
        if !all && !env.spell_is_usable(s.number, class, level, align, true) {
            continue;
        }
        out.push(label(s));
    }
    for &n in include {
        if !out.iter().any(|c| c.number == n)
            && spell_is_bless_spell(&world.spells, n)
            && let Some(s) = world.get_spell(n)
        {
            out.push(label(s));
        }
    }
    out
}

pub async fn bless_spells_route(
    State(state): State<Arc<AppState>>,
    Query(q): Query<BlessQuery>,
) -> ApiResult {
    let mut include = Vec::new();
    for part in q.include.as_deref().unwrap_or("").split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        include.push(
            part.parse::<i32>()
                .map_err(|_| bad(format!("include: {part:?} is not a spell number")))?,
        );
    }
    let loaded = state.current();
    let profile = loaded.profile;
    json(&bless_spells(
        profile,
        &loaded.world,
        q.class.unwrap_or(0),
        q.level.unwrap_or(0),
        q.alignment.unwrap_or(0),
        q.all,
        &include,
    ))
}

// ---------- saved characters ----------

/// Why a name cannot be a file name here.
pub fn validate_name(name: &str) -> Result<(), String> {
    let n = name.chars().count();
    if n == 0 {
        return Err("a character needs a name".into());
    }
    if n > MAX_NAME_LEN {
        return Err(format!("a name is at most {MAX_NAME_LEN} characters"));
    }
    if let Some(c) = name
        .chars()
        .find(|c| c.is_control() || "/\\:*?\"<>|".contains(*c))
    {
        return Err(format!("a name cannot contain {c:?}"));
    }
    if name.starts_with('.') {
        return Err("a name cannot start with a dot".into());
    }
    if name.ends_with(' ') || name.ends_with('.') || name.starts_with(' ') {
        return Err("a name cannot start with a space or end with a space or dot".into());
    }
    Ok(())
}

fn dir(state: &AppState) -> PathBuf {
    state.data_dir().join("characters")
}

fn file_for(dir: &FsPath, name: &str) -> PathBuf {
    dir.join(format!("{name}.json"))
}

fn io_fail(e: &std::io::Error) -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("character store: {e}"),
    )
}

#[derive(Serialize)]
struct Summary {
    name: String,
    race: Option<i32>,
    class: Option<i32>,
    level: i32,
}

pub async fn list(State(state): State<Arc<AppState>>) -> ApiResult {
    let dir = dir(&state);
    let mut rd = match tokio::fs::read_dir(&dir).await {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return json(&Vec::<Summary>::new()),
        Err(e) => return Err(io_fail(&e)),
    };
    let mut out = Vec::new();
    while let Some(entry) = rd.next_entry().await.map_err(|e| io_fail(&e))? {
        let path = entry.path();
        let Some(name) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".json"))
            .filter(|n| validate_name(n).is_ok())
        else {
            continue;
        };
        let Ok(bytes) = tokio::fs::read(&path).await else {
            continue;
        };
        if let Ok(dto) = serde_json::from_slice::<CharacterDto>(&bytes) {
            out.push(Summary {
                name: name.to_owned(),
                race: dto.race,
                class: dto.class,
                level: dto.level,
            });
        }
    }
    out.sort_by_key(|s| (s.name.to_lowercase(), s.name.clone()));
    json(&out)
}

async fn read_dto(state: &AppState, name: &str) -> Result<CharacterDto, (StatusCode, String)> {
    match tokio::fs::read(file_for(&dir(state), name)).await {
        Ok(bytes) => {
            let mut dto: CharacterDto = serde_json::from_slice(&bytes).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("saved character {name:?} is damaged: {e}"),
                )
            })?;
            dto.name = name.to_owned();
            Ok(dto)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(not_found(format!("no character {name:?}")))
        }
        Err(e) => Err(io_fail(&e)),
    }
}

/// A stored character, or `None` when there is no such file.
pub async fn read_dto_opt(
    state: &AppState,
    name: &str,
) -> Result<Option<CharacterDto>, (StatusCode, String)> {
    match read_dto(state, name).await {
        Ok(dto) => Ok(Some(dto)),
        Err((StatusCode::NOT_FOUND, _)) => Ok(None),
        Err(e) => Err(e),
    }
}

pub async fn get(State(state): State<Arc<AppState>>, Path(name): Path<String>) -> ApiResult {
    validate_name(&name).map_err(bad)?;
    json(&read_dto(&state, &name).await?)
}

static TMP: AtomicU64 = AtomicU64::new(0);

pub async fn put(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Body(dto): Body<CharacterDto>,
) -> ApiResult {
    validate_name(&name).map_err(bad)?;
    let mut dto = dto.normalized().map_err(bad)?;
    dto.name.clone_from(&name);
    store(&dir(&state), &name, &dto).await?;
    json(&dto)
}

/// Atomic write of a stored character.
async fn store(dir: &FsPath, name: &str, dto: &CharacterDto) -> Result<(), (StatusCode, String)> {
    let bytes = serde_json::to_vec_pretty(dto)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| io_fail(&e))?;
    // The temp file is in the same directory so the rename cannot cross filesystems, and starts
    // with a dot so a listing never mistakes it for a character.
    let tmp = dir.join(format!(
        ".{}-{}.tmp",
        std::process::id(),
        TMP.fetch_add(1, Ordering::Relaxed)
    ));
    let result = async {
        tokio::fs::write(&tmp, &bytes).await?;
        tokio::fs::rename(&tmp, file_for(dir, name)).await
    }
    .await;
    if let Err(e) = result {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(io_fail(&e));
    }
    Ok(())
}

pub async fn delete(State(state): State<Arc<AppState>>, Path(name): Path<String>) -> ApiResult {
    validate_name(&name).map_err(bad)?;
    match tokio::fs::remove_file(file_for(&dir(&state), &name)).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT.into_response()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(not_found(format!("no character {name:?}")))
        }
        Err(e) => Err(io_fail(&e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        for ok in ["Bob", "Mr. Bob", "a-b_c", "Zoë", "x"] {
            assert!(validate_name(ok).is_ok(), "{ok}");
        }
        for bad in [
            "", ".", "..", ".hidden", "a/b", "a\\b", "..\\x", "a:b", "a.", "a ", " a", "a\nb",
            "a\0b", "con*",
        ] {
            assert!(validate_name(bad).is_err(), "{bad:?}");
        }
        assert!(validate_name(&"x".repeat(MAX_NAME_LEN)).is_ok());
        assert!(validate_name(&"x".repeat(MAX_NAME_LEN + 1)).is_err());
    }

    #[test]
    fn short_lists_pad_and_long_ones_fail() {
        let dto = CharacterDto {
            stats: vec![1, 2],
            equipped: vec![0; 21],
            ..CharacterDto::default()
        };
        assert!(dto.to_character().is_err());
        let dto = CharacterDto {
            stats: vec![1, 2],
            ..CharacterDto::default()
        };
        assert_eq!(dto.to_character().unwrap().stats, [1, 2, 0, 0, 0, 0]);
        assert_eq!(dto.normalized().unwrap().bless.len(), 10);
    }
}
