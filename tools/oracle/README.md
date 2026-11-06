# Oracle

A stdlib-only Python interpreter for the subset of VB6 that MME's formula modules use. It loads
the modules unchanged from the pinned MME cache (`tools/mme-fetch.sh`) and runs their procedures,
so parity fixtures come from MME's own code. Nothing from MME is stored in this repository.

    python3 -I tools/oracle/oracle.py parse             # parse the four formula modules + two declaration modules
    python3 -I tools/oracle/oracle.py report [--md|--json]   # unbound names, procedures that run without shims
    python3 -I tools/oracle/oracle.py call PROC [ARG...]
    python3 -I -m unittest discover -s tools -p 'test_*.py'

Layout: `vbi/lexer.py`, `parser.py` (AST in `ast.py`), `compiler.py` (procedures become flat
instruction lists, so GoTo / On Error / Resume behave as in VB6), `interp.py` (loading, calls,
frames), `values.py` (typed values, coercion, operators, Format/Val/Rnd), `builtins.py`,
`objects.py` (variables, arrays, Types, class instances, the `ExtObject` base for shims),
`analysis.py` (static unbound-name report and smoke runs). `unbound-report.md` is the generated
list of what T-022 has to provide.

## For shims (T-022)

- `Interpreter.load_file(path, load_procs=False)` registers a module's declarations (globals, Types,
  Enums, constants) without its procedures; the procedures then count as unloaded.
- `it.set_global(name, value)` / `get_global` take Python values (int, float, str, bool, list for
  an array, dict for a Type). `it.call(name, *args, this=None, module=None)` runs a procedure;
  `it.instantiate(cls)` makes a class-module instance.
- `it.bind(name, obj)` binds a form or any object under a global name; objects subclass
  `objects.ExtObject` (`vb_get`, `vb_set`, `vb_default`, `vb_bang`, `vb_iter`).
  `it.bind_proc(name, fn)` provides a procedure of a module that is not loaded (`fn(it, *args)`
  receives VB values from `vbi.values`). `it.new_hooks[typename]` makes `New X` of an external class work.
- A name that is neither loaded nor bound raises `OracleUnbound(name)`. It is not a `VBError`, so
  `On Error` never swallows it. Method calls on a declared-but-unset external object variable
  (a Recordset global) also raise it.
- `Randomize` with no argument does nothing by default (the generator keeps VB6's initial state
  0x50000) so runs are reproducible; set `it.rnd.state` or `it.randomize_hook` to control it.

## VB6 decisions

The table in `fixtures/vb6-semantics.json` is the contract (the oracle passes all 498 cases, both
directly and through compiled VB source). Beyond it:

- Arithmetic result type is the most precise operand in Byte < Integer < Long < Single < Currency <
  Double, except Long with Single, which widens to Double. `/` gives Double, except Single (with
  Byte/Integer/Single) gives Single and Currency (with Byte/Integer/Long/Currency) gives Currency.
  Integer-typed intermediates overflow with error 6 even when assigned to a Long.
- Currency is a scaled 64-bit integer; `*` and `/` round to four decimals half to even.
- `True` is -1. `And/Or/Xor/Not` are bitwise; two Booleans give a Boolean.
- A string compared with a number is converted to a number (error 13 if it is not numeric); real VB
  only does that for typed Strings, a Variant string would sort after numbers.
- Enum members are converted with CLng, so a member written `29.99` is 30.
- Without `Call`, `Sub (x)` with one parenthesised argument passes it ByVal.
- A ByRef argument shares the caller's variable only when the declared types are equal; otherwise
  the callee gets a temporary (VB rejects most of those at compile time).
- A `For` counter that would leave its type's range ends the loop instead of raising.
- `Resume Next` continues at the instruction after the failing one (an error in an `If` condition
  continues inside the Then branch, as in VB6). An error raised inside a handler propagates to the
  caller; `Resume` or a new `On Error` re-arms the handler.
- Not supported (no MME module uses them): GoSub/Return, file I/O statements, date literals,
  the `Mid` statement, `Next i, j`, Decimal (CDec returns a Double), Collection/Dictionary/RegExp
  (shims, via `new_hooks` and `bind`).

## Scenarios and fixtures (T-022)

    python3 -I tools/oracle/oracle.py gen FAMILY|--all [--out DIR]
    python3 -I tools/oracle/oracle.py check [-j N] [FAMILY...]   # regenerate to a temp dir, byte-compare with fixtures/rules/

`fixtures/scenarios/<family>.json` in, `fixtures/rules/<family>.json` out (byte-identical on a re-run;
nothing time- or path-dependent is written). `check` is what `.github/workflows/oracle-check.yml`
runs (manual). `check` builds families in parallel worker processes (`-j N`, default
`min(4, cpu count)`, biggest scenario first) and prints each family as it finishes. Code: `harness.py` (environment, case runner, value coding), `shims.py` (recordsets,
forms), `scenario.py` (grids and seeded samples).

