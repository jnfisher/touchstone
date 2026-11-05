// Fixture tests for the spells family (FAM-10): see docs/porting.md step 5.
use crate::fixture;
use serde_json::Value;
use std::cell::Cell;
use std::collections::HashMap;
use touchstone_model::{AbilitySlot, Spell};
use touchstone_rules::Profile;
use touchstone_rules::spells::*;

fn i(v: &Value) -> i64 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}"))
}

/// The case "table" carries every stock row as [fields.., learnable, learned_from?, [ability, value, ..]].
fn spell_table(case: &Value) -> Vec<Spell> {
    case["inputs"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let f: Vec<i32> = r.as_array().unwrap()[..18]
                .iter()
                .map(|x| i(x) as i32)
                .collect();
            let flat = r[18].as_array().unwrap();
            let mut abilities = [AbilitySlot::default(); 10];
            for (k, p) in flat.chunks(2).enumerate() {
                abilities[k] = AbilitySlot {
                    ability: i(&p[0]) as i32,
                    value: i(&p[1]) as i32,
                };
            }
            Spell {
                number: f[0],
                name: None,
                short: None,
                req_level: f[1],
                energy_cost: f[2],
                mana_cost: 0,
                min_base: f[3],
                max_base: f[4],
                diff: 0,
                type_of_resists: 0,
                targets: f[5],
                dur: f[6],
                att_type: f[7],
                magery: f[8],
                magery_lvl: 0,
                cap: f[9],
                max_inc_lvls: f[10],
                max_inc: f[11],
                min_inc_lvls: f[12],
                min_inc: f[13],
                dur_inc_lvls: f[14],
                dur_inc: f[15],
                abilities,
                learnable: f[16] != 0,
                learned_from: (f[17] != 0).then(|| "x".to_string()),
                casted_by: None,
                classes: None,
            }
        })
        .collect()
}

/// GetAbilityStats / GetSpellName / GetMonsterName answers the oracle gave, replayed.
#[derive(Default)]
struct Recorded {
    stats: HashMap<(i16, i16, bool, bool), String>,
    spells: HashMap<i32, String>,
    monsters: HashMap<i32, String>,
    quick: Cell<bool>,
}

impl SpellText for Recorded {
    fn ability_stats(&self, ability: i16, value: i16, calc_level: bool, _pct: bool) -> String {
        // Only these abilities print a nested PullSpellEQ, so only they depend on the flags.
        let nest = matches!(ability, 43 | 148 | 151 | 153);
        let key = (
            ability,
            value,
            !nest || calc_level,
            nest && self.quick.get(),
        );
        self.stats
            .get(&key)
            .unwrap_or_else(|| panic!("no recorded GetAbilityStats for {key:?}"))
            .clone()
    }
    fn spell_name(&self, n: i32) -> String {
        self.spells
            .get(&n)
            .unwrap_or_else(|| panic!("no recorded GetSpellName({n})"))
            .clone()
    }
    fn monster_name(&self, n: i32) -> String {
        self.monsters
            .get(&n)
            .unwrap_or_else(|| panic!("no recorded GetMonsterName({n})"))
            .clone()
    }
}

fn is_missing(v: &Value) -> bool {
    v["t"] == "missing"
}

/// Optional integer argument `k` of a recorded call (absent or Missing is 0).
fn arg(args: &[Value], k: usize) -> i64 {
    args.get(k).filter(|v| !is_missing(v)).map_or(0, |v| {
        v.as_i64().or_else(|| v["value"].as_i64()).unwrap_or(0)
    })
}

fn flag(args: &[Value], k: usize) -> bool {
    args.get(k).filter(|v| !is_missing(v)).is_some_and(|v| {
        v.as_bool()
            .or_else(|| v["value"].as_bool())
            .unwrap_or(false)
    })
}

fn cur_ok(v: &Value, c: touchstone_vb6::Currency) -> bool {
    fixture::cur(v) == c.scaled()
}

fn pull_args(args: &[Value], spell: i32) -> PullArgs {
    PullArgs {
        calc_level: flag(args, 0),
        level: arg(args, 1) as i16,
        spell,
        min_max_damage_only: flag(args, 4),
        for_monster: flag(args, 5),
        percent_column: flag(args, 6),
        is_nested: flag(args, 7),
        no_show_level: flag(args, 8),
        override_min: arg(args, 9) as i32,
        override_max: arg(args, 10) as i32,
        spell_bonus: arg(args, 11) as i16,
    }
}

