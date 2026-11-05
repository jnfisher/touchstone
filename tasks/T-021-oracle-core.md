---
id: T-021
title: Oracle: a VB6-subset interpreter that runs MME's formula modules
model: sonnet
phase: 2
depends: [T-002, T-020]
touches:
  - tools/oracle/
---

## Goal
A stdlib Python interpreter that parses and runs MME's formula modules unmodified, with VB6
semantics matching `fixtures/vb6-semantics.json`. This is the reference every fixture comes from,
so correctness beats speed.

## Read
`docs/testing.md` "The oracle". The index (T-002) of `modMMudFunc`, `modSyntaxsFunc`,
`clsMonsterAttackSim`, `modExpPerHour` and the `_declarations.vb` of `modMain` and
`modMMudDatabase` (globals and Types the formulas use). Read the source files from the MME cache
(`iconv -f cp1252`), not from the repository.

## Do
1. Lexer and parser for the subset those modules use (find it by parsing them): module
   declarations (Dim/Public/Private/Global/Const, Type, Enum, Declare: record but do not run),
   Sub/Function/Property with Optional, ByRef/ByVal, ParamArray, defaults; If (block and single-line),
   Select Case (Is, To, lists), For/Next, For Each, Do/Loop (While/Until, both ends), While/Wend, With,
   Exit, GoTo/labels, On Error GoTo/Resume Next, Static, ReDim (Preserve), multi-dimensional arrays
   with explicit bounds, UDTs, class modules with properties and `New`, `Me`, line continuations,
   `:` statement separators, type suffixes (`%&!#$@`), `&H` literals.
2. Interpreter: typed variables with VB6 coercion (Integer, Long, Single, Double, Currency if used,
   String, Boolean, Byte, Variant, Date if used), ByRef semantics, overflow raising error 6, On Error
   handling, the built-in functions the modules call (from the T-020 table plus string/math
   functions), and the VB6 `Rnd`.
3. Unknown external references (forms, recordsets, other modules' procedures that are not loaded)
   raise a clear `OracleUnbound(name)` error; T-022 adds shims for them.
4. Tests (unittest): every case of `fixtures/vb6-semantics.json`; small invented VB programs for each
   construct; parsing every procedure of the four modules.

## Done when
- All four modules and the two declaration sections parse with zero errors.
- The VB6 semantics table passes in full.
- Report which procedures can already run with no shims (call them with simple arguments) and the
  list of unbound names the rest need, grouped (recordsets, forms, globals, other modules). This
  list sizes T-022.

If the interpreter is not converging, push what passes and write the remaining failures in the PR
body's `## Blocked` section; the orchestrator will escalate.
