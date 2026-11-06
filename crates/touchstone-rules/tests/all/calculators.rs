// Fixture tests for the calculator cores (CalcSwings, CalcBS, CalcCoin/ConvertCoin, CalcExp): see
// docs/porting.md step 5. Expected values come from fixtures/rules/calculators.json (MME's own
// procedures, cut out of the forms and run by the oracle).
use std::collections::HashMap;

use serde_json::Value;
use touchstone_model::{AbilitySlot, Class, Item, Race};
use touchstone_rules::Profile;
use touchstone_rules::attack::{
    AttackEnv, CastText, CharacterProfile, EquippedGlobals, HandGlobals,
};
use touchstone_rules::calculators::*;
use touchstone_vb6 as vb;

use crate::fixture;

fn int(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an int: {v}")) as i32
}

fn string(v: &Value) -> &str {
    v.as_str().unwrap_or("")
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

// [Number, Encum, Accy, StrReq, Speed, Min, Max, WeaponType, AbilitiesFlat]
fn item(row: &Value) -> Item {
    let r = row.as_array().unwrap();
    Item {
        number: int(&r[0]),
        name: String::new(),
        limit: 0,
        encum: int(&r[1]),
        item_type: 1,
        use_count: 0,
        price: 0,
        currency: 0,
        min: int(&r[5]),
        max: int(&r[6]),
        armour_class: 0,
        damage_resist: 0,
        weapon_type: int(&r[7]),
        armour_type: 0,
        worn: 0,
        accy: int(&r[2]),
        gettable: true,
        str_req: int(&r[3]),
        speed: int(&r[4]),
        not_droppable: false,
        destroy_on_death: false,
        retain_after_uses: false,
        class_rest: [0; 10],
        race_rest: [0; 10],
        negate_spell: [0; 10],
        abilities: slots(r[8].as_array().unwrap()),
        in_game: true,
        obtained_from: None,
        references: None,
    }
}

// [Number, CombatLVL, ExpTable, [Abil-0..9]]
fn class(row: &Value) -> Class {
    let r = row.as_array().unwrap();
    let mut abilities = [AbilitySlot::default(); 10];
    for (i, a) in r[3].as_array().unwrap().iter().enumerate() {
        abilities[i].ability = int(a);
    }
    Class {
        number: int(&r[0]),
        name: String::new(),
        min_hits: 0,
        max_hits: 0,
        exp_table: int(&r[2]),
        magery_type: 0,
        magery_lvl: 0,
        weapon_type: 0,
        armour_type: 0,
        combat_lvl: int(&r[1]),
        abilities,
    }
}

// [Number, ExpTable, [Abil-0..9]]
fn race(row: &Value) -> Race {
    let r = row.as_array().unwrap();
    let mut abilities = [AbilitySlot::default(); 10];
    for (i, a) in r[2].as_array().unwrap().iter().enumerate() {
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
        exp_table: int(&r[1]),
        base_cp: 0,
        abilities,
    }
}

/// The spell text MME's other procedures gave for each weapon spell, recorded by the oracle.
struct Recorded {
    names: HashMap<i32, String>,
    eqs: HashMap<(i32, i32), String>,
}

impl CastText for Recorded {
    fn spell_name(&self, number: i32) -> String {
        self.names
            .get(&number)
            .unwrap_or_else(|| panic!("no recorded GetSpellName({number})"))
            .clone()
    }
    fn spell_eq(&self, number: i32, bonus: i16) -> String {
        self.eqs
            .get(&(number, i32::from(bonus)))
            .unwrap_or_else(|| panic!("no recorded PullSpellEQ({number}, {bonus})"))
            .clone()
    }
}

/// `frmMain.<control>` of a case: its form values.
fn ctl<'a>(inputs: &'a Value, name: &str) -> &'a Value {
    &inputs["forms"]["frmMain"][name]
}

/// `Val` of an element's property of a control array, "" when unset.
fn elem<'a>(inputs: &'a Value, name: &str, i: usize, prop: &str) -> &'a str {
    string(&ctl(inputs, name)["$array"][i.to_string()][prop])
}

fn text<'a>(inputs: &'a Value, name: &str) -> &'a str {
    string(&ctl(inputs, name)["Text"])
}

fn selected(inputs: &Value, name: &str) -> Option<i32> {
    let c = ctl(inputs, name);
    let li = c["ListIndex"].as_i64()?;
    if li < 0 {
        return None;
    }
    Some(int(&c["ItemData"][li as usize]))
}

fn checked(inputs: &Value, name: &str) -> bool {
    ctl(inputs, name)["Value"].as_i64() == Some(1)
}

