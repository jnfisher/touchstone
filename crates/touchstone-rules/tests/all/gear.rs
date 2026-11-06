// Fixture tests for the gear family (FAM-04, first half): see docs/porting.md step 5.
// Expected values come from fixtures/rules/gear.json (MME's own code run by the oracle).
use serde_json::Value;
use touchstone_model::{AbilitySlot, Class, Info, Item, Race, Spell, World, WorldParts};
use touchstone_rules::Profile;
use touchstone_rules::gear::*;
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
    let items = data["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut class_rest = [0; 10];
            for (k, c) in r["cr"].as_array().unwrap().iter().enumerate() {
                class_rest[k] = i(c);
            }
            Item {
                number: i(&r["n"]),
                name: r["name"].as_str().unwrap().to_string(),
                limit: 0,
                encum: i(&r["enc"]),
                item_type: i(&r["t"]),
                use_count: 0,
                price: 0,
                currency: 0,
                min: 0,
                max: 0,
                armour_class: i(&r["ac"]),
                damage_resist: i(&r["dr"]),
                weapon_type: i(&r["wt"]),
                armour_type: i(&r["at"]),
                worn: i(&r["w"]),
                accy: i(&r["acc"]),
                gettable: true,
                // gear.json's rows carry no speed or strength requirement; gear2.json's do
                str_req: r["sr"].as_i64().unwrap_or(0) as i32,
                speed: r["sp"].as_i64().unwrap_or(0) as i32,
                not_droppable: false,
                destroy_on_death: false,
                retain_after_uses: false,
                class_rest,
                race_rest: [0; 10],
                negate_spell: [0; 10],
                abilities: pairs(&r["ab"]),
                in_game: true,
                obtained_from: None,
                references: None,
            }
        })
        .collect();
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
            mana_cost: 0,
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
            min_hits: 0,
            max_hits: 0,
            exp_table: 0,
            magery_type: 0,
            magery_lvl: 0,
            weapon_type: i(&r["wt"]),
            armour_type: i(&r["at"]),
            // gear.json's rows carry no combat level; gear2.json's do
            combat_lvl: r["cl"].as_i64().unwrap_or(0) as i32,
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
            exp_table: 0,
            base_cp: 0,
            abilities: pairs(&r["ab"]),
        })
        .collect();
    World::new(WorldParts {
        info: Info {
            nmr_version: String::new(),
            dat_file_version: String::new(),
            content_version: None,
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

/// frmMain's controls: the setup's, with the case's replacing those of the same name.
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
    fn text(&self, name: &str, idx: usize) -> f64 {
        vb::val(self.elem(name, idx)["Text"].as_str().unwrap_or(""))
    }
    /// A class or race combo: the number in ItemData(1) when ListIndex is 1, else 0.
    fn combo_number(&self, name: &str) -> i32 {
        let e = self.elem(name, 0);
        if e["ListIndex"].as_i64().unwrap_or(-1) > 0 {
            i(&e["ItemData"]["1"])
        } else {
            0
        }
    }
    fn character(&self) -> Character {
        Character {
            level: self.text("txtGlobalLevel", 0).round() as i32,
            class: self.combo_number("cmbGlobalClass"),
            race: self.combo_number("cmbGlobalRace"),
            alignment: self.ctl("cmbGlobalAlignment")["ListIndex"]
                .as_i64()
                .unwrap_or(0) as i32,
            min_item_level: vb::val(self.ctl("txtGlobalMinLVL")["Text"].as_str().unwrap_or(""))
                as i32,
        }
    }
    fn rows(&self) -> Vec<ItemManagerRow> {
        self.ctl("lvItemManager")["$listview"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| ItemManagerRow {
                text: r["Text"].as_str().unwrap().to_string(),
                sub_items: r["Sub"]
                    .as_array()
                    .map(|a| a.iter().map(|x| x.as_str().unwrap().to_string()).collect())
                    .unwrap_or_default(),
            })
            .collect()
    }
    fn bless_slots(&self) -> [i32; BLESS_SLOTS] {
        let mut out = [0; BLESS_SLOTS];
        for (k, o) in out.iter_mut().enumerate() {
            let e = self.elem("cmbCharBless", k);
            if !e.is_null() {
                *o = i(&e["ItemData"][0]);
            }
        }
        out
    }
}

fn equipped(case: &Value) -> [i32; EQUIP_SLOTS] {
    let mut out = [0; EQUIP_SLOTS];
    if let Some(a) = case["inputs"]["globals"]["nEquippedItem"].as_array() {
        for (k, v) in a.iter().enumerate() {
            out[k] = i(v);
        }
    }
    out
}

fn handled_error(case: &Value, source: &str) -> Option<i32> {
    case["events"].as_array()?.iter().find_map(|e| {
        (e["kind"] == "handled_error" && e["source"] == source).then(|| i(&e["number"]))
    })
}

fn gear_input<'a>(
    world: &'a World,
    header: &'a Value,
    case: &Value,
) -> Result<GearInput<'a>, String> {
    let f = Forms::new(header, case);
    let mut base_stats = [0.0; 6];
    for (k, b) in base_stats.iter_mut().enumerate() {
        *b = f.text("txtCharStats", k);
    }
    let mut adjustments = [0; STAT_COUNT];
    if let Some(a) = case["inputs"]["globals"]["char_StatAdjustments"].as_array() {
        for (k, v) in a.iter().enumerate() {
            adjustments[k] = i(v);
        }
    }
    let mut eq = [0; EQUIP_SLOTS];
    let mut carried_equipped = equipped(case);
    std::mem::swap(&mut eq, &mut carried_equipped);
    Ok(GearInput {
        profile: Profile::Stock,
        world,
        dat_ver_caption: header["setup"]["forms"]["frmMain"]["lblDatVer"]["Caption"]
            .as_str()
            .unwrap(),
        character: f.character(),
        base_stats,
        hide_class_race: f.ctl("chkInvenHideCharStats")["Value"].as_i64() == Some(1),
        add_weight: (f.ctl("chkInvenAddWeight")["Value"].as_i64() == Some(1))
            .then(|| vb::val(f.ctl("txtInvenAddWeight")["Text"].as_str().unwrap_or(""))),
        equipped: eq,
        carried: carried_items(world, &f.rows()).map_err(|e| format!("{e:?}"))?,
        adjustments,
        bless_slots: f.bless_slots(),
        martial_arts: case["inputs"]["globals"]["nGlobalAttackTypeMME"].as_i64() == Some(4),
    })
}

