// Fixture tests for the lair_info family: see docs/porting.md step 5.
//
// Expected values: fixtures/rules/lair_info.json, produced by the oracle running MME's own
// RefreshAll (the character), SetCurrentAttackTypeConfig, LoadLairInfo and GetLairInfo with the real
// GetDamageOutput, CalcCombatRounds and GetPreCalculatedMonsterDamage. The character is rebuilt with
// refresh_all from the pipeline scenario's controls, which each case copies. The World holds the
// lairs the scenario chose and the monsters in their mob lists (header "data"."table"); the oracle
// loads every lair, which cannot change a chosen one.
use super::fixture::{Fixture, load};
use super::lairs::{fields, same};
use super::pipeline::{Inputs, world as pipeline_world};
use serde_json::Value;
use touchstone_model::{AbilitySlot, Lair, Monster, MonsterAttack, World, WorldParts};
use touchstone_rules::Profile;
use touchstone_rules::attack::CastText;
use touchstone_rules::character::{Context, PartyFilter, set_current_attack_type_config};
use touchstone_rules::combat::DamageEnv;
use touchstone_rules::lairs::{
    CharCallouts, CharLairEnv, LairCache, LairEnv, LairInfo, LairLocsInput, LairQuery,
    calculate_monster_party_damage, get_lair_averages_from_locs,
};
use touchstone_rules::monster_glue::GlueEnv;
use touchstone_rules::monster_sim::MonsterSimInput;
use touchstone_rules::pipeline::{CharacterState, refresh_all};
use touchstone_rules::spells::SpellText;
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Rnd};

fn i(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
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

// Only the damage numbers are used here; a weapon that casts would ask for its spell text.
struct NoCasts;

impl CastText for NoCasts {
    fn spell_name(&self, n: i32) -> String {
        panic!("cast text asked for spell {n}")
    }
    fn spell_eq(&self, n: i32, _: i16) -> String {
        panic!("cast text asked for spell {n}")
    }
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

/// The pipeline world (items, spells, classes, races) plus the scenario's lairs and monsters.
fn build_world(pipe_data: &Value, table: &Value) -> World {
    let base = pipeline_world(pipe_data);
    let monsters = table["monsters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let attacks: [MonsterAttack; 5] = std::array::from_fn(|k| {
                let a = &r[5][k];
                MonsterAttack {
                    name: String::new(),
                    att_type: i(&a[0]),
                    acc: i(&a[1]),
                    pct: i(&a[2]),
                    true_pct: a[3].as_f64().unwrap(),
                    min: 0,
                    max: 0,
                    energy: 0,
                    hit_spell: 0,
                }
            });
            Monster {
                number: i(&r[0]),
                name: None,
                weapon: 0,
                armour_class: 0,
                damage_resist: 0,
                follow_pct: 0,
                magic_res: 0,
                bs_defense: i(&r[2]),
                exp: 0.0,
                exp_multi: 0.0,
                hp: 0,
                energy: 0,
                avg_dmg: r[3].as_f64().unwrap(),
                avg_dmg_phys: 0.0,
                avg_dmg_spell: 0.0,
                greet_txt: 0,
                hp_regen: 0,
                charm_lvl: 0,
                r#type: 0,
                undead: i(&r[1]),
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
                attacks,
                mid_spells: [Default::default(); 5],
                drops: [Default::default(); 10],
                abilities: pairs(&r[4]),
                in_game: true,
                summoned_by: None,
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
            avg_walk: r[5].as_f64().unwrap(),
            avg_exp: r[6].as_f64().unwrap(),
            avg_dmg: r[7].as_f64().unwrap(),
            avg_dmg_phys: 0.0,
            avg_dmg_spell: 0.0,
            avg_hp: r[8].as_f64().unwrap(),
            avg_ac: i(&r[9]),
            avg_dr: i(&r[10]),
            avg_mr: i(&r[11]),
            avg_dodge: i(&r[12]),
        })
        .collect();
    World::new(WorldParts {
        info: base.info.clone(),
        items: base.items.clone(),
        monsters,
        spells: base.spells.clone(),
        rooms: vec![],
        shops: vec![],
        classes: base.classes.clone(),
        races: base.races.clone(),
        lairs,
        tbinfo: vec![],
    })
    .unwrap()
}

