//! The Quicksilver profile type and its balance (card Q-003). The expected V120 values are the
//! 1.20.0 figures in Quicksilver's `[balance]` documentation (`docs/config.md`) and its frozen
//! `Balance::V120`; they are pinned here so an accidental edit fails.

use touchstone_rules::Profile;
use touchstone_rules::accuracy::{get_dodge_cap, get_hit_cap, get_hit_min, get_spell_hit_cap};
use touchstone_rules::quicksilver::{
    Anchors, Balance, ClassMeleePct, ClassSpellPct, ClassWeavePct, LevelPct, V120_BALANCE,
};
use touchstone_rules::stats::{ManaRegenInput, calc_mana_regen};

/// Expected: Quicksilver docs/config.md `[balance]` defaults, one row per key.
#[test]
fn v120_scalar_values_match_the_documented_defaults() {
    let b = Balance::V120;
    assert_eq!(
        (
            b.backstab_energy_pct,
            b.backstab_stealth_div,
            b.backstab_level_pct,
            b.backstab_crit_max_pct
        ),
        (150, 4, 200, 200)
    );
    assert_eq!((b.opportunist_pct, b.opportunist_level_pct), (10, 50));
    assert_eq!(
        (
            b.weave_pct,
            b.weave_pct_magery1,
            b.weave_budget_per_level,
            b.weave_power_min_pct,
            b.weave_power_max_pct,
            b.weave_stat_bonus_pct
        ),
        (20, 10, 3, 25, 100, 50)
    );
    assert_eq!(
        (
            b.spell_stat_permille,
            b.rest_mana_mult,
            b.meditate_mana_mult
        ),
        (5, 3, 5)
    );
    assert_eq!(
        (
            b.crit_knee,
            b.spell_stat_knee,
            b.mr_stat_knee,
            b.dodge_agi_knee
        ),
        (50, 100, 110, 150)
    );
    assert_eq!((b.softcap_floor, b.softcap_floor_price), (90, 10));
    assert_eq!(b.softcap_band_prices, [15, 25, 40]);
    assert_eq!((b.aoe_resist_cut_max_pct, b.aoe_resist_cut_div), (25, 4));
    let w = b.rank_weights;
    assert_eq!((w.front, w.mid, w.back, w.solo), (3, 2, 1, 2));
    assert_eq!((b.guard_chance_base, b.guard_chance_per_combat), (25, 5));
    assert_eq!(b.scalecap, 60);
    assert!(b.is_v120());
    assert_eq!(Balance::default(), b);
    assert_eq!(b.validate(), Ok(()));
}

/// Expected: Quicksilver `Balance::V120` class tables (`class_melee_pct`, `class_spell_pct`,
/// `class_weave_pct`), sampled at the anchors and between them.
#[test]
fn v120_class_tables_match() {
    let b = Balance::V120;
    // Class numbers: 1 Warrior, 2 Witchunter, 3 Paladin, 4 Cleric, 5 Priest, 6 Missionary,
    // 7 Ninja, 8 Thief, 9 Bard, 10 Gypsy, 11 Warlock, 12 Mage, 13 Druid, 14 Ranger, 15 Mystic.
    let melee = [
        100, 114, 88, 70, 100, 70, 0, 96, 70, 60, 70, 100, 100, 60, 0,
    ];
    for (i, want) in melee.iter().enumerate() {
        let class = i as u16 + 1;
        if *want != 0 {
            assert_eq!(
                b.class_melee_pct.of(class, 50),
                *want,
                "melee class {class}"
            );
        }
    }
    assert_eq!(b.class_melee_pct.of(7, 40), 77);
    assert_eq!(b.class_melee_pct.of(7, 50), 86); // 77 + 19 * 10 / 20 = 86.5, truncated
    assert_eq!(b.class_melee_pct.of(7, 60), 96);
    assert_eq!(b.class_melee_pct.of(7, 1), 77);
    assert_eq!(b.class_melee_pct.of(15, 40), 50);
    assert_eq!(b.class_melee_pct.of(15, 60), 77);
    assert_eq!(b.class_spell_pct.of(5, 10), 67);
    assert_eq!(b.class_spell_pct.of(12, 40), 87);
    assert_eq!(b.class_spell_pct.of(12, 60), 93);
    assert_eq!(b.class_spell_pct.of(1, 33), 100);
    assert_eq!(b.class_weave_pct.of(4, 20), 49);
    assert_eq!(b.class_weave_pct.of(6, 30), 70);
    assert_eq!(b.class_weave_pct.of(6, 40), 86);
    assert_eq!(b.class_weave_pct.of(9, 40), 35);
    assert_eq!(b.class_weave_pct.of(9, 60), 38);
    assert_eq!(b.class_weave_pct.of(10, 30), 50);
    assert_eq!(b.class_weave_pct.of(10, 40), 68);
    assert_eq!(b.class_weave_pct.of(10, 60), 122);
    assert_eq!(b.class_weave_pct.of(11, 40), 64);
    assert_eq!(b.class_weave_pct.of(11, 60), 84);
    assert_eq!(b.class_weave_pct.of(14, 1), 50);
    assert_eq!(b.class_weave_pct.of(3, 1), 100);
}

