//! Q-013b: the weave in the round (R6, second half) against the engine vectors in
//! `fixtures/quicksilver/weave_round.json` (written by `tools/qs-vectors`, family `weave_round`), and
//! its wiring into `get_damage_output`.
//!
//! `landed` cases give the share of landed blows that deal damage (the blows a weave can follow),
//! `round` cases the expected weave damage per landed blow, composed from the engine's pieces. The
//! vectors say how: the balance tool has no closed per-round figure that matches, so this is the
//! composition of the Q-013a proc figures and the engine's own blow resolution.

use std::path::PathBuf;

use serde_json::Value;
use touchstone_model::{AbilitySlot, Class, Item};
use touchstone_rules::Profile;
use touchstone_rules::attack::{
    AttackEnv, AttackRequest, EquippedGlobals, calc_stab_opener, calculate_attack,
};
use touchstone_rules::character::{
    AttackType, AttackTypeMud, CharStats, Character, Context, PartyFilter,
    populate_character_profile,
};
use touchstone_rules::combat::{
    DF_LIVING, DamageEnv, DamageRequest, add_weave, attack_profile, get_damage_output, weave_target,
};
use touchstone_rules::quicksilver::Balance;
use touchstone_rules::quicksilver::weave::{expected_per_hit, weaver};
use touchstone_rules::spell_cast::SpellCasterProfile;
use touchstone_rules::spells::SpellText;

use crate::quicksilver_melee::{NoCasts, Rig, hero, request, weapon};
use crate::quicksilver_weave::{book_of, class, int, target, usable};

fn load() -> (Value, Vec<Value>) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/quicksilver/weave_round.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let mut doc: Value = serde_json::from_str(&text).unwrap();
    let Value::Array(cases) = doc["cases"].take() else {
        panic!("no cases array");
    };
    (doc["header"].take(), cases)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * b.abs().max(1.0)
}

