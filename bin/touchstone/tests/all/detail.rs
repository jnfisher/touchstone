//! `/api/abilities`, `/api/spells/{n}/effect` and `/api/monsters/{n}/max-damage` over a small
//! invented World.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use touchstone::state::{Loaded, SourceInfo};
use touchstone::{AppState, app};
use touchstone_model::{
    AbilitySlot, Class, DropSlot, Info, Item, MidSpell, Monster, MonsterAttack, Race, Spell, World,
    WorldParts,
};
use tower::ServiceExt;

fn slots<const N: usize>(pairs: &[(i32, i32)]) -> [AbilitySlot; N] {
    let mut out = [AbilitySlot::default(); N];
    for (i, (ability, value)) in pairs.iter().enumerate() {
        out[i] = AbilitySlot {
            ability: *ability,
            value: *value,
        };
    }
    out
}

fn item(number: i32, name: &str, abilities: &[(i32, i32)]) -> Item {
    Item {
        number,
        name: name.to_owned(),
        limit: 0,
        encum: 0,
        item_type: 1,
        use_count: 0,
        price: 0,
        currency: 0,
        min: 0,
        max: 0,
        armour_class: 0,
        damage_resist: 0,
        weapon_type: 0,
        armour_type: 0,
        worn: 0,
        accy: 0,
        gettable: true,
        str_req: 0,
        speed: 0,
        not_droppable: false,
        destroy_on_death: false,
        retain_after_uses: false,
        class_rest: [0; 10],
        race_rest: [0; 10],
        negate_spell: [0; 10],
        abilities: slots(abilities),
        in_game: true,
        obtained_from: None,
        references: None,
    }
}

/// The columns of the `table` case of fixtures/rules/spells.json, in its order.
#[allow(clippy::too_many_arguments)]
fn spell(f: [i32; 17], name: &str, abilities: &[(i32, i32)]) -> Spell {
    Spell {
        number: f[0],
        name: Some(name.to_owned()),
        short: None,
        req_level: f[1],
        energy_cost: f[2],
        mana_cost: 0,
        min_base: f[3],
        max_base: f[4],
        diff: 0,
        type_of_resists: 0,
        targets: f[5],
        dur: f[6],
        att_type: f[7],
        magery: f[8],
        magery_lvl: 0,
        cap: f[9],
        max_inc_lvls: f[10],
        max_inc: f[11],
        min_inc_lvls: f[12],
        min_inc: f[13],
        dur_inc_lvls: f[14],
        dur_inc: f[15],
        abilities: slots(abilities),
        learnable: f[16] != 0,
        learned_from: None,
        casted_by: None,
        classes: None,
    }
}

fn race(number: i32, abilities: &[(i32, i32)]) -> Race {
    Race {
        number,
        name: format!("race {number}"),
        m_int: 0,
        m_wil: 0,
        m_str: 0,
        m_hea: 0,
        m_agl: 0,
        m_chm: 0,
        x_int: 0,
        x_wil: 0,
        x_str: 0,
        x_hea: 0,
        x_agl: 0,
        x_chm: 0,
        hp_per_lvl: 0,
        exp_table: 0,
        base_cp: 0,
        abilities: slots(abilities),
    }
}

fn class(number: i32, name: &str, abilities: &[(i32, i32)]) -> Class {
    Class {
        number,
        name: name.to_owned(),
        min_hits: 0,
        max_hits: 0,
        exp_table: 0,
        magery_type: 0,
        magery_lvl: 0,
        weapon_type: 0,
        armour_type: 0,
        combat_lvl: 0,
        abilities: slots(abilities),
    }
}

fn attack(att_type: i32, energy: i32, pct: i32, acc: i32, min: i32, max: i32) -> MonsterAttack {
    MonsterAttack {
        name: String::new(),
        att_type,
        acc,
        pct,
        true_pct: 0.0,
        min,
        max,
        energy,
        hit_spell: 0,
    }
}

fn monster(number: i32, weapon: i32, attacks: [MonsterAttack; 5]) -> Monster {
    Monster {
        number,
        name: Some(format!("monster {number}")),
        weapon,
        armour_class: 0,
        damage_resist: 0,
        follow_pct: 0,
        magic_res: 0,
        bs_defense: 0,
        exp: 0.0,
        exp_multi: 1.0,
        hp: 10,
        energy: 1000,
        avg_dmg: 0.0,
        avg_dmg_phys: 0.0,
        avg_dmg_spell: 0.0,
        greet_txt: 0,
        hp_regen: 0,
        charm_lvl: 0,
        r#type: 0,
        undead: 0,
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
        mid_spells: [MidSpell::default(); 5],
        drops: [DropSlot::default(); 10],
        abilities: [AbilitySlot::default(); 10],
        in_game: true,
        summoned_by: None,
    }
}

