//! The melee family (R5 class melee share, R4 Thief opportunist).
//!
//! Three kinds of case, all from the engine's own functions:
//! - `share`: the class percentage applied to a damage range (`ClassMeleePct::scale`), the building block;
//! - `fighter`: a player's non-stab blow through the engine's fighter builder and blow resolution
//!   (scripted rolls for the lowest and the highest landed, non-critical damage), so the move's own
//!   multiplier and the order of the share are the engine's;
//! - `opening`: the opportunist bonus range and, over the whole roll space (every percentile roll
//!   crossed with every damage roll, scripted), the sum of the bonus damage the engine deals and the
//!   number of rolls, so touchstone's closed-form mean can be compared exactly.
//!
//! Three balances: 1.20.0, a stock-equivalent one (class shares 100, no opportunist; the stab keys at
//! their stock values so the bonus range is the one a neutral profile computes) and an off-grid one
//! with anchored class shares and a different opportunist.

use serde_json::{Value, json};
use silvermere_engine::clear_player;
use silvermere_engine::world::blank;
use silvermere_rules::combat::{
    AlignGroup, Blow, Fighter, Move, Outcome, PlayerAbilities, player_fighter, resolve,
};
use silvermere_rules::rng::Rng;
use silvermere_rules::v120::opportunist::{Opportunist, THIEF, bonus_range, opportunist};
use silvermere_types::config::{Anchors, Balance, ClassMeleePct, LevelPct};
use silvermere_types::ids::{ClassNo, ItemNo};

use crate::Family;

const OUT_OF_REACH: u32 = i32::MAX as u32;

fn v120() -> Balance {
    Balance::V120
}

fn neutral() -> Balance {
    Balance {
        class_melee_pct: ClassMeleePct::even(100),
        opportunist_pct: 0,
        backstab_stealth_div: 10,
        backstab_level_pct: 100,
        scalecap: OUT_OF_REACH,
        ..Balance::V120
    }
}

fn alt() -> Balance {
    Balance {
        class_melee_pct: ClassMeleePct {
            warrior: LevelPct::Points(Anchors::new([(10, 80), (30, 135), (50, 90)])),
            thief: LevelPct::Points(Anchors::new([(5, 120), (45, 33)])),
            mage: LevelPct::One(37),
            ranger: LevelPct::Points(Anchors::new([(1, 10), (2, 11), (3, 13), (90, 250)])),
            ..ClassMeleePct::even(101)
        },
        opportunist_pct: 35,
        opportunist_level_pct: 120,
        backstab_stealth_div: 5,
        scalecap: 40,
        ..Balance::V120
    }
}

fn share_case(class: u16, level: u16, min: i32, max: i32) -> Value {
    let col = |b: &Balance| {
        json!([
            b.class_melee_pct.scale(class, level, min),
            b.class_melee_pct.scale(class, level, max)
        ])
    };
    json!({
        "id": format!("share/c{class:02}-l{level:03}-{min:04}x{max:04}"),
        "kind": "share",
        "inputs": { "class": class, "level": level, "min": min, "max": max },
        "v120": col(&v120()),
        "alt": col(&alt()),
        "neutral": col(&neutral()),
    })
}

/// Scripted draws for `resolve` against a defender with no dodge: the hit roll, the crit roll (never
/// under a crit chance of 0), then the damage roll, clamped into the range the engine asks for so
/// 0 gives the lowest damage and a large number the highest.
struct Script(Vec<i32>);

impl Rng for Script {
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        let v = self.0.remove(0);
        v.clamp(lo, (hi - 1).max(lo))
    }
    fn long(&mut self, lo: i64, _hi: i64) -> i64 {
        lo
    }
}

#[derive(Clone, Copy)]
struct Hero {
    class: u16,
    level: u16,
    mv: Move,
    wmin: i16,
    wmax: i16,
    plus_max: i16,
    /// The martial-arts level of the move (Punch, Kick, Jumpkick abilities).
    skill: i32,
    ma_dmg: i32,
}

fn move_tag(mv: Move) -> &'static str {
    match mv {
        Move::Attack => "attack",
        Move::Bash => "bash",
        Move::Smash => "smash",
        Move::Punch => "punch",
        Move::Kick => "kick",
        Move::JumpKick => "jumpkick",
        _ => unreachable!(),
    }
}

