//! The realm module (card R-001): a small set of characters written as SQL inserts, for the tests
//! of touchstone's realm reader, mapping and comparison.
//!
//! The characters come from the balance tool's builder, which runs Quicksilver's own training
//! ladder, can-use checks, secondary-stat and dynamic-stat code (the same builder `dev
//! seed-character` writes into a realm), with the class's standing buffs cast through the rules'
//! own spell code. A few records are then edited by hand the way a sysop could: a worn item
//! touchstone has no slot for, a second item for a slot it holds one in, carried items. Holds no
//! Quicksilver code: every number is a field of the record the engine built.
//!
//! Output: `fixtures/quicksilver/realm_characters.sql`, one transaction of INSERTs. Each
//! character has a stub account (blank password hash and e-mail, never real secrets).

use std::fmt::Write as _;

use quicksilver_balance::character::Cell;
use quicksilver_balance::data::Data;
use quicksilver_balance::kits::Kits;
use quicksilver_balance::sim;
use silvermere_rules::abilities::AbilityLookup as _;
use silvermere_rules::economy::encumbrance_pct;
use silvermere_types::config::Balance;
use silvermere_types::enums::{ItemType, WornLocation};
use silvermere_types::ids::ItemNo;
use silvermere_types::records::Character;

/// Every column of `characters`, in the table's order (migration 0001).
const COLUMNS: &str = include_str!("realm_columns.txt");

/// (first name, race, class, level) of each fixture character, in id order. Races rotate through
/// the ones the tool's kits serve; a race the kit cannot serve falls back to the next.
const SPECS: [(&str, &str, &str, u16); 12] = [
    ("Aldric", "Human", "Warrior", 1),
    ("Brenna", "Elf", "Thief", 9),
    ("Cormac", "Dwarf", "Priest", 16),
    ("Dara", "Human", "Mage", 30),
    ("Eamon", "Nekojin", "Ninja", 44),
    ("Fenwick", "Half-Orc", "Paladin", 60),
    ("Gwen", "Elf", "Bard", 23),
    ("Hale", "Human", "Ranger", 38),
    ("Ilsa", "Dwarf", "Druid", 52),
    ("Joren", "Half-Orc", "Warlock", 12),
    ("Kestrel", "Human", "Mystic", 27),
    ("Lorne", "Dwarf", "Cleric", 34),
];

const RACE_FALLBACK: [&str; 6] = ["Human", "Half-Orc", "Dwarf", "Nekojin", "Elf", "Human"];

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

struct Built {
    c: Character,
}

fn build_all(data: &Data) -> Vec<Built> {
    let kits = Kits::default_kits().expect("the tool's kits");
    let balance = Balance::V120;
    let mut out = Vec::new();
    for (idx, &(first, race, class, level)) in SPECS.iter().enumerate() {
        let mut built = None;
        let start = RACE_FALLBACK.iter().position(|r| *r == race).unwrap_or(0);
        for k in 0..RACE_FALLBACK.len() + 1 {
            let race = if k == 0 {
                race
            } else {
                RACE_FALLBACK[(start + k) % RACE_FALLBACK.len()]
            };
            let cell = Cell {
                race,
                class,
                level,
                kit: None,
            };
            if let Ok(pair) = sim::cell_builds(data, &kits, &cell, &balance) {
                // casters in their casting kit, the rest in their melee kit
                let b = if pair.1.class.magery == silvermere_types::enums::Magery::None {
                    pair.0
                } else {
                    pair.1
                };
                built = Some(b);
                break;
            }
        }
        let mut b = built.unwrap_or_else(|| panic!("no race fits {class} L{level}"));
        edit(data, idx, &mut b);
        // the stored figures again, from the engine's own recalculation, with the edits in
        let b = b.buffed(data, &[], &balance);
        let mut c = b.character.clone();
        c.firstname = first.to_string();
        c.lastname = "Fixture".to_string();
        c.userid = format!("fixture-account-{}", idx + 1);
        player_record(idx, &mut c, &b, data);
        out.push(Built { c });
    }
    out
}

/// Carried items for each character, by item number: stones, scrolls, a torch, and a raft whose
/// stealth penalty the game counts although nothing is worn. Added to the pack after the kit.
const CARRIED: [&[i32]; 12] = [
    &[175, 484, 484],
    &[881, 881],
    &[132, 131],
    &[119, 120, 889],
    &[690, 882],
    &[886, 887, 175],
    &[890],
    &[891, 892, 892],
    &[622],
    &[885, 885],
    &[888],
    &[133, 137],
];

