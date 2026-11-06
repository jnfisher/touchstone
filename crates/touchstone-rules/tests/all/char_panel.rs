// Fixture tests for the char panel family (FAM-05): see docs/porting.md step 5.
// Expected values come from fixtures/rules/char_panel.json (frmMain's own procedures run by the oracle).
use serde_json::Value;
use touchstone_model::{AbilitySlot, Class, Info, Race, Spell, World, WorldParts};
use touchstone_rules::Profile;
use touchstone_rules::char_panel::*;
use touchstone_vb6 as vb;

use crate::fixture;

fn i(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
}

fn pairs<const N: usize>(v: &Value) -> [AbilitySlot; N] {
    let mut out = [AbilitySlot::default(); N];
    for (k, p) in v.as_array().unwrap().iter().enumerate() {
        out[k] = AbilitySlot {
            ability: i(&p[0]),
            value: i(&p[1]),
        };
    }
    out
}

/// The rows the cases use, from the fixture header's `data`.
fn world(data: &Value) -> World {
    let spells = data["spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Spell {
            number: i(&r["n"]),
            name: None,
            short: None,
            req_level: i(&r["req"]),
            energy_cost: 0,
            mana_cost: i(&r["mana"]),
            min_base: i(&r["mn"]),
            max_base: i(&r["mx"]),
            diff: 0,
            type_of_resists: 0,
            targets: 0,
            dur: i(&r["dur"]),
            att_type: 0,
            magery: 0,
            magery_lvl: 0,
            cap: i(&r["cap"]),
            max_inc_lvls: i(&r["mxl"]),
            max_inc: i(&r["mxi"]),
            min_inc_lvls: i(&r["mnl"]),
            min_inc: i(&r["mni"]),
            dur_inc_lvls: i(&r["dl"]),
            dur_inc: i(&r["di"]),
            abilities: pairs(&r["ab"]),
            learnable: false,
            learned_from: None,
            casted_by: None,
            classes: None,
        })
        .collect();
    let classes = data["classes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Class {
            number: i(&r["n"]),
            name: String::new(),
            min_hits: i(&r["min"]),
            max_hits: i(&r["max"]),
            exp_table: i(&r["exp"]),
            magery_type: i(&r["mt"]),
            magery_lvl: i(&r["ml"]),
            weapon_type: 0,
            armour_type: 0,
            combat_lvl: 0,
            abilities: pairs(&r["ab"]),
        })
        .collect();
    let races = data["races"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Race {
            number: i(&r["n"]),
            name: String::new(),
            m_int: 0,
            m_wil: 0,
            m_str: 0,
            m_hea: 0,
            m_agl: 0,
            m_chm: 0,
            x_int: 0,
            x_wil: 0,
            x_str: 0,
            x_hea: 0,
            x_agl: 0,
            x_chm: 0,
            hp_per_lvl: i(&r["hp"]),
            exp_table: i(&r["exp"]),
            base_cp: i(&r["cp"]),
            abilities: [AbilitySlot::default(); 10],
        })
        .collect();
    World::new(WorldParts {
        info: Info {
            nmr_version: String::new(),
            dat_file_version: String::new(),
            date: String::new(),
            time: String::new(),
            custom: String::new(),
            legit: false,
            disable_kai: false,
            update_url: String::new(),
        },
        items: vec![],
        monsters: vec![],
        spells,
        rooms: vec![],
        shops: vec![],
        classes,
        races,
        lairs: vec![],
        tbinfo: vec![],
    })
    .unwrap()
}

/// frmMain's controls: the case's, falling back to the setup's.
struct Forms {
    setup: Value,
    case: Value,
}

