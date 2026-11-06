// Fixture tests for the monster_detail family: see docs/porting.md step 5.
//
// Expected values: fixtures/rules/monster_detail.json, produced by the oracle running MME's own RefreshAll
// (the character), SetCurrentAttackTypeConfig, LoadLairInfo and PullMonsterDetail (with AddMonsterDamageOutText,
// GetLocations and the rest of its callees) into a recording ListView. AddMonsterReachRows is stubbed there
// and here (the reachability module is optional). The character is rebuilt with refresh_all from the pipeline
// scenario's controls; the World holds the monsters, spells, items, rooms and lairs the scenario chose
// (header "data"."table", rows in the Access table's column order).
use super::fixture::{Fixture, load};
use super::pipeline::{Inputs, world as pipeline_world};
use serde_json::{Value, json};
use std::collections::HashMap;
use touchstone_model::{
    AbilitySlot, Class, DropSlot, Item, Lair, MidSpell, Monster, MonsterAttack, Room, Spell,
    TbInfo, World, WorldParts,
};
use touchstone_rules::Profile;
use touchstone_rules::ability_text::AbilityEnv;
use touchstone_rules::character::{
    AttackTypeMud, CharacterProfile, PartyFilter, populate_character_profile,
    set_char_defense_description, set_current_attack_type_config,
};
use touchstone_rules::detail::monster::{
    DamageCaches, DetailCell, DetailRow, MonsterDetailInputs, NoReach, pull_monster_detail,
};
use touchstone_rules::exp_per_hour::{ExpPerHourKnobs, ExpPerHourSettings};
use touchstone_rules::lairs::{CharCallouts, CharLairEnv, LairCache, LairQuery};
use touchstone_rules::monster_sim::MonsterSimInput;
use touchstone_rules::pipeline::refresh_all;
use touchstone_vb6::{Currency, Rnd};

fn i(v: &Value) -> i32 {
    match v {
        Value::Bool(b) => i32::from(*b),
        Value::Null => 0,
        _ => v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32,
    }
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

/// Column lookup for a row in the table's own column order.
struct Cols(HashMap<String, usize>);

impl Cols {
    fn new(names: &Value) -> Self {
        Self(
            names
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .map(|(k, n)| (s(n), k))
                .collect(),
        )
    }
    fn at<'a>(&self, r: &'a Value, name: &str) -> &'a Value {
        &r[*self
            .0
            .get(name)
            .unwrap_or_else(|| panic!("no column {name}"))]
    }
    fn i(&self, r: &Value, name: &str) -> i32 {
        i(self.at(r, name))
    }
    fn f(&self, r: &Value, name: &str) -> f64 {
        f(self.at(r, name))
    }
    fn s(&self, r: &Value, name: &str) -> String {
        s(self.at(r, name))
    }
    fn slots<const N: usize>(&self, r: &Value) -> [AbilitySlot; N] {
        std::array::from_fn(|k| AbilitySlot {
            ability: self.i(r, &format!("Abil-{k}")),
            value: self.i(r, &format!("AbilVal-{k}")),
        })
    }
}

