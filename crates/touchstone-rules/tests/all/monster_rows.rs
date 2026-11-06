// Fixture tests for the monster_rows family: see docs/porting.md step 5.
//
// Expected values: fixtures/rules/monster_rows.json, produced by the oracle running MME's own RefreshAll
// (the character), SetCurrentAttackTypeConfig, LoadLairInfo, PopulateCharacterProfile and AddMonster2LV
// into a recording ListView. The character is rebuilt with refresh_all from the pipeline scenario's
// controls, which each case copies; the World holds the monsters, lairs and spells the scenario chose
// (header "data"."table").
use super::fixture::{Fixture, load};
use super::pipeline::{Inputs, world as pipeline_world};
use serde_json::{Value, json};
use touchstone_model::{
    AbilitySlot, Lair, MidSpell, Monster, MonsterAttack, Spell, World, WorldParts,
};
use touchstone_rules::Profile;
use touchstone_rules::attack::CastText;
use touchstone_rules::character::{
    AttackTypeMud, CharacterProfile, PartyFilter, populate_character_profile,
    set_current_attack_type_config,
};
use touchstone_rules::exp_per_hour::{ExpPerHourKnobs, ExpPerHourSettings};
use touchstone_rules::lairs::{CharCallouts, CharLairEnv, LairCache, LairQuery};
use touchstone_rules::monster_sim::MonsterSimInput;
use touchstone_rules::pipeline::refresh_all;
use touchstone_rules::rows::Row;
use touchstone_rules::rows::monster::{
    MonsterRowArgs, MonsterRowInputs, add_monster_row, backstab_attack,
};
use touchstone_vb6::{Currency, Rnd};

pub(crate) fn i(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
}

pub(crate) fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

// Only damage numbers are used here; a weapon that casts would ask for its spell text.
pub(crate) struct NoCasts;

impl CastText for NoCasts {
    fn spell_name(&self, n: i32) -> String {
        panic!("cast text asked for spell {n}")
    }
    fn spell_eq(&self, n: i32, _: i16) -> String {
        panic!("cast text asked for spell {n}")
    }
}

pub(crate) fn pairs<const N: usize>(v: &Value) -> [AbilitySlot; N] {
    let mut out = [AbilitySlot::default(); N];
    for (k, p) in v.as_array().unwrap().iter().enumerate() {
        out[k] = AbilitySlot {
            ability: i(&p[0]),
            value: i(&p[1]),
        };
    }
    out
}

/// The pipeline world plus the scenario's monsters, lairs and spells.
fn build_world(pipe_data: &Value, table: &Value) -> World {
    let base = pipeline_world(pipe_data);
    let monsters = table["monsters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let attacks: [MonsterAttack; 5] = std::array::from_fn(|k| {
                let a = &r[17][k];
                MonsterAttack {
                    name: String::new(),
                    att_type: i(&a[0]),
                    acc: i(&a[1]),
                    pct: i(&a[2]),
                    true_pct: f(&a[3]),
                    min: 0,
                    max: 0,
                    energy: 0,
                    hit_spell: i(&a[4]),
                }
            });
            let mid_spells: [MidSpell; 5] = std::array::from_fn(|k| MidSpell {
                spell: i(&r[18][k][0]),
                pct: i(&r[18][k][1]),
                lvl: 0,
            });
            let name = r[1].as_str().unwrap();
            let summoned = r[15].as_str().unwrap();
            Monster {
                number: i(&r[0]),
                name: (!name.is_empty()).then(|| name.to_string()),
                weapon: 0,
                armour_class: i(&r[6]),
                damage_resist: i(&r[7]),
                follow_pct: 0,
                magic_res: i(&r[8]),
                bs_defense: i(&r[9]),
                exp: f(&r[3]),
                exp_multi: f(&r[4]),
                hp: i(&r[5]),
                energy: 0,
                avg_dmg: f(&r[13]),
                avg_dmg_phys: 0.0,
                avg_dmg_spell: 0.0,
                greet_txt: 0,
                hp_regen: i(&r[12]),
                charm_lvl: 0,
                r#type: 0,
                undead: i(&r[10]),
                align: 0,
                regen_time: i(&r[2]),
                game_limit: 0,
                avg_lair_exp: f(&r[11]),
                script_value: 0.0,
                r: 0,
                p: 0,
                g: 0,
                s: 0,
                c: 0,
                death_spell: i(&r[14]),
                create_spell: 0,
                attacks,
                mid_spells,
                drops: [Default::default(); 10],
                abilities: pairs(&r[16]),
                in_game: true,
                summoned_by: (!summoned.is_empty()).then(|| summoned.to_string()),
            }
        })
        .collect();
    let lairs = table["lairs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Lair {
            group_index: r[0].as_str().unwrap().to_string(),
            mob_list: r[1].as_str().unwrap().to_string(),
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
    // the pipeline's spells are full rows; the scenario's cover the ones monsters cast
    let mut spells: Vec<Spell> = base.spells.clone();
    for r in table["spells"].as_array().unwrap() {
        if spells.iter().any(|s| s.number == i(&r[0])) {
            continue;
        }
        let mut s = spells[0].clone();
        s.number = i(&r[0]);
        s.att_type = i(&r[1]);
        s.dur = i(&r[2]);
        s.dur_inc_lvls = i(&r[3]);
        s.dur_inc = i(&r[4]);
        s.abilities = pairs(&r[5]);
        spells.push(s);
    }
    World::new(WorldParts {
        info: base.info.clone(),
        items: base.items.clone(),
        monsters,
        spells,
        rooms: vec![],
        shops: vec![],
        classes: base.classes.clone(),
        races: base.races.clone(),
        lairs,
        tbinfo: vec![],
    })
    .unwrap()
}