/// Anchors run in straight lines, the fraction is kept for `scale`, and both truncate toward zero.
#[test]
fn level_pct_interpolates_and_truncates() {
    let p = LevelPct::Points(Anchors::new([(30, 70), (60, 64)]));
    assert_eq!(p.at(1), 70);
    assert_eq!(p.at(45), 67);
    assert_eq!(p.at(100), 64);
    // 70 - 6/30 = 69.8 percent at level 31
    assert_eq!(p.scale(31, 1000), 698);
    assert_eq!(p.scale(31, 10), 6);
    assert_eq!(p.scale(31, -10), -6);
    assert_eq!(LevelPct::One(50).scale(7, 9), 4);
    assert_eq!(LevelPct::One(50).scale(7, i32::MAX), i32::MAX / 2);
}

/// A class number outside the fifteen reads 100 and leaves the amount whole.
#[test]
fn unknown_class_is_whole() {
    let t = ClassMeleePct::even(80);
    assert_eq!(t.of(0, 10), 100);
    assert_eq!(t.of(16, 10), 100);
    assert_eq!(t.scale(16, 10, 77), 77);
    assert_eq!(t.scale(1, 10, 77), 61);
    assert!(ClassSpellPct::even(1).get(0).is_none());
    assert_eq!(ClassWeavePct::even(9).of(15, 1), 9);
}

#[test]
fn knee_third_counts_a_third_above() {
    assert_eq!(Balance::knee_third(90, 100), 90);
    assert_eq!(Balance::knee_third(100, 100), 100);
    assert_eq!(Balance::knee_third(102, 100), 100);
    assert_eq!(Balance::knee_third(130, 100), 110);
    assert_eq!(Balance::knee_third(255, 100), 151);
    assert_eq!(Balance::knee_third(255, 65_535), 255);
    assert_eq!(Balance::knee_third(-5, 40), -5);
}

#[test]
fn neutral_is_valid_and_stock_equivalent() {
    let n = Balance::NEUTRAL;
    assert_eq!(n.validate(), Ok(()));
    assert!(!n.is_v120());
    assert_eq!(n.crit_knee, 40);
    assert_eq!((n.backstab_stealth_div, n.backstab_level_pct), (10, 100));
    assert_eq!(
        (n.opportunist_pct, n.weave_pct, n.weave_pct_magery1),
        (0, 0, 0)
    );
    assert_eq!(n.aoe_resist_cut_max_pct, 0);
    for class in 1..=15 {
        assert_eq!(n.class_melee_pct.of(class, 30), 100);
        assert_eq!(n.class_spell_pct.of(class, 30), 100);
        assert_eq!(n.class_weave_pct.of(class, 30), 100);
    }
    // each knee is beyond any stat value
    for knee in [n.spell_stat_knee, n.mr_stat_knee, n.dodge_agi_knee] {
        assert_eq!(Balance::knee_third(32_767, knee), 32_767);
    }
}