fn report(what: &str, total: usize, bad: &[String]) {
    assert!(
        bad.is_empty(),
        "{what}: {} of {total} differ; first:\n{}",
        bad.len(),
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn weave_round_header_and_ids() {
    let (header, cases) = load();
    assert_eq!(header["family"], "weave_round");
    assert_eq!(header["content_version"], "1.20.0");
    assert_eq!(header["case_count"].as_u64().unwrap(), cases.len() as u64);
    let ids: Vec<&str> = cases.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids sorted and unique");
    for kind in ["landed", "round"] {
        assert!(cases.iter().any(|c| c["kind"] == kind), "no {kind} cases");
    }
    let keys = &header["balance_keys"];
    assert_eq!(
        int(&keys["weave_pct"]["v120"]),
        Balance::V120.weave_pct as i32
    );
    assert_eq!(
        int(&keys["weave_pct_magery1"]["v120"]),
        Balance::V120.weave_pct_magery1 as i32
    );
    assert_eq!(int(&keys["weave_pct"]["neutral"]), 0);
}

/// The blow of a vector's hero through `calculate_attack`: critical chance and damage reduction as
/// the case says, the move's attack type.
fn blow(profile: Profile, i: &Value, class_no: i32) -> touchstone_rules::attack::AttackDamage {
    let rig = Rig::new(int(&i["weapon_min"]), int(&i["weapon_max"]));
    let mut ch = hero(class_no, int(&i["level"]));
    ch.plus_max_damage = i.get("plus_max").map_or(0, |v| int(v) as i16);
    ch.crit = int(&i["crit"]) as i16;
    let attack_type = match i.get("move").and_then(Value::as_str).unwrap_or("attack") {
        "attack" => 5,
        "bash" => 6,
        "smash" => 7,
        other => panic!("unknown move {other}"),
    };
    let req = AttackRequest {
        vs_dr: int(&i["dr"]),
        ..request(attack_type, 1)
    };
    rig.attack(profile, &ch, &req)
}

/// The share of landed blows that deal damage equals the engine's walk over its rolls, under 1.20.0
/// and (Stock, NEUTRAL) against the engine with the class shares at 100.
#[test]
fn landed_share_matches_the_engine() {
    let (_, cases) = load();
    let mut bad = Vec::new();
    let mut n = 0;
    let (mut partial, mut crit_seen) = (0, 0);
    let neutral = Profile::Quicksilver(&Balance::NEUTRAL);
    for c in cases.iter().filter(|c| c["kind"] == "landed") {
        n += 1;
        let i = &c["inputs"];
        let class_no = int(&i["class"]);
        for (col, profile) in [
            ("v120", Profile::quicksilver_v120()),
            ("neutral", Profile::Stock),
            ("neutral", neutral),
        ] {
            let hit = blow(profile, i, class_no);
            let want = c[col].as_f64().unwrap();
            if !close(hit.landed_share, want) {
                bad.push(format!(
                    "{} {col}: got {}, vector {want}",
                    c["id"], hit.landed_share
                ));
            }
            if col == "v120" {
                partial += usize::from(want > 0.0 && want < 0.999);
                crit_seen += usize::from(hit.crit_chance > 0);
            }
        }
    }
    assert!(n > 1500, "{n} landed cases");
    assert!(
        partial > 300,
        "{partial} cases with part of the blows absorbed"
    );
    assert!(crit_seen > 300, "{crit_seen} cases with a crit");
    report("landed", n, &bad);
}

/// `landed_per_swing` is the hit chance (to the percent) times the share.
#[test]
fn landed_per_swing_is_hit_chance_times_share() {
    let (_, cases) = load();
    let mut n = 0;
    for c in cases.iter().filter(|c| c["kind"] == "landed").step_by(7) {
        let i = &c["inputs"];
        let hit = blow(Profile::quicksilver_v120(), i, int(&i["class"]));
        let expect = f64::from(hit.hit_chance) / 100.0 * hit.landed_share;
        assert!(
            (hit.landed_per_swing - expect).abs() <= 0.005 + 1e-12,
            "{}: {} vs {expect}",
            c["id"],
            hit.landed_per_swing
        );
        assert!(hit.landed_per_swing <= hit.landed_share + 1e-12);
        n += 1;
    }
    assert!(n > 100);
}

/// The weave a landed blow carries: touchstone's per-hit expectation times the share of blows that
/// deal damage equals the engine's composition, for every class, level, target and blow.
#[test]
fn weave_per_blow_matches_the_engine() {
    let (header, cases) = load();
    let spells = book_of(&header);
    let neutral = Profile::Quicksilver(&Balance::NEUTRAL);
    let mut bad = Vec::new();
    let mut n = 0;
    let mut weaving = 0;
    for c in cases.iter().filter(|c| c["kind"] == "round") {
        n += 1;
        let i = &c["inputs"];
        let classes = [class(i)];
        let t = target(&i["target"]);
        let who = SpellCasterProfile {
            casting_stat: int(&i["stat"]),
            class: classes[0].number,
            level: int(&i["level"]),
            spell_dmg_bonus: int(&i["bonus"]) as i16,
            ..SpellCasterProfile::default()
        };
        let share = blow(Profile::quicksilver_v120(), i, classes[0].number).landed_share;
        let env = usable(Profile::quicksilver_v120(), &spells, &classes);
        let got = expected_per_hit(&env, &classes[0], 2, &who, &t) * share;
        let want = c["v120"]["per_blow"].as_f64().unwrap();
        if !close(got, want) || !close(share, c["v120"]["landed"].as_f64().unwrap()) {
            bad.push(format!("{}: got {got}, vector {}", c["id"], c["v120"]));
        }
        weaving += usize::from(want > 0.0);
        // Stock and the neutral balance: no weave, as the neutral column says.
        assert_eq!(c["neutral"]["per_blow"].as_f64(), Some(0.0), "{}", c["id"]);
        for profile in [Profile::Stock, neutral] {
            let env = usable(profile, &spells, &classes);
            assert_eq!(
                expected_per_hit(&env, &classes[0], 2, &who, &t),
                0.0,
                "{}",
                c["id"]
            );
        }
    }
    assert!(n > 350, "{n} round cases");
    assert!(weaving > 200, "{weaving} cases with a weave");
    report("round", n, &bad);
}

// ---------------------------------------------------------------------------------------------
// The wiring: a hybrid character through `get_damage_output`.

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

/// 1.20.0 with no weave: the same melee share and opener, so only the weave differs from `V120`.
static NO_WEAVE: Balance = Balance {
    weave_pct: 0,
    weave_pct_magery1: 0,
    ..Balance::V120
};

/// A hybrid (magery level 1 or 2) or not, the weapon and the spell book.
struct World {
    classes: Vec<Class>,
    items: Vec<Item>,
    spells: Vec<touchstone_model::Spell>,
    stats: CharStats,
    party: PartyFilter,
    character: Character,
    equipped: EquippedGlobals,
}

impl World {
    fn new(header: &Value, class_no: i32, magery: i32, magelvl: i32, level: i32) -> Self {
        let class = class(&serde_json::json!({
            "class": class_no, "magery": magery, "magelvl": magelvl
        }));
        let mut stats = CharStats::default();
        stats.weapon_number[0] = 1;
        stats.stat[10] = 300.0;
        stats.stat[33] = 10.0;
        // backstab accuracy and stealth, so the opener's blows land
        stats.stat[13] = 300.0;
        stats.stat[19] = 100.0;
        // Strength, intellect, willpower, agility, health, charm
        stats.base_stat = [50.0, 120.0, 120.0, 60.0, 50.0, 120.0];
        stats.encum_max = 1000.0;
        let mut character = Character {
            level,
            class: Some(class_no),
            race: Some(1),
            alignment: 2,
            global_filter: true,
            ..Character::default()
        };
        character.attack.attack_type = AttackType::PhysAttack;
        Self {
            classes: vec![Class {
                combat_lvl: 3,
                ..class
            }],
            items: vec![{
                let mut w = weapon(14, 40, 1200);
                w.abilities = [AbilitySlot::default(); 20];
                w
            }],
            spells: book_of(header),
            stats,
            party: PartyFilter::default(),
            character,
            equipped: EquippedGlobals::default(),
        }
    }

    fn ctx(&self, profile: Profile) -> Context<'_> {
        Context {
            profile,
            nmr_ver: 1.83,
            avg_level_max_all_stats: 0.0,
            hide_record_numbers: false,
            recovery_only: false,
            classes: &self.classes,
            items: &self.items,
            spells: &self.spells,
            text: &NoText,
            stats: &self.stats,
            party: &self.party,
        }
    }

    fn damage_env<'a>(&'a self, ctx: &'a Context<'a>) -> DamageEnv<'a> {
        DamageEnv {
            ctx,
            character: &self.character,
            races: &[],
            monsters: &[],
            equipped: &self.equipped,
            cast_text: &NoCasts,
            dat_ver_caption: "",
            accuracy_tag: "300",
            damage_out_text: ["", ""],
            party_swings_text: "",
        }
    }
}