/// The edits a sysop's hand could make, and the pack. Written into the build before the engine
/// recalculates, so the stored figures follow from them.
fn edit(data: &Data, idx: usize, b: &mut quicksilver_balance::character::Build<'_>) {
    let c = &mut b.character;
    let first_free = c.items.iter().position(|i| i.0 == 0).unwrap_or(0);
    for (k, &n) in CARRIED[idx].iter().enumerate() {
        let item = &data.items[&ItemNo(n)];
        c.items[first_free + k] = item.number;
        c.itemuses[first_free + k] = item.charges;
    }
    // the readied light of the first character
    if idx == 0 {
        c.readied = first_free as i8;
    }
    let free_worn = |c: &Character| c.worn.iter().position(|w| w.0 == 0).expect("a free worn slot");
    match idx {
        // a second item for a location that holds one: the game would never wear it, a hand
        // edit can
        2 => {
            let held = c
                .worn
                .iter()
                .filter_map(|w| data.items.get(w))
                .find(|i| i.worn == WornLocation::Head)
                .map(|i| i.number);
            let other = data
                .items
                .values()
                .find(|i| {
                    i.item_type == ItemType::Armour
                        && i.worn == WornLocation::Head
                        && Some(i.number) != held
                })
                .expect("another head item");
            let slot = free_worn(c);
            c.worn[slot] = other.number;
            c.wornuses[slot] = other.charges;
            let pack = c.items.iter().position(|i| i.0 == 0).expect("a free pack slot");
            c.items[pack] = other.number;
            c.itemuses[pack] = other.charges;
        }
        // an item of a type with a wear location that MME has no slot for (a thrown item that
        // has a hands location)
        7 => {
            let odd = data
                .items
                .values()
                .filter(|i| {
                    !matches!(i.item_type, ItemType::Armour | ItemType::Weapon)
                        && i.worn != WornLocation::Nowhere
                })
                .min_by_key(|i| (i.weight, i.number))
                .expect("an item of another type with a wear location");
            let slot = free_worn(c);
            c.worn[slot] = odd.number;
            c.wornuses[slot] = odd.charges;
            let pack = c.items.iter().position(|i| i.0 == 0).expect("a free pack slot");
            c.items[pack] = odd.number;
            c.itemuses[pack] = odd.charges;
        }
        _ => {}
    }
    // the weight of the pack, worn and wielded included (the game counts every item held)
    c.wgt = c
        .items
        .iter()
        .filter_map(|n| data.items.get(n))
        .map(|i| i.weight.max(0) as u32)
        .sum::<u32>()
        .min(u32::from(u16::MAX)) as u16;
}

/// What a player's record has beyond the cell: names, place, evil, gang and the other fields the
/// game keeps for a character that has played.
fn player_record(idx: usize, c: &mut Character, b: &quicksilver_balance::character::Build<'_>, data: &Data) {
    let id = idx as i64 + 1;
    c.map = silvermere_types::ids::MapId(1);
    c.room = silvermere_types::ids::RoomId(1000 + 7 * id as u32);
    c.hunger = 1000;
    c.thirst = 1000;
    c.sex = if idx % 2 == 0 { b'M' } else { b'F' };
    c.lastdate = 1_760_000_000 + 3600 * id;
    c.deathdate = if idx == 5 { 1_759_000_000 } else { 0 };
    c.lives = if idx == 5 { 3 } else { 9 };
    // a purse that fits MME's extra-weight box, and one that does not
    c.coins.gold = if idx == 5 { 4_001 } else { u32::from(c.level) * 10 + id as u32 };
    c.coins.copper = 17 * id as u32;
    c.gang = match idx {
        0 | 2 | 4 => "Ironwood".to_string(),
        7 => "Night Watch".to_string(),
        _ => String::new(),
    };
    c.title = "Adventurer".to_string();
    // evil points: a fiend-bound thief, a good druid
    c.evil = match idx {
        1 => 45,
        8 => -60,
        _ => 0,
    };
    c.cp = (idx as i16) % 3;
    c.totalcp = 100 + 10 * c.level;
    c.autoengage = 1;
    // the pack's weight as a percentage of the capacity, as the engine's own helper has it
    let encumbrance = b.with_lookup(data, |l| l.value(silvermere_types::enums::Ability::Encumbrance));
    let mut maxw = i32::from(c.maxwgt as u16);
    if encumbrance != 0 {
        maxw = maxw * (encumbrance + 100) / 100;
    }
    c.encumpct = encumbrance_pct(c.wgt, c.coins, maxw);
}

