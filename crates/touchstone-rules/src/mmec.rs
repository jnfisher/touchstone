//! `.mmec` character files: MME's `SaveCharacter` and `LoadCharacter`, as text in and text out.
//! Card P4-24.
//!
//! A `.mmec` is a Windows INI file written key by key with `WritePrivateProfileString` and read
//! with `GetPrivateProfileString` (CRLF, Windows-1252, case-insensitive names). [`parse`] reads
//! one the way `LoadCharacter` does (missing keys take the default `ReadINI` passes, values are
//! clamped, optional blocks are read only when their gate key is set) and [`write`] emits the
//! keys in `SaveCharacter`'s order with its number formatting, so a file MME wrote reads and
//! writes back byte for byte.
//!
//! Only what [`Character`] models is parsed into fields. Everything else (the PlayerInfo keys
//! that hold form state touchstone has no use for, the whole MonsterFilter and Compare sections,
//! unknown keys and sections) stays in [`Mmec::extra`] and is written back as read.
//!
//! This module does no IO and needs no database. The rules of `LoadCharacter` that look at game
//! data (is the learned spell usable by the class, is the bless number a bless spell, where does
//! `InvenEquipItem` put an item) are applied by the caller, which has the tables; the parts that
//! do not need them are done here.

use touchstone_vb6 as vb;

use crate::character::{AttackType, Character};

/// An Item Manager row: `item|qty` in `IM_CARRIED` / `IM_STASH`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemStack {
    pub item: i32,
    pub qty: i32,
}

/// A key touchstone does not model, kept in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraEntry {
    pub section: String,
    pub key: String,
    pub value: String,
}

/// A character file: the modelled character plus everything else the file held.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mmec {
    pub character: Character,
    pub carried: Vec<ItemStack>,
    pub stash: Vec<ItemStack>,
    pub extra: Vec<ExtraEntry>,
}

impl Mmec {
    fn extra_get(&self, section: &str, key: &str) -> Option<&str> {
        self.extra
            .iter()
            .find(|e| e.section.eq_ignore_ascii_case(section) && e.key.eq_ignore_ascii_case(key))
            .map(|e| e.value.as_str())
    }
}

/// Why a file is not a character file.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MmecError {
    /// None of the sections `SaveCharacter` writes is there.
    #[error(
        "not an MMUD Explorer character file (no PlayerInfo, Inventory, Bless or LearnedSpells section)"
    )]
    NotMmec,
}

/// Equipment slot key names, by `nEquippedItem` index.
pub const SLOT_KEYS: [&str; 20] = [
    "Head",
    "Ears",
    "Neck",
    "Back",
    "Torso",
    "Arms",
    "Wrist",
    "Wrist2",
    "Hands",
    "Finger1",
    "Finger2",
    "Waist",
    "Legs",
    "Feet",
    "Worn",
    "Off-Hand",
    "Weapon",
    "Eyes",
    "Face",
    "Everywhere",
];

/// The order `LoadCharacter` equips the slots in (Waist before Hands); a slot's item is placed
/// by its own worn type, so the order decides which item wins a shared slot.
pub const LOAD_ORDER: [usize; 20] = [
    0, 1, 2, 3, 4, 5, 6, 7, 11, 8, 9, 10, 12, 13, 14, 15, 16, 17, 18, 19,
];

/// Highest `ListIndex` of each quest option list on the stock database (`SetUpFormObjects`).
pub const QUEST_OPTION_MAX: [i16; 4] = [5, 6, 6, 2];

/// `TOTAL_STAT_LBLS + 1`: entries in `char_StatAdjustments`.
const ADJUSTMENTS: usize = 47;
/// `SaveCharacter` stops after this many Item Manager rows per list.
const MAX_STACKS: usize = 50;

// ---------------------------------------------------------------------------------------------
// Windows-1252 and INI text

const CP1252_HIGH: [char; 32] = [
    '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}', '\u{017D}', '\u{008F}',
    '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
];

/// Windows-1252 bytes as text.
#[must_use]
pub fn decode_cp1252(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9F => CP1252_HIGH[usize::from(b - 0x80)],
            _ => char::from(b),
        })
        .collect()
}

