// Fixture tests for the spell_cast family (FAM-09): see docs/porting.md step 5.
// Expected values come from fixtures/rules/spell_cast.json (MME's own code run by the oracle).
use crate::fixture::{self, Names, Textblocks, cur};
use serde_json::Value;
use std::cell::Cell;
use touchstone_model::{AbilitySlot, Class, Spell};
use touchstone_rules::Profile;
use touchstone_rules::ability_text::AbilityEnv;
use touchstone_rules::spell_cast::*;
use touchstone_rules::spells::SpellText;
use touchstone_vb6::Currency;

fn i(v: &Value) -> i64 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}"))
}

fn is_missing(v: &Value) -> bool {
    v["t"] == "missing"
}

/// Numeric argument `k` of a recorded call; an absent or Missing argument is `default`.
fn num(args: &[Value], k: usize, default: f64) -> f64 {
    match args.get(k) {
        None => default,
        Some(v) if is_missing(v) => default,
        Some(v) => v
            .as_f64()
            .or_else(|| v["v"].as_f64())
            .unwrap_or_else(|| panic!("not a number: {v}")),
    }
}

fn flag(args: &[Value], k: usize, default: bool) -> bool {
    match args.get(k) {
        None => default,
        Some(v) if is_missing(v) => default,
        Some(v) => v.as_bool().unwrap_or_else(|| panic!("not a flag: {v}")),
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
            for (k, p) in f[25].as_array().unwrap().chunks(2).enumerate() {
                abilities[k] = AbilitySlot {
                    ability: i(&p[0]) as i32,
                    value: i(&p[1]) as i32,
                };
            }
            // Only the lengths of Learned From and Casted By matter to the port.
            let text = |len: i32| (len > 0).then(|| "x".repeat(len as usize));
            Spell {
                number: n(0),
                name: Some(f[1].as_str().unwrap().to_string()),
                short: None,
                req_level: n(2),
                energy_cost: n(3),
                mana_cost: n(4),
                min_base: n(5),
                max_base: n(6),
                diff: n(7),
                type_of_resists: n(8),
                targets: n(9),
                dur: n(10),
                att_type: n(11),
                magery: n(12),
                magery_lvl: n(13),
                cap: n(14),
                max_inc_lvls: n(15),
                max_inc: n(16),
                min_inc_lvls: n(17),
                min_inc: n(18),
                dur_inc_lvls: n(19),
                dur_inc: n(20),
                abilities,
                learnable: n(21) != 0,
                learned_from: text(n(22)),
                casted_by: text(n(23)),
                classes: Some(f[24].as_str().unwrap().to_string()).filter(|s| !s.is_empty()),
            }
        })
        .collect()
}

fn class_table(case: &Value) -> Vec<Class> {
    case["inputs"]["classes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Class {
            number: i(&r[0]) as i32,
            name: String::new(),
            min_hits: 0,
            max_hits: 0,
            exp_table: 0,
            magery_type: i(&r[1]) as i32,
            magery_lvl: i(&r[2]) as i32,
            weapon_type: 0,
            armour_type: 0,
            combat_lvl: 0,
            abilities: [AbilitySlot::default(); 10],
        })
        .collect()
}

fn check_cast(got: &SpellCastValues, want: &Value) -> bool {
    let w = |k: &str| &want[k];
    w("nmincast") == got.n_min_cast
        && w("nmaxcast") == got.n_max_cast
        && w("navgcast") == got.n_avg_cast
        && w("nnumcasts").as_f64() == Some(got.n_num_casts)
        && w("ncastchance") == got.n_cast_chance
        && w("navgrounddmg") == got.n_avg_round_dmg
        && w("nminrounddmg") == got.n_min_round_dmg
        && w("navgroundheals") == got.n_avg_round_heals
        && w("nduration") == got.n_duration
        && w("ndamageresisted") == got.n_damage_resisted
        && w("nfullresistchance") == got.n_full_resist_chance
        && w("nmanacost") == got.n_mana_cost
        && w("noom") == got.n_oom
        && w("bdoesheal") == got.b_does_heal
        && w("bdoesdamage") == got.b_does_damage
        && w("savground") == got.s_avg_round.as_str()
        && w("slvlincreases") == got.s_lvl_increases.as_str()
        && w("smma") == got.s_mma.as_str()
        && w("sspellname") == got.s_spell_name.as_str()
        && w("ncastlevel") == got.n_cast_level
        && w("nspellattacktype") == got.n_spell_attack_type
        && w("nrequiredlevel") == got.n_required_level
}

