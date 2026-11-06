//! Ability text (FAM-03, text half). Card P3-07.
//!
//! `GetAbilityName` is the ability-number-to-name table; `GetAbilityStats` prints an ability with
//! its value (names of spells, monsters, items and classes, spell equations, textblock casts);
//! `GetAbilityList` lists every name; `GetAbilDiffText` is the comparison-view variant.
//!
//! `GetAbilityStats` and `PullSpellEQ` call each other. [`AbilityEnv`] holds the globals both read
//! (`bHideRecordNumbers`, `bQuickSpell`, `nSpellNest`, the global level box, `gAvgLevelMaxAllStats`)
//! and implements `spells::SpellText`, so wiring is: build an `AbilityEnv` and call
//! [`AbilityEnv::pull_spell_eq`] instead of constructing a `SpellEnv` with a stub text provider.
//! `nSpellNest` is carried across the recursion by [`AbilityEnv::nest`]; recursion that
//! `SpellEnv` itself does between two text callbacks (an EndCast spell pulled by `PullSpellEQ`) is
//! invisible to it, which only matters for the 20-deep "to infinity and beyond" guard.
//!
//! The ListView rows `GetAbilityStats` adds when given a list (spell, cast and textblock rows)
//! are UI and are not ported: the port behaves as MME does with `lv = Nothing`. GreaterMUD and
//! Paramud names (abilities above 187, the GreaterMUD spellings of 15, 16 and 50) are not ported.
//!
//! Database names come through [`NameLookup`]. A number with no row (or an empty table) is `None`,
//! and MME's `GetSpellName`/`GetMonsterName`/`GetItemName`/`GetClassName` then answer with the
//! number itself. A row whose name is empty (the converted database stores an empty `Name` as
//! NULL, which MME's Access column holds as a zero-length string) is `Some("")`.

use std::cell::Cell;

use touchstone_model::{Spell, World};
use touchstone_vb6 as vb;
use touchstone_vb6::Vb6Error;

use crate::Profile;
use crate::attack::CastText;
use crate::spells::{PullArgs, SpellEnv, SpellText};
use crate::textblock::{TextblockData, get_textblock_action};

type Res<T> = Result<T, Vb6Error>;

fn to_i16(v: i32) -> Res<i16> {
    i16::try_from(v).map_err(|_| Vb6Error::Overflow)
}

/// The `Name` column of the rows the name lookups seek (`None`: no such row).
pub trait NameLookup {
    fn spell_name(&self, number: i32) -> Option<String>;
    fn monster_name(&self, number: i32) -> Option<String>;
    fn item_name(&self, number: i32) -> Option<String>;
    fn class_name(&self, number: i32) -> Option<String>;
}

/// The database itself: a row's `Name` column, or `None` when there is no such row. A NULL name is
/// `Some("")`, which is what MME's Access column holds.
impl NameLookup for World {
    fn spell_name(&self, number: i32) -> Option<String> {
        self.get_spell(number)
            .map(|r| r.name.clone().unwrap_or_default())
    }
    fn monster_name(&self, number: i32) -> Option<String> {
        self.get_monster(number)
            .map(|r| r.name.clone().unwrap_or_default())
    }
    fn item_name(&self, number: i32) -> Option<String> {
        self.get_item(number).map(|r| r.name.clone())
    }
    fn class_name(&self, number: i32) -> Option<String> {
        self.get_class(number).map(|r| r.name.clone())
    }
}

fn numbered(name: Option<String>, n: i32, no_number: bool) -> String {
    match name {
        None => n.to_string(),
        Some(s) if no_number => s,
        Some(s) => format!("{s}({n})"),
    }
}

/// Port of modMMudDatabase GetSpellName.
#[must_use]
pub fn get_spell_name(names: &dyn NameLookup, n: i32, no_number: bool) -> String {
    if n == 0 {
        return "None".into();
    }
    numbered(names.spell_name(n), n, no_number)
}

/// Port of modMMudDatabase GetMonsterName.
#[must_use]
pub fn get_monster_name(names: &dyn NameLookup, n: i32, no_number: bool) -> String {
    if n == 0 {
        return "None".into();
    }
    numbered(names.monster_name(n), n, no_number)
}