#[test]
fn spells_fixture() {
    let fx = fixture::load("spells");
    let table = fx
        .cases
        .iter()
        .find(|c| c["id"] == "table")
        .expect("table case");
    let spells = spell_table(table);
    assert!(spells.len() > 1000);

    let mut rec = Recorded::default();
    for c in &fx.cases {
        let id = c["id"].as_str().unwrap();
        let a = c["inputs"]["args"].as_array();
        if id.starts_with("as-") {
            let a = a.unwrap();
            let quick = c["inputs"]["globals"]["bQuickSpell"] == true;
            let (ab, v, calc) = (i(&a[0]) as i16, i(&a[1]) as i16, a[3].as_bool().unwrap());
            let nest = matches!(ab, 43 | 148 | 151 | 153);
            rec.stats.insert(
                (ab, v, !nest || calc, nest && quick),
                c["outputs"]["result"].as_str().unwrap().to_string(),
            );
        } else if id.starts_with("sn-") {
            rec.spells.insert(
                i(&a.unwrap()[0]) as i32,
                c["outputs"]["result"].as_str().unwrap().to_string(),
            );
        } else if id.starts_with("mn-") {
            rec.monsters.insert(
                i(&a.unwrap()[0]) as i32,
                c["outputs"]["result"].as_str().unwrap().to_string(),
            );
        }
    }

    let mut fails = Vec::new();
    let mut ran = 0;
    for c in &fx.cases {
        let id = c["id"].as_str().unwrap();
        if id == "table" || ["as-", "sn-", "mn-"].iter().any(|p| id.starts_with(p)) {
            continue;
        }
        let inp = &c["inputs"];
        let out = &c["outputs"];
        let avg = inp["globals"]["gAvgLevelMaxAllStats"]
            .as_f64()
            .unwrap_or(0.0);
        let escaped = c.get("error").map(|e| i(&e["number"]));
        ran += 1;
        let mut bad = |what: String| fails.push(format!("{id}: {what}; inputs {inp}"));

        if let Some(steps) = inp["steps"].as_array() {
            // Step 0 positions the cursor; step 1 is the call under test.
            // A failed seek leaves the cursor on the first record (MoveFirst).
            let n = i(&steps[0]["args"][0]) as i32;
            let n = if spells.iter().any(|s| s.number == n) {
                n
            } else {
                spells[0].number
            };
            let sargs = steps[1]["args"].as_array().unwrap();
            match steps[1]["call"].as_str().unwrap() {
                "GetCurrentSpellMinMax" => {
                    let req = MinMaxRequest {
                        use_level: flag(sargs, 0),
                        level: arg(sargs, 1) as i16,
                        no_header: flag(sargs, 2),
                        override_min: arg(sargs, 3) as i32,
                        override_max: arg(sargs, 4) as i32,
                        spell_bonus: arg(sargs, 5) as i16,
                    };
                    let got =
                        get_current_spell_min_max(spells.iter().find(|s| s.number == n), avg, &req);
                    let w = &out["r"];
                    let ok = cur_ok(&w["nmin"], got.n_min)
                        && cur_ok(&w["nmax"], got.n_max)
                        && cur_ok(&w["ndur"], got.n_dur)
                        && w["smin"] == got.s_min.as_str()
                        && w["smax"] == got.s_max.as_str()
                        && w["sdur"] == got.s_dur.as_str()
                        && out["r.refs"] == serde_json::json!([got.use_level, got.no_header]);
                    if !ok {
                        bad(format!("got {got:?}, want {w} {}", out["r.refs"]));
                    }
                }
                "PullSpellEQ" => {
                    // MME's nSpellNo 0 is the record the first step left the cursor on.
                    let env = env_for(&spells, &rec, inp, avg);
                    let got = env.pull_spell_eq(&pull_args(sargs, n));
                    if out["r"] != got.as_str() {
                        bad(format!("got {got:?}, want {}", out["r"]));
                    }
                }
                other => panic!("{id}: step {other}"),
            }
            continue;
        }

        let a = inp["args"].as_array().unwrap();
        let proc = inp["proc"].as_str().unwrap();
        let n = arg(a, 0) as i32;
        let want = &out["result"];
        match proc {
            "GetSpellAttackType" => {
                let got = get_spell_attack_type(&spells, n);
                if *want != got {
                    bad(format!("got {got}, want {want}"));
                }
            }
            "SpellIsAreaAttack" | "SpellIsBlessSpell" | "SpellDoesDamage" => {
                let got = match proc {
                    "SpellIsAreaAttack" => spell_is_area_attack(&spells, n),
                    "SpellIsBlessSpell" => spell_is_bless_spell(&spells, n),
                    _ => spell_does_damage(&spells, n, flag(a, 1)),
                };
                if *want != got {
                    bad(format!("got {got}, want {want}"));
                }
            }
            "GetMaxLevel" => match (get_max_level(avg), escaped) {
                (Ok(v), None) if *want == v => {}
                (Err(_), Some(6)) => {}
                (g, e) => bad(format!("got {g:?}, want {want} / error {e:?}")),
            },
            "GetSpellMinDamage" | "GetSpellMaxDamage" => {
                let mut energy = arg(a, 2) as i16;
                let (lvl, mon, heals) = (arg(a, 1) as i16, flag(a, 3), flag(a, 4));
                let got = if proc == "GetSpellMinDamage" {
                    get_spell_min_damage(&spells, n, lvl, &mut energy, mon, heals)
                } else {
                    get_spell_max_damage(&spells, n, lvl, &mut energy, mon, heals)
                };
                let refs_ok = a.get(2).is_none_or(|_| out["result.refs"][0] == energy);
                if *want != got || !refs_ok {
                    bad(format!(
                        "got {got} (energy {energy}), want {want} {}",
                        out["result.refs"]
                    ));
                }
            }
            "GetSpellDuration" => {
                let got = get_spell_duration(&spells, n, arg(a, 1) as i16, flag(a, 2));
                if *want != got {
                    bad(format!("got {got}, want {want}"));
                }
            }
            "PullSpellEQ" => {
                let env = env_for(&spells, &rec, inp, avg);
                // nSpell 0 means the record under the cursor, which on a fresh recordset is the first.
                let spell = if arg(a, 2) == 0 {
                    spells[0].number
                } else {
                    arg(a, 2) as i32
                };
                let got = env.pull_spell_eq(&pull_args(a, spell));
                if *want != got.as_str() {
                    bad(format!("got {got:?}, want {want}"));
                }
            }
            other => panic!("{id}: unknown proc {other}"),
        }
    }
    assert!(ran > 4000, "only {ran} cases ran");
    assert!(
        fails.is_empty(),
        "{} of {ran} cases failed, first: {:#?}",
        fails.len(),
        &fails[..fails.len().min(8)]
    );
}

