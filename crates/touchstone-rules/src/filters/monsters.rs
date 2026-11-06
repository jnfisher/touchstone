//! `FilterMonsters`: which monsters the Monsters list shows, and with which `AddMonster2LV` arguments.
//! Card P3-30.
//!
//! The port takes the controls of the explorer ([`MonsterFilter`], with the extra filters of the
//! "Monster Filters" window in [`MonsterExtras`]; `Default` is MME's start-up state) and walks the
//! Monsters table in order, as MME does. For every monster that is listed it makes a [`MonsterAdd`]: the
//! record, the optional arguments of the `AddMonster2LV` call, and the HP and HP regen of the profile at
//! that moment (the experience filter overwrites them for the rest of the pass when no character is in
//! use). The finished list is in [`MonsterList`]; a caller that builds the rows too gets each call as it
//! happens through the `add` callback, with the lair env, because `FilterMonsters` and `AddMonster2LV`
//! share the lair cache (`tLastAvgLairInfo`).
//!
//! # Lair mode
//!
//! `optMonsterFilter(1)` and the party boxes are the env's [`PartyFilter`](crate::character::PartyFilter).
//! In lair mode the lair averages replace the monster's own HP and damage, and the experience filter is on
//! exp/hr. `remove_filter` (`RemoveFilters`) lists every monster in solo mode with the defaults, so the
//! env's party must already be solo, as MME's click handler makes it.
//!
//! # Locals that outlive an iteration
//!
//! MME declares the working variables outside its loop and resets them after the remove-filter jump and the
//! three hard-coded skips. The arguments of an `AddMonster2LV` call made from those two places are therefore
//! the values the previous iteration left (or zero), and the port keeps them in one struct for that reason.
//!
//! # Errors
//!
//! A VB6 run-time error ends the pass where it happens with the list built so far (`aborted`), as MME's
//! `Resume out` does.

use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Vb6Error};

use super::Answer;
use crate::RulesError;
use crate::character::CharacterProfile;
use crate::combat::{DamageRequest, get_damage_output};
use crate::exp_per_hour::{ExpPerHourInput, ExpPerHourSettings, calc_exp_per_hour};
use crate::lairs::{
    CharLairEnv, LairCallouts, LairEnv, LairInfo, LairLocsInput, count_lairs_by_max_regen,
    get_lair_averages_from_locs, instr_count,
};
use crate::monster_glue::{GlueEnv, get_monster_attack_summary};
use crate::rows::monster::MonsterRowArgs;

type Res<T> = Result<T, Vb6Error>;

/// The value the extra filters use for "no limit".
pub const NO_LIMIT: i32 = 9999;

/// The "Monster Filters" window's settings (`filter_Monster_*`). `Default` is `Form_Load`'s state.
#[derive(Debug, Clone, PartialEq)]
pub struct MonsterExtras {
    /// `filter_Monster_bExtrasEnabled`: the extras apply at all.
    pub enabled: bool,
    pub armour_class: i32,
    pub damage_resist: i32,
    pub magic_res: i32,
    pub bs_def: i32,
    pub game_limit: i32,
    pub avg_lair_exp: f64,
    pub atk_accuracy_maj: i32,
    pub atk_accuracy_max: i32,
    pub dodge: i32,
    pub num_lairs: i32,
    pub num_mobs_lte: f64,
    pub num_mobs_gte: f64,
    pub follow_gte: i32,
    pub follow_lte: i32,
    pub at_least_lairs: i32,
    /// `filter_Monster_nAbilities(0..2, 0..2)`: ability number, operator (0 at most, else at least), value.
    pub abilities: [[i32; 3]; 3],
    pub is_undead: bool,
    pub non_hostile_vs_evil: bool,
    pub non_hostile_vs_ng: bool,
    pub reachable_only: bool,
    pub at_least_lairs_on: bool,
    pub drops_cash: bool,
    pub drops_r: bool,
    pub drops_p: bool,
    pub drops_g: bool,
    pub drops_s: bool,
    pub atk_no_poison: bool,
    pub atk_no_confusion: bool,
    pub atk_no_fear: bool,
    /// List the monsters that fail too, greyed.
    pub show_all: bool,
}