pub fn sql() -> String {
    let data = Data::content().expect("the stock records with the 1.20 content");
    let chars = build_all(&data);

    let cols: Vec<&str> = COLUMNS.split_whitespace().collect();
    let mut out = String::new();
    out.push_str(
        "-- Realm characters for the tests of card R-001. Written by tools/qs-vectors (module realm); do not edit.\n\
         -- Characters built by Quicksilver's balance-tool builder, then edited by hand where a comment says so.\n\
         -- Every account is a stub: no password hash, no e-mail.\n",
    );
    out.push_str("BEGIN;\n");
    for (i, b) in chars.iter().enumerate() {
        let id = i as i64 + 1;
        let _ = writeln!(
            out,
            "INSERT INTO accounts (id, username, password_hash, email, created, last_login, flags) VALUES ({id}, {}, '', NULL, '2026-01-01T00:00:00Z', NULL, 0);",
            quote(&b.c.userid)
        );
        let vals: Vec<String> = cols.iter().map(|col| column(id, &b.c, col)).collect();
        let _ = writeln!(
            out,
            "INSERT INTO characters ({}) VALUES ({});",
            cols.join(", "),
            vals.join(", ")
        );
        children(&mut out, id, &b.c);
    }
    out.push_str("COMMIT;\n");
    out
}

fn column(id: i64, c: &Character, col: &str) -> String {
    let n = |v: i64| v.to_string();
    match col {
        "id" | "account_id" => n(id),
        "userid" => quote(&c.userid),
        "firstname" => quote(&c.firstname),
        "lastname" => quote(&c.lastname),
        "title" => quote(&c.title),
        "gang" => quote(&c.gang),
        // no secret in a fixture
        "suicpw" => quote(""),
        "exp" => n(i64::from(c.expbil) * 1_000_000_000 + i64::from(c.exprem)),
        "expbil" => n(i64::from(c.expbil)),
        "exprem" => n(i64::from(c.exprem)),
        "race" => n(i64::from(c.race.0)),
        "class" => n(i64::from(c.class.0)),
        "level" => n(i64::from(c.level)),
        "basint" => n(i64::from(c.basint)),
        "baswil" => n(i64::from(c.baswil)),
        "basstr" => n(i64::from(c.basstr)),
        "bashea" => n(i64::from(c.bashea)),
        "basagi" => n(i64::from(c.basagi)),
        "bascha" => n(i64::from(c.bascha)),
        "intel" => n(i64::from(c.intel)),
        "wil" => n(i64::from(c.wil)),
        "str" => n(i64::from(c.str)),
        "hea" => n(i64::from(c.hea)),
        "agi" => n(i64::from(c.agi)),
        "cha" => n(i64::from(c.cha)),
        "maxhp" => n(i64::from(c.maxhp)),
        "hp" => n(i64::from(c.hp)),
        "maxwgt" => n(i64::from(c.maxwgt)),
        "wgt" => n(i64::from(c.wgt)),
        "maxenergy" => n(i64::from(c.maxenergy)),
        "energy" => n(i64::from(c.energy)),
        "poison" => n(i64::from(c.poison)),
        "maxmr" => n(i64::from(c.maxmr)),
        "mr" => n(i64::from(c.mr)),
        "map" => n(i64::from(c.map.0)),
        "room" => n(i64::from(c.room.0)),
        "light" => n(i64::from(c.light)),
        "hunger" => n(i64::from(c.hunger)),
        "thirst" => n(i64::from(c.thirst)),
        "ninvslots" => n(i64::from(c.ninvslots)),
        "nkeyslots" => n(i64::from(c.nkeyslots)),
        "qflags2" => n(i64::from(c.qflags2)),
        "hpbonus" => n(i64::from(c.hpbonus)),
        "manabonus" => n(i64::from(c.manabonus)),
        "nspellslots" => n(i64::from(c.nspellslots)),
        "evil" => n(i64::from(c.evil)),
        "lawful" => n(i64::from(c.lawful)),
        "verbose_mode" => n(i64::from(c.verbose)),
        "bchannel" => n(i64::from(c.bchannel)),
        "fasttalk" => n(i64::from(c.fasttalk)),
        "hidden" => n(i64::from(c.hidden)),
        "perception" => n(i64::from(c.perception)),
        "stealth" => n(i64::from(c.stealth)),
        "martial" => n(i64::from(c.martial)),
        "thievery" => n(i64::from(c.thievery)),
        "maxmana" => n(i64::from(c.maxmana)),
        "mana" => n(i64::from(c.mana)),
        "spellcast" => n(i64::from(c.spellcast)),
        "traps" => n(i64::from(c.traps)),
        "disarm" => n(i64::from(c.disarm)),
        "picklocks" => n(i64::from(c.picklocks)),
        "tracking" => n(i64::from(c.tracking)),
        "copper" => n(i64::from(c.coins.copper)),
        "silver" => n(i64::from(c.coins.silver)),
        "gold" => n(i64::from(c.coins.gold)),
        "platinum" => n(i64::from(c.coins.platinum)),
        "runic" => n(i64::from(c.coins.runic)),
        "weapon" => n(i64::from(c.weapon.0)),
        "autoengage" => n(i64::from(c.autoengage)),
        "lives" => n(i64::from(c.lives)),
        "rank" => n(i64::from(c.rank)),
        "lighttime" => n(i64::from(c.lighttime)),
        "readied" => n(i64::from(c.readied)),
        "roomlight" => n(i64::from(c.roomlight)),
        "cp" => n(i64::from(c.cp)),
        "movedir" => "-1".to_string(),
        "totalcp" => n(i64::from(c.totalcp)),
        "pagemax" | "gossmax" | "auctmax" => "3".to_string(),
        "paging" => "1".to_string(),
        "encumpct" => n(i64::from(c.encumpct)),
        "avbonus" => n(i64::from(c.avbonus)),
        "acbonus" => n(i64::from(c.acbonus)),
        "dmgbonus" => n(i64::from(c.dmgbonus)),
        "mabase" => n(i64::from(c.mabase)),
        "version" => "22".to_string(),
        "randomhp" => n(i64::from(c.randomhp)),
        "critbonus" => n(i64::from(c.critbonus)),
        "drbonus" => n(i64::from(c.drbonus)),
        "drpct" => n(i64::from(c.drpct)),
        "energyuse" => n(i64::from(c.energyuse)),
        "shield" => n(i64::from(c.shield)),
        "defmod" => n(i64::from(c.defmod)),
        "lastdate" => n(c.lastdate),
        "deathdate" => n(c.deathdate),
        "flags" => n(i64::from(c.flags.bits())),
        "status" => n(i64::from(c.status.bits())),
        "status2" => n(i64::from(c.status2.bits())),
        "settings" => n(i64::from(c.settings.bits())),
        "sex" => n(i64::from(c.sex)),
        "healrate" => n(i64::from(c.healrate)),
        "manarate" => n(i64::from(c.manarate)),
        // everything else (flags, counters, chat and display settings) starts at zero
        _ => "0".to_string(),
    }
}

