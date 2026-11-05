//! Dev-time comparison of the World loaded from the stock `.mdb` (converted to SQLite) with the
//! World loaded from silvermere's own import of the same stock game (T-015). Not part of CI.
//!
//! `cargo run -p touchstone-data --example parity -- MDB_SQLITE SILVERMERE_DB [OUT.md]`
//!
//! Stdout carries counts and example record numbers with both values (for debugging, may show game
//! values). `OUT.md` (default `docs/data-parity.md`) gets counts and record numbers only.
use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde_json::Value;
use touchstone_data::{mdb, silvermere};

/// Fields still not derived (docs/data-gaps.md, status none): reported, not compared.
const NOT_MAPPED: &[(&str, &str)] = &[
    ("items", "in_game"),
    ("items", "references"),
    ("monsters", "avg_dmg"),
    ("monsters", "avg_dmg_phys"),
    ("monsters", "avg_dmg_spell"),
    ("monsters", "avg_lair_exp"),
    ("monsters", "script_value"),
    ("monsters", "in_game"),
    ("lairs", "avg_walk"),
    ("lairs", "avg_dmg"),
    ("lairs", "avg_dmg_phys"),
    ("lairs", "avg_dmg_spell"),
    ("tbinfo", "action"),
];

#[derive(Default)]
struct Stat {
    matched: usize,
    mismatched: usize,
    examples: Vec<(String, Value, Value)>,
}

/// Flatten into `path -> leaf`, with array indexes kept (`attacks[2].acc`).
fn flatten(prefix: &str, v: &Value, out: &mut Vec<(String, Value)>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(&p, x, out);
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                flatten(&format!("{prefix}[{i}]"), x, out);
            }
        }
        _ => out.push((prefix.to_string(), v.clone())),
    }
}

/// `attacks[3].acc` -> `attacks[].acc`, so a field is one report line.
fn generic(path: &str) -> String {
    let mut s = String::new();
    let mut skip = false;
    for c in path.chars() {
        match c {
            '[' => {
                s.push_str("[]");
                skip = true;
            }
            ']' => skip = false,
            _ if skip => {}
            _ => s.push(c),
        }
    }
    s
}

/// Example count per field; `PARITY_EXAMPLES=n` raises it for digging into one class.
fn max_examples() -> usize {
    std::env::var("PARITY_EXAMPLES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5)
}

fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) {
            (Some(x), Some(y)) => (x - y).abs() < 1e-6,
            _ => x == y,
        },
        _ => a == b,
    }
}

struct Table {
    name: &'static str,
    left_only: Vec<String>,
    right_only: Vec<String>,
    both: usize,
    fields: BTreeMap<String, Stat>,
    unmapped: BTreeMap<String, usize>,
    /// Differences that are understood and not a loader fault, counted per field and reason.
    explained: BTreeMap<(String, &'static str), usize>,
}

/// `Some(reason)` when a value difference at `path` is a known property of the source data.
fn explain(table: &str, path: &str, left: &Value) -> Option<&'static str> {
    // An empty monster drop slot keeps a stale percentage in the .mdb; silvermere has no row.
    if table == "monsters" && path.starts_with("drops[") && path.ends_with("].pct") {
        let item = path.replace(".pct", ".item");
        let mut flat = Vec::new();
        flatten("", left, &mut flat);
        if flat.iter().any(|(p, v)| *p == item && v == &Value::from(0)) {
            return Some("unused drop slot keeps a stale percentage in the .mdb");
        }
    }
    None
}

fn compare(
    name: &'static str,
    left: &Value,
    right: &Value,
    key: impl Fn(&Value) -> String,
) -> Table {
    let index = |rows: &Value| -> BTreeMap<String, Value> {
        rows.as_array()
            .expect("table array")
            .iter()
            .map(|v| (key(v), v.clone()))
            .collect()
    };
    let (l, r) = (index(left), index(right));
    let mut t = Table {
        name,
        left_only: l.keys().filter(|k| !r.contains_key(*k)).cloned().collect(),
        right_only: r.keys().filter(|k| !l.contains_key(*k)).cloned().collect(),
        both: 0,
        fields: BTreeMap::new(),
        unmapped: BTreeMap::new(),
        explained: BTreeMap::new(),
    };
    for (k, lv) in &l {
        let Some(rv) = r.get(k) else { continue };
        t.both += 1;
        let (mut lf, mut rf) = (Vec::new(), Vec::new());
        flatten("", lv, &mut lf);
        flatten("", rv, &mut rf);
        let rmap: BTreeMap<_, _> = rf.into_iter().collect();
        for (path, a) in lf {
            let g = generic(&path);
            let top = g.split(['.', '[']).next().unwrap_or("");
            if NOT_MAPPED.contains(&(name, top)) {
                *t.unmapped.entry(g).or_default() += 1;
                continue;
            }
            let b = rmap.get(&path).cloned().unwrap_or(Value::Null);
            if !same(&a, &b)
                && let Some(why) = explain(name, &path, lv)
            {
                *t.explained.entry((g, why)).or_default() += 1;
                continue;
            }
            let s = t.fields.entry(g).or_default();
            if same(&a, &b) {
                s.matched += 1;
            } else {
                s.mismatched += 1;
                if s.examples.len() < max_examples() {
                    s.examples.push((format!("{k} {path}"), a, b));
                }
            }
        }
    }
    t
}

