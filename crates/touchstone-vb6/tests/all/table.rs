//! Runs every case of `fixtures/vb6-semantics.json` through the crate.
use serde_json::Value;
use std::collections::BTreeSet;
use touchstone_vb6::*;

fn load() -> Value {
    let path = format!(
        "{}/../../fixtures/vb6-semantics.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(path).expect("fixture present");
    serde_json::from_str(&text).expect("fixture parses")
}

/// Outcome of one op: a JSON value or a VB6 error number.
type Out = Result<Value, u32>;

fn num(v: f64) -> Out {
    Ok(serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number))
}
fn int_out<T: Into<i64>>(v: Result<T, Vb6Error>) -> Out {
    v.map(|x| Value::from(x.into())).map_err(Vb6Error::code)
}
fn f_out(v: Result<f64, Vb6Error>) -> Out {
    v.map_err(Vb6Error::code).and_then(num)
}
fn f(a: &Value, i: usize) -> f64 {
    a[i].as_f64()
        .unwrap_or_else(|| panic!("arg {i} not a number: {a}"))
}
fn i16a(a: &Value, i: usize) -> i16 {
    i16::try_from(a[i].as_i64().expect("integer arg")).expect("Integer-typed arg in range")
}
fn i32a(a: &Value, i: usize) -> i32 {
    i32::try_from(a[i].as_i64().expect("integer arg")).expect("Long-typed arg in range")
}
fn s(a: &Value, i: usize) -> &str {
    a[i].as_str().expect("string arg")
}
fn cur(a: &Value, i: usize) -> Result<Currency, Vb6Error> {
    Currency::from_f64(f(a, i))
}
fn cur_out(v: Result<Currency, Vb6Error>) -> Out {
    v.map_err(Vb6Error::code).and_then(|c| num(c.to_f64()))
}
fn truthy(v: &Value) -> bool {
    v.as_bool()
        .unwrap_or_else(|| v.as_f64().is_some_and(|x| x != 0.0))
}
fn text(v: &Value) -> String {
    match v {
        Value::String(x) => x.clone(),
        Value::Bool(b) => cstr_bool(*b).to_string(),
        other => cstr(other.as_f64().expect("number piece")),
    }
}

fn rnd_script(steps: &Value) -> Out {
    let mut r = Rnd::new();
    let mut out = Vec::new();
    for st in steps.as_array().expect("steps") {
        let name = st[0].as_str().expect("step name");
        out.push(match name {
            "Rnd" if st.get(1).is_some() => Value::from(f64::from(r.next_with(f(st, 1) as f32))),
            "Rnd" => Value::from(f64::from(r.next_value())),
            "Randomize" => {
                r.randomize(f(st, 1));
                Value::Null
            }
            "RandomNumber" => Value::from(r.random_number(i16a(st, 1), i16a(st, 2))),
            other => panic!("unknown step {other}"),
        });
    }
    Ok(Value::Array(out))
}