impl Default for MonsterExtras {
    fn default() -> Self {
        Self {
            enabled: false,
            armour_class: NO_LIMIT,
            damage_resist: NO_LIMIT,
            magic_res: NO_LIMIT,
            bs_def: NO_LIMIT,
            game_limit: NO_LIMIT,
            avg_lair_exp: 0.0,
            atk_accuracy_maj: NO_LIMIT,
            atk_accuracy_max: NO_LIMIT,
            dodge: NO_LIMIT,
            num_lairs: 0,
            num_mobs_lte: f64::from(NO_LIMIT),
            num_mobs_gte: 0.0,
            follow_gte: 0,
            follow_lte: NO_LIMIT,
            at_least_lairs: 1,
            abilities: [[0; 3]; 3],
            is_undead: false,
            non_hostile_vs_evil: false,
            non_hostile_vs_ng: false,
            reachable_only: false,
            at_least_lairs_on: false,
            drops_cash: false,
            drops_r: false,
            drops_p: false,
            drops_g: false,
            drops_s: false,
            atk_no_poison: false,
            atk_no_confusion: false,
            atk_no_fear: false,
            show_all: false,
        }
    }
}

/// The Monsters tab's controls of the active mode (by monster, or by lair solo or party), plus the extras.
#[derive(Debug, Clone, PartialEq)]
pub struct MonsterFilter {
    /// `cmbMonsterRegen.ListIndex`: 0 regen at most the box, 1 at least, other: no regen filter.
    pub regen_op: i32,
    /// `txtMonsterRegen.Text`.
    pub regen_text: String,
    /// `txtMonsterHP.Text`.
    pub hp_text: String,
    /// `txtMonsterEXP.Text` (its `Tag`, the number the filter reads, follows from it: [`exp_filter_tag`]).
    pub exp_text: String,
    /// `txtMonsterDamage.Text`.
    pub damage_text: String,
    /// `chkMonMagic.Value = 1`.
    pub magic: bool,
    /// `txtMonMagic.Text`.
    pub magic_text: String,
    pub extras: MonsterExtras,
}

impl Default for MonsterFilter {
    /// The values `RemoveFilters` writes into the boxes.
    fn default() -> Self {
        Self {
            regen_op: 0,
            regen_text: "999".to_string(),
            hp_text: "99999".to_string(),
            exp_text: "0".to_string(),
            damage_text: "99999".to_string(),
            magic: false,
            magic_text: "999".to_string(),
            extras: MonsterExtras::default(),
        }
    }
}

fn blank(s: &str) -> bool {
    s.trim_matches(' ').is_empty()
}

impl MonsterFilter {
    /// The boxes after `FilterMonsters` has put the defaults back into the empty ones.
    #[must_use]
    pub fn with_blanks_filled(&self) -> Self {
        let d = Self::default();
        let mut f = self.clone();
        if blank(&f.regen_text) {
            f.regen_text = d.regen_text;
        }
        if blank(&f.damage_text) {
            f.damage_text = d.damage_text;
        }
        if blank(&f.hp_text) {
            f.hp_text = d.hp_text;
        }
        if blank(&f.exp_text) {
            f.exp_text = d.exp_text;
        }
        if blank(&f.magic_text) {
            f.magic_text = d.magic_text;
        }
        f
    }

    /// The filter `FilterMonsters(True)` runs with: the boxes cleared, the extras off.
    #[must_use]
    pub fn cleared(&self) -> Self {
        Self {
            extras: MonsterExtras {
                enabled: false,
                ..self.extras.clone()
            },
            ..Self::default()
        }
    }
}

/// The `Tag` of `txtMonsterEXP` that `txtMonsterEXP_Change` derives from the text: the number the
/// experience filter compares with (a trailing K or M multiplies it).
#[must_use]
pub fn exp_filter_tag(text: &str) -> f64 {
    // sTemp = val(text): the text of the number, which the handler writes back into the box
    let n = vb::val(&vb::cstr(vb::val(text)));
    let last = text
        .trim_matches(' ')
        .chars()
        .last()
        .map(|c| c.to_ascii_uppercase());
    match last {
        Some('M') => n * 1_000_000.0,
        Some('K') => n * 1000.0,
        _ => n,
    }
}

/// `ReachEnsureComputed` and `ReachIsMonsterOK` (modReachability).
pub trait Reachability {
    /// `ReachIsMonsterOK(number, level)`: can a character of this level get to the monster?
    fn is_monster_ok(&self, number: i32, level: i32) -> bool;
}

/// No reachability data: every monster is reachable, as MME answers when nothing is computed.
#[derive(Debug, Clone, Copy, Default)]
pub struct AllReachable;

impl Reachability for AllReachable {
    fn is_monster_ok(&self, _number: i32, _level: i32) -> bool {
        true
    }
}

