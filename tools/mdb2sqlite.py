#!/usr/bin/env python3
"""Convert an Access .mdb to SQLite using mdbtools. Usage: mdb2sqlite.py SRC.mdb OUT.sqlite

Column names are kept exactly as in the .mdb. Writes to a temp file and renames, so a failed run
leaves nothing behind.
"""
import os
import re
import sqlite3
import subprocess
import sys

FS = "\x1f"  # field separator
RS = "\x1e"  # row separator
NULL = "\x1d"  # NULL marker (mdb-export -0)


def run(args):
    try:
        p = subprocess.run(args, capture_output=True, check=False)
    except FileNotFoundError:
        sys.exit("mdbtools is missing: apt install mdbtools")
    if p.returncode != 0:
        sys.exit("%s failed: %s" % (args[0], p.stderr.decode("utf-8", "replace")))
    return p.stdout.decode("utf-8")


def sqlite_type(jet):
    j = jet.lower()
    if "boolean" in j or j.startswith("bit"):
        return "INTEGER"
    if "integer" in j or j in ("byte", "long", "counter", "autonumber"):
        return "INTEGER"
    if j.startswith(("single", "double", "float", "real", "numeric", "decimal")):
        return "REAL"
    if j.startswith("currency"):
        return "REAL"
    return "TEXT"  # Text, Memo, DateTime, Binary, Replication ID...


def parse_schema(text):
    """Return {table: [(column, jet_type), ...]} in column order."""
    tables = {}
    cur = None
    for line in text.splitlines():
        m = re.match(r"CREATE TABLE \[(.+)\]\s*$", line)
        if m:
            cur = tables.setdefault(m.group(1), [])
            continue
        m = re.match(r"\s+\[(.+?)\]\s+(.*?)\s*,?\s*$", line)
        if m and cur is not None:
            cur.append((m.group(1), re.sub(r"\s+NOT NULL$", "", m.group(2))))
        elif line.startswith(")"):
            cur = None
    return tables


def convert(value, kind, jet):
    if value == NULL:
        return None
    if kind == "INTEGER":
        return int(value) if value != "" else None
    if kind == "REAL":
        return float(value) if value != "" else None
    return value


def main(src, out):
    schema = parse_schema(run(["mdb-schema", "--no-indexes", "--no-relations", src, "access"]))
    names = [t for t in run(["mdb-tables", "-1", src]).split("\n") if t.strip()]
    tmp = out + ".tmp.%d" % os.getpid()
    if os.path.exists(tmp):
        os.remove(tmp)
    try:
        con = sqlite3.connect(tmp)
        for t in names:
            cols = schema.get(t)
            if not cols:
                sys.exit("no schema for table %s" % t)
            kinds = [sqlite_type(j) for _, j in cols]
            ddl = ", ".join('"%s" %s' % (c.replace('"', '""'), k) for (c, _), k in zip(cols, kinds))
            con.execute('CREATE TABLE "%s" (%s)' % (t, ddl))
            raw = run([
                "mdb-export", "-H", "-Q", "-0", NULL, "-d", FS, "-R", RS,
                "-D", "%Y-%m-%d", "-T", "%Y-%m-%d %H:%M:%S", src, t,
            ])
            rows = []
            for rec in raw.split(RS):
                if rec == "" or rec == "\n":
                    continue
                if rec.startswith("\n"):
                    rec = rec[1:]  # tolerate a newline after the row delimiter
                fields = rec.split(FS)
                if len(fields) != len(cols):
                    sys.exit("%s: row has %d fields, expected %d" % (t, len(fields), len(cols)))
                rows.append([convert(v, k, j) for v, k, (_, j) in zip(fields, kinds, cols)])
            ph = ",".join("?" * len(cols))
            con.executemany('INSERT INTO "%s" VALUES (%s)' % (t, ph), rows)
        con.commit()
        con.close()
        os.replace(tmp, out)
    finally:
        if os.path.exists(tmp):
            os.remove(tmp)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit("usage: mdb2sqlite.py SRC.mdb OUT.sqlite")
    main(sys.argv[1], sys.argv[2])