fn run(op: &str, a: &Value) -> Out {
    let cmp = |r: bool| Ok(Value::from(vbool(r)));
    match op {
        "CInt" | "AssignInteger" => int_out(cint(f(a, 0))),
        "CLng" | "AssignLong" => int_out(clng(f(a, 0))),
        "CSng" => csng(f(a, 0))
            .map_err(Vb6Error::code)
            .and_then(|x| num(f64::from(x))),
        "CCur" => cur_out(cur(a, 0)),
        "CBool" => Ok(Value::from(cbool(if let Some(b) = a[0].as_bool() {
            f64::from(u8::from(b))
        } else {
            f(a, 0)
        }))),
        "Int" => num(int(f(a, 0))),
        "Fix" => num(fix(f(a, 0))),
        "Abs" => num(abs(f(a, 0))),
        "Sgn" => Ok(Value::from(sgn(f(a, 0)))),
        "Sqr" => f_out(sqr(f(a, 0))),
        "Log" => f_out(log(f(a, 0))),
        "Exp" => f_out(exp(f(a, 0))),
        "Round" => f_out(round(
            f(a, 0),
            if a.get(1).is_some() {
                clng(f(a, 1)).unwrap()
            } else {
                0
            },
        )),
        "IntDiv" => int_out(idiv(f(a, 0), f(a, 1))),
        "IntDivInt" => int_out(idiv_i16(i16a(a, 0), i16a(a, 1))),
        "Mod" => int_out(vmod(f(a, 0), f(a, 1))),
        "AddInt" => int_out(add_i16(i16a(a, 0), i16a(a, 1))),
        "SubInt" => int_out(sub_i16(i16a(a, 0), i16a(a, 1))),
        "MulInt" => int_out(mul_i16(i16a(a, 0), i16a(a, 1))),
        "AddLng" => int_out(add_i32(i32a(a, 0), i32a(a, 1))),
        "SubLng" => int_out(sub_i32(i32a(a, 0), i32a(a, 1))),
        "MulLng" => int_out(mul_i32(i32a(a, 0), i32a(a, 1))),
        "Div" => f_out(div(f(a, 0), f(a, 1))),
        "Pow" => f_out(pow(f(a, 0), f(a, 1))),
        "Eq" => cmp(f(a, 0) == f(a, 1)),
        "Ne" => cmp(f(a, 0) != f(a, 1)),
        "Lt" => cmp(f(a, 0) < f(a, 1)),
        "Le" => cmp(f(a, 0) <= f(a, 1)),
        "Gt" => cmp(f(a, 0) > f(a, 1)),
        "Ge" => cmp(f(a, 0) >= f(a, 1)),
        "And" => int_out(and(f(a, 0), f(a, 1))),
        "Or" => int_out(or(f(a, 0), f(a, 1))),
        "Xor" => int_out(xor(f(a, 0), f(a, 1))),
        "Not" => int_out(not(f(a, 0))),
        "AddSng" | "MulSng" | "DivSng" => {
            let (x, y) = (csng(f(a, 0)).unwrap(), csng(f(a, 1)).unwrap());
            let r = match op {
                "AddSng" => x + y,
                "MulSng" => x * y,
                _ if y == 0.0 => return Err(Vb6Error::DivisionByZero.code()),
                _ => x / y,
            };
            if r.is_finite() {
                num(f64::from(r))
            } else {
                Err(Vb6Error::Overflow.code())
            }
        }
        "AddCur" => cur_out(cur(a, 0).and_then(|x| x.checked_add(cur(a, 1)?))),
        "MulCur" => cur_out(cur(a, 0).and_then(|x| x.checked_mul(cur(a, 1)?))),
        "DivCur" => cur_out(cur(a, 0).and_then(|x| x.checked_div(cur(a, 1)?))),
        "CStr" => Ok(Value::from(match &a[0] {
            Value::Bool(b) => cstr_bool(*b).to_string(),
            _ => cstr(f(a, 0)),
        })),
        "Str" => Ok(Value::from(str_num(f(a, 0)))),
        "Concat" => Ok(Value::from(
            a.as_array().unwrap().iter().map(text).collect::<String>(),
        )),
        "Val" => num(val(s(a, 0))),
        "IsNumeric" => Ok(Value::from(vbool(is_numeric(s(a, 0))))),
        "CDblStr" => f_out(cdbl_str(s(a, 0))),
        "Left" => left(s(a, 0), i32a(a, 1))
            .map(Value::from)
            .map_err(Vb6Error::code),
        "Right" => right(s(a, 0), i32a(a, 1))
            .map(Value::from)
            .map_err(Vb6Error::code),
        "Mid" => mid(s(a, 0), i32a(a, 1), a.get(2).map(|_| i32a(a, 2)))
            .map(Value::from)
            .map_err(Vb6Error::code),
        "Len" => Ok(Value::from(len(s(a, 0)))),
        "IIf" => Ok(iif(truthy(&a[0]), a[1].clone(), a[2].clone())),
        "Format" => format(f(a, 0), s(a, 1))
            .map(Value::from)
            .map_err(Vb6Error::code),
        "RndScript" => rnd_script(&a[0]),
        other => panic!("op {other} has no handler"),
    }
}

fn same(expected: &Value, got: &Value) -> bool {
    match (expected, got) {
        (Value::Array(e), Value::Array(g)) => {
            e.len() == g.len() && e.iter().zip(g).all(|(x, y)| same(x, y))
        }
        (Value::Number(e), Value::Number(g)) => e.as_f64() == g.as_f64(),
        _ => expected == got,
    }
}

#[test]
fn every_case_in_the_table_matches() {
    let doc = load();
    let cases = doc["cases"].as_array().expect("cases");
    let mut failures = Vec::new();
    for (n, case) in cases.iter().enumerate() {
        let op = case["op"].as_str().expect("op");
        let got = run(op, &case["args"]);
        let ok = match (case.get("error"), &got) {
            (Some(e), Err(g)) => e.as_u64() == Some(u64::from(*g)),
            (None, Ok(v)) => same(&case["result"], v),
            _ => false,
        };
        if !ok {
            failures.push(format!(
                "#{n} {op}{} expected {} {} got {got:?}",
                case["args"],
                case.get("result")
                    .map_or_else(|| "error".to_string(), ToString::to_string),
                case.get("error")
                    .map_or_else(String::new, ToString::to_string)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases failed:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

#[test]
fn table_documents_every_op_it_uses_and_is_substantial() {
    let doc = load();
    let documented: BTreeSet<&str> = doc["ops"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let used: BTreeSet<&str> = doc["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["op"].as_str().unwrap())
        .collect();
    assert_eq!(documented, used);
    assert!(doc["cases"].as_array().unwrap().len() >= 400);
}