/// The least and the most a landed, non-critical blow deals: the fighter the engine builds, through
/// its own blow resolution with a certain hit and no damage reduction.
fn blow_range(h: &Hero, b: &Balance) -> (i32, i32) {
    let mut c = clear_player();
    c.class = ClassNo(h.class);
    c.level = h.level;
    c.str = 50;
    c.agi = 60;
    c.maxenergy = 1000;
    c.dmgbonus = h.plus_max;
    let mut class = blank::clear_class(ClassNo(h.class));
    class.combat = 3;
    let mut w = blank::clear_item(ItemNo(1));
    w.mindmg = h.wmin;
    w.maxdmg = h.wmax;
    w.speed = 1200;
    let abil = PlayerAbilities {
        punch: h.skill,
        kick: h.skill,
        jumpkick: h.skill,
        punch_dmg: h.ma_dmg,
        kick_dmg: h.ma_dmg,
        jumpkick_dmg: h.ma_dmg,
        ..PlayerAbilities::default()
    };
    let built = player_fighter(&c, &class, Some(&w), &[], abil, Blow::new(h.mv, AlignGroup::Evil), b);
    let roll = |high: bool| {
        let mut atk = Fighter { av: 200, crit: 0, ..built.clone() };
        let def = Fighter::default();
        let r = resolve(
            &mut atk,
            &def,
            h.mv,
            &mut Script(vec![1, 99, if high { i32::MAX } else { 0 }]),
            b,
        );
        match r.outcome {
            Outcome::Hit { damage, .. } => damage,
            // with no damage reduction a blow that deals nothing had a range end of 0
            Outcome::NoDamage => 0,
            other => panic!("expected a hit, got {other:?}"),
        }
    };
    (roll(false), roll(true))
}

fn fighter_case(h: Hero) -> Value {
    let col = |b: &Balance| {
        let (lo, hi) = blow_range(&h, b);
        json!([lo, hi])
    };
    json!({
        "id": format!(
            "fighter/c{:02}-l{:03}-{}-w{:03}x{:03}-p{:+03}-s{}-m{:+03}",
            h.class, h.level, move_tag(h.mv), h.wmin, h.wmax, h.plus_max, h.skill, h.ma_dmg
        ),
        "kind": "fighter",
        "inputs": {
            "class": h.class, "level": h.level, "move": move_tag(h.mv),
            "weapon_min": h.wmin, "weapon_max": h.wmax, "plus_max": h.plus_max,
            "skill": h.skill, "ma_dmg": h.ma_dmg,
        },
        "v120": col(&v120()),
        "alt": col(&alt()),
        "neutral": col(&neutral()),
    })
}

/// Scripted draws for the opportunist: the percentile roll, then the damage roll.
struct Opening {
    pct: i32,
    dmg: i32,
    calls: u32,
}

impl Rng for Opening {
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        self.calls += 1;
        let v = if self.calls == 1 { self.pct } else { self.dmg };
        assert!(lo <= v && (v < hi || hi <= lo), "roll {v} outside [{lo}, {hi})");
        v
    }
    fn long(&mut self, lo: i64, _hi: i64) -> i64 {
        lo
    }
}

struct Thief {
    class: u16,
    level: i32,
    stealth: i32,
    bs_min: i32,
    bs_max: i32,
    dr: i32,
}

/// `[range low, range high, sum of bonus damage over every roll, number of rolls]`.
fn opening_column(t: &Thief, b: &Balance) -> Value {
    let op = Opportunist {
        class: ClassNo(t.class),
        level: t.level,
        stealth: t.stealth,
        backstab_min: t.bs_min,
        backstab_max: t.bs_max,
    };
    let (lo, hi) = bonus_range(&op, b);
    let dmg_rolls = i64::from(hi - lo + 1);
    let mut sum = 0i64;
    for pct in 0..100 {
        for dmg in 0..dmg_rolls as i32 {
            let mut rng = Opening { pct, dmg, calls: 0 };
            // damage reduction is kept in tenths
            if let Some(d) = opportunist(&op, true, t.dr * 10, b, &mut rng) {
                sum += i64::from(d);
            }
        }
    }
    json!([lo, hi, sum, 100 * dmg_rolls])
}