/// What decides whether `FilterMonsters` asks to calculate the monster damage first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamagePrompts {
    /// `sMonsterDamageVsCharDefenseConfig`.
    pub vs_char_config: String,
    /// `sGlobalCharDefenseDescription`.
    pub char_defense_description: String,
    /// `bDontPromptCalcCharMonsterDamage`.
    pub dont_prompt_char: bool,
    /// `bMonsterDamageVsPartyCalculated`.
    pub party_calculated: bool,
    /// `bDontPromptCalcPartyMonsterDamage`.
    pub dont_prompt_party: bool,
}

impl Default for DamagePrompts {
    /// No question is asked.
    fn default() -> Self {
        Self {
            vs_char_config: String::new(),
            char_defense_description: String::new(),
            dont_prompt_char: true,
            party_calculated: false,
            dont_prompt_party: true,
        }
    }
}

/// The `MsgBox` questions of `FilterMonsters`, all yes/no/cancel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonsterDialog {
    /// "RE-Calculate Monster Damage vs Character Defenses first?"
    RecalcChar,
    /// "Calculate Monster Damage vs Character Defenses first?"
    CalcChar,
    /// "RE-Calculate Monster Damage vs Party Defenses first?"
    RecalcParty,
    /// "Calculate Monster Damage vs Party Defenses first?"
    CalcParty,
}

/// The globals and boxes `FilterMonsters` reads besides its own tab and the lair env.
pub struct MonsterFilterInputs<'a> {
    /// `bOnlyInGame`.
    pub only_in_game: bool,
    /// `UseExpMulti`.
    pub use_exp_multi: bool,
    /// The exp/hr settings (`bGlobal_ceph*`).
    pub settings: &'a ExpPerHourSettings,
    /// `nMonsterPossy`, indexed by monster number.
    pub possy: &'a [Currency],
    /// `Val(txtGlobalLevel(0).Text)`, the level the reachability test uses.
    pub global_level: f64,
    pub reach: &'a dyn Reachability,
    pub prompts: DamagePrompts,
}

/// One `AddMonster2LV` call.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonsterAdd {
    pub number: i32,
    /// The optional arguments after the list and the profile.
    pub args: MonsterRowArgs,
    /// `tChar.nHP` and `tChar.nHPRegen` when the call was made.
    pub hp: f64,
    pub hp_regen: f64,
}

/// The finished Monsters list.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MonsterList {
    pub entries: Vec<MonsterAdd>,
    /// A question was cancelled (or there is no monster): MME left the list as it was.
    pub cancelled: bool,
    /// A VB6 error ended the procedure; `entries` is what was built.
    pub aborted: bool,
    /// Some monster was filtered out (the "*Monsters*" caption).
    pub filtered: bool,
    /// The questions asked, with the answers.
    pub asked: Vec<(MonsterDialog, Answer)>,
    /// Some answer was Yes to the damage calculation: `Some(false)` for the character, `Some(true)` for the party.
    pub recalc: Option<bool>,
    /// Data 1.83 on: the captions of columns 11 and 12 (Exp/Hour or Exp/(Dmg+HP); Resting % or Lair Exp).
    pub headers: Option<(&'static str, &'static str)>,
}

fn rt(e: RulesError) -> Vb6Error {
    match e {
        RulesError::Vb6(_) | RulesError::Unsupported(_) => Vb6Error::Overflow,
    }
}

fn cur_f(x: f64) -> Res<Currency> {
    Currency::from_f64(x)
}

fn cur(v: i32) -> Currency {
    Currency::from_scaled(i64::from(v) * 10_000)
}

/// MME's locals that survive from one monster to the next.
struct Locals {
    magic_lvl: i32,
    mob_dodge: i16,
    exp_per_hour: f64,
    time_recovering: f64,
    pass_exp: Currency,
    pass_recovery: f64,
    damage_out: Currency,
    surprise_damage_out: i32,
    first_round: i32,
    min_round: i32,
    surprise_min: i32,
    surprise_chance: i16,
    does_not_match: bool,
    n_exp: Currency,
    avg_dmg: i32,
    hp: f64,
    hp_regen: f64,
}

impl Locals {
    fn new(t_char: &CharacterProfile) -> Self {
        let z = Currency::from_scaled(0);
        Self {
            magic_lvl: 0,
            mob_dodge: 0,
            exp_per_hour: 0.0,
            time_recovering: 0.0,
            pass_exp: z,
            pass_recovery: 0.0,
            damage_out: z,
            surprise_damage_out: 0,
            first_round: 0,
            min_round: 0,
            surprise_min: 0,
            surprise_chance: 0,
            does_not_match: false,
            n_exp: z,
            avg_dmg: 0,
            hp: t_char.hp,
            hp_regen: t_char.hp_regen,
        }
    }

