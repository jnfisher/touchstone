//! The weave family (R6): which spell a hybrid's weave fires, the per-level budget, the power
//! percentage and the expected damage of one proc.
//!
//! Every number is an answer of Quicksilver's public functions: the rule module for the weaver,
//! volley, budget, power and pick, and the spell effect code (`apply_effect`) for the damage a
//! hit does to a monster, run once per roll of the spell's range under a scripted source so the
//! mean is exact. The spells come from a small synthetic book, written in the header so the test
//! builds the same records.

use serde_json::{Value, json};
use silvermere_rules::abilities::AbilityLookup;
use silvermere_rules::rng::Rng;
use silvermere_rules::spells::apply::{
    Cast, CastPath, CastState, CasterStats, MonsterTarget, Outcome, Target, apply_effect,
};
use silvermere_rules::spells::resist::{
    apply_element_resistance, element_resistance, monster_magic_resistance, monster_resists_single,
};
use silvermere_rules::spells::{LegalBand, SpellEffects, amount_range};
use silvermere_rules::v120::weave::{
    Volley, budget, immune_to, is_candidate, pick, power_pct, roll_hits, weave_casting_stat,
    weaver,
};
use silvermere_types::config::Balance;
use silvermere_types::enums::{
    Ability, Alignment, ArmourType, Element, Gender, GroupRole, Magery, MonsterAttackType,
    MonsterType, ResistMode, ResistStat, SpellTarget, SpellType, WeaponClass,
};
use silvermere_types::flags::{ActiveMonsterFlags, SpellFlags};
use silvermere_types::ids::{ActiveMonsterId, ItemNo, SpellNo};
use silvermere_types::records::{
    ActiveMonster, ActiveSpell, Class, Coins, KnownMonster, MonsterAttack, MonsterItem,
    MonsterSpell, Spell,
};
use silvermere_types::{NAMTRAIL, NMONATTACKS, NMONITEMS, NMONSPELLS};

use crate::Family;

fn v120() -> Balance {
    Balance::V120
}

/// Chance 0 for both weave tiers: the one engine key the weaver's neutral setting changes.
fn neutral() -> Balance {
    Balance {
        weave_pct: 0,
        weave_pct_magery1: 0,
        ..Balance::V120
    }
}

/// One spell of the synthetic book.
struct Spec {
    number: i16,
    req_level: u16,
    energy: u16,
    min: i16,
    max: i16,
    min_inc: (u8, u8),
    max_inc: (u8, u8),
    cap: i16,
    magery: u8,
    magelvl: i16,
    target: u16,
    duration: i16,
    element: u16,
    resmode: u16,
    abilities: &'static [(u16, i16)],
}

const fn spec(number: i16, req_level: u16, energy: u16, (min, max): (i16, i16), magery: u8) -> Spec {
    Spec {
        number,
        req_level,
        energy,
        min,
        max,
        min_inc: (0, 0),
        max_inc: (0, 0),
        cap: 0,
        magery,
        magelvl: 2,
        target: 4,
        duration: 0,
        element: 4,
        resmode: 2,
        abilities: &[(17, 0)],
    }
}

