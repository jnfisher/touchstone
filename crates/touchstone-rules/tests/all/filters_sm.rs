// Fixture tests for the filters_sm family (FilterSpells, FilterMonsters): see docs/porting.md step 5.
//
// Expected values: fixtures/rules/filters_sm.json, produced by the oracle running frmMain's own FilterSpells and
// FilterMonsters (cut out of the form) over a subset of the stock tables. Spells are listed by number; monsters by number with
// the optional arguments of the AddMonster2LV call as a tag (see the generator), or as whole rows in the cases that ran the
// real AddMonster2LV. The World is rebuilt from the header's "data".
use super::fixture::{Fixture, load};
use super::monster_rows::{
    NoCasts, caches, currency_array, f, global, i, party_of, row_json, settings, text_of,
};
use super::pipeline::{Inputs, world as pipeline_world};
use serde_json::Value;
use touchstone_model::{
    AbilitySlot, Lair, MidSpell, Monster, MonsterAttack, Spell, World, WorldParts,
};
use touchstone_rules::Profile;
use touchstone_rules::character::{
    AttackTypeMud, CharacterProfile, populate_character_profile, set_current_attack_type_config,
};
use touchstone_rules::filters::{
    Answer, DamagePrompts, MonsterAdd, MonsterDialog, MonsterExtras, MonsterFilter,
    MonsterFilterInputs, Reachability, SpellFilter, SpellFilterEnv, filter_monsters, filter_spells,
};
use touchstone_rules::lairs::{CharCallouts, CharLairEnv, LairCache, LairQuery};
use touchstone_rules::monster_sim::MonsterSimInput;
use touchstone_rules::pipeline::refresh_all;
use touchstone_rules::rows::monster::{MonsterRowInputs, add_monster_row, backstab_attack};
use touchstone_rules::spell_cast::UsableEnv;
use touchstone_vb6 as vb;
use touchstone_vb6::{Currency, Rnd};

fn slots<const N: usize>(v: &Value) -> [AbilitySlot; N] {
    let mut out = [AbilitySlot::default(); N];
    for p in v.as_array().unwrap() {
        out[i(&p[0]) as usize] = AbilitySlot {
            ability: i(&p[1]),
            value: i(&p[2]),
        };
    }
    out
}

fn text_or_none(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_string())
}

