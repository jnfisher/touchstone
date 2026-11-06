//! Spell detail (modMain `PullSpellDetail`).
//!
//! The text box content is built from `CalculateSpellCast`, `PullSpellEQ` and a few lookups. MME
//! passes the location list to `PullSpellEQ`, which adds rows for teleports, summons, textblocks and
//! learned spells as a side effect; [`spell_eq_rows`] reproduces those rows (the P3-04 port of
//! `PullSpellEQ` returns only its text).

use touchstone_model::{Spell, World};
use touchstone_vb6::{self as vb, Currency, Vb6Error};

use super::shop::{cur_str, room_name};
use super::{DetailKind, DetailRow};
use crate::Profile;
use crate::ability_text::{AbilityEnv, get_class_name, get_monster_name, get_spell_name};
use crate::exp::get_max_level;
use crate::locations::{
    LocationArgs, LocationKind, LocationLairs, LocationList, LocationOptions, LocationRow,
    get_locations,
};
use crate::pipeline::CharacterState;
use crate::spell_cast::{
    CastRequest, SpellCastEnv, SpellCasterProfile, UsableEnv, spell_attack_type_enum,
};
use crate::spells::{MinMaxRequest, PullArgs, get_current_spell_min_max};

type Res<T> = Result<T, Vb6Error>;

const CRLF: &str = "\r\n";

/// What `PullSpellDetail` reads from frmMain besides the loaded character.
#[derive(Debug, Clone)]
pub struct SpellDetailOptions {
    /// `bStartup`.
    pub startup: bool,
    /// `chkSpellOptions(0).Value = 1`.
    pub combat_option: bool,
    /// `Val(txtSpellOptions(0).Text)`: the target's magic resistance for the combat figures.
    pub combat_value: f64,
    /// `chkSpellOptions(2).Value = 1`: the target is anti-magic.
    pub anti_magic: bool,
    /// `Val(txtGlobalLevel(1).Text)`: the spell pane only uses the character when this is above 0.
    pub level_box: f64,
    pub nmr_ver: f64,
    /// `bOnlyInGame`, `bDisableKaiAutolearn`, `bHideRecordNumbers`, `gAvgLevelMaxAllStats`.
    pub only_in_game: bool,
    pub disable_kai_autolearn: bool,
    pub hide_record_numbers: bool,
    pub avg_level_max_all_stats: f64,
    pub locations: LocationOptions,
}

/// What `PullSpellDetail` leaves in its controls.
#[derive(Debug, Clone, Default)]
pub struct SpellDetail {
    /// `DetailTB.Text` (lines are separated by CR LF).
    pub text: String,
    /// The same facts as (label, value, kind) rows.
    pub rows: Vec<DetailRow>,
    /// The location list: the rows `PullSpellEQ` adds, then "Learned From" (header "(learn) ") and
    /// "Casted By".
    pub locations: LocationList,
}

fn auto(base: &str, add: &str, glue: &str) -> String {
    if base.is_empty() {
        add.to_string()
    } else if add.trim_matches(' ').is_empty() {
        base.to_string()
    } else {
        format!("{base}{glue}{add}")
    }
}

/// `Replace(s, find, "", , , vbTextCompare)` for an ASCII `find`.
fn remove_ci(s: &str, find: &str) -> String {
    let (ls, lf) = (s.to_ascii_lowercase(), find.to_ascii_lowercase());
    let mut out = String::new();
    let mut at = 0;
    while let Some(i) = ls[at..].find(&lf) {
        out.push_str(&s[at..at + i]);
        at += i + lf.len();
    }
    out.push_str(&s[at..]);
    out
}

/// `InStr(1, s, find, vbTextCompare)` for an ASCII `find`: the 1-based character position or 0.
fn instr_ci(s: &str, find: &str) -> i32 {
    let (ls, lf) = (s.to_ascii_lowercase(), find.to_ascii_lowercase());
    ls.find(&lf)
        .map_or(0, |b| ls[..b].chars().count() as i32 + 1)
}

