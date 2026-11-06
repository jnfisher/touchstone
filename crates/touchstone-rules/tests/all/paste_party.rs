// Fixture tests for the paste-party family (FAM-25): see docs/porting.md step 5.
// Expected values come from fixtures/rules/paste_party.json (frmPasteChar's ParsePasteParty and
// CalculateAverageParty run by the oracle). The tables are the "data" key of the fixture header.
use serde_json::Value;
use touchstone_model::{AbilitySlot, Class, Item, Race, Spell};
use touchstone_rules::Profile;
use touchstone_rules::paste_party::*;
use touchstone_vb6 as vb;

use crate::fixture;

fn i(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
}

fn slots<const N: usize>(flat: &Value) -> [AbilitySlot; N] {
    let mut out = [AbilitySlot::default(); N];
    for (k, p) in flat.as_array().unwrap().chunks(2).enumerate().take(N) {
        out[k] = AbilitySlot {
            ability: i(&p[0]),
            value: i(&p[1]),
        };
    }
    out
}

fn item(r: &Value) -> Item {
    Item {
        number: i(&r[0]),
        name: r[1].as_str().unwrap().to_string(),
        limit: 0,
        encum: i(&r[2]),
        item_type: i(&r[11]),
        use_count: 0,
        price: 0,
        currency: 0,
        min: i(&r[6]),
        max: i(&r[7]),
        armour_class: 0,
        damage_resist: 0,
        weapon_type: i(&r[8]),
        armour_type: 0,
        worn: i(&r[10]),
        accy: i(&r[3]),
        gettable: true,
        str_req: i(&r[4]),
        speed: i(&r[5]),
        not_droppable: false,
        destroy_on_death: false,
        retain_after_uses: false,
        class_rest: [0; 10],
        race_rest: [0; 10],
        negate_spell: [0; 10],
        abilities: slots(&r[9]),
        in_game: i(&r[12]) != 0,
        obtained_from: None,
        references: None,
    }
}

fn class(r: &Value) -> Class {
    Class {
        number: i(&r[0]),
        name: r[1].as_str().unwrap().to_string(),
        min_hits: 0,
        max_hits: 0,
        exp_table: 0,
        magery_type: i(&r[3]),
        magery_lvl: i(&r[4]),
        weapon_type: 0,
        armour_type: 0,
        combat_lvl: i(&r[2]),
        abilities: slots(&r[5]),
    }
}

fn race(r: &Value) -> Race {
    Race {
        number: i(&r[0]),
        name: r[1].as_str().unwrap().to_string(),
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
        hp_per_lvl: 0,
        exp_table: 0,
        base_cp: 0,
        abilities: slots(&r[2]),
    }
}

/// Spell rows are written in the order of the data's `spell_keys`.
fn spell(r: &Value, keys: &[&str]) -> Spell {
    let at = |k: &str| &r[keys.iter().position(|x| *x == k).unwrap()];
    let text = |k: &str| at(k).as_str().unwrap().to_string();
    Spell {
        number: i(at("Number")),
        name: Some(text("Name")),
        short: Some(text("Short")),
        req_level: i(at("ReqLevel")),
        energy_cost: i(at("EnergyCost")),
        mana_cost: i(at("ManaCost")),
        min_base: i(at("MinBase")),
        max_base: i(at("MaxBase")),
        diff: i(at("Diff")),
        type_of_resists: i(at("TypeOfResists")),
        targets: i(at("Targets")),
        dur: i(at("Dur")),
        att_type: i(at("AttType")),
        magery: i(at("Magery")),
        magery_lvl: i(at("MageryLVL")),
        cap: i(at("Cap")),
        max_inc_lvls: i(at("MaxIncLVLs")),
        max_inc: i(at("MaxInc")),
        min_inc_lvls: i(at("MinIncLVLs")),
        min_inc: i(at("MinInc")),
        dur_inc_lvls: i(at("DurIncLVLs")),
        dur_inc: i(at("DurInc")),
        abilities: slots(&r[keys.len()]),
        learnable: i(at("Learnable")) != 0,
        learned_from: Some(text("Learned From")),
        casted_by: Some(text("Casted By")),
        classes: Some(text("Classes")),
    }
}