fn build_world(pipe_data: &Value, data: &Value) -> World {
    let base = pipeline_world(pipe_data);
    let monsters = data["monsters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let blank = MonsterAttack {
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
            let mut attacks: [MonsterAttack; 5] = std::array::from_fn(|_| blank.clone());
            for a in r[26].as_array().unwrap() {
                attacks[i(&a[0]) as usize] = MonsterAttack {
                    name: String::new(),
                    att_type: i(&a[1]),
                    acc: i(&a[2]),
                    pct: i(&a[3]),
                    true_pct: f(&a[4]),
                    min: 0,
                    max: 0,
                    energy: 0,
                    hit_spell: i(&a[5]),
                };
            }
            let mut mid_spells: [MidSpell; 5] = Default::default();
            for m in r[27].as_array().unwrap() {
                mid_spells[i(&m[0]) as usize] = MidSpell {
                    spell: i(&m[1]),
                    pct: i(&m[2]),
                    lvl: 0,
                };
            }
            Monster {
                number: i(&r[0]),
                name: text_or_none(r[1].as_str().unwrap()),
                weapon: 0,
                armour_class: i(&r[6]),
                damage_resist: i(&r[7]),
                follow_pct: i(&r[15]),
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
                align: i(&r[16]),
                regen_time: i(&r[2]),
                game_limit: i(&r[17]),
                avg_lair_exp: f(&r[11]),
                script_value: 0.0,
                r: i(&r[18]),
                p: i(&r[19]),
                g: i(&r[20]),
                s: i(&r[21]),
                c: i(&r[22]),
                death_spell: i(&r[14]),
                create_spell: 0,
                attacks,
                mid_spells,
                drops: Default::default(),
                abilities: slots(&r[25]),
                in_game: i(&r[23]) == 1,
                summoned_by: text_or_none(r[24].as_str().unwrap()),
            }
        })
        .collect();
    let lairs = data["lairs"]
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
    // the pipeline's spells are full rows (the characters cast some of them); the scenario's rows add what the filters read
    let spells: Vec<Spell> = data["spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let n = i(&r[0]);
            let mut s = base
                .spells
                .iter()
                .find(|s| s.number == n)
                .or_else(|| base.spells.first())
                .unwrap()
                .clone();
            s.number = n;
            s.req_level = i(&r[1]);
            s.magery = i(&r[2]);
            s.magery_lvl = i(&r[3]);
            s.learnable = i(&r[4]) > 0;
            s.targets = i(&r[5]);
            s.att_type = i(&r[6]);
            s.dur = i(&r[7]);
            s.dur_inc_lvls = i(&r[8]);
            s.dur_inc = i(&r[9]);
            s.learned_from = text_or_none(&"x".repeat(i(&r[10]) as usize));
            s.casted_by = text_or_none(&"x".repeat(i(&r[11]) as usize));
            s.classes = text_or_none(r[12].as_str().unwrap());
            s.abilities = [AbilitySlot::default(); 10];
            for (k, a) in r[13].as_array().unwrap().iter().enumerate() {
                s.abilities[k] = AbilitySlot {
                    ability: i(&a[0]),
                    value: i(&a[1]),
                };
            }
            s
        })
        .collect();
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

/// The ItemData of the selected entry of a combo element (a list, or the sparse object the pipeline uses).
fn selected(c: &Value) -> i32 {
    let k = c["ListIndex"].as_i64().unwrap_or(-1);
    let d = &c["ItemData"];
    if k < 0 {
        0
    } else if d.is_array() {
        i(&d[k as usize])
    } else {
        d[k.to_string()].as_i64().unwrap_or(0) as i32
    }
}

/// The inputs of a case: the form controls with the family-wide setup behind them, and the globals likewise.
fn inputs(fx: &Fixture, case: &Value) -> Inputs {
    let mut g = fx.header["setup"]["globals"].clone();
    for (k, v) in case["inputs"]["globals"].as_object().unwrap() {
        g[k] = v.clone();
    }
    let mut inp = Inputs::new(&fx.header, case);
    inp.globals = g;
    inp
}

fn nav_caption(case: &Value) -> &str {
    case["outputs"]["nav"].as_str().unwrap_or("")
}

// ---------------------------------------------------------------------------------------------- spells