fn none() -> MonsterAttack {
    attack(0, 0, 0, 0, 0, 0)
}

fn app_with_world() -> axum::Router {
    // Spell rows 1 and 4 are the stock ones in fixtures/rules/spells.json (case "table").
    let magic_missile = spell(
        [1, 1, 1000, 4, 12, 8, 0, 4, 1, 6, 0, 1, 0, 0, 0, 0, 1],
        "magic missile",
        &[(17, 0)],
    );
    let ethereal_shield = spell(
        [4, 5, 0, 3, 3, 1, 60, 4, 1, 18, 2, 1, 2, 1, 1, 3, 1],
        "ethereal shield",
        &[(10, 0)],
    );
    // A cheap spell that does a flat 7 damage per tick for 3 rounds.
    let poison = spell(
        [20, 1, 100, 0, 0, 1, 3, 4, 1, 0, 0, 0, 0, 0, 0, 0, 0],
        "venom",
        &[(1, 7)],
    );
    let mut bolt = monster(
        1,
        9,
        [
            attack(1, 500, 100, 90, 3, 10),
            none(),
            none(),
            none(),
            none(),
        ],
    );
    bolt.drops[0] = DropSlot { item: 10, pct: 50 };
    let mut caster = monster(
        2,
        0,
        [
            // Spell attack: AttAcc is the spell, AttMin its success chance, AttMax its level.
            attack(2, 1000, 100, 1, 100, 30),
            none(),
            none(),
            none(),
            none(),
        ],
    );
    caster.mid_spells[0] = MidSpell {
        spell: 20,
        pct: 100,
        lvl: 1,
    };
    let mut sting = monster(
        3,
        0,
        [
            attack(1, 1000, 100, 50, 4, 6),
            none(),
            none(),
            none(),
            none(),
        ],
    );
    sting.attacks[0].hit_spell = 20;
    let world = World::new(WorldParts {
        info: Info {
            nmr_version: "v1.8.3".into(),
            dat_file_version: String::new(),
            date: String::new(),
            time: String::new(),
            custom: String::new(),
            legit: true,
            disable_kai: false,
            update_url: String::new(),
        },
        items: vec![
            // Weapon 9 carries +3 MaxDamage (ability 4) for its wielder.
            item(9, "spiked club", &[(4, 3), (22, 5)]),
            // Drop 10 carries +4 MaxDamage and drops half the time: +2.
            item(10, "ogre tooth", &[(4, 4)]),
            item(
                11,
                "wand of missiles",
                &[(114, 50), (43, 1), (59, 3), (22, 5), (101, 1)],
            ),
        ],
        monsters: vec![bolt, caster, sting],
        spells: vec![magic_missile, ethereal_shield, poison],
        rooms: vec![],
        shops: vec![],
        classes: vec![class(3, "Paladin", &[(59, 3), (22, 5)])],
        races: vec![race(
            1,
            &[(22, 5), (7, 5), (2, -3), (42, 4), (73, 22), (101, 1)],
        )],
        lairs: vec![],
        tbinfo: vec![],
    })
    .unwrap();
    let info = SourceInfo {
        kind: "test",
        path: String::new(),
    };
    app(AppState::new(Loaded::build(world, info, None, 1).unwrap()))
}

async fn get(uri: &str) -> (StatusCode, Value) {
    let res = app_with_world()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&body)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&body).into_owned()));
    (status, v)
}