/// The control `name(idx)` as the case leaves it: the case's own spec replaces the family setup's.
fn control<'a>(
    fx: &'a fixture::Fixture,
    case: &'a Value,
    name: &str,
    idx: usize,
) -> Option<&'a Value> {
    let spec = case["inputs"]["forms"]["frmMain"]
        .get(name)
        .or_else(|| fx.header["setup"]["forms"]["frmMain"].get(name))?;
    spec.get("$array")
        .map_or(Some(spec), |a| a.get(idx.to_string()))
}

fn text_of(fx: &fixture::Fixture, case: &Value, name: &str, idx: usize) -> String {
    control(fx, case, name, idx)
        .and_then(|c| c.get("Text"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

const TEXTS: [&str; 13] = [
    "txtPastePartyName",
    "txtPastePartyAC",
    "txtPastePartyDR",
    "txtPastePartyMR",
    "txtPastePartyDodge",
    "txtPastePartyHitpoints",
    "txtPastePartyRestHP",
    "txtPastePartyRegenHP",
    "txtPastePartyACCY",
    "txtPastePartyHeals",
    "txtPastePartyDMG",
    "txtPastePartySpellDMG",
    "txtPastePartySwings",
];

fn column<'a>(f: &'a mut PartyForm, name: &str) -> &'a mut [String; SLOTS] {
    match name {
        "txtPastePartyName" => &mut f.name,
        "txtPastePartyAC" => &mut f.ac,
        "txtPastePartyDR" => &mut f.dr,
        "txtPastePartyMR" => &mut f.mr,
        "txtPastePartyDodge" => &mut f.dodge,
        "txtPastePartyHitpoints" => &mut f.hitpoints,
        "txtPastePartyRestHP" => &mut f.rest_hp,
        "txtPastePartyRegenHP" => &mut f.regen_hp,
        "txtPastePartyACCY" => &mut f.accy,
        "txtPastePartyHeals" => &mut f.heals,
        "txtPastePartyDMG" => &mut f.dmg,
        "txtPastePartySpellDMG" => &mut f.spell_dmg,
        _ => &mut f.swings,
    }
}

fn form_before(fx: &fixture::Fixture, case: &Value) -> PartyForm {
    let mut f = PartyForm::default();
    for name in TEXTS {
        for k in 0..SLOTS {
            column(&mut f, name)[k] = text_of(fx, case, name, k);
        }
    }
    for k in 0..SLOTS {
        f.anti_magic[k] = control(fx, case, "chkPastePartyAM", k)
            .and_then(|c| c.get("Value"))
            .and_then(Value::as_i64)
            .unwrap_or(0) as i16;
        if k >= 1 {
            let c = control(fx, case, "optPastyPartyAtkLast", k);
            f.atk_last[k] = c.and_then(|c| c.get("Value")).and_then(Value::as_bool) == Some(true);
            f.atk_last_tag[k] = c
                .and_then(|c| c.get("Tag"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
        }
    }
    f.am_total = control(fx, case, "txtPastePartyAMTotal", 0)
        .and_then(|c| c.get("Text"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    f.party_total = control(fx, case, "txtPastePartyPartyTotal", 0)
        .and_then(|c| c.get("Text"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    f.hold_refresh =
        global(fx, case, "bHoldPartyRefresh").is_some_and(|v| v.as_bool() == Some(true));
    f
}

fn global<'a>(fx: &'a fixture::Fixture, case: &'a Value, name: &str) -> Option<&'a Value> {
    case["inputs"]["globals"]
        .get(name)
        .or_else(|| fx.header["setup"]["globals"].get(name))
}

fn f64_of(v: Option<&Value>) -> f64 {
    v.and_then(Value::as_f64).unwrap_or(0.0)
}

fn flag(v: Option<&Value>) -> bool {
    v.and_then(Value::as_bool) == Some(true)
}

fn n(x: impl std::fmt::Display) -> String {
    x.to_string()
}

fn d(x: f64) -> String {
    vb::cstr(x)
}

fn probe(m: &Member) -> String {
    [
        n(m.weapon_num),
        n(m.weapon_speed),
        n(m.weapon_str),
        d(m.accy_worn),
        n(m.accy_abil),
        n(m.plus_dodge),
        n(m.plus_regen),
        n(m.phys_damage),
        d(m.swings),
        n(m.spell_damage),
        d(m.hp),
        d(m.hp_regen),
        d(m.mana_regen),
        n(m.encum_pct),
        d(m.accuracy),
        n(m.level),
        n(m.class),
        n(m.race),
        n(m.combat),
        n(m.str_),
        n(m.agi),
        n(m.cha),
        n(m.wis),
        n(m.int),
        n(m.hea),
        n(m.spellcasting),
        d(m.max_mana),
        n(m.crit),
        n(m.dodge),
        n(m.plus_max_damage),
        n(m.plus_bs_accy),
        n(m.plus_bs_min_dmg),
        n(m.plus_bs_max_dmg),
        n(m.stealth),
        n(m.ma_skill[0]),
        n(m.ma_skill[1]),
        n(m.ma_skill[2]),
        n(m.ma_accy[0]),
        n(m.ma_accy[1]),
        n(m.ma_accy[2]),
        n(m.ma_dmg[0]),
        n(m.ma_dmg[1]),
        n(m.ma_dmg[2]),
    ]
    .join("|")
}

fn equip(m: &Member) -> String {
    let mut s = String::new();
    for l in &m.equip_loc {
        s.push_str(l);
        s.push('~');
    }
    s.push_str(&m.worn[0]);
    s.push('~');
    s.push_str(&m.worn[1]);
    s
}

#[test]
fn paste_party_fixture() {
    check_fixture(Profile::Stock);
}

/// Shared with the Quicksilver identity harness.
pub(crate) fn check_fixture(profile: Profile) {
    let fx = fixture::load("paste_party");
    let data = &fx.header["data"];
    let items: Vec<Item> = data["items"].as_array().unwrap().iter().map(item).collect();
    let classes: Vec<Class> = data["classes"]
        .as_array()
        .unwrap()
        .iter()
        .map(class)
        .collect();
    let races: Vec<Race> = data["races"].as_array().unwrap().iter().map(race).collect();
    let keys: Vec<&str> = data["spell_keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k.as_str().unwrap())
        .collect();
    let spells: Vec<Spell> = data["spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| spell(r, &keys))
        .collect();
    let listing = |names: Vec<(String, i32)>| -> Vec<ListEntry> {
        std::iter::once(ListEntry {
            name: "Any".into(),
            number: 0,
        })
        .chain(
            names
                .into_iter()
                .map(|(name, number)| ListEntry { name, number }),
        )
        .collect()
    };
    let class_list = listing(classes.iter().map(|c| (c.name.clone(), c.number)).collect());
    let race_list = listing(races.iter().map(|r| (r.name.clone(), r.number)).collect());

    let mut bad: Vec<String> = Vec::new();
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        let step = &case["inputs"]["steps"][0];
        let out = &case["outputs"];
        let mut form = form_before(&fx, case);
        let mut diffs: Vec<String> = Vec::new();
        let mut done = false;
        let mut result = PasteOutcome::default();

        if step["call"] == "ParsePasteParty" {
            let text = case["inputs"]["forms"]["frmPasteChar"]["txtText"]["Text"]
                .as_str()
                .unwrap();
            let answers: Vec<Option<String>> = case["inputs"]
                .get("inputbox")
                .and_then(Value::as_array)
                .map(|a| a.iter().map(|x| x.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let env = PasteEnv {
                profile,
                items: &items,
                classes: &classes,
                races: &races,
                spells: &spells,
                class_list: &class_list,
                race_list: &race_list,
                only_in_game: flag(global(&fx, case, "bOnlyInGame")),
                use_second_wrist: flag(global(&fx, case, "bInvenUse2ndWrist")),
                nmr_ver: f64_of(global(&fx, case, "nNMRVer")),
                avg_level_max_all_stats: f64_of(global(&fx, case, "gAvgLevelMaxAllStats")),
                dat_ver_caption: "v1.11p",
                no_data_continue: false,
            };
            result = parse_paste_party(
                &env,
                &PasteInput {
                    text,
                    attack_answers: &answers,
                },
                &mut form,
            );
            done = result.status == PasteStatus::Done;
            let events = case
                .get("events")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let kind =
                |k: &str| -> Vec<&Value> { events.iter().filter(|e| e["kind"] == k).collect() };
            let msgs: Vec<String> = kind("msgbox")
                .iter()
                .map(|e| e["text"].as_str().unwrap().to_string())
                .collect();
            if msgs != result.messages {
                diffs.push(format!("messages {:?} vs {:?}", result.messages, msgs));
            }
            let boxes = kind("inputbox");
            if boxes.len() != result.prompts.len() {
                diffs.push(format!(
                    "prompts {} vs {}",
                    result.prompts.len(),
                    boxes.len()
                ));
            } else {
                for (b, p) in boxes.iter().zip(&result.prompts) {
                    let want = format!("Enter attack for {}\r\n\r\n", p.who);
                    if !b["prompt"].as_str().unwrap().starts_with(&want)
                        || b["default"] != p.default.as_str()
                    {
                        diffs.push(format!("prompt {:?} vs {:?}", p, b));
                    }
                }
            }
            if kind("handled_error").len() != usize::from(result.error) {
                diffs.push(format!(
                    "error flag {} vs {:?}",
                    result.error,
                    kind("handled_error")
                ));
            }
        } else {
            let what = step
                .get("args")
                .and_then(|a| a.get(0))
                .and_then(Value::as_i64)
                .unwrap_or(0);
            calculate_average_party(&mut form, what as i32);
        }

        let text_out = |name: &str| -> Vec<String> {
            out[name]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect()
        };
        for name in TEXTS {
            let got = column(&mut form, name).to_vec();
            if got != text_out(name) {
                diffs.push(format!("{name}: {got:?} vs {:?}", text_out(name)));
            }
        }
        let am: Vec<i64> = out["am"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap())
            .collect();
        let got_am: Vec<i64> = form.anti_magic.iter().map(|v| i64::from(*v)).collect();
        if am != got_am {
            diffs.push(format!("am {got_am:?} vs {am:?}"));
        }
        if form.am_total != out["amTotal"].as_str().unwrap() {
            diffs.push(format!("amTotal {:?} vs {}", form.am_total, out["amTotal"]));
        }
        if form.party_total != out["partyTotal"].as_str().unwrap() {
            diffs.push(format!(
                "partyTotal {:?} vs {}",
                form.party_total, out["partyTotal"]
            ));
        }
        let atk: Vec<bool> = out["atk"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_bool() == Some(true))
            .collect();
        if atk != form.atk_last[1..] {
            diffs.push(format!("atk {:?} vs {atk:?}", &form.atk_last[1..]));
        }
        let tags: Vec<String> = out["atkTag"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        if tags != form.atk_last_tag[1..] {
            diffs.push(format!("atkTag {:?} vs {tags:?}", &form.atk_last_tag[1..]));
        }

        if step["call"] == "ParsePasteParty" {
            for k in 1..=6 {
                let (want_p, want_e) = (
                    out["probe"][k - 1].as_str().unwrap(),
                    out["equip"][k - 1].as_str().unwrap(),
                );
                let (got_p, got_e) = if done {
                    (probe(&result.members[k]), equip(&result.members[k]))
                } else {
                    (String::new(), String::new())
                };
                if got_p != want_p {
                    diffs.push(format!("probe {k}: {got_p} vs {want_p}"));
                }
                if got_e != want_e {
                    diffs.push(format!("equip {k}: {got_e} vs {want_e}"));
                }
            }
            let got_x = if done {
                format!(
                    "{}|{}|{}|{}",
                    result.energy, result.combat, result.name_count, result.find_atk_last
                )
            } else {
                String::new()
            };
            if got_x != out["probeX"].as_str().unwrap() {
                diffs.push(format!("probeX {got_x} vs {}", out["probeX"]));
            }
        }
        if !diffs.is_empty() {
            bad.push(format!("{id}: {}", diffs.join(" ; ")));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} cases differ; first:\n{}",
        bad.len(),
        fx.cases.len(),
        bad.iter().take(6).cloned().collect::<Vec<_>>().join("\n")
    );
}