/// Magery codes: 1 mage, 2 priest, 3 druid, 4 bard. Abilities: 1 damage, 17 damage on armour,
/// 23 kill dead, 97 good, 98 evil, 108 affects living, 151 cast on ending, 144 non magical.
fn book() -> Vec<Spec> {
    vec![
        Spec { min_inc: (1, 2), max_inc: (3, 2), element: 1, ..spec(101, 10, 500, (20, 30), 1) },
        Spec { max_inc: (1, 4), cap: 40, element: 0, abilities: &[(1, 0)], ..spec(102, 25, 250, (8, 14), 1) },
        Spec { target: 8, resmode: 1, abilities: &[(17, 0), (151, 104)], ..spec(103, 35, 1000, (60, 90), 1) },
        Spec { magelvl: 3, abilities: &[(17, 0)], ..spec(104, 99, 500, (30, 40), 1) },
        Spec { target: 9, ..spec(105, 20, 1000, (200, 300), 1) },
        Spec { duration: 5, ..spec(106, 20, 1000, (200, 300), 1) },
        Spec { abilities: &[(23, 0), (1, 0)], ..spec(107, 20, 1000, (200, 300), 1) },
        Spec { magelvl: 3, element: 3, ..spec(108, 20, 1000, (90, 130), 1) },
        Spec { element: 2, abilities: &[(1, 25), (17, 0)], ..spec(109, 30, 333, (5, 9), 1) },
        Spec { abilities: &[(108, 0), (17, 0)], ..spec(110, 15, 1000, (95, 105), 1) },
        Spec { abilities: &[(98, 0), (17, 0)], ..spec(111, 5, 1000, (400, 500), 1) },
        Spec { target: 0, element: 3, abilities: &[(1, 0)], min_inc: (1, 3), ..spec(201, 8, 500, (15, 22), 2) },
        Spec { target: 8, abilities: &[(17, 0)], max_inc: (2, 3), ..spec(202, 30, 1000, (50, 70), 2) },
        Spec { magelvl: 1, element: 5, resmode: 0, ..spec(203, 12, 200, (4, 6), 2) },
        Spec { target: 0, min_inc: (1, 1), max_inc: (1, 1), ..spec(301, 12, 400, (12, 18), 3) },
        Spec { magelvl: 1, element: 6, ..spec(302, 20, 600, (30, 36), 3) },
        Spec { magelvl: 1, element: 0, ..spec(401, 5, 1000, (10, 15), 4) },
        Spec { abilities: &[(1, 0)], max_inc: (1, 2), ..spec(402, 30, 500, (25, 35), 4) },
    ]
}

fn spec_json(s: &Spec) -> Value {
    json!({
        "number": s.number, "req_level": s.req_level, "energy": s.energy,
        "min": s.min, "max": s.max, "min_inc": [s.min_inc.0, s.min_inc.1],
        "max_inc": [s.max_inc.0, s.max_inc.1], "cap": s.cap, "magery": s.magery,
        "magelvl": s.magelvl, "target": s.target, "duration": s.duration,
        "element": s.element, "resmode": s.resmode, "abilities": s.abilities,
    })
}

type Slots = [(Ability, i16); 10];

fn slots(given: &[(u16, i16)]) -> Slots {
    let mut out = [(Ability::None, 0); 10];
    for (i, &(a, v)) in given.iter().enumerate() {
        out[i] = (Ability::from_code(a), v);
    }
    out
}

fn magery_of(code: u8) -> Magery {
    match code {
        1 => Magery::Mage,
        2 => Magery::Priest,
        3 => Magery::Druid,
        4 => Magery::Bard,
        5 => Magery::Kai,
        _ => Magery::from_code(0),
    }
}

fn engine_spell(s: &Spec) -> Spell {
    Spell {
        number: SpellNo(s.number),
        name: format!("s{}", s.number),
        descs: [String::new(), String::new()],
        resist: 0,
        levelcap: s.cap,
        flags: SpellFlags::empty(),
        abilities: slots(s.abilities),
        energy: s.energy,
        level: s.req_level,
        minval: s.min,
        maxval: s.max,
        spell_type: SpellType::Autocombat,
        resmode: ResistMode::from_code(s.resmode),
        diff: 0,
        durlvl: 0,
        target: SpellTarget::from_code(s.target),
        duration: s.duration,
        element: Element::from_code(s.element),
        resstat: ResistStat::from_code(0),
        magery: magery_of(s.magery),
        msgnum: 0.into(),
        mana: 10,
        maxinc: s.max_inc.0,
        maxlvls: s.max_inc.1,
        magelvl: s.magelvl,
        mininc: s.min_inc.0,
        minlvls: s.min_inc.1,
        durinc: 0,
        durlvls: 0,
        shortnm: String::new(),
    }
}

fn engine_class(number: u16, magery: u8, magelvl: i16) -> Class {
    Class {
        number: number.into(),
        name: String::new(),
        basehp: 0,
        hplevel: 0,
        exp: 0,
        attmult: 0,
        maxenergy: 0,
        restored: 0,
        abilities: slots(&[]),
        magery: magery_of(magery),
        magelvl,
        weapon: WeaponClass::from_code(0),
        armour: ArmourType::from_code(0),
        combat: 0,
        titles: 0.into(),
    }
}