#[test]
fn spell_cast_fixture() {
    check_fixture(Profile::Stock);
}

/// The whole MME fixture under `profile`. The caster carries a casting stat, class and level so a
/// profile that scales by them must undo it: stock ignores them and the neutral Quicksilver
/// balance scales by 1.
pub(crate) fn check_fixture(profile: Profile) {
    let fx = fixture::load("spell_cast");
    let table = fx
        .cases
        .iter()
        .find(|c| c["id"] == "table")
        .expect("table case");
    let spells = spell_table(table);
    let classes = class_table(table);
    // CalculateSpellCast asks only for GetAbilityStats(n, 0), the bare ability name, which reads no
    // other table; the real text port runs over empty name and textblock tables.
    let names = Names::default();
    let tbs = Textblocks::default();
    let text = AbilityEnv {
        profile,
        names: &names,
        textblocks: &tbs,
        spells: &spells,
        hide_record_numbers: false,
        quick_spell: false,
        global_level_text: None,
        avg_level_max_all_stats: 0.0,
        nest: Cell::new(0),
    };

    let mut fails = Vec::new();
    let mut ran = 0;
    for c in &fx.cases {
        let id = c["id"].as_str().unwrap();
        if id == "table" {
            continue;
        }
        // GetAbilityStats(n, 0) as MME answered it, against the port's own.
        if id.starts_with("as-") {
            let ability = i(&c["inputs"]["args"][0]) as i16;
            let want = c["outputs"]["result"].as_str().unwrap();
            let got = text.ability_stats(ability, 0, false, false);
            if got != want {
                fails.push(format!("{id}: want {want:?}, got {got:?}"));
            }
            continue;
        }
        ran += 1;
        let inp = &c["inputs"];
        let a = inp["args"].as_array().unwrap();
        let proc = inp["proc"].as_str().unwrap();
        let want = &c["outputs"]["result"];
        let escaped = c.get("error").map(|e| i(&e["number"]));
        let g = &inp["globals"];
        let usable = UsableEnv {
            profile,
            spells: &spells,
            classes: &classes,
            nmr_ver: g["nNMRVer"].as_f64().unwrap_or(0.0),
            only_in_game: g["bOnlyInGame"] == true,
            disable_kai_autolearn: g["bDisableKaiAutolearn"] == true,
        };
        let mut bad = |what: String| fails.push(format!("{id}: {what}; inputs {inp}"));
        match proc {
            "CalculateSpellCast" => {
                let f = &a[0]["fields"];
                let who = SpellCasterProfile {
                    spell_dmg_bonus: f["nSpellDmgBonus"].as_i64().unwrap_or(0) as i16,
                    spellcasting: f["nSpellcasting"].as_i64().unwrap_or(0) as i16,
                    max_mana: f["nMaxMana"].as_f64().unwrap_or(0.0),
                    spell_overhead: f["nSpellOverhead"].as_f64().unwrap_or(0.0),
                    mana_regen: f["nManaRegen"].as_f64().unwrap_or(0.0),
                    casting_stat: 170,
                    class: 12,
                    level: 45,
                };
                let req = CastRequest {
                    spell: num(a, 1, 0.0) as i32,
                    cast_level: num(a, 2, 0.0) as i32,
                    vs_mr: num(a, 3, 0.0) as i32,
                    vs_anti_magic: flag(a, 4, false),
                    vs_cold: num(a, 5, 0.0) as i16,
                    vs_fire: num(a, 6, 0.0) as i16,
                    vs_stone: num(a, 7, 0.0) as i16,
                    vs_lightning: num(a, 8, 0.0) as i16,
                    vs_water: num(a, 9, 0.0) as i16,
                };
                let env = SpellCastEnv {
                    profile,
                    spells: &spells,
                    text: &text,
                    avg_level_max_all_stats: 0.0,
                };
                let got = env.calculate_spell_cast(&who, &req);
                if !check_cast(&got, want) {
                    bad(format!("got {got:?}, want {want}"));
                }
            }
            "CalculateResistDamage" => {
                let d = &a[0];
                let dmg = if d["t"] == "cur" {
                    Currency::from_f64(d["v"].as_f64().unwrap()).unwrap()
                } else {
                    Currency::from_scaled(i(d) * 10_000)
                };
                let got = calculate_resist_damage(
                    dmg,
                    num(a, 1, 0.0) as i32,
                    num(a, 2, 2.0) as i16,
                    flag(a, 3, true),
                    flag(a, 4, false),
                    flag(a, 5, false),
                    num(a, 6, 0.0) as i32,
                );
                if cur(want) != got.scaled() {
                    bad(format!("got {got:?}, want {want}"));
                }
            }
            "ResistPct_SignedOfBase" | "NegResistPct_ShareOfTotal" => {
                let (b, f) = (num(a, 0, 0.0), num(a, 1, 0.0));
                let got = if proc == "ResistPct_SignedOfBase" {
                    resist_pct_signed_of_base(b, f)
                } else {
                    neg_resist_pct_share_of_total(b, f)
                };
                match (got, escaped) {
                    (Ok(v), None) if *want == v => {}
                    (Err(e), Some(n)) if i64::from(e.code()) == n => {}
                    (g, e) => bad(format!("got {g:?}, want {want} / error {e:?}")),
                }
            }
            "SpellAttackTypeEnum" => {
                let got = spell_attack_type_enum(num(a, 0, 0.0) as i16, flag(a, 1, false));
                if *want != got.as_str() {
                    bad(format!("got {got:?}, want {want}"));
                }
            }
            "GetSpellCastChance" => {
                let got = get_spell_cast_chance(
                    profile,
                    &spells,
                    num(a, 0, 0.0) as i16,
                    num(a, 1, 0.0) as i16,
                    flag(a, 2, false),
                    num(a, 3, 0.0) as i32,
                );
                if *want != got {
                    bad(format!("got {got}, want {want}"));
                }
            }
            "CalcRoundsToOOM" => {
                let got = calc_rounds_to_oom(
                    num(a, 0, 0.0),
                    num(a, 1, 0.0) as i32,
                    num(a, 2, 0.0),
                    num(a, 3, 0.0) as i16,
                    num(a, 4, 1.0) as i32,
                );
                if *want != got {
                    bad(format!("got {got}, want {want}"));
                }
            }
            "CalcManaRecoveryRounds" => {
                let got = calc_mana_recovery_rounds(
                    Profile::Stock,
                    num(a, 0, 0.0) as i32,
                    num(a, 1, 0.0) as i32,
                    num(a, 2, 0.0) as i32,
                    num(a, 3, 95.0) as i16,
                );
                if *want != got {
                    bad(format!("got {got}, want {want}"));
                }
            }
            "SpellIsInGame" => {
                let got = usable.spell_is_in_game(num(a, 0, 0.0) as i32);
                if *want != got {
                    bad(format!("got {got}, want {want}"));
                }
            }
            "SpellIsUsable" => {
                let got = usable.spell_is_usable(
                    num(a, 0, 0.0) as i32,
                    num(a, 1, 0.0) as i32,
                    num(a, 2, 0.0) as i16,
                    num(a, 3, 0.0) as i16,
                    flag(a, 4, false),
                );
                if *want != got {
                    bad(format!("got {got}, want {want}"));
                }
            }
            other => panic!("{id}: unknown proc {other}"),
        }
    }
    assert!(ran > 2500, "only {ran} cases ran");
    assert!(
        fails.is_empty(),
        "{} of {ran} cases failed, first: {:#?}",
        fails.len(),
        &fails[..fails.len().min(8)]
    );
}