    /// The reset at the top of an iteration, after the jumps.
    fn reset(&mut self) {
        self.magic_lvl = 0;
        self.mob_dodge = -1;
        self.exp_per_hour = 0.0;
        self.pass_exp = cur(-1);
        self.pass_recovery = -1.0;
        self.damage_out = cur(-9999);
        self.surprise_damage_out = -9999;
        self.first_round = 0;
        self.min_round = 0;
        self.surprise_min = 0;
        self.surprise_chance = 0;
        self.does_not_match = false;
    }

    /// The call `AddMonster2LV(lvMonsters, tChar, nDamageOut, ...)` with what the locals hold (each
    /// converted to the parameter's type, which can overflow).
    fn add(&self, number: i32) -> Res<MonsterAdd> {
        Ok(MonsterAdd {
            number,
            args: MonsterRowArgs {
                damage_out: vb::clng(self.damage_out.to_f64())?,
                pass_exp: self.pass_exp,
                pass_recovery: self.pass_recovery,
                surprise_damage_out: self.surprise_damage_out,
                surprise_min_dmg: self.surprise_min,
                surprise_chance: self.surprise_chance,
                first_round_dmg: self.first_round,
                min_round_dmg: self.min_round,
                does_not_match_filter: self.does_not_match,
            },
            hp: self.hp,
            hp_regen: self.hp_regen,
        })
    }
}

/// Does the monster pass? `Skip` is MME's `GoTo skip`.
#[derive(PartialEq, Eq)]
enum Verdict {
    Pass,
    Skip,
}

/// Port of frmMain `FilterMonsters`. `remove_filter` is `bRemoveFilter` (`RemoveFilters`). `t_char` is
/// `PopulateCharacterProfile(tChar)` as made after `RefreshCombatHealingValues`. `ask` answers the
/// questions; `add` receives every `AddMonster2LV` call at the moment MME makes it.
#[must_use]
pub fn filter_monsters(
    env: &mut CharLairEnv<'_>,
    inp: &MonsterFilterInputs<'_>,
    filter: &MonsterFilter,
    t_char: &CharacterProfile,
    remove_filter: bool,
    ask: &mut dyn FnMut(MonsterDialog) -> Answer,
    add: &mut dyn FnMut(&mut CharLairEnv<'_>, &MonsterAdd),
) -> MonsterList {
    let mut out = MonsterList::default();
    let f = if remove_filter {
        filter.cleared()
    } else {
        filter.with_blanks_filled()
    };
    if env.callouts.world.monsters.is_empty() {
        out.cancelled = true;
        return out;
    }
    let nmr = env.callouts.damage.ctx.nmr_ver;
    let lair_mode = env.query.party.lair_mode && !remove_filter;

    if lair_mode && !ask_damage(env, inp, t_char, ask, &mut out) {
        out.cancelled = true;
        return out;
    }
    if nmr >= 1.83 {
        out.headers = Some(if lair_mode {
            (
                "Exp/Hour",
                if inp.settings.recovery_only {
                    "Lair Exp"
                } else {
                    "Resting %"
                },
            )
        } else {
            ("Exp/(Dmg+HP)", "Lair Exp")
        });
    }

    env.cache.last_avg_lair_info = env.get_lair_info("", 0);
    let mut pass = Pass {
        inp,
        f: &f,
        x: &f.extras,
        lair_mode,
        loc: Locals::new(t_char),
        t_char,
        regen_limit: vb::val(&f.regen_text),
        hp_limit: vb::val(&f.hp_text),
        magic_limit: vb::val(&f.magic_text),
        damage_limit: vb::val(&f.damage_text),
        exp_tag: exp_filter_tag(&f.exp_text),
        filter_abilities: f.extras.enabled && f.extras.abilities.iter().any(|a| a[0] > 0),
        filtered: false,
    };
    let numbers: Vec<i32> = env
        .callouts
        .world
        .monsters
        .iter()
        .map(|m| m.number)
        .collect();
    for number in numbers {
        match pass.one(env, number, remove_filter) {
            Ok(Some(call)) => {
                add(env, &call);
                out.entries.push(call);
            }
            Ok(None) => {}
            Err(_) => {
                out.aborted = true;
                break;
            }
        }
        out.filtered |= pass.filtered;
    }
    out
}

/// The "calculate the monster damage first" questions. False when the answer was Cancel.
fn ask_damage(
    env: &CharLairEnv<'_>,
    inp: &MonsterFilterInputs<'_>,
    t_char: &CharacterProfile,
    ask: &mut dyn FnMut(MonsterDialog) -> Answer,
    out: &mut MonsterList,
) -> bool {
    let p = &inp.prompts;
    let use_character = env.query.use_character;
    let dialog = if use_character && t_char.party < 2 {
        let differs = p.vs_char_config != p.char_defense_description;
        if !p.vs_char_config.is_empty() && differs && !p.dont_prompt_char {
            Some((MonsterDialog::RecalcChar, false))
        } else if differs && !p.dont_prompt_char {
            Some((MonsterDialog::CalcChar, false))
        } else {
            None
        }
    } else if t_char.party > 1 {
        if p.party_calculated && !p.dont_prompt_party {
            Some((MonsterDialog::RecalcParty, true))
        } else if !p.party_calculated && !p.dont_prompt_party {
            Some((MonsterDialog::CalcParty, true))
        } else {
            None
        }
    } else {
        None
    };
    let Some((d, party)) = dialog else {
        return true;
    };
    let a = ask(d);
    out.asked.push((d, a));
    match a {
        Answer::Cancel => false,
        Answer::Yes => {
            out.recalc = Some(party);
            true
        }
        Answer::No => true,
    }
}

struct Pass<'p> {
    inp: &'p MonsterFilterInputs<'p>,
    f: &'p MonsterFilter,
    x: &'p MonsterExtras,
    lair_mode: bool,
    loc: Locals,
    t_char: &'p CharacterProfile,
    regen_limit: f64,
    hp_limit: f64,
    magic_limit: f64,
    damage_limit: f64,
    exp_tag: f64,
    filter_abilities: bool,
    filtered: bool,
}

