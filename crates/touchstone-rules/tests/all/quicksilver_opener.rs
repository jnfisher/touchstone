//! Q-010: the backstab round (R2) against the engine vectors in `fixtures/quicksilver/opener.json`,
//! written by `tools/qs-vectors`.
//!
//! `opener` cases give, for one normal blow's energy, the stab's energy and the normal blows that
//! follow it, under the 1.20.0 balance (`v120`), a stock-equivalent one (`neutral`: a stab that
//! fills the round) and an off-grid one (`alt`: a stab cheaper than a blow). `energy` cases give the
//! engine's own normal-blow and stab energy for a character.

use std::path::PathBuf;

use serde_json::Value;
use touchstone_model::{AbilitySlot, Item};
use touchstone_rules::Profile;
use touchstone_rules::attack::{
    AttackEnv, AttackRequest, CastText, CharacterProfile, EnergyInput, EquippedGlobals,
    calc_energy_used, calc_stab_opener, calculate_attack, follow_blows, stab_energy,
};
use touchstone_rules::combat::DamageOutput;
use touchstone_rules::quicksilver::Balance;
use touchstone_vb6::Currency;

/// The off-grid balance of the vectors' `alt` column (see the header's `balance_keys`).
static ALT: Balance = Balance {
    backstab_energy_pct: 80,
    ..Balance::V120
};
static NEUTRAL: Balance = Balance::NEUTRAL;

fn load() -> (Value, Vec<Value>) {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/quicksilver/opener.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let mut doc: Value = serde_json::from_str(&text).unwrap();
    let Value::Array(cases) = doc["cases"].take() else {
        panic!("no cases array");
    };
    (doc["header"].take(), cases)
}

fn int(v: &Value) -> i32 {
    i32::try_from(
        v.as_i64()
            .unwrap_or_else(|| panic!("integer expected, got {v}")),
    )
    .unwrap()
}

fn pair(v: &Value) -> (i32, i32) {
    let a = v
        .as_array()
        .unwrap_or_else(|| panic!("pair expected, got {v}"));
    (int(&a[0]), int(&a[1]))
}

#[test]
fn opener_header_and_ids() {
    let (header, cases) = load();
    assert_eq!(header["family"], "opener");
    assert_eq!(header["content_version"], "1.20.0");
    assert_eq!(header["case_count"].as_u64().unwrap(), cases.len() as u64);
    let ids: Vec<&str> = cases.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids sorted and unique");
    for kind in ["opener", "energy"] {
        assert!(cases.iter().any(|c| c["kind"] == kind), "no {kind} cases");
    }
    // The vectors' balances are the ones this crate carries.
    let k = &header["balance_keys"]["backstab_energy_pct"];
    assert_eq!(int(&k["v120"]), Balance::V120.backstab_energy_pct as i32);
    assert_eq!(int(&k["neutral"]), NEUTRAL.backstab_energy_pct as i32);
    assert_eq!(int(&k["alt"]), ALT.backstab_energy_pct as i32);
}

