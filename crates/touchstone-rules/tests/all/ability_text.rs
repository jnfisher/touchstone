// Fixture tests for the ability text family (FAM-03 text half): see docs/porting.md step 5.
use crate::fixture::{self, Names, Textblocks};
use serde_json::Value;
use std::cell::Cell;
use touchstone_rules::Profile;
use touchstone_rules::ability_text::*;
use touchstone_rules::spells::PullArgs;

fn i(v: &Value) -> i64 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}"))
}

fn is_missing(v: &Value) -> bool {
    v["t"] == "missing"
}

fn flag(args: &[Value], k: usize) -> bool {
    args.get(k)
        .filter(|v| !is_missing(v))
        .is_some_and(|v| v.as_bool().unwrap())
}

#[test]
fn ability_text_fixture() {
    let fx = fixture::load("ability_text");
    let table = &fx.cases.iter().find(|c| c["id"] == "table").unwrap()["inputs"];
    let spells = fixture::spell_table(&fx.cases[0]);
    assert!(spells.len() > 1000);
    let names = Names::from_table(&table["names"]);
    let tbs = Textblocks::from_table(&table["textblocks"]);

    let mut fails = Vec::new();
    let mut ran = 0;
    for c in &fx.cases {
        let id = c["id"].as_str().unwrap();
        if id == "table" {
            continue;
        }
        let inp = &c["inputs"];
        let a = inp["args"].as_array().unwrap();
        let proc = inp["proc"].as_str().unwrap();
        let want = &c["outputs"]["result"];
        assert!(c.get("error").is_none(), "{id}: oracle error");
        let frm = &inp["forms"]["frmMain"];
        let env = AbilityEnv {
            profile: Profile::Stock,
            names: &names,
            textblocks: &tbs,
            spells: &spells,
            hide_record_numbers: inp["globals"]["bHideRecordNumbers"] == true,
            quick_spell: inp["globals"]["bQuickSpell"] == true,
            global_level_text: (frm["chkGlobalFilter"]["Value"] == 1).then(|| {
                frm["txtGlobalLevel"]["$array"]["0"]["Text"]
                    .as_str()
                    .unwrap_or("")
            }),
            avg_level_max_all_stats: 0.0,
            nest: Cell::new(inp["globals"]["nSpellNest"].as_i64().unwrap_or(0) as i32),
        };
        ran += 1;
        let got: Value = match proc {
            "GetAbilityName" => {
                get_ability_name(Profile::Stock, i(&a[0]) as i16, flag(a, 1)).into()
            }
            "GetAbilityList" => get_ability_list(Profile::Stock, env.hide_record_numbers).into(),
            "GetAbilityStats" => env
                .get_ability_stats(
                    i(&a[0]) as i16,
                    i(&a[1]) as i16,
                    a.get(3)
                        .is_none_or(|v| is_missing(v) || v.as_bool().unwrap()),
                    flag(a, 4),
                )
                .into(),
            "GetAbilDiffText" => env
                .get_abil_diff_text(
                    i(&a[0]) as i32,
                    i(&a[1]) as i32,
                    i(&a[2]) as i32,
                    i(&a[4]) as i16,
                    flag(a, 5),
                )
                .into(),
            other => panic!("{id}: unknown proc {other}"),
        };
        if got != *want {
            fails.push(format!("{id}: got {got}, want {want}; inputs {inp}"));
        }
    }
    assert!(ran > 5000, "only {ran} cases ran");
    assert!(
        fails.is_empty(),
        "{} of {ran} cases failed, first: {:#?}",
        fails.len(),
        &fails[..fails.len().min(8)]
    );
}

// An empty database: every name lookup answers with the number, PullSpellEQ with its placeholder.
#[test]
fn empty_tables() {
    let names = Names::default();
    let tbs = Textblocks::default();
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
    assert_eq!(env.get_ability_stats(42, 7, true, false), "LearnSp (7)");
    assert_eq!(env.get_ability_stats(59, 0, true, false), "ClassOk");
    assert_eq!(env.get_ability_stats(59, 3, true, false), "ClassOk 3");
    assert_eq!(
        env.get_ability_stats(43, 7, true, false),
        "CastsSp [7, (No Spell Records)]"
    );
    assert_eq!(
        env.pull_spell_eq(&PullArgs::default()),
        "(No Spell Records)"
    );
    assert_eq!(env.nest.get(), 0);
}