fn currency_array(v: &Value) -> Vec<Currency> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| Currency::from_f64(x.as_f64().unwrap()).unwrap())
        .collect()
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
        *t = inp.elem("txtMonsterLairFilter", k)["Text"]
            .as_str()
            .unwrap_or("")
            .to_string();
    }
    p
}

fn text_of(inp: &Inputs, name: &str, idx: usize) -> String {
    inp.elem(name, idx)["Text"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

/// Everything one case needs, built from the oracle case's controls.
struct Setup {
    state: CharacterState,
    party: PartyFilter,
    dat_ver: String,
    dout: [String; 2],
    swings: String,
}

fn setup(world: &World, fx: &Fixture, case: &Value) -> Setup {
    let inp = Inputs::new(&fx.header, case);
    let party = party_of(&inp);
    let mut opts = inp.options();
    opts.party = party.clone();
    let state = refresh_all(Profile::Stock, world, &inp.character(), &opts)
        .unwrap_or_else(|e| panic!("refresh_all: {e}"));
    Setup {
        dat_ver: opts.dat_ver_caption.clone(),
        dout: [
            text_of(&inp, "txtMonsterDamageOUT", 0),
            text_of(&inp, "txtMonsterDamageOUT", 1),
        ],
        swings: text_of(&inp, "txtMonsterLairFilter", 9),
        party,
        state,
    }
}

/// The pieces a `DamageEnv` borrows, so a test can hold them in locals.
struct Pieces {
    stats: touchstone_rules::character::CharStats,
    equipped: touchstone_rules::attack::EquippedGlobals,
    accuracy_tag: String,
}

fn pieces(st: &CharacterState) -> Pieces {
    Pieces {
        stats: st.profile_inputs(),
        equipped: st.equipped_globals(),
        accuracy_tag: st.stats.stat_tag[10].map_or(String::new(), vb::cstr),
    }
}

fn context<'a>(world: &'a World, p: &'a Pieces, s: &'a Setup) -> Context<'a> {
    Context {
        profile: Profile::Stock,
        nmr_ver: 1.83,
        avg_level_max_all_stats: 0.0,
        hide_record_numbers: false,
        recovery_only: false,
        classes: &world.classes,
        items: &world.items,
        spells: &world.spells,
        text: &NoText,
        stats: &p.stats,
        party: &s.party,
    }
}

#[allow(clippy::too_many_arguments)]
fn damage_env<'a>(
    world: &'a World,
    ctx: &'a Context<'a>,
    p: &'a Pieces,
    s: &'a Setup,
) -> DamageEnv<'a> {
    DamageEnv {
        ctx,
        character: &s.state.character,
        races: &world.races,
        monsters: &world.monsters,
        equipped: &p.equipped,
        cast_text: &NoCasts,
        dat_ver_caption: &s.dat_ver,
        accuracy_tag: &p.accuracy_tag,
        damage_out_text: [&s.dout[0], &s.dout[1]],
        party_swings_text: &s.swings,
    }
}

fn group_args(st: &Value) -> (String, i16) {
    let a = st["args"].as_array().unwrap();
    (
        a[0].as_str().unwrap().to_string(),
        a.get(1).map_or(0, |v| i(v) as i16),
    )
}

