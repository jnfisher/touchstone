//! The regeneration family (R10: rest and meditate restore mana).
//!
//! Four kinds of case, all from the engine's regen functions:
//! - `standing`: the mana a slow tick (30 s) gives a standing player;
//! - `hp`: the hit points one rest tick gives (meditating heals the same);
//! - `tick`: 30 consecutive rest or meditate payments, carry included;
//! - `recovery`: a second-by-second run from no mana to a target, as the fast cycle plays it, in the
//!   5-second rounds MME counts.
//!
//! The `neutral` column runs the same code with both multiples at 1, i.e. no extra mana from resting
//! or meditating. That is what stock MME answers when the caller passes no meditation rate, which is
//! how the stock screening works.

use serde_json::{Value, json};
use silvermere_rules::regen::{
    SLOW_SECS, class_stat, hp_regen_base, mana_regen, meditate_mana_regen, meditate_tick,
    rest_hp_regen, rest_mana_regen, rest_tick,
};
use silvermere_types::config::Balance;
use silvermere_types::enums::Magery;

use crate::Family;

fn v120() -> Balance {
    Balance::V120
}

/// Resting and meditating add nothing to the standing rate.
fn neutral() -> Balance {
    Balance {
        rest_mana_mult: 1,
        meditate_mana_mult: 1,
        ..Balance::V120
    }
}

/// MME's `enmMagicEnum` numbering, which touchstone's inputs use.
const MAGERIES: [(i32, Magery); 6] = [
    (0, Magery::None),
    (1, Magery::Mage),
    (2, Magery::Priest),
    (3, Magery::Druid),
    (4, Magery::Bard),
    (5, Magery::Kai),
];

fn magery_of(code: i32) -> Magery {
    MAGERIES.iter().find(|(c, _)| *c == code).expect("magic type").1
}

#[derive(Clone, Copy)]
struct Caster {
    magic_type: i32,
    int: u16,
    wil: u16,
    cha: u16,
    level: u16,
    magelvl: i16,
    manarate: i16,
}

impl Caster {
    fn inputs(&self) -> Value {
        json!({
            "magic_type": self.magic_type, "int": self.int, "wil": self.wil, "cha": self.cha,
            "level": self.level, "magic_level": self.magelvl, "mp_regen": self.manarate,
        })
    }

    fn tag(&self) -> String {
        format!(
            "m{}-i{:03}-w{:03}-c{:03}-l{:03}-ml{}-r{:+04}",
            self.magic_type, self.int, self.wil, self.cha, self.level, self.magelvl, self.manarate
        )
    }

    fn standing(&self, b: &Balance) -> i32 {
        let m = magery_of(self.magic_type);
        let stat = class_stat(m, self.int, self.wil, self.cha);
        mana_regen(m, stat, self.level, self.magelvl, self.manarate, b)
    }
}

fn standing_case(c: Caster) -> Value {
    json!({
        "id": format!("standing/{}", c.tag()),
        "kind": "standing",
        "inputs": c.inputs(),
        "v120": c.standing(&v120()),
        "neutral": c.standing(&neutral()),
    })
}

fn hp_case(level: u16, hea: u16, rate: i16) -> Value {
    let (v, n) = (v120(), neutral());
    json!({
        "id": format!("hp/l{level:03}-h{hea:03}-r{rate:+04}"),
        "kind": "hp",
        "inputs": { "level": level, "health": hea, "hp_regen": rate },
        "v120": rest_hp_regen(level, hea, rate, &v),
        "neutral": rest_hp_regen(level, hea, rate, &n),
        // the base before the rest tick triples it, as a cross-check of the hit point rule
        "base": hp_regen_base(level, hea),
    })
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Rest,
    Meditate,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Mode::Rest => "rest",
            Mode::Meditate => "meditate",
        }
    }
}

/// 30 consecutive payments of the rest or meditate tick for a standing rate, carry starting at 0.
fn tick_run(mode: Mode, normal: i32, b: &Balance) -> Vec<i32> {
    let mut carry = 0u8;
    (0..30)
        .map(|_| match mode {
            Mode::Rest => rest_mana_regen(normal, &mut carry, b),
            Mode::Meditate => meditate_mana_regen(normal, &mut carry, b),
        })
        .collect()
}

fn tick_case(mode: Mode, normal: i32) -> Value {
    json!({
        "id": format!("tick/{}-n{normal:04}", mode.name()),
        "kind": "tick",
        "inputs": { "mode": mode.name(), "standing": normal },
        "v120": tick_run(mode, normal, &v120()),
        "neutral": tick_run(mode, normal, &neutral()),
    })
}

