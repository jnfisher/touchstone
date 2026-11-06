//! The Monsters detail pane (FAM-20, monsters). Cards P3-32 to P3-34.
//!
//! [`pull_monster_detail`] ports modMain `PullMonsterDetail` and `AddMonsterDamageOutText`: the rows MME
//! puts into the detail list when a monster is selected. The attributes, abilities, drops and attacks come
//! from the record; the damage lines run the character's attack choice against the monster (and against
//! its average lair), and the scripting estimate runs `CalcExpPerHour`. A row is a [`MonsterRow`]: its
//! label, the tag MME gave the row (the kind of record it points at), the colour and bold flag, and the
//! value cell, all with MME's exact text. Colours are the light theme's (`TColor` is the identity).
//!
//! The reachability rows (`AddMonsterReachRows`) belong to an optional module: they come from a
//! [`ReachRows`] and the default gives none. "Spawns via ..." runs `GetLocations` with the arguments
//! `LookUpMonsterRegen` uses (two columns, not sorted), so no sort is asked for.
//!
//! A VB error stops the procedure where it happens and the rows built so far stay, as `Resume out` does.

use touchstone_model::{Monster, World};
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Vb6Error};

use crate::abilities::{item_has_ability, spell_has_ability};
use crate::ability_text::{AbilityEnv, get_item_name, get_monster_name, get_spell_name};
use crate::attack::AttackRequest;
use crate::character::{AttackType, AttackTypeMud, CharacterProfile};
use crate::combat::{
    AR_ANIMAL, AR_LIVING, AR_NONE, AR_UNDEAD, AR_UNKNOWN, CombatRoundsArgs, DF_ANIMAL,
    DF_ANTI_MAGIC, DF_LIVING, DF_UNDEAD, DamageEnv, DamageRequest, attack_with, calc_combat_rounds,
    get_damage_output, populate,
};
use crate::exp_per_hour::{ExpPerHourInfo, ExpPerHourInput, ExpPerHourSettings, calc_exp_per_hour};
use crate::lairs::{
    CharCallouts, CharLairEnv, LAIR_FLAG_RATIO, LairEnv, LairInfo, LairLocsInput, LairQuery,
    get_lair_averages_from_locs, instr_count,
};
use crate::locations::{LocationArgs, LocationLairs, LocationList, LocationOptions, get_locations};
use crate::monster_glue::{
    CharSimInputs, GlueEnv, calculate_monster_avg_dmg, calculate_monster_damage_vs_char,
};
use crate::spell_cast::{CastRequest, SpellCastEnv, SpellCastValues};
use crate::spells::{PullArgs, get_spell_duration, spell_is_area_attack};
use crate::{Profile, RulesError};

type Res<T> = Result<T, Vb6Error>;

const RED: i32 = 204;
const DARK_RED: i32 = 0xC0;
const GREEN: i32 = 0x8000;
const ORANGE: i32 = 0x80FF;
const BLUE: i32 = 0xCC_6600;
const PURPLE: i32 = 0x80_0080;
const PINK: i32 = 0x40C0;
/// `RGB(144, 4, 214)`.
const VS_CHAR: i32 = 144 + 4 * 256 + 214 * 65_536;
/// `&H80000015` (a system colour index).
const SYS_GREY: i32 = -2_147_483_627;

/// What a row points at (MME's `ListItem.Tag`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonsterRowKind {
    Plain,
    Item,
    Spell,
    Monster,
    GreetText,
    /// A row of the "Spawns via ..." list (rooms, lairs, ...): the tag is `map/room` or a record.
    Location,
}

/// A sub-item of a row (MME's `ListSubItems(1)`, the "Detail" column).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MonsterCell {
    pub text: String,
    /// `None`: MME never assigned the Tag.
    pub tag: Option<String>,
    pub fore_color: Option<i32>,
    pub bold: bool,
}

/// A `ListItem` of the detail list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MonsterRow {
    /// The `Text` column.
    pub label: String,
    /// `ListItem.Tag` as MME assigned it: the kind of record the row points at.
    pub tag: Option<String>,
    pub fore_color: Option<i32>,
    pub bold: bool,
    pub cells: Vec<MonsterCell>,
}

impl MonsterRow {
    /// The kind the tag names.
    #[must_use]
    pub fn kind(&self) -> MonsterRowKind {
        match self.tag.as_deref() {
            None => MonsterRowKind::Plain,
            Some("Item") => MonsterRowKind::Item,
            Some("Spell") => MonsterRowKind::Spell,
            Some("Monster" | "monster") => MonsterRowKind::Monster,
            Some("greet_text") => MonsterRowKind::GreetText,
            Some(_) => MonsterRowKind::Location,
        }
    }

    fn tag(&mut self, t: impl Into<String>) -> &mut Self {
        self.tag = Some(t.into());
        self
    }
    fn color(&mut self, c: i32) -> &mut Self {
        self.fore_color = Some(c);
        self
    }
    fn bold(&mut self) -> &mut Self {
        self.bold = true;
        self
    }
    /// `ListSubItems.Add (1), "Detail", text`.
    fn text(&mut self, t: impl Into<String>) -> &mut Self {
        self.cells.push(MonsterCell {
            text: t.into(),
            ..MonsterCell::default()
        });
        self
    }
    fn cell(&mut self) -> Res<&mut MonsterCell> {
        // `ListSubItems(1)` before an Add is VB error 35600
        self.cells.last_mut().ok_or(Vb6Error::InvalidProcedureCall)
    }
    fn vtag(&mut self, t: impl Into<String>) -> Res<&mut Self> {
        self.cell()?.tag = Some(t.into());
        Ok(self)
    }
    fn vcolor(&mut self, c: i32) -> Res<&mut Self> {
        self.cell()?.fore_color = Some(c);
        Ok(self)
    }
    fn vbold(&mut self, b: bool) -> Res<&mut Self> {
        self.cell()?.bold = b;
        Ok(self)
    }
    fn vappend(&mut self, t: &str) -> Res<&mut Self> {
        self.cell()?.text.push_str(t);
        Ok(self)
    }
}

/// The names the oracle tests (and the other panes' `DetailRow`) know these by; `detail::DetailRow` is the label/value
/// row of the other panes, this module's rows carry a tag, colours and sub-items.
pub type DetailRow = MonsterRow;
pub type DetailCell = MonsterCell;
pub type DetailKind = MonsterRowKind;

/// The rows the reachability module adds (`AddMonsterReachRows`); that module is optional.
pub trait ReachRows {
    fn rows(&self, monster: i32) -> Vec<MonsterRow>;
}

/// No reachability module: no rows.
pub struct NoReach;

impl ReachRows for NoReach {
    fn rows(&self, _monster: i32) -> Vec<MonsterRow> {
        Vec::new()
    }
}

/// `nMonsterDamageVsChar` and `nMonsterDamageVsParty`, indexed by monster number (-1: not calculated). The pane
/// writes them (a simulation on the spot, the old-version fill-in), and later panes and lair averages read what it wrote,
/// so one value lives as long as the list does.
#[derive(Debug, Clone, Default)]
pub struct DamageCaches {
    pub vs_party: Vec<Currency>,
    pub vs_char: Vec<Currency>,
}

fn store(a: &mut [Currency], n: i32, v: Currency) {
    if let Some(slot) = usize::try_from(n).ok().and_then(|i| a.get_mut(i)) {
        *slot = v;
    }
}

/// Runs `f` on a copy of the lair env whose damage caches are `caches` (the callees read the caches through the env).
fn with_caches<R>(
    env: &mut CharLairEnv<'_>,
    caches: &DamageCaches,
    f: impl FnOnce(&mut CharLairEnv<'_>) -> R,
) -> R {
    let d = &env.callouts.damage;
    let mut e2 = CharLairEnv {
        cache: &mut *env.cache,
        query: LairQuery {
            profile: env.query.profile,
            startup: env.query.startup,
            use_character: env.query.use_character,
            party: env.query.party,
            global_attack_config: env.query.global_attack_config,
        },
        callouts: CharCallouts {
            world: env.callouts.world,
            damage: DamageEnv {
                ctx: d.ctx,
                character: d.character,
                races: d.races,
                monsters: d.monsters,
                equipped: d.equipped,
                cast_text: d.cast_text,
                dat_ver_caption: d.dat_ver_caption,
                accuracy_tag: d.accuracy_tag,
                damage_out_text: d.damage_out_text,
                party_swings_text: d.party_swings_text,
            },
            vs_party: &caches.vs_party,
            vs_char: &caches.vs_char,
            vs_default: env.callouts.vs_default,
            sim_rounds: env.callouts.sim_rounds,
            sim: &mut *env.callouts.sim,
            rnd: &mut *env.callouts.rnd,
        },
    };
    f(&mut e2)
}

/// Globals and controls `PullMonsterDetail` reads besides what the lair env holds.
pub struct MonsterDetailInputs<'a> {
    /// `UseExpMulti`.
    pub use_exp_multi: bool,
    /// `frmMain.bAutoCalcMonDamage`: simulate the damage on the spot.
    pub auto_calc_mon_damage: bool,
    /// `frmMain.bDontLookupMonRegen`.
    pub dont_lookup_mon_regen: bool,
    /// `bMobPrintCharDamageOutFirst`.
    pub print_char_damage_out_first: bool,
    /// `bHideRecordNumbers`.
    pub hide_record_numbers: bool,
    /// `nGlobalMonsterSimRounds`.
    pub sim_rounds: i32,
    /// The exp/hr settings (`bGlobal_ceph*`).
    pub settings: &'a ExpPerHourSettings,
    /// `nMonsterPossy` and `nMonsterSpawnChance`, indexed by monster number.
    pub possy: &'a [Currency],
    pub spawn_chance: &'a [Currency],
    /// `sMonsterDamageVsCharDefenseConfig` and `sGlobalCharDefenseDescription`.
    pub vs_char_defense_config: &'a str,
    pub char_defense_description: &'a str,
    /// `bMonsterDamageVsPartyCalculated` and `bDontPromptCalcPartyMonsterDamage`.
    pub party_damage_calculated: bool,
    pub dont_prompt_calc_party: bool,
    /// `Val(frmMain.lblCharBless.Caption)`.
    pub bless_caption: f64,
    /// `frmMain.txtCharStats(5).Tag` (the charm, for the locations list).
    pub charm_tag: &'a str,
    /// The controls `CalculateMonsterDamageVsChar` reads for the loaded character.
    pub sim_chr: &'a CharSimInputs,
    pub reach: &'a dyn ReachRows,
}

struct Lv {
    rows: Vec<MonsterRow>,
}

impl Lv {
    /// `DetailLV.ListItems.Add()` with its `Text`.
    fn add(&mut self, label: impl Into<String>) -> &mut MonsterRow {
        self.rows.push(MonsterRow {
            label: label.into(),
            ..MonsterRow::default()
        });
        self.rows.last_mut().expect("just pushed")
    }
    fn blank(&mut self) {
        self.add("");
    }
}

fn cur(v: i32) -> Currency {
    Currency::from_scaled(i64::from(v) * 10_000)
}

fn cur_f(x: f64) -> Res<Currency> {
    Currency::from_f64(x)
}

/// `CStr` of a Currency.
fn cur_text(c: Currency) -> String {
    let n = c.scaled();
    let sign = if n < 0 { "-" } else { "" };
    let n = n.unsigned_abs();
    let (whole, frac) = (n / 10_000, n % 10_000);
    if frac == 0 {
        format!("{sign}{whole}")
    } else {
        format!(
            "{sign}{whole}.{}",
            format!("{frac:04}").trim_end_matches('0')
        )
    }
}

fn cur_round(c: Currency, digits: i32) -> Res<Currency> {
    Currency::from_f64(vb::round(c.to_f64(), digits)?)
}

