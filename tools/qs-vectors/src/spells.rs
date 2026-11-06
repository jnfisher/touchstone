//! The spells family (R7 casting-stat spell damage, R8 class spell share, R9 area resist).
//!
//! Each case runs the engine twice: under the 1.20.0 balance and under a balance that switches
//! the three rules off (no stat bonus, every class share 100, no resist cut). The area cases
//! enumerate the engine's resist roll over every value the host's `genrdn(1, 100)` can return,
//! under a scripted source, so the expectation is exact.

use serde_json::{Value, json};
use silvermere_rules::regen::class_stat;
use silvermere_rules::rng::Rng;
use silvermere_rules::spells::resist::{apply_area_resist_cut, monster_resists_area};
use silvermere_rules::spells::scale_spell_damage;
use silvermere_types::config::{Balance, ClassSpellPct};
use silvermere_types::enums::Magery;

use crate::Family;

/// Beyond any value touchstone's stock formulas take (they work in 32 bits); matches
/// `KNEE_OUT_OF_REACH` in touchstone-rules.
const KNEE_OUT_OF_REACH: u32 = i32::MAX as u32;

fn v120() -> Balance {
    Balance::V120
}

/// The three rules off. The other keys keep their 1.20.0 values: none of these functions reads one.
fn neutral() -> Balance {
    Balance {
        spell_stat_permille: 0,
        spell_stat_knee: KNEE_OUT_OF_REACH,
        class_spell_pct: ClassSpellPct::even(100),
        aoe_resist_cut_max_pct: 0,
        aoe_resist_cut_div: 1,
        ..Balance::V120
    }
}

/// A source that answers one scripted roll and records the range it was asked for.
struct Scripted {
    roll: i32,
    asked: Vec<(i32, i32)>,
}

impl Rng for Scripted {
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        self.asked.push((lo, hi));
        self.roll.clamp(lo, hi - 1)
    }

    fn long(&mut self, lo: i64, hi: i64) -> i64 {
        i64::from(self.range(lo as i32, hi as i32))
    }
}

/// The engine's player spell damage scaling, in its order: the casting-stat bonus, then the
/// class share (`Cast::stat_scaled` in the apply module, which is private).
fn scaled(amount: i32, stat: i64, class: u16, level: u16, b: &Balance) -> i32 {
    b.class_spell_pct
        .scale(class, level, scale_spell_damage(amount, stat, b))
}

fn stat_case(amount: i32, stat: i64) -> Value {
    json!({
        "id": format!("stat/a{amount:05}-s{stat:03}"),
        "kind": "stat",
        "inputs": { "amount": amount, "stat": stat },
        "v120": scale_spell_damage(amount, stat, &v120()),
        "neutral": scale_spell_damage(amount, stat, &neutral()),
    })
}

fn class_case(class: u16, level: u16, amount: i32) -> Value {
    let (v, n) = (v120(), neutral());
    json!({
        "id": format!("class/c{class:02}-l{level:03}-a{amount:05}"),
        "kind": "class",
        "inputs": { "class": class, "level": level, "amount": amount },
        "v120": v.class_spell_pct.scale(class, level, amount),
        "neutral": n.class_spell_pct.scale(class, level, amount),
    })
}

fn damage_case(class: u16, level: u16, stat: i64, amount: i32) -> Value {
    json!({
        "id": format!("damage/c{class:02}-l{level:03}-s{stat:03}-a{amount:05}"),
        "kind": "damage",
        "inputs": { "class": class, "level": level, "stat": stat, "amount": amount },
        "v120": scaled(amount, stat, class, level, &v120()),
        "neutral": scaled(amount, stat, class, level, &neutral()),
    })
}

fn casting_case(magery: Magery, code: i32, int: u16, wil: u16, cha: u16) -> Value {
    let stat = class_stat(magery, int, wil, cha);
    json!({
        "id": format!("casting/m{code}-i{int:03}-w{wil:03}-c{cha:03}"),
        "kind": "casting",
        "inputs": { "magery": code, "int": int, "wil": wil, "cha": cha },
        "v120": stat,
        "neutral": stat,
    })
}

/// What one monster's area share comes to over every resist roll: the sum of the integer shares
/// (the mean is the sum over `rolls`), how many rolls resist, and the cut each resisting roll takes.
fn area_column(mr: i32, share: i32, b: &Balance) -> Value {
    let mut sum = 0i64;
    let mut resisting = 0u32;
    let mut cut = 0i32;
    let mut rolls = 0u32;
    // genrdn(1, 100) returns 1..=99; the engine draws exactly one roll per monster.
    for roll in 1..=99 {
        let mut rng = Scripted {
            roll,
            asked: Vec::new(),
        };
        let c = monster_resists_area(mr, &mut rng, b);
        assert_eq!(rng.asked, [(1, 100)], "one roll of genrdn(1, 100) per monster");
        rolls += 1;
        let kept = apply_area_resist_cut(share, c);
        sum += i64::from(kept);
        // Whether the roll resists at all is told by the same engine call under a balance whose
        // cut is never zero (MR over 0), because a resist with a zero cut looks like a miss.
        let probe = Balance {
            aoe_resist_cut_max_pct: 100,
            aoe_resist_cut_div: 1,
            ..Balance::V120
        };
        let mut again = Scripted {
            roll,
            asked: Vec::new(),
        };
        if monster_resists_area(mr, &mut again, &probe) > 0 {
            resisting += 1;
        }
        cut = cut.max(c);
    }
    json!({ "rolls": rolls, "sum": sum, "resisting_rolls": resisting, "cut_pct": cut })
}

