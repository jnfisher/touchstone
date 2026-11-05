// Fixture tests for the item_value family: see docs/porting.md step 5.
use serde_json::Value;
use touchstone_model::{AbilitySlot, Item, Shop, ShopSlot};
use touchstone_rules::Profile;
use touchstone_rules::item_value::{
    ItemValue, get_item_copper_cost, get_item_shop_regen_pct, get_item_value, get_shop_markup,
};

use crate::fixture::{cur, load};

fn int(v: &Value, what: &str) -> i32 {
    v.as_i64()
        .unwrap_or_else(|| panic!("{what} not an integer: {v}")) as i32
}

/// Builds the item MME's seek found from the price and currency the fixture carries.
fn item(number: i32, row: &Value) -> Item {
    Item {
        number,
        name: String::new(),
        limit: 0,
        encum: 0,
        item_type: 0,
        use_count: 0,
        price: int(&row["price"], "price"),
        currency: int(&row["currency"], "currency"),
        min: 0,
        max: 0,
        armour_class: 0,
        damage_resist: 0,
        weapon_type: 0,
        armour_type: 0,
        worn: 0,
        accy: 0,
        gettable: false,
        str_req: 0,
        speed: 0,
        not_droppable: false,
        destroy_on_death: false,
        retain_after_uses: false,
        class_rest: [0; 10],
        race_rest: [0; 10],
        negate_spell: [0; 10],
        abilities: [AbilitySlot::default(); 20],
        in_game: false,
        obtained_from: None,
        references: None,
    }
}

fn shop(number: i32, row: &Value) -> Shop {
    let mut stock = [ShopSlot::default(); 20];
    if let Some(slots) = row["stock"].as_array() {
        for (slot, s) in stock.iter_mut().zip(slots) {
            *slot = ShopSlot {
                item: int(&s[0], "item"),
                max: int(&s[1], "max"),
                time: int(&s[2], "time"),
                amount: int(&s[3], "amount"),
                pct: int(&s[4], "pct"),
            };
        }
    }
    Shop {
        number,
        name: String::new(),
        shop_type: row["type"].as_i64().unwrap_or(0) as i32,
        min_lvl: 0,
        max_lvl: 0,
        markup_pct: int(&row["markup"], "markup"),
        class_rest: 0,
        stock,
        in_game: false,
        assigned_to: None,
    }
}

fn check_value(got: &ItemValue, want: &Value) -> Result<(), String> {
    let s = |k: &str| want[k].as_str().unwrap_or_default().to_string();
    let f = |k: &str| want[k].as_f64().unwrap_or(f64::NAN);
    let checks: [(&str, bool); 10] = [
        (
            "base_cost",
            i64::from(got.base_cost) == want["nbasecost"].as_i64().unwrap_or(-1),
        ),
        (
            "base_coin",
            i64::from(got.base_coin) == want["nbasecoin"].as_i64().unwrap_or(-1),
        ),
        ("copper_buy", got.copper_buy == f("ncopperbuy")),
        ("copper_sell", got.copper_sell == f("ncoppersell")),
        ("friendly_buy", got.friendly_buy == s("sfriendlybuy")),
        (
            "friendly_buy_short",
            got.friendly_buy_short == s("sfriendlybuyshort"),
        ),
        ("friendly_sell", got.friendly_sell == s("sfriendlysell")),
        (
            "friendly_sell_short",
            got.friendly_sell_short == s("sfriendlysellshort"),
        ),
        (
            "markup",
            i64::from(got.markup) == want["nmarkup"].as_i64().unwrap_or(-1),
        ),
        (
            "shop_number",
            i64::from(got.shop_number) == want["nshopnumber"].as_i64().unwrap_or(-1),
        ),
    ];
    let bad: Vec<&str> = checks
        .iter()
        .filter(|(_, ok)| !ok)
        .map(|(n, _)| *n)
        .collect();
    if bad.is_empty() {
        Ok(())
    } else {
        Err(format!("fields {bad:?}: got {got:?}"))
    }
}

#[test]
fn every_case_matches_the_oracle() {
    let fx = load("item_value");
    assert_eq!(fx.header["family"], "item_value");
    let mut failures = Vec::new();
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap_or("?");
        let inputs = &case["inputs"];
        let args = &inputs["args"];
        let rows = &inputs["rows"];
        let want = &case["outputs"]["result"];
        let a = |i: usize| int(&args[i], "arg");
        let result: Result<(), String> = match inputs["proc"].as_str().unwrap_or_default() {
            // GetShopMarkup(nNum): oracle result is the Integer.
            "GetShopMarkup" => {
                let n = a(0);
                let s = rows.get("shop").map(|r| shop(n, r));
                let got = get_shop_markup(n, s.as_ref());
                if i64::from(got) == want.as_i64().unwrap_or(-1) {
                    Ok(())
                } else {
                    Err(format!("got {got}"))
                }
            }
            // GetItemShopRegenPCT(nShopNum, nItemNum): oracle result is a Currency.
            "GetItemShopRegenPCT" => {
                let (sn, i) = (a(0), a(1));
                let s = rows.get("shop").map(|r| shop(sn, r));
                match get_item_shop_regen_pct(Profile::Stock, sn, i, s.as_ref()) {
                    Ok(got) if got.scaled() == cur(want) => Ok(()),
                    other => Err(format!("got {other:?}")),
                }
            }
            "GetItemCopperCost" => {
                let n = a(0);
                let it = rows.get("item").map(|r| item(n, r));
                let got = get_item_copper_cost(n, it.as_ref());
                if got == want.as_f64().unwrap_or(f64::NAN) {
                    Ok(())
                } else {
                    Err(format!("got {got}"))
                }
            }
            "GetItemValue" => {
                let n = a(0);
                let opt = |i: usize| args.get(i).map_or(0, |v| int(v, "arg"));
                let it = rows.get("item").map(|r| item(n, r));
                let sn = opt(3);
                let s = rows.get("shop").map(|r| shop(sn, r));
                let no_buy = args.get(4).and_then(Value::as_bool).unwrap_or(false);
                match get_item_value(
                    Profile::Stock,
                    n,
                    it.as_ref(),
                    opt(1) as i16,
                    opt(2) as i16,
                    sn,
                    s.as_ref(),
                    no_buy,
                ) {
                    Ok(got) => check_value(&got, want),
                    Err(e) => Err(format!("error {e}")),
                }
            }
            other => panic!("unknown procedure {other}"),
        };
        if let Err(e) = result {
            failures.push(format!("{id} {inputs}: {e}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ; first:\n{}",
        failures.len(),
        fx.cases.len(),
        failures
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
