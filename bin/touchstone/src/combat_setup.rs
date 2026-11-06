//! The combat setup dialog (frmPopUpOptions, FEAT-COMBAT). Card P4-22.
//!
//! Three things come from the server so the page holds no combat rule:
//!
//! - [`options`]: the lists the dialog fills (`RefreshSpells`, `RefreshItems`);
//! - [`preview`]: `RefreshCombatStats`, the numbers and captions under the controls, plus
//!   [`apply_choice`], the half of `PopUpChooseCombatGUI` that turns the controls into the
//!   character's attack globals when the dialog is accepted;
//! - [`attack_names`]: `GetCurrentAttackName`, the caption of the Char tab's button.
//!
//! The preview mixes two sources, as MME does. The dialog's own controls (the `draft`) decide the
//! numbers, but the character profile is built by `PopulateCharacterProfile`, which reads the
//! *saved* attack globals (meditate, backstab weapon, heal cost), not the dialog.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use touchstone_model::{Spell, World};
use touchstone_rules::Profile;
use touchstone_rules::ability_text::AbilityEnv;
use touchstone_rules::attack::{AttackEnv, AttackRequest, calculate_attack};
use touchstone_rules::character::{
    AttackType, AttackTypeMud, Character, Context, PartyFilter, get_current_attack_name,
    get_spell_mana_cost,
};
use touchstone_rules::pipeline::{CharacterState, PipelineOptions, refresh_all};
use touchstone_rules::spell_cast::{
    CastRequest, SpellCastEnv, SpellCasterProfile, calc_rounds_to_oom,
};
use touchstone_rules::spells::get_spell_duration;
use touchstone_rules::stats::{compute_avg_level_max_stats, get_magery_enum};
use touchstone_vb6::{self as vb, Vb6Error};

use crate::calc::{ApiResult, Body, bad, json, vb_error_text};
use crate::character::{AttackDto, CharacterDto, usable_env};
use crate::detail::nmr_ver;
use crate::state::AppState;

type ApiError = (StatusCode, String);

/// `ROUND_SECS` and `SPELL_ROUND_SECS`.
const ROUND_SECS: f64 = 5.0;
const SPELL_ROUND_SECS: f64 = 3.0;

fn unprocessable(msg: &str) -> ApiError {
    (StatusCode::UNPROCESSABLE_ENTITY, msg.to_owned())
}

// ---------- shared setup ----------

/// The character, evaluated as the Char tab does; the dialog needs its panels and weapon globals.
fn evaluate_character(
    world: &World,
    dto: &CharacterDto,
) -> Result<(Character, CharacterState), ApiError> {
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
        avg_level_max_all_stats: compute_avg_level_max_stats(Profile::Stock, &world.races)
            .all_stats,
        ..PipelineOptions::default()
    };
    let st = refresh_all(Profile::Stock, world, &ch, &opts)
        .map_err(|e| unprocessable(&format!("cannot evaluate this character: {e}")))?;
    Ok((ch, st))
}

/// `GetItemName(n, bHideRecordNumbers:=False)`.
fn item_name(world: &World, n: i32) -> String {
    if n == 0 {
        return "None".into();
    }
    world
        .get_item(n)
        .map_or_else(|| n.to_string(), |i| format!("{}({n})", i.name))
}

fn spell_label(s: &Spell) -> String {
    format!("{} ({})", s.name.as_deref().unwrap_or(""), s.number)
}

/// `AutoAppend`: the glue only goes between two non-empty parts.
fn auto_append(base: &str, add: &str, glue: &str) -> String {
    if base.is_empty() {
        add.to_owned()
    } else if add.trim_matches(' ').is_empty() {
        base.to_owned()
    } else {
        format!("{base}{glue}{add}")
    }
}

// ---------- the lists ----------

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct Choice {
    pub number: i32,
    pub label: String,
}

#[derive(Serialize, Debug, Default)]
pub struct Options {
    /// `nNMRVer`: below 1.83 MME has one page and no meditate or heal choices.
    pub nmr_ver: f64,
    /// Settings "No recovery": MME shows a note on the heal page.
    pub recovery_only: bool,
    /// `lblCurrentWeapon`.
    pub current_weapon: String,
    /// `cmbAttackSpell(0)`: learned spells that do damage.
    pub attack_learned: Vec<Choice>,
    /// `cmbAttackSpell(1)`: damage spells the class could use, at any level.
    pub attack_any: Vec<Choice>,
    /// `cmbHealingSpell(0)`.
    pub heal_learned: Vec<Choice>,
    /// `cmbHealingSpell(1)`.
    pub heal_any: Vec<Choice>,
    /// `cmbBackstabWeapon`: the entries after "[Equipped Weapon]" (0) and "[No Weapon (Punch)]" (-1).
    pub backstab_weapons: Vec<Choice>,
}