#[test]
fn validate_names_the_key() {
    let mut b = Balance::V120;
    b.backstab_stealth_div = 0;
    assert!(
        b.validate()
            .unwrap_err()
            .to_string()
            .contains("balance.backstab_stealth_div")
    );
    let mut b = Balance::V120;
    b.aoe_resist_cut_div = 0;
    assert!(
        b.validate()
            .unwrap_err()
            .to_string()
            .contains("balance.aoe_resist_cut_div")
    );
    let mut b = Balance::V120;
    b.rank_weights.mid = 0;
    assert!(
        b.validate()
            .unwrap_err()
            .to_string()
            .contains("balance.rank_weights.mid")
    );
    let mut b = Balance::V120;
    b.weave_pct = 101;
    let e = b.validate().unwrap_err().to_string();
    assert!(
        e.contains("balance.weave_pct") && e.contains("0..=100"),
        "{e}"
    );
    let mut b = Balance::V120;
    b.weave_power_min_pct = 101;
    assert!(
        b.validate()
            .unwrap_err()
            .to_string()
            .contains("weave_power_min_pct")
    );
}

/// A table pasted from Quicksilver's config: only the differing keys, V120 for the rest; a typo
/// fails.
#[test]
fn partial_override_reads_over_v120_and_unknown_keys_fail() {
    let b: Balance = serde_json::from_str(
        r#"{"crit_knee": 45, "softcap_band_prices": [1, 2, 3],
            "class_melee_pct": {"mystic": [[40, 50], [60, 80]], "warrior": 90}}"#,
    )
    .unwrap();
    assert_eq!(b.crit_knee, 45);
    assert_eq!(b.softcap_band_prices, [1, 2, 3]);
    assert_eq!(b.backstab_energy_pct, 150);
    assert_eq!(b.class_melee_pct.of(1, 5), 90);
    assert_eq!(b.class_melee_pct.of(15, 60), 80);
    // keys left out of a class table keep their 1.20.0 value
    assert_eq!(b.class_melee_pct.of(2, 5), 114);
    assert!(!b.is_v120());
    assert!(serde_json::from_str::<Balance>(r#"{"crit_nee": 45}"#).is_err());
    assert!(serde_json::from_str::<Balance>(r#"{"class_melee_pct": {"fighter": 1}}"#).is_err());
    assert!(serde_json::from_str::<LevelPct>("[[40, 70], [30, 65]]").is_err());
    assert!(serde_json::from_str::<LevelPct>("[]").is_err());
    assert_eq!(
        serde_json::from_str::<Balance>("{}").unwrap(),
        Balance::V120
    );
}

#[test]
fn profile_variant_is_copy_eq_and_defaults_to_stock() {
    assert_eq!(Profile::default(), Profile::Stock);
    let q = Profile::quicksilver_v120();
    let q2 = q;
    assert_eq!(q, q2);
    assert_eq!(q, Profile::Quicksilver(&V120_BALANCE));
    assert_ne!(q, Profile::Stock);
    assert_eq!(Profile::Stock.balance(), None);
    assert_eq!(q.balance(), Some(&Balance::V120));
    // equality is by value, so a different balance is a different profile
    static NEUTRAL: Balance = Balance::NEUTRAL;
    assert_ne!(q, Profile::Quicksilver(&NEUTRAL));
}

/// No behaviour change yet: at the sites that matched on the profile, Quicksilver answers as Stock.
/// Expected: the Stock answer (itself pinned to MME by the `stats` and `accuracy` fixtures).
#[test]
fn quicksilver_answers_as_stock_at_matched_sites() {
    static NEUTRAL: Balance = Balance::NEUTRAL;
    for q in [Profile::quicksilver_v120(), Profile::Quicksilver(&NEUTRAL)] {
        assert_eq!(get_hit_cap(q), get_hit_cap(Profile::Stock));
        assert_eq!(get_spell_hit_cap(q), get_spell_hit_cap(Profile::Stock));
        assert_eq!(get_hit_min(q, 3), get_hit_min(Profile::Stock, 3));
        assert_eq!(
            get_dodge_cap(q, 3, true),
            get_dodge_cap(Profile::Stock, 3, true)
        );
        for magic_type in 0..=5 {
            for meditating in [false, true] {
                let i = ManaRegenInput {
                    level: 37,
                    intellect: 88,
                    willpower: 61,
                    charm: 45,
                    magic_level: 3,
                    magic_type,
                    mp_regen: 25,
                    meditating,
                };
                assert_eq!(calc_mana_regen(q, &i), calc_mana_regen(Profile::Stock, &i));
            }
        }
    }
}