impl Forms {
    fn new(header: &Value, case: &Value) -> Self {
        Self {
            setup: header["setup"]["forms"]["frmMain"].clone(),
            case: case["inputs"]["forms"]["frmMain"].clone(),
        }
    }
    fn ctl(&self, name: &str) -> &Value {
        let v = &self.case[name];
        if v.is_null() { &self.setup[name] } else { v }
    }
    fn elem(&self, name: &str, idx: usize) -> &Value {
        &self.ctl(name)["$array"][idx.to_string()]
    }
    /// `Val` of a property of a control-array element.
    fn val(&self, name: &str, idx: usize, prop: &str) -> f64 {
        vb::val(self.elem(name, idx)[prop].as_str().unwrap_or(""))
    }
    /// A class or race combo's `ItemData(ListIndex)`: `None` when `ListIndex < 0` or unset.
    fn combo(&self, name: &str) -> Option<i32> {
        let e = self.elem(name, 0);
        match e["ListIndex"].as_i64() {
            Some(0) => Some(0),
            Some(n) if n > 0 => Some(i(&e["ItemData"][n.to_string()])),
            _ => None,
        }
    }
    fn level(&self) -> f64 {
        self.val("txtGlobalLevel", 0, "Text")
    }
}

fn want<'a>(case: &'a Value, key: &str) -> &'a str {
    case["outputs"][key]
        .as_str()
        .unwrap_or_else(|| panic!("no output {key}"))
}

/// An output the port leaves `None` where MME had not assigned it (the oracle reads "").
fn check_opt(name: &str, got: Option<String>, want: &str) -> Result<(), String> {
    let g = got.unwrap_or_default();
    if g == want {
        Ok(())
    } else {
        Err(format!("{name}: got {g:?}, want {want:?}"))
    }
}

fn check_error(case: &Value, source: &str, got: Option<i32>) -> Result<(), String> {
    let want = case["events"].as_array().and_then(|a| {
        a.iter().find_map(|e| {
            (e["kind"] == "handled_error" && e["source"] == source).then(|| i(&e["number"]))
        })
    });
    if got == want {
        Ok(())
    } else {
        Err(format!("error: got {got:?}, want {want:?}"))
    }
}

fn some<T: ToString>(v: Option<T>) -> Option<String> {
    v.map(|x| x.to_string())
}

fn run_cps(world: &World, f: &Forms, case: &Value) -> Result<(), String> {
    let mut stat_texts = [0.0; 6];
    let mut max_tags = [0.0; 6];
    for k in 0..6 {
        stat_texts[k] = f.val("txtCharStats", k, "Text");
        max_tags[k] = f.val("txtCharMaxStats", k, "Tag");
    }
    let got = refresh_cps(&CpInput {
        profile: Profile::Stock,
        world,
        stat_texts,
        max_tags,
        race: f.combo("cmbGlobalRace"),
        class: f.combo("cmbGlobalClass").unwrap_or(0),
        level: f.level(),
    });
    // the exp overflow is swallowed by CalcExpNeededByRaceClass's own handler
    check_error(case, "RefreshCPs", got.error)?;
    let w = want(case, "cps");
    let Some(s) = &got.summary else {
        return check_opt("cps", None, w);
    };
    check_opt("cps_tag", some(Some(s.avail)), want(case, "cps_tag"))?;
    let head = s.caption_head();
    let Some(tail) = w.strip_prefix(&head) else {
        return Err(format!("cps head: got {head:?}, want {w:?}"));
    };
    match s.exp {
        ExpRequired::NotShown if tail.is_empty() => Ok(()),
        ExpRequired::ALot if tail == "\r\nEXP Req: a lot." => Ok(()),
        ExpRequired::Value(v) => {
            let Some(text) = tail.strip_prefix("\r\nEXP Req: ") else {
                return Err(format!("cps exp line missing: {w:?}"));
            };
            // PutCommas(x, True): thousands commas, or trillions with 3 decimals
            if let Some(t) = text.strip_suffix('T') {
                let t: f64 = t.replace(',', "").parse().unwrap();
                if (t - v / 1e12).abs() < 0.0006 {
                    return Ok(());
                }
            } else if text.replace(',', "") == vb::cstr(v) {
                return Ok(());
            }
            Err(format!("cps exp: got {v}, want {text:?}"))
        }
        _ => Err(format!("cps exp: got {:?}, want tail {tail:?}", s.exp)),
    }
}