/// Text as Windows-1252 bytes; a character the code page lacks becomes `?`.
#[must_use]
pub fn encode_cp1252(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| {
            let u = u32::from(c);
            if u < 0x80 || (0xA0..=0xFF).contains(&u) {
                u8::try_from(u).unwrap_or(b'?')
            } else if let Some(i) = CP1252_HIGH.iter().position(|&h| h == c) {
                0x80 + u8::try_from(i).unwrap_or(0)
            } else {
                b'?'
            }
        })
        .collect()
}

struct Section {
    name: String,
    entries: Vec<(String, String)>,
}

/// `GetPrivateProfileString`'s reading of a value: surrounding blanks go, and so does one pair of
/// matching quotes around what is left.
fn clean_value(raw: &str) -> String {
    let t = raw.trim_matches([' ', '\t']);
    let b = t.as_bytes();
    if b.len() >= 2 && (b[0] == b'"' || b[0] == b'\'') && b[b.len() - 1] == b[0] {
        t[1..t.len() - 1].to_owned()
    } else {
        t.to_owned()
    }
}

fn parse_ini(text: &str) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    let mut cur: Option<usize> = None;
    for line in text.split(['\r', '\n']) {
        let t = line.trim_matches([' ', '\t']);
        if t.is_empty() || t.starts_with(';') {
            continue;
        }
        if let Some(rest) = t.strip_prefix('[') {
            if let Some(end) = rest.find(']') {
                let name = &rest[..end];
                cur = Some(
                    out.iter()
                        .position(|s| s.name.eq_ignore_ascii_case(name))
                        .unwrap_or_else(|| {
                            out.push(Section {
                                name: name.to_owned(),
                                entries: Vec::new(),
                            });
                            out.len() - 1
                        }),
                );
            }
            continue;
        }
        if let (Some(i), Some((k, v))) = (cur, t.split_once('=')) {
            out[i]
                .entries
                .push((k.trim_matches([' ', '\t']).to_owned(), clean_value(v)));
        }
    }
    out
}

struct Ini {
    sections: Vec<Section>,
}

impl Ini {
    /// The first value of a key; empty values count as missing, as `ReadINI` treats them.
    fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.sections
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(section))?
            .entries
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
            .filter(|v| !v.is_empty())
    }

    /// `ReadINI(section, key, file, default)`.
    fn read(&self, section: &str, key: &str, default: &str) -> String {
        self.get(section, key).unwrap_or(default).to_owned()
    }

    /// `Val(ReadINI(section, key, file))`: the default is `"0"`.
    fn num(&self, section: &str, key: &str) -> f64 {
        vb::val(self.get(section, key).unwrap_or("0"))
    }

    /// The same as a `Long` (an assignment to a `Long` rounds half to even).
    fn long(&self, section: &str, key: &str) -> i32 {
        to_long(self.num(section, key))
    }
}

fn to_long(x: f64) -> i32 {
    vb::clng(x.clamp(-2_147_483_647.0, 2_147_483_647.0)).unwrap_or(0)
}

fn to_int(x: f64) -> i16 {
    i16::try_from(to_long(x).clamp(i32::from(i16::MIN), i32::from(i16::MAX))).unwrap_or(0)
}

// ---------------------------------------------------------------------------------------------
// What SaveCharacter writes

