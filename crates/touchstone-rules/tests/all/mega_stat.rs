// Fixture tests for the mega_stat family: see docs/porting.md step 5.
// Expected values come from fixtures/rules/mega_stat.json (frmSwingCalc's own code run by the oracle).

use crate::fixture;
use serde_json::Value;
use touchstone_rules::mega_stat::*;

fn arg_str(case: &Value, n: usize) -> String {
    let args = case["inputs"]["proc"]
        .as_str()
        .map_or(
            &case["inputs"]["steps"][0]["args"],
            |_| &case["inputs"]["args"],
        );
    args[n].as_str().expect("string arg").to_string()
}

fn run(case: &Value) -> Result<(), String> {
    let inputs = &case["inputs"];
    let out = &case["outputs"];
    let want_err = case["error"]["number"].as_i64();
    if let Some(proc) = inputs["proc"].as_str() {
        let a = arg_str(case, 0);
        let want = &out["result"];
        let got: Value = match proc {
            "FirstMegaToken" => first_mega_token(&a).into(),
            "LastMegaToken" => last_mega_token(&a).into(),
            "IsMegaPlaceholder" => is_mega_placeholder(&a).into(),
            "IsMegaWholeNumberToken" => is_mega_whole_number_token(&a).into(),
            "CleanMegaNumber" => clean_mega_number(&a).into(),
            "GetMegaStatLine" => get_mega_stat_line(&a, &arg_str(case, 1)).into(),
            other => return Err(format!("unknown proc {other}")),
        };
        return (&got == want)
            .then_some(())
            .ok_or_else(|| format!("got {got}, want {want}"));
    }
    let step = &inputs["steps"][0];
    match step["call"].as_str().unwrap() {
        "ParseMegaStatLine" => {
            let got = parse_mega_stat_line(&arg_str(case, 0), &arg_str(case, 1));
            match (got, want_err) {
                (Err(e), Some(n)) if i64::from(e.code()) == n => Ok(()),
                (Err(e), w) => Err(format!("error {e:?}, want {w:?}")),
                (Ok(_), Some(n)) => Err(format!("no error, want {n}")),
                (Ok(g), None) => {
                    let refs = &out["p.refs"];
                    if out["p"].as_bool() == Some(g.ok)
                        && refs[0].as_f64() == Some(g.pct)
                        && refs[1].as_i64() == Some(i64::from(g.avg))
                    {
                        Ok(())
                    } else {
                        Err(format!("got {g:?}, want {} {refs}", out["p"]))
                    }
                }
            }
        }
        "cmdPasteMega_Click" => {
            let clip = inputs["forms"]["frmMain"]["txtClip"]["Text"]
                .as_str()
                .unwrap();
            let handled = case["events"]
                .as_array()
                .and_then(|a| a.iter().find(|e| e["kind"] == "handled_error"))
                .map(|e| e["number"].as_i64().unwrap());
            let incomplete = case["events"]
                .as_array()
                .is_some_and(|a| a.iter().any(|e| e["kind"] == "msgbox"));
            let want_box: Vec<&str> = out["box"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap())
                .collect();
            match paste_mega_stat_line(clip) {
                Err(e) => {
                    if handled == Some(i64::from(e.code())) && want_box.iter().all(|s| s.is_empty())
                    {
                        Ok(())
                    } else {
                        Err(format!(
                            "error {e:?}, want handled {handled:?} {want_box:?}"
                        ))
                    }
                }
                Ok(None) => {
                    if incomplete && handled.is_none() && want_box.iter().all(|s| s.is_empty()) {
                        Ok(())
                    } else {
                        Err(format!("none, want {want_box:?} incomplete={incomplete}"))
                    }
                }
                Ok(Some(b)) => {
                    if !incomplete
                        && handled.is_none()
                        && b.iter().map(String::as_str).eq(want_box.iter().copied())
                    {
                        Ok(())
                    } else {
                        Err(format!(
                            "got {b:?}, want {want_box:?} incomplete={incomplete}"
                        ))
                    }
                }
            }
        }
        other => Err(format!("unknown call {other}")),
    }
}

#[test]
fn mega_stat_fixture() {
    let fx = fixture::load("mega_stat");
    let mut failures = Vec::new();
    for case in &fx.cases {
        if let Err(msg) = run(case) {
            failures.push(format!(
                "{}: {msg}\n  inputs {}",
                case["id"], case["inputs"]
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases failed; first:\n{}",
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
