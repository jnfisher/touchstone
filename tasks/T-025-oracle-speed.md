---
id: T-025
title: Make the oracle fast
model: sonnet
phase: 2
depends: []
touches:
  - tools/oracle/vbi/
  - tools/oracle/harness.py
  - tools/oracle/shims.py
  - tools/oracle/oracle.py
  - tools/oracle/tests/
  - tools/oracle/README.md
---

## Goal
`python3 -I tools/oracle/oracle.py check` (regenerate every family, diff with the committed fixtures)
finishes in about 3 minutes instead of 15-25, without changing a byte of any fixture.

## Read
`tools/oracle/oracle.py`, `harness.py`, `shims.py`, `vbi/interp.py`, `vbi/compiler.py`, `vbi/values.py`.

## Do
Profile first, then fix what the profile shows. Findings (sampling profiler, `signal.setitimer`; cProfile
is 5x slower and unusable on `combat`):
1. `combat` (the slowest family, 584 s CPU) spends its time in one MME loop (`CalcCombatRounds`,
   the regen `Do While`) that runs millions of iterations of Double-with-Integer arithmetic,
   comparisons and `If`/`Do` tests. Cost was `values.arith` / `_prep` / `as_float` / `is_num`.
2. Item-table families (`item_match`, `textblock`, `monster_glue`) rebuilt and re-sorted the
   recordset index (`_set_index`) on every `tabX.Index = ...`, and every case reopens its recordsets.
3. `call_proc` argument binding: per-call `isinstance` chains, slot lists, static/return setup.
4. `run` stored the program counter on the frame for every instruction.
5. No module re-parsing per case (modules load once per family); no shim linear scans beyond `_set_index`.

Changes:
- `values.py`: fast paths in `arith`, `div`, `compare` for Double/integer-type operands; `EXACT_TYPE`.
- `compiler.py`: `+ - * /` closures handle Double with Double/integer inline and fall back to the general
  path (so Overflow, divide by zero and every other type keep their old behaviour); skip coercion of an
  already-exact type on assignment; inline `truth` in `Do` tests; cached scalar function-return default.
- `interp.py`: `call_proc` fast path for positional plain arguments; `run` records the pc only when an
  error is raised.
- `shims.py`: `Database.index_for` builds each index (order, first-row-per-key map, row->position map)
  once per database load; `Seek` and `Bookmark` use the position map instead of `list.index`.
- `oracle.py check --jobs N` (default `min(4, cpu_count)`): families in forked worker processes,
  biggest scenario first, each family reported as it finishes.
- `tests/test_fastpaths.py`: the fast paths agree with the general path, including errors.

## Timings
`oracle.py gen FAMILY`, CPU seconds, before -> after the arithmetic/interpreter changes (the machine was
shared; load average 3-5 throughout, CPU time is the stable number):
combat 584 -> 143, textblock 62 -> 59, monster_glue 46 -> 42, exp 25 -> 24, gear2 24 -> 23,
gear 19 -> 18, monster_sim 17 -> 16, pipeline 15 -> 15, spells 12 -> 12, ability_text 11 -> 10,
exphr_b 6.0 -> 3.7; the other families are under 6 s each. Sum 865 -> 406.
The index cache then took `item_match` 234 -> 37 s, `textblock` 147 -> 45 s and `monster_glue`
95 -> 30 s wall (measured under load, in a parallel `check`; those two families were still slow in the
sum above because the cache landed after that measurement).
Full `oracle.py check` (29 families, -j 4, load average ~3): 181 s wall, 350 s CPU. Before this card
the same check took 15-25 minutes (serial).

## Done when
- `python3 -I tools/oracle/oracle.py check` reports every family current, in under ~4 minutes.
- `python3 -I -m unittest discover -s tools -p 'test_*.py'` passes; `tools/guard.sh` passes.
