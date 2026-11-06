// Fixture tests for the combat family (FAM-08): see docs/porting.md step 5.
// Expected values come from fixtures/rules/combat.json (MME's own code run by the oracle).
use serde_json::Value;
use touchstone_model::{
    AbilitySlot, Class, DropSlot, Item, MidSpell, Monster, MonsterAttack, Race, Spell,
};
use touchstone_rules::Profile;
use touchstone_rules::attack::{CastText, EquippedGlobals, HandGlobals};
use touchstone_rules::character::*;
use touchstone_rules::combat::*;
use touchstone_rules::spells::SpellText;
use touchstone_vb6 as vb;

use crate::fixture;

fn i(v: &Value) -> i64 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}"))
}

/// A numeric argument: a plain number or `{"t": "f64", "v": x}`.
fn num(v: &Value) -> f64 {
    if let Some(x) = v.get("v") {
        return x.as_f64().unwrap();
    }
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

fn missing(v: &Value) -> bool {
    v.get("t").is_some_and(|t| t == "missing")
}

/// The argument at `k`, or `None` when the case left it out (an Optional default applies).
fn arg(args: &[Value], k: usize) -> Option<&Value> {
    args.get(k).filter(|v| !missing(v))
}

struct NoText;

impl SpellText for NoText {
    fn ability_stats(&self, _: i16, _: i16, _: bool, _: bool) -> String {
        String::new()
    }
    fn spell_name(&self, _: i32) -> String {
        String::new()
    }
    fn monster_name(&self, _: i32) -> String {
        String::new()
    }
}

/// The cases use no weapon with a cast ability, so this is never asked.
struct NoCasts;

impl CastText for NoCasts {
    fn spell_name(&self, n: i32) -> String {
        panic!("cast text asked for spell {n}")
    }
    fn spell_eq(&self, n: i32, _: i16) -> String {
        panic!("cast text asked for spell {n}")
    }
}

fn slots<const N: usize>(flat: &[Value]) -> [AbilitySlot; N] {
    let mut out = [AbilitySlot::default(); N];
    for (k, p) in flat.chunks(2).enumerate() {
        out[k] = AbilitySlot {
            ability: i(&p[0]) as i32,
            value: p.get(1).map_or(0, |v| i(v) as i32),
        };
    }
    out
}

fn spell_table(table: &Value) -> Vec<Spell> {
    table["spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let f = r.as_array().unwrap();
            let n = |k: usize| i(&f[k]) as i32;
            let text = |len: i32| (len > 0).then(|| "x".repeat(len as usize));
            Spell {
                number: n(0),
                name: Some(f[1].as_str().unwrap().to_string()),
                short: Some(f[2].as_str().unwrap().to_string()),
                req_level: n(3),
                energy_cost: n(4),
                mana_cost: n(5),
                min_base: n(6),
                max_base: n(7),
                diff: n(8),
                type_of_resists: n(9),
                targets: n(10),
                dur: n(11),
                att_type: n(12),
                magery: n(13),
                magery_lvl: n(14),
                cap: n(15),
                max_inc_lvls: n(16),
                max_inc: n(17),
                min_inc_lvls: n(18),
                min_inc: n(19),
                dur_inc_lvls: n(20),
                dur_inc: n(21),
                abilities: slots(f[26].as_array().unwrap()),
                learnable: n(22) != 0,
                learned_from: text(n(23)),
                casted_by: text(n(24)),
                classes: Some(f[25].as_str().unwrap().to_string()).filter(|s| !s.is_empty()),
            }
        })
        .collect()
}

fn item_table(table: &Value) -> Vec<Item> {
    table["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let n = |k: usize| i(&r[k]) as i32;
            Item {
                number: n(0),
                name: r[1].as_str().unwrap().to_string(),
                limit: 0,
                encum: n(2),
                item_type: n(10),
                use_count: 0,
                price: 0,
                currency: 0,
                min: n(6),
                max: n(7),
                armour_class: 0,
                damage_resist: 0,
                weapon_type: n(8),
                armour_type: 0,
                worn: 0,
                accy: n(3),
                gettable: true,
                str_req: n(4),
                speed: n(5),
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
        })
        .collect()
}

