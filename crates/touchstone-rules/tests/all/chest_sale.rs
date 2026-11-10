// Tests for the Chests tab's sale values (card CH-001). There is no MME number to match: the chances
// come from `chest_contents` (fixtures/rules/chests.json pins them) and the sell prices from
// `get_item_value` (fixtures/rules/item_value.json); the average is touchstone's own, worked out by
// hand in each test's comment.
use touchstone_model::{AbilitySlot, Info, Item, Spell, TbInfo, World, WorldParts};
use touchstone_rules::Profile;
use touchstone_rules::chests::{chest_detail, chest_summary};
use touchstone_rules::locations::{LocationLairs, LocationOptions};

struct NoLairs;
impl LocationLairs for NoLairs {
    fn get_lair_info(&mut self, _: &str, _: i16) -> touchstone_rules::lairs::LairInfo {
        touchstone_rules::lairs::LairInfo::default()
    }
}

fn item(number: i32, name: &str, item_type: i32, price: i32, currency: i32, from: &str) -> Item {
    Item {
        number,
        name: name.to_string(),
        limit: 0,
        encum: 0,
        item_type,
        use_count: 0,
        price,
        currency,
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
        abilities: [AbilitySlot::default(); 20],
        in_game: true,
        obtained_from: (!from.is_empty()).then(|| from.to_string()),
        references: None,
    }
}

fn chest(number: i32, spell: i32, from: &str) -> Item {
    let mut c = item(number, "test chest", 8, 0, 0, from);
    c.abilities[0] = AbilitySlot {
        ability: 43,
        value: spell,
    };
    c
}

fn spell(number: i32, tb: i32) -> Spell {
    let mut abilities = [AbilitySlot::default(); 10];
    abilities[0] = AbilitySlot {
        ability: 148,
        value: tb,
    };
    Spell {
        number,
        name: None,
        short: None,
        req_level: 0,
        energy_cost: 0,
        mana_cost: 0,
        min_base: 0,
        max_base: 0,
        diff: 0,
        type_of_resists: 0,
        targets: 0,
        dur: 0,
        att_type: 0,
        magery: 0,
        magery_lvl: 0,
        cap: 0,
        max_inc_lvls: 0,
        max_inc: 0,
        min_inc_lvls: 0,
        min_inc: 0,
        dur_inc_lvls: 0,
        dur_inc: 0,
        abilities,
        learnable: false,
        learned_from: None,
        casted_by: None,
        classes: None,
    }
}

fn world() -> World {
    // The chest's text hands the draw to textblock 10, which gives item 101 with chance 25% and item 102
    // with the remaining 75% (the lines hold cumulative percentages: 25, then 100); 103 is added at 0%.
    let tb = |n: i32, text: &str| TbInfo {
        number: n,
        link_to: 0,
        action: Some(text.to_string()),
        called_from: String::new(),
    };
    World::new(WorldParts {
        info: Info {
            nmr_version: String::new(),
            dat_file_version: String::new(),
            content_version: None,
            date: String::new(),
            time: String::new(),
            custom: String::new(),
            legit: false,
            disable_kai: false,
            update_url: String::new(),
        },
        items: vec![
            // 200 gold, bought by shop 1
            item(101, "gem a", 0, 200, 2, "Shop #1, Item #900(25%)"),
            // 10 silver, bought by shop 1
            item(102, "trinket b", 0, 10, 1, "Shop(sell) #1, Item #900(75%)"),
            // worth 1000 platinum but no shop takes it
            item(103, "relic c", 0, 1000, 3, "Item #900"),
            // a shop takes it, but it has no value
            item(104, "freebie d", 0, 0, 0, "Shop #1"),
            chest(
                900,
                800,
                "Monster #5(50%), Room 1/2, Monster #6(10%), Textblock #7(5%)",
            ),
            // nothing in its text gives an item
            chest(901, 801, "NPC #3"),
            // not a chest
            item(500, "sword", 1, 5, 2, ""),
        ],
        monsters: Vec::new(),
        spells: vec![spell(800, 20), spell(801, 21)],
        rooms: Vec::new(),
        shops: Vec::new(),
        classes: Vec::new(),
        races: Vec::new(),
        lairs: Vec::new(),
        tbinfo: vec![
            tb(20, "random 10\n"),
            tb(10, "25:giveitem 101\n100:giveitem 102\n"),
            tb(21, "message 1\n"),
        ],
    })
    .unwrap()
}