/// Port of frmPopUpOptions `RefreshSpells` and `RefreshItems`, with `bOnlyInGame` on.
pub fn options(
    world: &World,
    dto: &CharacterDto,
    recovery_only: bool,
) -> Result<Options, ApiError> {
    let (ch, st) = evaluate_character(world, dto)?;
    let class = ch.class.unwrap_or(0);
    let env = usable_env(world);
    let mut out = Options {
        nmr_ver: nmr_ver(&world.info.nmr_version),
        recovery_only,
        current_weapon: match st.stats.weapons[0].number {
            0 => "(no weapon equipped)".to_owned(),
            n => item_name(world, n),
        },
        ..Options::default()
    };
    for s in &world.spells {
        if s.short.as_deref().is_none_or(str::is_empty) {
            continue;
        }
        // Select Case takes the first matching case: drain (8) is damage, never a heal
        let mut has_dmg = false;
        let mut has_heal = false;
        for slot in &s.abilities {
            match slot.ability {
                1 | 8 | 17 => has_dmg = true,
                18 => has_heal = true,
                _ => {}
            }
        }
        if !has_dmg && !has_heal {
            continue;
        }
        let learned = s.number > 0 && ch.learned.spells.contains(&s.number);
        if learned && has_dmg {
            out.attack_learned.push(Choice {
                number: s.number,
                label: spell_label(s),
            });
        }
        if learned && has_heal {
            out.heal_learned.push(Choice {
                number: s.number,
                label: spell_label(s),
            });
        }
        // the other lists leave out timed spells that do not heal, and what the class cannot use
        if !has_heal && (s.dur > 0 || (s.dur_inc > 0 && s.dur_inc_lvls > 0)) {
            continue;
        }
        if !env.spell_is_usable(s.number, class, 0, 0, true) {
            continue;
        }
        let label = format!(
            "{} - LVL {} {}",
            spell_label(s),
            s.req_level,
            get_magery_enum(
                i16::try_from(s.magery).unwrap_or(0),
                i16::try_from(s.magery_lvl).unwrap_or(0)
            )
        );
        if has_dmg {
            out.attack_any.push(Choice {
                number: s.number,
                label: label.clone(),
            });
        }
        if has_heal {
            out.heal_any.push(Choice {
                number: s.number,
                label,
            });
        }
    }
    for it in &world.items {
        // 116 is the backstab accuracy ability
        if it.in_game && it.item_type == 1 && it.abilities.iter().any(|a| a.ability == 116) {
            out.backstab_weapons.push(Choice {
                number: it.number,
                label: format!("{} ({})", it.name, it.number),
            });
        }
    }
    Ok(out)
}

// ---------- the choice ----------

/// The radio button an `eAttackTypeMME` shows as: bash and smash are the weapon radio with a box.
fn attack_radio(t: i32) -> Result<i32, ApiError> {
    match t {
        0..=5 => Ok(if t == 6 || t == 7 { 1 } else { t }),
        6 | 7 => Ok(1),
        _ => Err(bad(format!("attack_type {t} is not 0 to 7"))),
    }
}

fn check_draft(d: &AttackDto) -> Result<(), ApiError> {
    attack_radio(d.attack_type)?;
    if AttackType::from_code(d.attack_type).is_none() {
        return Err(bad(format!("attack_type {} is not 0 to 7", d.attack_type)));
    }
    if !(0..=4).contains(&d.heal_type) {
        return Err(bad("heal_type must be 0 to 4"));
    }
    if !(0..=3).contains(&d.martial_art) {
        return Err(bad("martial_art must be 0 (none) to 3"));
    }
    Ok(())
}