fn trim(s: &str) -> String {
    s.trim_matches(' ').to_string()
}

/// Port of modMMudFunc `GetSpellTargetsEnum`.
#[must_use]
pub fn spell_targets_text(n: i32) -> String {
    match n {
        0 => "User",
        1 => "Self",
        2 => "Self or User",
        3 => "Divided Area (not self)",
        4 => "Monster",
        5 => "Divided Area (incl self)",
        6 => "Any",
        7 => "Item",
        8 => "Monster or User",
        9 => "Divided Attack Area",
        10 => "Divided Party Area",
        11 => "Full Area",
        12 => "Full Attack Area",
        13 => "Full Party Area",
        _ => return format!("Unknown ({n})"),
    }
    .to_string()
}

fn cur_clng(c: Currency) -> Res<i32> {
    // CLng of a Currency: banker's rounding
    let s = c.scaled();
    let (q, r) = (s.div_euclid(10_000), s.rem_euclid(10_000));
    let q = match (r * 2).cmp(&10_000) {
        std::cmp::Ordering::Less => q,
        std::cmp::Ordering::Greater => q + 1,
        std::cmp::Ordering::Equal => q + (q & 1),
    };
    i32::try_from(q).map_err(|_| Vb6Error::Overflow)
}

// ---- the rows PullSpellEQ adds to the list --------------------------------------------------------------

struct EqRows<'a> {
    world: &'a World,
    env: &'a AbilityEnv<'a>,
    rows: Vec<LocationRow>,
}

fn row(kind: LocationKind, text: String, tag: String) -> LocationRow {
    LocationRow {
        kind,
        text,
        tag: Some(tag),
        cells: Vec::new(),
    }
}

