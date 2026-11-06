// Fixture tests for the character pipeline (RefreshAll, then PopulateCharacterProfile): see docs/porting.md step 5.
// Expected values come from fixtures/rules/pipeline.json (frmMain's own procedures run by the oracle).
use serde_json::Value;
use touchstone_model::{AbilitySlot, Class, Info, Item, Race, Spell, World, WorldParts};
use touchstone_rules::Profile;
use touchstone_rules::char_panel::ExpRequired;
use touchstone_rules::character::{
    AttackConfig, AttackType, AttackTypeMud, Character, CharacterProfile,
};
use touchstone_rules::gear::ItemManagerRow;
use touchstone_rules::pipeline::{CharacterState, PipelineOptions, refresh_all};
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

/// The rows the cases use, from the fixture header's `data` (field lists, see the generator).
fn world(data: &Value) -> World {
    let rows = |k: &str| data[k].as_array().unwrap().clone();
    let items = rows("items")
        .iter()
        .map(|r| {
            let mut class_rest = [0; 10];
            for (k, c) in r[11].as_array().unwrap().iter().enumerate() {
                class_rest[k] = i(c);
            }
            Item {
                number: i(&r[0]),
                name: String::new(),
                limit: 0,
                encum: i(&r[1]),
                item_type: i(&r[2]),
                use_count: 0,
                price: 0,
                currency: 0,
                min: 0,
                max: 0,
                armour_class: i(&r[3]),
                damage_resist: i(&r[4]),
                weapon_type: i(&r[5]),
                armour_type: i(&r[6]),
                worn: i(&r[7]),
                accy: i(&r[8]),
                gettable: true,
                str_req: i(&r[9]),
                speed: i(&r[10]),
                not_droppable: false,
                destroy_on_death: false,
                retain_after_uses: false,
                class_rest,
                race_rest: [0; 10],
                negate_spell: [0; 10],
                abilities: pairs(&r[12]),
                in_game: true,
                obtained_from: None,
                references: None,
            }
        })
        .collect();
    let spells = rows("spells")
        .iter()
        .map(|r| Spell {
            number: i(&r[0]),
            name: None,
            short: None,
            req_level: i(&r[1]),
            energy_cost: i(&r[2]),
            mana_cost: i(&r[3]),
            min_base: i(&r[4]),
            max_base: i(&r[5]),
            diff: i(&r[6]),
            type_of_resists: i(&r[7]),
            targets: i(&r[8]),
            dur: i(&r[9]),
            att_type: i(&r[10]),
            magery: i(&r[11]),
            magery_lvl: i(&r[12]),
            cap: i(&r[13]),
            max_inc_lvls: i(&r[14]),
            max_inc: i(&r[15]),
            min_inc_lvls: i(&r[16]),
            min_inc: i(&r[17]),
            dur_inc_lvls: i(&r[18]),
            dur_inc: i(&r[19]),
            abilities: pairs(&r[20]),
            learnable: false,
            learned_from: None,
            casted_by: None,
            classes: None,
        })
        .collect();
    let classes = rows("classes")
        .iter()
        .map(|r| Class {
            number: i(&r[0]),
            name: String::new(),
            min_hits: i(&r[1]),
            max_hits: i(&r[2]),
            exp_table: i(&r[3]),
            magery_type: i(&r[4]),
            magery_lvl: i(&r[5]),
            weapon_type: i(&r[6]),
            armour_type: i(&r[7]),
            combat_lvl: i(&r[8]),
            abilities: pairs(&r[9]),
        })
        .collect();
    let races = rows("races")
        .iter()
        .map(|r| Race {
            number: i(&r[0]),
            name: String::new(),
            m_str: i(&r[1]),
            m_int: i(&r[2]),
            m_wil: i(&r[3]),
            m_agl: i(&r[4]),
            m_hea: i(&r[5]),
            m_chm: i(&r[6]),
            x_int: 0,
            x_wil: 0,
            x_str: 0,
            x_hea: 0,
            x_agl: 0,
            x_chm: 0,
            hp_per_lvl: i(&r[7]),
            exp_table: i(&r[8]),
            base_cp: i(&r[9]),
            abilities: pairs(&r[10]),
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
        items,
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

/// frmMain's controls and the globals: the case's, falling back to the setup's.
pub(crate) struct Inputs {
    pub(crate) forms_setup: Value,
    pub(crate) forms: Value,
    pub(crate) globals: Value,
}

impl Inputs {
    pub(crate) fn new(header: &Value, case: &Value) -> Self {
        Self {
            forms_setup: header["setup"]["forms"]["frmMain"].clone(),
            forms: case["inputs"]["forms"]["frmMain"].clone(),
            globals: case["inputs"]["globals"].clone(),
        }
    }
    pub(crate) fn ctl(&self, name: &str) -> &Value {
        let v = &self.forms[name];
        if v.is_null() {
            &self.forms_setup[name]
        } else {
            v
        }
    }
    pub(crate) fn elem(&self, name: &str, idx: usize) -> &Value {
        &self.ctl(name)["$array"][idx.to_string()]
    }
    pub(crate) fn text(&self, name: &str, idx: usize) -> f64 {
        vb::val(self.elem(name, idx)["Text"].as_str().unwrap_or(""))
    }
    pub(crate) fn combo(&self, name: &str) -> i32 {
        let e = self.elem(name, 0);
        if e["ListIndex"].as_i64().unwrap_or(-1) > 0 {
            i(&e["ItemData"]["1"])
        } else {
            0
        }
    }
    pub(crate) fn g(&self, name: &str) -> &Value {
        &self.globals[name]
    }
    pub(crate) fn gi(&self, name: &str) -> i32 {
        self.g(name).as_i64().unwrap_or(0) as i32
    }
    pub(crate) fn gb(&self, name: &str) -> bool {
        self.g(name).as_bool().unwrap_or(false)
    }
    pub(crate) fn character(&self) -> Character {
        let mut stats = [0; 6];
        for (k, s) in stats.iter_mut().enumerate() {
            *s = self.text("txtCharStats", k) as i32;
        }
        let mut bless = [0; 10];
        for (k, b) in bless.iter_mut().enumerate() {
            *b = self.elem("cmbCharBless", k)["ItemData"][0]
                .as_i64()
                .unwrap_or(0) as i32;
        }
        let mut quests = [false; 12];
        for (k, q) in quests.iter_mut().enumerate() {
            *q = self.elem("chkCharQuests", k)["Value"].as_i64() == Some(1);
        }
        let mut quest_options = [0i16; 4];
        for (k, q) in quest_options.iter_mut().enumerate() {
            *q = self.elem("cmbCharQuestOpts", k)["ListIndex"]
                .as_i64()
                .unwrap_or(-1) as i16;
        }
        let mut equipped = [0; 20];
        for (k, e) in self
            .g("nEquippedItem")
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            equipped[k] = i(e);
        }
        let stat_adjustments = self
            .g("char_StatAdjustments")
            .as_array()
            .map_or_else(Vec::new, |a| a.iter().map(i).collect());
        let attack = AttackConfig {
            attack_type: AttackType::from_code(self.gi("nGlobalAttackTypeMME")).unwrap(),
            backstab: self.gb("bGlobalAttackBackstab"),
            backstab_weapon: self.gi("nGlobalAttackBackstabWeapon"),
            martial_art: self.gi("nGlobalAttackMA") as i16,
            spell_num: self.gi("nGlobalAttackSpellNum"),
            spell_lvl: self.gi("nGlobalAttackSpellLVL") as i16,
            manual_p: self.gi("nGlobalAttackManualP"),
            manual_m: self.gi("nGlobalAttackManualM"),
            use_meditate: self.gb("bGlobalAttackUseMeditate"),
            heal_type: self.gi("nGlobalAttackHealType") as i16,
            heal_spell_num: self.gi("nGlobalAttackHealSpellNum"),
            heal_spell_lvl: self.gi("nGlobalAttackHealSpellLVL") as i16,
            heal_rounds: self.gi("nGlobalAttackHealRounds") as i16,
            heal_manual: self.gi("nGlobalAttackHealManual"),
            ..AttackConfig::default()
        };
        Character {
            race: Some(self.combo("cmbGlobalRace")),
            class: Some(self.combo("cmbGlobalClass")),
            level: self.text("txtGlobalLevel", 0) as i32,
            alignment: self.ctl("cmbGlobalAlignment")["ListIndex"]
                .as_i64()
                .unwrap_or(0) as i16,
            global_filter: self.ctl("chkGlobalFilter")["Value"].as_i64() == Some(1),
            stats,
            stat_adjustments,
            equipped,
            bless,
            quests,
            quest_options,
            use_add_weight: self.ctl("chkInvenAddWeight")["Value"].as_i64() == Some(1),
            add_weight: vb::val(self.ctl("txtInvenAddWeight")["Text"].as_str().unwrap_or(""))
                as i32,
            attack,
            ..Character::default()
        }
    }
    pub(crate) fn options(&self) -> PipelineOptions {
        let rows = self.ctl("lvItemManager")["$listview"].as_array().unwrap();
        PipelineOptions {
            hide_class_race: self.ctl("chkInvenHideCharStats")["Value"].as_i64() == Some(1),
            item_manager: rows
                .iter()
                .map(|r| ItemManagerRow {
                    text: r["Text"].as_str().unwrap().to_string(),
                    sub_items: r["Sub"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|s| s.as_str().unwrap().to_string())
                        .collect(),
                })
                .collect(),
            min_item_level: vb::val(self.ctl("txtGlobalMinLVL")["Text"].as_str().unwrap_or(""))
                as i32,
            dat_ver_caption: self.ctl("lblDatVer")["Caption"]
                .as_str()
                .unwrap()
                .to_string(),
            nmr_ver: 1.83,
            avg_level_max_all_stats: 0.0,
            ..PipelineOptions::default()
        }
    }
}

type Check = Result<(), String>;
type WeaponField = fn(&touchstone_rules::gear::WeaponGear) -> i32;

fn eq(name: &str, got: &str, want: &str) -> Check {
    if got == want {
        Ok(())
    } else {
        Err(format!("{name}: got {got:?}, want {want:?}"))
    }
}

fn eqv<T: ToString>(name: &str, got: T, want: &Value) -> Check {
    let w = match want {
        Value::String(s) => s.clone(),
        Value::Bool(b) => i32::from(*b).to_string(),
        other => other.to_string(),
    };
    eq(name, &got.to_string(), &w)
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("not a string: {v}"))
}

fn opt<T: ToString>(v: Option<T>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

fn double(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

fn check_cps(st: &CharacterState, o: &Value) -> Check {
    let Some(sum) = &st.cps.summary else {
        return eq("cps", "", s(&o["cps"]));
    };
    eq("cps_tag", &sum.avail.to_string(), s(&o["cps_tag"]))?;
    let w = s(&o["cps"]);
    let head = sum.caption_head();
    let Some(tail) = w.strip_prefix(&head) else {
        return Err(format!("cps head: got {head:?}, want {w:?}"));
    };
    match sum.exp {
        ExpRequired::NotShown if tail.is_empty() => Ok(()),
        ExpRequired::ALot if tail == "\r\nEXP Req: a lot." => Ok(()),
        ExpRequired::Value(v) => {
            let Some(text) = tail.strip_prefix("\r\nEXP Req: ") else {
                return Err(format!("cps exp line missing: {w:?}"));
            };
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
        _ => Err(format!("cps exp: got {:?}, want tail {tail:?}", sum.exp)),
    }
}

fn check_panels(st: &CharacterState, o: &Value) -> Check {
    check_cps(st, o)?;
    let hp = &st.hit_points;
    eq("rest_tag", &opt(hp.rest_resting), s(&o["rest_tag"]))?;
    eq(
        "rest_cap",
        &hp.rest_caption.clone().unwrap_or_default(),
        s(&o["rest_cap"]),
    )?;
    eq(
        "hp_tag",
        &opt(hp.range.as_ref().map(|r| vb::cstr(r.average))),
        s(&o["hp_tag"]),
    )?;
    eq(
        "hp_cap",
        &hp.max_hp_caption.clone().unwrap_or_default(),
        s(&o["hp_cap"]),
    )?;
    let m = st.magic.clone();
    let mg = |f: fn(&touchstone_rules::char_panel::Magic) -> Option<String>| {
        m.as_ref().and_then(f).unwrap_or_default()
    };
    eq(
        "mana_rate_tag",
        &mg(|m| m.mana_regen.map(|x| x.to_string())),
        s(&o["mana_rate_tag"]),
    )?;
    eq(
        "mana_rate_cap",
        &mg(|m| m.mana_regen_caption.clone()),
        s(&o["mana_rate_cap"]),
    )?;
    eq("medi", &mg(|m| m.medi_ticks.map(vb::cstr)), s(&o["medi"]))?;
    eq(
        "maxmana_tag",
        &mg(|m| m.max_mana.map(|x| x.to_string())),
        s(&o["maxmana_tag"]),
    )?;
    eq(
        "maxmana_cap",
        &mg(|m| m.max_mana_caption.clone()),
        s(&o["maxmana_cap"]),
    )?;
    eq(
        "sc_tag",
        &mg(|m| m.spellcasting.map(|x| x.to_string())),
        s(&o["sc_tag"]),
    )?;
    eq(
        "sc_cap",
        &mg(|m| m.spellcasting_caption.clone()),
        s(&o["sc_cap"]),
    )?;
    eq("mr", &opt(st.magic_res.ok()), s(&o["mr"]))?;
    eq("pick_tag", &opt(st.picklocks.total), s(&o["pick_tag"]))?;
    eq(
        "pick_cap",
        &st.picklocks.caption.clone().unwrap_or_default(),
        s(&o["pick_cap"]),
    )?;

    let w = s(&o["dodge_cap"]);
    if st.dodge.has_summary != w.contains("(After AC)") || !w.starts_with(&st.dodge.caption_head) {
        return Err(format!(
            "dodge caption: got {:?}, want {w:?}",
            st.dodge.caption_head
        ));
    }
    eq("dodge_tag", &vb::cstr(st.dodge.value), s(&o["dodge_tag"]))?;
    let (ec, et) = st
        .encum_level
        .as_ref()
        .map_or((String::new(), String::new()), |e| {
            (e.caption.clone(), e.tag.clone())
        });
    eq("encum_cap", &ec, s(&o["encum_cap"]))?;
    eq("encum_tag", &et, s(&o["encum_tag"]))?;
    eq(
        "bless_cap",
        &opt(st.bless_mana.map(vb::cstr)),
        s(&o["bless_cap"]),
    )?;

    // stat labels: caption per stat in lblLabelArray index order
    const IDX: [usize; 6] = [1, 4, 24, 2, 23, 3];
    for k in 0..6 {
        eq(
            &format!("label {k}"),
            &st.stat_labels.labels[k].clone().unwrap_or_default(),
            s(&o["labels"][IDX[k]]),
        )?;
        let want = vb::val(s(&o["label_tags"][IDX[k]]));
        if st.stats.label_bonus[k] != want {
            return Err(format!(
                "label bonus {k}: got {}, want {want}",
                st.stats.label_bonus[k]
            ));
        }
    }
    Ok(())
}

fn check_stats(st: &CharacterState, o: &Value) -> Check {
    let g = &st.stats;
    for k in 0..47 {
        eq(&format!("stat {k}"), &vb::cstr(g.stat[k]), s(&o["stat"][k]))?;
        eq(
            &format!("tag {k}"),
            &g.stat_tag[k].map(vb::cstr).unwrap_or_default(),
            s(&o["tags"][k]),
        )?;
    }
    for k in 0..6 {
        eq(
            &format!("ctag {k}"),
            &vb::cstr(g.stat_with_bonus[k]),
            s(&o["ctags"][k]),
        )?;
    }
    eq("hpregen", &vb::cstr(g.hp_regen), s(&o["hpregen"]))?;
    eq("manaregen", &vb::cstr(g.mana_regen), s(&o["manaregen"]))?;
    let globals: [(&str, i32); 10] = [
        ("CharAccyItems", g.accy_items),
        ("CharAccyAbils", g.accy_abils),
        ("CharAccyOther", g.accy_other),
        ("CharPlusDodge", g.plus_dodge),
        ("CharPlusMR", g.plus_mr),
        ("CharVileWard", g.vile_ward),
        ("CharShadowAC", g.shadow_ac),
        ("CharWornArmourType", g.worn_armour_type),
        ("CharQnDbonus", g.qnd_bonus),
        ("attack_accy_adj", g.attack_accy_adj),
    ];
    for (name, got) in &globals {
        eqv(name, got, &o[*name])?;
    }
    let tagv = &o["class_stealth_tag"][0];
    if st.class_stealth_tag != vb::val(tagv.as_str().unwrap_or("")) {
        return Err(format!(
            "class_stealth_tag: got {}, want {tagv}",
            st.class_stealth_tag
        ));
    }
    let hand: [(&str, WeaponField); 20] = [
        ("Number", |w| w.number),
        ("Encum", |w| w.encum),
        ("Accy", |w| w.accy),
        ("Crit", |w| w.crit),
        ("STR", |w| w.strength),
        ("AGI", |w| w.agility),
        ("MaxDmg", |w| w.max_dmg),
        ("BSaccy", |w| w.bs_accy),
        ("BSmindmg", |w| w.bs_min_dmg),
        ("BSmaxdmg", |w| w.bs_max_dmg),
        ("PunchSkill", |w| w.punch_skill),
        ("PunchAccy", |w| w.punch_accy),
        ("PunchDmg", |w| w.punch_dmg),
        ("KickSkill", |w| w.kick_skill),
        ("KickAccy", |w| w.kick_accy),
        ("KickDmg", |w| w.kick_dmg),
        ("JkSkill", |w| w.jumpkick_skill),
        ("JkAccy", |w| w.jumpkick_accy),
        ("JkDmg", |w| w.jumpkick_dmg),
        ("Stealth", |w| w.stealth),
    ];
    for (name, f) in hand {
        for k in 0..2 {
            eqv(
                &format!("weapon{name}[{k}]"),
                f(&g.weapons[k]),
                &o[format!("weapon{name}")][k],
            )?;
        }
    }
    Ok(())
}

fn check_setup(st: &CharacterState, o: &Value) -> Check {
    let Some(cs) = &st.class_setup else {
        return Err("SetupClass did nothing".into());
    };
    for k in 0..4 {
        eqv(&format!("qopt {k}"), cs.quest_options[k], &o["qopts"][k])?;
    }
    eqv("antimagic", i32::from(cs.anti_magic), &o["antimagic"])?;
    eqv("spellopt", i32::from(cs.has_magic), &o["spellopt"])?;
    eqv(
        "mlvl_idx",
        cs.magery_level_index.unwrap_or(-1),
        &o["mlvl_idx"],
    )?;
    eqv(
        "magery_idx",
        cs.magery_index.unwrap_or(-1),
        &o["magery_idx"],
    )?;
    eqv("fra_pick", i32::from(cs.has_picks), &o["fra_pick"])?;
    eqv("fra_sc", i32::from(cs.has_magic), &o["fra_sc"])?;
    eqv("MageryLVL", cs.magery_lvl, &o["CharMageryLVL"])?;
    eqv("Magery", cs.magery, &o["CharMagery"])
}

fn check_heal(st: &CharacterState, o: &Value) -> Check {
    let a = st.attack_config();
    eqv("heal value", a.heal_value, &o["healvalue"])?;
    eqv("heal rounds", a.heal_rounds, &o["healrounds"])?;
    eqv("heal lvl", a.heal_spell_lvl, &o["heallvl"])?;
    if a.heal_cost != double(&o["healcost"]) {
        return Err(format!(
            "heal cost: got {}, want {}",
            a.heal_cost, o["healcost"]
        ));
    }
    Ok(())
}

fn profile_fields(p: &CharacterProfile) -> Vec<(&'static str, f64)> {
    let b = |x: bool| f64::from(i32::from(x));
    let mut v = vec![
        ("bclassstealth", b(p.class_stealth)),
        ("bisloadedcharacter", b(p.is_loaded_character)),
        ("bracestealth", b(p.race_stealth)),
        ("naccuracy", p.accuracy),
        ("nagi", f64::from(p.agi)),
        ("nalign", f64::from(p.align)),
        ("ncha", f64::from(p.cha)),
        ("nclass", f64::from(p.class)),
        ("ncombat", f64::from(p.combat)),
        ("ncrit", f64::from(p.crit)),
        ("ndamagethreshold", p.damage_threshold),
        ("ndodge", f64::from(p.dodge)),
        ("nencumcurrent", f64::from(p.encum_current)),
        ("nencummax", f64::from(p.encum_max)),
        ("nencumpct", f64::from(p.encum_pct)),
        ("nhea", f64::from(p.hea)),
        ("nhitmagic", f64::from(p.hit_magic)),
        ("nhitmagicnonweapon", f64::from(p.hit_magic_non_weapon)),
        ("nhp", p.hp),
        ("nhpregen", p.hp_regen),
        ("nint", f64::from(p.int)),
        ("nlevel", f64::from(p.level)),
        ("nmanaregen", p.mana_regen),
        ("nmaxmana", p.max_mana),
        ("nmeditaterate", p.meditate_rate),
        ("nparty", f64::from(p.party)),
        ("nplusbsaccy", f64::from(p.plus_bs_accy)),
        ("nplusbsmaxdmg", f64::from(p.plus_bs_max_dmg)),
        ("nplusbsmindmg", f64::from(p.plus_bs_min_dmg)),
        ("nplusmaxdamage", f64::from(p.plus_max_damage)),
        ("nplusmindamage", f64::from(p.plus_min_damage)),
        ("nrace", f64::from(p.race)),
        ("nspellattackcost", p.spell_attack_cost),
        ("nspellcasting", f64::from(p.spellcasting)),
        ("nspelldmgbonus", f64::from(p.spell_dmg_bonus)),
        ("nspelloverhead", p.spell_overhead),
        ("nstealth", f64::from(p.stealth)),
        ("nstr", f64::from(p.str_)),
        ("nwalkspeed", p.walk_speed),
        ("nwis", f64::from(p.wis)),
    ];
    for (k, name) in ["nmaplusaccy", "nmaplusdmg", "nmaplusskill"]
        .iter()
        .enumerate()
    {
        let arr = match k {
            0 => p.ma_plus_accy,
            1 => p.ma_plus_dmg,
            _ => p.ma_plus_skill,
        };
        for (j, x) in arr.iter().enumerate() {
            let label: &'static str = match (k, j) {
                (0, 0) => "nmaplusaccy0",
                (0, 1) => "nmaplusaccy1",
                (0, _) => "nmaplusaccy2",
                (1, 0) => "nmaplusdmg0",
                (1, 1) => "nmaplusdmg1",
                (1, _) => "nmaplusdmg2",
                (_, 0) => "nmaplusskill0",
                (_, 1) => "nmaplusskill1",
                _ => "nmaplusskill2",
            };
            let _ = name;
            v.push((label, f64::from(*x)));
        }
    }
    v
}

fn check_profile(name: &str, got: &CharacterProfile, want: &Value) -> Check {
    for (key, g) in profile_fields(got) {
        let w = if let Some(base) = key.strip_prefix("nmaplus") {
            let (kind, ix) = base.split_at(base.len() - 1);
            &want[format!("nmaplus{kind}")][ix.parse::<usize>().unwrap()]
        } else {
            &want[key]
        };
        let wf = match w {
            Value::Bool(b) => f64::from(i32::from(*b)),
            other => double(other),
        };
        if g != wf {
            return Err(format!("{name}.{key}: got {g}, want {wf}"));
        }
    }
    Ok(())
}

fn check_adapters(st: &CharacterState) -> Check {
    let a = st.attack_profile();
    let p = &st.profile;
    if a.strength != p.str_ || a.accuracy != p.accuracy || a.class != p.class || a.crit != p.crit {
        return Err("attack_profile does not mirror the profile".into());
    }
    let sc = st.spell_caster();
    if sc.spellcasting != p.spellcasting || sc.max_mana != p.max_mana {
        return Err("spell_caster does not mirror the profile".into());
    }
    let e = st.equipped_globals();
    if e.hands[0].number != st.stats.weapons[0].number || e.accy_items != st.stats.accy_items {
        return Err("equipped_globals does not mirror the stats".into());
    }
    Ok(())
}

// Expected values: fixtures/rules/pipeline.json (frmMain RefreshAll and modMain PopulateCharacterProfile).
#[test]
fn pipeline_matches_the_oracle() {
    let fx = fixture::load("pipeline");
    let world = world(&fx.header["data"]);
    let mut failures = Vec::new();
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        let inputs = Inputs::new(&fx.header, case);
        let st = match refresh_all(
            Profile::Stock,
            &world,
            &inputs.character(),
            &inputs.options(),
        ) {
            Ok(st) => st,
            Err(e) => {
                failures.push(format!("{id}: {e}"));
                continue;
            }
        };
        let o = &case["outputs"];
        let events = case["events"].as_array().map_or(0, Vec::len);
        let result: Check = (|| {
            if events == 0 && st.stats.error.is_some() {
                return Err(format!("unexpected stats error {:?}", st.stats.error));
            }
            check_stats(&st, o)?;
            check_setup(&st, o)?;
            check_panels(&st, o)?;
            check_heal(&st, o)?;
            check_profile("profile", &st.profile, &o["profile.refs"][0])?;
            let bs = st.profile_for(&world, AttackTypeMud::Surprise, 0);
            check_profile("profile_bs", &bs, &o["profile_bs.refs"][0])?;
            check_adapters(&st)
        })();
        if let Err(e) = result {
            failures.push(format!("{id}: {e}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases failed, first:\n{}",
        failures.len(),
        fx.cases.len(),
        failures
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
