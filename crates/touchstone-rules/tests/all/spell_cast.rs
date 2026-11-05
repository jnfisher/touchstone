// Fixture tests for the spell_cast family (FAM-09): see docs/porting.md step 5.
// Expected values come from fixtures/rules/spell_cast.json (MME's own code run by the oracle).
use crate::fixture::{self, cur};
use serde_json::Value;
use std::collections::HashMap;
use touchstone_model::{AbilitySlot, Class, Spell};
use touchstone_rules::Profile;
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

/// GetAbilityStats(n, 0) answers the oracle gave, replayed.
struct Recorded(HashMap<i16, String>);

impl SpellText for Recorded {
    fn ability_stats(&self, ability: i16, value: i16, _calc: bool, _pct: bool) -> String {
        assert_eq!(value, 0, "CalculateSpellCast only asks for the bare name");
        self.0
            .get(&ability)
            .unwrap_or_else(|| panic!("no recorded GetAbilityStats for {ability}"))
            .clone()
    }
    fn spell_name(&self, _: i32) -> String {
        unreachable!("not used by CalculateSpellCast")
    }
    fn monster_name(&self, _: i32) -> String {
        unreachable!("not used by CalculateSpellCast")
    }
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
    let fx = fixture::load("spell_cast");
    let table = fx
        .cases
        .iter()
        .find(|c| c["id"] == "table")
        .expect("table case");
    let spells = spell_table(table);
    let classes = class_table(table);
    let mut rec = Recorded(HashMap::new());
    for c in &fx.cases {
        if c["id"].as_str().unwrap().starts_with("as-") {
            rec.0.insert(
                i(&c["inputs"]["args"][0]) as i16,
                c["outputs"]["result"].as_str().unwrap().to_string(),
            );
        }
    }

    let mut fails = Vec::new();
    let mut ran = 0;
    for c in &fx.cases {
        let id = c["id"].as_str().unwrap();
        if id == "table" || id.starts_with("as-") {
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
            profile: Profile::Stock,
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
                    profile: Profile::Stock,
                    spells: &spells,
                    text: &rec,
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
                    Profile::Stock,
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
