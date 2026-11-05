use touchstone_model::{
    AbilitySlot, DropSlot, Info, Item, MidSpell, ModelError, Monster, MonsterAttack, Room,
    ShopSlot, World, WorldParts,
};

fn item(number: i32) -> Item {
    Item {
        number,
        name: format!("item{number}"),
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
        weapon_type: 2,
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
        abilities: [AbilitySlot::default(); 20],
        in_game: true,
        obtained_from: None,
        references: None,
    }
}

fn room(map_number: i32, room_number: i32) -> Room {
    let none = || "0".to_string();
    Room {
        map_number,
        room_number,
        name: None,
        light: 0,
        shop: 0,
        npc: 0,
        cmd: 0,
        spell: 0,
        lair: None,
        delay: 5,
        placed: None,
        n: none(),
        s: none(),
        e: none(),
        w: none(),
        ne: none(),
        nw: none(),
        se: none(),
        sw: none(),
        u: none(),
        d: none(),
    }
}

fn parts() -> WorldParts {
    let _ = (
        ShopSlot::default(),
        DropSlot::default(),
        MidSpell::default(),
    );
    let _: Option<(Monster, MonsterAttack)> = None;
    WorldParts {
        info: Info {
            nmr_version: "v1".into(),
            dat_file_version: "v1".into(),
            date: String::new(),
            time: String::new(),
            custom: String::new(),
            legit: true,
            disable_kai: false,
            update_url: String::new(),
        },
        items: vec![item(9), item(2047)],
        monsters: vec![],
        spells: vec![],
        rooms: vec![room(1, 1), room(2, 1)],
        shops: vec![],
        classes: vec![],
        races: vec![],
        lairs: vec![],
        tbinfo: vec![],
    }
}

#[test]
fn lookups_use_record_numbers_not_positions() {
    let w = World::new(parts()).unwrap();
    assert_eq!(w.get_item(2047).unwrap().number, 2047);
    assert!(w.get_item(0).is_none());
    assert_eq!(w.get_room(2, 1).unwrap().map_number, 2);
    assert!(w.get_room(1, 2).is_none());
    assert_eq!(
        w.get_item(9).unwrap().item_type_enum().unwrap().label(),
        "Weapon"
    );
}

#[test]
fn duplicate_keys_are_rejected() {
    let mut p = parts();
    p.items.push(item(9));
    assert_eq!(
        World::new(p).unwrap_err(),
        ModelError::DuplicateKey {
            table: "Items",
            key: "9".into()
        }
    );
}

#[test]
fn world_serializes_without_its_index() {
    let w = World::new(parts()).unwrap();
    let v = serde_json::to_value(&w).unwrap();
    assert!(v.get("index").is_none());
    assert_eq!(v["items"].as_array().unwrap().len(), 2);
}