fn opening_case(t: Thief) -> Value {
    json!({
        "id": format!(
            "opening/c{:02}-l{:03}-s{:03}-b{:+03}{:+03}-d{:03}",
            t.class, t.level, t.stealth, t.bs_min, t.bs_max, t.dr
        ),
        "kind": "opening",
        "inputs": {
            "class": t.class, "level": t.level, "stealth": t.stealth,
            "bs_min": t.bs_min, "bs_max": t.bs_max, "dr": t.dr,
        },
        "v120": opening_column(&t, &v120()),
        "alt": opening_column(&t, &alt()),
        "neutral": opening_column(&t, &neutral()),
    })
}

pub fn family() -> Family {
    let mut cases = Vec::new();
    let ranges = [(0, 0), (1, 4), (2, 12), (7, 19), (10, 30), (45, 120), (100, 999), (5000, 9000)];
    for class in 0u16..=16 {
        for level in [1u16, 10, 29, 30, 35, 40, 41, 45, 50, 60, 80, 255] {
            for (min, max) in ranges {
                cases.push(share_case(class, level, min, max));
            }
        }
    }
    let moves = [Move::Attack, Move::Bash, Move::Smash, Move::Punch, Move::Kick, Move::JumpKick];
    for class in [1u16, 2, 8, 9, 12, 15] {
        for level in [3u16, 25, 40, 50, 70] {
            for mv in moves {
                let martial = matches!(mv, Move::Punch | Move::Kick | Move::JumpKick);
                for (wmin, wmax) in [(2i16, 12i16), (14, 40)] {
                    if martial && (wmin, wmax) != (2, 12) {
                        continue;
                    }
                    for plus_max in [0i16, 6] {
                        let skills: &[(i32, i32)] = if martial { &[(2, 0), (5, 4)] } else { &[(0, 0)] };
                        for &(skill, ma_dmg) in skills {
                            cases.push(fighter_case(Hero {
                                class, level, mv, wmin, wmax, plus_max, skill, ma_dmg,
                            }));
                        }
                    }
                }
            }
        }
    }
    for class in [8u16, 1, 12] {
        let thief = i32::from(class) == i32::from(THIEF.0);
        let levels: &[i32] = if thief { &[1, 5, 20, 40, 59, 60, 61, 90] } else { &[5, 60] };
        let stealths: &[i32] = if thief { &[0, 10, 37, 100, 200] } else { &[37] };
        let drs: &[i32] = if thief { &[0, 7, 40, 200] } else { &[0, 7] };
        for &level in levels {
            for &stealth in stealths {
                for (bs_min, bs_max) in [(0, 0), (5, 10), (-3, 4), (20, 8)] {
                    for &dr in drs {
                        cases.push(opening_case(Thief { class, level, stealth, bs_min, bs_max, dr }));
                    }
                }
            }
        }
    }
    let (v, n, a) = (v120(), neutral(), alt());
    Family {
        name: "melee",
        about: "Quicksilver's class melee share and Thief opportunist. kind share: [min, max] after the engine's \
                class_melee_pct for a class (1 Warrior to 15 Mystic; others unscaled) at a level. kind fighter: \
                [lowest, highest] damage of a landed non-critical blow with no damage reduction, through the \
                engine's fighter builder and blow resolution for a player (Strength 50, a weapon of the given \
                damage; martial-arts moves use skill as the move's level and ma_dmg as its damage ability; \
                plus_max is Alter Max Damage). kind opening: [bonus low, bonus high, sum of the bonus damage \
                over every percentile roll and every damage roll, number of rolls]; dr is the target's damage \
                reduction in MME's units; mean = sum / rolls. v120 is the 1.20.0 balance; neutral has class \
                shares 100, no opportunist and stock stab keys; alt is an off-grid balance (see balance_keys; \
                class_melee_pct under alt: Warrior anchors 10:80 30:135 50:90, Thief anchors 5:120 45:33, Mage \
                37, Ranger anchors 1:10 2:11 3:13 90:250, every other class 101).",
        keys: json!({
            "opportunist_pct": { "v120": v.opportunist_pct, "neutral": n.opportunist_pct, "alt": a.opportunist_pct },
            "opportunist_level_pct": { "v120": v.opportunist_level_pct, "neutral": n.opportunist_level_pct, "alt": a.opportunist_level_pct },
            "backstab_stealth_div": { "v120": v.backstab_stealth_div, "neutral": n.backstab_stealth_div, "alt": a.backstab_stealth_div },
            "scalecap": { "v120": v.scalecap, "neutral": n.scalecap, "alt": a.scalecap },
        }),
        cases,
    }
}