fn run_hp(world: &World, f: &Forms, case: &Value) -> Result<(), String> {
    let got = refresh_hit_points(
        Profile::Stock,
        &HpInput {
            world,
            level: f.level(),
            class: f.combo("cmbGlobalClass").unwrap_or(0),
            health: f.val("txtCharStats", 4, "Tag"),
            hp_regen: vb::val(f.ctl("txtCharHPRegen")["Text"].as_str().unwrap_or("")),
            hp_bonus: f.val("lblInvenCharStat", 5, "Caption"),
        },
    );
    check_error(case, "CalcRestingRatePrivate", got.error)?;
    check_opt("rest_tag", some(got.rest_resting), want(case, "rest_tag"))?;
    check_opt("rest_cap", got.rest_caption, want(case, "rest_cap"))?;
    check_opt(
        "hp_tag",
        got.range.as_ref().map(|r| vb::cstr(r.average)),
        want(case, "hp_tag"),
    )?;
    check_opt("hp_cap", got.max_hp_caption, want(case, "hp_cap"))
}

fn run_magic(world: &World, f: &Forms, case: &Value) -> Result<(), String> {
    let got = refresh_magic(&MagicInput {
        profile: Profile::Stock,
        world,
        level: f.level(),
        class: f.combo("cmbGlobalClass").unwrap_or(0),
        intellect: f.val("txtCharStats", 1, "Tag"),
        willpower: f.val("txtCharStats", 2, "Tag"),
        charm: f.val("txtCharStats", 5, "Tag"),
        mana_regen_pct: vb::val(f.ctl("txtCharManaRegen")["Text"].as_str().unwrap_or("")),
        mana_bonus: f.val("lblInvenCharStat", 6, "Caption"),
        spellcasting_bonus: f.val("lblInvenCharStat", 9, "Caption"),
    });
    check_error(case, "RefreshMagic", got.error)?;
    check_opt(
        "mana_rate_tag",
        some(got.mana_regen),
        want(case, "mana_rate_tag"),
    )?;
    check_opt(
        "mana_rate_cap",
        got.mana_regen_caption,
        want(case, "mana_rate_cap"),
    )?;
    check_opt("medi", got.medi_ticks.map(vb::cstr), want(case, "medi"))?;
    check_opt("maxmana_tag", some(got.max_mana), want(case, "maxmana_tag"))?;
    check_opt(
        "maxmana_cap",
        got.max_mana_caption,
        want(case, "maxmana_cap"),
    )?;
    check_opt("sc_tag", some(got.spellcasting), want(case, "sc_tag"))?;
    check_opt("sc_cap", got.spellcasting_caption, want(case, "sc_cap"))
}

fn run_picklocks(f: &Forms, case: &Value) -> Result<(), String> {
    let got = refresh_picklocks(
        Profile::Stock,
        f.level(),
        f.val("txtCharStats", 3, "Tag"),
        f.val("txtCharStats", 1, "Tag"),
        f.val("txtCharStats", 5, "Tag"),
        f.val("lblInvenCharStat", 22, "Caption"),
    );
    check_error(case, "RefreshPicklocks", got.error)?;
    check_opt("pick_tag", some(got.total), want(case, "pick_tag"))?;
    check_opt("pick_cap", got.caption, want(case, "pick_cap"))
}

fn run_mr(f: &Forms, case: &Value) -> Result<(), String> {
    let plus = case["inputs"]["globals"]["nGlobalCharPlusMR"]
        .as_i64()
        .unwrap_or(0) as i32;
    let got = refresh_magic_res(
        Profile::Stock,
        f.val("txtCharStats", 1, "Tag"),
        f.val("txtCharStats", 2, "Tag"),
        plus,
    );
    check_error(case, "RefreshMagicRes", got.err())?;
    check_opt("mr", got.ok().map(|v| v.to_string()), want(case, "mr"))
}