/// modSyntaxsFunc `RoundUp`.
fn round_up(x: f64) -> f64 {
    if 0.0 < x - vb::int(x) {
        vb::int(x) + 1.0
    } else {
        vb::int(x)
    }
}

/// `IIf(x > 0, Format(x, pattern), 0)`.
fn fmt_pos(x: f64, pattern: &str) -> Res<String> {
    if x > 0.0 {
        vb::format(x, pattern)
    } else {
        Ok("0".to_string())
    }
}

fn auto_append(base: &str, add: &str, glue: &str) -> String {
    if base.is_empty() {
        add.to_string()
    } else if add.trim_matches(' ').is_empty() {
        base.to_string()
    } else {
        format!("{base}{glue}{add}")
    }
}

/// modSyntaxsFunc `PutCommas` (without the shorten flag).
fn put_commas(number: &str) -> String {
    crate::rows::monster::put_commas(number)
}

fn i16c(v: i32) -> Res<i16> {
    i16::try_from(v).map_err(|_| Vb6Error::Overflow)
}

fn clng(c: Currency) -> Res<i32> {
    vb::clng(c.to_f64())
}

fn rt(e: RulesError) -> Vb6Error {
    match e {
        RulesError::Vb6(_) | RulesError::Unsupported(_) => Vb6Error::Overflow,
    }
}

fn contains_ci(hay: &str, needle: &str) -> bool {
    hay.to_lowercase().contains(&needle.to_lowercase())
}

/// An array read of one monster's entry; an index outside the array is VB error 9, which ends the pane.
fn at(a: &[Currency], n: i32) -> Res<Currency> {
    usize::try_from(n)
        .ok()
        .and_then(|i| a.get(i))
        .copied()
        .ok_or(Vb6Error::Overflow)
}

/// `SpellHasAbility`.
fn spell_ability(world: &World, spell: i32, ability: i16) -> i32 {
    let slots = world.get_spell(spell).map(|s| &s.abilities[..]);
    spell_has_ability(Profile::Stock, slots, spell, ability)
}

/// The colour (and bold) MME gives a spell text by what the spell does. `slay` adds the slay check and
/// `illu` the illusion one; the death spell has neither.
fn spell_style(world: &World, spell: i32, slay: bool, illu: bool) -> (Option<i32>, bool) {
    if spell_ability(world, spell, 60) >= 0 {
        (Some(DARK_RED), true)
    } else if spell_ability(world, spell, 19) >= 0 {
        (Some(GREEN), true)
    } else if spell_ability(world, spell, 71) >= 0 {
        (Some(ORANGE), true)
    } else if slay && spell_ability(world, spell, 95) >= 0 {
        (Some(DARK_RED), true)
    } else if illu && spell_ability(world, spell, 13) <= -999 {
        (None, true)
    } else {
        (None, false)
    }
}

fn apply_style(row: &mut MonsterRow, st: (Option<i32>, bool)) -> Res<()> {
    if let Some(c) = st.0 {
        row.vcolor(c)?;
    }
    if st.1 {
        row.vbold(true)?;
    }
    Ok(())
}

fn mon_type(n: i32) -> String {
    match n {
        0 => "Solo".into(),
        1 => "Leader".into(),
        2 => "Follower".into(),
        3 => "Stationary".into(),
        _ => format!("Unknown ({n})"),
    }
}

fn mon_alignment(n: i32) -> String {
    match n {
        0 => "Good".into(),
        1 => "Evil".into(),
        2 => "Chaotic Evil".into(),
        3 => "Neutral".into(),
        4 => "Lawful Good".into(),
        5 => "Neutral Evil".into(),
        6 => "Lawful Evil".into(),
        _ => format!("Unknown ({n})"),
    }
}

fn spell_targets(n: i32) -> String {
    match n {
        0 => "User".into(),
        1 => "Self".into(),
        2 => "Self or User".into(),
        3 => "Divided Area (not self)".into(),
        4 => "Monster".into(),
        5 => "Divided Area (incl self)".into(),
        6 => "Any".into(),
        7 => "Item".into(),
        8 => "Monster or User".into(),
        9 => "Divided Attack Area".into(),
        10 => "Divided Party Area".into(),
        11 => "Full Area".into(),
        12 => "Full Attack Area".into(),
        13 => "Full Party Area".into(),
        _ => format!("Unknown ({n})"),
    }
}

/// Answers `GetLairInfo` for `GetLocations` from the lair env.
struct LairAdapter<'a, 'b>(&'a mut CharLairEnv<'b>);

impl LocationLairs for LairAdapter<'_, '_> {
    fn get_lair_info(&mut self, group_index: &str, max_regen: i16) -> LairInfo {
        LairEnv::get_lair_info(self.0, group_index, max_regen)
    }
}

/// The parts of `tDamageOutput`/`tAttackDamage` the pane keeps between the two passes (mob, then lair).
#[derive(Default)]
struct Pass {
    damage_out: i32,
    first_round: i32,
    min_damage: i32,
    surprise: i32,
    surprise_min: i32,
    surprise_chance: i16,
}

