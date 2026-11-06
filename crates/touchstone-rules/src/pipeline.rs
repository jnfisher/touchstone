//! The character pipeline: frmMain `RefreshAll` over the per-procedure ports. Card P3-44.
//!
//! MME keeps the loaded character in `frmMain` controls and module globals, and the Char tab is
//! whatever its `Refresh*` procedures last wrote there. Earlier cards ported those procedures one by
//! one, each with its own input struct. [`refresh_all`] chains them in MME's order and threads the
//! values one step leaves in a control to the next.
//!
//! # Which struct is which MME state
//!
//! - [`crate::gear::Character`]: the *global filter* controls (`txtGlobalLevel`, `cmbGlobalClass`,
//!   `cmbGlobalRace`, `cmbGlobalAlignment`, `txtGlobalMinLVL`) that gear aggregation reads.
//! - [`crate::character::Character`]: the user's character, as an `.mmec` file stores it. This is
//!   the input of [`refresh_all`].
//! - [`crate::gear::CharStats`]: `lblInvenCharStat(0..46)` and the `nGlobalChar*` globals that
//!   `CalcCharacterStats` fills.
//! - [`crate::character::CharStats`]: the same numbers plus the panel tags (`lblCharMaxHP.Tag`, ...)
//!   that `PopulateCharacterProfile` reads; [`CharacterState::profile_inputs`] builds it.
//! - [`crate::character::CharacterProfile`]: `tCharacterProfile`, the whole profile
//!   `PopulateCharacterProfile` builds.
//! - [`crate::attack::CharacterProfile`]: the slice of the profile `CalculateAttack` reads
//!   ([`CharacterState::attack_profile`]); [`crate::spell_cast::SpellCasterProfile`] is the slice
//!   `CalculateSpellCast` reads ([`CharacterState::spell_caster`]).
//!
//! # Order and what is left out
//!
//! SetupClass, RefreshCPs, CalcCharacterStats (whose tail runs RefreshMagic, RefreshMainStatBonuses,
//! RefreshDodge and the encumbrance level), RefreshHitPoints, RefreshMagicRes, RefreshPicklocks,
//! RefreshCombatHealingValues. Not ported: RefreshShops and the list colours (display), the damage
//! output label (`lblInvenCharStat(46)`), `SetCharDefenseDescription` (a cache key) and tool tips.
//! Control event cascades re-run a procedure on final values, so each step runs once. A class or
//! race of `None` (combo `ListIndex < 0`) is [`RulesError::Unsupported`].

use touchstone_model::World;
use touchstone_vb6 as vb;

use crate::attack::{self, CastText, EquippedGlobals, HandGlobals};
use crate::char_panel::{
    ClassSetup, ClassSetupInput, CpInput, CpPanel, Dodge, EncumLevel, HitPoints, HpInput, Magic,
    MagicInput, Picklocks, StatLabels, bless_mana_per_round, encum_level, main_stat_labels,
    refresh_cps, refresh_dodge, refresh_hit_points, refresh_magic, refresh_magic_res,
    refresh_picklocks, setup_class,
};
use crate::character::{
    AttackConfig, AttackType, AttackTypeMud, CharStats as ProfileStats, Character,
    CharacterProfile, Context, PartyFilter, populate_character_profile,
    refresh_combat_healing_values,
};
use crate::combat::DamageEnv;
use crate::gear::{
    self, CharStats, GearInput, ItemManagerRow, STAT_COUNT, SecondHalfInput, calc_character_stats,
    carried_items,
};
use crate::monster_glue::CharSimInputs;
use crate::spell_cast::SpellCasterProfile;
use crate::spells::SpellText;
use crate::{Profile, RulesError};

/// `cmbSpellMageryLevel.ListCount` and `cmbSpellMagery.ListCount` as `SetUpFormObjects` fills them.
const MAGERY_LEVEL_COUNT: i32 = 4;
const MAGERY_COUNT: i32 = 6;