fn key_of(v: &Value, fields: &[&str]) -> String {
    fields
        .iter()
        .map(|f| v[*f].to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: parity MDB_SQLITE SILVERMERE_DB [OUT.md]");
        std::process::exit(2);
    }
    let out_md = args
        .get(3)
        .cloned()
        .unwrap_or_else(|| format!("{}/../../docs/data-parity.md", env!("CARGO_MANIFEST_DIR")));
    let m = mdb::load(&args[1]).expect("load mdb");
    let s = silvermere::open(&args[2])
        .expect("open silvermere")
        .load_world()
        .expect("load silvermere");

    // World serialises as one object of tables; the index maps are skipped.
    let m = serde_json::to_value(&m).expect("serialize");
    let s = serde_json::to_value(&s).expect("serialize");
    let tables = vec![
        compare("items", &m["items"], &s["items"], |v| {
            key_of(v, &["number"])
        }),
        compare("monsters", &m["monsters"], &s["monsters"], |v| {
            key_of(v, &["number"])
        }),
        compare("spells", &m["spells"], &s["spells"], |v| {
            key_of(v, &["number"])
        }),
        compare("rooms", &m["rooms"], &s["rooms"], |v| {
            key_of(v, &["map_number", "room_number"])
        }),
        compare("shops", &m["shops"], &s["shops"], |v| {
            key_of(v, &["number"])
        }),
        compare("classes", &m["classes"], &s["classes"], |v| {
            key_of(v, &["number"])
        }),
        compare("races", &m["races"], &s["races"], |v| {
            key_of(v, &["number"])
        }),
        compare("lairs", &m["lairs"], &s["lairs"], |v| {
            key_of(v, &["group_index"])
        }),
        compare("tbinfo", &m["tbinfo"], &s["tbinfo"], |v| {
            key_of(v, &["number"])
        }),
    ];

    let mut md = String::new();
    md.push_str(
        "# Stock parity: `.mdb` loader vs silvermere loader\n\n\
Generated by `cargo run -p touchstone-data --example parity` (T-015) on the stock game imported by \
`tools/silvermere-stock.sh`. Counts and record numbers only. Fields silvermere has no source for \
(docs/data-gaps.md, status none) are \"not derived\". Records are paired by the table's key; \
numbers compare within 1e-6. Every field that is not 100% is explained in docs/data-gaps.md.\n\n",
    );
    for t in &tables {
        println!(
            "== {}: {} paired, {} only in mdb, {} only in silvermere",
            t.name,
            t.both,
            t.left_only.len(),
            t.right_only.len()
        );
        let _ = writeln!(
            md,
            "## {}\n\nPaired records: {}. Only in the `.mdb`: {}. Only in silvermere: {}.\n",
            t.name,
            t.both,
            t.left_only.len(),
            t.right_only.len()
        );
        if !t.left_only.is_empty() {
            let ex: Vec<_> = t.left_only.iter().take(5).cloned().collect();
            println!("   mdb-only e.g. {ex:?}");
            let _ = writeln!(md, "Example mdb-only record numbers: {}.\n", ex.join(", "));
        }
        if !t.right_only.is_empty() {
            let ex: Vec<_> = t.right_only.iter().take(5).cloned().collect();
            println!("   silvermere-only e.g. {ex:?}");
            let _ = writeln!(
                md,
                "Example silvermere-only record numbers: {}.\n",
                ex.join(", ")
            );
        }
        md.push_str("| Field | Match | Mismatch | Examples (record, path) |\n|---|---|---|---|\n");
        for (f, st) in &t.fields {
            let rate = 100.0 * st.matched as f64 / (st.matched + st.mismatched).max(1) as f64;
            println!(
                "  {f:<32} {:>7} ok {:>7} bad ({rate:.2}%)",
                st.matched, st.mismatched
            );
            for (k, a, b) in &st.examples {
                println!("      {k}: mdb={a} silvermere={b}");
            }
            let ex: Vec<_> = st.examples.iter().map(|e| e.0.clone()).collect();
            let _ = writeln!(
                md,
                "| `{f}` | {} | {} | {} |",
                st.matched,
                st.mismatched,
                ex.join("; ")
            );
        }
        for ((f, why), n) in &t.explained {
            println!("  {f:<32} explained difference, {n} values: {why}");
            let _ = writeln!(md, "| `{f}` | explained | {n} | {why} |");
        }
        for (f, n) in &t.unmapped {
            println!("  {f:<32} not derived, {n} values");
            let _ = writeln!(md, "| `{f}` | not derived | | {n} values |");
        }
        md.push('\n');
    }
    // Info: one row each, version strings differ by nature, so only the shape is reported.
    md.push_str(
        "## info\n\nOne row per source; its fields are NMR's own metadata and silvermere has no \
equivalent (docs/data-gaps.md). Not compared.\n",
    );
    std::fs::write(&out_md, md).expect("write report");
    println!("wrote {out_md}");
}
