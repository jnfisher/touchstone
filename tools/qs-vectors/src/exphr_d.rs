//! The exp/hr Model D family (Q-014): HP and mana recovering in one phase (R10).
//!
//! The balance tool prints no exp/hr figure, so there is no closed engine number for a whole
//! model. The vectors are cross-checks assembled from the engine's own regeneration functions,
//! played second by second as the fast cycle plays them:
//! - `cycle`: the hit points and mana gained over one whole 210-second cycle (the least common
//!   multiple of the 30, 21 and 15 second ticks, so the counts are exact), resting or meditating;
//! - `recovery`: the seconds from a starting HP and mana to both targets, resting or meditating.
//!
//! The `neutral` column runs the same code with both mana multiples at 1 (resting and meditating
//! add nothing to the standing rate).

use serde_json::{Value, json};
use silvermere_rules::regen::{
    SLOW_SECS, class_stat, hp_regen, mana_regen, meditate_mana_regen, meditate_tick, rest_hp_regen,
    rest_mana_regen, rest_tick,
};
use silvermere_types::config::Balance;
use silvermere_types::enums::Magery;

use crate::Family;

const CYCLE_SECS: u32 = 210;
/// The longest a recovery run plays before it gives up (the cases keep clear of it).
const GIVE_UP_SECS: u32 = 999 * 5;

fn v120() -> Balance {
    Balance::V120
}

fn neutral() -> Balance {
    Balance {
        rest_mana_mult: 1,
        meditate_mana_mult: 1,
        ..Balance::V120
    }
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

/// MME's `enmMagicEnum` numbering, which touchstone's inputs use (the casting schools only).
const MAGERIES: [(i32, Magery); 4] = [
    (1, Magery::Mage),
    (2, Magery::Priest),
    (3, Magery::Druid),
    (4, Magery::Bard),
];

/// The engine's player: what the fast cycle needs to run.
#[derive(Clone, Copy)]
struct Player {
    standing: i32,
    level: u16,
    hea: u16,
    healrate: i16,
}

/// One second at a time: the slow tick (standing mana and hit points), the rest tick (hit points,
/// and mana when resting) and the meditate tick (mana when meditating). Returns the gains per second.
struct Run<'a> {
    p: Player,
    mode: Mode,
    b: &'a Balance,
    sec: u32,
    rest_counter: u8,
    med_counter: u8,
    rest_carry: u8,
    med_carry: u8,
    hp: i64,
    mana: i64,
}

impl<'a> Run<'a> {
    fn new(p: Player, mode: Mode, b: &'a Balance) -> Self {
        Run {
            p,
            mode,
            b,
            sec: 0,
            rest_counter: 0,
            med_counter: 0,
            rest_carry: 0,
            med_carry: 0,
            hp: 0,
            mana: 0,
        }
    }

    fn step(&mut self) {
        self.sec += 1;
        if self.sec % u32::from(SLOW_SECS) == 0 {
            self.mana += i64::from(self.p.standing);
            self.hp += i64::from(hp_regen(self.p.level, self.p.hea, self.p.healrate));
        }
        if rest_tick(&mut self.rest_counter) {
            self.hp += i64::from(rest_hp_regen(self.p.level, self.p.hea, self.p.healrate, self.b));
            if self.mode == Mode::Rest {
                self.mana += i64::from(rest_mana_regen(self.p.standing, &mut self.rest_carry, self.b));
            }
        }
        if self.mode == Mode::Meditate && meditate_tick(&mut self.med_counter) {
            self.mana +=
                i64::from(meditate_mana_regen(self.p.standing, &mut self.med_carry, self.b));
        }
    }
}

fn cycle(p: Player, mode: Mode, b: &Balance) -> Value {
    let mut run = Run::new(p, mode, b);
    for _ in 0..CYCLE_SECS {
        run.step();
    }
    json!({ "hp": run.hp, "mana": run.mana })
}

fn cycle_case(normal: i32, level: u16, hea: u16, healrate: i16, mode: Mode) -> Value {
    let p = Player { standing: normal, level, hea, healrate };
    json!({
        "id": format!("cycle/{}-n{normal:04}-l{level:03}-h{hea:03}-r{healrate:+04}", mode.name()),
        "kind": "cycle",
        "inputs": {
            "mode": mode.name(), "standing": normal,
            // the engine's pieces, so the test can rebuild MME's 20-second rest from them
            "rest_hp": rest_hp_regen(level, hea, healrate, &v120()),
            "slow_hp": hp_regen(level, hea, healrate),
        },
        "v120": cycle(p, mode, &v120()),
        "neutral": cycle(p, mode, &neutral()),
    })
}