fn class_table(table: &Value) -> Vec<Class> {
    table["classes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Class {
            number: i(&r[0]) as i32,
            name: String::new(),
            min_hits: 0,
            max_hits: 0,
            exp_table: 0,
            magery_type: 0,
            magery_lvl: 0,
            weapon_type: 0,
            armour_type: 0,
            combat_lvl: i(&r[1]) as i32,
            abilities: slots(r[2].as_array().unwrap()),
        })
        .collect()
}

fn race_table(table: &Value) -> Vec<Race> {
    table["races"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut abilities = [AbilitySlot::default(); 10];
            for (k, a) in r[1].as_array().unwrap().iter().enumerate() {
                abilities[k].ability = i(a) as i32;
            }
            Race {
                number: i(&r[0]) as i32,
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
        })
        .collect()
}

fn monster_table(table: &Value) -> Vec<Monster> {
    table["monsters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let attack = || MonsterAttack {
                name: String::new(),
                att_type: 0,
                acc: 0,
                pct: 0,
                true_pct: 0.0,
                min: 0,
                max: 0,
                energy: 0,
                hit_spell: 0,
            };
            Monster {
                number: i(&r[0]) as i32,
                name: None,
                weapon: 0,
                armour_class: i(&r[1]) as i32,
                damage_resist: i(&r[2]) as i32,
                follow_pct: 0,
                magic_res: i(&r[3]) as i32,
                bs_defense: i(&r[4]) as i32,
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
                undead: i(&r[5]) as i32,
                align: 0,
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
                attacks: [(); 5].map(|()| attack()),
                mid_spells: [MidSpell::default(); 5],
                drops: [DropSlot::default(); 10],
                abilities: slots(r[6].as_array().unwrap()),
                in_game: true,
                summoned_by: None,
            }
        })
        .collect()
}

// ---- reading a case's controls and globals

/// `frmMain.<name>[(idx)].<prop>`, if the case set it.
fn ctl<'a>(forms: &'a Value, name: &str, idx: Option<usize>, prop: &str) -> Option<&'a Value> {
    let c = forms.get("frmMain")?.get(name)?;
    let c = match idx {
        Some(k) => c.get("$array")?.get(k.to_string())?,
        None => c,
    };
    c.get(prop)
}

fn text(forms: &Value, name: &str, idx: Option<usize>, prop: &str) -> String {
    match ctl(forms, name, idx, prop) {
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => String::new(),
    }
}

fn val(forms: &Value, name: &str, idx: Option<usize>, prop: &str) -> f64 {
    vb::val(&text(forms, name, idx, prop))
}

fn combo(forms: &Value, name: &str) -> Option<i32> {
    let list_index = ctl(forms, name, Some(0), "ListIndex").map_or(-1, i);
    if list_index < 0 {
        return None;
    }
    let data = ctl(forms, name, Some(0), "ItemData")?;
    Some(i(&data[list_index as usize]) as i32)
}

fn g_f64(g: &Value, name: &str) -> f64 {
    g.get(name).and_then(Value::as_f64).unwrap_or(0.0)
}

fn g_i(g: &Value, name: &str) -> i32 {
    g.get(name).map_or(0, |v| i(v) as i32)
}

fn g_b(g: &Value, name: &str) -> bool {
    g.get(name).and_then(Value::as_bool).unwrap_or(false)
}

fn g_pair(g: &Value, name: &str) -> [i32; 2] {
    match g.get(name).and_then(Value::as_array) {
        Some(a) => [i(&a[0]) as i32, i(&a[1]) as i32],
        None => [0, 0],
    }
}

