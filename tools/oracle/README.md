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
