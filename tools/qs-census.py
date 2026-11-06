#!/usr/bin/env python3
"""Census of a Quicksilver 1.20 SQLite database against what touchstone's loader expects.

    python3 -I tools/qs-census.py [--db PATH] [--stock PATH] [--json]

Defaults: the databases tools/qs-fetch.sh and tools/silvermere-stock.sh leave in
~/.cache/touchstone. Read-only; stdlib only. Reports:
  - migrations and the recorded content version
  - row counts per table against the stock silvermere database
  - slot use of every child table against the capacity of the model's fixed arrays
  - every ability number in the five abilities tables, known or unknown to touchstone
    (known = it has a name in crates/touchstone-rules/src/ability_text.rs)
  - the Quicksilver-only ranges (reserved spell numbers, regeneration abilities, race slots)
  - integer values the loader cannot hold (outside 32 bits)
"""
import argparse
import json
import os
import re
import sqlite3
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CACHE = os.environ.get("TOUCHSTONE_CACHE", os.path.expanduser("~/.cache/touchstone"))

# table, parent key column, slot capacity of the model array it fills (crates/touchstone-model)
ABILITY_TABLES = [
    ("item_abilities", "item", 20),
    ("monster_abilities", "monster", 10),
    ("spell_abilities", "spell", 10),
    ("class_abilities", "class", 10),
    ("race_abilities", "race", 10),
]
OTHER_CHILDREN = [
    ("item_classes", "item", 10),
    ("item_races", "item", 10),
    ("item_negates", "item", 10),
    ("monster_attacks", "monster", 5),
    ("monster_spells", "monster", 5),
    ("monster_items", "monster", 10),
    ("shop_items", "shop", 20),
]


def known_abilities():
    """Ability numbers with a name in ability_text.rs (plain names and the message-only ones)."""
    path = os.path.join(ROOT, "crates/touchstone-rules/src/ability_text.rs")
    src = open(path, encoding="utf-8").read()
    names = {}
    for fn in ("plain_name", "forced_name"):
        m = re.search(r"fn %s\(.*?\n\}\n" % fn, src, re.S)
        if not m:
            sys.exit("cannot find %s in ability_text.rs" % fn)
        for n, name in re.findall(r'^\s*(\d+)\s*=>\s*"([^"]*)"', m.group(0), re.M):
            names[int(n)] = name
    return names


def connect(path):
    return sqlite3.connect("file:%s?mode=ro" % path, uri=True)


def tables(c):
    return [
        r[0]
        for r in c.execute(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' "
            "AND name NOT LIKE '\\_%' ESCAPE '\\' ORDER BY name"
        )
    ]


def count(c, t):
    try:
        return c.execute('SELECT count(*) FROM "%s"' % t).fetchone()[0]
    except sqlite3.Error:
        return None