// ---------------------------------------------------------------------------------------------
// Quicksilver (R7, R8, R9): CalculateSpellCast on a built spell. The building blocks are pinned by
// fixtures/quicksilver/spells.json (quicksilver_vectors.rs); these check where they act.

struct NoText;

impl SpellText for NoText {
    fn ability_stats(&self, _: i16, _: i16, _: bool, _: bool) -> String {
        "x".to_string()
    }
    fn spell_name(&self, _: i32) -> String {
        String::new()
    }
    fn monster_name(&self, _: i32) -> String {
        String::new()
    }
}

/// An instant spell of 100 to 200 at any level, one cast a round, resisted never.
fn built_spell(targets: i32, abilities: &[(i32, i32)], dur: i32) -> Spell {
    let mut slots = [AbilitySlot::default(); 10];
    for (k, &(ability, value)) in abilities.iter().enumerate() {
        slots[k] = AbilitySlot { ability, value };
    }
    Spell {
        number: 7,
        name: Some("test".to_string()),
        short: None,
        req_level: 1,
        energy_cost: 1000,
        mana_cost: 10,
        min_base: 100,
        max_base: 200,
        diff: 0,
        type_of_resists: 0,
        targets,
        dur,
        att_type: 4,
        magery: 1,
        magery_lvl: 1,
        cap: 0,
        max_inc_lvls: 0,
        max_inc: 0,
        min_inc_lvls: 0,
        min_inc: 0,
        dur_inc_lvls: 0,
        dur_inc: 0,
        abilities: slots,
        learnable: true,
        learned_from: None,
        casted_by: None,
        classes: None,
    }
}