fn character(forms: &Value, g: &Value) -> Character {
    let mut ch = Character {
        global_filter: ctl(forms, "chkGlobalFilter", None, "Value").is_some_and(|v| i(v) == 1),
        level: vb::clng(val(forms, "txtGlobalLevel", Some(0), "Text")).unwrap(),
        class: combo(forms, "cmbGlobalClass"),
        race: combo(forms, "cmbGlobalRace"),
        alignment: ctl(forms, "cmbGlobalAlignment", None, "ListIndex").map_or(-1, |v| i(v) as i16),
        mr_text: text(forms, "txtCharMR", None, "Text"),
        ..Character::default()
    };
    let a = &mut ch.attack;
    a.attack_type = AttackType::from_code(g_i(g, "nGlobalAttackTypeMME")).unwrap();
    a.backstab = g_b(g, "bGlobalAttackBackstab");
    a.backstab_weapon = g_i(g, "nGlobalAttackBackstabWeapon");
    a.martial_art = g_i(g, "nGlobalAttackMA") as i16;
    a.spell_num = g_i(g, "nGlobalAttackSpellNum");
    a.spell_lvl = g_i(g, "nGlobalAttackSpellLVL") as i16;
    a.manual_p = g_i(g, "nGlobalAttackManualP");
    a.manual_m = g_i(g, "nGlobalAttackManualM");
    a.heal_value = g_i(g, "nGlobalAttackHealValue");
    a.heal_cost = g_f64(g, "nGlobalAttackHealCost");
    ch
}

fn char_stats(forms: &Value, g: &Value) -> CharStats {
    let mut s = CharStats::default();
    for k in 0..47 {
        s.stat[k] = val(forms, "lblInvenCharStat", Some(k), "Tag");
    }
    s.encum_current = val(forms, "lblInvenCharStat", Some(0), "Caption");
    s.encum_max = val(forms, "lblInvenCharStat", Some(1), "Caption");
    for k in 0..6 {
        s.base_stat[k] = val(forms, "txtCharStats", Some(k), "Tag");
    }
    s.class_stealth_tag = val(forms, "lblInvenStats", Some(19), "Tag");
    s.max_hp = val(forms, "lblCharMaxHP", None, "Tag");
    s.rest_rate = val(forms, "lblCharRestRate", None, "Tag");
    s.meditate_regen = val(forms, "txtCharManaRegen", None, "Tag");
    s.max_mana = val(forms, "lblCharMaxMana", None, "Tag");
    s.mana_rate = val(forms, "lblCharManaRate", None, "Tag");
    s.spellcasting = val(forms, "lblCharSC", None, "Tag");
    s.bless_total = val(forms, "lblCharBless", None, "Caption");
    s.weapon_number = g_pair(g, "nGlobalCharWeaponNumber");
    s.weapon_accy = g_pair(g, "nGlobalCharWeaponAccy");
    s.weapon_bs_accy = g_pair(g, "nGlobalCharWeaponBSaccy");
    s.punch_skill = g_pair(g, "nGlobalCharWeaponPunchSkill");
    s.punch_accy = g_pair(g, "nGlobalCharWeaponPunchAccy");
    s.punch_dmg = g_pair(g, "nGlobalCharWeaponPunchDmg");
    s.kick_skill = g_pair(g, "nGlobalCharWeaponKickSkill");
    s.kick_accy = g_pair(g, "nGlobalCharWeaponKickAccy");
    s.kick_dmg = g_pair(g, "nGlobalCharWeaponKickDmg");
    s.jk_skill = g_pair(g, "nGlobalCharWeaponJkSkill");
    s.jk_accy = g_pair(g, "nGlobalCharWeaponJkAccy");
    s.jk_dmg = g_pair(g, "nGlobalCharWeaponJkDmg");
    s.accy_items = g_i(g, "nGlobalCharAccyItems");
    s.accy_other = g_i(g, "nGlobalCharAccyOther");
    s.accy_abils = g_i(g, "nGlobalCharAccyAbils");
    s.hit_magic_non_weapon = g_i(g, "nGlobalCharHitMagicNonWeapon");
    s.qnd_bonus = g_i(g, "nGlobalCharQnDbonus");
    s
}

fn party(forms: &Value) -> PartyFilter {
    let mut p = PartyFilter {
        lair_mode: ctl(forms, "optMonsterFilter", Some(1), "Value")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        monster_damage_text: text(forms, "txtMonsterDamage", None, "Text"),
        ..PartyFilter::default()
    };
    for k in 0..9 {
        p.lair_text[k] = text(forms, "txtMonsterLairFilter", Some(k), "Text");
    }
    p
}

fn equipped(g: &Value) -> EquippedGlobals {
    let hand = |h: usize| {
        let at = |name: &str| g_pair(g, &format!("nGlobalCharWeapon{name}"))[h];
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
        qnd_bonus: g_i(g, "nGlobalCharQnDbonus"),
        accy_abils: g_i(g, "nGlobalCharAccyAbils"),
        accy_other: g_i(g, "nGlobalCharAccyOther"),
        accy_items: g_i(g, "nGlobalCharAccyItems"),
        hands: [hand(0), hand(1)],
    }
}