fn run_calc(world: &World, header: &Value, case: &Value) -> Result<(), String> {
    let input = gear_input(world, header, case)?;
    let got = calc_gear_stats(&input).map_err(|e| format!("{e:?}"))?;
    let out = &case["outputs"];
    let want_err = handled_error(case, "CalcCharacterStats");
    if got.error != want_err {
        return Err(format!("error: got {:?}, want {want_err:?}", got.error));
    }
    for (k, w) in out["stat"].as_array().unwrap().iter().enumerate() {
        if got.caption(k) != w.as_str().unwrap() {
            return Err(format!("stat[{k}]: got {}, want {w}", got.caption(k)));
        }
    }
    for (k, w) in out["tags"].as_array().unwrap().iter().enumerate() {
        let g = vb::cstr(got.stat_with_bonus[k]);
        if g != w.as_str().unwrap() {
            return Err(format!("tag[{k}]: got {g}, want {w}"));
        }
    }
    let ints: [(&str, i32); 9] = [
        ("AccyItems", got.accy_items),
        ("AccyAbils", got.accy_abils),
        ("AccyOther", got.accy_other),
        ("PlusDodge", got.plus_dodge),
        ("PlusMR", got.plus_mr),
        ("VileWard", got.vile_ward),
        ("ShadowAC", got.shadow_ac),
        ("WornArmourType", got.worn_armour_type),
        ("HitMagicNonWeapon", got.hit_magic_non_weapon),
    ];
    for (name, g) in ints {
        if i(&out[name]) != g {
            return Err(format!("{name}: got {g}, want {}", out[name]));
        }
    }
    type Getter = fn(&WeaponGear) -> i32;
    let wf: [(&str, Getter); 20] = [
        ("WeaponNumber", |w| w.number),
        ("WeaponEncum", |w| w.encum),
        ("WeaponAccy", |w| w.accy),
        ("WeaponCrit", |w| w.crit),
        ("WeaponSTR", |w| w.strength),
        ("WeaponAGI", |w| w.agility),
        ("WeaponMaxDmg", |w| w.max_dmg),
        ("WeaponBSaccy", |w| w.bs_accy),
        ("WeaponBSmindmg", |w| w.bs_min_dmg),
        ("WeaponBSmaxdmg", |w| w.bs_max_dmg),
        ("WeaponPunchSkill", |w| w.punch_skill),
        ("WeaponPunchAccy", |w| w.punch_accy),
        ("WeaponPunchDmg", |w| w.punch_dmg),
        ("WeaponKickSkill", |w| w.kick_skill),
        ("WeaponKickAccy", |w| w.kick_accy),
        ("WeaponKickDmg", |w| w.kick_dmg),
        ("WeaponJkSkill", |w| w.jumpkick_skill),
        ("WeaponJkAccy", |w| w.jumpkick_accy),
        ("WeaponJkDmg", |w| w.jumpkick_dmg),
        ("WeaponStealth", |w| w.stealth),
    ];
    for (name, get) in wf {
        for k in 0..2 {
            if i(&out[name][k]) != get(&got.weapons[k]) {
                return Err(format!(
                    "{name}[{k}]: got {}, want {}",
                    get(&got.weapons[k]),
                    out[name][k]
                ));
            }
        }
    }
    if want_err.is_none() {
        let p = out["probe"].as_array().unwrap();
        let g = [
            got.encum_pct.to_string(),
            got.shadow_ac.to_string(),
            got.weapon_hit_magic.to_string(),
            vb::cstr_bool(got.class_stealth).to_string(),
            vb::cstr_bool(got.race_stealth).to_string(),
        ];
        for k in 0..5 {
            if p[k].as_str().unwrap() != g[k] {
                return Err(format!("probe[{k}]: got {}, want {}", g[k], p[k]));
            }
        }
    }
    Ok(())
}