fn run_case(world: &World, fx: &Fixture, caches: &Caches, case: &Value) -> Vec<String> {
    let id = case["id"].as_str().unwrap();
    assert!(case.get("error").is_none(), "{id}: the oracle raised");
    assert!(
        case.get("events").is_none(),
        "{id}: MME's handler fired: {}",
        case["events"]
    );
    let s = setup(world, fx, case);
    let p = pieces(&s.state);
    let ctx = context(world, &p, &s);
    let startup = case["inputs"]["globals"]["bStartup"] == true;
    let key = set_current_attack_type_config(&ctx, &s.state.character, "");
    let want_cfg = case["outputs"]["config"].as_str().unwrap();
    let mut bad = Vec::new();
    if key.config != want_cfg {
        bad.push(format!("{id}: config got {}, want {want_cfg}", key.config));
    }
    let mut cache = LairCache::load(world, Profile::Stock, 1.83);
    let mut sim = MonsterSimInput::default();
    let mut rnd = Rnd::from_state(327_680);
    let mut env = CharLairEnv {
        cache: &mut cache,
        query: LairQuery {
            profile: Profile::Stock,
            startup,
            use_character: s.state.character.global_filter,
            party: &s.party,
            global_attack_config: &key.config,
        },
        callouts: CharCallouts {
            world,
            damage: damage_env(world, &ctx, &p, &s),
            vs_party: &caches.party,
            vs_char: &caches.chr,
            vs_default: &caches.default,
            sim_rounds: 1000,
            sim: &mut sim,
            rnd: &mut rnd,
        },
    };
    for st in case["inputs"]["steps"].as_array().unwrap() {
        if st["call"] != "GetLairInfo" {
            continue;
        }
        let name = st["as"].as_str().unwrap();
        let (group, regen) = group_args(st);
        let got = env.get_lair_info(&group, regen);
        let want = &case["outputs"][name];
        for (field, v) in fields(&got) {
            if !same(&v, &want[field]) {
                bad.push(format!(
                    "{id} {name} GetLairInfo({group:?}, {regen}): {field} got {v:?}, want {}",
                    want[field]
                ));
                break;
            }
        }
    }
    bad
}

struct Caches {
    party: Vec<Currency>,
    chr: Vec<Currency>,
    default: Vec<Currency>,
}

fn caches(fx: &Fixture) -> Caches {
    let g = &fx.header["setup"]["globals"];
    Caches {
        party: currency_array(&g["nMonsterDamageVsParty"]),
        chr: currency_array(&g["nMonsterDamageVsChar"]),
        default: currency_array(&g["nMonsterDamageVsDefault"]),
    }
}

fn pipeline_data() -> Value {
    let pipe = load("pipeline");
    pipe.header["data"].clone()
}

