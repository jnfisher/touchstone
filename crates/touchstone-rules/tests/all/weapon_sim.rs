//! The weapon simulator's dice. touchstone's own tool, so there is no MME fixture: the checks are
//! the arithmetic the simulation must agree with (`CalculateAttack`'s averages, `CalcSwings`'
//! swings a round) and that a seed always rolls the same dice.

use touchstone_rules::attack::AttackDamage;
use touchstone_rules::weapon_sim::{
    Blow, Opener, Rng, simulate_fights, simulate_openers, simulate_rounds,
};

/// A blow that always lands for exactly `dmg`, costing `energy`.
fn sure(dmg: u32, energy: u32) -> Blow {
    Blow {
        hit_pct: 100,
        crit_pct: 0,
        min: dmg,
        max: dmg,
        crit_min: 0,
        crit_max: 0,
        extra_chance: 0.0,
        extra_dmg: 0,
        opp_pct: 0,
        opp_dmg: 0,
        energy,
    }
}

fn close(got: f64, want: f64, pct: f64) {
    assert!(
        (got - want).abs() <= want.abs() * pct / 100.0,
        "{got} is not within {pct}% of {want}"
    );
}

#[test]
fn leftover_energy_carries_to_the_next_round() {
    // frmSwingCalc CalcSwings: 400 energy a swing is 2, 3, 2, 3 swings; the average is the raw 2.5
    let mut buf = Vec::new();
    let s = simulate_rounds(&sure(10, 400), 1000, &mut Rng::new(1), &mut buf);
    assert_eq!((s.min, s.max), (20, 30));
    assert!((s.mean - 25.0).abs() < 1e-9, "{}", s.mean);
    assert_eq!(s.rounds, 1000);
    assert_eq!(s.zero_pct, 0.0);
}

#[test]
fn a_round_holds_at_most_five_swings() {
    // modMMudFunc CalculateAttack: swings are capped at 5
    let mut buf = Vec::new();
    let s = simulate_rounds(&sure(1, 50), 100, &mut Rng::new(1), &mut buf);
    assert_eq!((s.min, s.max), (5, 5));
}

#[test]
fn the_mean_lands_on_calculate_attacks_average() {
    // modMMudFunc CalculateAttack: per swing (1 - crit) * avg hit + crit * avg crit, times swings
    // and the hit chance. Hit 6..14 (avg 10), crit 28..56 (avg 42) at 25%, 80% to hit, 2 swings.
    let blow = Blow {
        hit_pct: 80,
        crit_pct: 25,
        min: 6,
        max: 14,
        crit_min: 28,
        crit_max: 56,
        ..sure(0, 500)
    };
    let want = (0.75 * 10.0 + 0.25 * 42.0) * 2.0 * 0.8;
    let mut buf = Vec::new();
    let s = simulate_rounds(&blow, 50_000, &mut Rng::new(7), &mut buf);
    close(s.mean, want, 1.5);
    assert!(s.min == 0 && s.max <= 112, "{s:?}");
    assert!(s.p10 <= s.median && s.median <= s.p90);
    // both swings miss 4% of the time
    assert!((s.zero_pct - 4.0).abs() < 0.6, "{}", s.zero_pct);
}

#[test]
fn cast_and_opportunist_damage_ride_on_landed_blows() {
    // CalculateAttack: the cast adds its per-swing average to every swing that hits; the
    // opportunist (Quicksilver R4) adds its mean to a share of the blows that dealt damage
    let blow = Blow {
        hit_pct: 50,
        extra_chance: 0.25,
        extra_dmg: 40,
        opp_pct: 20,
        opp_dmg: 30,
        ..sure(10, 1000)
    };
    let want = 0.5 * (10.0 + 0.25 * 40.0 + 0.2 * 30.0);
    let mut buf = Vec::new();
    let s = simulate_rounds(&blow, 50_000, &mut Rng::new(3), &mut buf);
    close(s.mean, want, 2.5);
}

#[test]
fn a_seed_rolls_the_same_dice_and_streams_differ() {
    let blow = Blow {
        hit_pct: 70,
        min: 1,
        max: 20,
        ..sure(0, 300)
    };
    let run = |rng: &mut Rng| simulate_rounds(&blow, 500, rng, &mut Vec::new());
    assert_eq!(
        run(&mut Rng::for_stream(9, 100, 0)),
        run(&mut Rng::for_stream(9, 100, 0))
    );
    assert_ne!(
        run(&mut Rng::for_stream(9, 100, 0)),
        run(&mut Rng::for_stream(9, 100, 1))
    );
    assert_ne!(
        run(&mut Rng::for_stream(9, 100, 0)),
        run(&mut Rng::for_stream(9, 101, 0))
    );
}