fn run_bless(world: &World, header: &Value, case: &Value) -> Result<(), String> {
    let f = Forms::new(header, case);
    let worn = i(&case["inputs"]["globals"]["nGlobalCharWornArmourType"]);
    let got = refresh_char_bless(
        Profile::Stock,
        world,
        f.character().level,
        &f.bless_slots(),
        worn,
    );
    if got.error != handled_error(case, "RefreshCharBless") {
        return Err(format!("error: got {:?}", got.error));
    }
    for (k, w) in case["outputs"]["bless_Stats"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        if w.as_f64().unwrap() != got.stats[k] {
            return Err(format!("bless_Stats({k}): got {}, want {w}", got.stats[k]));
        }
    }
    for (k, w) in case["outputs"]["slots"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        if i(w) != got.spells[k] {
            return Err(format!("slot {k}: got {}, want {w}", got.spells[k]));
        }
    }
    Ok(())
}

fn run(world: &World, header: &Value, case: &Value) -> Result<(), String> {
    let id = case["id"].as_str().unwrap();
    let inp = &case["inputs"];
    let out = &case["outputs"];
    if id.starts_with("calc-") {
        return run_calc(world, header, case);
    }
    if id.starts_with("bless-") {
        return run_bless(world, header, case);
    }
    if id.starts_with("buffed-") {
        let a = inp["args"].as_array().unwrap();
        let got = pasted_stat_is_buffed(a[0].as_str().unwrap(), a[1].as_str().unwrap());
        return (got == out["result"].as_bool().unwrap())
            .then_some(())
            .ok_or_else(|| format!("got {got}, want {}", out["result"]));
    }
    let f = Forms::new(header, case);
    let who = f.character();
    let eq = equipped(case);
    if id.starts_with("usable-") {
        let a = inp["args"].as_array().unwrap();
        let got = item_is_usable_by_char(
            Profile::Stock,
            world,
            &who,
            &eq,
            i(&a[0]),
            a[1].as_bool().unwrap(),
            a[2].as_bool().unwrap(),
        );
        return (got == out["result"].as_bool().unwrap())
            .then_some(())
            .ok_or_else(|| format!("got {got}, want {}", out["result"]));
    }
    if id.starts_with("accum-") {
        let a = inp["steps"][0]["args"].as_array().unwrap();
        let mut acc = StatBonus::default();
        let got = accum_item_stat_bonus(
            Profile::Stock,
            world,
            &who,
            &eq,
            i(&a[0]),
            a[1].as_bool().unwrap().then(|| i(&a[2])),
            &mut acc,
        );
        let refs = out["r.refs"].as_array().unwrap();
        let bonus: Vec<i32> = refs[0].as_array().unwrap().iter().map(i).collect();
        let src: Vec<&str> = refs[1]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect();
        if got != out["r"].as_bool().unwrap() || bonus != acc.bonus || src != acc.sources {
            return Err(format!(
                "got {got} {:?} {:?}, want {} {bonus:?} {src:?}",
                acc.bonus, acc.sources, out["r"]
            ));
        }
        return Ok(());
    }
    if id.starts_with("carry-") {
        let n = i(&inp["steps"][0]["args"][0]);
        let active = item_is_carry_active(Profile::Stock, world, &who, &eq, n);
        let flagged = item_is_flagged_carried(n, &f.rows());
        if active != out["active"].as_bool().unwrap()
            || flagged != out["flagged"].as_bool().unwrap()
        {
            return Err(format!(
                "got {active} {flagged}, want {} {}",
                out["active"], out["flagged"]
            ));
        }
        return Ok(());
    }
    Err("unknown case kind".into())
}