def census(qs, stock):
    out = {}
    out["migrations"] = [r[0] for r in qs.execute("SELECT version FROM _sqlx_migrations ORDER BY version")]
    cols = [r[1] for r in qs.execute("PRAGMA table_info(world_import)")]
    out["content_version"] = (
        qs.execute("SELECT content_version FROM world_import ORDER BY id DESC LIMIT 1").fetchone()[0]
        if "content_version" in cols
        else None
    )
    stock_tables = set(tables(stock)) if stock else set()
    out["counts"] = {
        t: {"quicksilver": count(qs, t), "stock": count(stock, t) if t in stock_tables else None}
        for t in tables(qs)
    }

    known = known_abilities()
    used = {}
    slots = {}
    for table, key, cap in ABILITY_TABLES + [(t, k, c) for t, k, c in OTHER_CHILDREN]:
        mx = qs.execute('SELECT max(slot) FROM "%s"' % table).fetchone()[0]
        slots[table] = {"max_slot_used": mx, "capacity": cap, "overflow": mx is not None and mx >= cap}
    out["slots"] = slots
    for table, key, _ in ABILITY_TABLES:
        for k, ab, val in qs.execute('SELECT %s, ability, value FROM "%s" WHERE ability <> 0' % (key, table)):
            e = used.setdefault(ab, {"rows": 0, "tables": {}, "examples": {}})
            e["rows"] += 1
            e["tables"][table] = e["tables"].get(table, 0) + 1
            ex = e["examples"].setdefault(table, [])
            if len(ex) < 3:
                ex.append([k, val])
    stock_used = set()
    if stock:
        for table, _, _ in ABILITY_TABLES:
            stock_used |= {r[0] for r in stock.execute('SELECT DISTINCT ability FROM "%s" WHERE ability <> 0' % table)}
    out["abilities"] = {
        "known_count": len(known),
        "used_count": len(used),
        "used_in_stock_count": len(stock_used),
        "new_vs_stock": sorted(set(used) - stock_used),
        "unknown": {str(a): used[a] for a in sorted(used) if a not in known},
        "forced_message_only": sorted(a for a in used if a in (101, 115, 120, 137, 144, 155)),
        "max_used": max(used) if used else None,
    }

    # Quicksilver-only ranges
    sp = lambda lo, hi: qs.execute("SELECT count(*) FROM spells WHERE number BETWEEN ? AND ?", (lo, hi)).fetchone()[0]
    out["ranges"] = {
        "spells_total": count(qs, "spells"),
        "spells_monster_copies_5000_5999": sp(5000, 5999),
        "spells_mana_regen_6000_6020": sp(6000, 6020),
        "spells_max_number": qs.execute("SELECT max(number) FROM spells").fetchone()[0],
        "items_max_number": qs.execute("SELECT max(number) FROM items").fetchone()[0],
        "monsters_max_number": qs.execute("SELECT max(number) FROM monsters").fetchone()[0],
    }
    out["race_abilities"] = {
        str(r[0]): [[a, v] for a, v in qs.execute("SELECT ability, value FROM race_abilities WHERE race=? ORDER BY slot", (r[0],))]
        for r in qs.execute("SELECT number FROM races ORDER BY number")
    }
    if stock:
        out["race_abilities_stock"] = {
            str(r[0]): [[a, v] for a, v in stock.execute("SELECT ability, value FROM race_abilities WHERE race=? ORDER BY slot", (r[0],))]
            for r in stock.execute("SELECT number FROM races ORDER BY number")
        }

    # enum-like columns (at most 40 distinct stock values) that hold a value stock never does
    new_codes = []
    if stock:
        for t in sorted(set(tables(qs)) & stock_tables):
            if count(qs, t) == 0:
                continue
            scols = {r[1] for r in stock.execute('PRAGMA table_info("%s")' % t)}
            for _, name, typ, *_ in qs.execute('PRAGMA table_info("%s")' % t):
                if name == "id" or name not in scols or typ.upper().startswith(("TEXT", "BLOB")):
                    continue
                sv = {r[0] for r in stock.execute('SELECT DISTINCT "%s" FROM "%s"' % (name, t))}
                if len(sv) > 40:
                    continue
                new = {r[0] for r in qs.execute('SELECT DISTINCT "%s" FROM "%s"' % (name, t))} - sv
                if new:
                    new_codes.append({"table": t, "column": name, "new_values": sorted(new)})
    out["new_codes"] = new_codes

    # references to rows that do not exist, quicksilver against stock
    refs = [
        ("monster_spells", "spell", "spells"), ("monster_items", "item", "items"),
        ("shop_items", "item", "items"), ("item_negates", "spell", "spells"),
        ("room_placed_items", "item", "items"), ("monsters", "deathspell", "spells"),
        ("monsters", "createspell", "spells"), ("monsters", "weapon", "items"),
        ("monster_attacks", "hitspell", "spells"),
    ]
    dang = lambda c, t, col, r: c.execute(
        'SELECT count(*) FROM "%s" WHERE "%s" <> 0 AND "%s" NOT IN (SELECT number FROM "%s")' % (t, col, col, r)
    ).fetchone()[0]
    out["dangling"] = {
        "%s.%s" % (t, col): {"quicksilver": dang(qs, t, col, r), "stock": dang(stock, t, col, r) if stock else None}
        for t, col, r in refs
    }
    # spells whose 16-bit level cap carries padding in the high byte (the loader keeps the low byte)
    out["spell_cap_padding"] = {
        "quicksilver": qs.execute("SELECT count(*) FROM spells WHERE levelcap > 255").fetchone()[0],
        "stock": stock.execute("SELECT count(*) FROM spells WHERE levelcap > 255").fetchone()[0] if stock else None,
    }

    # values outside 32 bits in any integer column
    wide = []
    for t in tables(qs):
        for _, name, typ, *_ in qs.execute('PRAGMA table_info("%s")' % t):
            if typ.upper().startswith(("INT", "BIGINT")) and name not in ("id",):
                n = qs.execute(
                    'SELECT count(*) FROM "%s" WHERE typeof("%s")=\'integer\' AND ("%s" > 2147483647 OR "%s" < -2147483648)'
                    % (t, name, name, name)
                ).fetchone()[0]
                if n:
                    wide.append({"table": t, "column": name, "rows": n})
    out["wider_than_i32"] = wide
    return out