/// The stab's energy and the blows after it, from one normal blow's energy.
#[test]
fn opener_cost_and_follow_blows() {
    let (_, cases) = load();
    let mut n = 0;
    let mut bad = Vec::new();
    for c in cases.iter().filter(|c| c["kind"] == "opener") {
        n += 1;
        let normal = int(&c["inputs"]["normal"]);
        for (profile, col) in [
            (Profile::quicksilver_v120(), "v120"),
            (Profile::Quicksilver(&ALT), "alt"),
            (Profile::Quicksilver(&NEUTRAL), "neutral"),
        ] {
            let stab = stab_energy(profile, normal);
            let got = (stab, follow_blows(stab, normal));
            if got != pair(&c[col]) {
                bad.push(format!(
                    "{} {col}: got {got:?}, vector {:?}",
                    c["id"], c[col]
                ));
            }
        }
        // Stock: the stab is the whole round, the same as the neutral column says.
        let stab = stab_energy(Profile::Stock, normal);
        assert_eq!(
            (stab, follow_blows(stab, normal)),
            pair(&c["neutral"]),
            "{}",
            c["id"]
        );
    }
    assert!(n > 500);
    assert!(
        bad.is_empty(),
        "{} differ; first:\n{}",
        bad.len(),
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}

/// MME's energy for the same character. Where it is 200 or more it must equal the engine's normal
/// blow (the screening of the neutral column); below that the engine's floor of 200 applies and MME
/// has none (docs/differences.md).
fn mme_energy(i: &Value) -> i32 {
    let e = calc_energy_used(&EnergyInput {
        combat: Currency::from_f64(f64::from(int(&i["combat"]))).unwrap(),
        level: Currency::from_f64(f64::from(int(&i["level"]))).unwrap(),
        attack_speed: Currency::from_f64(f64::from(int(&i["speed"]))).unwrap(),
        agility: Currency::from_f64(f64::from(int(&i["agility"]))).unwrap(),
        encum: int(&i["encum_pct"]) as i16,
        ..EnergyInput::default()
    })
    .unwrap();
    (e.to_f64()) as i32
}

#[test]
fn energy_cases_match_mme_and_the_stab_rule() {
    let (_, cases) = load();
    let (mut n, mut above) = (0, 0);
    let mut bad = Vec::new();
    for c in cases.iter().filter(|c| c["kind"] == "energy") {
        n += 1;
        let mme = mme_energy(&c["inputs"]);
        let (engine_normal, v120_stab) = pair(&c["v120"]);
        if mme.max(200) != engine_normal {
            bad.push(format!("{}: MME {mme}, engine {engine_normal}", c["id"]));
            continue;
        }
        // The engine's neutral stab fills the round; its other stabs cost the share of its own
        // normal blow, which is what stab_energy computes from that energy.
        for (profile, col) in [
            (Profile::quicksilver_v120(), "v120"),
            (Profile::Quicksilver(&ALT), "alt"),
            (Profile::Quicksilver(&NEUTRAL), "neutral"),
        ] {
            let want = pair(&c[col]);
            if want.0 != engine_normal || stab_energy(profile, engine_normal) != want.1 {
                bad.push(format!("{} {col}: vector {want:?}", c["id"]));
            }
        }
        if mme >= 200 {
            above += 1;
            assert_eq!(stab_energy(Profile::quicksilver_v120(), mme), v120_stab);
        }
    }
    assert!(n > 2000 && above > 1000, "{n} cases, {above} unfloored");
    assert!(
        bad.is_empty(),
        "{} differ; first:\n{}",
        bad.len(),
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}

/// The weapon text is never asked: the test weapon has no cast ability.
struct NoCasts;

impl CastText for NoCasts {
    fn spell_name(&self, n: i32) -> String {
        panic!("cast text asked for spell {n}")
    }
    fn spell_eq(&self, n: i32, _: i16) -> String {
        panic!("cast text asked for spell {n}")
    }
}

fn weapon(min: i32, max: i32, speed: i32) -> Item {
    Item {
        number: 1,
        name: "test weapon".into(),
        limit: 0,
        encum: 0,
        item_type: 1,
        use_count: 0,
        price: 0,
        currency: 0,
        min,
        max,
        armour_class: 0,
        damage_resist: 0,
        weapon_type: 0,
        armour_type: 0,
        worn: 0,
        accy: 0,
        gettable: true,
        str_req: 0,
        speed,
        not_droppable: false,
        destroy_on_death: false,
        retain_after_uses: false,
        class_rest: [0; 10],
        race_rest: [0; 10],
        negate_spell: [0; 10],
        abilities: [AbilitySlot::default(); 20],
        in_game: true,
        obtained_from: None,
        references: None,
    }
}

struct Rig {
    items: [Item; 1],
    equipped: EquippedGlobals,
    ch: CharacterProfile,
}

impl Rig {
    fn new(i: &Value) -> Self {
        Self {
            items: [weapon(20, 30, int(&i["speed"]))],
            equipped: EquippedGlobals::default(),
            ch: CharacterProfile {
                level: int(&i["level"]),
                strength: 50,
                agility: int(&i["agility"]) as i16,
                stealth: 100,
                class_stealth: true,
                combat: int(&i["combat"]) as i16,
                encum_pct: int(&i["encum_pct"]) as i16,
                ..CharacterProfile::default()
            },
        }
    }

    fn env(&self) -> AttackEnv<'_> {
        AttackEnv {
            items: &self.items,
            classes: &[],
            races: &[],
            equipped: &self.equipped,
            cast_text: &NoCasts,
            dat_ver_caption: "",
            avg_level_max_all_stats: 0.0,
            accuracy_tag: "",
        }
    }
}

const STAB: AttackRequest = AttackRequest {
    attack_type: 4,
    weapon_number: 1,
    abil68_slow: false,
    speed_adj: 100,
    vs_ac: 0,
    vs_dr: 0,
    vs_dodge: 0,
    force_calc: false,
    specify_damage: -1.0,
    specify_accy: -1.0,
    bs_defense: 0,
};

/// `calc_stab_opener` end to end: the energies and blow count are the engine's, and the damage is
/// that many normal blows of the same weapon (no crit and every blow lands in this rig).
#[test]
fn stab_opener_through_calculate_attack() {
    let (_, cases) = load();
    let mut some = 0;
    for c in cases.iter().filter(|c| c["kind"] == "energy").step_by(3) {
        let i = &c["inputs"];
        let mme = mme_energy(i);
        if mme < 200 {
            continue;
        }
        let rig = Rig::new(i);
        let env = rig.env();
        for (profile, col) in [
            (Profile::quicksilver_v120(), "v120"),
            (Profile::Quicksilver(&ALT), "alt"),
        ] {
            let (normal, stab) = pair(&c[col]);
            let want_blows = follow_blows(stab, normal);
            let got = calc_stab_opener(profile, &env, &rig.ch, &STAB);
            let id = c["id"].as_str().unwrap();
            let Some(open) = got else {
                assert_eq!(want_blows, 0, "{id} {col}: no opener");
                continue;
            };
            some += 1;
            assert_eq!(
                (open.normal_energy, open.stab_energy, open.follow_blows),
                (normal, stab, want_blows),
                "{id} {col}"
            );
            let hit = calculate_attack(
                profile,
                &env,
                &rig.ch,
                &AttackRequest {
                    attack_type: 5,
                    ..STAB
                },
                &mut String::new(),
            );
            assert_eq!(
                open.follow_damage,
                hit.avg_hit * open.follow_blows,
                "{id} {col}: damage"
            );
            assert_eq!(
                open.follow_min_damage,
                hit.min_dmg * open.follow_blows,
                "{id} {col}"
            );
        }
        // Stock and the neutral balance: the stab fills the round.
        for profile in [Profile::Stock, Profile::Quicksilver(&NEUTRAL)] {
            assert_eq!(calc_stab_opener(profile, &env, &rig.ch, &STAB), None);
        }
    }
    assert!(some > 100, "{some} openers");
}

/// A request that is not a stab, or a manual one, has no opener.
#[test]
fn no_opener_without_a_stab() {
    let i =
        serde_json::json!({"speed": 800, "level": 5, "agility": 60, "combat": 3, "encum_pct": 0});
    let rig = Rig::new(&i);
    let env = rig.env();
    let p = Profile::quicksilver_v120();
    assert!(calc_stab_opener(p, &env, &rig.ch, &STAB).is_some());
    let normal = AttackRequest {
        attack_type: 5,
        ..STAB
    };
    assert_eq!(calc_stab_opener(p, &env, &rig.ch, &normal), None);
    let manual = AttackRequest {
        specify_damage: 50.0,
        ..STAB
    };
    assert_eq!(calc_stab_opener(p, &env, &rig.ch, &manual), None);
}

/// `GetDamageOutput` over the combat fixture's characters: under 1.20.0 the surprise figure grows by
/// the blows after the stab and nothing else about it moves; with the stab costing the whole round it
/// is the stab alone.
#[test]
fn surprise_round_in_get_damage_output() {
    static FILLS_ROUND: Balance = Balance {
        backstab_energy_pct: 100_000,
        ..Balance::V120
    };
    let with = crate::combat::run_cases(Profile::quicksilver_v120());
    let alone = crate::combat::run_cases(Profile::Quicksilver(&FILLS_ROUND));
    assert_eq!(with.len(), alone.len());
    let (mut grew, mut seen) = (0, 0);
    for (a, b) in with.iter().zip(&alone) {
        assert_eq!(a.id, b.id);
        let (Some(a), Some(b)) = (&a.damage, &b.damage) else {
            continue;
        };
        let same = |x: &DamageOutput, y: &DamageOutput| {
            x.average_damage == y.average_damage
                && x.first_round_damage == y.first_round_damage
                && x.min_round_damage == y.min_round_damage
                && x.surprise_damage_chance == y.surprise_damage_chance
                && x.swings.to_bits() == y.swings.to_bits()
        };
        assert!(same(a, b), "only the surprise figures differ");
        if a.surprise_damage.scaled() > 0 {
            seen += 1;
            assert!(a.surprise_damage >= b.surprise_damage);
            assert!(a.surprise_min_damage >= b.surprise_min_damage);
            if a.surprise_damage > b.surprise_damage {
                grew += 1;
            }
        } else {
            assert_eq!(a.surprise_damage, b.surprise_damage);
        }
    }
    assert!(seen > 10 && grew > 5, "{seen} surprise cases, {grew} grew");
}