/// A Mage (class 12) of level 40 with 130 Intellect.
fn mage() -> SpellCasterProfile {
    SpellCasterProfile {
        spellcasting: 0,
        max_mana: 500.0,
        mana_regen: 10.0,
        casting_stat: 130,
        class: 12,
        level: 40,
        ..SpellCasterProfile::default()
    }
}

fn cast(profile: Profile, sp: &Spell, who: &SpellCasterProfile, vs_mr: i32) -> SpellCastValues {
    let spells = [sp.clone()];
    let env = SpellCastEnv {
        profile,
        spells: &spells,
        text: &NoText,
        avg_level_max_all_stats: 0.0,
    };
    let req = CastRequest {
        spell: sp.number,
        cast_level: 20,
        vs_mr,
        ..CastRequest::default()
    };
    env.calculate_spell_cast(who, &req)
}

fn v120() -> Profile {
    Profile::quicksilver_v120()
}

fn neutral() -> Profile {
    Profile::Quicksilver(&touchstone_rules::quicksilver::Balance::NEUTRAL)
}

#[test]
fn quicksilver_scales_a_damage_spell_by_stat_and_class() {
    let who = mage();
    // fixtures/quicksilver/spells.json kind damage: class 12 level 40 stat 130 turns 1000 into 1131
    let scale = |a: i32| scale_spell_damage(v120(), a, &who);
    assert_eq!(scale(1000), 1131);
    let sp = built_spell(4, &[(1, 0)], 0);
    let stock = cast(Profile::Stock, &sp, &who, 0);
    assert_eq!((stock.n_min_cast, stock.n_max_cast), (100, 200));
    assert_eq!(stock.n_avg_round_dmg, 150);
    let qs = cast(v120(), &sp, &who, 0);
    assert_eq!(qs.n_min_cast, scale(100));
    assert_eq!(qs.n_max_cast, scale(200));
    assert_eq!(qs.n_avg_round_dmg, scale(150));
    assert_eq!(qs.n_min_round_dmg, scale(100));
    assert!(qs.n_avg_round_dmg > stock.n_avg_round_dmg);
    // the same spell cast by a character with no class and a 50 stat is untouched
    let plain = SpellCasterProfile {
        casting_stat: 50,
        class: 0,
        ..mage()
    };
    assert_eq!(cast(v120(), &sp, &plain, 0), stock_with(&sp, &plain));
}

fn stock_with(sp: &Spell, who: &SpellCasterProfile) -> SpellCastValues {
    cast(Profile::Stock, sp, who, 0)
}

