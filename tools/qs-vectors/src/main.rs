//! Writes `fixtures/quicksilver/<family>.json` from Quicksilver's own rule functions.
//!
//! Holds no Quicksilver code: every number in a case is the answer of a public function of the
//! linked crates. Deterministic: fixed input grids, cases sorted by id, keys sorted, no clock.
//! See docs/quicksilver-profile.md part 4 and tasks/Q-005-qs-vectors.md.

mod backstab;
mod cells;
mod energy;
mod exphr_d;
mod knees;
mod melee;
mod opener;
mod realm;
mod regen;
mod spells;
mod training;
mod weave;
mod weave_round;

use std::path::Path;

use serde_json::{Value, json};

/// One fixture file: its family name, the keys it exercises and its cases.
pub struct Family {
    pub name: &'static str,
    /// What the cases are and how to read them (units, the shape of `inputs`/`v120`/`neutral`).
    pub about: &'static str,
    /// Which `[balance]` keys differ between the `v120` and `neutral` columns, with both values.
    pub keys: Value,
    pub cases: Vec<Value>,
}

fn render(fam: &Family, commit: &str, content_version: &str) -> String {
    let mut cases = fam.cases.clone();
    cases.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    let header = json!({
        "about": fam.about,
        "balance_keys": fam.keys,
        "case_count": cases.len(),
        "content_version": content_version,
        "family": fam.name,
        "generator": "tools/qs-vectors",
        "quicksilver_commit": commit,
    });
    let mut out = String::from("{\n\"header\": ");
    out.push_str(&header.to_string());
    out.push_str(",\n\"cases\": [\n");
    for (i, c) in cases.iter().enumerate() {
        out.push_str(&c.to_string());
        out.push_str(if i + 1 < cases.len() { ",\n" } else { "\n" });
    }
    out.push_str("]\n}\n");
    out
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().expect("usage: qs-vectors <fixtures/quicksilver> [--check]");
    let check = args.next().as_deref() == Some("--check");
    let commit = std::env::var("QS_COMMIT").expect("QS_COMMIT (set by run.sh)");
    let version = std::env::var("QS_CONTENT_VERSION").expect("QS_CONTENT_VERSION (set by run.sh)");

    let mut stale = Vec::new();
    for fam in [
        backstab::family(),
        cells::family(),
        energy::family(),
        exphr_d::family(),
        knees::family(),
        melee::family(),
        opener::family(),
        regen::family(),
        spells::family(),
        training::family(),
        weave::family(),
        weave_round::family(),
    ] {
        let text = render(&fam, &commit, &version);
        let path = Path::new(&dir).join(format!("{}.json", fam.name));
        if check {
            if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
                stale.push(path.display().to_string());
            }
        } else {
            std::fs::create_dir_all(&dir).expect("create output dir");
            std::fs::write(&path, &text).expect("write fixture");
            println!("{}: {} cases, {} bytes", path.display(), fam.cases.len(), text.len());
        }
    }
    // the realm characters are SQL, not a JSON family
    let text = realm::sql();
    let path = Path::new(&dir).join("realm_characters.sql");
    if check {
        if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
            stale.push(path.display().to_string());
        }
    } else {
        std::fs::write(&path, &text).expect("write fixture");
        println!("{}: {} bytes", path.display(), text.len());
    }
    if !stale.is_empty() {
        eprintln!("out of date (rerun tools/qs-vectors/run.sh): {}", stale.join(", "));
        std::process::exit(1);
    }
}