fn describe_damage(d: &DamageOutput) -> Value {
    serde_json::json!({
        "avg": d.average_damage.scaled(),
        "first": d.first_round_damage.scaled(),
        "min": d.min_round_damage.scaled(),
        "sur": d.surprise_damage.scaled(),
        "surmin": d.surprise_min_damage.scaled(),
        "surchance": d.surprise_damage_chance,
        "swings": d.swings,
    })
}

fn expected_damage(r: &Value) -> Value {
    serde_json::json!({
        "avg": fixture::cur(&r["naveragedamage"]),
        "first": fixture::cur(&r["nfirstrounddamage"]),
        "min": fixture::cur(&r["nminrounddamage"]),
        "sur": fixture::cur(&r["nsurprisedamage"]),
        "surmin": fixture::cur(&r["nsurprisemindamage"]),
        "surchance": r["nsurprisedamagechance"],
        "swings": r["nswings"],
    })
}

fn describe_rounds(r: &CombatRoundInfo) -> Value {
    serde_json::json!({
        "nrtk": r.rtk, "nrtd": r.rtd, "srtk": r.rtk_text, "srtd": r.rtd_text,
        "nsuccess": r.success, "ssuccess": r.success_text,
    })
}

/// One fixture case run under a profile: the port's answer, MME's, and the damage output for
/// `GetDamageOutput` cases.
pub(crate) struct Ran {
    pub id: String,
    pub got: Value,
    pub want: Value,
    pub damage: Option<DamageOutput>,
}

#[test]
fn combat_fixture() {
    check_fixture(Profile::Stock);
}

/// Runs every combat fixture case under `prof`; the layer-1 identity harness reuses it.
pub(crate) fn check_fixture(prof: Profile) {
    let ran = run_cases(prof);
    let run = ran.len();
    let bad: Vec<String> = ran
        .iter()
        .filter(|r| r.got != r.want)
        .map(|r| format!("{}:\n  got  {}\n  want {}", r.id, r.got, r.want))
        .collect();
    assert!(
        bad.is_empty(),
        "{} of {run} cases differ; first:\n{}",
        bad.len(),
        bad.iter().take(6).cloned().collect::<Vec<_>>().join("\n")
    );
}