impl Pass<'_> {
    /// One monster: the call to make, if any.
    fn one(
        &mut self,
        env: &mut CharLairEnv<'_>,
        number: i32,
        remove_filter: bool,
    ) -> Res<Option<MonsterAdd>> {
        // the `filtered` flag of this monster is reported through `self.filtered`
        self.filtered = false;
        let world = env.callouts.world;
        let Some(m) = world.get_monster(number) else {
            return Ok(None);
        };
        if self.inp.only_in_game && !m.in_game {
            return Ok(None);
        }
        if remove_filter {
            return self.loc.add(number).map(Some);
        }
        let nmr = env.callouts.damage.ctx.nmr_ver;
        let name = m.name.as_deref().unwrap_or("");
        // hard-coded skips of lair mode (before the locals are reset)
        let hard = nmr >= 1.83
            && self.lair_mode
            && match number {
                1086 => name == "cloaked figure",
                1087 => name == "disenchanter",
                597 => name == "Dao Lord",
                _ => false,
            };
        let verdict = if hard {
            Verdict::Skip
        } else {
            self.loc.reset();
            self.check(env, number)?
        };
        if verdict == Verdict::Pass {
            return self.loc.add(number).map(Some);
        }
        self.filtered = true;
        if self.x.enabled && self.x.show_all {
            self.loc.does_not_match = true;
            return self.loc.add(number).map(Some);
        }
        Ok(None)
    }
}

