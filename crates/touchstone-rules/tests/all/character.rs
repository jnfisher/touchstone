// Fixture tests for the character family (FAM-05, modMain half): see docs/porting.md step 5.
// Expected values come from fixtures/rules/character.json (MME's own code run by the oracle).
use crate::fixture;
use serde_json::{Value, json};
use touchstone_model::{AbilitySlot, Class, Item, Spell};
use touchstone_rules::Profile;
use touchstone_rules::character::*;
use touchstone_rules::spells::SpellText;
use touchstone_vb6 as vb;

fn i(v: &Value) -> i64 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}"))
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

fn spell_table(case: &Value) -> Vec<Spell> {
    case["inputs"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let f = r.as_array().unwrap();
            let n = |k: usize| i(&f[k]) as i32;
            let mut abilities = [AbilitySlot::default(); 10];
            for (k, p) in f[26].as_array().unwrap().chunks(2).enumerate() {
                abilities[k] = AbilitySlot {
                    ability: i(&p[0]) as i32,
                    value: i(&p[1]) as i32,
                };
            }
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
                abilities,
                learnable: n(22) != 0,
                learned_from: text(n(23)),
                casted_by: text(n(24)),
                classes: Some(f[25].as_str().unwrap().to_string()).filter(|s| !s.is_empty()),
            }
        })
        .collect()
}

fn item_table(case: &Value) -> Vec<Item> {
    case["inputs"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut abilities = [AbilitySlot::default(); 20];
            for t in r[5].as_array().unwrap().chunks(3) {
                abilities[i(&t[0]) as usize] = AbilitySlot {
                    ability: i(&t[1]) as i32,
                    value: i(&t[2]) as i32,
                };
            }
            Item {
                number: i(&r[0]) as i32,
                name: r[1].as_str().unwrap().to_string(),
                limit: 0,
                encum: 0,
                item_type: i(&r[2]) as i32,
                use_count: 0,
                price: 0,
                currency: 0,
                min: 0,
                max: 0,
                armour_class: 0,
                damage_resist: 0,
                weapon_type: i(&r[3]) as i32,
                armour_type: 0,
                worn: 0,
                accy: 0,
                gettable: false,
                str_req: i(&r[4]) as i32,
                speed: 0,
                not_droppable: false,
                destroy_on_death: false,
                retain_after_uses: false,
                class_rest: [0; 10],
                race_rest: [0; 10],
                negate_spell: [0; 10],
                abilities,
                in_game: false,
                obtained_from: None,
                references: None,
            }
        })
        .collect()
}