fn monster(c: &Cols, r: &Value) -> Monster {
    let name = c.s(r, "Name");
    let sb = c.s(r, "Summoned By");
    Monster {
        number: c.i(r, "Number"),
        name: (!name.is_empty()).then_some(name),
        weapon: c.i(r, "Weapon"),
        armour_class: c.i(r, "ArmourClass"),
        damage_resist: c.i(r, "DamageResist"),
        follow_pct: c.i(r, "Follow%"),
        magic_res: c.i(r, "MagicRes"),
        bs_defense: c.i(r, "BSDefense"),
        exp: c.f(r, "EXP"),
        exp_multi: c.f(r, "ExpMulti"),
        hp: c.i(r, "HP"),
        energy: c.i(r, "Energy"),
        avg_dmg: c.f(r, "AvgDmg"),
        avg_dmg_phys: c.f(r, "AvgDmgPhys"),
        avg_dmg_spell: c.f(r, "AvgDmgSpell"),
        greet_txt: c.i(r, "GreetTXT"),
        hp_regen: c.i(r, "HPRegen"),
        charm_lvl: c.i(r, "CharmLVL"),
        r#type: c.i(r, "Type"),
        undead: c.i(r, "Undead"),
        align: c.i(r, "Align"),
        regen_time: c.i(r, "RegenTime"),
        game_limit: c.i(r, "GameLimit"),
        avg_lair_exp: c.f(r, "AvgLairExp"),
        script_value: c.f(r, "ScriptValue"),
        r: c.i(r, "R"),
        p: c.i(r, "P"),
        g: c.i(r, "G"),
        s: c.i(r, "S"),
        c: c.i(r, "C"),
        death_spell: c.i(r, "DeathSpell"),
        create_spell: c.i(r, "CreateSpell"),
        attacks: std::array::from_fn(|k| MonsterAttack {
            name: c.s(r, &format!("AttName-{k}")),
            att_type: c.i(r, &format!("AttType-{k}")),
            acc: c.i(r, &format!("AttAcc-{k}")),
            pct: c.i(r, &format!("Att%-{k}")),
            true_pct: c.f(r, &format!("AttTrue%-{k}")),
            min: c.i(r, &format!("AttMin-{k}")),
            max: c.i(r, &format!("AttMax-{k}")),
            energy: c.i(r, &format!("AttEnergy-{k}")),
            hit_spell: c.i(r, &format!("AttHitSpell-{k}")),
        }),
        mid_spells: std::array::from_fn(|k| MidSpell {
            spell: c.i(r, &format!("MidSpell-{k}")),
            pct: c.i(r, &format!("MidSpell%-{k}")),
            lvl: c.i(r, &format!("MidSpellLVL-{k}")),
        }),
        drops: std::array::from_fn(|k| DropSlot {
            item: c.i(r, &format!("DropItem-{k}")),
            pct: c.i(r, &format!("DropItem%-{k}")),
        }),
        abilities: c.slots(r),
        in_game: true,
        summoned_by: (!sb.is_empty()).then_some(sb),
    }
}

fn spell(c: &Cols, r: &Value) -> Spell {
    let name = c.s(r, "Name");
    Spell {
        number: c.i(r, "Number"),
        name: (!name.is_empty()).then_some(name),
        short: Some(c.s(r, "Short")),
        req_level: c.i(r, "ReqLevel"),
        energy_cost: c.i(r, "EnergyCost"),
        mana_cost: c.i(r, "ManaCost"),
        min_base: c.i(r, "MinBase"),
        max_base: c.i(r, "MaxBase"),
        diff: c.i(r, "Diff"),
        type_of_resists: c.i(r, "TypeOfResists"),
        targets: c.i(r, "Targets"),
        dur: c.i(r, "Dur"),
        att_type: c.i(r, "AttType"),
        magery: c.i(r, "Magery"),
        magery_lvl: c.i(r, "MageryLVL"),
        cap: c.i(r, "Cap"),
        max_inc_lvls: c.i(r, "MaxIncLVLs"),
        max_inc: c.i(r, "MaxInc"),
        min_inc_lvls: c.i(r, "MinIncLVLs"),
        min_inc: c.i(r, "MinInc"),
        dur_inc_lvls: c.i(r, "DurIncLVLs"),
        dur_inc: c.i(r, "DurInc"),
        abilities: c.slots(r),
        learnable: c.i(r, "Learnable") != 0,
        learned_from: None,
        casted_by: None,
        classes: None,
    }
}

fn item(c: &Cols, r: &Value) -> Item {
    Item {
        number: c.i(r, "Number"),
        name: c.s(r, "Name"),
        limit: c.i(r, "Limit"),
        encum: c.i(r, "Encum"),
        item_type: c.i(r, "ItemType"),
        use_count: c.i(r, "UseCount"),
        price: c.i(r, "Price"),
        currency: c.i(r, "Currency"),
        min: c.i(r, "Min"),
        max: c.i(r, "Max"),
        armour_class: c.i(r, "ArmourClass"),
        damage_resist: c.i(r, "DamageResist"),
        weapon_type: c.i(r, "WeaponType"),
        armour_type: c.i(r, "ArmourType"),
        worn: c.i(r, "Worn"),
        accy: c.i(r, "Accy"),
        gettable: c.i(r, "Gettable") != 0,
        str_req: c.i(r, "StrReq"),
        speed: c.i(r, "Speed"),
        not_droppable: c.i(r, "Not Droppable") != 0,
        destroy_on_death: c.i(r, "Destroy On Death") != 0,
        retain_after_uses: c.i(r, "Retain After Uses") != 0,
        class_rest: std::array::from_fn(|k| c.i(r, &format!("ClassRest-{k}"))),
        race_rest: std::array::from_fn(|k| c.i(r, &format!("RaceRest-{k}"))),
        negate_spell: std::array::from_fn(|k| c.i(r, &format!("NegateSpell-{k}"))),
        abilities: c.slots(r),
        in_game: c.i(r, "In Game") != 0,
        obtained_from: Some(c.s(r, "Obtained From")),
        references: None,
    }
}