#[test]
fn average_is_chance_times_sell_price_summed() {
    let w = world();
    // charm 0: a shop pays the full price. 101 = 200 gold = 20,000 copper at 0.25, 102 = 10 silver =
    // 100 copper at 0.75: 5,000 + 75 = 5,075.
    let s = chest_summary(&w, Profile::Stock, 0).unwrap();
    let c = s.iter().find(|r| r.item == 900).unwrap();
    assert!((c.average_copper - 5075.0).abs() < 1e-9, "{c:?}");
    // charm 50: the sell modifier is fix(50 / 2) + 25 = 50 percent, so half of that: 2,537.5.
    let s = chest_summary(&w, Profile::Stock, 50).unwrap();
    let c = s.iter().find(|r| r.item == 900).unwrap();
    assert!((c.average_copper - 2537.5).abs() < 1e-9, "{c:?}");
    assert_eq!(c.contents, 2);
    assert_eq!(c.name, "test chest");
}

#[test]
fn summary_lists_chests_only_and_counts_monsters() {
    let w = world();
    let s = chest_summary(&w, Profile::Stock, 50).unwrap();
    assert_eq!(s.iter().map(|r| r.item).collect::<Vec<_>>(), [900, 901]);
    // Monster #5 and Monster #6; the room and the textblock are other sources
    assert_eq!(s[0].dropped_by, 2);
    // a chest that gives nothing: no lines, an average of 0 (not -0), and an NPC is not a monster
    assert_eq!(s[1].contents, 0);
    assert_eq!(s[1].average_copper.to_bits(), 0.0f64.to_bits());
    assert_eq!(s[1].dropped_by, 0);
}

#[test]
fn detail_prices_each_line_and_flags_what_cannot_be_sold() {
    let w = world();
    let opt = LocationOptions {
        nmr_ver: 1.8,
        hide_record_numbers: false,
        global_filter: false,
        charm_tag: String::new(),
        attack_type: 0,
    };
    let d = chest_detail(&w, Profile::Stock, &mut NoLairs, &opt, 900, 50)
        .unwrap()
        .unwrap();
    // best chance first
    let lines: Vec<_> = d
        .contents
        .iter()
        .map(|c| {
            (
                c.item,
                c.percent_text.as_str(),
                c.sale.sellable,
                c.sale.copper,
            )
        })
        .collect();
    assert_eq!(
        lines,
        [
            (102, "75%", true, 50.0),    // 100 copper at 50 percent
            (101, "25%", true, 10000.0), // 20,000 copper at 50 percent
        ]
    );
    assert!((d.contents[0].expected_copper - 37.5).abs() < 1e-9);
    assert!((d.contents[1].expected_copper - 2500.0).abs() < 1e-9);
    assert_eq!(d.contents[1].sale.short, "100G");
    // the droppers are the Items pane's reference rows in the order GetLocations adds them (rooms, then
    // monsters, then textblocks); the server sorts them by chance
    let kinds: Vec<_> = d
        .droppers
        .rows
        .iter()
        .map(|r| format!("{:?}", r.kind))
        .collect();
    assert_eq!(kinds, ["Room", "Monster", "Monster", "Textblock"]);
}

#[test]
fn unsellable_items_count_as_nothing() {
    let mut w = world();
    // give chest 901 a text that rolls 103 (no shop buys it) and 104 (no value) at 50% each
    let mut parts = WorldParts {
        info: w.info.clone(),
        items: std::mem::take(&mut w.items),
        monsters: Vec::new(),
        spells: std::mem::take(&mut w.spells),
        rooms: Vec::new(),
        shops: Vec::new(),
        classes: Vec::new(),
        races: Vec::new(),
        lairs: Vec::new(),
        tbinfo: std::mem::take(&mut w.tbinfo),
    };
    parts.tbinfo.retain(|t| t.number != 21);
    parts.tbinfo.push(TbInfo {
        number: 21,
        link_to: 0,
        action: Some("random 11\n".into()),
        called_from: String::new(),
    });
    parts.tbinfo.push(TbInfo {
        number: 11,
        link_to: 0,
        action: Some("50:giveitem 103\n100:giveitem 104\n".into()),
        called_from: String::new(),
    });
    let w = World::new(parts).unwrap();
    let s = chest_summary(&w, Profile::Stock, 50).unwrap();
    let c = s.iter().find(|r| r.item == 901).unwrap();
    assert_eq!(c.contents, 2);
    assert_eq!(c.average_copper, 0.0);
}