#[test]
fn gear_cases_match_the_oracle() {
    let fx = fixture::load("gear");
    let w = world(&fx.header["data"]);
    let mut fails = Vec::new();
    for case in &fx.cases {
        if let Err(e) = run(&w, &fx.header, case) {
            fails.push(format!("{}: {e}", case["id"].as_str().unwrap()));
        }
    }
    assert!(
        fails.is_empty(),
        "{} of {} cases differ; first ones:\n{}",
        fails.len(),
        fx.cases.len(),
        fails
            .iter()
            .take(15)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The second-half inputs: quest boxes, quest options, the global filter and the attack type.
fn second_input(header: &Value, case: &Value) -> SecondHalfInput {
    let f = Forms::new(header, case);
    let mut q = SecondHalfInput::default();
    for x in 0..12 {
        q.quests[x] = f.elem("chkCharQuests", x)["Value"].as_i64() == Some(1);
    }
    for (k, o) in q.quest_options.iter_mut().enumerate() {
        *o = f.elem("cmbCharQuestOpts", k)["ListIndex"]
            .as_i64()
            .unwrap_or(-1) as i32;
    }
    q.global_filter = f.ctl("chkGlobalFilter")["Value"].as_i64().unwrap_or(0) as i32;
    q.attack_type = case["inputs"]["globals"]["nGlobalAttackTypeMME"]
        .as_i64()
        .unwrap_or(0) as i32;
    q
}

fn run_calc_both_halves(world: &World, header: &Value, case: &Value) -> Result<(), String> {
    let input = gear_input(world, header, case)?;
    let got =
        calc_character_stats(&input, &second_input(header, case)).map_err(|e| format!("{e:?}"))?;
    let out = &case["outputs"];
    let want_err = handled_error(case, "CalcCharacterStats");
    if got.error != want_err {
        return Err(format!("error: got {:?}, want {want_err:?}", got.error));
    }
    for (k, w) in out["stat"].as_array().unwrap().iter().enumerate() {
        if got.caption(k) != w.as_str().unwrap() {
            return Err(format!("stat[{k}]: got {}, want {w}", got.caption(k)));
        }
    }
    for (k, w) in out["lbltags"].as_array().unwrap().iter().enumerate() {
        let g = got.stat_tag[k].map(vb::cstr).unwrap_or_default();
        if g != w.as_str().unwrap() {
            return Err(format!("lbltag[{k}]: got {g:?}, want {w}"));
        }
    }
    for (k, w) in out["tags"].as_array().unwrap().iter().enumerate() {
        let g = vb::cstr(got.stat_with_bonus[k]);
        if g != w.as_str().unwrap() {
            return Err(format!("tag[{k}]: got {g}, want {w}"));
        }
    }
    let ints: [(&str, i32); 7] = [
        ("AccyItems", got.accy_items),
        ("AccyAbils", got.accy_abils),
        ("AccyOther", got.accy_other),
        ("PlusDodge", got.plus_dodge),
        ("PlusMR", got.plus_mr),
        ("AttackAccyAdj", got.attack_accy_adj),
        ("QnDbonus", got.qnd_bonus),
    ];
    for (name, g) in ints {
        if i(&out[name]) != g {
            return Err(format!("{name}: got {g}, want {}", out[name]));
        }
    }
    // txtCharHPRegen / txtCharManaRegen are only assigned when the procedure got that far
    if want_err.is_none() {
        for (name, g) in [("hpregen", got.hp_regen), ("manaregen", got.mana_regen)] {
            if vb::val(out[name].as_str().unwrap()) != g {
                return Err(format!("{name}: got {g}, want {}", out[name]));
            }
        }
        let p = out["probe"].as_array().unwrap();
        let g = [got.encum_pct.to_string(), vb::cstr(got.walk_speed)];
        for k in 0..2 {
            if p[k].as_str().unwrap() != g[k] {
                return Err(format!("probe[{k}]: got {}, want {}", g[k], p[k]));
            }
        }
    }
    Ok(())
}

// Expected values come from fixtures/rules/gear2.json: CalcCharacterStats run whole by the oracle,
// cut at the call to RefreshMagic.
#[test]
fn gear2_cases_match_the_oracle() {
    let fx = fixture::load("gear2");
    let w = world(&fx.header["data"]);
    let mut fails = Vec::new();
    for case in &fx.cases {
        if let Err(e) = run_calc_both_halves(&w, &fx.header, case) {
            fails.push(format!("{}: {e}", case["id"].as_str().unwrap()));
        }
    }
    assert!(
        fails.is_empty(),
        "{} of {} cases differ; first ones:\n{}",
        fails.len(),
        fx.cases.len(),
        fails
            .iter()
            .take(15)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