/// Rounds of 5 seconds to go from no mana to `target`, playing the fast cycle one second at a
/// time: the slow tick every 30 s and the rest or meditate tick on its counter. 999 when it is not
/// reached in 999 rounds, 0 for a target of nothing.
fn recovery_rounds(mode: Mode, normal: i32, target: i32, b: &Balance) -> i32 {
    if target <= 0 {
        return 0;
    }
    let (mut mana, mut counter, mut carry) = (0i64, 0u8, 0u8);
    for sec in 1..=999 * 5 {
        if sec % u32::from(SLOW_SECS) == 0 {
            mana += i64::from(normal);
        }
        match mode {
            Mode::Rest => {
                if rest_tick(&mut counter) {
                    mana += i64::from(rest_mana_regen(normal, &mut carry, b));
                }
            }
            Mode::Meditate => {
                if meditate_tick(&mut counter) {
                    mana += i64::from(meditate_mana_regen(normal, &mut carry, b));
                }
            }
        }
        if mana >= i64::from(target) {
            return i32::try_from(sec.div_ceil(5)).unwrap();
        }
    }
    999
}

fn recovery_case(c: Caster, mode: Mode, full: bool) -> Value {
    let max_mana = i32::from(c.magelvl) * i32::from(c.level) * 2 + 6;
    let target = if full { max_mana } else { max_mana / 2 };
    let (v, n) = (v120(), neutral());
    json!({
        "id": format!("recovery/{}-{}-{}", mode.name(), if full { "full" } else { "half" }, c.tag()),
        "kind": "recovery",
        "inputs": {
            "mode": mode.name(), "caster": c.inputs(), "standing": c.standing(&v), "target": target,
        },
        "v120": recovery_rounds(mode, c.standing(&v), target, &v),
        "neutral": recovery_rounds(mode, c.standing(&n), target, &n),
    })
}

pub fn family() -> Family {
    let mut cases = Vec::new();

    let stat_sets: [(u16, u16, u16); 4] = [(30, 30, 30), (60, 40, 45), (90, 110, 50), (255, 255, 255)];
    for (magic_type, _) in MAGERIES {
        for level in [1u16, 10, 30, 60, 100] {
            for (int, wil, cha) in stat_sets {
                for magelvl in [1i16, 3] {
                    for manarate in [-50i16, 0, 50, 100] {
                        cases.push(standing_case(Caster {
                            magic_type, int, wil, cha, level, magelvl, manarate,
                        }));
                    }
                }
            }
        }
    }

    for level in [1u16, 10, 30, 60, 100] {
        for hea in [0u16, 30, 50, 90, 100, 150, 255] {
            for rate in [-100i16, -50, 0, 15, 50, 100] {
                cases.push(hp_case(level, hea, rate));
            }
        }
    }

    for mode in [Mode::Rest, Mode::Meditate] {
        for normal in [0, 1, 2, 3, 4, 5, 7, 10, 13, 25, 40, 77, 120] {
            cases.push(tick_case(mode, normal));
        }
    }

    // One caster per magic school (Kai and the unmagical included), a spread of levels and rates.
    let recovery_stats: [(u16, u16, u16); 2] = [(60, 40, 45), (100, 120, 90)];
    for (magic_type, _) in MAGERIES {
        for level in [1u16, 5, 12, 30, 60] {
            for (int, wil, cha) in recovery_stats {
                for magelvl in [1i16, 3] {
                    for manarate in [0i16, 50] {
                        let c = Caster { magic_type, int, wil, cha, level, magelvl, manarate };
                        for mode in [Mode::Rest, Mode::Meditate] {
                            for full in [true, false] {
                                cases.push(recovery_case(c, mode, full));
                            }
                        }
                    }
                }
            }
        }
    }

    let (v, n) = (v120(), neutral());
    Family {
        name: "regen",
        about: "Quicksilver's rest and meditate regeneration, one engine run per column. kind standing: the mana a \
                30-second tick gives a standing player (unchanged by the 1.20 rules; the column is the same in both). \
                kind hp: the hit points one rest tick gives, which meditating gives too; base is the value before it \
                is tripled. kind tick: the mana of 30 consecutive rest (21 s) or meditate (15 s) ticks for a standing \
                rate, with the remainder carried. kind recovery: 5-second rounds to go from no mana to a target \
                (999 when not reached), resting or meditating, standing ticks included; inputs.standing is the \
                standing rate of the caster. v120 is the 1.20.0 balance; neutral has both multiples at 1, so \
                resting and meditating add nothing to the standing rate.",
        keys: json!({
            "rest_mana_mult": { "v120": v.rest_mana_mult, "neutral": n.rest_mana_mult },
            "meditate_mana_mult": { "v120": v.meditate_mana_mult, "neutral": n.meditate_mana_mult },
        }),
        cases,
    }
}