/// The pipeline world plus the scenario's rows (a scenario row replaces the pipeline's of the same number).
fn build_world(pipe_data: &Value, table: &Value) -> World {
    let base = pipeline_world(pipe_data);
    let mc = Cols::new(&table["monster_cols"]);
    let sc = Cols::new(&table["spell_cols"]);
    let ic = Cols::new(&table["item_cols"]);
    let monsters: Vec<Monster> = table["monsters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| monster(&mc, r))
        .collect();
    let mut spells = base.spells.clone();
    for r in table["spells"].as_array().unwrap() {
        let sp = spell(&sc, r);
        match spells.iter_mut().find(|x| x.number == sp.number) {
            Some(slot) => *slot = sp,
            None => spells.push(sp),
        }
    }
    let mut items = base.items.clone();
    for r in table["items"].as_array().unwrap() {
        let it = item(&ic, r);
        match items.iter_mut().find(|x| x.number == it.number) {
            Some(slot) => *slot = it,
            None => items.push(it),
        }
    }
    let mut classes: Vec<Class> = base.classes.clone();
    for r in table["classes"].as_array().unwrap() {
        if let Some(c) = classes.iter_mut().find(|c| c.number == i(&r[0])) {
            c.name = s(&r[1]);
        }
    }
    let rooms = table["rooms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Room {
            map_number: i(&r[0]),
            room_number: i(&r[1]),
            name: Some(s(&r[2])),
            light: 0,
            shop: 0,
            npc: 0,
            cmd: 0,
            spell: 0,
            lair: None,
            delay: i(&r[3]),
            placed: None,
            n: String::new(),
            s: String::new(),
            e: String::new(),
            w: String::new(),
            ne: String::new(),
            nw: String::new(),
            se: String::new(),
            sw: String::new(),
            u: String::new(),
            d: String::new(),
        })
        .collect();
    let tbinfo = table["tbinfo"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| TbInfo {
            number: i(&r[0]),
            link_to: i(&r[1]),
            action: Some(s(&r[2])),
            called_from: s(&r[3]),
        })
        .collect();
    let lairs = table["lairs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Lair {
            group_index: s(&r[0]),
            mob_list: s(&r[1]),
            mobs: i(&r[2]),
            total_lairs: i(&r[3]),
            avg_delay: i(&r[4]),
            avg_walk: f(&r[5]),
            avg_exp: f(&r[6]),
            avg_dmg: f(&r[7]),
            avg_dmg_phys: 0.0,
            avg_dmg_spell: 0.0,
            avg_hp: f(&r[8]),
            avg_ac: i(&r[9]),
            avg_dr: i(&r[10]),
            avg_mr: i(&r[11]),
            avg_dodge: i(&r[12]),
        })
        .collect();
    World::new(WorldParts {
        info: base.info.clone(),
        items,
        monsters,
        spells,
        rooms,
        shops: vec![],
        classes,
        races: base.races.clone(),
        lairs,
        tbinfo,
    })
    .unwrap()
}

fn currency_array(v: &Value) -> Vec<Currency> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| Currency::from_f64(f(x)).unwrap())
        .collect()
}