/// Port of modMMudDatabase GetItemName.
#[must_use]
pub fn get_item_name(names: &dyn NameLookup, n: i32, no_number: bool) -> String {
    if n == 0 {
        return "None".into();
    }
    numbered(names.item_name(n), n, no_number)
}

/// Port of modMMudDatabase GetClassName (never numbered).
#[must_use]
pub fn get_class_name(names: &dyn NameLookup, n: i32) -> String {
    if n == 0 {
        return "None".into();
    }
    names.class_name(n).unwrap_or_else(|| n.to_string())
}

/// Stock names for the abilities that always have one.
fn plain_name(num: i16) -> Option<&'static str> {
    Some(match num {
        0 => "None",
        1 => "Damage",
        2 => "AC",
        3 => "Resist-Cold",
        4 => "MaxDamage",
        5 => "Resist-Fire",
        6 => "Enslave",
        7 => "DR",
        8 => "DrainLife",
        9 => "Shadow",
        10 => "AC Blur",
        11 => "AlterEnergyLevel",
        12 => "Summon",
        13 => "Illu",
        14 => "RoomIllu",
        15 => "Alterhunger",
        16 => "Alterthirst",
        17 => "Damage(-MR)",
        18 => "Heal",
        19 => "Poison",
        20 => "CurePoison",
        21 => "ImmuPoison",
        22 => "Accuracy",
        23 => "AffectsUndeadOnly",
        24 => "ProtEvil",
        25 => "ProtGood",
        26 => "DetectMagic",
        27 => "Stealth",
        28 => "Magical",
        29 => "Punch",
        30 => "Kick",
        31 => "Bash",
        32 => "Smash",
        33 => "Killblow",
        34 => "Dodge",
        35 => "JumpKick",
        36 => "M.R.",
        37 => "Picklocks",
        38 => "Tracking",
        39 => "Thievery",
        40 => "FindTraps",
        41 => "DisarmTraps",
        42 => "LearnSp",
        43 => "CastsSp",
        44 => "Intel",
        45 => "Wisdom",
        46 => "Strength",
        47 => "Health",
        48 => "Agility",
        49 => "Charm",
        50 => "MageBaneQuest",
        51 => "AntiMagic",
        52 => "EvilInCombat",
        53 => "BlindingLight",
        54 => "IlluTarget",
        55 => "AlterLightDuration",
        56 => "RechargeItem",
        57 => "SeeHidden",
        58 => "Crits",
        59 => "ClassOk",
        60 => "Fear",
        61 => "AffectExit",
        62 => "AlterEvilChance",
        63 => "AlterExperience",
        64 => "AddCP",
        65 => "Resist-Stone",
        66 => "Resist-Lightning",
        67 => "Quickness",
        68 => "Slowness",
        69 => "MaxMana",
        70 => "Spellcasting",
        71 => "Confusion",
        72 => "ShockShield",
        73 => "DispellMagic",
        74 => "HoldPerson",
        75 => "Paralyze",
        76 => "Mute",
        77 => "Perception",
        78 => "Animal",
        79 => "MageBind",
        80 => "AffectsAnimalsOnly",
        81 => "Freedom",
        82 => "Cursed",
        83 => "CursedMajor",
        84 => "RemoveCurse",
        85 => "Shatter",
        86 => "Quality",
        87 => "Speed",
        88 => "MaxHP",
        89 => "PunchAcc",
        90 => "KickAcc",
        91 => "JumpKAcc",
        92 => "PunchDmg",
        93 => "KickDmg",
        94 => "JumpKDmg",
        95 => "Slay",
        96 => "Encum%",
        97 => "GoodOnly",
        98 => "EvilOnly",
        99 => "AlterDRpercent",
        100 => "LoyalItem",
        102 => "RaceStealth",
        103 => "ClassStealth",
        104 => "DefenseModifier",
        105 => "Accuracy2",
        106 => "Accuracy3",
        107 => "BlindUser",
        108 => "AffectsLivingOnly",
        109 => "NonLiving",
        110 => "NotGood",
        111 => "NotEvil",
        112 => "NeutralOnly",
        113 => "NotNeutral",
        114 => "%Spell",
        116 => "BSAccu",
        117 => "BsMinDmg",
        118 => "BsMaxDmg",
        119 => "Del@Maint",
        121 => "Recharge",
        122 => "RemovesSpell",
        123 => "HPRegen",
        124 => "NegateAbility",
        125 => "IceSorcQuest",
        126 => "GoodQuest",
        127 => "NeutralQuest",
        128 => "EvilQuest",
        129 => "DarkDruidQuest",
        130 => "BloodChampQuest",
        131 => "SheDragonQuest",
        132 => "WereratQuest",
        133 => "PhoenixQuest",
        134 => "DaoLordQuest",
        135 => "MinLevel",
        136 => "MaxLevel",
        138 => "RoomVisible",
        139 => "SpellImmu",
        140 => "TeleportRoom",
        141 => "TeleportMap",
        142 => "HitMagic",
        143 => "ClearItem",
        145 => "ManaRgn",
        146 => "MonsGuards",
        147 => "Resist-Water",
        148 => "TextBlock",
        149 => "Remove@Maint",
        150 => "HealMana",
        151 => "EndCast",
        152 => "Rune",
        153 => "KillSpell",
        154 => "Visible@Maint",
        156 => "QuestItem",
        157 => "ScatterItems",
        158 => "ReqToHit",
        159 => "KaiBind",
        160 => "GiveTempSpell",
        161 => "OpenDoor",
        162 => "Lore",
        163 => "SpellComponent",
        164 => "EndCast%",
        165 => "AlterSpDmg",
        166 => "AlterSpLength",
        167 => "UnEquipItem",
        168 => "EquipItem",
        169 => "CannotWearLocation",
        170 => "Sleep",
        171 => "Invisibility",
        172 => "SeeInvisible",
        173 => "Scry",
        174 => "StealMana",
        175 => "StealHPtoMP",
        176 => "StealMPtoHP",
        177 => "SpellColours",
        178 => "Shadowform",
        179 => "FindTrapsValue",
        180 => "PickLocksValue",
        181 => "GHouseDeed",
        182 => "GHouseTax",
        183 => "GHouseItem",
        184 => "GShopItem",
        185 => "NoAttackIfItemNum",
        186 => "PerfectStealth",
        187 => "Meditate",
        _ => return None,
    })
}