fn env_for<'a>(spells: &'a [Spell], rec: &'a Recorded, inp: &'a Value, avg: f64) -> SpellEnv<'a> {
    let quick = inp["globals"]["bQuickSpell"] == true;
    rec.quick.set(quick);
    let frm = &inp["forms"]["frmMain"];
    let filter_on = frm["chkGlobalFilter"]["Value"] == 1;
    SpellEnv {
        profile: Profile::Stock,
        spells,
        text: rec,
        quick_spell: quick,
        global_level_text: filter_on.then(|| {
            frm["txtGlobalLevel"]["$array"]["0"]["Text"]
                .as_str()
                .unwrap_or("")
        }),
        avg_level_max_all_stats: avg,
        nest: Cell::new(inp["globals"]["nSpellNest"].as_i64().unwrap_or(0) as i32),
    }
}

// An empty table is the one PullSpellEQ branch the stock database cannot reach: "tabSpells.RecordCount = 0".
#[test]
fn pull_spell_eq_with_no_spell_records() {
    let rec = Recorded::default();
    let env = SpellEnv {
        profile: Profile::Stock,
        spells: &[],
        text: &rec,
        quick_spell: false,
        global_level_text: None,
        avg_level_max_all_stats: 0.0,
        nest: Cell::new(0),
    };
    assert_eq!(
        env.pull_spell_eq(&PullArgs::default()),
        "(No Spell Records)"
    );
    assert_eq!(env.nest.get(), 0);
}
