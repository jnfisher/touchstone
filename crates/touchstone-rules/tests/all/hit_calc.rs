// Fixture tests for the Hit Calculator core (DoHitCalc, SetHitCalcVals, GetMonsterData,
// GetVileWardValue, ModifyHitCalcValues): see docs/porting.md step 5. Expected values come from
// fixtures/rules/hit_calc.json (MME's own procedures, cut out of frmHitCalc and run by the oracle).
use serde_json::Value;
use touchstone_model::{AbilitySlot, Class, Item, Monster, MonsterAttack};
use touchstone_rules::Profile;
use touchstone_rules::character::AttackType;
use touchstone_rules::hit_calc::*;
use touchstone_vb6 as vb;

use crate::fixture;

fn int(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an int: {v}")) as i32
}

fn string(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

/// A control property the oracle reads as true: Boolean true or a non-zero number.
fn truthy(v: &Value) -> bool {
    v.as_bool()
        .unwrap_or_else(|| v.as_i64().is_some_and(|n| n != 0))
}

fn slots<const N: usize>(flat: &[Value]) -> [AbilitySlot; N] {
    let mut out = [AbilitySlot::default(); N];
    for (i, pair) in flat.chunks(2).enumerate() {
        out[i] = AbilitySlot {
            ability: int(&pair[0]),
            value: pair.get(1).map_or(0, int),
        };
    }
    out
}

// [Number, StrReq, AbilitiesFlat]
fn item(row: &Value) -> Item {
    let r = row.as_array().unwrap();
    Item {
        number: int(&r[0]),
        name: String::new(),
        limit: 0,
        encum: 0,
        item_type: 1,
        use_count: 0,
        price: 0,
        currency: 0,
        min: 0,
        max: 0,
        armour_class: 0,
        damage_resist: 0,
        weapon_type: 0,
        armour_type: 0,
        worn: 0,
        accy: 0,
        gettable: true,
        str_req: int(&r[1]),
        speed: 0,
        not_droppable: false,
        destroy_on_death: false,
        retain_after_uses: false,
        class_rest: [0; 10],
        race_rest: [0; 10],
        negate_spell: [0; 10],
        abilities: slots(r[2].as_array().unwrap()),
        in_game: true,
        obtained_from: None,
        references: None,
    }
}

// [Number, [Abil-0..9]]
fn class(row: &Value) -> Class {
    let r = row.as_array().unwrap();
    let mut abilities = [AbilitySlot::default(); 10];
    for (i, a) in r[1].as_array().unwrap().iter().enumerate() {
        abilities[i].ability = int(a);
    }
    Class {
        number: int(&r[0]),
        name: String::new(),
        min_hits: 0,
        max_hits: 0,
        exp_table: 0,
        magery_type: 0,
        magery_lvl: 0,
        weapon_type: 0,
        armour_type: 0,
        combat_lvl: 0,
        abilities,
    }
}

// [Number, ArmourClass, BSDefense, Align, AbilitiesFlat, [AttType, AttAcc, Att%] x 5 flat]
fn monster(row: &Value) -> Monster {
    let r = row.as_array().unwrap();
    let att = r[5].as_array().unwrap();
    let attacks: [MonsterAttack; 5] = std::array::from_fn(|i| MonsterAttack {
        name: String::new(),
        att_type: int(&att[i * 3]),
        acc: int(&att[i * 3 + 1]),
        pct: int(&att[i * 3 + 2]),
        true_pct: 0.0,
        min: 0,
        max: 0,
        energy: 0,
        hit_spell: 0,
    });
    Monster {
        number: int(&r[0]),
        name: None,
        weapon: 0,
        armour_class: int(&r[1]),
        damage_resist: 0,
        follow_pct: 0,
        magic_res: 0,
        bs_defense: int(&r[2]),
        exp: 0.0,
        exp_multi: 0.0,
        hp: 0,
        energy: 0,
        avg_dmg: 0.0,
        avg_dmg_phys: 0.0,
        avg_dmg_spell: 0.0,
        greet_txt: 0,
        hp_regen: 0,
        charm_lvl: 0,
        r#type: 0,
        undead: 0,
        align: int(&r[3]),
        regen_time: 0,
        game_limit: 0,
        avg_lair_exp: 0.0,
        script_value: 0.0,
        r: 0,
        p: 0,
        g: 0,
        s: 0,
        c: 0,
        death_spell: 0,
        create_spell: 0,
        attacks,
        mid_spells: Default::default(),
        drops: Default::default(),
        abilities: slots(r[4].as_array().unwrap()),
        in_game: true,
        summoned_by: None,
    }
}

fn ctl<'a>(inputs: &'a Value, name: &str) -> &'a Value {
    &inputs["forms"]["frmMain"][name]
}