fn known_monster() -> KnownMonster {
    KnownMonster {
        number: 0.into(),
        name: String::new(),
        montype: MonsterType::from_code(0),
        level: 0,
        genindex: 0,
        wornitem: ItemNo(0),
        weapon: ItemNo(0),
        dr: 0,
        ac: 0,
        group: 0,
        followpct: 0,
        magicres: 0,
        bsdefense: 0,
        exp: 0,
        maxhp: 0,
        energy: 0,
        hpregen: 0,
        abilities: slots(&[]),
        maxalive: 0,
        curalive: 0,
        grouprole: GroupRole::from_code(0),
        maxfollow: 0,
        undead: 0,
        alignment: Alignment::from_code(0),
        regenhrs: 0,
        killed_at: 0,
        arrivemsg: 0.into(),
        deathmsg: 0.into(),
        items: [MonsterItem { item: ItemNo(0), uses: 0, pct: 0 }; NMONITEMS],
        cash: Coins::default(),
        talktext: 0.into(),
        charmlvl: 0,
        namelist: 0,
        attacks: [MonsterAttack {
            kind: MonsterAttackType::from_code(0),
            accuracy: 0,
            pct: 0,
            min: 0,
            max: 0,
            hitmsg: 0.into(),
            defmsg: 0.into(),
            missmsg: 0.into(),
            energy: 0,
            hitspell: SpellNo(0),
        }; NMONATTACKS],
        randtext: 0.into(),
        charmres: 0,
        deathspell: SpellNo(0),
        createspell: SpellNo(0),
        spells: [MonsterSpell { spell: SpellNo(0), pct: 0, level: 0 }; NMONSPELLS],
        desc: Default::default(),
        gender: Gender::from_code(0),
    }
}

fn active_monster() -> ActiveMonster {
    ActiveMonster {
        number: ActiveMonsterId(1),
        knownnum: 0.into(),
        level: 0,
        map: 0.into(),
        room: 0.into(),
        poison: 0,
        energy: 0,
        hp: 30000,
        following: String::new(),
        trail: [0.into(); NAMTRAIL],
        hunters: [ActiveMonsterId(0); 10],
        hunting: ActiveMonsterId(0),
        name: String::new(),
        wornitem: ItemNo(0),
        weapon: ItemNo(0),
        items: [(ItemNo(0), 0); NMONITEMS],
        cash: Coins::default(),
        maxhp: 30000,
        alignment: Alignment::from_code(0),
        followpct: 0,
        dr: 0,
        ac: 0,
        exp: 0,
        maxenergy: 0,
        friendly: 0,
        genroom: 0.into(),
        inactive: 0,
        flags: ActiveMonsterFlags::empty(),
        montype: MonsterType::from_code(0),
        target: 0,
        hpregen: 0,
        lastdir: 0,
        lastupd: 0,
        grouprole: GroupRole::from_code(0),
        spells: [ActiveSpell { spell: SpellNo(0), value: 0, rounds: 0 }; NMONSPELLS],
        downrounds: 0,
        created: 0,
    }
}

/// A source answering a script: draw `n` returns `draws[n]` (0 past the end), held to the range.
struct Script {
    draws: Vec<i32>,
    pos: usize,
}

impl Rng for Script {
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        let v = self.draws.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        v.clamp(lo, hi - 1)
    }

    fn long(&mut self, lo: i64, hi: i64) -> i64 {
        i64::from(self.range(lo as i32, hi as i32))
    }
}

/// The monster as the case describes it.
#[derive(Clone, Copy)]
struct Target1 {
    mr: i32,
    anti: bool,
    /// cold, fire, stone, lightning, water, poison resistance in percent
    el: [i32; 6],
    immunity: i32,
    animal: bool,
    non_living: bool,
}

impl Target1 {
    const PLAIN: Self = Self { mr: 1, anti: false, el: [0; 6], immunity: 0, animal: false, non_living: false };

    fn json(&self) -> Value {
        json!({
            "mr": self.mr, "anti_magic": self.anti, "cold": self.el[0], "fire": self.el[1],
            "stone": self.el[2], "lightning": self.el[3], "water": self.el[4],
            "poison": self.el[5], "spell_immunity": self.immunity,
            "animal": self.animal, "non_living": self.non_living,
        })
    }
}

