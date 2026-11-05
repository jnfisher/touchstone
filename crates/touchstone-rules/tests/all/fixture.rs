//! Loads the oracle's fixtures (`fixtures/rules/<family>.json`, written by
//! `tools/oracle/oracle.py gen`). The format is documented in `tools/oracle/README.md`.

#![allow(dead_code)] // each family's tests use a different subset

use serde_json::Value;
use std::path::PathBuf;

pub struct Fixture {
    pub header: Value,
    pub cases: Vec<Value>,
}

/// Reads a fixture family. Panics with the path if it is missing or malformed: a missing
/// fixture is a test failure, never a skip.
pub fn load(family: &str) -> Fixture {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/rules")
        .join(format!("{family}.json"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let mut doc: Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parsing {}: {e}", path.display()));
    let header = doc["header"].take();
    let cases = match doc["cases"].take() {
        Value::Array(cases) => cases,
        _ => panic!("{} has no cases array", path.display()),
    };
    assert!(!cases.is_empty(), "{} has no cases", path.display());
    Fixture { header, cases }
}

/// A Currency value as the oracle writes it: `{"cur": units of 1/10000, "s": display}`.
pub fn cur(v: &Value) -> i64 {
    v["cur"]
        .as_i64()
        .unwrap_or_else(|| panic!("not a Currency value: {v}"))
}

/// A Single value as the oracle writes it: `{"single": x, "s": shortest decimal}`.
pub fn single(v: &Value) -> f32 {
    v["single"]
        .as_f64()
        .unwrap_or_else(|| panic!("not a Single value: {v}")) as f32
}
