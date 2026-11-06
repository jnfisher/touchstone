// Fixture tests for the attack family (swings, energy, weapon damage): see docs/porting.md step 5.
// Expected values come from fixtures/rules/attack.json (MME's own code run by the oracle).
use std::cell::Cell;

use serde_json::Value;
use touchstone_model::{AbilitySlot, Class, Item, Race};
use touchstone_rules::Profile;
use touchstone_rules::ability_text::AbilityEnv;
use touchstone_rules::attack::*;
use touchstone_vb6::Currency;

use crate::fixture::{self, Names, Textblocks, cur};

fn int(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an int: {v}")) as i32
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

fn item(row: &Value) -> Item {
    let r = row.as_array().unwrap();
    Item {
        number: int(&r[0]),
        name: r[1].as_str().unwrap().to_string(),
        limit: 0,
        encum: int(&r[2]),
        item_type: 1,
        use_count: 0,
        price: 0,
        currency: 0,
        min: int(&r[6]),
        max: int(&r[7]),
        armour_class: 0,
        damage_resist: 0,
        weapon_type: int(&r[8]),
        armour_type: 0,
        worn: 0,
        accy: int(&r[3]),
        gettable: true,
        str_req: int(&r[4]),
        speed: int(&r[5]),
        not_droppable: false,
        destroy_on_death: false,
        retain_after_uses: false,
        class_rest: [0; 10],
        race_rest: [0; 10],
        negate_spell: [0; 10],
        abilities: slots(r[9].as_array().unwrap()),
        in_game: true,
        obtained_from: None,
        references: None,
    }
}

fn class(row: &Value) -> Class {
    let r = row.as_array().unwrap();
    let mut abilities = [AbilitySlot::default(); 10];
    for (i, a) in r[2].as_array().unwrap().iter().enumerate() {
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
        combat_lvl: int(&r[1]),
        abilities,
    }
}

fn race(row: &Value) -> Race {
    let r = row.as_array().unwrap();
    let mut abilities = [AbilitySlot::default(); 10];
    for (i, a) in r[1].as_array().unwrap().iter().enumerate() {
        abilities[i].ability = int(a);
    }
    Race {
        number: int(&r[0]),
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
        hp_per_lvl: 0,
        exp_table: 0,
        base_cp: 0,
        abilities,
    }
}

fn profile(udt: &Value) -> CharacterProfile {
    let f = &udt["fields"];
    let n = |k: &str| f.get(k).map_or(0, int);
    let b = |k: &str| f.get(k).is_some_and(|v| v.as_bool().unwrap());
    let arr = |k: &str| {
        let mut out = [0i16; 3];
        if let Some(a) = f.get(k) {
            for (i, v) in a.as_array().unwrap().iter().enumerate() {
                out[i] = int(v) as i16;
            }
        }
        out
    };
    CharacterProfile {
        is_loaded_character: b("bIsLoadedCharacter"),
        level: n("nLevel"),
        class: n("nClass"),
        race: n("nRace"),
        combat: n("nCombat") as i16,
        strength: n("nSTR") as i16,
        agility: n("nAGI") as i16,
        intellect: n("nINT") as i16,
        charm: n("nCHA") as i16,
        crit: n("nCrit") as i16,
        plus_max_damage: n("nPlusMaxDamage") as i16,
        plus_min_damage: n("nPlusMinDamage") as i16,
        plus_bs_accuracy: n("nPlusBSaccy") as i16,
        plus_bs_min_damage: n("nPlusBSmindmg") as i16,
        plus_bs_max_damage: n("nPlusBSmaxdmg") as i16,
        ma_plus_skill: arr("nMAPlusSkill"),
        ma_plus_accuracy: arr("nMAPlusAccy"),
        ma_plus_damage: arr("nMAPlusDmg"),
        stealth: n("nStealth") as i16,
        class_stealth: b("bClassStealth"),
        race_stealth: b("bRaceStealth"),
        accuracy: f.get("nAccuracy").map_or(0.0, |v| v.as_f64().unwrap()),
        encum_pct: n("nEncumPCT") as i16,
        encum_current: n("nEncumCurrent"),
        encum_max: n("nEncumMax"),
        spell_dmg_bonus: n("nSpellDmgBonus") as i16,
    }
}

fn globals(inputs: &Value) -> EquippedGlobals {
    let g = &inputs["globals"];
    let one = |k: &str| g.get(k).map_or(0, int);
    let hand = |h: usize| {
        let at = |name: &str| {
            g.get(format!("nGlobalCharWeapon{name}"))
                .map_or(0, |a| int(&a[h]))
        };
        HandGlobals {
            number: at("Number"),
            encum: at("Encum"),
            accy: at("Accy"),
            crit: at("Crit"),
            strength: at("STR"),
            agility: at("AGI"),
            max_dmg: at("MaxDmg"),
            bs_accy: at("BSaccy"),
            bs_min_dmg: at("BSmindmg"),
            bs_max_dmg: at("BSmaxdmg"),
            punch_skill: at("PunchSkill"),
            punch_accy: at("PunchAccy"),
            punch_dmg: at("PunchDmg"),
            kick_skill: at("KickSkill"),
            kick_accy: at("KickAccy"),
            kick_dmg: at("KickDmg"),
            jk_skill: at("JkSkill"),
            jk_accy: at("JkAccy"),
            jk_dmg: at("JkDmg"),
            stealth: at("Stealth"),
        }
    };
    EquippedGlobals {
        qnd_bonus: one("nGlobalCharQnDbonus"),
        accy_abils: one("nGlobalCharAccyAbils"),
        accy_other: one("nGlobalCharAccyOther"),
        accy_items: one("nGlobalCharAccyItems"),
        hands: [hand(0), hand(1)],
    }
}

fn text_arg(inputs: &Value, path: &[&str]) -> String {
    let mut v = &inputs["forms"]["frmMain"];
    for p in path {
        v = &v[*p];
    }
    v.as_str().unwrap_or("").to_string()
}

fn f64_of(v: &Value) -> f64 {
    v.as_f64().unwrap()
}

fn cur_arg(v: &Value) -> Currency {
    Currency::from_scaled((f64_of(&v["v"]) * 10_000.0).round() as i64)
}

fn is_missing(v: &Value) -> bool {
    v["t"] == "missing"
}

#[test]
fn attack_fixture() {
    let fx = fixture::load("attack");
    let table = &fx.cases[0]["inputs"];
    assert_eq!(fx.cases[0]["id"], "table");
    let items: Vec<Item> = table["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(item)
        .collect();
    let classes: Vec<Class> = table["classes"]
        .as_array()
        .unwrap()
        .iter()
        .map(class)
        .collect();
    let races: Vec<Race> = table["races"]
        .as_array()
        .unwrap()
        .iter()
        .map(race)
        .collect();
    let spells = fixture::spell_table(&fx.cases[0]);
    let names = Names::from_table(&table["names"]);
    let tbs = Textblocks::from_table(&table["textblocks"]);
    // The real text port over the fixture's tables, with MME's start-up globals.
    let text = AbilityEnv {
        profile: Profile::Stock,
        names: &names,
        textblocks: &tbs,
        spells: &spells,
        hide_record_numbers: false,
        quick_spell: false,
        global_level_text: None,
        avg_level_max_all_stats: 0.0,
        nest: Cell::new(0),
    };

    let mut bad: Vec<String> = Vec::new();
    let mut checked = 0;
    for c in &fx.cases {
        let id = c["id"].as_str().unwrap();
        if id == "table" {
            continue;
        }
        // GetSpellName and PullSpellEQ as MME answered them, against the port's own.
        if id.starts_with("sn-") || id.starts_with("pe-") {
            let args = &c["inputs"]["args"];
            let want = c["outputs"]["result"].as_str().unwrap();
            let got = if id.starts_with("sn-") {
                text.spell_name(int(&args[0]))
            } else {
                text.spell_eq(int(&args[2]), int(&args[11]) as i16)
            };
            checked += 1;
            if got != want {
                bad.push(format!("{id}: want {want:?}, got {got:?}"));
            }
            continue;
        }
        let inputs = &c["inputs"];
        let args = &inputs["args"];
        let out = &c["outputs"]["result"];
        let errored = c.get("error").is_some();
        let proc = inputs["proc"].as_str().unwrap();
        let mismatch = match proc {
            "CalculateAttack" => {
                let equipped = globals(inputs);
                let caption = text_arg(inputs, &["lblDatVer", "Caption"]);
                let tag = inputs["forms"]["frmMain"]["lblInvenCharStat"]["$array"]["10"]["Tag"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let avg = inputs["globals"]
                    .get("gAvgLevelMaxAllStats")
                    .map_or(0.0, f64_of);
                let env = AttackEnv {
                    items: &items,
                    classes: &classes,
                    races: &races,
                    equipped: &equipped,
                    cast_text: &text,
                    dat_ver_caption: &caption,
                    avg_level_max_all_stats: avg,
                    accuracy_tag: &tag,
                };
                let req = AttackRequest {
                    attack_type: int(&args[1]),
                    weapon_number: int(&args[2]),
                    abil68_slow: args[3].as_bool().unwrap(),
                    speed_adj: int(&args[4]) as i16,
                    vs_ac: int(&args[5]),
                    vs_dr: int(&args[6]),
                    vs_dodge: int(&args[7]),
                    force_calc: args[9].as_bool().unwrap(),
                    specify_damage: f64_of(&args[10]),
                    specify_accy: f64_of(&args[11]),
                    bs_defense: int(&args[12]),
                };
                let mut casts = args[8]["value"].as_str().unwrap().to_string();
                let got =
                    calculate_attack(Profile::Stock, &env, &profile(&args[0]), &req, &mut casts);
                let want = AttackDamage {
                    min_dmg: int(&out["nmindmg"]),
                    max_dmg: int(&out["nmaxdmg"]),
                    hit_chance: int(&out["nhitchance"]) as i16,
                    dodge_chance: int(&out["ndodgechance"]) as i16,
                    crit_chance: int(&out["ncritchance"]) as i16,
                    qnd_bonus: int(&out["nqndbonus"]) as i16,
                    accy: int(&out["naccy"]) as i16,
                    avg_hit: int(&out["navghit"]),
                    avg_crit: int(&out["navgcrit"]),
                    max_crit: int(&out["nmaxcrit"]),
                    avg_extra_hit: int(&out["navgextrahit"]),
                    avg_extra_swing: int(&out["navgextraswing"]),
                    swings: f64_of(&out["nswings"]),
                    round_physical: int(&out["nroundphysical"]),
                    round_total: int(&out["nroundtotal"]),
                    first_round_damage: int(&out["nfirstrounddamage"]),
                    min_round_damage: int(&out["nminrounddamage"]),
                    attack_desc: out["sattackdesc"].as_str().unwrap().to_string(),
                    attack_detail: out["sattackdetail"].as_str().unwrap().to_string(),
                    attack_speed: int(&out["nattackspeed"]) as i16,
                };
                let want_casts = c["outputs"]["result.refs"][0].as_str().unwrap();
                if got != want || casts != want_casts {
                    Some(format!(
                        "got {got:?} / {casts:?}\n want {want:?} / {want_casts:?}"
                    ))
                } else {
                    None
                }
            }
            "CalcEnergyUsed" => {
                let a = args.as_array().unwrap();
                let mut i = EnergyInput {
                    combat: cur_arg(&a[0]),
                    level: cur_arg(&a[1]),
                    attack_speed: cur_arg(&a[2]),
                    agility: cur_arg(&a[3]),
                    ..EnergyInput::default()
                };
                if a.len() > 4 {
                    i.strength = cur_arg(&a[4]);
                    i.encum = int(&a[5]) as i16;
                    i.item_strength = cur_arg(&a[6]);
                    i.speed_adj = cur_arg(&a[7]);
                    i.is_backstab = a[8].as_bool().unwrap();
                }
                match (calc_energy_used(&i), errored) {
                    (Ok(v), false) if v.scaled() == cur(out) => None,
                    (Err(e), true)
                        if i64::from(e.code()) == c["error"]["number"].as_i64().unwrap() =>
                    {
                        None
                    }
                    (r, _) => Some(format!("got {r:?}, want {out} / {:?}", c.get("error"))),
                }
            }
            "CalcEnergyUsedWithEncum" => {
                let a = args.as_array().unwrap();
                let item_str = if is_missing(&a[6]) {
                    Currency::from_scaled(0)
                } else {
                    cur_arg(&a[6])
                };
                let r = calc_energy_used_with_encum(
                    cur_arg(&a[0]),
                    cur_arg(&a[1]),
                    cur_arg(&a[2]),
                    cur_arg(&a[3]),
                    cur_arg(&a[4]),
                    cur_arg(&a[5]),
                    item_str,
                );
                check_cur(r, out, errored)
            }
            "AdjustEnergyUsedWithSpeed" => check_cur(
                adjust_energy_used_with_speed(cur_arg(&args[0]), cur_arg(&args[1])),
                out,
                errored,
            ),
            "AdjustEnergyUsedWithEncum" => check_cur(
                adjust_energy_used_with_encum(cur_arg(&args[0]), cur_arg(&args[1])),
                out,
                errored,
            ),
            "CalcQuickAndDeadlyBonus" => {
                let v = calc_quick_and_deadly_bonus(
                    cur_arg(&args[0]),
                    cur_arg(&args[1]),
                    int(&args[2]) as i16,
                );
                (v.scaled() != cur(out)).then(|| format!("got {v:?}, want {out}"))
            }
            "CalcBSDamage" => {
                let v = calc_bs_damage(
                    int(&args[0]) as i16,
                    int(&args[1]) as i16,
                    int(&args[2]) as i16,
                    int(&args[3]) as i16,
                    args[4].as_bool().unwrap(),
                );
                (i64::from(v) != out.as_i64().unwrap()).then(|| format!("got {v}, want {out}"))
            }
            "CalcTrueAverage" => {
                let v = calc_true_average(
                    f64_of(&args[0]),
                    f64_of(&args[1]),
                    int(&args[2]),
                    f64_of(&args[3]),
                    int(&args[4]),
                    f64_of(&args[5]),
                    int(&args[6]),
                );
                (v != f64_of(out)).then(|| format!("got {v}, want {out}"))
            }
            "GetQuickAndDeadlyBonus" => {
                let ctx_forms = &inputs["forms"]["frmMain"];
                let cb = &ctx_forms["cmbGlobalClass"]["$array"]["0"];
                let idx = cb["ListIndex"].as_i64().unwrap();
                let strength = text_arg(inputs, &["txtCharStats", "$array", "0", "Tag"]);
                let agility = text_arg(inputs, &["txtCharStats", "$array", "3", "Tag"]);
                let ec = text_arg(inputs, &["lblInvenCharStat", "$array", "0", "Caption"]);
                let em = text_arg(inputs, &["lblInvenCharStat", "$array", "1", "Caption"]);
                let lv = text_arg(inputs, &["txtGlobalLevel", "$array", "0", "Text"]);
                let ctx = QndContext {
                    global_filter: int(&ctx_forms["chkGlobalFilter"]["Value"]),
                    class_number: (idx >= 0).then(|| int(&cb["ItemData"][idx as usize])),
                    strength_tag: &strength,
                    agility_tag: &agility,
                    encum_current_caption: &ec,
                    encum_max_caption: &em,
                    level_text: &lv,
                };
                let v = get_quick_and_deadly_bonus(&items, &classes, &ctx, int(&args[0]));
                (i64::from(v) != out.as_i64().unwrap()).then(|| format!("got {v}, want {out}"))
            }
            other => panic!("unknown procedure {other} in case {id}"),
        };
        checked += 1;
        if let Some(m) = mismatch {
            bad.push(format!("{id}: {m}\n inputs {inputs}"));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {checked} cases differ; first:\n{}",
        bad.len(),
        bad.iter().take(6).cloned().collect::<Vec<_>>().join("\n")
    );
    assert!(checked > 1000);
}

fn check_cur(
    r: Result<Currency, touchstone_vb6::Vb6Error>,
    out: &Value,
    errored: bool,
) -> Option<String> {
    match (r, errored) {
        (Ok(v), false) if v.scaled() == cur(out) => None,
        (r, _) => Some(format!("got {r:?}, want {out}")),
    }
}