pub(crate) fn run_cases(prof: Profile) -> Vec<Ran> {
    let fx = fixture::load("combat");
    let table = fx
        .cases
        .iter()
        .find(|c| c["id"] == "table")
        .expect("table case");
    let table = &table["inputs"];
    let spells = spell_table(table);
    let items = item_table(table);
    let classes = class_table(table);
    let races = race_table(table);
    let monsters = monster_table(table);
    let mut ran = Vec::new();
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        if id == "table" {
            continue;
        }
        let inp = &case["inputs"];
        let proc = inp["proc"].as_str().unwrap();
        let args = inp["args"].as_array().unwrap();
        let out = &case["outputs"]["result"];
        let f = |k: usize| num(&args[k]);
        let mut damage = None;
        let (got, want): (Value, Value) = match proc {
            "GetDamageOutput" => {
                let forms = &inp["forms"];
                let g = &inp["globals"];
                let stats = char_stats(forms, g);
                let party = party(forms);
                let ctx = Context {
                    profile: prof,
                    nmr_ver: g_f64(g, "nNMRVer"),
                    avg_level_max_all_stats: g_f64(g, "gAvgLevelMaxAllStats"),
                    hide_record_numbers: false,
                    recovery_only: false,
                    classes: &classes,
                    items: &items,
                    spells: &spells,
                    text: &NoText,
                    stats: &stats,
                    party: &party,
                };
                let ch = character(forms, g);
                let eq = equipped(g);
                let acc = text(forms, "lblInvenCharStat", Some(10), "Tag");
                let caption = text(forms, "lblDatVer", None, "Caption");
                let swings = text(forms, "txtMonsterLairFilter", Some(9), "Text");
                let dout = [
                    text(forms, "txtMonsterDamageOUT", Some(0), "Text"),
                    text(forms, "txtMonsterDamageOUT", Some(1), "Text"),
                ];
                let env = DamageEnv {
                    ctx: &ctx,
                    character: &ch,
                    races: &races,
                    monsters: &monsters,
                    equipped: &eq,
                    cast_text: &NoCasts,
                    dat_ver_caption: &caption,
                    accuracy_tag: &acc,
                    damage_out_text: [&dout[0], &dout[1]],
                    party_swings_text: &swings,
                };
                let mut r = DamageRequest::default();
                let n = |k: usize| arg(args, k).map(|v| i(v) as i32);
                r.single_monster = n(0).unwrap_or(0);
                r.vs_ac = n(1).unwrap_or(0);
                r.vs_dr = n(2).unwrap_or(0);
                r.vs_mr = n(3).unwrap_or(0);
                r.vs_dodge = n(4).unwrap_or(-1);
                r.defense_flags = n(5).unwrap_or(0);
                // args[6] is the speed adjustment, which MME overwrites with 100
                r.spell_immunity_lvl = n(7).unwrap_or(0) as i16;
                r.vs_magic_lvl = n(8).unwrap_or(0) as i16;
                r.attack_flags = n(9).unwrap_or(0);
                r.vs_bs_defense = n(10).unwrap_or(0) as i16;
                r.vs_cold = n(11).unwrap_or(0) as i16;
                r.vs_fire = n(12).unwrap_or(0) as i16;
                r.vs_stone = n(13).unwrap_or(0) as i16;
                r.vs_lightning = n(14).unwrap_or(0) as i16;
                r.vs_water = n(15).unwrap_or(0) as i16;
                r.force_character = arg(args, 16).is_some_and(|v| v.as_bool().unwrap());
                let d = get_damage_output(&env, &r);
                damage = Some(d);
                (describe_damage(&d), expected_damage(out))
            }
            "CalcCombatRounds" => {
                let d = CombatRoundsArgs {
                    damage_out: i(&args[0]) as i32,
                    mob_health: i(&args[1]) as i32,
                    mob_damage: i(&args[2]) as i32,
                    char_health: i(&args[3]) as i32,
                    mob_hp_regen: i(&args[4]) as i32,
                    num_mobs: f(5),
                    override_rtk: f(6),
                    surprise_damage_out: f(7),
                    first_round_damage_out: i(&args[8]) as i32,
                };
                let r = calc_combat_rounds(prof, &d);
                (
                    describe_rounds(&r),
                    serde_json::json!({
                        "nrtk": out["nrtk"], "nrtd": out["nrtd"], "srtk": out["srtk"],
                        "srtd": out["srtd"], "nsuccess": out["nsuccess"], "ssuccess": out["ssuccess"],
                    }),
                )
            }
            "IsMobKillable" => (
                serde_json::json!(is_mob_killable(
                    prof,
                    f(0),
                    i(&args[1]) as i32,
                    f(2),
                    i(&args[3]) as i32,
                    i(&args[4]) as i16,
                    i(&args[5]) as i32
                )),
                out.clone(),
            ),
            "CalcAverageNonZero" => {
                let v: Vec<f64> = args[0]["values"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(num)
                    .collect();
                (serde_json::json!(calc_average_non_zero(&v)), out.clone())
            }
            "ccr_Saturate" => (serde_json::json!(ccr_saturate(f(0))), out.clone()),
            "ccr_SmoothStep" => (
                serde_json::json!(ccr_smooth_step(f(0), f(1), f(2))),
                out.clone(),
            ),
            "ccr_Lerp" => (serde_json::json!(ccr_lerp(f(0), f(1), f(2))), out.clone()),
            "ccr_SafeDiv" => (
                serde_json::json!(ccr_safe_div(f(0), f(1), arg(args, 2).map_or(0.0, num))),
                out.clone(),
            ),
            "ccr_Min" => (serde_json::json!(ccr_min(f(0), f(1))), out.clone()),
            "ccr_Max" => (serde_json::json!(ccr_max(f(0), f(1))), out.clone()),
            other => panic!("unknown proc {other} in {id}"),
        };
        ran.push(Ran {
            id: id.to_string(),
            got,
            want,
            damage,
        });
    }
    ran
}