/// Where a key's value comes from when writing.
enum Src {
    /// The modelled character.
    Val(String),
    /// A key touchstone does not model: `Mmec::extra`, else this default when there is one.
    Extra(Option<&'static str>),
}

const MONSTER_FILTER_KEYS: [&str; 21] = [
    "RegenOpt",
    "RegenVal",
    "MonMagicOpt",
    "MonMagicVal",
    "Damage",
    "DamageParty",
    "HP",
    "HPParty",
    "EXP",
    "EXPParty",
    "MonDmgOUT",
    "MonMagDmgOUT",
    "MonPartyDmgOUT",
    "MonPartyMagDmgOUT",
    "MobRegenOpt",
    "MobRegenVal",
    "MobMagicOpt",
    "MobMagicVal",
    "MobDamage",
    "MobHP",
    "MobEXP",
];

const MONSTER_FILTER_EXTRA_KEYS: [&str; 25] = [
    "MonExtraFilterAC",
    "MonExtraFilterDR",
    "MonExtraFilterMR",
    "MonExtraFilterBSDef",
    "MonExtraFilterGL",
    "MonExtraFilterLairExp",
    "MonExtraFilterAccMaj",
    "MonExtraFilterAccMax",
    "MonExtraFilterNumLairs",
    "MonExtraFilterMbLt",
    "MonExtraFilterMbGt",
    "MonExtraFilterFollowGt",
    "MonExtraFilterFollowLt",
    "MonExtraFilterAtLeastLairs",
    "MonExtraFilterAtLeastLairsNum",
    "MonExtraFilterDodge",
    "MonExtraFilterCash",
    "MonExtraFilterUndead",
    "MonExtraFilterNonHostile_vEvil",
    "MonExtraFilterNonHostile_vNG",
    "MonExtraFilterNoPoison",
    "MonExtraFilterNoConfusion",
    "MonExtraFilterNoFear",
    "MonExtraFilterShowAll",
    "MonExtraFilterReachableOnly",
];

const COMPARE_KEYS: [&str; 4] = [
    "WeaponCompare",
    "ArmourCompare",
    "SpellCompare",
    "MonsterCompare",
];

fn flag(b: bool) -> String {
    String::from(if b { "1" } else { "0" })
}

fn stacks_text(list: &[ItemStack]) -> String {
    let mut s = String::new();
    for st in list.iter().take(MAX_STACKS) {
        s += &format!("{}|{},", st.item, st.qty.max(1));
    }
    s
}

/// Every key `SaveCharacter` writes, in its order.
fn layout(m: &Mmec) -> Vec<(&'static str, String, Src)> {
    let c = &m.character;
    let a = &c.attack;
    let mut v: Vec<(&'static str, String, Src)> = Vec::new();
    let mut put = |sec: &'static str, key: &str, src: Src| v.push((sec, key.to_owned(), src));
    let val = |s: String| Src::Val(s);
    let p = "PlayerInfo";

    // MME creates the file with this key, so it comes first
    put(p, "Name", val(c.name.clone()));
    put(p, "DataFile", Src::Extra(Some("")));
    put(p, "DataFileVer", Src::Extra(Some("")));
    put(p, "Class", val(c.class.unwrap_or(0).to_string()));
    put(p, "Race", val(c.race.unwrap_or(0).to_string()));
    put(p, "Level", val(c.level.to_string()));
    put(p, "Alignment", val(c.alignment.to_string()));
    put(p, "GlobalMinLVL", Src::Extra(Some("0")));
    for (k, s) in [
        "Strength",
        "Intellect",
        "Widsom",
        "Agility",
        "Health",
        "Charm",
    ]
    .into_iter()
    .zip(c.stats)
    {
        put(p, k, val(s.to_string()));
    }
    put(p, "WeaponVSCharAC", Src::Extra(Some("50")));
    put(p, "WeaponVSCharDR", Src::Extra(Some("0")));
    put(p, "WeaponVSCharDodge", Src::Extra(Some("0")));
    put(p, "SpellVSMR", Src::Extra(Some("50")));
    put(p, "AddWeight", val(c.add_weight.to_string()));
    put(p, "UseAddWeight", val(flag(c.use_add_weight)));
    put(p, "MR", val(vb::cstr(vb::val(&c.mr_text))));
    put(p, "AntiMagic", val(flag(c.anti_magic)));
    put(p, "UnequipMissing", Src::Extra(Some("1")));
    for (x, q) in c.quests.iter().enumerate() {
        put(p, &format!("Quest{x}"), val(flag(*q)));
    }
    for (k, o) in ["Quest_2nd", "Quest_6th", "Quest_Extra1", "Quest_Extra2"]
        .into_iter()
        .zip(c.quest_options)
    {
        put(p, k, val(o.to_string()));
    }
    put(
        p,
        "CurrentAttackType",
        val((a.attack_type as i32).to_string()),
    );
    put(p, "CurrentAttackMA", val(a.martial_art.to_string()));
    put(p, "CurrentAttackSpellNum", val(a.spell_num.to_string()));
    put(p, "CurrentAttackSpellLVL", val(a.spell_lvl.to_string()));
    put(p, "CurrentAttackManual", val(a.manual_p.to_string()));
    put(p, "CurrentAttackManualMag", val(a.manual_m.to_string()));
    put(p, "nGlobalAttackHealType", val(a.heal_type.to_string()));
    put(
        p,
        "nGlobalAttackHealSpellNum",
        val(a.heal_spell_num.to_string()),
    );
    put(
        p,
        "nGlobalAttackHealSpellLVL",
        val(a.heal_spell_lvl.to_string()),
    );
    put(p, "nGlobalAttackHealManual", val(a.heal_manual.to_string()));
    put(p, "bGlobalAttackUseMeditate", val(flag(a.use_meditate)));
    put(p, "bGlobalAttackBackstab", val(flag(a.backstab)));
    put(p, "nGlobalAttackHealRounds", val(a.heal_rounds.to_string()));
    put(
        p,
        "nGlobalAttackBackstabWeapon",
        val(a.backstab_weapon.to_string()),
    );
    let adj: Vec<String> = (0..ADJUSTMENTS)
        .map(|i| c.stat_adjustments.get(i).copied().unwrap_or(0).to_string())
        .collect();
    put(p, "char_StatAdjustments", val(adj.join(",")));

    let f = "MonsterFilter";
    for k in &MONSTER_FILTER_KEYS {
        put(f, k, Src::Extra(None));
    }
    for x in 0..10 {
        put(f, &format!("MonLairFilterTXT{x}"), Src::Extra(None));
    }
    for k in MONSTER_FILTER_EXTRA_KEYS.iter() {
        put(f, k, Src::Extra(None));
    }

    let i = "Inventory";
    for (k, n) in SLOT_KEYS.iter().zip(c.equipped) {
        put(i, k, val(n.to_string()));
    }
    put(i, "IM_CARRIED", val(stacks_text(&m.carried)));
    put(i, "IM_STASH", val(stacks_text(&m.stash)));

    for (x, n) in c.bless.iter().enumerate() {
        put("Bless", &format!("Bless{x}"), val(n.to_string()));
    }
    for (x, n) in c.learned.spells.iter().enumerate() {
        put(
            "LearnedSpells",
            &format!("LearnedSpell{x}"),
            val(n.to_string()),
        );
    }
    for k in &COMPARE_KEYS {
        put("Compare", k, Src::Extra(None));
    }
    v
}

/// Writes a character file as `SaveCharacter` does: sections in its order separated by a blank
/// line, `Key=Value` lines, CRLF, Windows-1252.
#[must_use]
pub fn write(m: &Mmec) -> Vec<u8> {
    let lay = layout(m);
    let mut sections: Vec<Section> = Vec::new();
    let push = |sections: &mut Vec<Section>, sec: &str, key: &str, value: String| {
        let i = sections
            .iter()
            .position(|s| s.name.eq_ignore_ascii_case(sec))
            .unwrap_or_else(|| {
                sections.push(Section {
                    name: sec.to_owned(),
                    entries: Vec::new(),
                });
                sections.len() - 1
            });
        sections[i].entries.push((key.to_owned(), value));
    };
    for (sec, key, src) in &lay {
        let value = match src {
            Src::Val(s) => Some(s.clone()),
            Src::Extra(d) => m
                .extra_get(sec, key)
                .map(str::to_owned)
                .or_else(|| d.map(str::to_owned)),
        };
        if let Some(v) = value {
            push(&mut sections, sec, key, v);
        }
    }
    for e in &m.extra {
        let known = lay
            .iter()
            .any(|(s, k, _)| s.eq_ignore_ascii_case(&e.section) && k.eq_ignore_ascii_case(&e.key));
        if !known {
            push(&mut sections, &e.section, &e.key, e.value.clone());
        }
    }
    let mut text = String::new();
    for (n, s) in sections
        .iter()
        .filter(|s| !s.entries.is_empty())
        .enumerate()
    {
        if n > 0 {
            text += "\r\n";
        }
        text += &format!("[{}]\r\n", s.name);
        for (k, v) in &s.entries {
            text += &format!("{k}={v}\r\n");
        }
    }
    encode_cp1252(&text)
}

// ---------------------------------------------------------------------------------------------
// What LoadCharacter reads

fn stacks(text: &str) -> Vec<ItemStack> {
    let t = text.trim();
    let mut out = Vec::new();
    if t.is_empty() {
        return out;
    }
    for part in t.split(',') {
        let mut f = part.split('|');
        if let (Some(item), Some(qty)) = (f.next(), f.next()) {
            out.push(ItemStack {
                item: to_long(vb::val(item)),
                qty: to_long(vb::val(qty)),
            });
        }
    }
    out
}

/// Reads a character file as `LoadCharacter` does. The world-dependent steps are the caller's:
/// the learned spells come back as written, `equipped` holds each slot's key value (the caller
/// places items by worn type in [`LOAD_ORDER`]), and a bless number is only checked for
/// duplicates.
///
/// # Errors
/// [`MmecError::NotMmec`] when no section `SaveCharacter` writes is present.
pub fn parse(bytes: &[u8]) -> Result<Mmec, MmecError> {
    let ini = Ini {
        sections: parse_ini(&decode_cp1252(bytes)),
    };
    let known = ["PlayerInfo", "Inventory", "Bless", "LearnedSpells"];
    if !ini
        .sections
        .iter()
        .any(|s| known.iter().any(|k| s.name.eq_ignore_ascii_case(k)))
    {
        return Err(MmecError::NotMmec);
    }
    let p = "PlayerInfo";
    let mut m = Mmec::default();
    let c = &mut m.character;

    let name = ini.read(p, "Name", "0");
    c.name = if name == "0" { String::new() } else { name };
    c.level = to_long(ini.num(p, "Level")).clamp(0, 999);
    let align = to_int(ini.num(p, "Alignment"));
    c.alignment = if (0..=3).contains(&align) { align } else { 0 };
    for (s, k) in c.stats.iter_mut().zip([
        "Strength",
        "Intellect",
        "Widsom",
        "Agility",
        "Health",
        "Charm",
    ]) {
        *s = to_long(ini.num(p, k)).clamp(0, 999);
    }
    c.add_weight = to_long(ini.num(p, "AddWeight")).clamp(0, 999);
    c.use_add_weight = ini.num(p, "UseAddWeight") > 0.0;
    c.anti_magic = ini.num(p, "AntiMagic") > 0.0;
    // LoadCharacter never reads MR (RefreshMagicRes recomputes it); touchstone keeps it as text
    c.mr_text = ini
        .get(p, "MR")
        .map(|v| vb::cstr(vb::val(v)))
        .unwrap_or_default();

    for (x, q) in c.quests.iter_mut().enumerate() {
        *q = ini.long(p, &format!("Quest{x}")) > 0;
    }
    for (x, k) in ["Quest_2nd", "Quest_6th", "Quest_Extra1", "Quest_Extra2"]
        .into_iter()
        .enumerate()
    {
        let n = to_int(ini.num(p, k));
        c.quest_options[x] = if (0..=QUEST_OPTION_MAX[x]).contains(&n) {
            n
        } else {
            0
        };
    }
    c.class = Some(ini.long(p, "Class"));
    c.race = Some(ini.long(p, "Race"));

    let adj = ini.read(p, "char_StatAdjustments", "");
    let adj = adj.trim();
    c.stat_adjustments = vec![0; ADJUSTMENTS];
    if adj.chars().count() > 1 && adj.contains(',') {
        for (slot, part) in c.stat_adjustments.iter_mut().zip(adj.split(',')) {
            *slot = to_long(vb::val(part));
        }
    }

    load_attack(&ini, c);

    for (x, n) in c.bless.iter_mut().enumerate() {
        *n = ini.long("Bless", &format!("Bless{x}")).max(0);
    }
    // EquipBlessSpell: a spell already in a slot is not equipped again
    for x in 0..c.bless.len() {
        if c.bless[x] > 0 && c.bless[..x].contains(&c.bless[x]) {
            c.bless[x] = 0;
        }
    }
    for (x, n) in c.learned.spells.iter_mut().enumerate() {
        *n = ini.long("LearnedSpells", &format!("LearnedSpell{x}"));
    }
    for (slot, key) in c.equipped.iter_mut().zip(SLOT_KEYS) {
        *slot = ini.long("Inventory", key);
    }
    fix_attack_for_weapon(c);

    m.carried = stacks(&ini.read("Inventory", "IM_CARRIED", ""));
    m.stash = stacks(&ini.read("Inventory", "IM_STASH", ""));

    let modelled: Vec<(&str, String)> = layout(&m)
        .into_iter()
        .filter(|(_, _, s)| matches!(s, Src::Val(_)))
        .map(|(sec, k, _)| (sec, k.to_ascii_lowercase()))
        .collect();
    for s in &ini.sections {
        for (k, v) in &s.entries {
            let lk = k.to_ascii_lowercase();
            if !modelled
                .iter()
                .any(|(sec, mk)| sec.eq_ignore_ascii_case(&s.name) && *mk == lk)
            {
                m.extra.push(ExtraEntry {
                    section: s.name.clone(),
                    key: k.clone(),
                    value: v.clone(),
                });
            }
        }
    }
    Ok(m)
}

/// The attack and heal blocks of `LoadCharacter`. A block is read only when its gate key
/// (`CurrentAttackType`, `nGlobalAttackHealType`) is above zero; otherwise its fields take the
/// start-up values, and the backstab and meditate flags keep theirs (off, in a new character).
fn load_attack(ini: &Ini, c: &mut Character) {
    let p = "PlayerInfo";
    let a = &mut c.attack;
    let code = ini.num(p, "CurrentAttackType");
    if code > 0.0 {
        let t = to_long(code);
        a.attack_type = AttackType::from_code(t).unwrap_or(AttackType::OneShot);
        a.martial_art = to_int(ini.num(p, "CurrentAttackMA"));
        a.spell_num = ini.long(p, "CurrentAttackSpellNum");
        a.spell_lvl = to_int(ini.num(p, "CurrentAttackSpellLVL"));
        a.manual_p = ini.long(p, "CurrentAttackManual").clamp(0, 9_999_999);
        a.manual_m = ini.long(p, "CurrentAttackManualMag").clamp(0, 9_999_999);
        a.backstab = ini.num(p, "bGlobalAttackBackstab") > 0.0;
        a.backstab_weapon = ini.long(p, "nGlobalAttackBackstabWeapon");
        a.use_meditate = ini.num(p, "bGlobalAttackUseMeditate") > 0.0;
        if matches!(a.attack_type, AttackType::Spell | AttackType::SpellAny) && a.spell_num < 1 {
            a.attack_type = AttackType::OneShot;
            a.spell_num = 0;
            a.spell_lvl = 0;
        }
    } else {
        a.attack_type = AttackType::OneShot;
        a.martial_art = 0;
        a.spell_num = 0;
        a.spell_lvl = 0;
        a.manual_p = 0;
        a.manual_m = 0;
    }

    if ini.num(p, "nGlobalAttackHealType") > 0.0 {
        let t = to_int(ini.num(p, "nGlobalAttackHealType"));
        a.heal_type = if t <= 4 { t } else { 0 };
        a.heal_spell_num = ini.long(p, "nGlobalAttackHealSpellNum");
        a.heal_spell_lvl = to_int(ini.num(p, "nGlobalAttackHealSpellLVL")).clamp(0, 9999);
        a.heal_rounds = to_int(ini.num(p, "nGlobalAttackHealRounds")).clamp(1, 50);
        a.use_meditate = ini.num(p, "bGlobalAttackUseMeditate") > 0.0;
        if matches!(a.heal_type, 2 | 3) && a.heal_spell_num < 1 {
            a.heal_type = 0;
            a.heal_spell_num = 0;
            a.heal_spell_lvl = 0;
        }
        a.heal_manual = ini.long(p, "nGlobalAttackHealManual").clamp(0, 999_999);
    } else {
        a.heal_type = 0;
        a.heal_spell_num = 0;
        a.heal_spell_lvl = 0;
        a.heal_manual = 0;
        a.heal_rounds = 1;
    }
}

/// `LoadCharacter`'s last attack fix: a physical attack with no weapon equipped becomes one-shot.
/// Call again after the equipment is placed.
pub fn fix_attack_for_weapon(c: &mut Character) {
    if c.attack.attack_type == AttackType::PhysAttack && c.equipped[16] == 0 {
        c.attack.attack_type = AttackType::OneShot;
    }
}