impl EqRows<'_> {
    fn teleport(&mut self, map: i32, room: i32) {
        self.rows.push(row(
            LocationKind::Room,
            format!("Teleport: {}", room_name(self.world, map, room)),
            format!("{map}/{room}"),
        ));
    }

    fn spell_name(&self, n: i32) -> String {
        get_spell_name(self.world, n, self.env.hide_record_numbers)
    }

    fn monster_name(&self, n: i32) -> String {
        get_monster_name(self.world, n, self.env.hide_record_numbers)
    }

    fn summon(&mut self, n: i32, tag: String) {
        self.rows.push(row(
            LocationKind::Monster,
            format!("Summon: {}", self.monster_name(n)),
            tag,
        ));
    }

    /// One `PullSpellEQ(.., lv)` call: `depth` is `nSpellNest` before the call.
    #[allow(clippy::too_many_lines)]
    fn walk(
        &mut self,
        spell: i32,
        calc_level: bool,
        level: i16,
        overrides: (i32, i32),
        depth: i32,
    ) -> Res<()> {
        let nest = depth + 1;
        if self.env.spells.is_empty() || nest > 19 {
            return Ok(());
        }
        let Some(sp) = self.env.spells.iter().find(|s| s.number == spell) else {
            return Ok(());
        };
        let mut level = level;
        let mut use_level = calc_level;
        if use_level {
            if level == 0
                && let Some(t) = self.env.global_level_text
            {
                level = vb::cint(vb::val(t))?;
            }
            if i32::from(level) > sp.cap && sp.cap > 0 {
                level = i16::try_from(sp.cap).map_err(|_| Vb6Error::Overflow)?;
            }
            if i32::from(level) < sp.req_level {
                level = i16::try_from(sp.req_level).map_err(|_| Vb6Error::Overflow)?;
            }
            if level < 1 {
                level = i16::try_from(sp.req_level).map_err(|_| Vb6Error::Overflow)?;
            }
            if level == 0 {
                use_level = false;
            }
        }
        let base = get_current_spell_min_max(
            Some(sp),
            self.env.avg_level_max_all_stats,
            &MinMaxRequest {
                use_level,
                level,
                no_header: false,
                override_min: overrides.0,
                override_max: overrides.1,
                spell_bonus: 0,
            },
        );
        let (n_min, n_max) = (base.n_min, base.n_max);
        let (s_min, s_max) = (base.s_min.as_str(), base.s_max.as_str());
        let tele_map = |sp: &Spell| {
            sp.abilities
                .iter()
                .filter(|a| a.ability == 141)
                .map(|a| a.value)
                .next_back()
                .unwrap_or(0)
        };
        for slot in &sp.abilities {
            let (abil, v) = (slot.ability, slot.value);
            if abil == 0 || abil == 122 || abil == 137 {
                continue;
            }
            i16::try_from(abil).map_err(|_| Vb6Error::Overflow)?;
            if v == 0 {
                match abil {
                    140 => {
                        let map = tele_map(sp);
                        if map > 0 {
                            let (lo, hi) = (vb::clng(vb::val(s_min))?, vb::clng(vb::val(s_max))?);
                            for y in lo..=hi {
                                self.teleport(map, y);
                            }
                        }
                    }
                    148 => {
                        let (lo, hi) = (vb::clng(vb::val(s_min))?, vb::clng(vb::val(s_max))?);
                        for y in lo..=hi {
                            self.rows.push(row(
                                LocationKind::Textblock,
                                format!("Execute: Textblock {y}"),
                                y.to_string(),
                            ));
                        }
                    }
                    151 => {
                        let (lo, hi) = (cur_clng(n_min)?, cur_clng(n_max)?);
                        if n_min >= n_max {
                            self.walk(lo, calc_level, level, (0, 0), nest)?;
                        } else {
                            self.walk(lo, calc_level, level, (0, 0), nest)?;
                            let from = cur_clng(Currency::from_scaled(n_min.scaled() + 10_000))?;
                            for y in from..=hi {
                                self.walk(y, calc_level, level, (0, 0), nest)?;
                            }
                        }
                    }
                    12 => {
                        let (lo, hi) = (cur_clng(n_min)?, cur_clng(n_max)?);
                        if n_min >= n_max {
                            self.summon(lo, cur_str(n_min));
                        } else {
                            self.summon(lo, cur_str(n_min));
                            let from = cur_clng(Currency::from_scaled(n_min.scaled() + 10_000))?;
                            for y in from..=hi {
                                self.summon(y, y.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            } else {
                i16::try_from(v).map_err(|_| Vb6Error::Overflow)?;
                match abil {
                    148 => self.rows.push(row(
                        LocationKind::Textblock,
                        format!("Execute: Textblock {v}"),
                        v.to_string(),
                    )),
                    12 => self.summon(v, v.to_string()),
                    140 => {
                        let map = tele_map(sp);
                        if map > 0 {
                            self.teleport(map, v);
                        }
                    }
                    164 | 23 | 51 | 52 | 80 | 97 | 98 | 100 | 108..=113 | 119 | 138 | 144 | 178 => {
                    }
                    // GetAbilityStats(.., lv): learn/temp-spell rows and nested equations
                    42 | 160 => self.rows.push(row(
                        LocationKind::Spell,
                        format!("Spell: {}", self.spell_name(v)),
                        v.to_string(),
                    )),
                    43 | 153 | 151 => self.walk(v, calc_level, 0, (0, 0), nest)?,
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

/// The rows `PullSpellEQ(True, level, , lv, ..., min, max)` adds to the list for the spell, in
/// MME's order (teleport, summon, textblock, learn-spell and nested-equation rows).
#[must_use]
pub fn spell_eq_rows(
    env: &AbilityEnv,
    world: &World,
    spell: i32,
    level: i16,
    overrides: (i32, i32),
) -> Vec<LocationRow> {
    let mut w = EqRows {
        world,
        env,
        rows: Vec::new(),
    };
    // A VB6 error ends the equation where it happens; the rows added so far stay.
    let _ = w.walk(spell, true, level, overrides, env.nest.get());
    w.rows
}

// ---- PullSpellDetail ---------------------------------------------------------------------------------------

fn class_list(world: &World, classes: &str) -> Res<String> {
    let strip = |s: &str| s.replace(['(', ')'], "");
    let mut out = String::new();
    let parts: Vec<String> = if classes.contains(',') {
        classes.split(',').map(strip).collect()
    } else {
        vec![strip(classes)]
    };
    for (x, p) in parts.iter().enumerate() {
        if x > 0 {
            out.push_str(", ");
        }
        // the number goes through a String array and back into the Integer parameter
        let n = vb::cint(vb::val(&vb::cstr(vb::val(p))))?;
        out.push_str(&get_class_name(world, i32::from(n)));
    }
    Ok(out)
}

/// The facts [`pull_spell_detail`] builds, in MME's order, before they are joined into the text.
struct Build {
    detail: String,
    rows: Vec<DetailRow>,
}

/// Port of modMain `PullSpellDetail`: the text box and location list for a spell, at the level of
/// the loaded character (`character`) or the maximum level without one. `character` is also what
/// `chkGlobalFilter` being on means here: the pane uses it only when `opts.level_box > 0` and the
/// class can use the spell.
///
/// A VB6 error ends the procedure as in MME: the text stays empty and later steps do not run.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn pull_spell_detail(
    profile: Profile,
    world: &World,
    lairs: &mut dyn LocationLairs,
    character: Option<&CharacterState>,
    number: i32,
    opts: &SpellDetailOptions,
) -> SpellDetail {
    let mut out = SpellDetail::default();
    if opts.startup {
        return out;
    }
    let Some(sp) = world.get_spell(number) else {
        out.text = "spell not found".to_string();
        out.rows = vec![DetailRow::new("", "spell not found", DetailKind::Message)];
        return out;
    };
    let _ = detail_inner(profile, world, lairs, character, sp, opts, &mut out);
    out
}

#[allow(clippy::too_many_lines)]
fn detail_inner(
    profile: Profile,
    world: &World,
    lairs: &mut dyn LocationLairs,
    character: Option<&CharacterState>,
    sp: &Spell,
    opts: &SpellDetailOptions,
    out: &mut SpellDetail,
) -> Res<()> {
    let combat = opts.combat_option && opts.combat_value > 0.0;
    let filter = character.filter(|c| c.character.global_filter);
    let mut use_character = filter.is_some() && opts.level_box > 0.0;
    let mut usable = false;
    if use_character {
        let ch = filter.expect("checked");
        let class = ch.character.class.unwrap_or(0);
        usable = UsableEnv {
            profile,
            spells: &world.spells,
            classes: &world.classes,
            nmr_ver: opts.nmr_ver,
            only_in_game: opts.only_in_game,
            disable_kai_autolearn: opts.disable_kai_autolearn,
        }
        .spell_is_usable(sp.number, class, 0, 0, false);
        use_character = usable;
    }
    // LocationLV.ListItems.clear
    out.locations = LocationList::default();

    let mut env = AbilityEnv::for_world(world, profile);
    env.hide_record_numbers = opts.hide_record_numbers;
    env.avg_level_max_all_stats = opts.avg_level_max_all_stats;
    let level_text = filter.map(|c| c.character.level.to_string());
    env.global_level_text = level_text.as_deref();

    // PopulateCharacterProfile: the loaded character, or the "nobody" profile at the maximum level
    let (cast_level, caster, bless, heal_cost) = match character.filter(|_| use_character) {
        Some(c) => (
            c.profile.level,
            c.spell_caster(),
            c.bless_mana.map_or(0.0, |b| vb::val(&vb::cstr(b))),
            c.character.attack.heal_cost,
        ),
        None => (
            get_max_level(opts.avg_level_max_all_stats).unwrap_or(0),
            SpellCasterProfile::default(),
            0.0,
            0.0,
        ),
    };
    let cast_env = SpellCastEnv {
        profile,
        spells: &world.spells,
        text: &env,
        avg_level_max_all_stats: opts.avg_level_max_all_stats,
    };
    let mut req = CastRequest {
        spell: sp.number,
        cast_level,
        ..CastRequest::default()
    };
    if combat {
        req.vs_mr = vb::clng(opts.combat_value)?;
        req.vs_anti_magic = opts.anti_magic;
    }
    let cast = cast_env.calculate_spell_cast(&caster, &req);
    let cast_lvl = cast.n_cast_level;
    let mut b = Build {
        detail: String::new(),
        rows: Vec::new(),
    };
    let mut br = false;

    if combat && (cast.b_does_damage || cast.b_does_heal) && !cast.s_avg_round.is_empty() {
        b.detail = auto(&b.detail, &cast.s_avg_round, CRLF);
        b.rows.push(DetailRow::new(
            "Combat",
            &cast.s_avg_round,
            DetailKind::Cost,
        ));
        br = true;
    }
    if (combat || !use_character)
        && !cast.s_mma.is_empty()
        && cast.n_min_cast > 0
        && (cast.n_min_cast != cast.n_max_cast || cast.n_max_cast != cast.n_avg_cast)
    {
        b.detail = auto(&b.detail, &cast.s_mma, CRLF);
        b.rows
            .push(DetailRow::new("Min/Avg/Max", &cast.s_mma, DetailKind::Cost));
        br = true;
    }
    let helped = bless > 0.0 || heal_cost > 0.0;
    if combat && use_character && cast.n_oom > 0 {
        b.detail = auto(&b.detail, &format!("OOM in {} rounds", cast.n_oom), CRLF);
        let casts = vb::idiv_i16(cast.n_oom, cast.n_duration);
        if cast.n_duration > 1 && helped {
            b.detail.push_str(&format!(
                " (after {} casts, with current heals/bless set)",
                casts?
            ));
        } else if helped {
            b.detail.push_str(" (with current heals/bless set)");
        } else if cast.n_duration > 1 {
            b.detail.push_str(&format!(" (after {} casts)", casts?));
        }
        if cast.n_duration > 1 && cast.n_cast_chance > 0 && cast.n_cast_chance < 100 {
            b.detail
                .push_str(&format!(" @ {}% chance to cast", cast.n_cast_chance));
        }
        br = true;
    }
    if usable && cast.n_cast_level > 0 {
        b.detail = trim(&remove_ci(
            &b.detail,
            &format!("(@lvl {}) ", cast.n_cast_level),
        ));
        b.detail = trim(&remove_ci(
            &b.detail,
            &format!(" (@lvl {})", cast.n_cast_level),
        ));
    }
    if br {
        b.detail.push_str(CRLF);
    }

    // the equation and the rows PullSpellEQ adds
    let overrides = (cast.n_min_cast, cast.n_max_cast);
    out.locations.rows = spell_eq_rows(&env, world, sp.number, cast_lvl, overrides);
    let mut eq = env.pull_spell_eq(&PullArgs {
        calc_level: true,
        level: cast_lvl,
        spell: sp.number,
        override_min: overrides.0,
        override_max: overrides.1,
        ..PullArgs::default()
    });
    let mut removes = String::new();
    if instr_ci(&eq, " -- RemovesSpells") > 0 {
        let pos = instr_ci(&eq, " -- RemovesSpells(");
        removes = trim(&vb::mid(&eq, pos + 4, Some(vb::len(&eq)))?);
        eq = vb::left(&eq, vb::len(&eq) - vb::len(&removes) - 4)?;
        let head = 18;
        removes = trim(&vb::mid(
            &removes,
            head - 3,
            Some(vb::len(&removes) - head + 3),
        )?);
    }
    if sp.cap != 0 && (!cast.s_lvl_increases.is_empty() || eq.contains('@')) {
        eq = format!("LVL Cap: {} {eq}", sp.cap);
    }
    if !eq.is_empty() {
        b.detail = auto(&b.detail, &eq, CRLF);
        b.rows.push(DetailRow::new("Effect", &eq, DetailKind::Fact));
    }
    if cast.n_mana_cost > 0 && cast.n_num_casts > 1.0 {
        let each = vb::cstr(vb::fix(f64::from(cast.n_mana_cost) / cast.n_num_casts));
        b.detail = auto(&b.detail, &format!(" ({each} mana/ea)"), "");
        b.rows
            .push(DetailRow::new("Mana each", each, DetailKind::Cost));
    }
    if !cast.s_lvl_increases.is_empty() {
        b.detail = auto(&b.detail, &cast.s_lvl_increases, CRLF);
        b.rows.push(DetailRow::new(
            "LVL Increases",
            &cast.s_lvl_increases,
            DetailKind::Fact,
        ));
    }
    if !combat && use_character && cast.n_oom > 0 {
        b.detail = auto(&b.detail, &format!("OOM in {} rounds", cast.n_oom), CRLF);
        if bless > 0.0 {
            b.detail.push_str(" (with current bless set)");
        }
    }

    let target = spell_targets_text(sp.targets);
    b.detail.push_str(&format!("{CRLF}{CRLF}Target: {target}"));
    b.rows
        .push(DetailRow::new("Target", target, DetailKind::Fact));
    let can_learn =
        sp.learnable || (sp.magery == 5 && !opts.disable_kai_autolearn && sp.req_level > 0);
    if sp.diff < 200 && (sp.diff != 0 || can_learn) {
        b.detail = auto(&b.detail, &format!("Difficulty: {}", sp.diff), ", ");
        b.rows.push(DetailRow::new(
            "Difficulty",
            sp.diff.to_string(),
            DetailKind::Fact,
        ));
    }
    let att = spell_attack_type_enum(
        i16::try_from(sp.att_type).map_err(|_| Vb6Error::Overflow)?,
        false,
    );
    b.detail = auto(&b.detail, &format!("Attack Type: {att}"), ", ");
    b.rows
        .push(DetailRow::new("Attack Type", att, DetailKind::Fact));
    if opts.nmr_ver >= 1.8 {
        let chance = if cast.n_full_resist_chance > 0 {
            format!(" ({}%)", cast.n_full_resist_chance)
        } else {
            String::new()
        };
        let resist = match sp.type_of_resists {
            1 => format!("Fully-Resistable by Anti-Magic Only{chance}"),
            2 => format!("Fully-Resistable by All{chance}"),
            _ => "Can Not be Fully-Resisted".to_string(),
        };
        b.detail.push_str(&format!(", {resist}"));
        b.rows
            .push(DetailRow::new("Resist", resist, DetailKind::Fact));
    }
    if opts.nmr_ver >= 1.7 {
        let classes = sp.classes.as_deref().unwrap_or("");
        if vb::len(classes) > 2 && classes != "(*)" {
            let list = class_list(world, classes)?;
            b.detail.push_str(&format!(
                "{CRLF}Class Restricted (via learning method): {list}"
            ));
            b.rows
                .push(DetailRow::new("Classes", list, DetailKind::Fact));
        }
    }
    if !removes.is_empty() {
        b.detail.push_str(&format!("{CRLF}Removes: {removes}"));
        b.rows
            .push(DetailRow::new("Removes", &removes, DetailKind::Fact));
    }
    out.text = b.detail;
    out.rows = b.rows;

    get_locations(
        world,
        lairs,
        &opts.locations,
        &LocationArgs {
            loc: sp.learned_from.as_deref().unwrap_or(""),
            dont_clear: true,
            header: "(learn) ",
            ..LocationArgs::default()
        },
        &mut out.locations,
    );
    get_locations(
        world,
        lairs,
        &opts.locations,
        &LocationArgs {
            loc: sp.casted_by.as_deref().unwrap_or(""),
            dont_clear: true,
            ..LocationArgs::default()
        },
        &mut out.locations,
    );
    Ok(())
}