/// The write half of `PopUpChooseCombatGUI`: what the character's attack becomes when the dialog is
/// accepted. It starts from the saved attack and overwrites only what MME overwrites; a choice that
/// is missing its spell (or art, or damage) falls back to one-shot, and a one-shot leaves the
/// backstab flag alone.
#[must_use]
pub fn apply_choice(saved: &AttackDto, d: &AttackDto) -> AttackDto {
    let mut a = saved.clone();
    let radio = attack_radio(d.attack_type).unwrap_or(0);
    let mut set = 0;
    'attack: {
        match radio {
            0 | 1 => set = radio,
            2 | 3 => {
                if d.spell_num <= 0 {
                    break 'attack;
                }
                a.spell_num = d.spell_num;
                if radio == 3 {
                    a.spell_lvl = d.spell_lvl;
                }
                set = radio;
                a.use_meditate = d.use_meditate;
            }
            4 => {
                if d.martial_art == 0 {
                    break 'attack;
                }
                a.martial_art = d.martial_art;
                set = 4;
            }
            _ => {
                if d.manual_p > 0 || d.manual_m > 0 {
                    a.manual_p = d.manual_p;
                    a.manual_m = d.manual_m;
                    set = 5;
                } else {
                    break 'attack;
                }
            }
        }
        if set > 0 {
            if d.backstab {
                a.backstab = true;
                // 0 is the equipped weapon, -1 fists; any other pick must be an item
                if d.backstab_weapon == 0 || d.backstab_weapon == -1 || d.backstab_weapon > 0 {
                    a.backstab_weapon = d.backstab_weapon;
                }
            } else {
                a.backstab = false;
            }
        }
    }
    a.attack_type = if set == 1 && matches!(d.attack_type, 6 | 7) {
        d.attack_type
    } else {
        set
    };

    match d.heal_type {
        0 | 1 => a.heal_type = d.heal_type,
        2 | 3 => {
            a.heal_spell_num = 0;
            if d.heal_spell_num > 0 {
                a.heal_spell_num = d.heal_spell_num;
                if d.heal_type == 3 {
                    a.heal_spell_lvl = d.heal_spell_lvl;
                }
                a.heal_type = d.heal_type;
                a.use_meditate = d.use_meditate;
                a.heal_spell_lvl = a.heal_spell_lvl.clamp(0, 9999);
                a.heal_rounds = d.heal_rounds.clamp(1, 50);
            } else {
                a.heal_type = 0;
            }
        }
        _ => {
            a.heal_type = 4;
            a.heal_manual = d.heal_manual.clamp(0, 999_999);
        }
    }
    a
}

// ---------- the preview ----------

#[derive(Serialize, Debug, Default, PartialEq)]
pub struct AttackPreview {
    /// `lblAttackDMGPerRound.Caption`.
    pub caption: String,
    /// `lblAttackManaOOM.Caption`.
    pub oom_caption: String,
    /// `nLocalAttackDamage`: the weapon or martial art round, a manual physical amount, or 999999.
    pub damage: i32,
    /// `nLocalAttackSpellValue`: the spell round or the manual magic amount.
    pub spell_value: i32,
    /// `nLocalBackstabDamage`.
    pub backstab_damage: i32,
    pub oom: i16,
    /// Not shown by MME: the melee swing's hit chance and swings per round.
    pub hit_chance: Option<i16>,
    pub swings: Option<f64>,
    /// Not shown by MME: the spell's cast level, average cast, cast chance and mana cost.
    pub cast_level: Option<i16>,
    pub avg_cast: Option<i32>,
    pub cast_chance: Option<i16>,
    pub mana_cost: Option<i16>,
}

#[derive(Serialize, Debug, Default, PartialEq)]
pub struct HealPreview {
    /// `lblHealHEALSPerRound.Caption`.
    pub caption: String,
    /// `lblHealMANAPerRound.Caption`.
    pub mana_caption: String,
    /// `lblHealManaOOM.Caption`.
    pub oom_caption: String,
    pub value: i32,
    pub cost: f64,
    pub oom: i16,
    /// The rounds box after the dialog's own correction (a timed spell needs at least its duration).
    pub rounds: i16,
}

#[derive(Serialize, Debug)]
pub struct Preview {
    /// The VB6 error that ended `RefreshCombatStats` before it set the captions; the dialog keeps
    /// whatever it showed.
    pub error_text: Option<String>,
    pub attack: AttackPreview,
    pub heal: HealPreview,
    /// What Save stores.
    pub applied: AttackDto,
    /// `GetCurrentAttackName` for `applied`, short and long.
    pub attack_name: String,
    pub attack_name_long: String,
}

struct Scratch {
    attack: AttackPreview,
    heal: HealPreview,
}