def text(r):
    L = []
    L.append("migrations %s, content version %s" % (r["migrations"], r["content_version"]))
    L.append("")
    L.append("%-26s %10s %10s" % ("table", "quicksilver", "stock"))
    for t, c in r["counts"].items():
        mark = "" if c["stock"] is None or c["quicksilver"] == c["stock"] else "  (+%d)" % (c["quicksilver"] - c["stock"])
        L.append("%-26s %10s %10s%s" % (t, c["quicksilver"], c["stock"], mark))
    L.append("")
    L.append("slot use (max slot used vs model capacity)")
    for t, s in r["slots"].items():
        L.append("  %-20s %s / %d%s" % (t, s["max_slot_used"], s["capacity"], "  OVERFLOW" if s["overflow"] else ""))
    a = r["abilities"]
    L.append("")
    L.append(
        "abilities: %d known to touchstone, %d used (stock uses %d), highest used %s"
        % (a["known_count"], a["used_count"], a["used_in_stock_count"], a["max_used"])
    )
    L.append("  used by Quicksilver but not by stock: %s" % a["new_vs_stock"])
    L.append("  unknown to touchstone: %s" % (list(a["unknown"]) or "none"))
    for n, e in a["unknown"].items():
        L.append("    %s: %d rows %s" % (n, e["rows"], e["tables"]))
    L.append("  message-only abilities in use: %s" % a["forced_message_only"])
    L.append("")
    for k, v in r["ranges"].items():
        L.append("%-36s %s" % (k, v))
    L.append("")
    L.append("race abilities (quicksilver; stock differs marked *)")
    for n, v in r["race_abilities"].items():
        s = r.get("race_abilities_stock", {}).get(n)
        L.append("  race %-3s %s%s" % (n, [x for x in v if x[0]], "  *" if s is not None and s != v else ""))
    L.append("")
    L.append("codes in enum-like columns that stock never uses:")
    for n in r["new_codes"] or [None]:
        L.append("  none" if n is None else "  %s.%s: %s" % (n["table"], n["column"], n["new_values"]))
    L.append("")
    L.append("references to missing rows (quicksilver / stock)")
    for k, v in r["dangling"].items():
        L.append("  %-28s %s / %s" % (k, v["quicksilver"], v["stock"]))
    L.append("spells with a padded 16-bit level cap (quicksilver / stock): %(quicksilver)s / %(stock)s" % r["spell_cap_padding"])
    L.append("")
    L.append("integer columns wider than 32 bits: %s" % (r["wider_than_i32"] or "none"))
    return "\n".join(L)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--db", default=os.path.join(CACHE, "quicksilver-1.20.sqlite"))
    ap.add_argument("--stock", default=os.path.join(CACHE, "silvermere-stock.sqlite"))
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args()
    if not os.path.isfile(a.db):
        sys.exit("no database at %s: run tools/qs-fetch.sh" % a.db)
    qs = connect(a.db)
    stock = connect(a.stock) if os.path.isfile(a.stock) else None
    r = census(qs, stock)
    print(json.dumps(r, indent=1, sort_keys=True) if a.json else text(r))


if __name__ == "__main__":
    main()