#[test]
fn quicksilver_scales_damage_on_armour_before_magic_resist() {
    let who = mage();
    let sp = built_spell(4, &[(17, 0)], 0);
    let stock = cast(Profile::Stock, &sp, &who, 80);
    let qs = cast(v120(), &sp, &who, 80);
    assert!(qs.n_avg_round_dmg > stock.n_avg_round_dmg);
    // resisting 80 takes (80 - 50) / 2 = 15% off whatever the scaled roll is
    let scaled = scale_spell_damage(v120(), 150, &who);
    assert_eq!(
        qs.n_avg_round_dmg,
        (f64::from(scaled) * 0.85).round() as i32
    );
}

#[test]
fn quicksilver_leaves_heals_and_lasting_drains_alone() {
    let who = mage();
    let heal = built_spell(1, &[(18, 0)], 0);
    assert_eq!(
        cast(v120(), &heal, &who, 0),
        cast(Profile::Stock, &heal, &who, 0)
    );

    let lasting = built_spell(4, &[(8, 0)], 5);
    assert_eq!(
        cast(v120(), &lasting, &who, 0),
        cast(Profile::Stock, &lasting, &who, 0)
    );

    // a drain that does not last scales both what it deals and what it gives back
    let drain = built_spell(4, &[(8, 0)], 0);
    let stock = cast(Profile::Stock, &drain, &who, 0);
    let qs = cast(v120(), &drain, &who, 0);
    let scaled = scale_spell_damage(v120(), 150, &who);
    assert_eq!(stock.n_avg_round_dmg, stock.n_avg_round_heals);
    assert_eq!(qs.n_avg_round_dmg, scaled);
    assert_eq!(qs.n_avg_round_heals, scaled);
}

#[test]
fn quicksilver_area_spells_lose_the_resisting_share() {
    let who = SpellCasterProfile {
        casting_stat: 0,
        class: 0,
        ..mage()
    };
    // Full Attack Area, resist mode Never: stock never resists it
    let area = built_spell(12, &[(1, 0)], 0);
    let stock = cast(Profile::Stock, &area, &who, 100);
    assert_eq!(stock.n_avg_round_dmg, 150);
    assert_eq!(stock.n_full_resist_chance, 0);
    // MR 100: the roll resists 50 times in 99 and then cuts min(25, 100 / 4) = 25%
    let keep = 1.0 - 50.0 * 25.0 / 9900.0;
    assert!((area_resist_keep(v120(), 100) - keep).abs() < 1e-12);
    let qs = cast(v120(), &area, &who, 100);
    assert_eq!(qs.n_avg_round_dmg, (150.0 * keep).round() as i32);
    assert_eq!(qs.n_min_round_dmg, (100.0 * keep).round() as i32);
    // the cast roll itself is not an expectation, and it is not a full resist
    assert_eq!((qs.n_min_cast, qs.n_max_cast), (100, 200));
    assert_eq!(qs.n_full_resist_chance, 0);
    assert!(
        qs.s_avg_round
            .contains("51% chance for a monster to resist and take 25% less"),
        "{}",
        qs.s_avg_round
    );
    // no monster resistance, a single-target spell, or a heal: nothing to cut
    assert_eq!(cast(v120(), &area, &who, 0).n_avg_round_dmg, 150);
    let single = built_spell(4, &[(1, 0)], 0);
    assert_eq!(cast(v120(), &single, &who, 100).n_avg_round_dmg, 150);
    // a lasting area spell shows its cut in the "not including" list
    let lasting = built_spell(12, &[(1, 0)], 5);
    let text = cast(v120(), &lasting, &who, 100).s_avg_round;
    assert!(
        text.contains("not including 51% chance for a monster"),
        "{text}"
    );
}

#[test]
fn quicksilver_neutral_balance_changes_nothing_in_a_cast() {
    let who = mage();
    for (targets, abilities, dur) in [
        (4, vec![(1, 0)], 0),
        (4, vec![(17, 0)], 0),
        (12, vec![(1, 0)], 0),
        (12, vec![(17, 0)], 3),
        (4, vec![(8, 0)], 0),
        (1, vec![(18, 0)], 0),
    ] {
        let sp = built_spell(targets, &abilities, dur);
        for mr in [0, 1, 40, 100, 400] {
            assert_eq!(
                cast(neutral(), &sp, &who, mr),
                cast(Profile::Stock, &sp, &who, mr),
                "targets {targets} {abilities:?} dur {dur} mr {mr}"
            );
        }
    }
}