#[test]
fn an_opening_round_is_the_stab_and_its_follow_blows() {
    let mut buf = Vec::new();
    let stock = Opener {
        stab: sure(100, 1000),
        follow: None,
    };
    let s = simulate_openers(&stock, 200, &mut Rng::new(1), &mut buf);
    assert_eq!((s.min, s.max), (100, 100));
    // Quicksilver R2: the stab, then the normal blows the rest of the round pays for
    let quick = Opener {
        stab: sure(100, 500),
        follow: Some((sure(10, 250), 2)),
    };
    let s = simulate_openers(&quick, 200, &mut Rng::new(1), &mut buf);
    assert_eq!((s.min, s.max), (120, 120));
}

#[test]
fn fights_count_rounds_to_the_kill() {
    let mut buf = Vec::new();
    // 2, 3, 2 swings of 10: 70 hit points fall in the third round
    let k = simulate_fights(
        None,
        Some(&sure(10, 400)),
        70,
        50,
        100,
        &mut Rng::new(1),
        &mut buf,
    )
    .unwrap();
    assert_eq!((k.mean, k.median, k.p90, k.killed_pct), (3.0, 3, 3, 100.0));
    // an opener of 60 leaves one round of work; one of 70 is the whole fight
    let opener = |dmg| Opener {
        stab: sure(dmg, 1000),
        follow: None,
    };
    let k = simulate_fights(
        Some(&opener(60)),
        Some(&sure(10, 400)),
        70,
        50,
        100,
        &mut Rng::new(1),
        &mut buf,
    )
    .unwrap();
    assert_eq!(k.mean, 2.0);
    let k = simulate_fights(
        Some(&opener(70)),
        None,
        70,
        50,
        100,
        &mut Rng::new(1),
        &mut buf,
    )
    .unwrap();
    assert_eq!(k.mean, 1.0);
    // a fight that runs out of rounds counts as the limit and is not a kill
    let k = simulate_fights(
        None,
        Some(&sure(1, 1000)),
        500,
        10,
        20,
        &mut Rng::new(1),
        &mut buf,
    )
    .unwrap();
    assert_eq!((k.mean, k.killed_pct), (20.0, 0.0));
    // nothing that can hurt, or no hit points to take: no figure
    assert!(
        simulate_fights(
            None,
            Some(&sure(0, 400)),
            70,
            50,
            100,
            &mut Rng::new(1),
            &mut buf
        )
        .is_none()
    );
    assert!(
        simulate_fights(
            None,
            Some(&sure(5, 400)),
            0,
            50,
            100,
            &mut Rng::new(1),
            &mut buf
        )
        .is_none()
    );
}

#[test]
fn a_blow_is_read_from_calculate_attacks_answer() {
    let a = AttackDamage {
        min_dmg: 3,
        max_dmg: 9,
        hit_chance: 85,
        crit_chance: 12,
        min_crit: 18,
        max_crit: 36,
        avg_extra_hit: 40,
        avg_extra_swing: 10,
        swings: 2.5,
        energy: 400,
        opportunist_pct: 25,
        opportunist_avg: 14,
        ..AttackDamage::default()
    };
    let b = Blow::from_attack(&a).unwrap();
    assert_eq!((b.hit_pct, b.crit_pct, b.min, b.max), (85, 12, 3, 9));
    assert_eq!((b.crit_min, b.crit_max, b.energy), (18, 36, 400));
    // the cast averages 40 when it fires and 10 over every swing: it fires a quarter of the time
    assert_eq!((b.extra_chance, b.extra_dmg), (0.25, 40));
    assert_eq!((b.opp_pct, b.opp_dmg), (25, 14));
    assert!(b.can_hurt());
    // CalculateAttack's all-zero answer (an attack that cannot be made) is no blow
    assert_eq!(Blow::from_attack(&AttackDamage::default()), None);
    // manual damage has no energy: one swing a round
    let manual = AttackDamage { swings: 1.0, ..a };
    assert_eq!(
        Blow::from_attack(&AttackDamage {
            energy: 0,
            ..manual
        })
        .unwrap()
        .energy,
        1000
    );
}