pub(crate) fn currency_array(v: &Value) -> Vec<Currency> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| Currency::from_f64(f(x)).unwrap())
        .collect()
}

pub(crate) fn text_of(inp: &Inputs, name: &str, idx: usize) -> String {
    inp.elem(name, idx)["Text"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

pub(crate) fn party_of(inp: &Inputs) -> PartyFilter {
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
pub(crate) fn global<'a>(fx: &'a Fixture, case: &'a Value, name: &str) -> &'a Value {
    let v = &case["inputs"]["globals"][name];
    if v.is_null() {
        &fx.header["setup"]["globals"][name]
    } else {
        v
    }
}

pub(crate) fn settings(fx: &Fixture, case: &Value) -> ExpPerHourSettings {
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

/// The optional arguments of the `rows` step after the list and profile.
fn row_args(extra: &[Value]) -> MonsterRowArgs {
    let mut a = MonsterRowArgs::default();
    let cur = |v: &Value| match v {
        Value::Object(o) => {
            Currency::from_scaled(o["v"].as_f64().map_or(0, |x| (x * 10_000.0).round() as i64))
        }
        other => Currency::from_f64(f(other)).unwrap(),
    };
    if let Some(v) = extra.first() {
        a.damage_out = i(v);
    }
    if let Some(v) = extra.get(1) {
        a.pass_exp = cur(v);
    }
    if let Some(v) = extra.get(2) {
        a.pass_recovery = f(v);
    }
    if let Some(v) = extra.get(3) {
        a.surprise_damage_out = i(v);
    }
    if let Some(v) = extra.get(4) {
        a.surprise_min_dmg = i(v);
    }
    if let Some(v) = extra.get(5) {
        a.surprise_chance = i(v) as i16;
    }
    if let Some(v) = extra.get(6) {
        a.first_round_dmg = i(v);
    }
    if let Some(v) = extra.get(7) {
        a.min_round_dmg = i(v);
    }
    if let Some(v) = extra.get(8) {
        a.does_not_match_filter = v.as_bool().unwrap_or(false);
    }
    a
}

/// A row in the shape the oracle's recording ListView dumps it.
pub(crate) fn row_json(r: &Row) -> Value {
    let cells: Vec<Value> = r
        .cells
        .iter()
        .map(|c| {
            if c.tag.is_none() && c.fore_color.is_none() {
                json!(c.text)
            } else {
                json!([c.text, c.tag, c.fore_color])
            }
        })
        .collect();
    let mut out = vec![json!(r.text), json!(r.tag), json!(cells)];
    if let Some(c) = r.fore_color {
        out.push(json!(c));
    }
    Value::Array(out)
}

fn run_case(world: &World, fx: &Fixture, case: &Value) -> Vec<String> {
    let caches = &caches(fx, case);
    let id = case["id"].as_str().unwrap();
    assert!(case.get("error").is_none(), "{id}: the oracle raised");
    let events = case.get("events").is_some();
    let allow_events = id.contains("-events");
    assert!(
        !events || allow_events,
        "{id}: MME's handler fired: {}",
        case["events"]
    );
    let nmr = global(fx, case, "nNMRVer").as_f64().unwrap();
    let inp = Inputs::new(&fx.header, case);
    let party = party_of(&inp);
    let mut opts = inp.options();
    opts.party = party.clone();
    opts.nmr_ver = nmr;
    let state = refresh_all(Profile::Stock, world, &inp.character(), &opts)
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
    let mut cache = LairCache::load(world, Profile::Stock, nmr);
    let mut sim = MonsterSimInput::default();
    let mut rnd = Rnd::from_state(327_680);
    let mut env = CharLairEnv {
        cache: &mut cache,
        query: LairQuery {
            profile: Profile::Stock,
            startup: false,
            use_character: state.character.global_filter,
            party: &party,
            global_attack_config: &key.config,
        },
        callouts: CharCallouts {
            world,
            damage: cenv.damage_env(world, &ctx, &state.character, &NoCasts),
            vs_party: &caches.party,
            vs_char: &caches.chr,
            vs_default: &caches.default,
            sim_rounds: 1000,
            sim: &mut sim,
            rnd: &mut rnd,
        },
    };
    let settings = settings(fx, case);
    let inputs = MonsterRowInputs {
        use_exp_multi: global(fx, case, "UseExpMulti").as_bool().unwrap_or(false),
        is_main_list: true,
        settings: &settings,
        possy: &caches.possy,
        spawn_chance: &caches.chance,
        backstab_attack: backstab_attack(state.attack_config()),
    };
    let mut bad = Vec::new();
    for st in case["inputs"]["steps"].as_array().unwrap() {
        let Some(spec) = st.get("rows") else {
            continue;
        };
        let name = st["as"].as_str().unwrap();
        let want_rows = case["outputs"][name].as_array().unwrap();
        let keys = spec["keys"].as_array().unwrap();
        let args = row_args(&st["args"].as_array().unwrap()[2..]);
        let inputs = MonsterRowInputs {
            is_main_list: spec.get("hwnd").is_none(),
            settings: inputs.settings,
            possy: inputs.possy,
            spawn_chance: inputs.spawn_chance,
            ..inputs
        };
        for (k, key) in keys.iter().enumerate() {
            let got = add_monster_row(&mut env, &inputs, &t_char, i(key), &args);
            let got = got.as_ref().map_or(Value::Null, row_json);
            if got != want_rows[k] {
                bad.push(format!(
                    "{id} {name}[{k}] monster {key}:\n  got  {got}\n  want {}",
                    want_rows[k]
                ));
            }
        }
    }
    bad
}

pub(crate) struct Caches {
    pub(crate) party: Vec<Currency>,
    pub(crate) chr: Vec<Currency>,
    pub(crate) default: Vec<Currency>,
    pub(crate) possy: Vec<Currency>,
    pub(crate) chance: Vec<Currency>,
}

/// The damage and spawn arrays: the case's own where it sets them, else the family-wide ones.
pub(crate) fn caches(fx: &Fixture, case: &Value) -> Caches {
    let g = |n: &str| currency_array(global(fx, case, n));
    Caches {
        party: g("nMonsterDamageVsParty"),
        chr: g("nMonsterDamageVsChar"),
        default: g("nMonsterDamageVsDefault"),
        possy: g("nMonsterPossy"),
        chance: g("nMonsterSpawnChance"),
    }
}

// Expected values: fixtures/rules/monster_rows.json (MME's AddMonster2LV, run by the oracle).
#[test]
fn monster_rows_fixture() {
    let fx = load("monster_rows");
    let pipe = load("pipeline");
    let world = build_world(&pipe.header["data"], &fx.header["data"]["table"]);
    let mut bad = Vec::new();
    let mut rows = 0;
    for case in &fx.cases {
        rows += case["outputs"]
            .as_object()
            .unwrap()
            .values()
            .map(|v| v.as_array().map_or(0, Vec::len))
            .sum::<usize>();
        bad.extend(run_case(&world, &fx, case));
    }
    eprintln!(
        "monster_rows: {} cases, {rows} rows, {} mismatches",
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