/// The view toggles and globals the pipeline reads besides the character, with MME's defaults.
#[derive(Debug, Clone)]
pub struct PipelineOptions {
    /// `chkInvenHideCharStats.Value = 1`.
    pub hide_class_race: bool,
    /// `lvItemManager` rows (CARRIED ones add their abilities).
    pub item_manager: Vec<ItemManagerRow>,
    /// `Val(txtGlobalMinLVL.Text)`.
    pub min_item_level: i32,
    /// `frmMain.lblDatVer.Caption`.
    pub dat_ver_caption: String,
    /// `nNMRVer` and `gAvgLevelMaxAllStats`, which only the profile of "no character" reads.
    pub nmr_ver: f64,
    pub avg_level_max_all_stats: f64,
    /// The lair-mode controls (party size, HP, ...).
    pub party: PartyFilter,
    /// `RefreshAll`'s optional `bSetupClass`.
    pub setup_class: bool,
    /// `bHideRecordNumbers`, for the names the combat contexts print.
    pub hide_record_numbers: bool,
}

impl Default for PipelineOptions {
    fn default() -> Self {
        Self {
            hide_class_race: false,
            item_manager: Vec::new(),
            min_item_level: 0,
            dat_ver_caption: "v1.11p".to_string(),
            nmr_ver: 1.83,
            avg_level_max_all_stats: 0.0,
            party: PartyFilter::default(),
            setup_class: true,
            hide_record_numbers: false,
        }
    }
}

/// Everything `RefreshAll` leaves behind.
#[derive(Debug, Clone)]
pub struct CharacterState {
    /// `SetupClass`; `None` when `bSetupClass` was false.
    pub class_setup: Option<ClassSetup>,
    /// `cmbCharQuestOpts(0..3).ListIndex` as `CalcCharacterStats` saw them.
    pub quest_options: [i32; 4],
    pub cps: CpPanel,
    /// Both halves of `CalcCharacterStats`.
    pub stats: CharStats,
    /// `RefreshMagic` (`None` where `CalcCharacterStats` stopped before reaching it).
    pub magic: Option<Magic>,
    pub stat_labels: StatLabels,
    pub dodge: Dodge,
    /// The encumbrance level box, or the VB6 error that left it as it was.
    pub encum_level: Result<EncumLevel, i32>,
    pub hit_points: HitPoints,
    /// `txtCharMR.Text` (the error number where MME's handler left it unchanged).
    pub magic_res: Result<i32, i32>,
    pub picklocks: Picklocks,
    /// `lblCharBless.Caption`: bless mana per six rounds; `None` where MME leaves the label alone.
    pub bless_mana: Option<f64>,
    /// The character with the heal values `RefreshCombatHealingValues` wrote.
    pub character: Character,
    /// `lblInvenStats(19).Tag`: 2 per class stealth ability, 1 per race stealth ability.
    pub class_stealth_tag: f64,
    /// `PopulateCharacterProfile(force_use_char, force_no_party, Mud None, weapon 0)`.
    pub profile: CharacterProfile,
    options: PipelineOptions,
}

fn val_cstr(x: f64) -> f64 {
    vb::val(&vb::cstr(x))
}