fn texts(v: &Value) -> Vec<String> {
    v["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["text"].as_str().unwrap().to_owned())
        .collect()
}

mod abilities {
    use super::*;

    #[tokio::test]
    async fn race_lines_match_the_ability_text_fixture() {
        // fixtures/rules/ability_text.json: as22-0013 "Accuracy +5", rep7-0009 "DR +0.5",
        // rep-0010 "AC -3", as42-0002 "LearnSp (ethereal shield(4))". Ability 73 (derived from GetAbilityStats) names its
        // value; 101 is message-only and prints nothing.
        let (st, v) = get("/api/abilities/races/1").await;
        assert_eq!(st, StatusCode::OK);
        let t = texts(&v);
        assert_eq!(
            &t[..4],
            [
                "Accuracy +5",
                "DR +0.5",
                "AC -3",
                "LearnSp (ethereal shield(4))"
            ]
        );
        assert_eq!(t.len(), 5, "{t:?}");
        assert_eq!(t[4], "DispellMagic (Accuracy)");
        // The label/value split keeps the part after the name.
        assert_eq!(v["lines"][0]["name"], "Accuracy");
        assert_eq!(v["lines"][0]["detail"], "+5");
    }

    #[tokio::test]
    async fn classes_skip_class_ok() {
        let (_, v) = get("/api/abilities/classes/3").await;
        assert_eq!(texts(&v), ["Accuracy +5"]);
    }

    #[tokio::test]
    async fn item_folds_the_cast_percent_and_lists_class_ok_apart() {
        let (st, v) = get("/api/abilities/items/11").await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["class_ok"], serde_json::json!(["Paladin"]));
        let t = texts(&v);
        assert_eq!(t.len(), 2, "{t:?}");
        // Spell 1 is 4 to 12 (fixture ability_text rep43h-0000), here with its record number.
        assert!(t[0].starts_with("CastsSp [magic missile(1), "), "{}", t[0]);
        assert!(t[0].contains("4 to 12"), "{}", t[0]);
        assert!(t[0].ends_with(", 50%]"), "{}", t[0]);
        assert_eq!(t[1], "Accuracy +5");
    }

    #[tokio::test]
    async fn unknown_table_and_record() {
        assert_eq!(
            get("/api/abilities/rooms/1").await.0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            get("/api/abilities/items/999").await.0,
            StatusCode::NOT_FOUND
        );
    }
}

mod spell_effect {
    use super::*;

    #[tokio::test]
    async fn damage_range_matches_the_spells_fixture() {
        // fixtures/rules/spells.json maxx-1-2 (12) and minx-1-2 (4): spell 1 is 4 to 12 at any
        // level, its cap being 6.
        let (st, v) = get("/api/spells/1/effect?level=30").await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["damage"], serde_json::json!({ "min": 4, "max": 12 }));
        assert_eq!(v["used_level"], 6);
        assert_eq!(v["heal"], Value::Null);
        assert_eq!(v["duration"], 0);
        assert!(v["effect"].as_str().unwrap().contains("4 to 12"));
    }

    #[tokio::test]
    async fn duration_matches_the_spells_fixture() {
        // fixtures/rules/spells.json dur-0007 (spell 4 at level 12 lasts 96) and dur-0006
        // (level 0 is lifted to the required level 5: 75). The level defaults to the latter.
        let (_, v) = get("/api/spells/4/effect?level=12").await;
        assert_eq!(v["duration"], 96);
        assert_eq!(v["level"], 12);
        let (_, v) = get("/api/spells/4/effect").await;
        assert_eq!(v["duration"], 75);
        assert_eq!(v["level"], 5);
        assert_eq!(v["damage"], Value::Null);
    }

    #[tokio::test]
    async fn bad_input() {
        assert_eq!(
            get("/api/spells/1/effect?level=-1").await.0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            get("/api/spells/1/effect?level=40000").await.0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(get("/api/spells/999/effect").await.0, StatusCode::NOT_FOUND);
    }
}

mod max_damage {
    use super::*;

    #[tokio::test]
    async fn weapon_and_drop_bonuses_raise_each_attack() {
        // GetMaxDamage by hand: one 500-energy attack, 1000 energy a round. The least energy
        // used grows to 1000, the remainder is 0 and lifts to 499, so the round holds 1499
        // energy: two attacks. Each hits for AttMax + weapon bonus 3 + drop bonus 4 x 50% = 15.
        let (st, v) = get("/api/monsters/1/max-damage").await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["max_damage"], 30);
        assert_eq!(v["energy_per_round"], 1000);
    }

    #[tokio::test]
    async fn spell_attack_uses_the_spell_and_between_round_spells_add_to_it() {
        // Attack: spell 1 at level 30 for a monster is 4 to 12 (fixtures/rules/spells.json
        // maxx-1-2), one cast of 1000 energy, and 1499 energy holds just one. Between rounds:
        // spell 20 does a flat 7 and lasts 3 rounds, so GetMaxDamage adds its 7 once for
        // the duration above 1 and again for the duration above 2.
        let (_, v) = get("/api/monsters/2/max-damage").await;
        assert_eq!(v["max_damage"], 12 + 14);
    }

    #[tokio::test]
    async fn a_lasting_hit_spell_adds_its_ticks() {
        // Spell 20 lasts 3 rounds, so the swing (6) does not carry it and the duration ticks
        // add its 7 twice. One 1000-energy swing fits the round.
        let (_, v) = get("/api/monsters/3/max-damage").await;
        assert_eq!(v["max_damage"], 6 + 7 + 7);
    }

    #[tokio::test]
    async fn unknown_monster() {
        assert_eq!(
            get("/api/monsters/99/max-damage").await.0,
            StatusCode::NOT_FOUND
        );
    }
}
