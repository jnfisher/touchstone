//! The opener family (R2: a stab no longer ends the round).
//!
//! Two kinds of case, both from the engine's own energy rules:
//! - `opener`: from one normal blow's energy, the stab's energy (`backstab_energy`) and the normal
//!   blows the rest of the round pays for (the round's blow limit counts the stab);
//! - `energy`: from a character (MME's inputs: level, class combat value, agility, weapon speed,
//!   encumbrance percent), the engine's own normal-blow and stab energy through its fighter builder.
//!
//! Three balances: 1.20.0, a stock-equivalent one (a stab that costs 1000 times a normal blow, which
//! the round's cap turns into the whole round) and an off-grid one (a stab cheaper than a blow).

use serde_json::{Value, json};
use silvermere_engine::clear_player;
use silvermere_engine::world::blank;
use silvermere_rules::combat::{
    AlignGroup, Blow, MAX_BLOWS, MAXENERGY, Move, PlayerAbilities, backstab_energy, energy_round,
    player_fighter,
};
use silvermere_types::config::Balance;
use silvermere_types::flags::PlayerSettings;
use silvermere_types::ids::{ClassNo, ItemNo};

use crate::Family;

fn v120() -> Balance {
    Balance::V120
}

fn neutral() -> Balance {
    Balance {
        backstab_energy_pct: 100_000,
        ..Balance::V120
    }
}

fn alt() -> Balance {
    Balance {
        backstab_energy_pct: 80,
        ..Balance::V120
    }
}

/// The normal blows after a stab of `stab` energy when a normal blow costs `normal`: the engine's
/// energy round started from the full pool less the stab, held to the round's blow limit less the
/// stab itself.
fn follow_blows(stab: u32, normal: u32) -> u32 {
    energy_round(-(stab as i32), normal as i32)
        .blows
        .min(MAX_BLOWS - 1)
}

fn opener(normal: u32, b: &Balance) -> Value {
    let stab = backstab_energy(normal, MAXENERGY, b);
    json!([stab, follow_blows(stab, normal)])
}

fn opener_case(normal: u32) -> Value {
    json!({
        "id": format!("opener/{normal:04}"),
        "kind": "opener",
        "inputs": { "normal": normal },
        "v120": opener(normal, &v120()),
        "alt": opener(normal, &alt()),
        "neutral": opener(normal, &neutral()),
    })
}

struct Hero {
    level: u16,
    combat: i32,
    agility: u16,
    speed: i16,
    encum_pct: u8,
}

/// The engine's energy for a normal blow and for a stab, in that order.
fn energies(h: &Hero, b: &Balance) -> (u32, u32) {
    let mut c = clear_player();
    c.level = h.level;
    c.agi = h.agility;
    c.str = 50;
    c.maxenergy = 1000;
    c.encumpct = i16::from(h.encum_pct);
    // a player who has not just cast: the engine adds 10% to a normal blow after a cast
    c.settings.insert(PlayerSettings::CANCAST);
    let mut class = blank::clear_class(ClassNo(1));
    // MME hands CalcEnergyUsed the class's CombatLVL less 2 and adds the 2 back
    class.combat = (h.combat + 2) as i16;
    let mut w = blank::clear_item(ItemNo(1));
    w.mindmg = 1;
    w.maxdmg = 4;
    w.speed = h.speed;
    let fighter = |mv: Move| {
        player_fighter(
            &c,
            &class,
            Some(&w),
            &[],
            PlayerAbilities::default(),
            Blow::new(mv, AlignGroup::Evil),
            b,
        )
        .energy
    };
    (fighter(Move::Attack), fighter(Move::Backstab))
}

fn energy_case(h: Hero) -> Value {
    let col = |b: &Balance| {
        let (n, s) = energies(&h, b);
        json!([n, s])
    };
    json!({
        "id": format!(
            "energy/l{:03}-c{}-a{:03}-v{:04}-e{:03}",
            h.level, h.combat, h.agility, h.speed, h.encum_pct
        ),
        "kind": "energy",
        "inputs": {
            "level": h.level, "combat": h.combat, "agility": h.agility,
            "speed": h.speed, "encum_pct": h.encum_pct,
        },
        "v120": col(&v120()),
        "alt": col(&alt()),
        "neutral": col(&neutral()),
    })
}

pub fn family() -> Family {
    let mut cases = Vec::new();
    let mut normal: u32 = 1;
    while normal <= 1500 {
        cases.push(opener_case(normal));
        normal += if normal < 420 { 1 } else { 13 };
    }
    for level in [1u16, 2, 3, 5, 8, 12, 20, 40] {
        for combat in [1i32, 3, 5] {
            for agility in [30u16, 60, 100, 150, 255] {
                for speed in [1000i16, 1500, 2000, 3000, 4500, 6000] {
                    for encum_pct in [0u8, 20, 50, 80] {
                        cases.push(energy_case(Hero { level, combat, agility, speed, encum_pct }));
                    }
                }
            }
        }
    }
    let (v, n, a) = (v120(), neutral(), alt());
    Family {
        name: "opener",
        about: "Quicksilver's stab energy and opening round. kind opener: [stab energy, normal blows after the \
                stab] for one normal blow costing `normal` energy, from the engine's backstab_energy and \
                energy round (the round's blow limit counts the stab). kind energy: [normal blow energy, stab \
                energy] from the engine's fighter builder for a player (Strength 50, a 1-4 weapon of the \
                given speed, no strength requirement); combat is the class value MME uses (the class's \
                CombatLVL less 2). v120 is the 1.20.0 balance, neutral has a stab that costs 1000 times a \
                normal blow (so it fills the round), alt is an off-grid balance (see balance_keys).",
        keys: json!({
            "backstab_energy_pct": { "v120": v.backstab_energy_pct, "neutral": n.backstab_energy_pct, "alt": a.backstab_energy_pct },
        }),
        cases,
    }
}
