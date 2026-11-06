//! The weave-round family (R6, second half): how often a blow lands for damage, and what a woven
//! blow is worth per landed blow.
//!
//! Quicksilver's balance tool has no closed per-round figure that matches touchstone's (it works at
//! the middle of each roll and builds whole kits), so the vectors are composed from the engine's own
//! pieces, as the rule itself composes them:
//! - `landed`: the engine's blow resolution (`resolve`) walked over every roll it asks for after a
//!   certain hit (the critical roll and the damage roll, each with its own range), weighted by the
//!   chance of the path: the share of landed blows that deal damage, i.e. the blows a weave can follow.
//!   The engine's hit roll is pinned to a hit because MME's hit chance is its own formula; what is
//!   compared is the share given a hit.
//! - `round`: that share times the engine's weave pieces from the `weave` family (the weaver's chance,
//!   the exact mean of an unresisted proc, the resist rolls that stop it), as the expected weave
//!   damage per swing that lands: `chance/100 * unresisted * (99 - resist_rolls)/99 * landed`.

use serde_json::{Value, json};
use silvermere_rules::combat::{Move, Outcome, resolve};
use silvermere_rules::rng::Rng;
use silvermere_rules::v120::weave::weaver;
use silvermere_types::config::Balance;

use crate::Family;
use crate::melee::{Hero, built_fighter, move_tag, neutral, v120};
use crate::weave::{CLASSES, Target1, book, engine_class, proc_answer, spec_json, world};

/// A walk over every outcome of a blow: answers the pinned prefix, and takes the first roll past it
/// as the one to branch on (the answer for any later roll is its lowest).
struct Walk {
    prefix: Vec<i32>,
    pos: usize,
    asked: Vec<(i32, i32)>,
}

impl Rng for Walk {
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        let at = self.pos;
        self.pos += 1;
        self.asked.push((lo, hi));
        self.prefix.get(at).copied().unwrap_or(lo)
    }

    fn long(&mut self, lo: i64, _hi: i64) -> i64 {
        lo
    }
}

/// The weight of the paths on which `run` says the blow dealt damage, with the paths of the
/// prefix's own rolls weighing `weight` between them.
fn walk(run: &dyn Fn(&mut Walk) -> bool, prefix: &mut Vec<i32>, weight: f64, out: &mut f64) {
    let mut w = Walk { prefix: prefix.clone(), pos: 0, asked: Vec::new() };
    let dealt = run(&mut w);
    if let Some(&(lo, hi)) = w.asked.get(prefix.len()) {
        let n = (hi - lo).max(1);
        for v in lo..lo + n {
            prefix.push(v);
            walk(run, prefix, weight / f64::from(n), out);
            prefix.pop();
        }
    } else if dealt {
        *out += weight;
    }
}

/// The hero's blow against a defender with `dr` damage reduction (MME's units, the engine's tenths)
/// and no dodge, the hit roll pinned to a hit, with the critical chance `crit` before the knee.
fn landed_share(h: &Hero, crit: i32, dr: i32, b: &Balance) -> f64 {
    let mut built = built_fighter(h, b);
    built.av = 200;
    built.crit = crit;
    let mut def = silvermere_rules::combat::Fighter::default();
    def.dr = dr * 10;
    let run = |rng: &mut Walk| {
        let mut atk = built.clone();
        matches!(resolve(&mut atk, &def, h.mv, rng, b).outcome, Outcome::Hit { .. })
    };
    let mut out = 0.0;
    walk(&run, &mut vec![1], 1.0, &mut out);
    out
}

fn hero(class: u16, level: u16, mv: Move, wmin: i16, wmax: i16, plus_max: i16) -> Hero {
    Hero { class, level, mv, wmin, wmax, plus_max, skill: 0, ma_dmg: 0 }
}

fn landed_case(h: Hero, crit: i32, dr: i32) -> Value {
    json!({
        "id": format!(
            "landed/c{:02}-l{:03}-{}-w{:03}x{:03}-p{:+03}-k{crit:02}-d{dr:03}",
            h.class, h.level, move_tag(h.mv), h.wmin, h.wmax, h.plus_max
        ),
        "kind": "landed",
        "inputs": {
            "class": h.class, "level": h.level, "move": move_tag(h.mv),
            "weapon_min": h.wmin, "weapon_max": h.wmax, "plus_max": h.plus_max,
            "crit": crit, "dr": dr,
        },
        "v120": landed_share(&h, crit, dr, &v120()),
        "neutral": landed_share(&h, crit, dr, &neutral()),
    })
}