fn caster(t: &touchstone_rules::character::CharacterProfile, overhead: f64) -> SpellCasterProfile {
    SpellCasterProfile {
        spell_overhead: overhead,
        ..t.spell_caster()
    }
}

/// Port of frmPopUpOptions `RefreshCombatStats`.
#[allow(clippy::too_many_lines)] // one MME procedure, in its order
fn combat_stats(
    world: &World,
    ch: &Character,
    st: &CharacterState,
    d: &AttackDto,
) -> Result<Scratch, Vb6Error> {
    let radio = attack_radio(d.attack_type).unwrap_or(0);
    let inputs = st.profile_inputs();
    let bless_share = inputs.bless_total / 6.0;
    let level = ch.level;

    // ---- the attack radio
    let mut local_type = 0;
    let mut spell_num = 0;
    let mut spell_cost = 0.0_f64;
    let mut spell_lvl = 0;
    let mut damage = 0_i32;
    let mut manual_magic = 0_i32;
    match radio {
        0 | 1 | 4 => local_type = radio,
        2 | 3 => {
            if d.spell_num > 0 {
                spell_num = d.spell_num;
                spell_cost = f64::from(get_spell_mana_cost(&world.spells, spell_num));
                if radio == 3 {
                    spell_lvl = i32::from(d.spell_lvl).clamp(0, 9999);
                }
                local_type = radio;
            }
        }
        _ => {
            local_type = 5;
            damage = d.manual_p;
            manual_magic = d.manual_m;
        }
    }

    // ---- the heal radio
    let mut heal_type = 0;
    let mut heal_spell = 0;
    let mut heal_lvl = 0;
    let mut heal_rounds: i16 = 1;
    let mut heal_manual = 0_i32;
    match d.heal_type {
        0 | 1 => heal_type = d.heal_type,
        2 | 3 => {
            if d.heal_spell_num > 0 {
                heal_spell = d.heal_spell_num;
                if d.heal_type == 3 {
                    heal_lvl = i32::from(d.heal_spell_lvl).clamp(0, 9999);
                }
                heal_type = d.heal_type;
                heal_rounds = d.heal_rounds.clamp(1, 999);
            }
        }
        _ => {
            heal_type = 4;
            heal_manual = d.heal_manual;
        }
    }

    let mud = match local_type {
        1 => match d.attack_type {
            6 => AttackTypeMud::Bash,
            7 => AttackTypeMud::Smash,
            _ => AttackTypeMud::Normal,
        },
        4 => match d.martial_art {
            1 => AttackTypeMud::Punch,
            2 => AttackTypeMud::Kick,
            3 => AttackTypeMud::Jumpkick,
            _ => AttackTypeMud::None,
        },
        _ => AttackTypeMud::None,
    };
    let t_char = st.profile_for(world, mud, 0);

    let names = AbilityEnv::for_world(world, Profile::Stock);
    let cast_env = SpellCastEnv {
        profile: Profile::Stock,
        spells: &world.spells,
        text: &names,
        avg_level_max_all_stats: compute_avg_level_max_stats(Profile::Stock, &world.races)
            .all_stats,
    };
    let max_mana = vb::clng(inputs.max_mana)?;

    // ---- heals
    let mut heal_value: i32;
    let mut heal_cost = 0.0_f64;
    let mut heal_oom: i16 = 0;
    match heal_type {
        0 => heal_value = 99_999,
        1 => heal_value = vb::clng(inputs.rest_rate / 18.0)?,
        2 | 3 => {
            let lvl = if heal_type == 3 { heal_lvl } else { level };
            let who = caster(&t_char, spell_cost + bless_share);
            let cast = cast_env.calculate_spell_cast(
                &who,
                &CastRequest {
                    spell: heal_spell,
                    cast_level: lvl,
                    ..CastRequest::default()
                },
            );
            let dur = get_spell_duration(
                &world.spells,
                heal_spell,
                i16::try_from(lvl).map_err(|_| Vb6Error::Overflow)?,
                false,
            );
            if dur > 1 {
                let avg = vb::mul_i32(cast.n_avg_cast, dur)?;
                heal_value =
                    vb::clng((f64::from(avg) / (f64::from(dur) * SPELL_ROUND_SECS)) * ROUND_SECS)?;
                if i32::from(d.heal_rounds) < dur {
                    heal_rounds = i16::try_from(dur).map_err(|_| Vb6Error::Overflow)?;
                } else if i32::from(heal_rounds) > dur {
                    let v = f64::from(heal_value);
                    heal_value =
                        vb::clng(v - (v * (1.0 - (f64::from(dur) / f64::from(heal_rounds)))))?;
                }
            } else {
                heal_value = vb::clng(vb::round(
                    f64::from(cast.n_avg_cast) / f64::from(heal_rounds),
                    2,
                )?)?;
            }
            heal_cost = vb::round(f64::from(cast.n_mana_cost) / f64::from(heal_rounds), 2)?;
            if heal_cost < 0.1 {
                heal_cost = 0.0;
            }
            if heal_cost > 9999.0 {
                heal_cost = 9999.0;
            }
            if heal_cost > 0.0 {
                heal_oom = calc_rounds_to_oom(
                    heal_cost + spell_cost + bless_share,
                    max_mana,
                    inputs.mana_rate,
                    cast.n_cast_chance,
                    i32::from(cast.n_duration),
                );
            }
        }
        _ => heal_value = heal_manual,
    }
    heal_value = heal_value.clamp(0, 99_999);

    // ---- attacks
    let accuracy_tag = st.stats.stat_tag[10].map_or_else(String::new, vb::cstr);
    let equipped = st.equipped_globals();
    let attack_env = AttackEnv {
        items: &world.items,
        classes: &world.classes,
        races: &world.races,
        equipped: &equipped,
        cast_text: &names,
        dat_ver_caption: &world.info.dat_file_version,
        avg_level_max_all_stats: cast_env.avg_level_max_all_stats,
        accuracy_tag: &accuracy_tag,
    };
    let weapon = inputs.weapon_number[0];
    let swing = |profile: &touchstone_rules::character::CharacterProfile, mud: AttackTypeMud, w| {
        let mut casts = String::new();
        calculate_attack(
            Profile::Stock,
            &attack_env,
            &CharacterState::attack_profile_of(profile),
            &AttackRequest {
                attack_type: mud as i32,
                weapon_number: w,
                ..AttackRequest::default()
            },
            &mut casts,
        )
    };

    let mut out = AttackPreview::default();
    let mut spell_value = 0_i32;
    let mut cast_level = 0_i16;
    match local_type {
        0 => damage = 999_999,
        1 | 4 => {
            if mud != AttackTypeMud::None {
                let m = swing(&t_char, mud, weapon);
                damage = m.round_total;
                out.hit_chance = Some(m.hit_chance);
                out.swings = Some(m.swings);
            }
        }
        2 | 3 => {
            if spell_num > 0 {
                let lvl = if local_type == 3 { spell_lvl } else { level };
                let who = caster(&t_char, heal_cost + bless_share);
                let cast = cast_env.calculate_spell_cast(
                    &who,
                    &CastRequest {
                        spell: spell_num,
                        cast_level: lvl,
                        ..CastRequest::default()
                    },
                );
                spell_value = cast.n_avg_round_dmg;
                out.oom = cast.n_oom;
                cast_level = cast.n_cast_level;
                out.cast_level = Some(cast.n_cast_level);
                out.avg_cast = Some(cast.n_avg_cast);
                out.cast_chance = Some(cast.n_cast_chance);
                out.mana_cost = Some(cast.n_mana_cost);
            }
        }
        _ => spell_value = manual_magic,
    }

    let mut backstab = 0_i32;
    if local_type > 0 && d.backstab {
        let mut w = match d.backstab_weapon {
            -1 => -1,
            n if n > 0 => n,
            _ => weapon,
        };
        let t_bs = st.profile_for(world, AttackTypeMud::Surprise, w);
        if w < 0 {
            w = 0;
        }
        backstab = swing(&t_bs, AttackTypeMud::Surprise, w).round_total;
    }
    damage = damage.clamp(0, 999_999);
    backstab = backstab.clamp(0, 999_999);

    // ---- captions
    let bless = inputs.bless_total > 0.0;
    let mut heal = HealPreview {
        caption: format!("{heal_value} heal/rnd"),
        mana_caption: format!("{} mana/rnd", vb::cstr(heal_cost)),
        oom_caption: String::new(),
        value: heal_value,
        cost: heal_cost,
        oom: heal_oom,
        rounds: heal_rounds,
    };
    if heal_oom > 0 {
        heal.oom_caption = format!("{heal_oom} rnds to oom");
        if bless {
            heal.oom_caption += "*";
        }
        if spell_cost > 0.0 {
            heal.oom_caption += "+";
        }
    } else if out.oom > 0 {
        heal.oom_caption = format!("{} rnds to oom", out.oom);
        if bless {
            heal.oom_caption += "*";
        }
        heal.oom_caption += "+";
    }

    out.caption = format!("{} dmg/rnd", vb::add_i32(damage, spell_value)?);
    if cast_level > 0 && i32::from(cast_level) != t_char.level {
        out.caption += &format!(" (lvl {cast_level})");
    }
    if backstab != 0 {
        out.caption = auto_append(&out.caption, &format!("{backstab} bs dmg"), "\r\n");
    }
    if out.oom > 0 {
        out.oom_caption = format!("{} rnds to oom", out.oom);
        if bless {
            out.oom_caption += "*";
        }
        if heal_cost > 0.0 {
            out.oom_caption += "+";
        }
    }
    out.damage = damage;
    out.spell_value = spell_value;
    out.backstab_damage = backstab;
    Ok(Scratch { attack: out, heal })
}

