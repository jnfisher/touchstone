//! `.mmec` parsing and writing (card P4-24). The sample files in `fixtures/data/mmec/` are
//! hand-written in the key order and number formatting of MME's SaveCharacter.

use touchstone_rules::character::AttackType;
use touchstone_rules::mmec::{self, ItemStack};

fn sample(name: &str) -> Vec<u8> {
    let p = format!(
        "{}/../../fixtures/data/mmec/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

const FILES: [&str; 4] = [
    "warrior.mmec",
    "empty-name.mmec",
    "mage-spell.mmec",
    "thief-backstab.mmec",
];

#[test]
fn round_trip_is_byte_exact() {
    // expected bytes: the file itself, written in SaveCharacter's order
    for f in FILES {
        let bytes = sample(f);
        let m = mmec::parse(&bytes).unwrap();
        assert_eq!(mmec::write(&m), bytes, "{f}");
    }
}

#[test]
fn warrior_values() {
    let m = mmec::parse(&sample("warrior.mmec")).unwrap();
    let c = &m.character;
    assert_eq!(c.name, "Thorin");
    assert_eq!(
        (c.class, c.race, c.level, c.alignment),
        (Some(1), Some(2), 30, 1)
    );
    assert_eq!(c.stats, [95, 45, 50, 70, 85, 40]);
    assert_eq!(c.mr_text, "62");
    assert!(c.quests[0] && c.quests[5] && !c.quests[1]);
    assert_eq!(c.quest_options, [1, 0, 0, 0]);
    assert_eq!(c.attack.attack_type, AttackType::PhysAttack);
    assert_eq!(c.attack.heal_rounds, 1);
    assert_eq!(c.equipped[0], 101);
    assert_eq!(c.equipped[16], 310);
    assert_eq!((c.equipped[9], c.equipped[10]), (412, 413));
    assert_eq!(c.stat_adjustments.len(), 47);
    assert_eq!((c.stat_adjustments[0], c.stat_adjustments[5]), (2, -1));
    assert_eq!(
        m.carried,
        vec![
            ItemStack { item: 321, qty: 1 },
            ItemStack { item: 150, qty: 3 }
        ]
    );
    assert_eq!(m.stash, vec![ItemStack { item: 77, qty: 2 }]);
    // untouched sections ride along
    assert!(
        m.extra
            .iter()
            .any(|e| e.key == "WeaponCompare" && e.value == "310,311,")
    );
    assert!(
        m.extra
            .iter()
            .any(|e| e.section == "MonsterFilter" && e.key == "HP")
    );
    assert!(
        m.extra
            .iter()
            .any(|e| e.key == "DataFile" && e.value == "data-v1.11p.mdb")
    );
}

#[test]
fn empty_name_and_spell_attack() {
    assert_eq!(
        mmec::parse(&sample("empty-name.mmec"))
            .unwrap()
            .character
            .name,
        ""
    );
    let m = mmec::parse(&sample("mage-spell.mmec")).unwrap();
    let a = &m.character.attack;
    assert_eq!(a.attack_type, AttackType::Spell);
    assert_eq!((a.spell_num, a.spell_lvl, a.manual_p), (17, 12, 0));
    assert_eq!(
        (
            a.heal_type,
            a.heal_spell_num,
            a.heal_spell_lvl,
            a.heal_rounds
        ),
        (3, 17, 5, 20)
    );
    assert!(a.use_meditate && !a.backstab);
    assert_eq!(m.character.bless[..4], [11, 12, 13, 0]);
    assert_eq!(m.character.learned.spells[..5], [17, 18, 19, 20, 0]);
    assert_eq!(m.character.add_weight, 25);
    assert!(m.character.use_add_weight);
}

#[test]
fn cp1252_name() {
    let bytes = sample("thief-backstab.mmec");
    assert!(bytes.contains(&0xE9));
    let m = mmec::parse(&bytes).unwrap();
    assert_eq!(m.character.name, "Kit \u{e9}");
    assert_eq!(m.character.attack.backstab_weapon, 310);
    assert!(m.character.attack.backstab);
    assert_eq!(m.character.attack.heal_manual, 15000);
}

fn parse_text(t: &str) -> mmec::Mmec {
    mmec::parse(t.as_bytes()).unwrap()
}

#[test]
fn defaults_and_gates() {
    // LoadCharacter: missing keys read as 0, the attack block needs CurrentAttackType > 0
    let m = parse_text(
        "[PlayerInfo]\nbGlobalAttackBackstab=1\nCurrentAttackMA=2\nnGlobalAttackHealRounds=9\n",
    );
    let c = &m.character;
    assert_eq!((c.class, c.race, c.level), (Some(0), Some(0), 0));
    assert!(!c.attack.backstab);
    assert_eq!(c.attack.martial_art, 0);
    assert_eq!(c.attack.heal_rounds, 1);
    assert_eq!(c.stat_adjustments, vec![0; 47]);
}

#[test]
fn clamps() {
    let m = parse_text(
        "[PlayerInfo]\nName=0\nLevel=5000\nStrength=-4\nAlignment=9\nQuest_2nd=7\nQuest_6th=6\nQuest_Extra2=-1\n\
         CurrentAttackType=5\nCurrentAttackManual=99999999\nCurrentAttackManualMag=-3\n\
         nGlobalAttackHealType=1\nnGlobalAttackHealRounds=99\nnGlobalAttackHealSpellLVL=20000\nnGlobalAttackHealManual=-1\n\
         char_StatAdjustments=1,2,x\n[Bless]\nBless0=5\nBless1=5\nBless2=-3\n",
    );
    let c = &m.character;
    assert_eq!(c.name, "");
    assert_eq!((c.level, c.stats[0], c.alignment), (999, 0, 0));
    assert_eq!(c.quest_options, [0, 6, 0, 0]);
    assert_eq!((c.attack.manual_p, c.attack.manual_m), (9_999_999, 0));
    assert_eq!(
        (
            c.attack.heal_rounds,
            c.attack.heal_spell_lvl,
            c.attack.heal_manual
        ),
        (50, 9999, 0)
    );
    assert_eq!(c.stat_adjustments[..4], [1, 2, 0, 0]);
    assert_eq!(c.bless[..3], [5, 0, 0]);
}

#[test]
fn spell_attack_without_spell_and_weaponless_attack_fall_back() {
    let m = parse_text("[PlayerInfo]\nCurrentAttackType=2\n");
    assert_eq!(m.character.attack.attack_type, AttackType::OneShot);
    let m = parse_text("[PlayerInfo]\nCurrentAttackType=1\n[Inventory]\nWeapon=0\n");
    assert_eq!(m.character.attack.attack_type, AttackType::OneShot);
}

#[test]
fn ini_semantics() {
    // case-insensitive names, first duplicate wins, comments, quotes, blank value = default
    let m = parse_text(
        "; c\n[playerinfo]\nlevel = 12 \nLEVEL=13\nName=\"Bob\"\nStrength=\nAddWeight=7\n[Mystery]\nX=1\n",
    );
    assert_eq!(m.character.level, 12);
    assert_eq!(m.character.name, "Bob");
    assert_eq!(m.character.stats[0], 0);
    assert_eq!(m.extra.len(), 1);
    assert_eq!(m.extra[0].section, "Mystery");
}

#[test]
fn unknown_keys_survive_and_not_mmec_is_refused() {
    let m = parse_text("[PlayerInfo]\nName=A\nFuture=yes\n[<DataVersion>]\nLastSaveDir=C:\\x\n");
    let out = String::from_utf8(mmec::write(&m)).unwrap();
    assert!(out.contains("Future=yes\r\n"));
    assert!(out.contains("[<DataVersion>]\r\nLastSaveDir=C:\\x\r\n"));
    assert!(mmec::parse(b"hello").is_err());
    assert!(mmec::parse(b"[Settings]\nA=1\n").is_err());
}

#[test]
fn native_character_writes_loadable_file() {
    // a character with no extra: PlayerInfo defaults are LoadCharacter's, filter sections absent
    let mut m = mmec::Mmec::default();
    m.character.name = "N".into();
    let text = String::from_utf8(mmec::write(&m)).unwrap();
    assert!(text.starts_with("[PlayerInfo]\r\nName=N\r\nDataFile=\r\n"));
    assert!(text.contains("WeaponVSCharAC=50\r\n"));
    assert!(!text.contains("[MonsterFilter]"));
    let back = mmec::parse(text.as_bytes()).unwrap();
    assert_eq!(back.character.name, "N");
}

#[test]
fn item_manager_limits() {
    let m = mmec::Mmec {
        carried: (1..=60).map(|i| ItemStack { item: i, qty: 0 }).collect(),
        ..mmec::Mmec::default()
    };
    let text = String::from_utf8(mmec::write(&m)).unwrap();
    let line = text.lines().find(|l| l.starts_with("IM_CARRIED=")).unwrap();
    assert_eq!(line.matches('|').count(), 50);
    assert!(line.starts_with("IM_CARRIED=1|1,2|1,"));
}