// `filtered` is per monster, read by the loop after each call.
impl Pass<'_> {
    #[allow(clippy::too_many_lines)]
    fn check(&mut self, env: &mut CharLairEnv<'_>, number: i32) -> Res<Verdict> {
        use Verdict::{Pass, Skip};
        let world = env.callouts.world;
        let Some(m) = world.get_monster(number) else {
            return Ok(Skip);
        };
        let nmr = env.callouts.damage.ctx.nmr_ver;
        let profile = env.callouts.damage.ctx.profile;
        let x = self.x;
        let summoned = m.summoned_by.clone().unwrap_or_default();

        if x.enabled {
            if x.reachable_only {
                let level = vb::clng(self.inp.global_level)?;
                if !self.inp.reach.is_monster_ok(number, level) {
                    return Ok(Skip);
                }
            }
            if x.drops_cash {
                if m.r + m.p + m.g + m.s + m.c == 0 {
                    return Ok(Skip);
                }
            } else if x.drops_r {
                if m.r < 1 {
                    return Ok(Skip);
                }
            } else if x.drops_p {
                if m.p + m.r < 1 {
                    return Ok(Skip);
                }
            } else if x.drops_g {
                if m.g + m.p + m.r < 1 {
                    return Ok(Skip);
                }
            } else if x.drops_s && m.s + m.g + m.p + m.r < 1 {
                return Ok(Skip);
            }
            if x.armour_class != NO_LIMIT && m.armour_class > x.armour_class {
                return Ok(Skip);
            }
            if x.damage_resist != NO_LIMIT && m.damage_resist > x.damage_resist {
                return Ok(Skip);
            }
            if x.magic_res != NO_LIMIT && m.magic_res > x.magic_res {
                return Ok(Skip);
            }
            if x.follow_gte > 0 && m.follow_pct < x.follow_gte {
                return Ok(Skip);
            }
            if x.follow_lte != NO_LIMIT && m.follow_pct > x.follow_lte {
                return Ok(Skip);
            }
            if nmr >= 1.83 && x.bs_def != NO_LIMIT && m.bs_defense > x.bs_def {
                return Ok(Skip);
            }
            if x.is_undead && m.undead == 0 {
                return Ok(Skip);
            }
            if x.non_hostile_vs_evil && !matches!(m.align, 6 | 0 | 4 | 3) {
                return Ok(Skip);
            }
            if x.non_hostile_vs_ng && !matches!(m.align, 0 | 4 | 3) {
                return Ok(Skip);
            }
        }

        match self.f.regen_op {
            0 if f64::from(m.regen_time) > self.regen_limit => return Ok(Skip),
            1 if f64::from(m.regen_time) < self.regen_limit => return Ok(Skip),
            _ => {}
        }

        // the lair this monster comes from
        if nmr >= 1.83 && env.cache.last_avg_lair_info.group_index != summoned {
            let cfg = env.query.global_attack_config.to_string();
            let avg = get_lair_averages_from_locs(
                env,
                &summoned,
                &LairLocsInput {
                    nmr_ver: nmr,
                    summoned_by: &summoned,
                    global_attack_config: &cfg,
                },
            );
            env.cache.last_avg_lair_info = avg;
        }
        let lair: LairInfo = env.cache.last_avg_lair_info.clone();

        if nmr >= 1.83 && self.lair_mode && lair.total_lairs > 0 {
            if f64::from(lair.avg_hp) > self.hp_limit {
                return Ok(Skip);
            }
        } else if f64::from(m.hp) > self.hp_limit {
            return Ok(Skip);
        }

        if x.enabled {
            if nmr >= 1.83 {
                if x.game_limit != NO_LIMIT && m.game_limit > x.game_limit {
                    return Ok(Skip);
                }
                if x.avg_lair_exp > 0.0 && m.avg_lair_exp < x.avg_lair_exp {
                    return Ok(Skip);
                }
            }
            if x.num_lairs > 0 {
                if nmr >= 1.83 {
                    if lair.total_lairs < x.num_lairs {
                        return Ok(Skip);
                    }
                } else if summoned.to_lowercase().contains("(lair)") {
                    let spawns = instr_count(&summoned, "(lair)");
                    if spawns < x.num_lairs {
                        return Ok(Skip);
                    }
                }
            }
            let possy = |env: &CharLairEnv<'_>| -> Res<f64> {
                let _ = env;
                usize::try_from(number)
                    .ok()
                    .and_then(|i| self.inp.possy.get(i))
                    .map(|c| c.to_f64())
                    .ok_or(Vb6Error::Overflow)
            };
            if x.at_least_lairs_on
                && nmr >= 1.82
                && (x.num_mobs_lte != f64::from(NO_LIMIT) || x.num_mobs_gte > 0.0)
            {
                let min = if x.num_mobs_gte > 0.0 {
                    x.num_mobs_gte
                } else {
                    1.0
                };
                let max = if x.num_mobs_lte != f64::from(NO_LIMIT) {
                    x.num_mobs_lte
                } else {
                    f64::from(NO_LIMIT)
                };
                if count_lairs_by_max_regen(&summoned, nmr, min, max) < x.at_least_lairs {
                    return Ok(Skip);
                }
            } else {
                if x.num_mobs_lte != f64::from(NO_LIMIT) {
                    if possy(env)? > x.num_mobs_lte {
                        return Ok(Skip);
                    }
                    if lair.max_regen.to_f64() > x.num_mobs_lte {
                        return Ok(Skip);
                    }
                }
                if x.num_mobs_gte > 0.0 {
                    if nmr >= 1.83 {
                        if lair.max_regen.to_f64() < x.num_mobs_gte {
                            return Ok(Skip);
                        }
                    } else if possy(env)? < x.num_mobs_gte {
                        return Ok(Skip);
                    }
                }
            }
        }

        let n_party = self.t_char.party;
        self.loc.avg_dmg =
            if nmr >= 1.83 && self.lair_mode && lair.total_lairs > 0 && m.regen_time == 0 {
                vb::clng(lair.avg_dmg_lair.to_f64())?
            } else {
                vb::clng(env.callouts.monster_damage(number, n_party))?
            };

        // attack filters
        if x.enabled
            && (x.atk_accuracy_maj != NO_LIMIT
                || x.atk_accuracy_max != NO_LIMIT
                || x.atk_no_poison
                || x.atk_no_confusion
                || x.atk_no_fear)
        {
            let s = get_monster_attack_summary(
                world,
                &GlueEnv {
                    profile,
                    nmr_ver: nmr,
                },
                number,
                false,
                true,
            );
            if x.atk_no_poison && s.atk_poison {
                return Ok(Skip);
            }
            if x.atk_no_confusion && s.atk_confusion {
                return Ok(Skip);
            }
            if x.atk_no_fear && s.atk_fear {
                return Ok(Skip);
            }
            if x.atk_accuracy_maj != NO_LIMIT && s.acc_majority > x.atk_accuracy_maj {
                return Ok(Skip);
            }
            if x.atk_accuracy_max != NO_LIMIT && s.acc_max > x.atk_accuracy_max {
                return Ok(Skip);
            }
        }

        // ability filters
        let by_lair_exp = nmr >= 1.83 && self.lair_mode;
        if self.f.magic
            || (self.exp_tag > 0.0 && by_lair_exp)
            || (x.enabled && x.dodge != NO_LIMIT)
            || self.filter_abilities
        {
            let mut pass = [false; 3];
            let mut has = [false; 3];
            if self.filter_abilities {
                for (p, a) in pass.iter_mut().zip(&x.abilities) {
                    *p = a[0] < 1;
                }
            }
            for slot in &m.abilities {
                if slot.ability == 0 {
                    continue;
                }
                if self.filter_abilities {
                    for y in 0..3 {
                        if x.abilities[y][0] == slot.ability {
                            has[y] = true;
                            if x.abilities[y][1] == 0 {
                                if slot.value <= x.abilities[y][2] {
                                    pass[y] = true;
                                }
                            } else if slot.value >= x.abilities[y][2] {
                                pass[y] = true;
                            }
                        }
                    }
                }
                match slot.ability {
                    28 => self.loc.magic_lvl = slot.value,
                    34 if slot.value > 0 => {
                        self.loc.mob_dodge =
                            i16::try_from(slot.value).map_err(|_| Vb6Error::Overflow)?;
                    }
                    _ => {}
                }
            }
            if self.filter_abilities {
                for y in 0..3 {
                    // "at most N" with N above 0 counts a missing ability as 0 and passes
                    if !has[y] && x.abilities[y][1] == 0 && x.abilities[y][2] > 0 {
                        pass[y] = true;
                    }
                    if !pass[y] {
                        return Ok(Skip);
                    }
                }
            }
        }
        if self.f.magic && f64::from(self.loc.magic_lvl) > self.magic_limit {
            return Ok(Skip);
        }
        if x.enabled && x.dodge != NO_LIMIT && i32::from(self.loc.mob_dodge) > x.dodge {
            return Ok(Skip);
        }

        if self.exp_tag > 0.0 {
            self.loc.n_exp = if self.inp.use_exp_multi {
                cur_f(m.exp * m.exp_multi)?
            } else {
                cur_f(m.exp)?
            };
            if by_lair_exp {
                self.lair_exp(env, number, &lair, &summoned)?;
            }
            if self.loc.n_exp.to_f64() < self.exp_tag {
                return Ok(Skip);
            }
        }

        if self.damage_limit > 0.0
            && (nmr < 1.83 || !self.lair_mode || self.inp.settings.recovery_only)
            && f64::from(self.loc.avg_dmg) > self.damage_limit
        {
            return Ok(Skip);
        }
        Ok(Pass)
    }

    /// The experience filter in lair mode: exp/hr of the average lair or of the single monster.
    #[allow(clippy::too_many_lines)]
    fn lair_exp(
        &mut self,
        env: &mut CharLairEnv<'_>,
        number: i32,
        lair: &LairInfo,
        summoned: &str,
    ) -> Res<()> {
        let world = env.callouts.world;
        let m = world.get_monster(number).ok_or(Vb6Error::Overflow)?;
        let profile = env.callouts.damage.ctx.profile;
        let use_character = env.query.use_character;
        let t = self.t_char;

        if !use_character && t.party < 2 {
            // no character: a generic one that survives two rounds of this monster
            self.loc.hp = f64::from(vb::mul_i32(self.loc.avg_dmg, 2)?);
            self.loc.hp_regen = self.loc.hp * 0.05;
        }
        let hp = vb::clng(self.loc.hp)?;
        let hp_regen = vb::clng(self.loc.hp_regen)?;
        if m.regen_time == 0 && lair.total_lairs > 0 {
            let info = calc_exp_per_hour(
                &ExpPerHourInput {
                    n_exp: lair.avg_exp,
                    n_regen_time: lair.avg_delay,
                    n_num_mobs: lair.max_regen.to_f64(),
                    n_total_lairs: lair.total_lairs,
                    n_poss_spawns: lair.poss_spawns,
                    n_rtk: lair.rtk,
                    n_char_dmg: f64::from(lair.damage_out),
                    n_char_hp: hp,
                    n_char_hp_regen: hp_regen,
                    n_mob_dmg: lair.avg_dmg_lair.to_f64(),
                    n_mob_hp: lair.avg_hp,
                    n_mob_hp_regen: 0,
                    n_damage_threshold: vb::clng(t.damage_threshold)?,
                    n_spell_cost: vb::cint(t.spell_attack_cost)?,
                    n_spell_overhead: t.spell_overhead,
                    n_char_mana: vb::clng(t.max_mana)?,
                    n_char_mp_regen: vb::clng(t.mana_regen)?,
                    n_meditate_rate: vb::clng(t.meditate_rate)?,
                    n_avg_walk: lair.avg_walk.to_f64(),
                    n_walk_speed: t.walk_speed,
                    n_surprise_dmg: f64::from(lair.surprise_damage_out),
                    n_surprise_min_dmg: f64::from(lair.surprise_min_damage_out),
                    n_surprise_chance: lair.surprise_chance,
                    n_char_first_round_dmg: f64::from(lair.first_round_damage_out),
                    n_min_round_dmg: f64::from(lair.min_round_damage_out),
                },
                self.inp.settings,
                profile,
            )
            .map_err(rt)?;
            self.loc.exp_per_hour = info.n_exp_per_hour;
            self.loc.time_recovering = info.n_time_recovering;
            self.loc.pass_recovery = info.n_time_recovering;
        } else if m.regen_time > 0 || summoned.to_lowercase().contains("room") {
            let d = get_damage_output(
                &env.callouts.damage,
                &DamageRequest {
                    single_monster: number,
                    ..DamageRequest::default()
                },
            );
            let l = |c: Currency| vb::clng(c.to_f64());
            self.loc.damage_out = d.average_damage;
            self.loc.first_round = l(d.first_round_damage)?;
            self.loc.min_round = l(d.min_round_damage)?;
            self.loc.surprise_damage_out = l(d.surprise_damage)?;
            self.loc.surprise_min = l(d.surprise_min_damage)?;
            self.loc.surprise_chance = d.surprise_damage_chance;
            let info = calc_exp_per_hour(
                &ExpPerHourInput {
                    n_exp: self.loc.n_exp,
                    n_regen_time: f64::from(m.regen_time),
                    n_num_mobs: 1.0,
                    n_total_lairs: -1,
                    n_poss_spawns: 0,
                    n_rtk: 0.0,
                    n_char_dmg: self.loc.damage_out.to_f64(),
                    n_char_hp: hp,
                    n_char_hp_regen: hp_regen,
                    n_mob_dmg: f64::from(self.loc.avg_dmg),
                    n_mob_hp: m.hp,
                    n_mob_hp_regen: m.hp_regen,
                    n_damage_threshold: vb::clng(t.damage_threshold)?,
                    n_spell_cost: vb::cint(t.spell_attack_cost)?,
                    n_spell_overhead: t.spell_overhead,
                    n_char_mana: vb::clng(t.max_mana)?,
                    n_char_mp_regen: vb::clng(t.mana_regen)?,
                    n_meditate_rate: vb::clng(t.meditate_rate)?,
                    n_avg_walk: 0.0,
                    n_walk_speed: t.walk_speed,
                    n_surprise_dmg: f64::from(self.loc.surprise_damage_out),
                    n_surprise_min_dmg: f64::from(self.loc.surprise_min),
                    n_surprise_chance: self.loc.surprise_chance,
                    n_char_first_round_dmg: f64::from(self.loc.first_round),
                    n_min_round_dmg: f64::from(self.loc.min_round),
                },
                self.inp.settings,
                profile,
            )
            .map_err(rt)?;
            self.loc.exp_per_hour = info.n_exp_per_hour;
            self.loc.time_recovering = info.n_time_recovering;
            self.loc.pass_recovery = info.n_time_recovering;
        }

        let mut n_exp = cur_f(self.loc.exp_per_hour)?;
        if n_exp.scaled() > 0 && t.party > 1 {
            n_exp = cur_f(vb::round_half_even(
                n_exp.checked_div(cur(i32::from(t.party)))?.to_f64(),
            ))?;
        }
        self.loc.n_exp = Currency::from_scaled(n_exp.scaled());
        self.loc.pass_exp = self.loc.n_exp;
        Ok(())
    }
}