fn elem<'a>(inputs: &'a Value, name: &str, i: usize) -> &'a Value {
    &ctl(inputs, name)["$array"][i.to_string()]
}

fn tag(inputs: &Value, name: &str, i: usize) -> f64 {
    vb::val(string(&elem(inputs, name, i)["Tag"]))
}

fn radio(inputs: &Value, name: &str, n: usize) -> Option<usize> {
    (0..n).find(|&i| elem(inputs, name, i)["Value"] == Value::Bool(true))
}

fn global_i(inputs: &Value, name: &str) -> i32 {
    inputs["globals"]
        .get(name)
        .map_or(0, |v| v.as_bool().map_or_else(|| int(v), i32::from))
}

fn global_b(inputs: &Value, name: &str) -> bool {
    inputs["globals"].get(name).is_some_and(truthy)
}

fn form(inputs: &Value) -> HitCalcForm {
    let mut f = HitCalcForm {
        calc_type: radio(inputs, "optHitCalcType", 2)
            .map(|i| [HitCalcType::Normal, HitCalcType::Backstab][i]),
        attacker: radio(inputs, "optAttacker", 3)
            .map(|i| [Attacker::Character, Attacker::Monster, Attacker::Manual][i]),
        defender: radio(inputs, "optDefender", 4).map(|i| {
            [
                Defender::Character,
                Defender::Monster,
                Defender::Player,
                Defender::Manual,
            ][i]
        }),
        ..HitCalcForm::default()
    };
    for i in 0..6 {
        f.text[i] = string(&elem(inputs, "txtHitCalc", i)["Text"]).to_string();
    }
    let en = |i: usize| truthy(&elem(inputs, "txtHitCalc", i)["Enabled"]);
    f.prot_evil_enabled = en(3);
    f.vile_ward_enabled = en(4);
    f.percep_enabled = en(5);
    f.shadow = ctl(inputs, "chkShadow")["Value"].as_i64() == Some(1);
    f.shadow_enabled = truthy(&ctl(inputs, "chkShadow")["Enabled"]);
    f.see_hidden = ctl(inputs, "chkSeeHidden")["Value"].as_i64() == Some(1);
    f.see_hidden_enabled = truthy(&ctl(inputs, "chkSeeHidden")["Enabled"]);
    f.evil_index = ctl(inputs, "cmbEvil")["ListIndex"].as_i64().unwrap_or(-1) as i32;
    f.evil_enabled = truthy(&ctl(inputs, "cmbEvil")["Enabled"]);
    f.monster_list_enabled = truthy(&ctl(inputs, "cmbMonsterList")["Enabled"]);
    f.bs_label = string(&ctl(inputs, "lblBSPercep")["Caption"]).to_string();
    f.main_text = string(&ctl(inputs, "lblMain")["Caption"]).to_string();
    f.sub_left = string(&ctl(inputs, "lblSubLeft")["Caption"]).to_string();
    f.sub_right = string(&ctl(inputs, "lblSubRight")["Caption"]).to_string();
    f
}

fn state(inputs: &Value) -> HitCalcState {
    HitCalcState {
        monster_accy: global_i(inputs, "nMonsterAccy"),
        monster_ac: global_i(inputs, "nMonsterAC"),
        monster_dodge: global_i(inputs, "nMonsterDodge"),
        monster_bs_defense: global_i(inputs, "nMonsterBSdef"),
        monster_evil: global_b(inputs, "bMonsterEvil"),
        monster_see_hidden: global_b(inputs, "bMonsterSeeHidden"),
        defender_ac: global_i(inputs, "nDefenderAC"),
        defender_dodge: global_i(inputs, "nDefenderDodge"),
        defender_perception: global_i(inputs, "nDefenderPerception"),
        defender_prot_evil: global_i(inputs, "nDefenderProtEvil"),
        defender_vile_ward: global_i(inputs, "nDefenderVileWard"),
        defender_evilness: global_i(inputs, "nDefenderEvilness"),
        defender_shadow: global_b(inputs, "bDefenderShadow"),
        char_perception: global_i(inputs, "nCharPerception"),
    }
}

/// The class combo of the case: `cmbGlobalClass(0).ItemData(ListIndex)`, `None` for nothing selected.
fn class_selected(inputs: &Value) -> Option<i32> {
    let c = elem(inputs, "cmbGlobalClass", 0);
    let li = c["ListIndex"].as_i64()?;
    (li >= 0).then(|| int(&c["ItemData"][li as usize]))
}