/// Port of modMain `PullMonsterDetail`: the detail rows of monster `number`. `ab` supplies the spell and
/// ability text (its `hide_record_numbers` is `bHideRecordNumbers`). `limit` is `nLookupLimit` (MME's
/// default is 100).
#[must_use]
pub fn pull_monster_detail(
    env: &mut CharLairEnv<'_>,
    ab: &AbilityEnv<'_>,
    inp: &MonsterDetailInputs<'_>,
    caches: &mut DamageCaches,
    number: i32,
    limit: i16,
) -> Vec<MonsterRow> {
    let mut lv = Lv { rows: Vec::new() };
    let world = env.callouts.world;
    let Some(m) = world.get_monster(number) else {
        lv.add("Monster not found");
        return lv.rows;
    };
    let _ = detail_steps(env, ab, inp, caches, m, number, limit, &mut lv);
    lv.rows
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn detail_steps(
    env: &mut CharLairEnv<'_>,
    ab: &AbilityEnv<'_>,
    inp: &MonsterDetailInputs<'_>,
    caches: &mut DamageCaches,
    m: &Monster,
    number: i32,
    limit: i16,
    lv: &mut Lv,
) -> Res<()> {
    let world = env.callouts.world;
    let ctx = env.callouts.damage.ctx;
    let character = env.callouts.damage.character;
    let nmr = ctx.nmr_ver;
    let hide = inp.hide_record_numbers;
    let use_char = character.global_filter;
    let lair_mode = env.query.party.lair_mode;
    let atype = character.attack.attack_type;

    // PopulateCharacterProfile: the profile of the list (a party's, when there is one) and the forced one
    let mut t_char = CharacterProfile::default();
    populate(
        &env.callouts.damage,
        &mut t_char,
        false,
        false,
        AttackTypeMud::None,
        0,
    );
    let ma = i32::from(character.attack.martial_art);
    let ma_mud =
        AttackTypeMud::from_code(if ma > 1 { ma } else { 1 }).unwrap_or(AttackTypeMud::Punch);
    let forced = if atype == AttackType::MartialArts && use_char {
        let mut f = CharacterProfile::default();
        populate(&env.callouts.damage, &mut f, true, true, ma_mud, 0);
        f
    } else if t_char.party > 1 && use_char {
        let mut f = CharacterProfile::default();
        populate(
            &env.callouts.damage,
            &mut f,
            true,
            true,
            AttackTypeMud::None,
            0,
        );
        f
    } else if atype == AttackType::MartialArts && t_char.party == 1 {
        let mut f = CharacterProfile::default();
        populate(&env.callouts.damage, &mut f, use_char, true, ma_mud, 0);
        f
    } else {
        t_char.clone()
    };

    // ---- attributes
    {
        let r = lv.add("Name");
        r.bold();
        r.text(format!("{} ({number})", m.name.clone().unwrap_or_default()));
        r.vbold(true)?;
    }
    let exp = if inp.use_exp_multi {
        cur_f(m.exp * m.exp_multi)?
    } else {
        cur_f(m.exp)?
    };
    lv.add("Experience").text(fmt_pos(exp.to_f64(), "#,#")?);

    if m.regen_time != 0 {
        lv.add("Regen Time")
            .text(format!("{} hour(s)", m.regen_time));
    }
    if nmr >= 1.83 && m.game_limit != 0 {
        let r = lv.add("Game Limit");
        if m.regen_time == 0 {
            r.color(RED);
        }
        r.text(m.game_limit.to_string());
        if m.regen_time == 0 {
            r.vcolor(RED)?;
        }
    }
    lv.add("Type").text(mon_type(m.r#type));

    let mut sa = mon_alignment(m.align);
    match m.align {
        0 | 3 | 4 => sa.push_str(" [Not-Hostile]"),
        6 => {
            if use_char && character.alignment == 3 {
                sa.push_str(" [Not-Hostile to EVIL]");
            } else {
                sa.push_str(" [Hostile]");
            }
        }
        _ => sa.push_str(" [Hostile]"),
    }
    lv.add("Alignment").text(sa);

    let mut mob_flags: i32 = 0;
    if m.undead == 1 {
        lv.add("Undead").text("Yes");
        mob_flags |= DF_UNDEAD;
    }
    lv.add("HPs").text(format!(
        "{} (Regens: {} HPs every 90 seconds [18 rounds])",
        m.hp, m.hp_regen
    ));
    lv.add("AC/DR")
        .text(format!("{}/{}", m.armour_class, m.damage_resist));
    lv.add("MR").text(m.magic_res.to_string());
    lv.add("Follow %").text(m.follow_pct.to_string());
    lv.add("Charm LVL").text(m.charm_lvl.to_string());

    // cash
    let mut cash = cur(vb::mul_i32(m.r, 1_000_000)?);
    cash = cash.checked_add(cur(vb::mul_i32(m.p, 10_000)?))?;
    cash = cash.checked_add(cur(vb::mul_i32(m.g, 100)?))?;
    cash = cash.checked_add(cur(vb::mul_i32(m.s, 10)?))?;
    cash = cash.checked_add(cur(m.c))?;
    let mut db_coin = String::new();
    for (v, ch) in [(m.r, "R"), (m.p, "P"), (m.g, "G"), (m.s, "S"), (m.c, "C")] {
        if v > 0 {
            db_coin = auto_append(&db_coin, &format!("{v}{ch}"), "/");
        }
    }
    let mut coin_name = "Copper".to_string();
    let mut reduced = cash;
    if reduced.scaled() > 0 {
        if cash.scaled() >= 1_000_000 * 10_000 {
            reduced = cash.checked_div(cur(1_000_000))?;
            coin_name = "Runic".into();
        } else if cash.scaled() >= 10_000 * 10_000 {
            reduced = cash.checked_div(cur(10_000))?;
            coin_name = "Platinum".into();
        } else if cash.scaled() >= 100 * 10_000 {
            reduced = cash.checked_div(cur(100))?;
            coin_name = "Gold".into();
        } else if cash.scaled() >= 10 * 10_000 {
            reduced = cash.checked_div(cur(10))?;
            coin_name = "Silver".into();
        }
        if coin_name != "Copper" {
            reduced = cur_round(reduced, 2)?;
        }
        let mut s_cash = vb::format(reduced.to_f64(), "##,##0.00")?;
        if s_cash.ends_with(".00") {
            s_cash.truncate(s_cash.len() - 3);
        }
        if !db_coin.is_empty()
            && ((m.r > 0 && !contains_ci(&coin_name, "Runic"))
                || (m.p > 0 && !contains_ci(&coin_name, "Platinum"))
                || (m.g > 0 && !contains_ci(&coin_name, "Gold"))
                || (m.s > 0 && !contains_ci(&coin_name, "Silver"))
                || (m.c > 0 && !contains_ci(&coin_name, "Copper")))
        {
            coin_name = format!("{coin_name} ({db_coin})");
        }
        lv.add("Cash (up to)").text(format!("{s_cash} {coin_name}"));
    }

    if m.weapon != 0 {
        let r = lv.add("Weapon");
        r.tag("Item");
        r.text(get_item_name(ab.names, m.weapon, hide));
        r.vtag(m.weapon.to_string())?;
    }

    for (spell, label) in [
        (m.create_spell, "Create Spell:"),
        (m.death_spell, "Death Spell"),
    ] {
        if spell > 0 {
            let eq = ab.pull_spell_eq(&PullArgs {
                spell,
                ..PullArgs::default()
            });
            let r = lv.add(label);
            r.tag("Spell");
            r.text(format!("[{}, {eq}]", get_spell_name(ab.names, spell, hide)));
            r.vtag(spell.to_string())?;
            if label == "Death Spell" {
                apply_style(r, spell_style(world, spell, false, false))?;
            }
        }
    }

    if m.greet_txt > 0
        && let Some(tb) = world.get_tbinfo(m.greet_txt)
    {
        let a = tb.action.clone().unwrap_or_default().to_lowercase();
        let pre = |n: usize| a.chars().take(n).collect::<String>();
        if pre(4) != "heh:"
            && pre(8) != "nothing:"
            && pre(5) != "yada:"
            && pre(5) != "hehe:"
            && pre(5) != "shit:"
        {
            let r = lv.add("Greet Commands:");
            r.tag("greet_text");
            r.color(GREEN);
            r.text(format!("Textblock {}", m.greet_txt));
            r.vtag(m.greet_txt.to_string())?;
            r.vcolor(GREEN)?;
        }
    }

    if nmr >= 1.83 && m.bs_defense > 0 {
        let r = lv.add("BS Defense:");
        r.color(BLUE);
        r.text(m.bs_defense.to_string());
        r.vcolor(BLUE)?;
    }

    // ---- abilities
    let acc_tag = env.callouts.damage.accuracy_tag;
    let mut living = true;
    let mut s_abil = String::new();
    let mut mob_dodge: i16 = 0;
    let mut spell_immu_lvl: i16 = 0;
    let mut magic_lvl: i16 = 0;
    let mut mob_res = [0i16; 6];
    for slot in &m.abilities {
        if slot.ability > 0 && slot.ability != 146 {
            if !s_abil.is_empty() {
                s_abil.push_str(", ");
            }
            s_abil.push_str(&ab.get_ability_stats(
                i16c(slot.ability)?,
                i16c(slot.value)?,
                true,
                false,
            ));
            if s_abil.ends_with(", ") {
                s_abil.truncate(s_abil.len() - 2);
            }
            if slot.ability == 34 && slot.value > 0 {
                mob_dodge = i16c(slot.value)?;
                if use_char && vb::val(acc_tag) > 0.0 {
                    let tag_v = vb::val(acc_tag);
                    let q = vb::div(f64::from(slot.value) * 10.0, vb::fix(tag_v / 8.0))?;
                    s_abil.push_str(&format!(
                        " ({}% @ {} accy)",
                        vb::cstr(vb::fix(q)),
                        vb::cstr(tag_v)
                    ));
                }
            } else {
                match slot.ability {
                    51 => mob_flags |= DF_ANTI_MAGIC,
                    139 => spell_immu_lvl = i16c(slot.value)?,
                    28 => magic_lvl = i16c(slot.value)?,
                    109 => living = false,
                    78 => mob_flags |= DF_ANIMAL,
                    3 => mob_res[0] = i16c(slot.value)?,
                    5 => mob_res[1] = i16c(slot.value)?,
                    65 => mob_res[2] = i16c(slot.value)?,
                    66 => mob_res[3] = i16c(slot.value)?,
                    147 => mob_res[5] = i16c(slot.value)?,
                    _ => {}
                }
            }
        }
    }
    if living {
        mob_flags |= DF_LIVING;
    }
    if !s_abil.is_empty() {
        let r = lv.add("Abilities: ");
        r.color(BLUE);
        r.text(s_abil);
        r.vcolor(BLUE)?;
    }

    let mut first_guard = true;
    for slot in &m.abilities {
        if slot.ability == 146 && slot.value > 0 {
            let r = if first_guard {
                let r = lv.add("Guarded by: ");
                r.color(PURPLE);
                r
            } else {
                lv.add("")
            };
            first_guard = false;
            r.tag("Monster");
            r.text(get_monster_name(ab.names, slot.value, hide));
            r.vcolor(PURPLE)?;
            r.vtag(slot.value.to_string())?;
        }
    }

    lv.blank();

    let mut st = State {
        need_blank: false,
        energy: 1000,
        weapon_magic: 0,
        backstab_weapon_magic: 0,
        caches,
        mob_dmg: 0,
        avg_dmg: 0,
        defense_desc: String::new(),
        t_avg: LairInfo::default(),
        spellcast: SpellCastValues::default(),
        attack_flags: AR_UNKNOWN,
        exp,
        pass: Pass::default(),
        this: Pass::default(),
        bs_defense: 0,
    };
    let mob = Mob {
        flags: mob_flags,
        dodge: mob_dodge,
        spell_immu_lvl,
        magic_lvl,
        res: mob_res,
    };
    let first = inp.print_char_damage_out_first && nmr >= 1.83 && lair_mode;
    if first {
        char_damage_out(env, ab, inp, m, number, &t_char, &forced, &mob, &mut st, lv)?;
        mob_attacks(env, ab, inp, m, number, &t_char, &mut st, lv)?;
    } else {
        mob_attacks(env, ab, inp, m, number, &t_char, &mut st, lv)?;
        char_damage_out(env, ab, inp, m, number, &t_char, &forced, &mob, &mut st, lv)?;
    }

    // reachability from the Bank of Godfrey (the optional module)
    for r in inp.reach.rows(number) {
        lv.rows.push(r);
    }

    // ---- where it spawns
    if !inp.dont_lookup_mon_regen {
        let sb = m.summoned_by.clone().unwrap_or_default();
        if vb::len(&sb) > 4 {
            lv.add("Spawns via ...").bold();
            look_up_monster_regen(env, inp, &st, number, &sb, limit, lv);
        }
    }
    Ok(())
}

/// The mob's own defences, gathered from its abilities.
struct Mob {
    flags: i32,
    dodge: i16,
    spell_immu_lvl: i16,
    magic_lvl: i16,
    res: [i16; 6],
}

/// The procedure-level variables the two sections share.
struct State<'c> {
    need_blank: bool,
    energy: i32,
    weapon_magic: i32,
    backstab_weapon_magic: i32,
    /// The damage arrays the pane reads and, before data 1.8 or after a simulation on the spot, writes.
    caches: &'c mut DamageCaches,
    mob_dmg: i32,
    avg_dmg: i32,
    defense_desc: String,
    t_avg: LairInfo,
    spellcast: SpellCastValues,
    attack_flags: i32,
    exp: Currency,
    pass: Pass,
    this: Pass,
    bs_defense: i16,
}

/// `LookUpMonsterRegen(n, False, DetailLV, limit)`: `GetLocations` of the Summoned By text, two columns, not sorted.
fn look_up_monster_regen(
    env: &mut CharLairEnv<'_>,
    inp: &MonsterDetailInputs<'_>,
    st: &State<'_>,
    number: i32,
    summoned_by: &str,
    limit: i16,
    lv: &mut Lv,
) {
    if number < 1 || vb::len(summoned_by) < 5 {
        return;
    }
    let world = env.callouts.world;
    let opt = LocationOptions {
        nmr_ver: env.callouts.damage.ctx.nmr_ver,
        // the record numbers are shown here whatever the setting is
        hide_record_numbers: false,
        global_filter: env.callouts.damage.character.global_filter,
        charm_tag: inp.charm_tag.to_string(),
        attack_type: env.callouts.damage.character.attack.attack_type as i32,
    };
    let mut out = LocationList::default();
    with_caches(env, st.caches, |e| {
        let mut lairs = LairAdapter(e);
        get_locations(
            world,
            &mut lairs,
            &opt,
            &LocationArgs {
                loc: summoned_by,
                dont_clear: true,
                two_columns: true,
                dont_sort: true,
                limit,
                ..LocationArgs::default()
            },
            &mut out,
        );
    });
    for r in out.rows {
        lv.rows.push(MonsterRow {
            label: r.text,
            tag: r.tag,
            fore_color: None,
            bold: false,
            cells: r
                .cells
                .into_iter()
                .map(|c| MonsterCell {
                    text: c.text,
                    tag: c.tag,
                    fore_color: None,
                    bold: false,
                })
                .collect(),
        });
    }
}

// ---------------------------------------------------------------------------------------------
// drops and the mob's attacks

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn mob_attacks(
    env: &mut CharLairEnv<'_>,
    ab: &AbilityEnv<'_>,
    inp: &MonsterDetailInputs<'_>,
    m: &Monster,
    number: i32,
    t_char: &CharacterProfile,
    st: &mut State<'_>,
    lv: &mut Lv,
) -> Res<()> {
    let world = env.callouts.world;
    let ctx = env.callouts.damage.ctx;
    let nmr = ctx.nmr_ver;
    let hide = inp.hide_record_numbers;
    let use_char = env.callouts.damage.character.global_filter;
    let blank_if = |st: &mut State<'_>, lv: &mut Lv| {
        if st.need_blank {
            lv.blank();
            st.need_blank = false;
        }
    };

    blank_if(st, lv);

    let mut y = 0;
    for d in &m.drops {
        if d.item != 0 {
            y += 1;
            let r = lv.add(if y == 1 { "Item Drops" } else { "" });
            if y == 1 {
                r.bold();
            }
            r.tag("Item");
            r.text(format!(
                "{y}. {} ({}%)",
                get_item_name(ab.names, d.item, hide),
                d.pct
            ));
            r.vtag(d.item.to_string())?;
            st.need_blank = true;
        }
    }

    let mut has_attacks = false;
    for x in 0..5 {
        if m.mid_spells[x].spell != 0 {
            has_attacks = true;
        }
        let a = &m.attacks[x];
        if a.att_type > 0 && a.att_type <= 3 && a.pct > 0 {
            has_attacks = true;
        }
        if has_attacks {
            break;
        }
    }

    st.energy = 1000;
    if nmr >= 1.71 && m.energy != 0 {
        st.energy = m.energy;
    }
    blank_if(st, lv);

    if has_attacks {
        let r = lv.add("Mob's Attacks");
        r.bold();
        if nmr >= 1.71 && m.energy != 0 {
            r.text(format!("{} energy/round", st.energy));
        }
        lv.blank();
        st.need_blank = false;
    }

    st.need_blank = false;
    if !has_attacks {
        blank_if(st, lv);
        return Ok(());
    }

    // the average damage of the mob, before the character's defences
    let glue = GlueEnv {
        profile: ctx.profile,
        nmr_ver: nmr,
    };
    let local_avg = if nmr >= 1.8 {
        let mut r = calculate_monster_avg_dmg(
            world,
            &glue,
            number,
            0,
            &mut *env.callouts.sim,
            &mut *env.callouts.rnd,
        )
        .map_err(rt)?;
        r.average_damage = cur_f(m.avg_dmg)?;
        r
    } else {
        let r = calculate_monster_avg_dmg(
            world,
            &glue,
            number,
            inp.sim_rounds,
            &mut *env.callouts.sim,
            &mut *env.callouts.rnd,
        )
        .map_err(rt)?;
        if vs_char_at(st, number)?.scaled() == cur(-1).scaled() {
            store(&mut st.caches.vs_char, number, r.average_damage);
        }
        r
    };
    if local_avg.average_damage.scaled() > 0 || local_avg.max_damage.scaled() > 0 {
        let r = lv.add("Dmg/Round *");
        r.color(RED);
        if local_avg.average_damage.scaled() < local_avg.max_damage.scaled() {
            r.text(format!(
                "AVG: {}, Max: {}",
                cur_text(local_avg.average_damage),
                cur_text(local_avg.max_damage)
            ));
        } else {
            r.text(format!("AVG: {}", cur_text(local_avg.average_damage)));
        }
        if nmr >= 1.8 {
            r.vappend("   * before character defenses, calculated when DB created")?;
        } else {
            r.vappend(&format!(
                "   * before character defenses, {} round sim",
                inp.sim_rounds
            ))?;
        }
        r.vcolor(RED)?;
        st.need_blank = true;
    }

    let auto = inp.auto_calc_mon_damage;
    let mut max_seen = 0i64;
    let mut damage = cur(-1);
    if use_char {
        if auto {
            let r = calculate_monster_damage_vs_char(
                world,
                &glue,
                number,
                false,
                inp.sim_rounds,
                inp.sim_chr,
                env.query.party,
                &mut *env.callouts.sim,
                &mut *env.callouts.rnd,
            )
            .map_err(rt)?;
            damage = r.damage;
            max_seen = r.max_round_damage;
            if let Some(c) = r.cached {
                store(&mut st.caches.vs_char, number, c);
            }
        } else if vs_char_at(st, number)?.scaled() >= 0 {
            damage = vs_char_at(st, number)?;
        }
        if damage.scaled() >= 0 {
            let r = lv.add("Dmg/Round *");
            r.color(VS_CHAR);
            if auto {
                r.text(format!("AVG: {}, Max Seen: {max_seen}", cur_text(damage)));
            } else {
                r.text(format!("AVG: {}", cur_text(damage)));
            }
            r.vappend(&format!(
                "   * versus current character defenses, {} round sim",
                inp.sim_rounds
            ))?;
            r.vcolor(VS_CHAR)?;
            st.need_blank = true;
        }
    }

    damage = cur(-1);
    if t_char.party > 1 {
        if auto {
            let r = calculate_monster_damage_vs_char(
                world,
                &glue,
                number,
                true,
                inp.sim_rounds,
                inp.sim_chr,
                env.query.party,
                &mut *env.callouts.sim,
                &mut *env.callouts.rnd,
            )
            .map_err(rt)?;
            damage = r.damage;
            max_seen = r.max_round_damage;
            if let Some(c) = r.cached {
                store(&mut st.caches.vs_party, number, c);
            }
        } else if vs_party_at(st, number)?.scaled() >= 0 {
            damage = vs_party_at(st, number)?;
        }
        if damage.scaled() >= 0 {
            let r = lv.add("Dmg/Round *");
            r.color(PINK);
            if auto {
                r.text(format!("AVG: {}, Max Seen: {max_seen}", cur_text(damage)));
            } else {
                r.text(format!("AVG: {}", cur_text(damage)));
            }
            r.vappend(&format!(
                "   * versus current PARTY defenses, {} round sim",
                inp.sim_rounds
            ))?;
            r.vcolor(PINK)?;
            st.need_blank = true;
        }
    }
    blank_if(st, lv);

    // between-round spells
    let mut pct: i32 = 0;
    let mut y = 0;
    for x in 0..5 {
        let ms = &m.mid_spells[x];
        if ms.spell != 0 {
            y += 1;
            pct = ms.pct - pct;
            let eq = ab.pull_spell_eq(&PullArgs {
                calc_level: true,
                level: i16c(ms.lvl)?,
                spell: ms.spell,
                for_monster: true,
                ..PullArgs::default()
            });
            let r = lv.add(if y == 1 { "Between Rounds" } else { "" });
            r.tag("Spell");
            r.text(format!(
                "({pct}%) [{}, {eq}]",
                get_spell_name(ab.names, ms.spell, hide)
            ));
            r.vtag(format!("{}@{}", ms.spell, ms.lvl))?;
            apply_style(r, spell_style(world, ms.spell, true, true))?;
            pct = ms.pct;
            st.need_blank = true;
        }
    }
    blank_if(st, lv);

    // the attacks
    pct = 0;
    let mut y = 0;
    for x in 0..5 {
        let a = &m.attacks[x];
        if !(a.att_type > 0 && a.att_type <= 3 && a.pct > 0) {
            continue;
        }
        y += 1;
        let row_idx = lv.rows.len();
        lv.add("");
        st.need_blank = true;
        pct = a.pct - pct;
        if pct < 0 {
            pct = 0;
        }
        let label = if nmr >= 1.8 {
            let tr = vb::round(a.true_pct, 0)?;
            let mut l = if tr == 0.0 {
                format!("({pct}%) ")
            } else {
                format!("({}%) ", vb::cstr(tr))
            };
            let nm = a.name.trim_matches(' ');
            if nm.is_empty() {
                l.push_str(&format!("Attack {y}"));
            } else {
                l.push_str(nm);
            }
            l
        } else {
            format!("Attack {y} ({pct}%)")
        };
        lv.rows[row_idx].label = label;
        pct = a.pct;

        let energy_text = |energy: i32, mon_energy: i32| -> Res<String> {
            if energy > 0 {
                Ok(format!(
                    "Energy: {energy} (Max {}x/round)",
                    vb::cstr(vb::fix(vb::div(f64::from(mon_energy), f64::from(energy))?))
                ))
            } else {
                Ok(format!("Energy: {energy}"))
            }
        };
        let hit_spell_row = |lv: &mut Lv, slay: bool| -> Res<()> {
            let eq = ab.pull_spell_eq(&PullArgs {
                spell: a.hit_spell,
                ..PullArgs::default()
            });
            let r = lv.add("");
            r.tag("Spell");
            r.text(format!(
                "Hit Spell: [{}, {eq}]",
                get_spell_name(ab.names, a.hit_spell, hide)
            ));
            r.vtag(a.hit_spell.to_string())?;
            apply_style(r, spell_style(world, a.hit_spell, slay, true))?;
            Ok(())
        };

        match a.att_type {
            1 | 3 => {
                lv.rows[row_idx].text(format!("Min-Max: {}-{}", a.min, a.max));
                st.need_blank = true;
                lv.add("").text(format!("Accuracy: {}", a.acc));
                lv.add("").text(energy_text(a.energy, st.energy)?);
                if a.hit_spell != 0 {
                    st.need_blank = true;
                    hit_spell_row(lv, false)?;
                }
            }
            2 => {
                st.need_blank = true;
                let eq = ab.pull_spell_eq(&PullArgs {
                    calc_level: true,
                    level: i16c(a.max)?,
                    spell: a.acc,
                    for_monster: true,
                    ..PullArgs::default()
                });
                {
                    let r = &mut lv.rows[row_idx];
                    r.tag("Spell");
                    r.text(format!(
                        "Spell: [{}, {eq}]",
                        get_spell_name(ab.names, a.acc, hide)
                    ));
                    r.vtag(format!("{}@{}", a.acc, a.max))?;
                    apply_style(r, spell_style(world, a.acc, true, true))?;
                }
                if spell_is_area_attack(&world.spells, a.acc) {
                    let targets = world.get_spell(a.acc).map_or(0, |s| s.targets);
                    let r = lv.add("");
                    r.text(format!("Target: {}", spell_targets(targets)));
                    r.vcolor(255)?;
                }
                lv.add("").text(format!("Success %: {}", a.min));
                lv.add("").text(energy_text(a.energy, st.energy)?);
                if a.hit_spell != 0 {
                    hit_spell_row(lv, true)?;
                }
                if spell_is_area_attack(&world.spells, a.acc)
                    && get_spell_duration(&world.spells, a.acc, i16c(a.max)?, true) == 0
                    && spell_ability(world, a.acc, 1) > -1
                {
                    st.need_blank = true;
                    let r = lv.add("");
                    r.text(
                        "This is an invalid spell and will not be cast (area attack spells from regular monster casts must use ability 17, Damage-MR).",
                    );
                    r.vbold(true)?;
                }
            }
            _ => {}
        }
        if st.need_blank {
            lv.blank();
            st.need_blank = false;
        }
    }
    Ok(())
}

/// `nMonsterDamageVsChar(n)`, with what this pane stored itself before data 1.8.
fn vs_char_at(st: &State<'_>, n: i32) -> Res<Currency> {
    at(&st.caches.vs_char, n)
}

/// `nMonsterDamageVsParty(n)`, with what this pane stored itself.
fn vs_party_at(st: &State<'_>, n: i32) -> Res<Currency> {
    at(&st.caches.vs_party, n)
}

// ---------------------------------------------------------------------------------------------
// the character's damage, the scripting estimate and the lair stats

#[allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::cognitive_complexity
)]
fn char_damage_out(
    env: &mut CharLairEnv<'_>,
    ab: &AbilityEnv<'_>,
    inp: &MonsterDetailInputs<'_>,
    m: &Monster,
    number: i32,
    t_char: &CharacterProfile,
    forced: &CharacterProfile,
    mob: &Mob,
    st: &mut State<'_>,
    lv: &mut Lv,
) -> Res<()> {
    let world = env.callouts.world;
    let ctx = env.callouts.damage.ctx;
    let character = env.callouts.damage.character;
    let nmr = ctx.nmr_ver;
    let use_char = character.global_filter;
    let lair_mode = env.query.party.lair_mode;
    let atype = character.attack.attack_type;
    let party = t_char.party;
    let summoned = m.summoned_by.clone().unwrap_or_default();

    if st.need_blank {
        lv.blank();
        st.need_blank = false;
    }

    // which cached damage the mob does
    st.defense_desc.clear();
    let vs_party = vs_party_at(st, number)?;
    let vs_char = vs_char_at(st, number)?;
    if party > 1 && vs_party.scaled() >= 0 {
        st.mob_dmg = clng(vs_party)?;
        st.defense_desc = " (vs party defenses)".into();
    } else if use_char && vs_char.scaled() >= 0 {
        st.mob_dmg = clng(vs_char)?;
        st.defense_desc = " (vs char defenses)".into();
    } else if nmr >= 1.8 {
        st.mob_dmg = vb::clng(m.avg_dmg)?;
        st.defense_desc = " (vs default defenses)".into();
    } else if at(env.callouts.vs_default, number)?.scaled() >= 0 {
        st.mob_dmg = clng(at(env.callouts.vs_default, number)?)?;
        st.defense_desc = " (vs default defenses)".into();
    } else {
        st.mob_dmg = 0;
    }
    st.avg_dmg = st.mob_dmg;

    // the average lair
    if nmr >= 1.83 && contains_ci(&summoned, "lair") {
        let cfg = env.query.global_attack_config.to_string();
        let avg = with_caches(env, st.caches, |e| {
            get_lair_averages_from_locs(
                e,
                &summoned,
                &LairLocsInput {
                    nmr_ver: nmr,
                    summoned_by: &summoned,
                    global_attack_config: &cfg,
                },
            )
        });
        env.cache.last_avg_lair_info = avg;
    } else if !env.cache.last_avg_lair_info.group_index.is_empty() {
        env.cache.last_avg_lair_info =
            with_caches(env, st.caches, |e| LairEnv::get_lair_info(e, "", 0));
    }
    st.t_avg = env.cache.last_avg_lair_info.clone();
    let t_avg = st.t_avg.clone();

    let item_ab = |item: i32, a: i16| -> i32 {
        let slots = world.get_item(item).map(|i| &i.abilities[..]);
        item_has_ability(Profile::Stock, slots, item, a)
    };
    let weapon0 = ctx.stats.weapon_number[0];
    if party == 1 {
        st.weapon_magic = match atype {
            AttackType::OneShot => 9999,
            AttackType::MartialArts | AttackType::Spell | AttackType::SpellAny => {
                t_char.hit_magic_non_weapon
            }
            _ => t_char.hit_magic,
        };
        if atype != AttackType::OneShot && character.attack.backstab {
            let mut w = character.attack.backstab_weapon;
            if w == 0 {
                w = weapon0;
            }
            if w > 0 {
                let mut bm = item_ab(w, 28);
                let hm = item_ab(w, 142);
                if hm > -500 {
                    bm = vb::add_i32(bm, hm)?;
                }
                st.backstab_weapon_magic = vb::add_i32(bm, t_char.hit_magic_non_weapon)?;
            } else {
                st.backstab_weapon_magic = t_char.hit_magic_non_weapon;
            }
        }
    }
    if st.weapon_magic < 0 {
        st.weapon_magic = 0;
    }
    if st.backstab_weapon_magic < 0 {
        st.backstab_weapon_magic = 0;
    }

    let spell_env = SpellCastEnv {
        profile: ctx.profile,
        spells: ctx.spells,
        text: ab,
        avg_level_max_all_stats: ctx.avg_level_max_all_stats,
    };
    let mut backstab_detail = String::new();

    let passes = if t_avg.total_lairs > 0 { 2 } else { 1 };
    for i_attack in 1..=passes {
        let mut damage_out: i32 = 0;
        let mut first_round: i32 = -9999;
        let mut override_rtk: f64 = 0.0;
        let mut backstab_text = String::new();
        backstab_detail.clear();
        let mut immu = String::new();
        let mut flags: i32 = 0;
        let mut valid_target = false;
        let header;
        let (calc_hp, calc_hp_regen, calc_ac, calc_dr, calc_dodge, calc_mr);
        let mut calc_res = [0i16; 6];
        let num_mobs: f64;
        let (calc_immu, calc_magic);
        if i_attack == 1 {
            header = "Damage vs Mob";
            st.avg_dmg = st.mob_dmg;
            calc_hp = m.hp;
            calc_hp_regen = m.hp_regen;
            calc_ac = m.armour_class;
            calc_dr = m.damage_resist;
            calc_dodge = i32::from(mob.dodge);
            calc_mr = m.magic_res;
            calc_res = mob.res;
            num_mobs = 1.0;
            calc_immu = mob.spell_immu_lvl;
            calc_magic = mob.magic_lvl;
            flags = mob.flags;
        } else {
            header = "Damage vs Lair";
            st.avg_dmg = clng(t_avg.avg_dmg_lair)?;
            calc_hp = t_avg.avg_hp;
            calc_hp_regen = 0;
            calc_ac = i32::from(t_avg.avg_ac);
            calc_dr = i32::from(t_avg.avg_dr);
            calc_dodge = i32::from(t_avg.avg_dodge);
            calc_mr = i32::from(t_avg.avg_mr);
            calc_res[0] = t_avg.avg_rcol;
            calc_res[1] = t_avg.avg_rfir;
            calc_res[2] = t_avg.avg_rsto;
            calc_res[3] = t_avg.avg_rlit;
            calc_res[5] = t_avg.avg_rwat;
            num_mobs = t_avg.max_regen.to_f64();
            override_rtk = t_avg.rtk;
            calc_immu = t_avg.spell_immu_lvl;
            calc_magic = t_avg.magic_lvl;
            let mobs = t_avg.mobs.to_f64();
            if t_avg.num_anti_magic > 0 && f64::from(t_avg.num_anti_magic) >= mobs / 2.0 {
                flags |= DF_ANTI_MAGIC;
            }
            let ratio = mobs * LAIR_FLAG_RATIO;
            if t_avg.num_undeads > 0 && f64::from(t_avg.num_undeads) >= ratio {
                flags |= DF_UNDEAD;
            }
            if t_avg.num_living > 0 && f64::from(t_avg.num_living) >= ratio {
                flags |= DF_LIVING;
            }
            if t_avg.num_animals > 0 && f64::from(t_avg.num_animals) >= ratio {
                flags |= DF_ANIMAL;
            }
        }

        let char_hp = vb::clng(t_char.hp)?;
        let calc_dmg_out = |st: &State<'_>,
                            dmg: i32,
                            detail: String,
                            detail2: &str,
                            oom: i16,
                            surprise: i32,
                            surprise_detail: &str,
                            first: i32,
                            override_rtk: f64,
                            lv: &mut Lv|
         -> Res<()> {
            add_damage_out_text(
                lv,
                &DamageOutText {
                    header,
                    detail: &detail,
                    detail2,
                    damage_out: dmg,
                    mob_health: calc_hp,
                    mob_hp_regen: calc_hp_regen,
                    mob_damage: st.avg_dmg,
                    char_health: char_hp,
                    defense_text: &st.defense_desc,
                    num_mobs,
                    oom,
                    override_rtk,
                    surprise_damage_out: f64::from(surprise),
                    surprise_detail,
                    first_round: first,
                    use_character: use_char,
                    bless: inp.bless_caption,
                    heal_cost: character.attack.heal_cost,
                },
                ctx.profile,
            )
        };

        'attack: {
            if party > 1 || atype == AttackType::Manual {
                let tag = if party > 1 {
                    "/round (party)"
                } else {
                    "/round (manual)"
                };
                if i_attack == 2 {
                    damage_out = t_avg.damage_out;
                    first_round = t_avg.first_round_damage_out;
                    st.pass.min_damage = t_avg.min_round_damage_out;
                    st.pass.surprise = t_avg.surprise_damage_out;
                    st.pass.surprise_min = t_avg.surprise_min_damage_out;
                    st.pass.surprise_chance = t_avg.surprise_chance;
                    if st.pass.surprise > 0 {
                        backstab_text = format!(" + {} surprise round", st.pass.surprise);
                    }
                    calc_dmg_out(
                        st,
                        damage_out,
                        format!("{damage_out}{tag}{immu}{backstab_text}"),
                        "",
                        0,
                        st.pass.surprise,
                        "",
                        first_round,
                        override_rtk,
                        lv,
                    )?;
                } else {
                    let d = get_damage_output(
                        &env.callouts.damage,
                        &DamageRequest {
                            single_monster: number,
                            vs_ac: calc_ac,
                            vs_dr: calc_dr,
                            vs_mr: calc_mr,
                            vs_dodge: calc_dodge,
                            defense_flags: flags,
                            spell_immunity_lvl: calc_immu,
                            vs_magic_lvl: calc_magic,
                            ..DamageRequest::default()
                        },
                    );
                    damage_out = clng(d.average_damage)?;
                    first_round = clng(d.first_round_damage)?;
                    st.pass.min_damage = clng(d.min_round_damage)?;
                    st.pass.surprise = clng(d.surprise_damage)?;
                    st.pass.surprise_min = clng(d.surprise_min_damage)?;
                    st.pass.surprise_chance = d.surprise_damage_chance;
                    st.this.damage_out = damage_out;
                    st.this.first_round = first_round;
                    st.this.min_damage = st.pass.min_damage;
                    st.this.surprise = st.pass.surprise;
                    st.this.surprise_min = st.pass.surprise_min;
                    st.this.surprise_chance = st.pass.surprise_chance;
                    if st.pass.surprise > -9999 {
                        if st.pass.surprise == -9998 {
                            st.pass.surprise = -9999;
                            backstab_text = " + 0 surprise round [immune:MagicLVL]".into();
                        } else {
                            backstab_text = format!(" + {} surprise round", st.pass.surprise);
                        }
                    }
                    if damage_out == -9998 {
                        immu = " [immune:MagicLVL]".into();
                        damage_out = 0;
                    }
                    calc_dmg_out(
                        st,
                        damage_out,
                        format!("{damage_out}{tag}{immu}{backstab_text}"),
                        "",
                        0,
                        st.pass.surprise,
                        "",
                        first_round,
                        override_rtk,
                        lv,
                    )?;
                }
                break 'attack;
            }

            // a backstab opener
            if party == 1 && atype != AttackType::OneShot && character.attack.backstab {
                if calc_magic > 0 && st.backstab_weapon_magic < i32::from(calc_magic) {
                    st.pass.surprise = 0;
                    backstab_text = " + 0 surprise round [immune:MagicLVL]".into();
                } else {
                    let mut w = 0;
                    if character.attack.backstab_weapon > 0 {
                        w = character.attack.backstab_weapon;
                    } else if character.attack.backstab_weapon == 0 && weapon0 > 0 {
                        w = weapon0;
                    }
                    let mut bs_prof = CharacterProfile::default();
                    populate(
                        &env.callouts.damage,
                        &mut bs_prof,
                        false,
                        true,
                        AttackTypeMud::Surprise,
                        w,
                    );
                    if nmr >= 1.83 {
                        st.bs_defense = i16c(m.bs_defense)?;
                    }
                    let bs = attack_with(
                        &env.callouts.damage,
                        &bs_prof,
                        &AttackRequest {
                            attack_type: 4,
                            weapon_number: w,
                            speed_adj: 100,
                            vs_ac: calc_ac,
                            vs_dr: calc_dr,
                            vs_dodge: calc_dodge,
                            bs_defense: i32::from(st.bs_defense),
                            ..AttackRequest::default()
                        },
                    );
                    st.pass.surprise = vb::add_i32(bs.avg_hit, bs.avg_extra_hit)?;
                    st.pass.surprise_min = vb::add_i32(bs.min_dmg, bs.avg_extra_swing)?;
                    st.pass.surprise_chance = bs.hit_chance;
                    backstab_text = format!(" + {} surprise round", st.pass.surprise);
                    backstab_detail = bs.attack_detail;
                }
            } else {
                st.pass.surprise = 0;
                st.pass.surprise_min = 0;
                st.pass.surprise_chance = 0;
            }

            match atype {
                AttackType::PhysAttack
                | AttackType::PhysBash
                | AttackType::PhysSmash
                | AttackType::MartialArts => {
                    let ar = match atype {
                        AttackType::PhysBash => (6, weapon0),
                        AttackType::PhysSmash => (7, weapon0),
                        AttackType::MartialArts => match character.attack.martial_art {
                            2 => (2, 0),
                            3 => (3, 0),
                            _ => (1, 0),
                        },
                        _ => (5, weapon0),
                    };
                    let a = attack_with(
                        &env.callouts.damage,
                        forced,
                        &AttackRequest {
                            attack_type: ar.0,
                            weapon_number: ar.1,
                            speed_adj: 100,
                            vs_ac: calc_ac,
                            vs_dr: calc_dr,
                            vs_dodge: calc_dodge,
                            ..AttackRequest::default()
                        },
                    );
                    let (t_attack_desc, t_attack_detail) =
                        (a.attack_desc.clone(), a.attack_detail.clone());
                    if calc_magic > 0 && st.weapon_magic < i32::from(calc_magic) {
                        damage_out = 0;
                        first_round = 0;
                        st.pass.min_damage = 0;
                        override_rtk = 0.0;
                        immu = " [immune:MagicLVL]".into();
                    } else {
                        damage_out = a.round_total;
                        first_round = a.first_round_damage;
                        st.pass.min_damage = a.min_round_damage;
                    }
                    calc_dmg_out(
                        st,
                        damage_out,
                        format!("{damage_out}/round ({t_attack_desc}){immu}{backstab_text}"),
                        &t_attack_detail,
                        0,
                        st.pass.surprise,
                        &backstab_detail,
                        first_round,
                        override_rtk,
                        lv,
                    )?;
                }
                AttackType::Spell | AttackType::SpellAny => {
                    let spell_num = character.attack.spell_num;
                    if spell_num <= 0 {
                        break 'attack;
                    }
                    let level = if atype == AttackType::SpellAny {
                        i32::from(character.attack.spell_lvl)
                    } else {
                        forced.level
                    };
                    let cast = spell_env.calculate_spell_cast(
                        &forced.spell_caster(),
                        &CastRequest {
                            spell: spell_num,
                            cast_level: level,
                            vs_mr: calc_mr,
                            vs_anti_magic: (flags & DF_ANTI_MAGIC) != 0,
                            vs_cold: calc_res[0],
                            vs_fire: calc_res[1],
                            vs_stone: calc_res[2],
                            vs_lightning: calc_res[3],
                            vs_water: calc_res[5],
                        },
                    );
                    st.spellcast = cast;
                    let sc = &st.spellcast;
                    let immu_lvl = i32::from(calc_immu);
                    if immu_lvl == 0
                        || i32::from(sc.n_required_level) >= immu_lvl
                        || (sc.n_required_level == 0 && i32::from(sc.n_cast_level) >= immu_lvl)
                    {
                        if st.attack_flags == AR_UNKNOWN {
                            if let Some(sp) = world.get_spell(spell_num) {
                                for slot in &sp.abilities {
                                    match slot.ability {
                                        23 => st.attack_flags |= AR_UNDEAD,
                                        80 => st.attack_flags |= AR_ANIMAL,
                                        108 => st.attack_flags |= AR_LIVING,
                                        _ => {}
                                    }
                                }
                                if st.attack_flags <= AR_NONE {
                                    valid_target = true;
                                }
                            }
                        } else if st.attack_flags == AR_NONE {
                            valid_target = true;
                        }
                        if !valid_target {
                            if st.attack_flags > AR_NONE {
                                if (st.attack_flags & AR_UNDEAD) != 0 {
                                    if (flags & DF_UNDEAD) != 0 {
                                        valid_target = true;
                                    }
                                } else if (st.attack_flags & AR_ANIMAL) != 0 {
                                    if (flags & DF_ANIMAL) != 0 {
                                        valid_target = true;
                                    }
                                } else if (st.attack_flags & AR_LIVING) != 0
                                    && (flags & DF_LIVING) != 0
                                {
                                    valid_target = true;
                                }
                            } else {
                                valid_target = true;
                            }
                        }
                    }
                    let sc = &st.spellcast;
                    if valid_target {
                        damage_out = sc.n_avg_round_dmg;
                        first_round = damage_out;
                        st.pass.min_damage = sc.n_min_round_dmg;
                    } else {
                        damage_out = 0;
                        first_round = 0;
                        st.pass.min_damage = 0;
                        let req = i32::from(sc.n_required_level);
                        let castl = i32::from(sc.n_cast_level);
                        if immu_lvl > 0
                            && ((req > 0 && req < immu_lvl) || (req == 0 && castl < immu_lvl))
                        {
                            immu = auto_append(&immu, "SpellImmuLVL", "+");
                        }
                        if (st.attack_flags & AR_UNDEAD) != 0 && (flags & DF_UNDEAD) == 0 {
                            immu = auto_append(&immu, "NotUndead", "+");
                        }
                        if (st.attack_flags & AR_ANIMAL) != 0 && (flags & DF_ANIMAL) == 0 {
                            immu = auto_append(&immu, "NotAnimal", "+");
                        }
                        if (st.attack_flags & AR_LIVING) != 0 && (flags & DF_LIVING) == 0 {
                            immu = auto_append(&immu, "NotLiving", "+");
                        }
                        immu = format!(" [immune:{immu}]");
                    }
                    let avg_text = if immu.is_empty() {
                        sc.s_avg_round.clone()
                    } else {
                        "0".to_string()
                    };
                    let oom = sc.n_oom;
                    let mma = sc.s_mma.clone();
                    let name = sc.s_spell_name.clone();
                    calc_dmg_out(
                        st,
                        damage_out,
                        format!("{avg_text}/round ({name}){immu}{backstab_text}"),
                        &mma,
                        oom,
                        st.pass.surprise,
                        &backstab_detail,
                        first_round,
                        override_rtk,
                        lv,
                    )?;
                }
                _ => {
                    // one-shot (the manual choice left above)
                    damage_out = 9_999_999;
                    add_damage_out_text(
                        lv,
                        &DamageOutText {
                            header,
                            detail: "(assuming one-shot)",
                            ..DamageOutText::plain(use_char, inp.bless_caption)
                        },
                        ctx.profile,
                    )?;
                }
            }
        }
        // no_attack:
        if i_attack == 1 {
            st.this.damage_out = damage_out;
            st.this.first_round = first_round;
            st.this.min_damage = st.pass.min_damage;
            st.this.surprise = st.pass.surprise;
            st.this.surprise_min = st.pass.surprise_min;
            st.this.surprise_chance = st.pass.surprise_chance;
        }
    }

    if t_avg.total_lairs > 0 {
        st.avg_dmg = clng(t_avg.avg_dmg)?;
        st.defense_desc = if party > 1 {
            " (LAIR avg vs party defenses)".into()
        } else {
            " (LAIR avg vs defenses)".into()
        };
    }

    scripting_and_lair(env, ab, inp, m, number, t_char, st, lv, lair_mode)?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// AddMonsterDamageOutText