fn class_table(case: &Value) -> Vec<Class> {
    case["inputs"]["classes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut abilities = [AbilitySlot::default(); 10];
            for (k, p) in r[2].as_array().unwrap().chunks(2).enumerate() {
                abilities[k] = AbilitySlot {
                    ability: i(&p[0]) as i32,
                    value: i(&p[1]) as i32,
                };
            }
            Class {
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
                abilities,
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

fn g_s(g: &Value, name: &str) -> String {
    g.get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
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
        anti_magic: ctl(forms, "chkCharAntiMagic", None, "Value").is_some_and(|v| i(v) == 1),
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
    a.use_meditate = g_b(g, "bGlobalAttackUseMeditate");
    a.heal_type = g_i(g, "nGlobalAttackHealType") as i16;
    a.heal_spell_num = g_i(g, "nGlobalAttackHealSpellNum");
    a.heal_spell_lvl = g_i(g, "nGlobalAttackHealSpellLVL") as i16;
    a.heal_rounds = g_i(g, "nGlobalAttackHealRounds") as i16;
    a.heal_manual = g_i(g, "nGlobalAttackHealManual");
    a.heal_value = g_i(g, "nGlobalAttackHealValue");
    a.heal_cost = g_f64(g, "nGlobalAttackHealCost");
    if let Some(l) = g.get("nEquippedItem").and_then(Value::as_array) {
        for (k, v) in l.iter().enumerate() {
            ch.equipped[k] = i(v) as i32;
        }
    }
    if let Some(l) = g.get("nLearnedSpells").and_then(Value::as_array) {
        for (k, v) in l.iter().enumerate() {
            ch.learned.spells[k] = i(v) as i32;
        }
    }
    ch.learned.class = g_i(g, "nLearnedSpellClass") as i16;
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

fn profile_json(t: &CharacterProfile) -> Value {
    let mut m = serde_json::Map::new();
    let mut put = |k: &str, v: Value| {
        m.insert(k.to_string(), v);
    };
    put("bisloadedcharacter", json!(t.is_loaded_character));
    put("nparty", json!(t.party));
    put("nhp", json!(t.hp));
    put("nhpregen", json!(t.hp_regen));
    put("ndamagethreshold", json!(t.damage_threshold));
    put("nspellattackcost", json!(t.spell_attack_cost));
    put("nspelloverhead", json!(t.spell_overhead));
    put("nspelldmgbonus", json!(t.spell_dmg_bonus));
    put("nmaxmana", json!(t.max_mana));
    put("nspellcasting", json!(t.spellcasting));
    put("nmanaregen", json!(t.mana_regen));
    put("nmeditaterate", json!(t.meditate_rate));
    put("nencumpct", json!(t.encum_pct));
    put("nencumcurrent", json!(t.encum_current));
    put("nencummax", json!(t.encum_max));
    put("nwalkspeed", json!(t.walk_speed));
    put("naccuracy", json!(t.accuracy));
    put("nlevel", json!(t.level));
    put("nclass", json!(t.class));
    put("nrace", json!(t.race));
    put("nalign", json!(t.align));
    put("ncombat", json!(t.combat));
    put("nstr", json!(t.str_));
    put("nagi", json!(t.agi));
    put("ncha", json!(t.cha));
    put("nwis", json!(t.wis));
    put("nint", json!(t.int));
    put("nhea", json!(t.hea));
    put("ncrit", json!(t.crit));
    put("ndodge", json!(t.dodge));
    put("nplusmaxdamage", json!(t.plus_max_damage));
    put("nplusmindamage", json!(t.plus_min_damage));
    put("nplusbsaccy", json!(t.plus_bs_accy));
    put("nplusbsmindmg", json!(t.plus_bs_min_dmg));
    put("nplusbsmaxdmg", json!(t.plus_bs_max_dmg));
    put("nmaplusskill", json!(t.ma_plus_skill));
    put("nmaplusaccy", json!(t.ma_plus_accy));
    put("nmaplusdmg", json!(t.ma_plus_dmg));
    put("nstealth", json!(t.stealth));
    put("bclassstealth", json!(t.class_stealth));
    put("bracestealth", json!(t.race_stealth));
    put("nhitmagic", json!(t.hit_magic));
    put("nhitmagicnonweapon", json!(t.hit_magic_non_weapon));
    Value::Object(m)
}

fn heal_json(a: &AttackConfig) -> Value {
    json!({"value": a.heal_value, "cost": a.heal_cost, "lvl": a.heal_spell_lvl, "rounds": a.heal_rounds})
}

fn learned_json(l: &LearnedSpells) -> Value {
    json!({"spells": l.spells.to_vec(), "cls": l.class})
}

/// Outputs the case expects, restricted to the keys `want` lists.
fn pick(outputs: &Value, keys: &[&str]) -> Value {
    let mut m = serde_json::Map::new();
    for k in keys {
        if let Some(v) = outputs.get(*k) {
            m.insert((*k).to_string(), v.clone());
        }
    }
    Value::Object(m)
}

#[test]
fn character_fixture() {
    let fx = fixture::load("character");
    let table = fx
        .cases
        .iter()
        .find(|c| c["id"] == "table")
        .expect("table case");
    let spells = spell_table(table);
    let items = item_table(table);
    let classes = class_table(table);
    let mut bad = Vec::new();
    let mut run = 0;
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        if id == "table" {
            continue;
        }
        run += 1;
        let inp = &case["inputs"];
        let forms = &inp["forms"];
        let g = &inp["globals"];
        let out = &case["outputs"];
        let stats = char_stats(forms, g);
        let party = party(forms);
        let ctx = Context {
            profile: Profile::Stock,
            nmr_ver: g_f64(g, "nNMRVer"),
            avg_level_max_all_stats: g_f64(g, "gAvgLevelMaxAllStats"),
            hide_record_numbers: g_b(g, "bHideRecordNumbers"),
            recovery_only: g_b(g, "bGlobal_cephRecoveryOnly"),
            classes: &classes,
            items: &items,
            spells: &spells,
            text: &NoText,
            stats: &stats,
            party: &party,
        };
        let mut ch = character(forms, g);
        let proc = inp["proc"].as_str().unwrap();
        let args = inp["args"].as_array().unwrap();
        let (got, want): (Value, Value) = match proc {
            "PopulateCharacterProfile" => {
                let mut t = CharacterProfile::default();
                if let Some(p) = args[0]["fields"].get("nParty") {
                    t.party = i(p) as i16;
                }
                let flag = |k: usize| args[k].as_bool().unwrap();
                populate_character_profile(
                    &ctx,
                    &ch,
                    &mut t,
                    flag(1),
                    flag(2),
                    AttackTypeMud::from_code(i(&args[3]) as i32).unwrap(),
                    i(&args[4]) as i32,
                    flag(5),
                );
                (profile_json(&t), out["result.refs"][0].clone())
            }
            "SetCurrentAttackTypeConfig" => {
                let prev = g_s(g, "sGlobalAttackConfig");
                let k = set_current_attack_type_config(&ctx, &ch, &prev);
                let cdv = if k.changed {
                    prev
                } else {
                    g_s(g, "sCharDamageVsMonsterConfig")
                };
                let mut got = json!({"cfg": k.config, "cdv": cdv});
                if out.get("cdv").is_none() {
                    got.as_object_mut().unwrap().remove("cdv");
                }
                (got, pick(out, &["cfg", "cdv"]))
            }
            "SetCharDefenseDescription" => {
                let prev = g_s(g, "sGlobalCharDefenseDescription");
                let dont = g_b(g, "bDontPromptCalcCharMonsterDamage");
                let (desc, dont) = match set_char_defense_description(&ctx, &ch, &prev) {
                    Some(d) => (d.description, dont && !d.reset_prompt),
                    None => (prev, dont),
                };
                (
                    json!({"desc": desc, "dont": dont}),
                    pick(out, &["desc", "dont"]),
                )
            }
            "RefreshCombatHealingValues" => {
                refresh_combat_healing_values(&ctx, &mut ch);
                (
                    heal_json(&ch.attack),
                    pick(out, &["value", "cost", "lvl", "rounds"]),
                )
            }
            "GetCurrentAttackName" => {
                let name = get_current_attack_name(&ctx, &mut ch, args[0].as_bool().unwrap());
                let mut v = heal_json(&ch.attack);
                v["result"] = json!(name);
                (v, pick(out, &["value", "cost", "lvl", "rounds", "result"]))
            }
            "bCharIsEvilAligned" => (json!(char_is_evil_aligned(&ch)), out["result"].clone()),
            "GetCharActiveVileWard" => (
                json!(get_char_active_vile_ward(Profile::Stock)),
                out["result"].clone(),
            ),
            "ClearLearnedSpells" | "LearnSpell" | "UnLearnSpell" | "LearnOrUnlearnSpell" => {
                let n = args.first().map_or(0, |a| i(&a["value"]) as i32);
                match proc {
                    "ClearLearnedSpells" => ch.clear_learned_spells(),
                    "LearnSpell" => ch.learn_spell(n),
                    "UnLearnSpell" => ch.unlearn_spell(n),
                    _ => ch.learn_or_unlearn_spell(n),
                }
                (learned_json(&ch.learned), pick(out, &["spells", "cls"]))
            }
            other => panic!("unknown proc {other} in {id}"),
        };
        if got != want {
            bad.push(format!("{id}:\n  got  {got}\n  want {want}"));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {run} cases differ; first:\n{}",
        bad.len(),
        bad.iter().take(6).cloned().collect::<Vec<_>>().join("\n")
    );
}