/// Port of frmMain `RefreshAll` followed by modMain `PopulateCharacterProfile`: every Char-tab
/// panel, the aggregate stats and the character profile for the loaded character.
///
/// # Errors
/// [`RulesError::Unsupported`] when the character has no class or race selected.
pub fn refresh_all(
    profile: Profile,
    world: &World,
    ch: &Character,
    opts: &PipelineOptions,
) -> Result<CharacterState, RulesError> {
    let (Profile::Stock | Profile::Quicksilver(_)) = profile;
    let class = ch
        .class
        .ok_or(RulesError::Unsupported("no class selected"))?;
    let race = ch.race.ok_or(RulesError::Unsupported("no race selected"))?;
    let level = ch.level;

    // SetupClass writes the 2nd-quest option that CalcCharacterStats reads
    let mut quest_options = ch.quest_options.map(i32::from);
    let class_setup = if opts.setup_class {
        let s = setup_class(&ClassSetupInput {
            world,
            class: Some(class),
            none_text: false,
            quest_checks: [ch.quests[5], ch.quests[9], ch.quests[10], ch.quests[11]],
            quest_options,
            magery_level_count: MAGERY_LEVEL_COUNT,
            magery_count: MAGERY_COUNT,
        });
        if let Some(s) = &s {
            quest_options = s.quest_options;
        }
        s
    } else {
        None
    };

    // RefreshCPs: the stat floors are what cmbGlobalRace_Click wrote into txtCharMaxStats(i).Tag
    let base_stats = ch.stats.map(f64::from);
    let floors = world.get_race(race).map_or([0.0; 6], |r| {
        [r.m_str, r.m_int, r.m_wil, r.m_agl, r.m_hea, r.m_chm].map(f64::from)
    });
    let cps = refresh_cps(&CpInput {
        profile,
        world,
        stat_texts: base_stats,
        max_tags: floors,
        race: Some(race),
        class,
        level: f64::from(level),
    });

    // CalcCharacterStats
    let mut adjustments = [0; STAT_COUNT];
    for (slot, v) in adjustments.iter_mut().zip(&ch.stat_adjustments) {
        *slot = *v;
    }
    let gear_input = GearInput {
        profile,
        world,
        dat_ver_caption: &opts.dat_ver_caption,
        character: gear::Character {
            level,
            class,
            race,
            alignment: i32::from(ch.alignment),
            min_item_level: opts.min_item_level,
        },
        base_stats,
        hide_class_race: opts.hide_class_race,
        add_weight: ch.use_add_weight.then(|| f64::from(ch.add_weight)),
        equipped: ch.equipped,
        carried: carried_items(world, &opts.item_manager)?,
        adjustments,
        bless_slots: ch.bless,
        martial_arts: ch.attack.attack_type == AttackType::MartialArts,
    };
    let second = SecondHalfInput {
        quests: ch.quests,
        quest_options,
        global_filter: i32::from(ch.global_filter),
        attack_type: ch.attack.attack_type as i32,
    };
    let stats = calc_character_stats(&gear_input, &second)?;
    let t = stats.stat_with_bonus;

    // the tail of CalcCharacterStats: RefreshMagic (only if no error was swallowed), then the out: section
    let magic = stats.error.is_none().then(|| {
        refresh_magic(&MagicInput {
            profile,
            world,
            level: f64::from(level),
            class,
            intellect: t[1],
            willpower: t[2],
            charm: t[5],
            mana_regen_pct: stats.mana_regen,
            mana_bonus: stats.stat[6],
            spellcasting_bonus: stats.stat[9],
        })
    });
    let stat_labels = main_stat_labels(&stats.label_bonus);
    let dodge = refresh_dodge(stats.stat_tag[8]);
    let encum = encum_level(stats.stat[0], stats.stat[1]);

    let hit_points = refresh_hit_points(
        profile,
        &HpInput {
            world,
            level: f64::from(level),
            class,
            health: t[4],
            hp_regen: stats.hp_regen,
            hp_bonus: stats.stat[5],
        },
    );
    let magic_res = refresh_magic_res(profile, t[1], t[2], stats.plus_mr);
    let picklocks = refresh_picklocks(profile, f64::from(level), t[3], t[1], t[5], stats.stat[22]);
    let bless_mana = bless_mana_per_round(profile, world, f64::from(level), &ch.bless);

    // RefreshCombatHealingValues reads the profile, which reads everything above
    let count = |slots: &[touchstone_model::AbilitySlot], ability: i32| {
        slots.iter().filter(|a| a.ability == ability).count() as f64
    };
    let mut class_stealth_tag = 0.0;
    if !opts.hide_class_race {
        if let Some(c) = world.get_class(class).filter(|_| class > 0) {
            class_stealth_tag += 2.0 * count(&c.abilities, 103);
        }
        if let Some(r) = world.get_race(race).filter(|_| race > 0) {
            class_stealth_tag += count(&r.abilities, 102);
        }
    }
    let mut character = ch.clone();
    character.attack.accy_adj = i16::try_from(stats.attack_accy_adj).unwrap_or(0);
    let mut state = CharacterState {
        class_setup,
        quest_options,
        cps,
        stats,
        magic,
        stat_labels,
        dodge,
        encum_level: encum,
        hit_points,
        magic_res,
        picklocks,
        bless_mana,
        character,
        class_stealth_tag,
        profile: CharacterProfile::default(),
        options: opts.clone(),
    };
    state.refresh_heal(world);
    state.profile = state.profile_for(world, AttackTypeMud::None, 0);
    Ok(state)
}

