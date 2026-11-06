// Fixture tests for the stats family: see docs/porting.md step 5.
// Expected values come from fixtures/rules/stats.json (MME's own code run by the oracle).
use serde_json::Value;
use touchstone_model::{AbilitySlot, Class, Race};
use touchstone_rules::Profile;
use touchstone_rules::RulesError;
use touchstone_rules::stats::*;
use touchstone_vb6::Currency;

use crate::fixture::{self, cur};

fn abilities(row: &Value) -> [AbilitySlot; 10] {
    let mut out = [AbilitySlot::default(); 10];
    for (i, a) in row["abilities"].as_array().unwrap().iter().enumerate() {
        out[i] = AbilitySlot {
            ability: a[0].as_i64().unwrap() as i32,
            value: a[1].as_i64().unwrap() as i32,
        };
    }
    out
}

fn n(row: &Value, k: &str) -> i32 {
    row[k].as_i64().unwrap_or_else(|| panic!("{k} in {row}")) as i32
}

fn race(row: &Value) -> Option<Race> {
    if row.is_null() {
        return None;
    }
    Some(Race {
        number: n(row, "number"),
        name: String::new(),
        m_int: n(row, "m_int"),
        m_wil: n(row, "m_wil"),
        m_str: n(row, "m_str"),
        m_hea: n(row, "m_hea"),
        m_agl: n(row, "m_agl"),
        m_chm: n(row, "m_chm"),
        x_int: n(row, "x_int"),
        x_wil: n(row, "x_wil"),
        x_str: n(row, "x_str"),
        x_hea: n(row, "x_hea"),
        x_agl: n(row, "x_agl"),
        x_chm: n(row, "x_chm"),
        hp_per_lvl: n(row, "hp_per_lvl"),
        exp_table: 0,
        base_cp: n(row, "base_cp"),
        abilities: abilities(row),
    })
}

fn class(row: &Value) -> Option<Class> {
    if row.is_null() {
        return None;
    }
    Some(Class {
        number: n(row, "number"),
        name: String::new(),
        min_hits: n(row, "min_hits"),
        max_hits: n(row, "max_hits"),
        exp_table: 0,
        magery_type: n(row, "magery_type"),
        magery_lvl: n(row, "magery_lvl"),
        weapon_type: 0,
        armour_type: n(row, "armour_type"),
        combat_lvl: n(row, "combat_lvl"),
        abilities: abilities(row),
    })
}

fn int(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an int: {v}")) as i32
}

fn flag(v: &Value) -> bool {
    v.as_bool().unwrap_or_else(|| panic!("not a bool: {v}"))
}

/// A Currency argument: typed `{"t":"cur","v":x}` or a plain number.
fn cur_arg(v: &Value) -> Currency {
    let x = if v.is_object() { &v["v"] } else { v };
    Currency::from_f64(x.as_f64().unwrap()).unwrap()
}

fn arg(a: &[Value], i: usize) -> i32 {
    a.get(i).map_or(0, int)
}

fn cur_out(c: Currency) -> i64 {
    c.scaled()
}