fn options<const N: usize>(inputs: &Value, name: &str) -> [bool; N] {
    let mut out = [false; N];
    for (i, o) in out.iter_mut().enumerate() {
        *o = ctl(inputs, name)["$array"][i.to_string()]["Value"] == Value::Bool(true);
    }
    out
}

fn hands(inputs: &Value) -> [HandGlobals; 2] {
    let g = &inputs["globals"];
    let at = |name: &str, h: usize| {
        g.get(format!("nGlobalCharWeapon{name}"))
            .map_or(0, |a| int(&a[h]))
    };
    let hand = |h: usize| HandGlobals {
        number: at("Number", h),
        encum: at("Encum", h),
        accy: at("Accy", h),
        crit: at("Crit", h),
        strength: at("STR", h),
        agility: at("AGI", h),
        max_dmg: at("MaxDmg", h),
        bs_accy: at("BSaccy", h),
        bs_min_dmg: at("BSmindmg", h),
        bs_max_dmg: at("BSmaxdmg", h),
        punch_skill: at("PunchSkill", h),
        punch_accy: at("PunchAccy", h),
        punch_dmg: at("PunchDmg", h),
        kick_skill: at("KickSkill", h),
        kick_accy: at("KickAccy", h),
        kick_dmg: at("KickDmg", h),
        jk_skill: at("JkSkill", h),
        jk_accy: at("JkAccy", h),
        jk_dmg: at("JkDmg", h),
        stealth: at("Stealth", h),
    };
    [hand(0), hand(1)]
}

/// What PopulateCharacterProfile builds from the main window's panels (global filter on); the
/// fields CalcSwings overwrites afterwards are left out.
fn profile(inputs: &Value) -> CharacterProfile {
    let tag = |i: usize| vb::val(elem(inputs, "lblInvenCharStat", i, "Tag"));
    let i16_of = |x: f64| vb::cint(x).unwrap();
    let arr = |base: usize| [0, 1, 2].map(|k| i16_of(tag(base + k)));
    let class_item = &ctl(inputs, "cmbGlobalClass")["$array"]["0"]["ItemData"][0];
    let race_item = &ctl(inputs, "cmbGlobalRace")["$array"]["0"]["ItemData"][0];
    CharacterProfile {
        is_loaded_character: true,
        class: int(class_item),
        race: int(race_item),
        intellect: i16_of(vb::val(elem(inputs, "txtCharStats", 1, "Tag"))),
        charm: i16_of(vb::val(elem(inputs, "txtCharStats", 5, "Tag"))),
        crit: i16_of(tag(7)),
        plus_max_damage: i16_of(tag(11)),
        plus_min_damage: i16_of(tag(30)),
        plus_bs_accuracy: i16_of(tag(13)),
        plus_bs_min_damage: i16_of(tag(14)),
        plus_bs_max_damage: i16_of(tag(15)),
        ma_plus_skill: arr(37),
        ma_plus_accuracy: arr(40),
        ma_plus_damage: arr(34),
        stealth: i16_of(tag(19)),
        accuracy: tag(10),
        encum_current: vb::clng(vb::val(elem(inputs, "lblInvenCharStat", 0, "Caption"))).unwrap(),
        encum_max: vb::clng(vb::val(elem(inputs, "lblInvenCharStat", 1, "Caption"))).unwrap(),
        spell_dmg_bonus: i16_of(tag(33)),
        ..CharacterProfile::default()
    }
}

fn vb_error(c: &Value) -> Option<u32> {
    c.get("error").map(|e| e["number"].as_u64().unwrap() as u32)
}

fn handled_error(c: &Value, source: &str) -> Option<u32> {
    c.get("events")?
        .as_array()?
        .iter()
        .find(|e| e["kind"] == "handled_error" && e["source"] == source)
        .map(|e| e["number"].as_u64().unwrap() as u32)
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| string(s).to_string())
        .collect()
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