/// The message-only abilities MME hides unless `bForceAll` is set.
fn forced_name(num: i16) -> Option<&'static str> {
    Some(match num {
        101 => "ConfuseMsg",
        115 => "DescMsg",
        120 => "StartMsg",
        137 => "ShockMsg",
        144 => "NonMagicalSpell",
        155 => "DeathText",
        _ => return None,
    })
}

/// Port of modMMudFunc GetAbilityName: the ability's name, `"Ability n"` for a number with none,
/// and `""` for the message-only abilities (101, 115, 120, 137, 144, 155) unless `force_all`.
#[must_use]
pub fn get_ability_name(profile: Profile, num: i16, force_all: bool) -> String {
    match profile {
        Profile::Stock | Profile::Quicksilver(_) => {}
    }
    if let Some(s) = plain_name(num) {
        return s.to_string();
    }
    if let Some(s) = forced_name(num) {
        return if force_all {
            s.to_string()
        } else {
            String::new()
        };
    }
    format!("Ability {num}")
}

/// Port of modMMudFunc GetAbilityList: index 0 is empty, 1..=200 the names (`"[Ability n]"` for
/// a number with none, `"Name (n)"` unless `hide_record_numbers`).
#[must_use]
pub fn get_ability_list(profile: Profile, hide_record_numbers: bool) -> Vec<String> {
    let n_max: i16 = match profile {
        Profile::Stock | Profile::Quicksilver(_) => 200,
    };
    let mut list = vec![String::new()];
    for x in 1..=n_max {
        let name = get_ability_name(profile, x, false);
        list.push(if name.is_empty() || name == format!("Ability {x}") {
            format!("[Ability {x}]")
        } else if !hide_record_numbers {
            format!("{name} ({x})")
        } else {
            name
        });
    }
    list
}

