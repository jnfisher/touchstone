// Fixture tests for the game_text family: see docs/porting.md step 5.
// Expected values come from fixtures/rules/game_text.json (the oracle running modItemParse).

use crate::fixture;
use serde_json::Value;
use touchstone_rules::game_text::{
    ItemParseResult, item_name_in_parse_result, parse_game_text_inventory, parse_name_and_qty,
};

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap_or_else(|| panic!("not a list: {v}"))
        .iter()
        .map(|s| s.as_str().expect("string").to_string())
        .collect()
}

fn id(case: &Value) -> &str {
    case["id"].as_str().expect("case id")
}

#[test]
fn game_text_fixture() {
    let fx = fixture::load("game_text");
    let mut failures = Vec::new();
    let (mut parsed, mut pnq, mut ipr) = (0, 0, 0);
    for case in &fx.cases {
        let outputs = &case["outputs"];
        match case["inputs"]["proc"].as_str() {
            Some("ParseGameTextInventory") => {
                parsed += 1;
                let text = case["inputs"]["args"][0].as_str().expect("text");
                let got = parse_game_text_inventory(text);
                let want = &outputs["result"];
                let want_events: Vec<(String, u64)> = case["events"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|e| {
                                assert_eq!(e["kind"], "handled_error");
                                (
                                    e["source"].as_str().unwrap().to_string(),
                                    e["number"].as_u64().unwrap(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let got_events: Vec<(String, u64)> = got
                    .handled
                    .iter()
                    .map(|h| (h.source.to_string(), u64::from(h.number)))
                    .collect();
                if strings(&want["sequipped"]) != got.items.equipped
                    || strings(&want["sinventory"]) != got.items.inventory
                    || strings(&want["skeys"]) != got.items.keys
                    || strings(&want["sground"]) != got.items.ground
                    || want_events != got_events
                {
                    failures.push(format!(
                        "{}: {text:?}\n  want {want} {want_events:?}\n  got  {:?}",
                        id(case),
                        got
                    ));
                }
            }
            Some("ItemNameInParseResult") => {
                ipr += 1;
                let args = case["inputs"]["args"].as_array().unwrap();
                let f = &args[0]["fields"];
                let list = |k: &str| f.get(k).map(strings).unwrap_or_default();
                let items = ItemParseResult {
                    equipped: list("sEquipped"),
                    inventory: list("sInventory"),
                    keys: list("sKeys"),
                    ground: list("sGround"),
                };
                let flag = |n: usize| args.get(n).is_none_or(|v| v.as_bool().unwrap());
                let got = item_name_in_parse_result(
                    &items,
                    args[1].as_str().unwrap(),
                    flag(2),
                    flag(3),
                    flag(4),
                );
                if outputs["result"].as_bool() != Some(got) {
                    failures.push(format!(
                        "{}: {args:?} want {} got {got}",
                        id(case),
                        outputs["result"]
                    ));
                }
            }
            None => {
                pnq += 1;
                let step = &case["inputs"]["steps"][0];
                assert_eq!(step["call"], "ParseNameAndQty");
                let got = parse_name_and_qty(step["args"][0].as_str().unwrap());
                let ok = match (case.get("error"), &got) {
                    (Some(e), Err(g)) => e["number"].as_u64() == Some(u64::from(g.code())),
                    (None, Ok((name, qty))) => {
                        let want = &outputs["pnq.refs"];
                        want[0].as_str() == Some(name.as_str())
                            && want[1].as_i64() == Some(i64::from(*qty))
                    }
                    _ => false,
                };
                if !ok {
                    failures.push(format!(
                        "{}: {step} want {outputs} err {:?} got {got:?}",
                        id(case),
                        case.get("error")
                    ));
                }
            }
            Some(other) => panic!("unexpected procedure {other}"),
        }
    }
    assert!(
        parsed > 500 && pnq > 40 && ipr > 100,
        "fixture shrank: {parsed} {pnq} {ipr}"
    );
    assert!(
        failures.is_empty(),
        "{} of {} cases differ; first ones:\n{}",
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