// Expected values: fixtures/rules/lair_info.json (MME's LoadLairInfo and GetLairInfo, run by the oracle).
#[test]
fn lair_info_fixture() {
    let fx = load("lair_info");
    let world = build_world(&pipeline_data(), &fx.header["data"]["table"]);
    let caches = caches(&fx);
    let mut bad = Vec::new();
    for case in &fx.cases {
        bad.extend(run_case(&world, &fx, &caches, case));
    }
    eprintln!(
        "lair_info: {} cases, {} mismatches",
        fx.cases.len(),
        bad.len()
    );
    assert!(
        bad.is_empty(),
        "{} mismatches, first:\n{}",
        bad.len(),
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
    assert!(fx.cases.len() >= 25);
}

// GetLairInfoIndex and SetLairInfo, derived by hand from the procedures: slots count up from 1 (0
// is MME's unused slot), the text is compared without case and keeps its first spelling, the
// empty text has no slot, and a short index is not stored.
#[test]
fn index_and_set_follow_the_procedures() {
    let mut c = LairCache::new();
    assert_eq!(c.get_lair_info_index(""), 0);
    assert_eq!(c.get_lair_info_index("AB-1-2"), 1);
    assert_eq!(c.get_lair_info_index("ab-1-2"), 1);
    assert_eq!(c.get_lair_info_index("10-1-1"), 2);
    assert_eq!(c.get_lair_info_index("AB-1-2"), 1);
    assert_eq!(c.stored("aB-1-2").unwrap().group_index, "AB-1-2");

    let mut u = LairInfo {
        group_index: "AB-1-2".into(),
        mob_list: "5,6".into(),
        avg_hp: 40,
        damage_out: 9,
        max_regen: Currency::from_scaled(30_000),
        ..LairInfo::default()
    };
    c.set_lair_info(&u);
    let s = c.stored("ab-1-2").unwrap();
    assert_eq!((s.mob_list.as_str(), s.avg_hp), ("5,6", 40));
    // no attack config: the damage is not stored; a positive max regen is
    assert_eq!(s.damage_out, 0);
    assert_eq!(s.max_regen, Currency::from_scaled(30_000));
    u.global_attack_config = "cfg".into();
    u.max_regen = Currency::from_scaled(0);
    c.set_lair_info(&u);
    let s = c.stored("ab-1-2").unwrap();
    assert_eq!((s.damage_out, s.global_attack_config.as_str()), (9, "cfg"));
    assert_eq!(s.max_regen, Currency::from_scaled(30_000));
    let before = c.get_lair_info_index("zz-1");
    c.set_lair_info(&LairInfo {
        group_index: "zz-1".into(),
        ..LairInfo::default()
    });
    assert_eq!(c.get_lair_info_index("zz-1"), before);
    assert_eq!(c.get_lair_info_index("zz-1"), 3);
}

// CalculateMonsterPartyDamage is CalculateMonsterDamageVsCharALL(True) and then
// `tLastAvgLairInfo = GetLairInfo("")` (read from the procedure; the oracle cannot run frmMain's
// progress-bar form).
#[test]
fn party_damage_is_the_all_function_and_resets_the_last_average() {
    let fx = load("lair_info");
    let world = build_world(&pipeline_data(), &fx.header["data"]["table"]);
    let case = fx
        .cases
        .iter()
        .find(|c| c["id"] == "party3-warrior-anti1")
        .unwrap();
    let s = setup(&world, &fx, case);
    let env = GlueEnv {
        profile: Profile::Stock,
        nmr_ver: 1.83,
    };
    let chr = s.state.sim_inputs();
    let mut cache = LairCache::new();
    cache.last_avg_lair_info = LairInfo {
        group_index: "1-2-3".into(),
        mobs: Currency::from_scaled(10_000),
        ..LairInfo::default()
    };
    let run = |cache: &mut LairCache| {
        let mut sim = MonsterSimInput::default();
        let mut rnd = Rnd::from_state(327_680);
        calculate_monster_party_damage(cache, &world, &env, 100, &chr, &s.party, &mut sim, &mut rnd)
            .unwrap()
    };
    let got = run(&mut cache);
    let mut sim = MonsterSimInput::default();
    let mut rnd = Rnd::from_state(327_680);
    let all = touchstone_rules::monster_glue::calculate_monster_damage_vs_char_all(
        &world, &env, true, 100, &chr, &s.party, &mut sim, &mut rnd,
    )
    .unwrap();
    assert_eq!(got, all);
    assert_eq!(got.len(), world.monsters.len());
    assert_eq!(cache.last_avg_lair_info, LairInfo::default());
}

// The real LairEnv answers GetLairAveragesFromLocs: with one lair entry the average is that lair
// (hand-derived from the procedure: one lair divides by one, so the lair's own figures come back
// with regen applied to exp and HP).
#[test]
fn averages_over_one_lair_use_the_cache() {
    let fx = load("lair_info");
    let world = build_world(&pipeline_data(), &fx.header["data"]["table"]);
    let caches = caches(&fx);
    let case = fx.cases.iter().find(|c| c["id"] == "solo-warrior").unwrap();
    let s = setup(&world, &fx, case);
    let p = pieces(&s.state);
    let ctx = context(&world, &p, &s);
    let key = set_current_attack_type_config(&ctx, &s.state.character, "");
    let mut cache = LairCache::load(&world, Profile::Stock, 1.83);
    let mut sim = MonsterSimInput::default();
    let mut rnd = Rnd::from_state(327_680);
    let mut env = CharLairEnv {
        cache: &mut cache,
        query: LairQuery {
            profile: Profile::Stock,
            startup: false,
            use_character: true,
            party: &s.party,
            global_attack_config: &key.config,
        },
        callouts: CharCallouts {
            world: &world,
            damage: damage_env(&world, &ctx, &p, &s),
            vs_party: &caches.party,
            vs_char: &caches.chr,
            vs_default: &caches.default,
            sim_rounds: 1000,
            sim: &mut sim,
            rnd: &mut rnd,
        },
    };
    let (group, regen) = ("11-6-8", 2);
    let one = env.get_lair_info(group, regen);
    assert!(one.mobs.scaled() > 0, "the lair is in the table");
    let loc = format!("[{group}][{regen}]Group(lair): 1/1");
    let avg = get_lair_averages_from_locs(
        &mut env,
        &loc,
        &LairLocsInput {
            nmr_ver: 1.83,
            summoned_by: &loc,
            global_attack_config: &key.config,
        },
    );
    assert_eq!(avg.total_lairs, 1);
    assert_eq!(avg.mob_list, one.mob_list);
    assert_eq!(avg.mobs, one.mobs);
    assert_eq!(avg.damage_out, one.damage_out);
    assert_eq!(avg.avg_exp, one.avg_exp.checked_mul(one.max_regen).unwrap());
}