fn foe() -> DamageRequest {
    DamageRequest {
        vs_ac: 20,
        vs_dr: 4,
        vs_mr: 40,
        vs_dodge: 0,
        defense_flags: DF_LIVING,
        ..DamageRequest::default()
    }
}

fn rounded(x: f64) -> i32 {
    // VB6 rounds halves to even; the figures here are never exactly half
    x.round() as i32
}

/// A hybrid's weave adds `round(per_hit * swings * landed_per_swing)` to the round and
/// `round(per_hit * whole swings * landed_per_swing)` to the first round; nothing else moves; a class
/// that does not weave, Stock and the neutral balance are untouched.
#[test]
fn weave_enters_get_damage_output() {
    let (header, _) = load();
    let mut seen = 0;
    for (class_no, magery, magelvl, level) in [
        (3, 2, 1, 20),
        (10, 1, 2, 40),
        (9, 4, 2, 60),
        (14, 3, 1, 100),
    ] {
        let w = World::new(&header, class_no, magery, magelvl, level);
        let ctx_on = w.ctx(Profile::quicksilver_v120());
        let ctx_off = w.ctx(Profile::Quicksilver(&NO_WEAVE));
        let on = get_damage_output(&w.damage_env(&ctx_on), &foe());
        let off = get_damage_output(&w.damage_env(&ctx_off), &foe());

        // The same blow worked out by hand from the populated profile.
        let mut t = touchstone_rules::character::CharacterProfile::default();
        populate_character_profile(
            &ctx_on,
            &w.character,
            &mut t,
            false,
            false,
            AttackTypeMud::Normal,
            1,
            false,
        );
        let env = AttackEnv {
            items: &w.items,
            classes: &w.classes,
            races: &[],
            equipped: &w.equipped,
            cast_text: &NoCasts,
            dat_ver_caption: "",
            avg_level_max_all_stats: 0.0,
            accuracy_tag: "300",
        };
        let req = AttackRequest {
            attack_type: 5,
            weapon_number: 1,
            speed_adj: 100,
            vs_ac: 20,
            vs_dr: 4,
            vs_dodge: 0,
            ..AttackRequest::default()
        };
        let mut hit = calculate_attack(
            Profile::quicksilver_v120(),
            &env,
            &attack_profile(&t),
            &req,
            &mut String::new(),
        );
        let plain = hit.clone();
        // the hand-worked blow is the one `get_damage_output` computes without the weave
        assert_eq!(
            off.average_damage.scaled(),
            i64::from(plain.round_total) * 10_000,
            "class {class_no}: the rig reproduces the round"
        );
        assert_eq!(
            off.first_round_damage.scaled(),
            i64::from(plain.first_round_damage) * 10_000
        );

        let usable_env = usable(Profile::quicksilver_v120(), &w.spells, &w.classes);
        let target = weave_target(DF_LIVING, 40, 0, [0; 5]);
        let per_hit = expected_per_hit(
            &usable_env,
            &w.classes[0],
            t.align,
            &t.spell_caster(),
            &target,
        );
        assert!(per_hit > 0.0, "class {class_no} weaves");
        let add = rounded(per_hit * hit.swings * plain.landed_per_swing);
        let add_first = rounded(per_hit * hit.swings.trunc() * plain.landed_per_swing);
        assert!(add > 0, "class {class_no}: the weave adds damage");

        assert_eq!(
            on.average_damage.scaled(),
            off.average_damage.scaled() + i64::from(add) * 10_000,
            "class {class_no} round"
        );
        assert_eq!(
            on.first_round_damage.scaled(),
            off.first_round_damage.scaled() + i64::from(add_first) * 10_000,
            "class {class_no} first round"
        );
        assert_eq!(on.min_round_damage, off.min_round_damage);
        assert_eq!(on.surprise_damage, off.surprise_damage);
        assert_eq!(on.swings.to_bits(), off.swings.to_bits());

        // The helper that does it, and its detail text.
        let w_cfg = weaver(Profile::quicksilver_v120(), &w.classes[0]).unwrap();
        let denv = w.damage_env(&ctx_on);
        let got = add_weave(&denv, &t, &target, hit.swings.trunc(), &mut hit).unwrap();
        let mean = rounded(per_hit * 100.0 / f64::from(w_cfg.chance_pct));
        assert_eq!(got, Some((w_cfg.chance_pct, mean)));
        assert_eq!(hit.round_total, plain.round_total + add);
        assert!(
            hit.attack_detail
                .ends_with(&format!("Weave: {}% for {mean}", w_cfg.chance_pct)),
            "{}",
            hit.attack_detail
        );
        seen += 1;

        // Stock and the neutral balance: nothing moves, not even the text.
        for profile in [Profile::Stock, Profile::Quicksilver(&Balance::NEUTRAL)] {
            let ctx = w.ctx(profile);
            let denv = w.damage_env(&ctx);
            let mut h = plain.clone();
            assert_eq!(
                add_weave(&denv, &t, &target, hit.swings.trunc(), &mut h),
                Ok(None)
            );
            assert_eq!(h, plain);
        }
    }
    assert_eq!(seen, 4);

    // A class with no magery, and one with spells it cannot weave (magery level 3), weaves nothing.
    for (magery, magelvl) in [(0, 0), (1, 3), (5, 2)] {
        let w = World::new(&header, 1, magery, magelvl, 40);
        let on = get_damage_output(&w.damage_env(&w.ctx(Profile::quicksilver_v120())), &foe());
        let off = get_damage_output(
            &w.damage_env(&w.ctx(Profile::Quicksilver(&NO_WEAVE))),
            &foe(),
        );
        assert_eq!(on, off, "magery {magery} level {magelvl}");
    }
}

