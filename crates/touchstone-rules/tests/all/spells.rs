// Fixture tests for the spells family (FAM-10): see docs/porting.md step 5.
use crate::fixture::{self, Names, Textblocks};
use serde_json::Value;
use std::cell::Cell;
use touchstone_model::Spell;
use touchstone_rules::Profile;
use touchstone_rules::ability_text::AbilityEnv;
use touchstone_rules::spells::*;

fn i(v: &Value) -> i64 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}"))
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
    let spells = fixture::spell_table(table);
    assert!(spells.len() > 1000);
    let names = Names::from_table(&table["inputs"]["names"]);
    let tbs = Textblocks::from_table(&table["inputs"]["textblocks"]);
    let text = Tables {
        names: &names,
        tbs: &tbs,
    };

    let mut fails = Vec::new();
    let mut ran = 0;
    for c in &fx.cases {
        let id = c["id"].as_str().unwrap();
        if id == "table" {
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
                    let env = env_for(&spells, &text, inp, avg);
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
                let env = env_for(&spells, &text, inp, avg);
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

/// The name and textblock tables the case "table" carries.
struct Tables<'a> {
    names: &'a Names,
    tbs: &'a Textblocks,
}

/// MME's globals for the case, over the real text port.
fn env_for<'a>(spells: &'a [Spell], t: &Tables<'a>, inp: &'a Value, avg: f64) -> AbilityEnv<'a> {
    let frm = &inp["forms"]["frmMain"];
    let filter_on = frm["chkGlobalFilter"]["Value"] == 1;
    AbilityEnv {
        profile: Profile::Stock,
        names: t.names,
        textblocks: t.tbs,
        spells,
        hide_record_numbers: false,
        quick_spell: inp["globals"]["bQuickSpell"] == true,
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
    let (names, tbs) = (Names::default(), Textblocks::default());
    let env = AbilityEnv {
        profile: Profile::Stock,
        names: &names,
        textblocks: &tbs,
        spells: &[],
        hide_record_numbers: false,
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