/// Seconds until the hit points and the mana have both reached their targets.
fn recovery_secs(p: Player, mode: Mode, hp_missing: i64, mp_missing: i64, b: &Balance) -> u32 {
    let mut run = Run::new(p, mode, b);
    while run.sec < GIVE_UP_SECS && (run.hp < hp_missing || run.mana < mp_missing) {
        run.step();
    }
    run.sec
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
    fn standing(&self, b: &Balance) -> i32 {
        let m = MAGERIES.iter().find(|(c, _)| *c == self.magic_type).expect("magic type").1;
        let stat = class_stat(m, self.int, self.wil, self.cha);
        mana_regen(m, stat, self.level, self.magelvl, self.manarate, b)
    }

    fn tag(&self) -> String {
        format!(
            "m{}-i{:03}-w{:03}-c{:03}-l{:03}-ml{}-r{:+04}",
            self.magic_type, self.int, self.wil, self.cha, self.level, self.magelvl, self.manarate
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn recovery_case(
    c: Caster,
    mode: Mode,
    hea: u16,
    healrate: i16,
    scenario: usize,
    hp_missing: i64,
    mp_missing: i64,
) -> Value {
    let (v, n) = (v120(), neutral());
    let standing = c.standing(&v);
    let p = Player { standing, level: c.level, hea, healrate };
    json!({
        "id": format!(
            "recovery/{}-{}-h{hea:03}-hr{healrate:+04}-s{scenario}", mode.name(), c.tag()
        ),
        "kind": "recovery",
        "inputs": {
            "mode": mode.name(), "standing": standing,
            "rest_hp": rest_hp_regen(c.level, hea, healrate, &v),
            "hp_missing": hp_missing, "mp_missing": mp_missing,
        },
        "v120": recovery_secs(p, mode, hp_missing, mp_missing, &v),
        "neutral": recovery_secs(p, mode, hp_missing, mp_missing, &n),
    })
}

pub fn family() -> Family {
    let mut cases = Vec::new();

    for normal in [0, 2, 3, 7, 13, 40, 120] {
        for (level, hea, healrate) in [(1u16, 30u16, 0i16), (30, 100, 50), (60, 200, 0)] {
            for mode in [Mode::Rest, Mode::Meditate] {
                cases.push(cycle_case(normal, level, hea, healrate, mode));
            }
        }
    }

    let stat_sets: [(u16, u16, u16); 2] = [(60, 40, 45), (100, 120, 90)];
    let hp_sets: [(u16, i16); 3] = [(30, 0), (100, 50), (200, 0)];
    for (magic_type, _) in MAGERIES {
        for level in [1u16, 10, 30, 60] {
            for (int, wil, cha) in stat_sets {
                for magelvl in [1i16, 3] {
                    for manarate in [0i16, 50] {
                        let c = Caster { magic_type, int, wil, cha, level, magelvl, manarate };
                        // a character who cannot regenerate mana has nothing to wait for
                        if c.standing(&v120()) <= 0 {
                            continue;
                        }
                        let max_hp = 30 + i64::from(level) * 12;
                        let max_mp = i64::from(magelvl) * i64::from(level) * 2 + 6;
                        // missing to the 90 percent target: a deep drain, then a shallow one
                        let scenarios = [
                            (max_hp * 50 / 100, max_mp * 80 / 100),
                            (max_hp * 20 / 100, max_mp * 30 / 100),
                        ];
                        for (hea, healrate) in hp_sets {
                            for mode in [Mode::Rest, Mode::Meditate] {
                                for (i, (hp_missing, mp_missing)) in scenarios.iter().enumerate() {
                                    cases.push(recovery_case(
                                        c, mode, hea, healrate, i, *hp_missing, *mp_missing,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let (v, n) = (v120(), neutral());
    Family {
        name: "exphr_d",
        about: "Cross-checks for exp/hr Model D under Quicksilver (R10: rest and meditate restore mana, so hit \
                points and mana recover in one phase). The engine has no exp/hr figure, so the cases are built \
                from its own regeneration ticks played second by second. kind cycle: hit points and mana gained \
                over one 210-second cycle (the least common multiple of the 30, 21 and 15 second ticks), resting \
                or meditating, with the engine's rest-tick and slow-tick hit points in inputs. kind recovery: \
                seconds until the missing hit points and the missing mana are both restored, from a standing \
                rate, resting or meditating; inputs.rest_hp is the rest-tick hit point gain. v120 is the 1.20.0 \
                balance; neutral has both mana multiples at 1.",
        keys: json!({
            "rest_mana_mult": { "v120": v.rest_mana_mult, "neutral": n.rest_mana_mult },
            "meditate_mana_mult": { "v120": v.meditate_mana_mult, "neutral": n.meditate_mana_mult },
        }),
        cases,
    }
}