struct NoSpellText;

impl SpellText for NoSpellText {
    fn ability_stats(&self, _: i16, _: i16, _: bool, _: bool) -> String {
        String::new()
    }
    fn spell_name(&self, _: i32) -> String {
        String::new()
    }
    fn monster_name(&self, _: i32) -> String {
        String::new()
    }
}

impl CharacterState {
    /// The `CharStats` (`character` module) that `PopulateCharacterProfile` reads: the stat vector
    /// as `lblInvenCharStat(i).Tag`, and the panel tags.
    #[must_use]
    pub fn profile_inputs(&self) -> ProfileStats {
        let s = &self.stats;
        let tag = |i: usize| s.stat_tag[i].unwrap_or(0.0);
        let mut stat = [0.0; 47];
        for (i, v) in stat.iter_mut().enumerate() {
            *v = tag(i);
        }
        let w = |k: usize| &s.weapons[k];
        let pair = |f: fn(&gear::WeaponGear) -> i32| [f(w(0)), f(w(1))];
        ProfileStats {
            stat,
            encum_current: s.stat[0],
            encum_max: s.stat[1],
            base_stat: s.stat_with_bonus,
            class_stealth_tag: self.class_stealth_tag,
            max_hp: self
                .hit_points
                .range
                .as_ref()
                .map_or(0.0, |r| val_cstr(r.average)),
            rest_rate: self.hit_points.rest_resting.map_or(0.0, f64::from),
            meditate_regen: self
                .magic
                .as_ref()
                .and_then(|m| m.medi_ticks)
                .map_or(0.0, val_cstr),
            max_mana: self
                .magic
                .as_ref()
                .and_then(|m| m.max_mana)
                .map_or(0.0, f64::from),
            mana_rate: self
                .magic
                .as_ref()
                .and_then(|m| m.mana_regen)
                .map_or(0.0, f64::from),
            spellcasting: self
                .magic
                .as_ref()
                .and_then(|m| m.spellcasting)
                .map_or(0.0, f64::from),
            bless_total: self.bless_mana.map_or(0.0, val_cstr),
            weapon_number: pair(|g| g.number),
            weapon_accy: pair(|g| g.accy),
            weapon_bs_accy: pair(|g| g.bs_accy),
            punch_skill: pair(|g| g.punch_skill),
            punch_accy: pair(|g| g.punch_accy),
            punch_dmg: pair(|g| g.punch_dmg),
            kick_skill: pair(|g| g.kick_skill),
            kick_accy: pair(|g| g.kick_accy),
            kick_dmg: pair(|g| g.kick_dmg),
            jk_skill: pair(|g| g.jumpkick_skill),
            jk_accy: pair(|g| g.jumpkick_accy),
            jk_dmg: pair(|g| g.jumpkick_dmg),
            accy_items: s.accy_items,
            accy_other: s.accy_other,
            accy_abils: s.accy_abils,
            hit_magic_non_weapon: s.hit_magic_non_weapon,
            qnd_bonus: s.qnd_bonus,
            energy_use: s.energy_use,
        }
    }

    fn refresh_heal(&mut self, world: &World) {
        let inputs = self.profile_inputs();
        let ctx = Context {
            profile: Profile::Stock,
            nmr_ver: self.options.nmr_ver,
            avg_level_max_all_stats: self.options.avg_level_max_all_stats,
            hide_record_numbers: self.options.hide_record_numbers,
            recovery_only: false,
            classes: &world.classes,
            items: &world.items,
            spells: &world.spells,
            text: &NoSpellText,
            stats: &inputs,
            party: &self.options.party,
        };
        refresh_combat_healing_values(&ctx, &mut self.character);
    }