/// `GetCurrentAttackName`, short (the button) and long (a description), for a character's attack.
#[must_use]
pub fn attack_names(
    world: &World,
    ch: &Character,
    st: &CharacterState,
    recovery_only: bool,
) -> (String, String) {
    let inputs = st.profile_inputs();
    let names = AbilityEnv::for_world(world, Profile::Stock);
    let party = PartyFilter::default();
    let ctx = Context {
        profile: Profile::Stock,
        nmr_ver: nmr_ver(&world.info.nmr_version),
        avg_level_max_all_stats: compute_avg_level_max_stats(Profile::Stock, &world.races)
            .all_stats,
        hide_record_numbers: false,
        recovery_only,
        classes: &world.classes,
        items: &world.items,
        spells: &world.spells,
        text: &names,
        stats: &inputs,
        party: &party,
    };
    let mut short = ch.clone();
    let mut long = ch.clone();
    (
        get_current_attack_name(&ctx, &mut short, false),
        get_current_attack_name(&ctx, &mut long, true),
    )
}

/// The dialog's numbers for a draft choice. `character.attack` is the saved choice.
pub fn preview(
    world: &World,
    dto: &CharacterDto,
    draft: &AttackDto,
    recovery_only: bool,
) -> Result<Preview, ApiError> {
    check_draft(draft)?;
    let (ch, st) = evaluate_character(world, dto)?;
    let applied = apply_choice(&dto.attack, draft);
    let (attack_name, attack_name_long) = {
        let mut next = dto.clone();
        next.attack = applied.clone();
        let next = next.to_character().map_err(bad)?;
        // the profile for the name is the saved one's; only the choice differs
        attack_names(world, &next, &st, recovery_only)
    };
    let (attack, heal, error_text) = match combat_stats(world, &ch, &st, draft) {
        Ok(s) => (s.attack, s.heal, None),
        // MME's handler leaves the captions as they were
        Err(e) => (
            AttackPreview::default(),
            HealPreview::default(),
            Some(vb_error_text(e.code())),
        ),
    };
    Ok(Preview {
        error_text,
        attack,
        heal,
        applied,
        attack_name,
        attack_name_long,
    })
}

// ---------- routes ----------

#[derive(Deserialize)]
pub struct OptionsReq {
    character: CharacterDto,
    #[serde(default)]
    recovery_only: bool,
}

pub async fn options_route(
    State(state): State<Arc<AppState>>,
    Body(req): Body<OptionsReq>,
) -> ApiResult {
    let loaded: Arc<_> = state.current();
    json(&options(&loaded.world, &req.character, req.recovery_only)?)
}

#[derive(Deserialize)]
pub struct PreviewReq {
    /// The character with its saved attack.
    character: CharacterDto,
    /// The dialog's controls; the saved attack when absent.
    draft: Option<AttackDto>,
    #[serde(default)]
    recovery_only: bool,
}

pub async fn preview_route(
    State(state): State<Arc<AppState>>,
    Body(req): Body<PreviewReq>,
) -> ApiResult {
    let loaded = state.current();
    let draft = req.draft.unwrap_or_else(|| req.character.attack.clone());
    json(&preview(
        &loaded.world,
        &req.character,
        &draft,
        req.recovery_only,
    )?)
}
