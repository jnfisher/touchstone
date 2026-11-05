#!/usr/bin/env python3
"""Write fixtures/data/stock.json: per table the row count, the column names in schema order, and
for each row its record key plus a short hash of a canonical serialisation. Hashes only: no game
value is ever written.

usage: python3 -I tools/data-fixtures.py SQLITE OUT_DIR

Canonical serialisation of a row: the cells in schema order, joined with U+001F. Integers are
decimal, REAL columns use Python's repr of a float, text is as-is and NULL is empty. The hash is
the first 12 hex digits of the sha256 of the UTF-8 bytes. The record key is the Number column, or
`map/room` for Rooms, the GroupIndex for Lairs, and the row's position for Info.
"""
import hashlib
import json
import os
import sqlite3
import sys

SEP = "\x1f"
HASH_LEN = 12
TABLES = ["Classes", "Info", "Items", "Lairs", "Monsters", "Races", "Rooms", "Shops", "Spells", "TBInfo"]


def cell(value, decl):
    if value is None:
        return ""
    if decl.upper() == "REAL":
        return repr(float(value))
    if isinstance(value, int):
        return str(value)
    if isinstance(value, float):
        return repr(value)
    return str(value)


def record_key(table, names, cells, pos):
    col = dict(zip(names, cells))
    if table == "Rooms":
        return "%s/%s" % (col["Map Number"], col["Room Number"])
    if table == "Lairs":
        return col["GroupIndex"]
    if table == "Info":
        return str(pos)
    return col["Number"]


def table_fixture(con, table):
    info = list(con.execute('PRAGMA table_info("%s")' % table))
    names = [r[1] for r in info]
    decls = [r[2] for r in info]
    rows = []
    for pos, row in enumerate(con.execute('SELECT * FROM "%s" ORDER BY rowid' % table)):
        cells = [cell(v, d) for v, d in zip(row, decls)]
        digest = hashlib.sha256(SEP.join(cells).encode("utf-8")).hexdigest()[:HASH_LEN]
        rows.append((record_key(table, names, cells, pos), digest))
    return {"count": len(rows), "columns": names, "rows": rows}


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    sqlite_path, out_dir = sys.argv[1:]
    con = sqlite3.connect("file:%s?mode=ro" % sqlite_path, uri=True)
    os.makedirs(out_dir, exist_ok=True)
    out = os.path.join(out_dir, "stock.json")
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write('{\n"hash_len": %d,\n"tables": {\n' % HASH_LEN)
        for i, t in enumerate(TABLES):
            fx = table_fixture(con, t)
            f.write('%s: {\n"count": %d,\n"columns": %s,\n"rows": [\n' % (json.dumps(t), fx["count"], json.dumps(fx["columns"])))
            f.write(",\n".join(json.dumps(r) for r in fx["rows"]))
            f.write("\n]\n}%s\n" % ("," if i < len(TABLES) - 1 else ""))
        f.write("}\n}\n")
    print(out)


main()