fn run_dodge(f: &Forms, case: &Value) -> Result<(), String> {
    let tag = f.elem("lblInvenCharStat", 8)["Tag"].as_str().map(vb::val);
    let got = refresh_dodge(tag);
    let w = want(case, "dodge_cap");
    if got.has_summary != w.contains("(After AC)") || !w.starts_with(&got.caption_head) {
        return Err(format!(
            "dodge caption: got {:?}, want {w:?}",
            got.caption_head
        ));
    }
    let t = want(case, "dodge_tag");
    if vb::cstr(got.value) == t {
        Ok(())
    } else {
        Err(format!("dodge tag: got {}, want {t}", got.value))
    }
}

fn run_labels(f: &Forms, case: &Value) -> Result<(), String> {
    const IDX: [usize; 6] = [1, 4, 24, 2, 23, 3];
    let mut bonus = [0.0; 6];
    for k in 0..6 {
        bonus[k] = f.val("lblLabelArray", IDX[k], "Tag");
    }
    let got = main_stat_labels(&bonus);
    check_error(case, "RefreshMainStatBonuses", got.error)?;
    let want = case["outputs"]["labels"].as_array().unwrap();
    for k in 0..6 {
        check_opt(
            &format!("label {k}"),
            got.labels[k].clone(),
            want[IDX[k]].as_str().unwrap(),
        )?;
    }
    Ok(())
}

fn run_setup(world: &World, f: &Forms, case: &Value) -> Result<(), String> {
    let o = &case["outputs"];
    let e = f.elem("cmbGlobalClass", 0);
    let mut quest_options = [0; 4];
    for (k, q) in quest_options.iter_mut().enumerate() {
        *q = f.elem("cmbCharQuestOpts", k)["ListIndex"].as_i64().unwrap() as i32;
    }
    let chk = |k: usize| f.elem("chkCharQuests", k)["Value"].as_i64() == Some(1);
    let input = ClassSetupInput {
        world,
        class: f.combo("cmbGlobalClass"),
        none_text: e["Text"].as_str() == Some("(none)"),
        quest_checks: [chk(5), chk(9), chk(10), chk(11)],
        quest_options,
        magery_level_count: f.ctl("cmbSpellMageryLevel")["ListCount"].as_i64().unwrap() as i32,
        magery_count: f.ctl("cmbSpellMagery")["ListCount"].as_i64().unwrap() as i32,
    };
    let opts_want: Vec<i32> = o["qopts"].as_array().unwrap().iter().map(i).collect();
    let Some(got) = setup_class(&input) else {
        // ListIndex < 0: nothing is touched (the sentinel 5 stays in the check boxes)
        if opts_want != quest_options || i(&o["antimagic"]) != 5 {
            return Err("no-class run changed something".into());
        }
        return Ok(());
    };
    let flag = |b: bool| i32::from(b);
    let mut checks: Vec<(&str, i32, i32)> = vec![
        ("antimagic", flag(got.anti_magic), i(&o["antimagic"])),
        ("nonmagic", flag(got.anti_magic), i(&o["nonmagic"])),
        ("weaponopt", flag(got.anti_magic), i(&o["weaponopt"])),
        ("spellopt", flag(got.has_magic), i(&o["spellopt"])),
        ("mageryLVL", got.magery_lvl, i(&o["mageryLVL"])),
        ("magery", got.magery, i(&o["magery"])),
        (
            "mlvl_idx",
            got.magery_level_index.unwrap_or(-1),
            i(&o["mlvl_idx"]),
        ),
        (
            "magery_idx",
            got.magery_index.unwrap_or(-1),
            i(&o["magery_idx"]),
        ),
        (
            "lblpick",
            flag(got.has_picks),
            flag(o["lblpick"].as_bool().unwrap()),
        ),
        (
            "lblsc",
            flag(got.has_magic),
            flag(o["lblsc"].as_bool().unwrap()),
        ),
        (
            "fra2",
            flag(got.has_magic),
            flag(o["fra"][0].as_bool().unwrap()),
        ),
        (
            "fra3",
            flag(got.has_picks),
            flag(o["fra"][1].as_bool().unwrap()),
        ),
    ];
    for (g, w) in got.quest_options.iter().zip(&opts_want) {
        checks.push(("qopts", *g, *w));
    }
    for k in 0..7 {
        checks.push(("armour", flag(got.class_filters_ticked), i(&o["armour"][k])));
    }
    for k in 0..4 {
        checks.push(("handed", flag(got.class_filters_ticked), i(&o["handed"][k])));
    }
    for (name, g, w) in checks {
        if g != w {
            return Err(format!("{name}: got {g}, want {w}"));
        }
    }
    Ok(())
}