fn text_of(inp: &Inputs, name: &str, idx: usize) -> String {
    inp.elem(name, idx)["Text"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

fn party_of(inp: &Inputs) -> PartyFilter {
    let mut p = PartyFilter {
        lair_mode: inp.elem("optMonsterFilter", 1)["Value"].as_bool() == Some(true),
        monster_damage_text: inp.ctl("txtMonsterDamage")["Text"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        ..PartyFilter::default()
    };
    for (k, t) in p.lair_text.iter_mut().enumerate() {
        *t = text_of(inp, "txtMonsterLairFilter", k);
    }
    p
}

/// A global: the case's value, else the family-wide setup's.
fn global<'a>(fx: &'a Fixture, case: &'a Value, name: &str) -> &'a Value {
    let v = &case["inputs"]["globals"][name];
    if v.is_null() {
        &fx.header["setup"]["globals"][name]
    } else {
        v
    }
}

fn settings(fx: &Fixture, case: &Value) -> ExpPerHourSettings {
    let b = |n: &str| global(fx, case, n).as_bool().unwrap_or(false);
    let k = |n: &str| global(fx, case, n).as_f64().unwrap_or(0.0);
    ExpPerHourSettings {
        model_a: b("bGlobal_cephModelA"),
        model_b: b("bGlobal_cephModelB"),
        model_c: b("bGlobal_cephModelC"),
        model_d: b("bGlobal_cephModelD"),
        recovery_only: b("bGlobal_cephRecoveryOnly"),
        show_all: b("bGlobal_cephShowAll"),
        knobs: ExpPerHourKnobs {
            xp: k("nGlobal_cephXP_Knob"),
            dmg: k("nGlobal_cephDMG_Knob"),
            mana: k("nGlobal_cephMana_Knob"),
            mv: k("nGlobal_cephMove_Knob"),
        },
    }
}

fn cell_json(c: &DetailCell) -> Value {
    if c.tag.is_none() && c.fore_color.is_none() && !c.bold {
        json!(c.text)
    } else {
        json!([c.text, c.tag, c.fore_color, c.bold])
    }
}

/// A row in the shape the oracle's recording ListView dumps it for the detail panes.
fn row_json(r: &DetailRow) -> Value {
    if r.tag.is_none() && r.fore_color.is_none() && !r.bold && r.cells.is_empty() {
        return json!(r.label);
    }
    let cells: Vec<Value> = r.cells.iter().map(cell_json).collect();
    json!([r.label, r.tag, r.fore_color, r.bold, cells])
}

fn run_case(prof: Profile, world: &World, fx: &Fixture, case: &Value) -> Vec<String> {
    let id = case["id"].as_str().unwrap();
    assert!(case.get("error").is_none(), "{id}: the oracle raised");
    assert!(
        case.get("events").is_none(),
        "{id}: MME's handler fired: {}",
        case["events"]
    );
    let garr = |n: &str| currency_array(global(fx, case, n));
    let (vs_party, vs_char, vs_default) = (
        garr("nMonsterDamageVsParty"),
        garr("nMonsterDamageVsChar"),
        garr("nMonsterDamageVsDefault"),
    );
    let (possy, chance) = (garr("nMonsterPossy"), garr("nMonsterSpawnChance"));
    let nmr = global(fx, case, "nNMRVer").as_f64().unwrap();
    let inp = Inputs::new(&fx.header, case);
    let party = party_of(&inp);
    let mut opts = inp.options();
    opts.party = party.clone();
    opts.nmr_ver = nmr;
    let state = refresh_all(prof, world, &inp.character(), &opts)
        .unwrap_or_else(|e| panic!("{id}: refresh_all: {e}"));
    let cenv = state.env(
        [
            text_of(&inp, "txtMonsterDamageOUT", 0),
            text_of(&inp, "txtMonsterDamageOUT", 1),
        ],
        text_of(&inp, "txtMonsterLairFilter", 9),
    );
    let ctx = cenv.context(world);
    let key = set_current_attack_type_config(&ctx, &state.character, "");
    let mut t_char = CharacterProfile::default();
    populate_character_profile(
        &ctx,
        &state.character,
        &mut t_char,
        false,
        false,
        AttackTypeMud::None,
        0,
        false,
    );
    let hide = global(fx, case, "bHideRecordNumbers")
        .as_bool()
        .unwrap_or(false);
    let level_text = text_of(&inp, "txtGlobalLevel", 0);
    let mut ab = AbilityEnv::for_world(world, prof);
    ab.hide_record_numbers = hide;
    if state.character.global_filter {
        ab.global_level_text = Some(&level_text);
    }
    let mut cache = LairCache::load(world, prof, nmr);
    let mut sim = MonsterSimInput::default();
    let mut rnd = Rnd::from_state(327_680);
    let mut env = CharLairEnv {
        cache: &mut cache,
        query: LairQuery {
            profile: prof,
            startup: false,
            use_character: state.character.global_filter,
            party: &party,
            global_attack_config: &key.config,
        },
        callouts: CharCallouts {
            world,
            damage: cenv.damage_env(world, &ctx, &state.character, &ab),
            vs_party: &vs_party,
            vs_char: &vs_char,
            vs_default: &vs_default,
            sim_rounds: global(fx, case, "nGlobalMonsterSimRounds")
                .as_i64()
                .unwrap() as i32,
            sim: &mut sim,
            rnd: &mut rnd,
        },
    };
    let settings = settings(fx, case);
    let sim_chr = state.sim_inputs();
    let charm = touchstone_vb6::cstr(state.stats.stat_with_bonus[5]);
    let defense_cfg = global(fx, case, "sMonsterDamageVsCharDefenseConfig")
        .as_str()
        .unwrap()
        .to_string();
    // RefreshAll runs SetCharDefenseDescription, which overwrites the global with its key
    let start_desc = global(fx, case, "sGlobalCharDefenseDescription")
        .as_str()
        .unwrap()
        .to_string();
    let defense_desc = set_char_defense_description(&ctx, &state.character, &start_desc)
        .map_or(start_desc, |d| d.description);
    let flag = |name: &str| inp.ctl(name).as_bool().unwrap_or(false);
    let inputs = MonsterDetailInputs {
        use_exp_multi: global(fx, case, "UseExpMulti").as_bool().unwrap_or(false),
        auto_calc_mon_damage: flag("bAutoCalcMonDamage"),
        dont_lookup_mon_regen: flag("bDontLookupMonRegen"),
        print_char_damage_out_first: global(fx, case, "bMobPrintCharDamageOutFirst")
            .as_bool()
            .unwrap_or(false),
        hide_record_numbers: hide,
        sim_rounds: global(fx, case, "nGlobalMonsterSimRounds")
            .as_i64()
            .unwrap() as i32,
        settings: &settings,
        possy: &possy,
        spawn_chance: &chance,
        vs_char_defense_config: &defense_cfg,
        char_defense_description: &defense_desc,
        party_damage_calculated: global(fx, case, "bMonsterDamageVsPartyCalculated")
            .as_bool()
            .unwrap_or(false),
        dont_prompt_calc_party: global(fx, case, "bDontPromptCalcPartyMonsterDamage")
            .as_bool()
            .unwrap_or(false),
        bless_caption: state.bless_mana.unwrap_or(0.0),
        charm_tag: &charm,
        sim_chr: &sim_chr,
        reach: &NoReach,
    };
    // the damage arrays are globals: what one pane stores, the next reads
    let mut caches = DamageCaches {
        vs_party: vs_party.clone(),
        vs_char: vs_char.clone(),
    };
    let mut bad = Vec::new();
    for st in case["inputs"]["steps"].as_array().unwrap() {
        let Some(spec) = st.get("rows") else {
            continue;
        };
        let name = st["as"].as_str().unwrap();
        let want_rows = case["outputs"][name].as_array().unwrap();
        let keys = spec["keys"].as_array().unwrap();
        let limit = st["args"]
            .as_array()
            .unwrap()
            .get(2)
            .map_or(100, |v| i(v) as i16);
        for (k, key) in keys.iter().enumerate() {
            let got = pull_monster_detail(&mut env, &ab, &inputs, &mut caches, i(key), limit);
            let got: Vec<Value> = got.iter().map(row_json).collect();
            let want = want_rows[k].as_array().unwrap();
            if got != *want {
                let at = got
                    .iter()
                    .zip(want)
                    .position(|(g, w)| g != w)
                    .unwrap_or(got.len().min(want.len()));
                bad.push(format!(
                    "{id} {name}[{k}] monster {key}: {} rows (want {}), first difference at row {at}:\n  got  {}\n  want {}",
                    got.len(),
                    want.len(),
                    got.get(at).map_or("-".into(), ToString::to_string),
                    want.get(at).map_or("-".into(), ToString::to_string)
                ));
            }
        }
    }
    bad
}

// Expected values: fixtures/rules/monster_detail.json (MME's PullMonsterDetail, run by the oracle).
#[test]
fn monster_detail_fixture() {
    check_fixture(Profile::Stock);
}

/// Runs every monster-detail case under `prof`; the layer-1 identity harness reuses it.
pub(crate) fn check_fixture(prof: Profile) {
    let fx = load("monster_detail");
    let pipe = load("pipeline");
    let world = build_world(&pipe.header["data"], &fx.header["data"]["table"]);
    let mut bad = Vec::new();
    let mut panes = 0;
    for case in &fx.cases {
        panes += case["outputs"]
            .as_object()
            .unwrap()
            .values()
            .map(|v| v.as_array().map_or(0, Vec::len))
            .sum::<usize>();
        bad.extend(run_case(prof, &world, &fx, case));
    }
    eprintln!(
        "monster_detail: {} cases, {panes} panes, {} mismatches",
        fx.cases.len(),
        bad.len()
    );
    assert!(
        bad.is_empty(),
        "{} mismatches, first:\n{}",
        bad.len(),
        bad.iter().take(6).cloned().collect::<Vec<_>>().join("\n")
    );
}