    /// `PopulateCharacterProfile` for the loaded character (`force_use_char`, `force_no_party`)
    /// and an attack of kind `mud`; `weapon` is `nWeaponNumber` (positive: that weapon, negative:
    /// fists, 0: the current one). The state's own profile is `(AttackTypeMud::None, 0)`.
    #[must_use]
    pub fn profile_for(&self, world: &World, mud: AttackTypeMud, weapon: i32) -> CharacterProfile {
        let inputs = self.profile_inputs();
        let ctx = Context {
            profile: Profile::Stock,
            nmr_ver: self.options.nmr_ver,
            avg_level_max_all_stats: self.options.avg_level_max_all_stats,
            hide_record_numbers: self.options.hide_record_numbers,
            recovery_only: false,
            classes: &world.classes,
            items: &world.items,
            spells: &world.spells,
            text: &NoSpellText,
            stats: &inputs,
            party: &self.options.party,
        };
        let mut t = CharacterProfile::default();
        populate_character_profile(
            &ctx,
            &self.character,
            &mut t,
            true,
            true,
            mud,
            weapon,
            false,
        );
        t
    }

    /// The slice of the profile `CalculateAttack` reads.
    #[must_use]
    pub fn attack_profile(&self) -> attack::CharacterProfile {
        Self::attack_profile_of(&self.profile)
    }

    /// The same slice for any profile, such as one [`CharacterState::profile_for`] built for
    /// another kind of attack, or the profile of "no character".
    #[must_use]
    pub fn attack_profile_of(p: &CharacterProfile) -> attack::CharacterProfile {
        attack::CharacterProfile {
            is_loaded_character: p.is_loaded_character,
            level: p.level,
            class: p.class,
            race: p.race,
            combat: p.combat,
            strength: p.str_,
            agility: p.agi,
            intellect: p.int,
            charm: p.cha,
            crit: p.crit,
            plus_max_damage: p.plus_max_damage,
            plus_min_damage: p.plus_min_damage,
            plus_bs_accuracy: p.plus_bs_accy,
            plus_bs_min_damage: p.plus_bs_min_dmg,
            plus_bs_max_damage: p.plus_bs_max_dmg,
            ma_plus_skill: p.ma_plus_skill,
            ma_plus_accuracy: p.ma_plus_accy,
            ma_plus_damage: p.ma_plus_dmg,
            stealth: p.stealth,
            class_stealth: p.class_stealth,
            race_stealth: p.race_stealth,
            accuracy: p.accuracy,
            encum_pct: p.encum_pct,
            encum_current: p.encum_current,
            encum_max: p.encum_max,
            spell_dmg_bonus: p.spell_dmg_bonus,
            energy_pct: p.energy_pct,
        }
    }

    /// The weapon-related `nGlobalChar*` globals `CalculateAttack` reads.
    #[must_use]
    pub fn equipped_globals(&self) -> EquippedGlobals {
        let s = &self.stats;
        let hand = |w: &gear::WeaponGear| HandGlobals {
            number: w.number,
            encum: w.encum,
            accy: w.accy,
            crit: w.crit,
            strength: w.strength,
            agility: w.agility,
            max_dmg: w.max_dmg,
            bs_accy: w.bs_accy,
            bs_min_dmg: w.bs_min_dmg,
            bs_max_dmg: w.bs_max_dmg,
            punch_skill: w.punch_skill,
            punch_accy: w.punch_accy,
            punch_dmg: w.punch_dmg,
            kick_skill: w.kick_skill,
            kick_accy: w.kick_accy,
            kick_dmg: w.kick_dmg,
            jk_skill: w.jumpkick_skill,
            jk_accy: w.jumpkick_accy,
            jk_dmg: w.jumpkick_dmg,
            stealth: w.stealth,
        };
        EquippedGlobals {
            qnd_bonus: s.qnd_bonus,
            accy_abils: s.accy_abils,
            accy_other: s.accy_other,
            accy_items: s.accy_items,
            hands: [hand(&s.weapons[0]), hand(&s.weapons[1])],
        }
    }

    /// The five profile fields `CalculateSpellCast` reads.
    #[must_use]
    pub fn spell_caster(&self) -> SpellCasterProfile {
        self.profile.spell_caster()
    }