impl AbilityLookup for Target1 {
    fn value(&self, ability: Ability) -> i32 {
        match ability {
            Ability::ColdDefence => self.el[0],
            Ability::HotDefence => self.el[1],
            Ability::ResistStone => self.el[2],
            Ability::ResistLightning => self.el[3],
            Ability::ResistWater => self.el[4],
            Ability::PoisonImmune => self.el[5],
            _ => 0,
        }
    }

    fn has(&self, ability: Ability) -> bool {
        self.value(ability) != 0
    }

    fn pair(&self, first: Ability, second: Ability) -> (i32, i32) {
        (self.value(first), self.value(second))
    }

    fn best(&self, ability: Ability) -> Option<i32> {
        Some(self.value(ability))
    }
}

struct World {
    spells: std::collections::HashMap<SpellNo, Spell>,
}

fn world() -> World {
    World { spells: book().iter().map(|s| (SpellNo(s.number), engine_spell(s))).collect() }
}

/// The engine's pick for a class at a level against a target (None when nothing qualifies).
fn engine_pick<'a>(
    w: &'a World,
    class: &Class,
    level: u16,
    t: &Target1,
    b: &Balance,
) -> Option<silvermere_rules::v120::weave::Pick<'a>> {
    let cands = w
        .spells
        .values()
        .filter(|s| is_candidate(s, class, level, LegalBand::Neutral))
        .filter(|s| !immune_to(s, t.immunity, false, t.animal, t.non_living));
    pick(cands, level, &|n| w.spells.get(&n), b)
}

const CLASSES: [(u16, u8, i16); 7] = [
    (10, 1, 2), // gypsy
    (11, 1, 2), // warlock
    (4, 2, 2),  // cleric
    (6, 2, 2),  // missionary
    (9, 4, 2),  // bard
    (3, 2, 1),  // paladin
    (14, 3, 1), // ranger
];

fn weaver_case(number: u16, magery: u8, magelvl: i16) -> Value {
    let class = engine_class(number, magery, magelvl);
    let col = |b: &Balance| {
        weaver(&class, b).map_or(Value::Null, |w| {
            json!({ "chance_pct": w.chance_pct, "bard": w.bard, "class": w.class })
        })
    };
    json!({
        "id": format!("weaver/c{number:02}-m{magery}-l{magelvl}"),
        "kind": "weaver",
        "inputs": { "class": number, "magery": magery, "magelvl": magelvl },
        "v120": col(&v120()),
        "neutral": col(&neutral()),
    })
}

fn budget_case(level: u16) -> Value {
    json!({
        "id": format!("budget/l{level:03}"),
        "kind": "budget",
        "inputs": { "level": level },
        "v120": budget(level, &v120()),
        "neutral": Value::Null,
    })
}

fn power_case(budget_v: i64, twice_average: i64) -> Value {
    let vol = Volley { hits: Vec::new(), twice_average };
    json!({
        "id": format!("power/b{budget_v:05}-t{twice_average:06}"),
        "kind": "power",
        "inputs": { "budget": budget_v, "twice_average": twice_average },
        "v120": power_pct(budget_v, &vol, &v120()),
        "neutral": Value::Null,
    })
}

fn pick_case(w: &World, tag: &str, class: (u16, u8, i16), level: u16, t: &Target1) -> Value {
    let b = v120();
    let c = engine_class(class.0, class.1, class.2);
    let answer = engine_pick(w, &c, level, t, &b).map_or(Value::Null, |p| {
        json!({
            "spell": p.spell.number.0, "twice_average": p.volley.twice_average,
            "power_pct": p.power_pct,
            "hits": p.volley.hits.iter().map(|s| s.number.0).collect::<Vec<_>>(),
        })
    });
    json!({
        "id": format!("pick/{tag}-c{:02}-l{level:03}", class.0),
        "kind": "pick",
        "inputs": { "class": class.0, "magery": class.1, "magelvl": class.2, "level": level, "target": t.json() },
        "v120": answer,
        "neutral": Value::Null,
    })
}