struct DamageOutText<'a> {
    header: &'a str,
    detail: &'a str,
    detail2: &'a str,
    damage_out: i32,
    mob_health: i32,
    mob_hp_regen: i32,
    mob_damage: i32,
    char_health: i32,
    defense_text: &'a str,
    num_mobs: f64,
    oom: i16,
    override_rtk: f64,
    surprise_damage_out: f64,
    surprise_detail: &'a str,
    first_round: i32,
    use_character: bool,
    bless: f64,
    heal_cost: f64,
}

impl<'a> DamageOutText<'a> {
    /// MME's defaults for every optional argument.
    fn plain(use_character: bool, bless: f64) -> Self {
        Self {
            header: "",
            detail: "",
            detail2: "",
            damage_out: -9999,
            mob_health: 0,
            mob_hp_regen: 0,
            mob_damage: -1,
            char_health: 0,
            defense_text: "",
            num_mobs: 1.0,
            oom: 0,
            override_rtk: 0.0,
            surprise_damage_out: -9999.0,
            surprise_detail: "",
            first_round: -9999,
            use_character,
            bless,
            heal_cost: 0.0,
        }
    }
}

/// Port of modMain `AddMonsterDamageOutText`.
fn add_damage_out_text(lv: &mut Lv, a: &DamageOutText<'_>, profile: Profile) -> Res<()> {
    let cr = calc_combat_rounds(
        profile,
        &CombatRoundsArgs {
            damage_out: a.damage_out,
            mob_health: a.mob_health,
            mob_damage: a.mob_damage,
            char_health: a.char_health,
            mob_hp_regen: a.mob_hp_regen,
            num_mobs: a.num_mobs,
            override_rtk: a.override_rtk,
            surprise_damage_out: a.surprise_damage_out,
            first_round_damage_out: a.first_round,
        },
    );
    {
        let r = lv.add(a.header);
        r.bold();
        r.text(a.detail);
        if a.detail.to_lowercase().contains("immune:") {
            r.color(RED);
            r.vcolor(RED)?;
        }
    }
    if a.surprise_damage_out > 0.0 && vb::len(a.surprise_detail) > 0 {
        lv.add("").text(a.surprise_detail);
    }
    if !a.detail2.is_empty() {
        lv.add("").text(a.detail2);
    }
    if !(cr.rtk_text.is_empty() && cr.rtd_text.is_empty()) {
        let r = lv.add("");
        if !cr.rtk_text.is_empty() && !cr.rtd_text.is_empty() {
            r.text(format!(
                "{}{}{}",
                auto_append(&cr.rtk_text, &cr.rtd_text, " "),
                cr.success_text,
                a.defense_text
            ));
        } else {
            r.text(format!("{}{}{}", cr.rtk_text, cr.rtd_text, cr.success_text));
        }
        if cr.rtd > 0.0 && cr.rtk > 1.0 && cr.success < 70 {
            r.vcolor(DARK_RED)?;
        } else if cr.rtd >= 1.0 && cr.success >= 95 {
            r.vcolor(GREEN)?;
        }
    }
    if a.oom > 0 && a.oom < 100 {
        let mut ext = "";
        if a.use_character && (a.bless > 0.0 || a.heal_cost > 0.0) {
            ext = " (with current heals/bless set)";
        }
        let r = lv.add("");
        r.text(format!("OOM in {} rounds{ext}", a.oom));
        if f64::from(a.oom) * 0.9 < cr.rtk * 1.1 {
            r.vcolor(DARK_RED)?;
        }
    }
    lv.blank();
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// scripting and the lair stats

fn exp_text(x: f64, one_decimal_m: bool) -> Res<String> {
    Ok(if x > 1_000_000.0 {
        if one_decimal_m {
            format!("{} M", vb::format(x / 1_000_000.0, "#,#.0")?)
        } else {
            format!("{} M", vb::format(x / 1_000_000.0, "#,#.00")?)
        }
    } else if x > 1000.0 {
        format!("{} K", vb::format(x / 1000.0, "#,#.0")?)
    } else {
        fmt_pos(round_up(x), "#,#")?
    })
}

#[allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::cognitive_complexity
)]
fn scripting_and_lair(
    env: &mut CharLairEnv<'_>,
    ab: &AbilityEnv<'_>,
    inp: &MonsterDetailInputs<'_>,
    m: &Monster,
    number: i32,
    t_char: &CharacterProfile,
    st: &mut State<'_>,
    lv: &mut Lv,
    lair_mode: bool,
) -> Res<()> {
    let ctx = env.callouts.damage.ctx;
    let character = env.callouts.damage.character;
    let nmr = ctx.nmr_ver;
    let use_char = character.global_filter;
    let atype = character.attack.attack_type;
    let party = t_char.party;
    let profile = ctx.profile;
    let acc_tag = env.callouts.damage.accuracy_tag;
    let t_avg = st.t_avg.clone();
    let summoned = m.summoned_by.clone().unwrap_or_default();
    let exp = st.exp;
    let possy_n = at(inp.possy, number);
    let avg_dmg = st.avg_dmg;
    let char_attack_spell = character.attack.spell_num;

    'done: {
        if exp.scaled() <= 10_000 || m.hp < 1 {
            break 'done;
        }
        {
            let r = lv.add(
                if t_avg.total_lairs > 0 && (m.regen_time > 0 || contains_ci(&summoned, "room")) {
                    "Scripting vs MOB"
                } else if t_avg.total_lairs > 0 {
                    "Scripting vs Lair"
                } else {
                    "Scripting"
                },
            );
            r.bold();
            if nmr >= 1.83 && !use_char {
                r.text("(global filter inactive. default stats used.)");
            }
        }

        if nmr >= 1.83 {
            let mut exp_info = ExpPerHourInfo::default();
            let est_idx = lv.rows.len();
            lv.add("Scripting Estimate");
            let mut exp_per_hour = Currency::from_scaled(0);
            let sc_cost = st.spellcast.n_mana_cost;
            if m.regen_time == 0 && t_avg.total_lairs > 0 {
                let l = &t_avg;
                exp_info = calc_exp_per_hour(
                    &ExpPerHourInput {
                        n_exp: l.avg_exp,
                        n_regen_time: l.avg_delay,
                        n_num_mobs: l.max_regen.to_f64(),
                        n_total_lairs: l.total_lairs,
                        n_poss_spawns: l.poss_spawns,
                        n_rtk: l.rtk,
                        n_char_dmg: f64::from(l.damage_out),
                        n_char_hp: vb::clng(t_char.hp)?,
                        n_char_hp_regen: vb::clng(t_char.hp_regen)?,
                        n_mob_dmg: l.avg_dmg_lair.to_f64(),
                        n_mob_hp: l.avg_hp,
                        n_mob_hp_regen: 0,
                        n_damage_threshold: vb::clng(t_char.damage_threshold)?,
                        n_spell_cost: sc_cost,
                        n_spell_overhead: t_char.spell_overhead,
                        n_char_mana: vb::clng(t_char.max_mana)?,
                        n_char_mp_regen: vb::clng(t_char.mana_regen)?,
                        n_meditate_rate: vb::clng(t_char.meditate_rate)?,
                        n_avg_walk: l.avg_walk.to_f64(),
                        n_walk_speed: t_char.walk_speed,
                        n_surprise_dmg: f64::from(l.surprise_damage_out),
                        n_surprise_min_dmg: f64::from(l.surprise_min_damage_out),
                        n_surprise_chance: l.surprise_chance,
                        n_char_first_round_dmg: f64::from(l.first_round_damage_out),
                        n_min_round_dmg: f64::from(l.min_round_damage_out),
                    },
                    inp.settings,
                    profile,
                )
                .map_err(rt)?;
                exp_per_hour = cur_f(exp_info.n_exp_per_hour)?;
            } else if m.regen_time > 0 || contains_ci(&summoned, "room") {
                exp_info = calc_exp_per_hour(
                    &ExpPerHourInput {
                        n_exp: exp,
                        n_regen_time: f64::from(m.regen_time),
                        n_num_mobs: 1.0,
                        n_total_lairs: -1,
                        n_char_dmg: f64::from(st.this.damage_out),
                        n_char_hp: vb::clng(t_char.hp)?,
                        n_char_hp_regen: vb::clng(t_char.hp_regen)?,
                        n_mob_dmg: f64::from(st.mob_dmg),
                        n_mob_hp: m.hp,
                        n_mob_hp_regen: m.hp_regen,
                        n_damage_threshold: vb::clng(t_char.damage_threshold)?,
                        n_spell_cost: sc_cost,
                        n_spell_overhead: t_char.spell_overhead,
                        n_char_mana: vb::clng(t_char.max_mana)?,
                        n_char_mp_regen: vb::clng(t_char.mana_regen)?,
                        n_meditate_rate: vb::clng(t_char.meditate_rate)?,
                        n_avg_walk: 0.0,
                        n_walk_speed: 1.0,
                        n_surprise_dmg: f64::from(st.this.surprise),
                        n_surprise_min_dmg: f64::from(st.this.surprise_min),
                        n_surprise_chance: st.this.surprise_chance,
                        n_char_first_round_dmg: f64::from(st.this.first_round),
                        n_min_round_dmg: f64::from(st.this.min_damage),
                        ..ExpPerHourInput::default()
                    },
                    inp.settings,
                    profile,
                )
                .map_err(rt)?;
                exp_per_hour = cur_f(exp_info.n_exp_per_hour)?;
            } else {
                exp_info.s_rtc_text = "(No lairs and not assigned as an NPC)".into();
            }

            let exp_each = if exp_per_hour.scaled() > 0 && party > 1 {
                cur_round(exp_per_hour.checked_div(cur(i32::from(party)))?, 1)?
            } else {
                exp_per_hour
            };
            let eph = exp_per_hour.to_f64();
            let mut text;
            if eph > 0.0 {
                text = exp_text(eph, false)?;
                text.push_str("/hr");
                if exp_per_hour.scaled() != exp_each.scaled() && exp_each.to_f64() > 0.0 {
                    text.push_str(&format!(" ({}/hr ea.)", exp_text(exp_each.to_f64(), true)?));
                }
                if inp.settings.show_all {
                    text.push_str(&exp_info.s_exp_all);
                }
            } else if exp_per_hour.scaled() == cur(-1).scaled() {
                text = if m.regen_time == 0 && t_avg.total_lairs > 0 {
                    "The lairs of this mob".into()
                } else {
                    "This mob".to_string()
                };
                text.push_str(" deemed undefeatable against current stats .");
            } else {
                text = "0".into();
            }
            lv.rows[est_idx].text(text);

            if vb::len(&exp_info.s_rtc_text) > 0 {
                lv.add("").text(exp_info.s_rtc_text.clone());
            }
            if vb::len(&exp_info.s_move_text) > 0 {
                lv.add("").text(exp_info.s_move_text.clone());
            }
            if (exp_info.n_time_recovering > 0.0
                || (inp.settings.show_all
                    && (vb::len(&exp_info.s_hitpoint_recovery) > 6
                        || vb::len(&exp_info.s_mana_recovery) > 6)))
                && exp_per_hour.scaled() >= 0
            {
                let mut glue = String::new();
                if vb::len(&exp_info.s_mana_recovery) > 0
                    && vb::len(&exp_info.s_hitpoint_recovery) > 0
                {
                    lv.add("").text(exp_info.s_time_recovering.clone());
                    glue = " > ".into();
                }
                if vb::len(&exp_info.s_hitpoint_recovery) > 0 {
                    lv.add("")
                        .text(format!("{glue}{}", exp_info.s_hitpoint_recovery));
                }
                if vb::len(&exp_info.s_mana_recovery) > 0 {
                    let oom = if st.spellcast.n_oom > 0 {
                        format!(" (OOM every {} rounds)", st.spellcast.n_oom)
                    } else {
                        String::new()
                    };
                    lv.add("")
                        .text(format!("{glue}{}{oom}", exp_info.s_mana_recovery));
                }
            }

            if m.regen_time == 0 && t_avg.total_lairs > 0 {
                if use_char && party < 2 {
                    if f64::from(avg_dmg) > t_char.damage_threshold
                        || exp_info.n_hitpoint_recovery > 0.0
                    {
                        if !inp.vs_char_defense_config.is_empty()
                            && inp.vs_char_defense_config != inp.char_defense_description
                        {
                            lv.add("")
                                .text("Monster damage vs Character defenses sims may be stale.")
                                .vbold(true)?;
                            lv.add("")
                                .text("Old or base damage utilized where missing. Recalculate from the from options menu.")
                                .vbold(false)?;
                        } else if inp.vs_char_defense_config.is_empty()
                            && !inp.char_defense_description.is_empty()
                        {
                            lv.add("")
                                .text("Monster damage vs Character defenses not calculated.")
                                .vbold(true)?;
                            lv.add("")
                                .text("Old or base damage utilized where missing. Calculate from options menu.")
                                .vbold(false)?;
                        }
                    }
                } else if party > 1 && (!inp.party_damage_calculated || !inp.dont_prompt_calc_party)
                {
                    lv.add("")
                        .text("Monster damage vs Party defense sims may be incomplete. Calculate from options menu.")
                        .vbold(true)?;
                }
            }
        } else {
            // before 1.83 the label row has no value and the old script value follows
            legacy_script(env, inp, m, number, st, lv, &summoned, possy_n, lair_mode)?;
        }
    }

    // done_scripting:
    let t_avg = st.t_avg.clone();
    let possy = at(inp.possy, number)?;
    if t_avg.total_lairs > 0 || (nmr >= 1.82 && possy.scaled() > 0) {
        lv.blank();
        let r = lv.add("Lair Stats");
        r.bold();
        if m.regen_time > 0 && t_avg.mobs.scaled() > 0 {
            r.text("Note: Mobs with regen time >0 are not included in lair stats");
        }
    }

    if t_avg.total_lairs > 0 {
        lair_stats(
            env,
            ab,
            inp,
            m,
            number,
            st,
            lv,
            &t_avg,
            atype,
            char_attack_spell,
            acc_tag,
            use_char,
        )?;
    } else if nmr >= 1.82 && possy.scaled() > 0 {
        lv.add("Avg # Mobs / Lair").text(cur_text(possy));
        let chance = at(inp.spawn_chance, number)?;
        if chance.scaled() > 0 {
            lv.add("Avg Spawn Chance").text(format!(
                "{}%  (the chance for this monster to spawn per lair)",
                cur_text(chance.checked_mul(cur(100))?)
            ));
        }
    }

    if nmr < 1.83 {
        let r = lv.add(" ");
        r.text("This database is out of date and unable to use all features.");
        r.vcolor(SYS_GREY)?;
    }
    lv.blank();
    Ok(())
}