fn spell_case(world: &World, fx: &Fixture, case: &Value) -> Vec<String> {
    let id = case["id"].as_str().unwrap();
    assert!(case.get("error").is_none(), "{id}: the oracle raised");
    assert!(case.get("events").is_none(), "{id}: {}", case["events"]);
    let inp = inputs(fx, case);
    let remove = case["inputs"]["steps"][0]["args"]
        .as_array()
        .is_some_and(|a| !a.is_empty());
    let use_character = inp.ctl("chkGlobalFilter")["Value"].as_i64() == Some(1);
    let class = selected(inp.elem("cmbGlobalClass", 0));
    let mut t_char = CharacterProfile::default();
    if use_character && !remove {
        t_char.class = class;
        t_char.level = inp.text("txtGlobalLevel", 0) as i32;
        t_char.align = inp.ctl("cmbGlobalAlignment")["ListIndex"]
            .as_i64()
            .unwrap_or(0) as i16;
    }
    let ci = |n: &str| inp.ctl(n)["ListIndex"].as_i64().unwrap_or(0) as i32;
    let filter = SpellFilter {
        magery: ci("cmbSpellMagery"),
        magery_level: ci("cmbSpellMageryLevel"),
        learnable_only: inp.elem("chkSpellOptions", 1)["Value"].as_i64() == Some(1),
        attack_type: ci("cmbSpellAttackType"),
        target: ci("cmbSpellTarget"),
        contains_ability: selected(inp.ctl("cmbSpellContainsAbil")),
    };
    let usable = UsableEnv {
        profile: Profile::Stock,
        spells: &world.spells,
        classes: &world.classes,
        nmr_ver: inp.g("nNMRVer").as_f64().unwrap(),
        only_in_game: inp.gb("bOnlyInGame"),
        disable_kai_autolearn: inp.gb("bDisableKaiAutolearn"),
    };
    let env = SpellFilterEnv {
        usable: &usable,
        use_character,
        global_class: class,
        global_level: inp.text("txtGlobalLevel", 1),
    };
    let list = filter_spells(&env, &filter, &t_char, remove);
    let want: Vec<String> = case["outputs"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    // MME lists spells in the table's physical order, which is not by number; the port lists by number
    let mut got: Vec<String> = list.entries.iter().map(ToString::to_string).collect();
    let mut want = want;
    got.sort();
    want.sort();
    let mut bad = Vec::new();
    if got != want {
        let missing: Vec<_> = want.iter().filter(|w| !got.contains(w)).take(8).collect();
        let extra: Vec<_> = got.iter().filter(|g| !want.contains(g)).take(8).collect();
        bad.push(format!(
            "{id}: spells: {} listed, want {}; missing {missing:?}, extra {extra:?}",
            got.len(),
            want.len()
        ));
    }
    let caption = if list.filtered { "*Spells*" } else { "Spells" };
    if caption != nav_caption(case) {
        bad.push(format!(
            "{id}: caption {caption} want {}",
            nav_caption(case)
        ));
    }
    bad
}

// ---------------------------------------------------------------------------------------------- monsters

/// The scripted reachability of the generator: multiples of 7 are out of reach below level 30.
struct Scripted;

impl Reachability for Scripted {
    fn is_monster_ok(&self, number: i32, level: i32) -> bool {
        !(number % 7 == 0 && level < 30)
    }
}

fn extras_of(inp: &Inputs, case: &Value) -> MonsterExtras {
    let n = |k: &str| {
        inp.g(&format!("filter_Monster_{k}"))
            .as_f64()
            .unwrap_or(0.0)
    };
    let b = |k: &str| inp.gb(&format!("filter_Monster_{k}"));
    let mut abilities = [[0; 3]; 3];
    for st in case["inputs"]["steps"].as_array().unwrap() {
        if st["call"] == "SetAbil" {
            let a = st["args"].as_array().unwrap();
            let y = i(&a[0]) as usize;
            abilities[y] = [i(&a[1]), i(&a[2]), i(&a[3])];
        }
    }
    MonsterExtras {
        enabled: b("bExtrasEnabled"),
        armour_class: n("nArmourClass") as i32,
        damage_resist: n("nDamageResist") as i32,
        magic_res: n("nMagicRes") as i32,
        bs_def: n("nBSDef") as i32,
        game_limit: n("nGameLimit") as i32,
        avg_lair_exp: n("nAvgLairExp"),
        atk_accuracy_maj: n("nAtkAccuracyMaj") as i32,
        atk_accuracy_max: n("nAtkAccuracyMax") as i32,
        dodge: n("nDodge") as i32,
        num_lairs: n("nNumLairs") as i32,
        num_mobs_lte: n("nNumMobsLTE"),
        num_mobs_gte: n("nNumMobsGTE"),
        follow_gte: n("nFollowGTE") as i32,
        follow_lte: n("nFollowLTE") as i32,
        at_least_lairs: n("nAtLeastLairs") as i32,
        abilities,
        is_undead: b("bIsUndead"),
        non_hostile_vs_evil: b("bIsNonHostile_vEvil"),
        non_hostile_vs_ng: b("bIsNonHostile_vNG"),
        reachable_only: b("bReachableOnly"),
        at_least_lairs_on: b("bAtLeastLairs"),
        drops_cash: b("bDropsCash"),
        drops_r: b("bDropsR"),
        drops_p: b("bDropsP"),
        drops_g: b("bDropsG"),
        drops_s: b("bDropsS"),
        atk_no_poison: b("bAtkNoPoison"),
        atk_no_confusion: b("bAtkNoConfusion"),
        atk_no_fear: b("bAtkNoFear"),
        show_all: b("bShowAll"),
    }
}

/// `CStr` of a Currency.
fn cur_str(c: Currency) -> String {
    let n = c.scaled();
    let sign = if n < 0 { "-" } else { "" };
    let n = n.abs();
    let (w, fr) = (n / 10_000, n % 10_000);
    if fr == 0 {
        format!("{sign}{w}")
    } else {
        format!("{sign}{w}.{}", format!("{fr:04}").trim_end_matches('0'))
    }
}

/// The tag of the generator's recording stub for an `AddMonster2LV` call.
fn tag_of(a: &MonsterAdd, base: &CharacterProfile) -> String {
    let r = &a.args;
    let mut s = String::new();
    if r.damage_out != -9999 {
        s += &format!("d{};", r.damage_out);
    }
    if r.pass_exp.scaled() != -10_000 {
        s += &format!("e{};", cur_str(r.pass_exp));
    }
    if r.pass_recovery != -1.0 {
        s += &format!("r{};", vb::cstr(r.pass_recovery));
    }
    if r.surprise_damage_out != -9999 {
        s += &format!("s{};", r.surprise_damage_out);
    }
    if r.surprise_min_dmg != 0 {
        s += &format!("m{};", r.surprise_min_dmg);
    }
    if r.surprise_chance != 0 {
        s += &format!("c{};", r.surprise_chance);
    }
    if r.first_round_dmg != 0 {
        s += &format!("f{};", r.first_round_dmg);
    }
    if r.min_round_dmg != 0 {
        s += &format!("n{};", r.min_round_dmg);
    }
    if r.does_not_match_filter {
        s += "x;";
    }
    if (a.hp - base.hp).abs() > 0.0 {
        s += &format!("h{};", vb::cstr(a.hp));
    }
    if (a.hp_regen - base.hp_regen).abs() > 0.0 {
        s += &format!("g{};", vb::cstr(a.hp_regen));
    }
    s
}

fn dialog_of(text: &str) -> MonsterDialog {
    match text {
        "RE-Calculate Monster Damage vs Character Defenses first?" => MonsterDialog::RecalcChar,
        "Calculate Monster Damage vs Character Defenses first?" => MonsterDialog::CalcChar,
        "RE-Calculate Monster Damage vs Party Defenses first?" => MonsterDialog::RecalcParty,
        "Calculate Monster Damage vs Party Defenses first?" => MonsterDialog::CalcParty,
        other => panic!("unknown question {other:?}"),
    }
}

fn monster_case(world: &World, fx: &Fixture, case: &Value) -> Vec<String> {
    let id = case["id"].as_str().unwrap();
    assert!(case.get("error").is_none(), "{id}: the oracle raised");
    let inp = inputs(fx, case);
    let remove = case["inputs"]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["call"] == "FilterMonsters" && !s["args"].as_array().unwrap().is_empty());
    let real = inp.gb("bRealAdd");
    let nmr = inp.g("nNMRVer").as_f64().unwrap();
    let mut party = party_of(&inp);
    if remove {
        party.lair_mode = false;
    }
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
    let cc = caches(fx, case);
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
            vs_party: &cc.party,
            vs_char: &cc.chr,
            vs_default: &cc.default,
            sim_rounds: 1000,
            sim: &mut sim,
            rnd: &mut rnd,
        },
    };
    let settings = settings(fx, case);
    let ci = |n: &str| inp.ctl(n)["ListIndex"].as_i64().unwrap_or(0) as i32;
    let mut filter = MonsterFilter {
        regen_op: ci("cmbMonsterRegen"),
        regen_text: String::new(),
        hp_text: String::new(),
        exp_text: String::new(),
        damage_text: String::new(),
        magic: inp.ctl("chkMonMagic")["Value"].as_i64() == Some(1),
        magic_text: String::new(),
        ..MonsterFilter::default()
    };
    let txt = |n: &str| inp.ctl(n)["Text"].as_str().unwrap_or("").to_string();
    filter.regen_text = txt("txtMonsterRegen");
    filter.hp_text = txt("txtMonsterHP");
    filter.exp_text = txt("txtMonsterEXP");
    filter.damage_text = txt("txtMonsterDamage");
    filter.magic_text = txt("txtMonMagic");
    filter.extras = extras_of(&inp, case);
    // the SetPrompts step sets the five globals the questions read (RefreshAll resets them)
    let sp = case["inputs"]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["call"] == "SetPrompts")
        .map(|s| s["args"].as_array().unwrap().clone())
        .unwrap();
    let prompts = DamagePrompts {
        vs_char_config: sp[0].as_str().unwrap().to_string(),
        char_defense_description: sp[1].as_str().unwrap().to_string(),
        dont_prompt_char: sp[2].as_bool().unwrap(),
        party_calculated: sp[3].as_bool().unwrap(),
        dont_prompt_party: sp[4].as_bool().unwrap(),
    };
    let binp = MonsterFilterInputs {
        only_in_game: inp.gb("bOnlyInGame"),
        use_exp_multi: inp.gb("UseExpMulti"),
        settings: &settings,
        possy: &cc.possy,
        global_level: inp.text("txtGlobalLevel", 0),
        reach: &Scripted,
        prompts,
    };
    let row_inputs = MonsterRowInputs {
        use_exp_multi: binp.use_exp_multi,
        is_main_list: true,
        settings: &settings,
        possy: &cc.possy,
        spawn_chance: &cc.chance,
        backstab_attack: backstab_attack(state.attack_config()),
    };
    let mut answers: Vec<i64> = case["inputs"]["msgbox"]
        .as_array()
        .map(|a| a.iter().map(|v| v.as_i64().unwrap()).collect())
        .unwrap_or_default();
    answers.reverse();
    let mut ask = |_d: MonsterDialog| match answers.pop() {
        Some(6) => Answer::Yes,
        Some(7) => Answer::No,
        Some(2) => Answer::Cancel,
        other => panic!("no answer for a question: {other:?}"),
    };
    let mut rows = Vec::new();
    let mut add = |env: &mut CharLairEnv<'_>, a: &MonsterAdd| {
        if real {
            let mut t = t_char.clone();
            t.hp = a.hp;
            t.hp_regen = a.hp_regen;
            let row = add_monster_row(env, &row_inputs, &t, a.number, &a.args);
            rows.push(row.as_ref().map_or(Value::Null, row_json));
        }
    };
    let list = filter_monsters(
        &mut env, &binp, &filter, &t_char, remove, &mut ask, &mut add,
    );
    let mut bad = Vec::new();
    let want_rows = case["outputs"]["mons"].as_array().unwrap();
    // a cancelled question leaves the oracle's list empty, as the Rust list is
    if list.cancelled {
        if !want_rows.is_empty() {
            bad.push(format!(
                "{id}: cancelled, but the oracle listed {} rows",
                want_rows.len()
            ));
        }
    } else if real {
        if rows.len() != want_rows.len() {
            bad.push(format!(
                "{id}: {} rows, want {}",
                rows.len(),
                want_rows.len()
            ));
        }
        for (k, (g, w)) in rows.iter().zip(want_rows).enumerate() {
            if g != w {
                bad.push(format!("{id} row {k}:\n  got  {g}\n  want {w}"));
                break;
            }
        }
    } else {
        let got: Vec<(String, String)> = list
            .entries
            .iter()
            .map(|a| (a.number.to_string(), tag_of(a, &t_char)))
            .collect();
        let want: Vec<(String, String)> = want_rows
            .iter()
            .map(|r| {
                // the recording list dumps a row without cells as "text:tag"
                let t = r.as_str().unwrap();
                let (n, tag) = t.split_once(':').unwrap_or((t, ""));
                (n.to_string(), tag.to_string())
            })
            .collect();
        if got != want {
            let first = got.iter().zip(&want).position(|(a, b)| a != b);
            bad.push(format!(
                "{id}: {} monsters, want {}; first difference at {first:?}\n  got  {:?}\n  want {:?}",
                got.len(),
                want.len(),
                got.iter().take(first.map_or(5, |k| k + 3)).skip(first.unwrap_or(0).saturating_sub(1)).collect::<Vec<_>>(),
                want.iter().take(first.map_or(5, |k| k + 3)).skip(first.unwrap_or(0).saturating_sub(1)).collect::<Vec<_>>(),
            ));
        }
    }
    if !list.cancelled && !list.aborted {
        let caption = if list.filtered {
            "*Monsters*"
        } else {
            "Monsters"
        };
        if caption != nav_caption(case) {
            bad.push(format!(
                "{id}: caption {caption} want {}",
                nav_caption(case)
            ));
        }
    }
    // the questions asked, and what a Yes would have recalculated
    let texts: Vec<MonsterDialog> = case
        .get("events")
        .and_then(Value::as_array)
        .map(|ev| {
            ev.iter()
                .filter(|e| e["kind"] == "msgbox")
                .map(|e| dialog_of(e["text"].as_str().unwrap()))
                .collect()
        })
        .unwrap_or_default();
    let asked: Vec<MonsterDialog> = list.asked.iter().map(|(d, _)| *d).collect();
    if asked != texts {
        bad.push(format!("{id}: asked {asked:?}, MME asked {texts:?}"));
    }
    let yes = |party: bool| {
        list.asked
            .iter()
            .filter(|(d, a)| {
                *a == Answer::Yes
                    && matches!(d, MonsterDialog::RecalcParty | MonsterDialog::CalcParty) == party
            })
            .count() as i64
    };
    if case["outputs"]["recalc"].as_i64().unwrap_or(0) != yes(false)
        || case["outputs"]["recalc_party"].as_i64().unwrap_or(0) != yes(true)
    {
        bad.push(format!("{id}: recalculation requests differ"));
    }
    if !list.cancelled && list.aborted {
        bad.push(format!("{id}: aborted by a VB error"));
    }
    let _ = (currency_array, global);
    bad
}

// Expected values: fixtures/rules/filters_sm.json (frmMain FilterSpells and FilterMonsters, run by the oracle).
#[test]
fn filters_sm_fixture() {
    let fx = load("filters_sm");
    let pipe = load("pipeline");
    let world = build_world(&pipe.header["data"], &fx.header["data"]);
    let mut bad = Vec::new();
    let (mut spells, mut mons) = (0, 0);
    for case in &fx.cases {
        if case["id"].as_str().unwrap().starts_with("sp-") {
            spells += 1;
            bad.extend(spell_case(&world, &fx, case));
        } else {
            mons += 1;
            bad.extend(monster_case(&world, &fx, case));
        }
    }
    eprintln!(
        "filters_sm: {spells} spell cases, {mons} monster cases, {} mismatches",
        bad.len()
    );
    assert!(
        bad.is_empty(),
        "{} mismatches, first:\n{}",
        bad.len(),
        bad.iter().take(6).cloned().collect::<Vec<_>>().join("\n")
    );
}