fn area_case(mr: i32, share: i32) -> Value {
    json!({
        "id": format!("area/mr{mr:04}-a{share:05}"),
        "kind": "area",
        "inputs": { "mr": mr, "share": share },
        "v120": area_column(mr, share, &v120()),
        "neutral": area_column(mr, share, &neutral()),
    })
}

pub fn family() -> Family {
    let mut cases = Vec::new();

    let amounts = [1, 7, 10, 50, 100, 333, 1000, 2500, 9999];
    let stats: [i64; 22] = [
        0, 1, 30, 49, 50, 51, 60, 75, 90, 99, 100, 101, 102, 103, 110, 130, 150, 170, 200, 230, 255, 400,
    ];
    for &a in &amounts {
        for &s in &stats {
            cases.push(stat_case(a, s));
        }
    }

    // class numbers 1..=15 are the stock classes; 0 and 16 are outside the table (whole damage)
    let levels = [0u16, 1, 20, 30, 39, 40, 41, 50, 59, 60, 61, 100, 255];
    for class in 0..=16u16 {
        for &l in &levels {
            for a in [1, 7, 100, 999, 1234] {
                cases.push(class_case(class, l, a));
            }
        }
    }

    for class in [5u16, 12, 1, 10, 13, 9] {
        for level in [10u16, 40, 50, 60, 90] {
            for stat in [40i64, 50, 90, 100, 130, 200] {
                for amount in [7, 100, 1000] {
                    cases.push(damage_case(class, level, stat, amount));
                }
            }
        }
    }

    let stat_triples = [
        (0u16, 0u16, 0u16),
        (50, 50, 50),
        (90, 60, 40),
        (60, 91, 33),
        (100, 100, 100),
        (130, 71, 150),
        (255, 255, 255),
        (45, 46, 47),
        (101, 40, 77),
    ];
    for (code, m) in [
        (0, Magery::None),
        (1, Magery::Mage),
        (2, Magery::Priest),
        (3, Magery::Druid),
        (4, Magery::Bard),
        (5, Magery::Kai),
    ] {
        for (i, w, c) in stat_triples {
            cases.push(casting_case(m, code, i, w, c));
        }
    }

    for mr in [
        0, 1, 2, 3, 4, 8, 10, 20, 30, 40, 50, 60, 80, 100, 120, 150, 196, 197, 198, 200, 250, 300, 400,
    ] {
        for share in [10, 100, 333, 1000, 2500] {
            cases.push(area_case(mr, share));
        }
    }

    cases.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    cases.dedup_by(|a, b| a["id"] == b["id"]);
    let (v, n) = (v120(), neutral());
    Family {
        name: "spells",
        about: "Quicksilver spell damage, one engine call per column. kind stat: the casting-stat bonus on an amount \
                (per mille added per stat point over 50, the stat counting a third above the knee). kind class: the \
                class share at a level, truncating. kind damage: both, stat first then class. kind casting: the class's \
                casting stat from Int, Wil and Cha for a magery code (0 none, 1 mage, 2 priest, 3 druid, 4 bard, 5 kai). \
                kind area: one monster's share of an area damage spell over every value of the engine's resist roll \
                (genrdn(1, 100): 1 to 99): `rolls` is 99, `sum` is the sum of the integer shares kept (the mean is \
                sum / rolls), `resisting_rolls` the rolls that resist with a nonzero cut, `cut_pct` that cut. v120 is \
                the 1.20.0 balance; neutral has the stat bonus, the class shares and the area cut off.",
        keys: json!({
            "spell_stat_permille": { "v120": v.spell_stat_permille, "neutral": n.spell_stat_permille },
            "spell_stat_knee": { "v120": v.spell_stat_knee, "neutral": n.spell_stat_knee },
            "class_spell_pct": { "v120": "1.20.0 table", "neutral": "100 for every class" },
            "aoe_resist_cut_max_pct": { "v120": v.aoe_resist_cut_max_pct, "neutral": n.aoe_resist_cut_max_pct },
            "aoe_resist_cut_div": { "v120": v.aoe_resist_cut_div, "neutral": n.aoe_resist_cut_div },
        }),
        cases,
    }
}