#[test]
fn calculators_fixture() {
    let fx = fixture::load("calculators");
    assert_eq!(fx.cases[0]["id"], "table");
    let table = &fx.cases[0]["inputs"];
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
    let mut rec = Recorded {
        names: HashMap::new(),
        eqs: HashMap::new(),
    };
    for c in &fx.cases {
        let id = c["id"].as_str().unwrap();
        let args = &c["inputs"]["args"];
        if id.starts_with("sn-") {
            rec.names
                .insert(int(&args[0]), string(&c["outputs"]["result"]).to_string());
        } else if id.starts_with("pe-") {
            rec.eqs.insert(
                (int(&args[2]), int(&args[11])),
                string(&c["outputs"]["result"]).to_string(),
            );
        }
    }
    let setup = &fx.header["setup"];
    let caption = string(&setup["forms"]["frmMain"]["lblDatVer"]["Caption"]).to_string();
    let avg_level = setup["globals"]["gAvgLevelMaxAllStats"].as_f64().unwrap();

    let mut bad: Vec<String> = Vec::new();
    let mut checked_cases = 0;
    for c in &fx.cases {
        let id = c["id"].as_str().unwrap();
        if id == "table" || id.starts_with("sn-") || id.starts_with("pe-") {
            continue;
        }
        checked_cases += 1;
        let inputs = &c["inputs"];
        let out = &c["outputs"];
        let before = bad.len();
        if id.starts_with("swing") {
            let g = &inputs["globals"];
            let one = |k: &str| g.get(k).map_or(0, int);
            let equipped = EquippedGlobals {
                qnd_bonus: one("nGlobalCharQnDbonus"),
                accy_abils: one("nGlobalCharAccyAbils"),
                accy_other: one("nGlobalCharAccyOther"),
                accy_items: one("nGlobalCharAccyItems"),
                hands: hands(inputs),
            };
            let filter = ctl(inputs, "chkGlobalFilter")["Value"].as_i64() == Some(1);
            let tag = elem(inputs, "lblInvenCharStat", 10, "Tag");
            let env = AttackEnv {
                items: &items,
                classes: &classes,
                races: &races,
                equipped: &equipped,
                cast_text: &rec,
                dat_ver_caption: &caption,
                avg_level_max_all_stats: avg_level,
                accuracy_tag: tag,
            };
            let input = SwingInput {
                weapon: selected(inputs, "cmbWeapon"),
                combat: selected(inputs, "cmbCombat").unwrap(),
                slowness: checked(inputs, "chkSlowness"),
                bashing: checked(inputs, "chkBashing"),
                encum_text: text(inputs, "txtEncum"),
                max_encum_text: text(inputs, "txtMaxEncum"),
                level_text: text(inputs, "txtLevel"),
                agility_text: text(inputs, "txtAgility"),
                strength_text: text(inputs, "txtStrength"),
                speed_text: text(inputs, "txtSpeed"),
                speed_options: options(inputs, "optSpeed"),
                global_filter: filter,
                profile: if filter {
                    profile(inputs)
                } else {
                    CharacterProfile::default()
                },
            };
            let got = calc_swings(Profile::Stock, &input, &env);
            same(&mut bad, id, "error", got.error, vb_error(c));
            same(
                &mut bad,
                id,
                "qnd",
                got.qnd_text.as_str(),
                string(&out["qnd"]),
            );
            same(
                &mut bad,
                id,
                "energy",
                got.energy_text.as_str(),
                string(&out["energy"]),
            );
            same(
                &mut bad,
                id,
                "encum",
                got.encum_text.as_str(),
                string(&out["encum"]),
            );
            same(
                &mut bad,
                id,
                "encum_tag",
                got.encum_tag.as_str(),
                string(&out["encum_tag"]),
            );
            same(
                &mut bad,
                id,
                "raw",
                got.raw_swing_text.as_str(),
                string(&out["raw"]),
            );
            same(
                &mut bad,
                id,
                "raw_tag",
                got.raw_swing_tag.as_str(),
                string(&out["raw_tag"]),
            );
            same(
                &mut bad,
                id,
                "avg6",
                got.true_avg_6.as_str(),
                string(&out["avg6"]),
            );
            same(&mut bad, id, "eu", got.eu.to_vec(), strings(&out["eu"]));
            same(
                &mut bad,
                id,
                "swing",
                got.swings.to_vec(),
                strings(&out["swing"]),
            );
            same(
                &mut bad,
                id,
                "dmg0",
                got.damage_text.as_str(),
                string(&out["dmg0"]),
            );
            same(
                &mut bad,
                id,
                "dmg1",
                got.damage_caption.as_str(),
                string(&out["dmg1"]),
            );
            same(
                &mut bad,
                id,
                "speed_text",
                got.speed_text.as_str(),
                string(&out["speed_text"]),
            );
        } else if id.starts_with("bs") {
            let input = BsInput {
                weapon: selected(inputs, "cmbWeapon"),
                bs_min_text: text(inputs, "txtBSMinDMG"),
                bs_max_text: text(inputs, "txtBSMaxDMG"),
                level_text: text(inputs, "txtLevel"),
                stealth_text: text(inputs, "txtStealth"),
                max_dmg_text: text(inputs, "txtMaxDMG"),
                strength_text: text(inputs, "txtStrength"),
                class_stealth: checked(inputs, "chkClassStealth"),
                global_filter: ctl(inputs, "chkGlobalFilter")["Value"].as_i64() == Some(1),
                equipped: hands(inputs),
            };
            let got = calc_bs(&input, &items);
            same(&mut bad, id, "error", got.error, handled_error(c, "CalcBS"));
            same(
                &mut bad,
                id,
                "dmg",
                got.damage_text.as_str(),
                string(&out["dmg"]),
            );
            same(
                &mut bad,
                id,
                "max_adj",
                got.max_adj.as_str(),
                string(&out["max_adj"]),
            );
            same(
                &mut bad,
                id,
                "bsmin_adj",
                got.bs_min_adj.as_str(),
                string(&out["bsmin_adj"]),
            );
            same(
                &mut bad,
                id,
                "bsmax_adj",
                got.bs_max_adj.as_str(),
                string(&out["bsmax_adj"]),
            );
            same(
                &mut bad,
                id,
                "stealth_adj",
                got.stealth_adj.as_str(),
                string(&out["stealth_adj"]),
            );
            same(
                &mut bad,
                id,
                "t_bsmin",
                got.bs_min_text.as_str(),
                string(&out["t_bsmin"]),
            );
            same(
                &mut bad,
                id,
                "t_bsmax",
                got.bs_max_text.as_str(),
                string(&out["t_bsmax"]),
            );
            same(
                &mut bad,
                id,
                "t_level",
                got.level_text.as_str(),
                string(&out["t_level"]),
            );
            same(
                &mut bad,
                id,
                "t_stealth",
                got.stealth_text.as_str(),
                string(&out["t_stealth"]),
            );
            same(
                &mut bad,
                id,
                "t_maxdmg",
                got.max_dmg_text.as_str(),
                string(&out["t_maxdmg"]),
            );
        } else if id.starts_with("coin") {
            let step = &inputs["steps"][0]["args"];
            let (src, dst) = (int(&step[0]) as usize, int(&step[1]) as usize);
            let coin_text = |i: usize| elem(inputs, "txtCoin", i, "Text");
            let input = CoinInput {
                source_index: src,
                destination_index: dst,
                top: options(inputs, "optCoinTop"),
                bottom: options(inputs, "optCoinBottom"),
                source_text: coin_text(src),
                force_source: step.get(2).map_or(0, int),
            };
            let got = calc_coin(&input);
            let key = |i: usize| if i == 0 { "top" } else { "bottom" };
            same(
                &mut bad,
                id,
                "text",
                got.destination_text.as_str(),
                string(&out[key(dst)]),
            );
            // CalcCoin only writes the destination box and eCurrentCoin(destination).
            if src != dst {
                same(
                    &mut bad,
                    id,
                    "source box",
                    string(&out[key(src)]),
                    coin_text(src),
                );
            }
            same(
                &mut bad,
                id,
                "coin",
                got.current_coin,
                int(&out["cur"][dst]),
            );
        } else if id.starts_with("convert") {
            let a = &inputs["args"];
            let got = convert_coin(a[0].as_f64().unwrap(), int(&a[1]), int(&a[2]));
            same(
                &mut bad,
                id,
                "result",
                got.to_bits(),
                out["result"].as_f64().unwrap().to_bits(),
            );
        } else if id.starts_with("exp") {
            let pick = |name: &str| {
                let c = ctl(inputs, name);
                let li = c["ListIndex"].as_i64().unwrap_or(-1);
                (li > 0).then(|| int(&c["ItemData"][li as usize]))
            };
            let got = calc_exp(
                &ExpCalcInput {
                    class: pick("cmbClass"),
                    race: pick("cmbRace"),
                },
                &classes,
                &races,
            );
            let shown = got
                .table_text
                .unwrap_or_else(|| text(inputs, "txtCalcEXPTable").to_string());
            same(&mut bad, id, "table", shown.as_str(), string(&out["table"]));
            same(&mut bad, id, "error", got.error, handled_error(c, ""));
        } else {
            panic!("unknown case {id}");
        }
        let _ = before;
    }
    assert!(
        bad.is_empty(),
        "{} of {} cases differ; first mismatches:\n{}",
        bad.len(),
        checked_cases,
        bad.iter().take(20).cloned().collect::<Vec<_>>().join("\n")
    );
    assert!(checked_cases > 1000, "only {checked_cases} cases ran");
}
