---
id: T-002
title: Fetch tools for MME source and data, and the MME procedure index
model: sonnet
phase: 0
depends: []
touches:
  - tools/mme-fetch.sh
  - tools/mdb-fetch.sh
  - tools/mdb2sqlite.py
  - tools/mme-index/
  - .github/workflows/tools.yml
---

## Goal
Agents can get MME's source and the stock database at the pin with one command each, and read MME
one procedure at a time through an index instead of opening 1.6 MB forms.

## Read
`CLAUDE.md` (hard rules 1, 3, 6), `docs/design.md` ("Reference sources", "Data").

## Do
1. `tools/mme-fetch.sh`: `MME_SHA` defaults to the pin in design.md
   (`28d23d70caf4f971418aa7ea25471cdb0ab0888a`). Downloads `jnfisher/MMUD-Explorer` at that commit
   (codeload tarball, or `git init` + `git fetch --depth 1 origin <sha>`) into
   `${TOUCHSTONE_CACHE:-~/.cache/touchstone}/mme/<sha>/`. Idempotent (a marker file), prints the
   directory. Never writes inside the repository.
2. `tools/mdb2sqlite.py SRC.mdb OUT.sqlite`: stdlib Python that shells out to mdbtools
   (`mdb-tables -1`, `mdb-schema`, `mdb-export` with `-D '%Y-%m-%d %H:%M:%S'` and a delimiter safe for
   embedded commas and newlines, e.g. `-d '\x1f' -R '\x1e'`, or `-I sqlite` INSERT output) and writes
   every table into SQLite with column names exactly as in the `.mdb` and SQLite types mapped from
   the Jet types (Integer/Long -> INTEGER, Single/Double -> REAL, Text/Memo -> TEXT, Boolean ->
   INTEGER 0/1). Writes to a temp file and renames, so a failed run leaves nothing.
3. `tools/mdb-fetch.sh`: takes the `.mdb` from the MME cache (running `mme-fetch.sh` if needed),
   checks sha256 `eba20f06af8dc08df755eb6dcd9763b023794f12e809cbab6762236d64d7345f`, converts it to
   `<cache>/mdb/<sha256>.sqlite` with `mdb2sqlite.py`, prints that path. Fails clearly if mdbtools is
   missing (`apt install mdbtools`).
4. `tools/mme-index/mme_index.py [--mme DIR] [--out DIR]` (stdlib, `python3 -I`): for every
   `.bas`, `.cls` and `.frm` in the MME directory, decode Windows-1252, drop the `.frm` designer block
   (keep a list of control names and types from it), join `_` line continuations for parsing but
   keep original text in output, and split into:
   - `<out>/<module>/_declarations.vb`: the module-level section (globals, Const, Type, Enum,
     Declare).
   - `<out>/<module>/<proc>.vb`: one file per Sub/Function/Property Get/Let/Set (any of Public,
     Private, Friend, Static modifiers), UTF-8, with a first comment line giving module, original
     line range and signature. Property procedures get `<name>.get.vb` etc.
   - `<out>/index.json`: per procedure: module, name, kind, visibility, params, return type, line
     range, line count, `calls` (other indexed procedures it references), `tables` (DAO recordset
     globals referenced, `tab*`), `forms` (form/control references like `frmMain.chkX`), `globals`
     (module-level variables of other modules it reads or writes).
   - `<out>/index.md`: one line per procedure grouped by module: name, kind, lines, signature.
   - Default out dir: `<cache>/mme-index/<sha>/`.
   - `--deps NAME` prints the transitive `calls` closure of a procedure with line counts and total.
5. `tools/mme-index/tests/test_mme_index.py` (unittest) over small **invented** VB snippets in the
   test file (never MME code): continuations, all procedure kinds, `.frm` designer block skipping,
   cp1252 bytes, Property triplets, calls detection.
6. `.github/workflows/tools.yml`: on `pull_request` with paths `tools/**`, runs
   `python3 -I -m unittest discover -s tools -p 'test_*.py'`. Fast; no fetching.

## Done when
- The unittest suite passes.
- On the real pinned MME: the index has no parse warnings; the number of Sub/Function procedures in
  `frmMain` matches `grep -ciE '^(public |private |friend )?(static )?(function|sub) '` on the
  decoded file (423 at the pin), and likewise for every module.
- `tools/mdb-fetch.sh` produces the SQLite, and for every table the row count equals `mdb-count`.
- Report in your final message: per module procedure count and total lines, the 20 largest
  procedures, and the per-table row counts. (These numbers feed T-003.)