/// Runs one case; returns a description of the mismatch, if any.
pub(crate) fn run(profile: Profile, case: &Value) -> Result<(), String> {
    let inp = &case["inputs"];
    let out = &case["outputs"];
    let expect_err = case
        .get("error")
        .map(|e| e["number"].as_i64().unwrap() as i32);
    let (proc, args): (&str, Vec<Value>) = if let Some(p) = inp["proc"].as_str() {
        (p, inp["args"].as_array().cloned().unwrap_or_default())
    } else if let Some(s) = inp["steps"].as_array() {
        (
            s[0]["call"].as_str().unwrap(),
            s[0]["args"].as_array().cloned().unwrap_or_default(),
        )
    } else {
        ("ComputeAvgLevelMaxStats", vec![])
    };
    let p = profile;
    let a = &args[..];

    // Compare an Ok value against the expected output / error.
    macro_rules! check {
        ($res:expr, $want:expr) => {{
            match ($res, expect_err) {
                (Err(RulesError::Vb6(c)), Some(e)) if c == e => return Ok(()),
                (Err(e), _) => {
                    return Err(format!("unexpected error {e:?}, expected {expect_err:?}"));
                }
                (Ok(_), Some(e)) => return Err(format!("expected error {e}")),
                (Ok(v), None) => (v, $want),
            }
        }};
    }
    macro_rules! eq {
        ($got:expr, $want:expr) => {{
            let (g, w) = ($got, $want);
            if g == w {
                Ok(())
            } else {
                Err(format!("got {g:?}, want {w:?}"))
            }
        }};
    }

    let r = &out["result"];
    let want_i = || r.as_i64().map(|x| x as i32);
    match proc {
        "GetRaceHPBonus" => eq!(
            get_race_hp_bonus(race(&inp["race"]).as_ref()),
            want_i().unwrap()
        ),
        "GetRaceCP" => eq!(get_race_cp(race(&inp["race"]).as_ref()), want_i().unwrap()),
        "GetRaceStealth" => {
            let rc = race(&inp["race"]);
            if let Some(sel) = inp.get("selected") {
                let ids: Vec<i32> = sel["item_data"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(int)
                    .collect();
                let combo = ComboSelection {
                    global_filter: sel["chk"].as_i64() == Some(1),
                    item_data: &ids,
                    list_index: int(&sel["list_index"]),
                };
                let num = stealth_race_number(arg(a, 0), &combo);
                if rc.as_ref().map_or(0, |x| x.number) != num {
                    return Err(format!("resolved race number {num}"));
                }
            }
            eq!(get_race_stealth(rc.as_ref()), flag(r))
        }
        "GetClassMaxHP" => eq!(
            get_class_max_hp(class(&inp["class"]).as_ref()),
            want_i().unwrap()
        ),
        "GetClassMinHP" => eq!(
            get_class_min_hp(class(&inp["class"]).as_ref()),
            want_i().unwrap()
        ),
        "GetClassMageryLVL" => eq!(
            get_class_magery_lvl(class(&inp["class"]).as_ref()),
            want_i().unwrap()
        ),
        "GetClassMagery" => eq!(
            get_class_magery(class(&inp["class"]).as_ref()) as i32,
            want_i().unwrap()
        ),
        "GetClassArmourType" => eq!(
            get_class_armour_type(class(&inp["class"]).as_ref()),
            want_i().unwrap()
        ),
        "GetClassCombat" => eq!(
            get_class_combat(class(&inp["class"]).as_ref()),
            want_i().unwrap()
        ),
        "GetClassStealth" => {
            let cl = class(&inp["class"]);
            if let Some(sel) = inp.get("selected") {
                let ids: Vec<i32> = sel["item_data"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(int)
                    .collect();
                let combo = ComboSelection {
                    global_filter: sel["chk"].as_i64() == Some(1),
                    item_data: &ids,
                    list_index: int(&sel["list_index"]),
                };
                let num = stealth_class_number(arg(a, 0), &combo);
                if cl.as_ref().map_or(0, |x| x.number) != num {
                    return Err(format!("resolved class number {num}"));
                }
            }
            eq!(get_class_stealth(cl.as_ref()), flag(r))
        }
        "ComputeAvgLevelMaxStats" => {
            let races: Vec<Race> = inp["races"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(race)
                .collect();
            let got = compute_avg_level_max_stats(p, &races);
            eq!(
                (got.all_stats, got.single_stat),
                (
                    out["all"].as_f64().unwrap(),
                    out["single"].as_f64().unwrap()
                )
            )
        }
        "CP_CostForRaise" => {
            let (g, w) = check!(cp_cost_for_raise(p, arg(a, 0)), want_i().unwrap());
            eq!(g, w)
        }
        "LevelNeededForCP" => {
            let (g, w) = check!(level_needed_for_cp(arg(a, 0), arg(a, 1)), want_i().unwrap());
            eq!(g, w)
        }
        "CalcCPLevel" => {
            let (g, w) = check!(calc_cp_level(arg(a, 0)), want_i().unwrap());
            eq!(g, w)
        }
        "CalcEncum" => {
            let ver = inp["forms"]["frmMain"]["lblDatVer"]["Caption"]
                .as_str()
                .unwrap();
            eq!(
                calc_encum(arg(a, 0) as i16, arg(a, 1) as i16, ver),
                want_i().unwrap()
            )
        }
        "GetMageryEnum" => eq!(
            get_magery_enum(arg(a, 0) as i16, arg(a, 1) as i16),
            r.as_str().unwrap().to_string()
        ),
        "CalcMovementSpeed" => eq!(
            calc_movement_speed(p, arg(a, 0), arg(a, 1), arg(a, 2)),
            want_i().unwrap()
        ),
        "CalcRestingRate" => eq!(
            calc_resting_rate(
                p,
                arg(a, 0),
                arg(a, 1),
                arg(a, 2),
                a.get(3).is_some_and(flag)
            ),
            want_i().unwrap()
        ),
        "CalcManaRegen" => {
            let input = ManaRegenInput {
                level: arg(a, 0),
                intellect: arg(a, 1),
                willpower: arg(a, 2),
                charm: arg(a, 3),
                magic_level: arg(a, 4),
                magic_type: arg(a, 5),
                mp_regen: arg(a, 6),
                meditating: a.get(7).is_some_and(flag),
            };
            eq!(cur_out(calc_mana_regen(p, &input)), cur(r))
        }
        "CalcMR" => eq!(
            calc_mr(profile, arg(a, 0), arg(a, 1), arg(a, 2)),
            want_i().unwrap()
        ),
        "CalcMaxHP" => eq!(
            calc_max_hp(arg(a, 0), arg(a, 1), arg(a, 2), arg(a, 3)),
            want_i().unwrap()
        ),
        "CalcMaxMana" => eq!(calc_max_mana(arg(a, 0), arg(a, 1)), want_i().unwrap()),
        "CalcSpellCasting" => eq!(
            calc_spell_casting(
                arg(a, 0),
                arg(a, 1),
                arg(a, 2),
                arg(a, 3),
                arg(a, 4),
                arg(a, 5)
            ),
            want_i().unwrap()
        ),
        "GetEncumPercents" => eq!(
            get_encum_percents(arg(a, 0)),
            r.as_str().unwrap().to_string()
        ),
        "CalcPicklocks" => {
            let (g, w) = check!(
                calc_picklocks(p, arg(a, 0), arg(a, 1), arg(a, 2)),
                want_i().unwrap()
            );
            eq!(g, w)
        }
        "CalcEncumbrancePercent" => {
            let (g, w) = check!(
                calc_encumbrance_percent(cur_arg(&a[0]), cur_arg(&a[1])),
                want_i().unwrap()
            );
            eq!(i32::from(g), w)
        }
        "AdjustSpeedForSlowness" => {
            let (g, w) = check!(adjust_speed_for_slowness(cur_arg(&a[0])), cur(r));
            eq!(cur_out(g), w)
        }
        "CalculateStealth" => {
            let input = StealthInput {
                level: arg(a, 0) as i16,
                agility: arg(a, 1) as i16,
                intellect: arg(a, 2) as i16,
                charm: arg(a, 3) as i16,
                class_stealth: flag(&a[4]),
                race_stealth: flag(&a[5]),
                plus_stealth: arg(a, 7) as i16,
                encum_pct: arg(a, 8) as i16,
            };
            let text_in = a.get(6).map_or("", |t| t["value"].as_str().unwrap());
            let got = calculate_stealth(p, &input, text_in);
            // With the optional text argument omitted the oracle has no variable to read the text from.
            let mut got = got;
            match out.get("result.refs") {
                Some(v) => {
                    let want_text = v[0].as_str().unwrap().to_string();
                    eq!(
                        (i32::from(got.value), got.text),
                        (want_i().unwrap(), want_text)
                    )
                }
                None => {
                    got.text.clear();
                    eq!(
                        (i32::from(got.value), got.text),
                        (want_i().unwrap(), String::new())
                    )
                }
            }
        }
        "RoundUpToNearest5" => {
            let (g, w) = check!(round_up_to_nearest5(arg(a, 0)), want_i().unwrap());
            eq!(g, w)
        }
        "Get_Enc_Ratio" => {
            let (g, w) = check!(get_enc_ratio(arg(a, 0), arg(a, 1), arg(a, 2)), cur(r));
            eq!(cur_out(g), w)
        }
        other => Err(format!("no port wired for {other}")),
    }
}

/// Runs every stats fixture case under `profile`; the layer-1 identity harness reuses it.
pub(crate) fn check_fixture(profile: Profile) {
    let fx = fixture::load("stats");
    let mut bad = Vec::new();
    for case in &fx.cases {
        if let Err(msg) = run(profile, case) {
            bad.push(format!(
                "{}: {msg}\n   inputs {}",
                case["id"], case["inputs"]
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} cases differ; first:\n{}",
        bad.len(),
        fx.cases.len(),
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn stats_fixture_cases() {
    check_fixture(Profile::Stock);
}