### Scenario file

    { "family", "description", "notes": [..], "database": true|false,
      "tolerance": {"default": 0, "fields": {"<output name>": 1e-9}},
      "globals": {..}, "forms": {..},          // family-wide setup, copied into the fixture header
      "cases": [..], "grids": [..], "samples": [..] }

A **case** (`id` is filled in for grids/samples):

    { "id", "rnd": <24-bit Rnd state, default 327680>,
      "globals": {"nNMRVer": 1.8, "nGlobalCharWeaponAccy": [1,2,3]},   // module globals; arrays as lists, Types as objects
      "forms": {"frmMain": {"lblDatVer": {"Caption": "v1.11p"}, "lblInvenCharStat": {"$array": {"3": {"Caption": "7"}}}}},
      "objects": {"sim": {"class": "clsMonsterAttackSim", "props": {"nUserAC": 40, "nAtkMin": [5,0,0,0,0]}}},
      "steps": [{"call": "sim.RunSim"} | {"call": "Proc", "args": [..], "as": "name"} | {"set": "sim.prop", "index": [0], "value": 1}],
      "proc": "Proc", "args": [..],            // shorthand for one call stored as outputs.result
      "read": {"name": "sim.prop" | "sim.prop[0..4]" | "%rnd" | "%g.globalName"} }

Values: JSON numbers (small ints are Integer, larger Long, with a point Double), strings, booleans;
typed values as `{"t": "cur|single|i16|i32|byte|f64|str|null|empty", "v": ..}`. A call argument
`{"ref": "long", "value": 0}` is passed ByRef; its final value lands in `outputs["<as>.refs"]`. A ByRef array argument is
`{"ref": "array:currency|string|<udt>", "bounds": [[lo, hi], ..]}` (no bounds: unallocated); it lands in `.refs` as a flat
list, row-major. `{"ref": "array:double", "values": [..]}` passes an array holding those values (zero-based).
Objects are class-module instances created fresh per case; properties given as lists are indexed
Property Lets. `grids`: `{"name", "axes": {a: [..]}, "case": {..}}` (`"$a"` is replaced by the axis
value; `{"$add"|"$sub"|"$mul"|"$min"|"$max": [x, y]}` work elementwise on lists; `{"$merge": [obj, ..]}`).
`samples`: `{"name", "seed", "count", "draw": {v: {"int": [lo, hi]} | {"choice": [..]}, "n": k for lists}, "case"}`,
drawn with splitmix64 so they never depend on the Python version.

Scenario extras: `"modules": ["modItemParse.bas"]` loads more MME files beside the six always loaded;
a call argument `{"udt": "TypeName", "fields": {...}}` builds a Type value (a list fills a dynamic array,
`[]` leaves it unallocated). `Not Not arr` evaluates to 0/1 for unallocated/allocated arrays.

### Fixture file

One line per case. `header`: family, `mme_sha`, `generator_version`, scenario and database sha256,
`tolerance` (exact unless the scenario says otherwise), `setup`, `notes`, `case_count`. Each case has
`inputs` (the expanded case), `outputs`, and when they happen `error` (a VB error that escaped a
step: number, description) and `events` (MsgBox text and errors that MME's `HandleError` would have shown).
Outputs: Integer/Long/Boolean/String plain; Double always with a point or exponent; Currency as
`{"cur": value*10000, "s": "display"}`; Single as `{"single": x, "s": shortest decimal}`.

### Environment the shims build

All six modules (modMain, modMMudDatabase, the four formula modules) are loaded with their
procedures, so MME's own helpers (GetItemName, GetSpellName, F1..F6, the Get* lookups, ...) run
unchanged. Replaced by recorded no-ops: DebugLogPrint, AppTerminate, Sleep, Unload, SetClipboardText,
the Win32 declares (return 0), HandleError (recorded as an event, not shown), EnsureDatabaseConnection
(False); MsgBox returns its default button. Globals are reset to VB defaults before every case and then
set from the scenario. `frmMain` and any form named under `forms` are `FormShim`s: any control reads as
its VB default (Text/Caption/Tag "", ListIndex -1, other numbers 0) until the scenario sets it. With
`"database": true` each `tab*` global is a fresh table-type Recordset over the converted stock SQLite
(cursor on the first record in insertion order, no index until MME sets one; `pkX` orders by Number,
`idxRooms` by Map Number, Room Number; a failed Seek leaves the cursor at EOF; NULL in the SQLite reads
as the NOT NULL column's empty value, because the conversion stores an empty text as NULL).
`Static` locals are not reset between cases.
