//! Reading an Obtained From line. The entry forms are NMR's, as the stock database and the
//! silvermere derivation write them (`docs/data-gaps.md`, `obtained_from`).

use touchstone_rules::acquire::{Entry, parse};

#[test]
fn every_entry_form_is_read() {
    let line = "Room 1/2223, Shop #9, Shop(nogen) #167, Shop(sell) #88, Monster #372(10%), \
                Textblock #9123(1%), Textblock #181, Item #907(5.9%), NPC #38";
    assert_eq!(
        parse(line),
        [
            Entry::Room { map: 1, room: 2223 },
            Entry::Shop {
                number: 9,
                restocks: true
            },
            Entry::Shop {
                number: 167,
                restocks: false
            },
            Entry::ShopBuys { number: 88 },
            Entry::Monster {
                number: 372,
                pct: 10.0
            },
            Entry::Script {
                number: 9123,
                pct: Some(1.0)
            },
            Entry::Script {
                number: 181,
                pct: None
            },
            Entry::Chest {
                item: 907,
                pct: 5.9
            },
            Entry::Npc { number: 38 },
        ]
    );
}

#[test]
fn a_cut_off_or_unknown_entry_is_left_out() {
    // the column holds 1,996 characters; the last entry is then cut and ends in "+"
    assert_eq!(
        parse("Shop #9, Room 2/186+, Roo+, R+, Monster #5, Wizard #3, "),
        [Entry::Shop {
            number: 9,
            restocks: true
        }]
    );
    assert_eq!(parse(""), []);
}