/// The exact mean damage of one unresisted proc, and how many of the 99 resist rolls stop it.
fn proc_answer(
    w: &World,
    class: (u16, u8, i16),
    level: u16,
    stat: i64,
    bonus: i32,
    t: &Target1,
    b: &Balance,
) -> Value {
    let c = engine_class(class.0, class.1, class.2);
    let Some(wv) = weaver(&c, b) else {
        return Value::Null;
    };
    let Some(p) = engine_pick(w, &c, level, t, b) else {
        return Value::Null;
    };
    let mr = monster_magic_resistance(t.mr, 0, None);
    let stats = CasterStats {
        firstname: "x",
        level,
        spelldamage_pct: bonus,
        spellduration_pct: 0,
        spellcast: 0,
        casting_stat: weave_casting_stat(stat, b),
        class: wv.class,
        balance: b,
    };
    let (km, mut am) = (known_monster(), active_monster());
    let mut unresisted = 0.0;
    let nhits = if wv.bard { 1 } else { p.volley.hits.len() };
    for h in 0..nhits {
        let hit_spell = if wv.bard { p.spell } else { p.volley.hits[h] };
        let range = amount_range(hit_spell, level);
        let rolls = if wv.bard { 1 } else { range.max - range.min + 1 };
        let mut sum = 0i64;
        for i in 0..rolls {
            let mut draws = vec![0; h + 1];
            draws[h] = i;
            let mut rng = Script { draws, pos: 0 };
            let hits = roll_hits(&p, &wv, level, b, &mut rng);
            let (spell, amount) = hits[h.min(hits.len() - 1)];
            let resval = element_resistance(spell, t);
            let power = apply_element_resistance(amount, resval);
            let effects = SpellEffects::from_record(spell);
            let mut state = CastState::new();
            for slot in effects
                .iter()
                .filter(|s| matches!(s.ability, Ability::DamageNoArmour | Ability::DamageArmour))
            {
                let cast = Cast { spell, caster: Some(&stats), path: CastPath::Single, power };
                let mut target = Target::Monster(MonsterTarget {
                    monster: &mut am,
                    known: &km,
                    magic_resistance: mr,
                    anti_magic: t.anti,
                    poison_immune: false,
                    element_resist_pct: resval,
                });
                let mut quiet = Script { draws: vec![], pos: 0 };
                if let Outcome::Applied { amount, .. } =
                    apply_effect(slot, &cast, &mut target, &mut state, &mut quiet)
                {
                    sum += i64::from(amount);
                }
            }
        }
        unresisted += sum as f64 / f64::from(rolls);
    }
    // the engine's one resist roll, over every value the host's genrdn(1, 100) can return
    let resisting = (1..=99)
        .filter(|&roll| {
            let mut rng = Script { draws: vec![roll], pos: 0 };
            monster_resists_single(p.spell, mr, t.anti, &mut rng)
        })
        .count();
    json!({ "spell": p.spell.number.0, "unresisted": unresisted, "resist_rolls": resisting })
}

#[allow(clippy::too_many_arguments)]
fn proc_case(
    w: &World,
    tag: &str,
    class: (u16, u8, i16),
    level: u16,
    stat: i64,
    bonus: i32,
    t: &Target1,
) -> Value {
    json!({
        "id": format!("proc/{tag}-c{:02}-l{level:03}-s{stat:03}-b{bonus:02}", class.0),
        "kind": "proc",
        "inputs": {
            "class": class.0, "magery": class.1, "magelvl": class.2, "level": level,
            "stat": stat, "bonus": bonus, "target": t.json(),
        },
        "v120": proc_answer(w, class, level, stat, bonus, t, &v120()),
        "neutral": Value::Null,
    })
}