/// The part of the pane that only data before 1.83 has: `Exp/((Dmg*2)+HP)` and the script value.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn legacy_script(
    env: &mut CharLairEnv<'_>,
    _inp: &MonsterDetailInputs<'_>,
    m: &Monster,
    number: i32,
    st: &mut State<'_>,
    lv: &mut Lv,
    summoned: &str,
    possy_n: Res<Currency>,
    lair_mode: bool,
) -> Res<()> {
    let ctx = env.callouts.damage.ctx;
    let nmr = ctx.nmr_ver;
    let use_char = env.callouts.damage.character.global_filter;
    let t_avg = st.t_avg.clone();
    let exp = st.exp;
    let avg_dmg = st.avg_dmg;
    let hp = cur(m.hp);
    let mut exp_dmg_hp = cur(0);
    // (tAvgLairInfo.nTotalLairs = 0 Or optMonsterFilter(1) = False) And (optMonsterFilter(1) = False Or nNMRVer < 1.83)
    if (t_avg.total_lairs == 0 || !lair_mode) && (!lair_mode || nmr < 1.83) {
        if avg_dmg > 0 || m.hp > 0 {
            let denom = cur_f(f64::from(vb::mul_i32(avg_dmg, 2)?))?.checked_add(hp)?;
            exp_dmg_hp = cur_round(exp.checked_div(denom)?, 2)?.checked_mul(cur(100))?;
        } else {
            exp_dmg_hp = exp;
        }
        lv.add("Exp/((Dmg*2)+HP)").text(format!(
            "{} ({} / (({} x 2) + {})) * 100{}",
            fmt_pos(exp_dmg_hp.to_f64(), "#,#")?,
            cur_text(exp),
            avg_dmg,
            m.hp,
            st.defense_desc
        ));
        if !use_char && vs_char_at(st, number)?.scaled() >= 0 {
            lv.add(" ").text(
                "Calculated damage vs character defenses not utilized because global filter is disabled",
            );
        }
    }
    if t_avg.total_lairs > 0 && lair_mode {
        return Ok(());
    }

    let mut max_before = cur(36);
    let mut lair_pct = cur(0);
    if contains_ci(summoned, "(lair)") {
        let poss_spawns = instr_count(summoned, "(lair)");
        let possy = possy_n?;
        if possy.scaled() > 0 {
            max_before = cur_round(max_before.checked_div(possy)?, 2)?;
        }
        lair_pct = if cur(poss_spawns).scaled() < max_before.scaled() {
            cur_round(cur(poss_spawns).checked_div(max_before)?, 2)?
        } else {
            cur(1)
        };
    }
    let mut possy_pct = cur(1);
    let mut script = cur(0);
    let possy = possy_n?;
    if possy.scaled() > 0 {
        possy_pct = cur(1).checked_add(possy.checked_sub(cur(1))?.checked_div(cur(5))?)?;
        if possy_pct.scaled() > cur(3).scaled() {
            possy_pct = cur(3);
        }
        if possy_pct.scaled() < cur(1).scaled() {
            possy_pct = cur(1);
        }
    }
    if m.regen_time == 0 && lair_pct.scaled() > 0 {
        if possy_pct.scaled() > cur(1).scaled()
            && (t_avg.total_lairs == 0 || !lair_mode)
            && (avg_dmg > 0 || m.hp != 0)
        {
            let denom = cur_f(f64::from(vb::mul_i32(avg_dmg, 2)?))?
                .checked_add(hp)?
                .checked_mul(possy_pct)?;
            exp_dmg_hp = cur_round(exp.checked_div(denom)?, 2)?.checked_mul(cur(100))?;
        }
        script = exp_dmg_hp.checked_mul(lair_pct)?;
    }
    let sv = script.to_f64();
    if sv > 0.0 {
        let s_script = if sv > 1_000_000_000.0 {
            format!("{}M", vb::format(sv / 1_000_000.0, "#,#")?)
        } else if sv > 1_000_000.0 {
            format!("{}K", vb::format(sv / 1000.0, "#,#")?)
        } else {
            fmt_pos(round_up(sv), "#,#")?
        };
        lv.add("");
        let r = lv.add("Script Value");
        if lair_pct.scaled() < cur(1).scaled() || possy_pct.scaled() > cur(1).scaled() {
            if possy_pct.scaled() > cur(1).scaled() {
                let mut t = format!("{s_script} calculated by (Exp/(((Dmg*2)+HP)*Possy%))");
                if lair_pct.scaled() < cur(1).scaled() {
                    t.push_str("*Lair%");
                }
                r.text(t);
                let mut t2 = format!(
                    "Calculation: (({} / ( (({} x 2) + {}) * {})) * 100)",
                    cur_text(exp),
                    avg_dmg,
                    m.hp,
                    cur_text(possy_pct)
                );
                if lair_pct.scaled() < cur(1).scaled() {
                    t2.push_str(&format!(" * {}", cur_text(lair_pct)));
                }
                lv.add(" ").text(t2);
                lv.add(" ").text(format!(
                    "DMG+HP inflated by {}% to account for mob's possy",
                    cur_text(possy_pct.checked_mul(cur(100))?.checked_sub(cur(100))?)
                ));
            } else {
                r.text(format!(
                    "{s_script} calculated from [Exp/((Dmg*2)+HP)] above"
                ));
            }
        } else {
            r.text("Same as [Exp/((Dmg*2)+HP)] above as there are no effects from monster possy or lair difficiency");
        }
        if lair_pct.scaled() < cur(1).scaled() {
            lv.add(" ").text(format!(
                "Value reduced by {}% due insufficient number of mobs/lairs to outpace max regen",
                cur_text(cur(1).checked_sub(lair_pct)?.checked_mul(cur(100))?)
            ));
        }
    }
    if nmr < 1.82 {
        lv.add("")
            .text("Note: This database is old and does not provide max regen per lair.  Calculations limited.")
            .vbold(true)?;
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::cognitive_complexity
)]
fn lair_stats(
    env: &mut CharLairEnv<'_>,
    ab: &AbilityEnv<'_>,
    inp: &MonsterDetailInputs<'_>,
    m: &Monster,
    number: i32,
    st: &mut State<'_>,
    lv: &mut Lv,
    t: &LairInfo,
    atype: AttackType,
    attack_spell: i32,
    acc_tag: &str,
    use_char: bool,
) -> Res<()> {
    let character = env.callouts.damage.character;
    let hide = inp.hide_record_numbers;
    let max_regen = t.max_regen;
    let backstab = character.attack.backstab;
    let weapon0 = env.callouts.damage.ctx.stats.weapon_number[0];

    lv.add("Total Lairs").text(t.total_lairs.to_string());

    let chance = at(inp.spawn_chance, number)?;
    {
        let r = lv.add("AVG # Mobs/Lair");
        if chance.scaled() > 0 {
            r.text(format!(
                "{}  ({}% avg chance for this monster to spawn/lair)",
                cur_text(at(inp.possy, number)?),
                cur_text(chance.checked_mul(cur(100))?)
            ));
        } else {
            r.text(cur_text(max_regen));
        }
    }

    {
        let r = lv.add("AVG DMG/mob");
        r.text(format!("{}/mob/round", put_commas(&cur_text(t.avg_dmg))));
        if t.damage_mitigated > 0 {
            r.vappend(&format!(" ({} dmg/round mitigated)", t.damage_mitigated))?;
        }
    }
    if t.rtk > 1.0 || t.rtc > 1.0 {
        lv.add("AVG Rounds").text(format!(
            "{} RTK/mob, {} RTC/lair",
            vb::cstr(t.rtk),
            vb::cstr(t.rtc)
        ));
    }
    if t.rtc > 1.0 || t.avg_dmg.scaled() != t.avg_dmg_clear_round.scaled() {
        let mut s = format!("{}/round", put_commas(&cur_text(t.avg_dmg_clear_round)));
        if t.avg_dmg_clear.scaled() > 0 {
            s.push_str(&format!(
                ", {}/clear",
                put_commas(&cur_text(t.avg_dmg_clear))
            ));
        }
        lv.add("AVG DMG/clear")
            .text(format!("{s} (average damage taken, before any healing)"));
    }
    if t.avg_delay > 0.0 && max_regen.scaled() > 0 {
        let mr = max_regen.to_f64();
        let n1 = vb::round(vb::div(t.avg_delay * 60.0, mr * 5.0)?, 0)?;
        let mut s = format!(
            " (lairs/regen period: {} @ {} RTC",
            vb::cstr(n1),
            cur_text(max_regen)
        );
        if t.rtc > mr {
            let n2 = vb::round(vb::div(t.avg_delay * 60.0, t.rtc * 5.0)?, 0)?;
            if n2 != n1 {
                s.push_str(&format!(" [{} @ {} RTC]", vb::cstr(n2), vb::cstr(t.rtc)));
            }
        }
        s.push(')');
        lv.add("AVG Regen")
            .text(format!("{} minutes{s}", vb::cstr(t.avg_delay)));
    }
    if t.avg_walk.scaled() > 0 {
        lv.add("AVG Walk")
            .text(format!("{} rooms lair to lair", cur_text(t.avg_walk)));
    }
    {
        let lair_exp = cur_f(m.avg_lair_exp)?;
        let per_mob = vb::round(vb::div(lair_exp.to_f64(), max_regen.to_f64())?, 0)?;
        lv.add("AVG Exp").text(format!(
            "{}  ({}/mob)",
            put_commas(&cur_text(lair_exp)),
            put_commas(&vb::cstr(per_mob))
        ));
    }
    lv.add("ACC Maj/Max")
        .text(format!("{} / {}", t.accy_majority, t.accy_max));
    lv.add("AVG HP").text(format!(
        "{}  ({}/mob)",
        put_commas(&t.avg_hp.to_string()),
        vb::cstr(vb::round(
            vb::div(f64::from(t.avg_hp), max_regen.to_f64())?,
            0
        )?)
    ));
    lv.add("AVG AC/DR")
        .text(format!("{}/{}", t.avg_ac, t.avg_dr));
    if t.avg_dodge > 0 {
        let r = lv.add("AVG Dodge");
        if use_char && vb::val(acc_tag) > 0.0 {
            let tag_v = vb::val(acc_tag);
            let q = vb::div(f64::from(t.avg_dodge) * 10.0, vb::fix(tag_v / 8.0))?;
            r.text(format!(
                "{} ({}% @ {} accy)",
                t.avg_dodge,
                vb::cstr(vb::fix(q)),
                vb::cstr(tag_v)
            ));
        } else {
            r.text(t.avg_dodge.to_string());
        }
    }
    {
        let r = lv.add("AVG BS Defense");
        r.text(t.avg_bs_defense.to_string());
        if t.avg_bs_defense != 0 && backstab {
            r.color(BLUE);
            r.vcolor(BLUE)?;
        }
    }
    lv.add("AVG MR").text(t.avg_mr.to_string());
    if t.num_anti_magic > 0 && t.mobs.scaled() > 0 {
        let pct = vb::round(
            vb::div(f64::from(t.num_anti_magic), t.mobs.to_f64())? * 100.0,
            0,
        )?;
        lv.add("AVG Anti-Magic")
            .text(format!("{}% of mobs/lair", vb::cstr(pct)));
    }
    if t.avg_rcol != 0 || t.avg_rfir != 0 || t.avg_rsto != 0 || t.avg_rlit != 0 || t.avg_rwat != 0 {
        let mut s = String::new();
        for (v, n) in [
            (t.avg_rcol, "Cold"),
            (t.avg_rfir, "Fire"),
            (t.avg_rsto, "Stone"),
            (t.avg_rlit, "Litng"),
            (t.avg_rwat, "Water"),
        ] {
            if v != 0 {
                s = auto_append(
                    &s,
                    &format!("{n}: {}{v}", if v > 0 { "+" } else { "" }),
                    ", ",
                );
            }
        }
        lv.add("AVG EL. Resist").text(s);
    }
    if i32::from(t.magic_lvl) + i32::from(t.max_magic_lvl) > 0 {
        let r = lv.add("Effective MagicLVL");
        let mut s = format!(
            "{} (immune to attacks with < {} magic/hitmagic)",
            t.magic_lvl, t.magic_lvl
        );
        if t.max_magic_lvl > t.magic_lvl {
            s.push_str(&format!(" - Max MagicLVL in lairs: {}", t.max_magic_lvl));
        }
        r.text(s);
        let mut red = false;
        if weapon0 > 0
            && matches!(
                atype,
                AttackType::PhysAttack | AttackType::PhysBash | AttackType::PhysSmash
            )
            && st.weapon_magic < i32::from(t.max_magic_lvl)
        {
            red = true;
        }
        if atype != AttackType::OneShot
            && backstab
            && st.backstab_weapon_magic < i32::from(t.max_magic_lvl)
        {
            red = true;
        }
        if red {
            r.color(RED);
            r.vcolor(RED)?;
            r.bold();
        }
    }
    if i32::from(t.spell_immu_lvl) + i32::from(t.max_spell_immu_lvl) > 0 {
        let r = lv.add("Effective SpellImmu");
        let mut s = format!(
            "{} (immune to spells < level {})",
            t.spell_immu_lvl, t.spell_immu_lvl
        );
        if t.max_spell_immu_lvl > t.spell_immu_lvl {
            s.push_str(&format!(
                " - Max SpellImmu in lairs: {}",
                t.max_spell_immu_lvl
            ));
        }
        r.text(s);
        if attack_spell > 0 && matches!(atype, AttackType::Spell | AttackType::SpellAny) {
            let sc = &st.spellcast;
            let req = i32::from(sc.n_required_level);
            let castl = i32::from(sc.n_cast_level);
            let mx = i32::from(t.max_spell_immu_lvl);
            if (req > 0 && req < mx) || (req == 0 && castl > 0 && castl < mx) {
                r.color(RED);
                r.vcolor(RED)?;
                r.bold();
            }
        }
    }

    let mobs = t.mobs.to_f64();
    let spell_atk = attack_spell > 0 && matches!(atype, AttackType::Spell | AttackType::SpellAny);
    let share =
        |n: i16| -> Res<f64> { vb::round(vb::div(f64::from(n), round_up(mobs))? * 100.0, 0) };
    for (count, flag, label, lt100_lt) in [
        (t.num_undeads, AR_UNDEAD, "Effective Undead", true),
        (t.num_living, AR_LIVING, "Effective Living", false),
        (t.num_animals, AR_ANIMAL, "Effective Animals", true),
    ] {
        let show = if label == "Effective Living" {
            (count > 0 && f64::from(count) < mobs) || (mobs > 0.0 && (st.attack_flags & flag) != 0)
        } else {
            (count > 0 && mobs > 0.0) || (mobs > 0.0 && (st.attack_flags & flag) != 0)
        };
        let _ = lt100_lt;
        if show {
            let r = lv.add(label);
            r.text(format!("{}% of mobs/lair", vb::cstr(share(count)?)));
            if (st.attack_flags & flag) != 0 && spell_atk && share(count)? < 100.0 {
                r.color(RED);
                r.vcolor(RED)?;
                r.bold();
            }
        }
    }

    if t.mob_list.contains(',') {
        let arr: Vec<&str> = t.mob_list.split(',').collect();
        let ub = arr.len() as i32 - 1;
        let mut y = 0;
        let mut first = true;
        let mut x = 0i32;
        while x <= ub {
            let item = arr[x as usize];
            if vb::val(item) != f64::from(number) {
                let r = if first {
                    lv.add("Other Lair Mobs")
                } else {
                    lv.add("")
                };
                first = false;
                // CLng-free: the name lookup takes the text as a Long
                let n = vb::clng(vb::val(item))?;
                r.text(get_monster_name(ab.names, n, hide));
                r.tag("monster");
                r.vtag(item.to_string())?;
                y += 1;
                if y > 14 && ub > 20 {
                    lv.add("").text(format!("... plus {} more.", ub - y));
                    x = ub + 1;
                }
            }
            x += 1;
        }
        if first {
            // every entry was the monster itself: MME had already added the label row
            lv.add("Other Lair Mobs");
        }
    }
    Ok(())
}