fn character(inputs: &Value) -> HitCalcCharacter {
    let g = &inputs["globals"];
    let first = |name: &str| g.get(name).map_or(0, |a| int(&a[0]));
    HitCalcCharacter {
        accuracy: tag(inputs, "lblInvenCharStat", 10),
        ac: tag(inputs, "lblInvenCharStat", 2),
        dodge: tag(inputs, "lblInvenCharStat", 8),
        prot_evil: tag(inputs, "lblInvenCharStat", 20),
        stealth: tag(inputs, "lblInvenCharStat", 19),
        bs_accuracy: tag(inputs, "lblInvenCharStat", 13),
        agility: tag(inputs, "txtCharStats", 3),
        strength: tag(inputs, "txtCharStats", 0),
        level: vb::val(string(&elem(inputs, "txtGlobalLevel", 0)["Text"])),
        class: class_selected(inputs),
        shadow_ac: global_i(inputs, "nGlobalCharShadowAC"),
        accy_abilities: global_i(inputs, "nGlobalCharAccyAbils"),
        accy_other: global_i(inputs, "nGlobalCharAccyOther"),
        accy_items: global_i(inputs, "nGlobalCharAccyItems"),
        weapon_number: first("nGlobalCharWeaponNumber"),
        weapon_accy: first("nGlobalCharWeaponAccy"),
        weapon_bs_accy: first("nGlobalCharWeaponBSaccy"),
        attack_type: AttackType::from_code(global_i(inputs, "nGlobalAttackTypeMME")).unwrap(),
        attack_accy_adj: global_i(inputs, "nGlobalAttackAccyAdj") as i16,
        attack_backstab: global_b(inputs, "bGlobalAttackBackstab"),
        attack_backstab_weapon: global_i(inputs, "nGlobalAttackBackstabWeapon"),
        attack_martial_art: global_i(inputs, "nGlobalAttackMA") as i16,
        vile_ward: 0,
    }
}

