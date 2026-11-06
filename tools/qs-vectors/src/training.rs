//! The training family (R14 stat soft caps and training prices).
//!
//! Each case runs the engine twice: under the 1.20.0 balance and under a balance with the
//! soft-cap keys switched off (floor 0, 10 a point in every band).

use serde_json::{Value, json};
use silvermere_rules::progression::{soft_cap, train_cost, train_max};
use silvermere_types::config::Balance;

use crate::Family;

fn v120() -> Balance {
    Balance::V120
}

fn neutral() -> Balance {
    Balance {
        softcap_floor: 0,
        softcap_floor_price: 10,
        softcap_band_prices: [10, 10, 10],
        ..Balance::V120
    }
}

fn cost_case(race_min: u16, race_max: u16, from: u16, to: u16) -> Value {
    // The engine at neutral prices a point past the race maximum at a flat 10 where MME keeps
    // climbing the stock ladder, so the two only agree up to the race maximum: no neutral column
    // there (the vectors test checks this is exactly the rule).
    let neutral_cost = (to <= race_max)
        .then(|| train_cost(race_min, race_max, from, to, &neutral()));
    json!({
        "id": format!("cost/n{race_min:03}-x{race_max:03}-f{from:03}-t{to:03}"),
        "kind": "cost",
        "inputs": { "race_min": race_min, "race_max": race_max, "from": from, "to": to },
        "v120": train_cost(race_min, race_max, from, to, &v120()),
        "neutral": neutral_cost,
    })
}

fn max_case(kind: &str, race_max: u16) -> Value {
    let (v, n) = (v120(), neutral());
    let f = |b: &Balance| match kind {
        "softcap" => soft_cap(race_max, b),
        _ => train_max(race_max, b),
    };
    json!({
        "id": format!("{kind}/x{race_max:03}"),
        "kind": kind,
        "inputs": { "race_max": race_max },
        "v120": f(&v),
        "neutral": f(&n),
    })
}

pub fn family() -> Family {
    let mut cases = Vec::new();
    // race ranges: narrow and wide, below, at and above the 90 floor
    let ranges = [
        (20u16, 60u16),
        (30, 80),
        (40, 90),
        (45, 95),
        (50, 100),
        (60, 120),
        (25, 70),
        (80, 90),
    ];
    for (mn, mx) in ranges {
        let mut ts: Vec<u16> = Vec::new();
        for d in [-30i32, -10, -1, 0, 1, 5, 10, 11, 20, 21, 30, 31, 40, 45, 60] {
            for base in [i32::from(mx), 90] {
                let t = base + d;
                if t > i32::from(mn) {
                    ts.push(t as u16);
                }
            }
        }
        ts.extend([mn + 1, mn + 10, mn + 55, mn + 90, mn + 95, 150, 200, 255]);
        ts.sort_unstable();
        ts.dedup();
        for from in [mn, mn + 5, mx.saturating_sub(3).max(mn), mx, mx + 4, 95] {
            for &to in &ts {
                cases.push(cost_case(mn, mx, from, to));
            }
        }
        // a lowering costs nothing
        cases.push(cost_case(mn, mx, mx + 5, mx));
    }
    for race_max in [40u16, 70, 89, 90, 91, 100, 120, 150, 255] {
        cases.push(max_case("softcap", race_max));
        cases.push(max_case("train_max", race_max));
    }
    cases.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    cases.dedup_by(|a, b| a["id"] == b["id"]);
    let (v, n) = (v120(), neutral());
    Family {
        name: "training",
        about: "Quicksilver's stat soft caps and TRAIN STATS prices, one engine call per column. kind cost: character \
                points to raise a stat from `from` to `to` for a race range race_min..race_max (the stock ladder up to \
                the race maximum, the floor price up to the soft cap, then three bands). kind softcap: the soft cap for \
                a race maximum. kind train_max: the highest value training takes the stat to. v120 is the 1.20.0 balance; \
                neutral has the soft-cap keys off, and is null for a cost case whose `to` is above the race maximum, \
                where the engine's flat price and the stock ladder differ for reasons unrelated to Quicksilver.",
        keys: json!({
            "softcap_floor": { "v120": v.softcap_floor, "neutral": n.softcap_floor },
            "softcap_floor_price": { "v120": v.softcap_floor_price, "neutral": n.softcap_floor_price },
            "softcap_band_prices": { "v120": v.softcap_band_prices, "neutral": n.softcap_band_prices },
        }),
        cases,
    }
}