/// What `GetAbilityStats`, `GetAbilDiffText` and `PullSpellEQ` read between them.
pub struct AbilityEnv<'a> {
    pub profile: Profile,
    pub names: &'a dyn NameLookup,
    pub textblocks: &'a dyn TextblockData,
    pub spells: &'a [Spell],
    /// `bHideRecordNumbers`.
    pub hide_record_numbers: bool,
    /// `bQuickSpell`.
    pub quick_spell: bool,
    /// `Val(txtGlobalLevel(0).Text)` source: `Some(text)` when `chkGlobalFilter` is on.
    pub global_level_text: Option<&'a str>,
    /// `gAvgLevelMaxAllStats`.
    pub avg_level_max_all_stats: f64,
    /// `nSpellNest`.
    pub nest: Cell<i32>,
}

impl SpellText for AbilityEnv<'_> {
    fn ability_stats(
        &self,
        ability: i16,
        value: i16,
        calc_level: bool,
        percent_column: bool,
    ) -> String {
        self.get_ability_stats(ability, value, calc_level, percent_column)
    }
    fn spell_name(&self, number: i32) -> String {
        get_spell_name(self.names, number, self.hide_record_numbers)
    }
    fn monster_name(&self, number: i32) -> String {
        get_monster_name(self.names, number, self.hide_record_numbers)
    }
}

/// `GetSpellName` and `PullSpellEQ(True, 0, n, ..., True, ..., bonus)` for `CalculateAttack`.
impl CastText for AbilityEnv<'_> {
    fn spell_name(&self, number: i32) -> String {
        get_spell_name(self.names, number, self.hide_record_numbers)
    }
    fn spell_eq(&self, number: i32, spell_bonus: i16) -> String {
        self.pull_spell_eq(&PullArgs {
            calc_level: true,
            spell: number,
            percent_column: true,
            spell_bonus,
            ..PullArgs::default()
        })
    }
}

impl<'a> AbilityEnv<'a> {
    /// The environment over a loaded database, with MME's globals at their start-up values
    /// (record numbers shown, no quick-spell style, no global level filter, `nSpellNest` 0).
    /// Set the public fields afterwards for any other state. This is the production
    /// implementation of `spells::SpellText` (for `SpellEnv` and `SpellCastEnv`) and of
    /// `attack::CastText` (for `AttackEnv`).
    #[must_use]
    pub fn for_world(world: &'a World, profile: Profile) -> Self {
        AbilityEnv {
            profile,
            names: world,
            textblocks: world,
            spells: &world.spells,
            hide_record_numbers: false,
            quick_spell: false,
            global_level_text: None,
            avg_level_max_all_stats: 0.0,
            nest: Cell::new(0),
        }
    }
}