fn handled(c: &Value) -> Vec<(String, u32)> {
    c.get("events")
        .and_then(Value::as_array)
        .map(|evs| {
            evs.iter()
                .filter(|e| e["kind"] == "handled_error")
                .map(|e| {
                    (
                        string(&e["source"]).to_string(),
                        e["number"].as_u64().unwrap() as u32,
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn same<T: PartialEq + std::fmt::Debug>(
    bad: &mut Vec<String>,
    id: &str,
    what: &str,
    got: T,
    want: T,
) {
    if got != want {
        bad.push(format!("{id}: {what}: got {got:?}, want {want:?}"));
    }
}

/// Compares every output the oracle read with the form and state the port produced.
fn check_outputs(bad: &mut Vec<String>, id: &str, out: &Value, r: &HitCalcResult) {
    let f = &r.form;
    let s = &r.state;
    let text = |k: &str, got: &str, bad: &mut Vec<String>| {
        if let Some(v) = out.get(k) {
            same(bad, id, k, got, string(v));
        }
    };
    let flag = |k: &str, got: bool, bad: &mut Vec<String>| {
        if let Some(v) = out.get(k) {
            same(bad, id, k, got, truthy(v));
        }
    };
    let num = |k: &str, got: i32, bad: &mut Vec<String>| {
        if let Some(v) = out.get(k) {
            same(bad, id, k, got, int(v));
        }
    };
    if let Some(t) = out.get("txt") {
        for i in 0..6 {
            same(
                bad,
                id,
                &format!("txt[{i}]"),
                f.text[i].as_str(),
                string(&t[i]),
            );
        }
    }
    if let Some(e) = out.get("en") {
        same(bad, id, "en[3]", f.prot_evil_enabled, truthy(&e[0]));
        same(bad, id, "en[4]", f.vile_ward_enabled, truthy(&e[1]));
        same(bad, id, "en[5]", f.percep_enabled, truthy(&e[2]));
    }
    flag("shadow", f.shadow, bad);
    flag("shadow_en", f.shadow_enabled, bad);
    flag("hidden", f.see_hidden, bad);
    flag("hidden_en", f.see_hidden_enabled, bad);
    num("evil", f.evil_index, bad);
    flag("evil_en", f.evil_enabled, bad);
    flag("mlist_en", f.monster_list_enabled, bad);
    text("bs", &f.bs_label, bad);
    text("main", &f.main_text, bad);
    text("left", &f.sub_left, bad);
    text("right", &f.sub_right, bad);
    flag("bold", f.sub_right_bold, bad);
    num("nDefenderAC", s.defender_ac, bad);
    num("nDefenderDodge", s.defender_dodge, bad);
    num("nDefenderPerception", s.defender_perception, bad);
    num("nDefenderProtEvil", s.defender_prot_evil, bad);
    num("nDefenderVileWard", s.defender_vile_ward, bad);
    num("nDefenderEvilness", s.defender_evilness, bad);
    flag("bDefenderShadow", s.defender_shadow, bad);
    num("nCharPerception", s.char_perception, bad);
    num("nMonsterAccy", s.monster_accy, bad);
    num("nMonsterAC", s.monster_ac, bad);
    num("nMonsterDodge", s.monster_dodge, bad);
    num("nMonsterBSdef", s.monster_bs_defense, bad);
    flag("bMonsterEvil", s.monster_evil, bad);
    flag("bMonsterSeeHidden", s.monster_see_hidden, bad);
}

#[test]
fn hit_calc_fixture() {
    let fx = fixture::load("hit_calc");
    assert_eq!(fx.cases[0]["id"], "table");
    let table = &fx.cases[0]["inputs"];
    let arr = |k: &str| table[k].as_array().unwrap();
    let items: Vec<Item> = arr("items").iter().map(item).collect();
    let classes: Vec<Class> = arr("classes").iter().map(class).collect();
    let monsters: Vec<Monster> = arr("monsters").iter().map(monster).collect();

    let mut bad: Vec<String> = Vec::new();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for c in fx.cases.iter().skip(1) {
        let id = c["id"].as_str().unwrap();
        let inputs = &c["inputs"];
        let out = &c["outputs"];
        let kind = id.split('-').next().unwrap();
        *counts.entry(kind).or_default() += 1;
        match kind {
            "vw" => {
                let a = &inputs["args"];
                same(
                    &mut bad,
                    id,
                    "result",
                    get_vile_ward_value(int(&a[0]), int(&a[1])),
                    int(&out["result"]),
                );
            }
            "mon" => {
                let mut st = HitCalcState::default();
                let nmr = inputs["globals"]["nNMRVer"].as_f64().unwrap();
                let found = get_monster_data(&monsters, int(&inputs["args"][0]), nmr, &mut st);
                same(&mut bad, id, "result", found, truthy(&out["result"]));
                let r = HitCalcResult {
                    form: HitCalcForm::default(),
                    state: st,
                    errors: vec![],
                    numbers: None,
                };
                check_outputs(&mut bad, id, out, &r);
            }
            "mod" => {
                let idx = int(&inputs["steps"][0]["args"][0]) as usize;
                let mut f = form(inputs);
                modify_hit_calc_values(&mut f, idx);
                let r = HitCalcResult {
                    form: f,
                    state: HitCalcState::default(),
                    errors: vec![],
                    numbers: None,
                };
                check_outputs(&mut bad, id, out, &r);
            }
            _ => {
                let f = form(inputs);
                let mut st = state(inputs);
                let r = match inputs["steps"][0]["call"].as_str().unwrap() {
                    "DoHitCalc" => do_hit_calc(Profile::Stock, &f, &st, class_selected(inputs)),
                    "SetHitCalcVals" => {
                        let ch = character(inputs);
                        let args = inputs["steps"][0]["args"].as_array().unwrap();
                        let refresh = Refresh {
                            character_only: args.first().is_some_and(truthy),
                            monster_only: args.get(1).is_some_and(truthy),
                        };
                        set_hit_calc_vals(
                            Profile::Stock,
                            &SetValsInput {
                                form: &f,
                                state: &st,
                                character: &ch,
                                items: &items,
                                classes: &classes,
                                refresh,
                            },
                        )
                    }
                    "GetMonsterData" => {
                        // cmbMonsterList_Click: a found monster refreshes the monster half.
                        let nmr = 1.83;
                        let found = get_monster_data(
                            &monsters,
                            int(&inputs["steps"][0]["args"][0]),
                            nmr,
                            &mut st,
                        );
                        assert!(found, "{id}: pick cases use known monsters");
                        let ch = character(inputs);
                        set_hit_calc_vals(
                            Profile::Stock,
                            &SetValsInput {
                                form: &f,
                                state: &st,
                                character: &ch,
                                items: &items,
                                classes: &classes,
                                refresh: Refresh {
                                    character_only: false,
                                    monster_only: true,
                                },
                            },
                        )
                    }
                    other => panic!("{id}: unknown step {other}"),
                };
                check_outputs(&mut bad, id, out, &r);
                let got: Vec<(String, u32)> = r
                    .errors
                    .iter()
                    .map(|e| (e.source.to_string(), e.number))
                    .collect();
                same(&mut bad, id, "errors", got, handled(c));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{} mismatches, first 25 of {:?}:\n{}",
        bad.len(),
        counts,
        bad.iter().take(25).cloned().collect::<Vec<_>>().join("\n")
    );
}