fn run_encum(f: &Forms, case: &Value) -> Result<(), String> {
    let cur = f.val("lblInvenCharStat", 0, "Caption");
    let max = f.val("lblInvenCharStat", 1, "Caption");
    match encum_level(cur, max) {
        Ok(g) => {
            check_opt("lvl_cap", Some(g.caption), want(case, "lvl_cap"))?;
            check_opt("lvl_tag", Some(g.tag), want(case, "lvl_tag"))
        }
        Err(e) => {
            // MME's handler leaves the box as it was (empty here)
            check_error(case, "lblInvenCharStat_Change", Some(e))?;
            check_opt("lvl_cap", None, want(case, "lvl_cap"))
        }
    }
}

fn run_adj(f: &Forms, case: &Value) -> Result<(), String> {
    let index = case["inputs"]["steps"][0]["args"][0].as_u64().unwrap() as usize;
    let current = i(&case["inputs"]["globals"]["char_StatAdjustments"][index]);
    let answer = f.ctl("tsInput")["Caption"].as_str().unwrap_or("");
    let got = stat_adjustment_answer(index, current, answer);
    let adj = case["outputs"]["adj"].as_array().unwrap();
    let dirty = case["outputs"]["dirty"].as_bool().unwrap();
    let (want_adj, want_dirty) = match got {
        AdjustmentOutcome::Changed(n) => (n, true),
        _ => (current, false),
    };
    if i(&adj[index]) != want_adj || dirty != want_dirty {
        return Err(format!("{got:?}: oracle adj {} dirty {dirty}", adj[index]));
    }
    Ok(())
}

fn run_bless(world: &World, f: &Forms, case: &Value) -> Result<(), String> {
    let mut slots = [0; 10];
    for (k, s) in slots.iter_mut().enumerate() {
        let e = f.elem("cmbCharBless", k);
        if !e.is_null() {
            *s = i(&e["ItemData"][0]);
        }
    }
    let got = bless_mana_per_round(Profile::Stock, world, f.level(), &slots);
    check_opt("mana", got.map(vb::cstr), want(case, "mana"))
}

fn run(world: &World, header: &Value, case: &Value) -> Result<(), String> {
    let f = Forms::new(header, case);
    match case["inputs"]["steps"][0]["call"].as_str().unwrap() {
        "RefreshCPs" => run_cps(world, &f, case),
        "RefreshHitPoints" => run_hp(world, &f, case),
        "RefreshMagic" => run_magic(world, &f, case),
        "RefreshPicklocks" => run_picklocks(&f, case),
        "RefreshMagicRes" => run_mr(&f, case),
        "RefreshDodge" => run_dodge(&f, case),
        "RefreshMainStatBonuses" => run_labels(&f, case),
        "SetupClass" => run_setup(world, &f, case),
        "lblInvenCharStat_Change" => run_encum(&f, case),
        "CharStatAdjustmentPrompt" => run_adj(&f, case),
        "RefreshCharBless" => run_bless(world, &f, case),
        other => Err(format!("unknown call {other}")),
    }
}

#[test]
fn char_panel_cases_match_the_oracle() {
    let fx = fixture::load("char_panel");
    let world = world(&fx.header["data"]);
    let mut failures = Vec::new();
    for case in &fx.cases {
        if let Err(msg) = run(&world, &fx.header, case) {
            failures.push(format!("{}: {msg}", case["id"].as_str().unwrap()));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ; first: {:#?}",
        failures.len(),
        fx.cases.len(),
        &failures[..failures.len().min(25)]
    );
}
