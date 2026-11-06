// Fixture tests for the clipboard family: expected texts come from fixtures/rules/clipboard.json
// (frmMain CopyChartoClip, CopyMonsterToClipboard, CopyShopToClipboard and modMain CopyLVLinetoClipboard,
// run unchanged by the oracle).

use crate::fixture;
use serde_json::Value;
use touchstone_model::{Class, Race};
use touchstone_rules::Profile;
use touchstone_rules::clipboard::*;
use touchstone_rules::exp::calc_exp_needed_by_race_class;

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

fn ctl<'a>(frm: &'a Value, name: &str) -> &'a Value {
    &frm[name]
}

fn elem<'a>(frm: &'a Value, name: &str, i: usize) -> &'a Value {
    &frm[name]["$array"][i.to_string()]
}

fn lv_rows(spec: &Value) -> Vec<LvRow> {
    spec["rows"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|r| LvRow {
                    text: s(&r["Text"]),
                    subs: r["Sub"]
                        .as_array()
                        .map(|x| x.iter().map(s).collect())
                        .unwrap_or_default(),
                    selected: r["Sel"].as_bool().unwrap_or(false),
                    ..LvRow::default()
                })
                .collect()
        })
        .unwrap_or_default()
}

fn lv_cols(spec: &Value) -> Vec<LvColumn> {
    spec["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| LvColumn {
            text: s(&c[0]),
            width: c[1].as_i64().unwrap() as i32,
        })
        .collect()
}

fn char_input(frm: &Value, data: &Value) -> CharClip {
    let id = |name: &str| elem(frm, name, 0)["ItemData"]["2"].as_i64().unwrap() as i32;
    let level: i32 = s(&elem(frm, "txtGlobalLevel", 0)["Text"]).parse().unwrap();
    let table = |key: &str| -> Vec<(i32, i32)> {
        data[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["n"].as_i64().unwrap() as i32,
                    r["exp"].as_i64().unwrap() as i32,
                )
            })
            .collect()
    };
    let classes: Vec<Class> = table("classes")
        .into_iter()
        .map(|(n, e)| crate::exp::class(n, e))
        .collect();
    let races: Vec<Race> = table("races")
        .into_iter()
        .map(|(n, e)| crate::exp::race(n, e))
        .collect();
    let exp = calc_exp_needed_by_race_class(
        Profile::Stock,
        level,
        id("cmbGlobalClass"),
        id("cmbGlobalRace"),
        &classes,
        &races,
    )
    .unwrap();
    let tag = |name: &str| s(&ctl(frm, name)["Tag"]);
    let stats: Vec<String> = (0..47)
        .map(|i| s(&elem(frm, "lblInvenCharStat", i)["Caption"]))
        .collect();
    // lblLabelArray index of each stat's bonus: strength, intellect, willpower, agility, health, charm
    let label = [1, 4, 24, 2, 23, 3];
    let mut c = CharClip {
        name: s(&ctl(frm, "txtCharName")["Text"]),
        cp_tag: tag("lblStatCalc"),
        race: s(&elem(frm, "cmbGlobalRace", 0)["Text"]),
        class: s(&elem(frm, "cmbGlobalClass", 0)["Text"]),
        level: level.to_string(),
        exp,
        max_hp_tag: tag("lblCharMaxHP"),
        ac_tag: s(&elem(frm, "lblInvenCharStat", 2)["Tag"]),
        dr_tag: s(&elem(frm, "lblInvenCharStat", 3)["Tag"]),
        enc_tag: s(&elem(frm, "lblEncumLevel", 1)["Tag"]),
        stats,
        ..CharClip::default()
    };
    if elem(frm, "fraChar", 2)["Enabled"].as_bool() == Some(true) {
        c.mana = Some((tag("lblCharMaxMana"), tag("lblCharSC")));
    }
    if elem(frm, "fraChar", 3)["Enabled"].as_bool() == Some(true) {
        c.picklocks = Some(tag("lblCharPicklocks"));
    }
    for (i, &lab) in label.iter().enumerate() {
        c.stat_tags[i] = s(&elem(frm, "txtCharStats", i)["Tag"]);
        c.stat_bonus[i] = s(&elem(frm, "lblLabelArray", lab)["Tag"]).parse().unwrap();
    }
    for x in 0..EQUIP_SLOTS {
        let e = elem(frm, "cmbEquip", x);
        if e["ListIndex"].as_i64() == Some(1) {
            c.equip[x] = Some(s(&e["Text"]));
        }
    }
    c
}

fn run(case: &Value, data: &Value) -> Clip {
    let frm = &case["inputs"]["forms"]["frmMain"];
    let step = &case["inputs"]["steps"][0];
    let args = step["args"].as_array().cloned().unwrap_or_default();
    let flag = |i: usize| args.get(i).and_then(Value::as_bool).unwrap_or(false);
    match step["call"].as_str().unwrap() {
        "CopyChartoClip" => char_to_clip(&char_input(frm, data), flag(0)),
        "CopyMonsterToClipboard" => monster_to_clip(&lv_rows(&args[0]["cliplv"]), flag(1)),
        "CopyShopToClipboard" => {
            let shops = &ctl(frm, "lvShops")["$cliplv"];
            let selected = lv_rows(shops).into_iter().find(|r| r.selected);
            let loc = &ctl(frm, "lvShopLoc")["$cliplv"];
            let det = &ctl(frm, "lvShopDetail")["$cliplv"];
            let clip = ShopClip {
                selected,
                shop_columns: lv_cols(shops).into_iter().map(|c| c.text).collect(),
                detail: s(&ctl(frm, "txtShopDetail")["Text"]),
                loc_header: lv_cols(loc)[0].text.clone(),
                locs: lv_rows(loc).into_iter().map(|r| r.text).collect(),
                detail_columns: lv_cols(det).into_iter().map(|c| c.text).collect(),
                detail_rows: lv_rows(det),
            };
            shop_to_clip(&clip, flag(0))
        }
        "CopyLVLinetoClipboard" => {
            let lv = &args[0]["cliplv"];
            let clip = LinesClip {
                list: s(&lv["name"]),
                columns: lv_cols(lv),
                rows: lv_rows(lv),
                detail: args[1]["textbox"].as_str().map(str::to_string),
                locations: args[2]["cliplv"]
                    .is_object()
                    .then(|| lv_rows(&args[2]["cliplv"])),
                exclude_column: args[3].as_i64().unwrap() as i32,
            };
            lines_to_clip(&clip, flag(4))
        }
        p => panic!("unknown proc {p}"),
    }
}

#[test]
fn clipboard_texts_match_the_oracle() {
    let fx = fixture::load("clipboard");
    let data = &fx.header["data"];
    let mut bad = Vec::new();
    for case in &fx.cases {
        let got = run(case, data);
        let want_text = case["outputs"]["clip"].as_str().unwrap_or("");
        let want_err = case["events"][0]["number"].as_i64().map(|n| n as i32);
        // a failed step that left an earlier text (the equipment block after the character lines) keeps it
        if got.text.clone().unwrap_or_default() != want_text || got.error != want_err {
            bad.push(format!(
                "{}: got {:?} / {:?}, want {:?} / {:?}",
                case["id"], got.text, got.error, want_text, want_err
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} differ:\n{}",
        bad.len(),
        fx.cases.len(),
        bad.iter().take(5).cloned().collect::<Vec<_>>().join("\n")
    );
}