pub fn family() -> Family {
    let w = world();
    let mut cases = Vec::new();

    for magery in 0..=5u8 {
        for magelvl in 0..=3i16 {
            for class in [3u16, 10] {
                cases.push(weaver_case(class, magery, magelvl));
            }
        }
    }

    for level in [0u16, 1, 5, 10, 20, 30, 40, 59, 60, 61, 90, 100, 200, 255] {
        cases.push(budget_case(level));
    }
    for b in [3i64, 30, 120, 180, 270, 765] {
        for t in [0i64, 1, 50, 200, 400, 1000, 5000, 20000] {
            cases.push(power_case(b, t));
        }
    }

    let el = |c, f, s, l, wa, po| [c, f, s, l, wa, po];
    let pick_targets: [(&str, Target1); 4] = [
        ("a", Target1::PLAIN),
        ("b", Target1 { immunity: 20, ..Target1::PLAIN }),
        ("c", Target1 { immunity: 40, non_living: true, ..Target1::PLAIN }),
        ("d", Target1 { animal: true, ..Target1::PLAIN }),
    ];
    for &class in &CLASSES {
        for level in [1u16, 5, 8, 10, 12, 15, 20, 25, 30, 35, 40, 45, 50, 60, 70, 100] {
            for (tag, t) in &pick_targets {
                cases.push(pick_case(&w, tag, class, level, t));
            }
        }
    }

    let proc_targets: [(&str, Target1); 6] = [
        ("a", Target1::PLAIN),
        ("b", Target1 { mr: 30, el: el(25, -10, 0, 50, 0, 0), ..Target1::PLAIN }),
        ("c", Target1 { mr: 60, anti: true, ..Target1::PLAIN }),
        ("d", Target1 { mr: 120, el: el(90, 0, 0, 0, 40, 10), ..Target1::PLAIN }),
        ("e", Target1 { mr: 200, anti: true, el: el(0, 0, 0, 20, 0, 0), ..Target1::PLAIN }),
        ("f", Target1 { mr: 52, el: el(0, 33, 17, 0, 0, 0), ..Target1::PLAIN }),
    ];
    for &class in &CLASSES {
        for level in [20u16, 40, 60, 100] {
            for (tag, t) in &proc_targets {
                for (stat, bonus) in [(40i64, 0i32), (100, 0), (160, 20)] {
                    cases.push(proc_case(&w, tag, class, level, stat, bonus, t));
                }
            }
        }
    }

    let (v, n) = (v120(), neutral());
    Family {
        name: "weave",
        about: "Quicksilver weave, the pick and the volley expectation. The spells of every case are the `book` \
                in balance_keys (number, req_level, energy per cast, min and max, [inc, levels] growth for each, \
                level cap, magery code, magery level, target, duration, element, resist mode, [ability, value] \
                slots). kind weaver: the proc chance by class magery code and level (null when the class does not \
                weave). kind budget: the proc budget at a level. kind power: the volley power percent from a \
                budget and twice the volley average. kind pick: the engine's pick for a class and level against a \
                target (immunity and what the spell spares), as spell number, twice the volley average, power \
                percent and the spell number of each hit. kind proc: the exact mean damage (`unresisted`, the \
                mean over every roll of every hit, run through the engine's own damage code, element and magic \
                resistance) of one proc for a character with the casting stat `stat` and spell damage bonus \
                `bonus` percent, and `resist_rolls`, how many of the 99 values of the resist roll stop it. The \
                level is the character's. v120 is the 1.20.0 balance; neutral exists only for weaver (chance 0).",
        keys: json!({
            "weave_pct": { "v120": v.weave_pct, "neutral": n.weave_pct },
            "weave_pct_magery1": { "v120": v.weave_pct_magery1, "neutral": n.weave_pct_magery1 },
            "weave_budget_per_level": { "v120": v.weave_budget_per_level, "neutral": v.weave_budget_per_level },
            "weave_power_min_pct": { "v120": v.weave_power_min_pct, "neutral": v.weave_power_min_pct },
            "weave_power_max_pct": { "v120": v.weave_power_max_pct, "neutral": v.weave_power_max_pct },
            "weave_stat_bonus_pct": { "v120": v.weave_stat_bonus_pct, "neutral": v.weave_stat_bonus_pct },
            "scalecap": { "v120": v.scalecap, "neutral": v.scalecap },
            "class_weave_pct": { "v120": "1.20.0 table", "neutral": "1.20.0 table" },
            "class_spell_pct": { "v120": "1.20.0 table", "neutral": "1.20.0 table" },
            "spell_stat_permille": { "v120": v.spell_stat_permille, "neutral": v.spell_stat_permille },
            "spell_stat_knee": { "v120": v.spell_stat_knee, "neutral": v.spell_stat_knee },
            "book": book().iter().map(spec_json).collect::<Vec<_>>(),
        }),
        cases,
    }
}