    /// The controls `SetupMonsterAttackSimWithCharStats` reads for the loaded character (P3-14b):
    /// AC and DR captions, MR text, the dodge and resist tags and the anti-magic box. A character
    /// whose `SetupClass` did not run keeps the box as the character file stored it.
    #[must_use]
    pub fn sim_inputs(&self) -> CharSimInputs {
        let tag = |i: usize| self.stats.stat_tag[i];
        CharSimInputs {
            class: self.character.class.filter(|&c| c > 0).unwrap_or(0),
            ac: self.stats.stat[2],
            dr: self.stats.stat[3],
            // an MR the procedure could not compute leaves the box empty, which reads as 0
            mr: f64::from(self.magic_res.unwrap_or(0)),
            dodge: self.dodge.value,
            anti_magic: self
                .class_setup
                .as_ref()
                .map_or(self.character.anti_magic, |s| s.anti_magic),
            prot_evil: tag(20).unwrap_or(0.0),
            resist_col: tag(28),
            resist_fir: tag(27),
            resist_sto: tag(25),
            resist_lit: tag(29),
            resist_wat: tag(26),
        }
    }

    /// The attack and heal choice with the values `RefreshCombatHealingValues` wrote.
    #[must_use]
    pub fn attack_config(&self) -> &AttackConfig {
        &self.character.attack
    }
}

/// The pieces combat's [`Context`] and [`DamageEnv`] borrow, owned in one place so a caller does not
/// assemble them by hand (the follow-up P3-16 asked for). Built by [`CharacterState::env`].
#[derive(Debug, Clone)]
pub struct CharacterEnv {
    stats: ProfileStats,
    equipped: EquippedGlobals,
    accuracy_tag: String,
    nmr_ver: f64,
    avg_level_max_all_stats: f64,
    /// `frmMain.lblDatVer.Caption`.
    pub dat_ver_caption: String,
    /// The lair-mode controls the state was built with.
    pub party: PartyFilter,
    /// `txtMonsterDamageOUT(0..1).Text`.
    pub damage_out_text: [String; 2],
    /// `txtMonsterLairFilter(9).Text`.
    pub party_swings_text: String,
    /// `bHideRecordNumbers`, as the state was built with it.
    hide_record_numbers: bool,
}

impl CharacterEnv {
    /// The [`Context`] over this character's stats.
    #[must_use]
    pub fn context<'a>(&'a self, world: &'a World) -> Context<'a> {
        Context {
            profile: Profile::Stock,
            nmr_ver: self.nmr_ver,
            avg_level_max_all_stats: self.avg_level_max_all_stats,
            hide_record_numbers: self.hide_record_numbers,
            recovery_only: false,
            classes: &world.classes,
            items: &world.items,
            spells: &world.spells,
            text: &NoSpellText,
            stats: &self.stats,
            party: &self.party,
        }
    }

    /// What `GetDamageOutput` reads: the context, the character, tables and the party's damage boxes.
    #[must_use]
    pub fn damage_env<'a>(
        &'a self,
        world: &'a World,
        ctx: &'a Context<'a>,
        character: &'a Character,
        cast_text: &'a dyn CastText,
    ) -> DamageEnv<'a> {
        DamageEnv {
            ctx,
            character,
            races: &world.races,
            monsters: &world.monsters,
            equipped: &self.equipped,
            cast_text,
            dat_ver_caption: &self.dat_ver_caption,
            accuracy_tag: &self.accuracy_tag,
            damage_out_text: [&self.damage_out_text[0], &self.damage_out_text[1]],
            party_swings_text: &self.party_swings_text,
        }
    }
}

impl CharacterState {
    /// The owned environment for combat calls, with the party controls the state was built with.
    #[must_use]
    pub fn env(&self, damage_out_text: [String; 2], party_swings_text: String) -> CharacterEnv {
        CharacterEnv {
            stats: self.profile_inputs(),
            equipped: self.equipped_globals(),
            accuracy_tag: self.stats.stat_tag[10].map_or_else(String::new, vb::cstr),
            nmr_ver: self.options.nmr_ver,
            avg_level_max_all_stats: self.options.avg_level_max_all_stats,
            dat_ver_caption: self.options.dat_ver_caption.clone(),
            party: self.options.party.clone(),
            damage_out_text,
            party_swings_text,
            hide_record_numbers: self.options.hide_record_numbers,
        }
    }
}