fn children(out: &mut String, id: i64, c: &Character) {
    for (slot, item) in c.worn.iter().enumerate().filter(|(_, i)| i.0 != 0) {
        let _ = writeln!(
            out,
            "INSERT INTO character_worn (character_id, slot, item, uses) VALUES ({id}, {slot}, {}, {});",
            item.0, c.wornuses[slot]
        );
    }
    for (slot, item) in c.items.iter().enumerate().filter(|(_, i)| i.0 != 0) {
        let _ = writeln!(
            out,
            "INSERT INTO character_inventory (character_id, slot, item, uses) VALUES ({id}, {slot}, {}, {});",
            item.0, c.itemuses[slot]
        );
    }
    for (slot, sp) in c.spellbook.iter().enumerate().filter(|(_, s)| s.0 != 0) {
        let _ = writeln!(
            out,
            "INSERT INTO character_spellbook (character_id, slot, spell) VALUES ({id}, {slot}, {});",
            sp.0
        );
    }
    for (slot, (ability, value)) in c.abils.iter().enumerate().filter(|(_, a)| a.0.code() != 0) {
        let _ = writeln!(
            out,
            "INSERT INTO character_abilities (character_id, slot, ability, value) VALUES ({id}, {slot}, {}, {value});",
            ability.code()
        );
    }
    for (slot, sp) in c.spells.iter().enumerate().filter(|(_, s)| s.0 != 0) {
        let _ = writeln!(
            out,
            "INSERT INTO character_spells_cast (character_id, slot, spell, value, rounds) VALUES ({id}, {slot}, {}, {}, {});",
            sp.0, c.spellvals[slot], c.spellrnds[slot]
        );
    }
}