/// A weave rides the normal blows after a stab (Q-010's opener) and shows in its text.
#[test]
fn weave_rides_the_openers_follow_up_blows() {
    let (header, _) = load();
    let mut grew = 0;
    for (class_no, magery, magelvl, level) in [(3, 2, 1, 20), (10, 1, 2, 40), (9, 4, 2, 60)] {
        let mut w = World::new(&header, class_no, magery, magelvl, level);
        w.character.attack.backstab = true;
        let ctx_on = w.ctx(Profile::quicksilver_v120());
        let ctx_off = w.ctx(Profile::Quicksilver(&NO_WEAVE));
        let on = get_damage_output(&w.damage_env(&ctx_on), &foe());
        let off = get_damage_output(&w.damage_env(&ctx_off), &foe());
        assert!(off.surprise_damage.scaled() > 0, "class {class_no} stabs");

        let mut t = touchstone_rules::character::CharacterProfile::default();
        populate_character_profile(
            &ctx_on,
            &w.character,
            &mut t,
            false,
            false,
            AttackTypeMud::Surprise,
            1,
            false,
        );
        let env = AttackEnv {
            items: &w.items,
            classes: &w.classes,
            races: &[],
            equipped: &w.equipped,
            cast_text: &NoCasts,
            dat_ver_caption: "",
            avg_level_max_all_stats: 0.0,
            accuracy_tag: "300",
        };
        let stab = AttackRequest {
            attack_type: 4,
            weapon_number: 1,
            speed_adj: 100,
            vs_ac: 20,
            vs_dr: 4,
            vs_dodge: 0,
            bs_defense: 0,
            ..AttackRequest::default()
        };
        let open = calc_stab_opener(
            Profile::quicksilver_v120(),
            &env,
            &attack_profile(&t),
            &stab,
        )
        .expect("the stab leaves energy for blows");
        let normal = calculate_attack(
            Profile::quicksilver_v120(),
            &env,
            &attack_profile(&t),
            &AttackRequest {
                attack_type: 5,
                ..stab
            },
            &mut String::new(),
        );
        let usable_env = usable(Profile::quicksilver_v120(), &w.spells, &w.classes);
        let target = weave_target(DF_LIVING, 40, 0, [0; 5]);
        let per_hit = expected_per_hit(
            &usable_env,
            &w.classes[0],
            t.align,
            &t.spell_caster(),
            &target,
        );
        let add = rounded(per_hit * f64::from(open.follow_blows) * normal.landed_per_swing);
        assert!(add > 0);
        assert_eq!(
            on.surprise_damage.scaled(),
            off.surprise_damage.scaled() + i64::from(add) * 10_000,
            "class {class_no}"
        );
        assert_eq!(on.surprise_min_damage, off.surprise_min_damage);
        assert_eq!(on.surprise_damage_chance, off.surprise_damage_chance);
        // The ordinary round still carries its own weave.
        assert!(on.average_damage > off.average_damage);
        grew += 1;
    }
    assert_eq!(grew, 3);
}
