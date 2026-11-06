//! The backstab family (R1 damage, R3 crit range).
//!
//! `bs` cases build the engine's player fighter for a stab (no armour, Strength 50 so the weapon's
//! damage enters unchanged) and report its damage range. `crit` cases run the engine's blow
//! resolution with scripted rolls (hit, crit, then the lowest and the highest damage roll) and
//! report the range a critical stab can roll; `bs` cases also carry that range for the stab they
//! built (`v120_crit`, `alt_crit`). Three balances: 1.20.0, a stock-equivalent one
//! (stealth divisor 10, level percentage 100, no level cap; a stock stab never crits so `crit`
//! has no neutral column) and an off-grid one that proves the numbers are read, not assumed.

use serde_json::{Value, json};
use silvermere_engine::clear_player;
use silvermere_engine::world::blank;
use silvermere_rules::combat::{
    AlignGroup, AttackResult, Blow, Fighter, Move, Outcome, PlayerAbilities, player_fighter,
    resolve,
};
use silvermere_rules::rng::Rng;
use silvermere_types::config::Balance;
use silvermere_types::flags::PlayerFlags;
use silvermere_types::ids::{ClassNo, ItemNo};

use crate::Family;

const OUT_OF_REACH: u32 = i32::MAX as u32;

fn v120() -> Balance {
    Balance::V120
}

fn neutral() -> Balance {
    Balance {
        backstab_stealth_div: 10,
        backstab_level_pct: 100,
        scalecap: OUT_OF_REACH,
        ..Balance::V120
    }
}

fn alt() -> Balance {
    Balance {
        backstab_stealth_div: 5,
        backstab_level_pct: 150,
        backstab_crit_max_pct: 120,
        scalecap: 40,
        ..Balance::V120
    }
}

struct Stab {
    level: u16,
    stealth: i16,
    wmin: i16,
    wmax: i16,
    bs_min: i32,
    bs_max: i32,
    class_stealth: bool,
}

fn stab_range(s: &Stab, b: &Balance) -> (i32, i32) {
    let mut c = clear_player();
    c.level = s.level;
    c.stealth = s.stealth;
    c.str = 50;
    c.maxenergy = 1000;
    c.flags = if s.class_stealth {
        PlayerFlags::CLASSSTEALTH
    } else {
        PlayerFlags::RACESTEALTH
    };
    let mut class = blank::clear_class(ClassNo(1));
    class.combat = 3;
    let mut w = blank::clear_item(ItemNo(1));
    w.mindmg = s.wmin;
    w.maxdmg = s.wmax;
    w.speed = 1200;
    let abil = PlayerAbilities {
        backstab_min: s.bs_min,
        backstab_max: s.bs_max,
        ..PlayerAbilities::default()
    };
    let f = player_fighter(
        &c,
        &class,
        Some(&w),
        &[],
        abil,
        Blow::new(Move::Backstab, AlignGroup::Evil),
        b,
    );
    (f.mindmg, f.maxdmg)
}

fn bs_case(s: Stab) -> Value {
    let col = |b: &Balance| {
        let (lo, hi) = stab_range(&s, b);
        json!([lo, hi])
    };
    json!({
        "id": format!(
            "bs/l{:03}-s{:03}-w{:03}x{:03}-b{:+03}{:+03}-{}",
            s.level, s.stealth, s.wmin, s.wmax, s.bs_min, s.bs_max,
            if s.class_stealth { "class" } else { "race" }
        ),
        "kind": "bs",
        "inputs": {
            "level": s.level, "stealth": s.stealth,
            "weapon_min": s.wmin, "weapon_max": s.wmax,
            "bs_min": s.bs_min, "bs_max": s.bs_max,
            "class_stealth": s.class_stealth,
        },
        "v120": col(&v120()),
        "alt": col(&alt()),
        "neutral": col(&neutral()),
        "v120_crit": crit_of(&s, &v120()),
        "alt_crit": crit_of(&s, &alt()),
    })
}

/// The range a critical stab rolls: the engine's stab range run through blow resolution.
fn crit_of(s: &Stab, b: &Balance) -> Value {
    let (lo, hi) = stab_range(s, b);
    json!([crit_roll(lo, hi, false, b), crit_roll(lo, hi, true, b)])
}

