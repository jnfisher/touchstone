//! The knee family (R11 crit, R12 magic resistance, R13 dodge).
//!
//! Each case runs the engine twice: under the 1.20.0 balance and under a balance whose knees
//! are set where the stock rules have them (crit 40, the others out of reach).

use serde_json::{Value, json};
use silvermere_engine::player::secondary::secondary;
use silvermere_engine::world::blank;
use silvermere_engine::clear_player;
use silvermere_rules::abilities::{Ability, AbilityLookup};
use silvermere_rules::combat::{AlignGroup, Blow, Move, PlayerAbilities, effective_crit, player_fighter};
use silvermere_types::config::Balance;
use silvermere_types::ids::{ClassNo, RaceNo};

use crate::Family;

fn v120() -> Balance {
    Balance::V120
}

/// The stock-equivalent knees; every other key keeps its 1.20.0 value because none of these
/// functions reads another one.
fn neutral() -> Balance {
    Balance {
        crit_knee: 40,
        mr_stat_knee: 65_535,
        dodge_agi_knee: 65_535,
        ..Balance::V120
    }
}

/// Ability totals where only Magic Resistance is given.
struct MagicResistOnly(i32);

impl AbilityLookup for MagicResistOnly {
    fn value(&self, a: Ability) -> i32 {
        if a == Ability::MagicResist { self.0 } else { 0 }
    }
    fn has(&self, a: Ability) -> bool {
        a == Ability::MagicResist
    }
    fn pair(&self, a: Ability, b: Ability) -> (i32, i32) {
        (self.value(a), self.value(b))
    }
    fn best(&self, a: Ability) -> Option<i32> {
        self.has(a).then(|| self.value(a))
    }
}

fn crit_case(crit: i32) -> Value {
    json!({
        "id": format!("crit/{crit:03}"),
        "kind": "crit",
        "inputs": { "crit": crit },
        "v120": effective_crit(crit, &v120()),
        "neutral": effective_crit(crit, &neutral()),
    })
}

fn mr_of(int: u16, wil: u16, gear: i32, b: &Balance) -> i32 {
    let mut c = clear_player();
    c.level = 60;
    c.intel = int;
    c.wil = wil;
    let class = blank::clear_class(ClassNo(1));
    let race = blank::clear_race(RaceNo(1));
    secondary(&c, &class, &race, &MagicResistOnly(gear), b).maxmr
}

fn mr_case(int: u16, wil: u16, gear: i32) -> Value {
    json!({
        "id": format!("mr/i{int:03}-w{wil:03}-g{gear:+04}"),
        "kind": "mr",
        "inputs": { "int": int, "wil": wil, "gear": gear },
        "v120": mr_of(int, wil, gear, &v120()),
        "neutral": mr_of(int, wil, gear, &neutral()),
    })
}

/// The dodge rating the player builder gives. Encumbrance 50 keeps the light-load bonus out,
/// and the player is alive, so only the level, charm, agility and ability terms remain.
fn dodge_of(level: u16, agi: u16, cha: u16, plus: i32, b: &Balance) -> i32 {
    let mut c = clear_player();
    c.level = level;
    c.agi = agi;
    c.cha = cha;
    c.hp = 1;
    c.maxenergy = 1000;
    c.encumpct = 50;
    let class = blank::clear_class(ClassNo(1));
    let abil = PlayerAbilities { dodge: plus, ..PlayerAbilities::default() };
    player_fighter(&c, &class, None, &[], abil, Blow::new(Move::Punch, AlignGroup::Evil), b).dodge
}

fn dodge_case(level: u16, agi: u16, cha: u16, plus: i32) -> Value {
    json!({
        "id": format!("dodge/l{level:03}-a{agi:03}-c{cha:03}-p{plus:02}"),
        "kind": "dodge",
        "inputs": { "level": level, "agility": agi, "charm": cha, "plus_dodge": plus },
        "v120": dodge_of(level, agi, cha, plus, &v120()),
        "neutral": dodge_of(level, agi, cha, plus, &neutral()),
    })
}

pub fn family() -> Family {
    let mut cases = Vec::new();
    for crit in 0..=200 {
        cases.push(crit_case(crit));
    }
    let stats = [0u16, 30, 50, 80, 100, 110, 115, 120, 140, 160, 200, 255];
    for &int in &stats {
        for &wil in &stats {
            for gear in [-20, 0, 30] {
                cases.push(mr_case(int, wil, gear));
            }
        }
    }
    for level in [1u16, 10, 30, 60, 100] {
        for agi in [0u16, 30, 49, 50, 90, 149, 150, 151, 153, 180, 255] {
            for cha in [20u16, 50, 90] {
                for plus in [0, 7] {
                    cases.push(dodge_case(level, agi, cha, plus));
                }
            }
        }
    }
    let (v, n) = (v120(), neutral());
    Family {
        name: "knees",
        about: "Quicksilver's knee rules, one engine call per column. kind crit: the crit chance a blow rolls against \
                (no 99 cap; the caller caps). kind mr: the player's magic resistance (stat part (Int+3*Wil)/4 with the \
                knee, plus gear). kind dodge: the player's dodge rating with encumbrance 50 (no light-load bonus). \
                v120 is the 1.20.0 balance; neutral has the knees where the stock rules put them.",
        keys: json!({
            "crit_knee": { "v120": v.crit_knee, "neutral": n.crit_knee },
            "mr_stat_knee": { "v120": v.mr_stat_knee, "neutral": n.mr_stat_knee },
            "dodge_agi_knee": { "v120": v.dodge_agi_knee, "neutral": n.dodge_agi_knee },
        }),
        cases,
    }
}