impl AbilityEnv<'_> {
    /// Port of modMMudDatabase PullSpellEQ with `GetAbilityStats` and the name lookups answered by
    /// this environment, sharing `nSpellNest` with the calls it makes back.
    #[must_use]
    pub fn pull_spell_eq(&self, a: &PullArgs) -> String {
        let depth = self.nest.get();
        let env = SpellEnv {
            profile: self.profile,
            spells: self.spells,
            text: self,
            quick_spell: self.quick_spell,
            global_level_text: self.global_level_text,
            avg_level_max_all_stats: self.avg_level_max_all_stats,
            nest: Cell::new(depth),
        };
        // `SpellEnv` raises its own counter for the call; text callbacks run inside it and must
        // see that raised value, as MME's global is.
        self.nest.set(depth + 1);
        let out = env.pull_spell_eq(a);
        self.nest.set(depth);
        out
    }

    fn pull_nested(&self, calc_level: bool, spell: i32, percent_column: bool) -> String {
        self.pull_spell_eq(&PullArgs {
            calc_level,
            spell,
            percent_column,
            is_nested: true,
            ..PullArgs::default()
        })
    }

    /// Port of modMMudFunc GetAbilityStats (with `lv = Nothing`): the ability name followed by its
    /// value, e.g. `"Damage +5"`, `"CastsSp [Name(12), Damage 5 to 9]"`. `calc_level` is
    /// `bCalcSpellLevel` (True when MME's call leaves it out).
    #[must_use]
    pub fn get_ability_stats(
        &self,
        num: i16,
        value: i16,
        calc_level: bool,
        percent_column: bool,
    ) -> String {
        let mut out = get_ability_name(self.profile, num, false);
        if out.is_empty() {
            return out;
        }
        // An error after the name is assigned leaves the name (plus anything appended) as the answer.
        let _ = self.stats_inner(num, value, calc_level, percent_column, &mut out);
        out
    }

    fn stats_inner(
        &self,
        num: i16,
        value: i16,
        calc_level: bool,
        percent_column: bool,
        out: &mut String,
    ) -> Res<()> {
        let hide = self.hide_record_numbers;
        if num == 148 && value > 0 {
            let action = get_textblock_action(self.textblocks, i32::from(value));
            if action.to_ascii_lowercase().contains("cast ") {
                let mut casts = String::new();
                let mut all_casts = true;
                for part in action.split(':') {
                    if vb::left(part, 5)? == "cast " {
                        let spell = vb::clng(vb::val(&vb::mid(part, 6, None)?))?;
                        let text = self.pull_spell_eq(&PullArgs {
                            spell,
                            ..PullArgs::default()
                        });
                        if !casts.is_empty() {
                            casts.push_str(", ");
                        }
                        casts.push_str(&text);
                    } else {
                        all_casts = false;
                        break;
                    }
                }
                if all_casts && !casts.is_empty() {
                    *out = if casts.to_ascii_lowercase().contains("(click)") {
                        "(click)".into()
                    } else {
                        casts
                    };
                    return Ok(());
                }
            }
        }
        if value == 0 {
            return Ok(());
        }
        let header = if value < 0 { " " } else { " +" };
        let v = i32::from(value);
        match num {
            7 => out.push_str(&format!("{header}{}", vb::cstr(f64::from(value) / 10.0))),
            42 | 122 | 160 => {
                out.push_str(&format!(" ({})", get_spell_name(self.names, v, hide)));
            }
            43 | 153 | 151 => {
                let eq = self.pull_nested(calc_level, v, percent_column);
                out.push_str(&format!(" [{}, {eq}]", get_spell_name(self.names, v, hide)));
            }
            73 | 124 => {
                out.push_str(&format!(
                    " ({})",
                    get_ability_name(self.profile, value, false)
                ));
            }
            59 => out.push_str(&format!(" {}", get_class_name(self.names, v))),
            146 | 12 => out.push_str(&format!(" {}", get_monster_name(self.names, v, hide))),
            1 | 8 | 17 | 18 | 19 | 140 | 141 | 148 => out.push_str(&format!(" {value}")),
            178 => {}
            185 | 1115 => out.push_str(&format!(" {}", get_item_name(self.names, v, hide))),
            _ => out.push_str(&format!("{header}{value}")),
        }
        Ok(())
    }

    /// Port of modMain GetAbilDiffText (without its unused `sCurrentText`): the text the
    /// comparison views show for an ability whose two values are `value1` and `value2`.
    /// `""` where MME's handler swallows a run-time error (a difference or number outside the
    /// Integer range, say) before anything was assigned.
    #[must_use]
    pub fn get_abil_diff_text(
        &self,
        abil: i32,
        value1: i32,
        value2: i32,
        percent: i16,
        opposite_math: bool,
    ) -> String {
        self.diff_inner(abil, value1, value2, percent, opposite_math)
            .unwrap_or_default()
    }

    fn diff_inner(
        &self,
        abil: i32,
        value1: i32,
        value2: i32,
        percent: i16,
        opposite_math: bool,
    ) -> Res<String> {
        let value = if opposite_math {
            vb::sub_i32(value2, value1)?
        } else {
            vb::sub_i32(value1, value2)?
        };
        let hide = self.hide_record_numbers;
        Ok(match abil {
            59 => get_class_name(self.names, value1),
            43 => {
                if value1 > 0 {
                    let mut s = format!(
                        "[{}, {}",
                        get_spell_name(self.names, value1, hide),
                        self.pull_spell_eq(&PullArgs {
                            calc_level: true,
                            spell: value1,
                            ..PullArgs::default()
                        })
                    );
                    if percent == 0 {
                        s.push(']');
                    } else {
                        s.push_str(&format!(", {percent}%]"));
                    }
                    s
                } else {
                    String::new()
                }
            }
            114 => String::new(),
            1114 => self.get_ability_stats(to_i16(abil)?, to_i16(value)?, true, false),
            _ => {
                let mut s = self.get_ability_stats(to_i16(abil)?, to_i16(value)?, true, false);
                if abil == 135 {
                    s.push_str(&format!(
                        " ({})",
                        if opposite_math { value2 } else { value1 }
                    ));
                }
                s
            }
        })
    }
}