/// Scripted draws, in the order `resolve` makes them for a landed critical blow against a
/// defender with no dodge: hit roll, crit roll, damage roll.
struct Script(Vec<i32>);

impl Rng for Script {
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        let v = self.0.remove(0);
        assert!(lo <= v && (v < hi || hi <= lo), "roll {v} outside [{lo}, {hi})");
        v
    }
    fn long(&mut self, lo: i64, _hi: i64) -> i64 {
        lo
    }
}

/// The damage of a critical stab whose damage roll is the lowest (`high` false) or highest.
fn crit_roll(min: i32, max: i32, high: bool, b: &Balance) -> i32 {
    let mut atk = Fighter { av: 200, crit: 50, mindmg: min, maxdmg: max, ..Fighter::default() };
    let def = Fighter::default();
    // The critical range the engine builds is not visible before the roll, so draw the lowest
    // roll first and, for the high end, size the roll to the range that pass reveals.
    let span = {
        let mut probe = atk.clone();
        let _ = resolve(&mut probe, &def, Move::Backstab, &mut Script(vec![1, 0, 0]), b);
        probe.maxdmg - probe.mindmg
    };
    let roll = if high { span } else { 0 };
    let r: AttackResult =
        resolve(&mut atk, &def, Move::Backstab, &mut Script(vec![1, 0, roll]), b);
    match r.outcome {
        Outcome::Hit { damage, critical: true } => damage,
        other => panic!("expected a critical hit, got {other:?}"),
    }
}

fn crit_case(min: i32, max: i32) -> Value {
    let col = |b: &Balance| json!([crit_roll(min, max, false, b), crit_roll(min, max, true, b)]);
    json!({
        "id": format!("crit/{min:04}-{max:04}"),
        "kind": "crit",
        "inputs": { "bs_min": min, "bs_max": max },
        "v120": col(&v120()),
        "alt": col(&alt()),
        "neutral": null,
    })
}

pub fn family() -> Family {
    let mut cases = Vec::new();
    let weapons = [(1i16, 4i16), (2, 12), (10, 30), (50, 120)];
    let mods = [(0, 0), (5, 9), (-3, -3)];
    for level in [1u16, 5, 20, 40, 59, 60, 61, 80, 100, 255] {
        for stealth in [0i16, 10, 37, 60, 100, 200, 255] {
            for (wmin, wmax) in weapons {
                for (bs_min, bs_max) in mods {
                    for class_stealth in [true, false] {
                        cases.push(bs_case(Stab {
                            level, stealth, wmin, wmax, bs_min, bs_max, class_stealth,
                        }));
                    }
                }
            }
        }
    }
    for (min, max) in [
        (1, 1), (1, 2), (3, 9), (10, 10), (14, 80), (60, 120), (100, 400), (250, 900), (999, 3000),
    ] {
        cases.push(crit_case(min, max));
    }
    let (v, n, a) = (v120(), neutral(), alt());
    Family {
        name: "backstab",
        about: "Quicksilver's backstab rules. kind bs: the engine's stab damage range [min, max] before any \
                critical, from a player with Strength 50 and the given weapon; class_stealth false means race \
                stealth only (the 75% cut). kind crit: the range [lowest, highest] a critical stab rolls, given \
                the stab's [bs_min, bs_max] (no damage reduction); a stock stab never crits so neutral is null. \
                v120 is the 1.20.0 balance, neutral has stock-equivalent stealth divisor, level percentage and \
                no level cap, alt is an off-grid balance (see balance_keys).",
        keys: json!({
            "backstab_stealth_div": { "v120": v.backstab_stealth_div, "neutral": n.backstab_stealth_div, "alt": a.backstab_stealth_div },
            "backstab_level_pct": { "v120": v.backstab_level_pct, "neutral": n.backstab_level_pct, "alt": a.backstab_level_pct },
            "backstab_crit_max_pct": { "v120": v.backstab_crit_max_pct, "alt": a.backstab_crit_max_pct },
            "scalecap": { "v120": v.scalecap, "neutral": n.scalecap, "alt": a.scalecap },
        }),
        cases,
    }
}