/// The weave's expected damage per landed-and-damaging blow, from the engine's pieces.
fn per_landed(
    w: &crate::weave::World,
    class: (u16, u8, i16),
    level: u16,
    stat: i64,
    bonus: i32,
    t: &Target1,
    share: f64,
    b: &Balance,
) -> Value {
    let c = engine_class(class.0, class.1, class.2);
    let Some(wv) = weaver(&c, b) else {
        return Value::Null;
    };
    let p = proc_answer(w, class, level, stat, bonus, t, b);
    let Some(unresisted) = p["unresisted"].as_f64() else {
        return Value::Null;
    };
    let resist = p["resist_rolls"].as_i64().unwrap_or(0);
    let proc = unresisted * (99 - resist) as f64 / 99.0;
    json!({
        "landed": share,
        "chance_pct": wv.chance_pct,
        "unresisted": unresisted,
        "resist_rolls": resist,
        "per_landed": f64::from(wv.chance_pct) / 100.0 * proc,
        "per_blow": f64::from(wv.chance_pct) / 100.0 * proc * share,
    })
}

#[allow(clippy::too_many_arguments)]
fn round_case(
    w: &crate::weave::World,
    tag: &str,
    class: (u16, u8, i16),
    level: u16,
    stat: i64,
    bonus: i32,
    t: &Target1,
    blow: (i16, i16, i32, i32),
) -> Value {
    let (wmin, wmax, crit, dr) = blow;
    let h = hero(class.0, level, Move::Attack, wmin, wmax, 0);
    let col = |b: &Balance| {
        per_landed(w, class, level, stat, bonus, t, landed_share(&h, crit, dr, b), b)
    };
    json!({
        "id": format!(
            "round/{tag}-c{:02}-l{level:03}-s{stat:03}-b{bonus:02}-w{wmin:03}x{wmax:03}-k{crit:02}-d{dr:03}",
            class.0
        ),
        "kind": "round",
        "inputs": {
            "class": class.0, "magery": class.1, "magelvl": class.2, "level": level,
            "stat": stat, "bonus": bonus, "target": t.json(),
            "weapon_min": wmin, "weapon_max": wmax, "crit": crit, "dr": dr,
        },
        "v120": col(&v120()),
        "neutral": col(&crate::weave::neutral()),
    })
}

pub fn family() -> Family {
    let mut cases = Vec::new();
    let moves = [Move::Attack, Move::Bash, Move::Smash];
    for class in [1u16, 8, 12, 15] {
        for level in [3u16, 25, 50] {
            for mv in moves {
                for (wmin, wmax) in [(2i16, 12i16), (14, 40)] {
                    for plus_max in [0i16, 6] {
                        for crit in [0, 12, 39] {
                            for dr in [0, 6, 25, 90] {
                                cases.push(landed_case(
                                    hero(class, level, mv, wmin, wmax, plus_max),
                                    crit,
                                    dr,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    let w = world();
    let el = |c, f, s, l, wa, po| [c, f, s, l, wa, po];
    let targets: [(&str, Target1); 3] = [
        ("a", Target1::PLAIN),
        ("b", Target1 { mr: 60, anti: true, el: el(25, -10, 0, 50, 0, 0), ..Target1::PLAIN }),
        ("c", Target1 { mr: 120, el: el(90, 0, 0, 0, 40, 10), ..Target1::PLAIN }),
    ];
    for &class in &CLASSES {
        for level in [20u16, 40, 100] {
            for (tag, t) in &targets {
                for (stat, bonus) in [(100i64, 0i32), (160, 20)] {
                    for blow in [(14i16, 40i16, 0i32, 0i32), (14, 40, 25, 25), (2, 12, 39, 90)] {
                        cases.push(round_case(&w, tag, class, level, stat, bonus, t, blow));
                    }
                }
            }
        }
    }

    Family {
        name: "weave_round",
        about: "The weave in a round: how many landed blows deal damage, and what the weave adds to one. kind \
                landed: the engine's blow resolution for a player hero (class, level, a weapon of the given \
                damage, plus_max is Alter Max Damage; Strength 50) walked over every roll after a certain hit, \
                with critical chance `crit` (before the knee, which is why it stays under 40) against \
                damage reduction `dr` in MME's units: the share of landed blows that deal damage. neutral is \
                the engine with every class share at 100. kind round: for a weaving class (class number, magery \
                code and level as in the weave family; the `book` in balance_keys), a character of `level` with \
                casting stat `stat` and spell damage bonus `bonus` against `target`, hitting with a weapon of \
                the given damage: `landed` is the share above for that hero's normal blow, `chance_pct`, \
                `unresisted` and `resist_rolls` are the weave family's proc pieces, `per_landed` = chance/100 * \
                unresisted * (99 - resist_rolls)/99 (the expected weave damage of one landed blow that deals \
                damage) and `per_blow` = per_landed * landed. v120 is the 1.20.0 balance; neutral has the \
                weave chance 0 (null) and class shares 100.",
        keys: json!({
            "weave_pct": { "v120": v120().weave_pct, "neutral": 0 },
            "weave_pct_magery1": { "v120": v120().weave_pct_magery1, "neutral": 0 },
            "class_melee_pct": { "v120": "1.20.0 table", "neutral": "every class 100" },
            "book": book().iter().map(spec_json).collect::<Vec<_>>(),
        }),
        cases,
    }
}
