---
id: T-020
title: VB6 semantics table and the touchstone-vb6 crate
model: sonnet
phase: 2
depends: [T-001]
touches:
  - crates/touchstone-vb6/
  - fixtures/vb6-semantics.json
---

## Goal
One table of VB6 behaviours that both the oracle (T-021) and the Rust port are tested against, and
a crate implementing them, so ported formulas round and overflow exactly like MME.

## Read
`docs/testing.md` layer 2. Which types and functions MME's formula modules use: `grep` the index
(T-002 output) of `modMMudFunc`, `modSyntaxsFunc`, `clsMonsterAttackSim`, `modExpPerHour` for
`As Integer|Long|Single|Double|Currency|Byte|Variant`, `CInt|CLng|CSng|CDbl|Fix|Int|Round|Val|Abs|
Sgn|Sqr|Log|Exp|Rnd|Randomize|Mod|\\|IIf|Format`.

## Do
1. `fixtures/vb6-semantics.json`: cases `{op, args, result | error}` from VB6's documented
   behaviour, at least: `CInt`/`CLng` banker's rounding (0.5, 1.5, 2.5, -0.5, -1.5), `Int` vs `Fix` on
   negatives, `\` (rounds both operands with banker's rounding first, then truncates), `Mod` (same
   operand rounding, sign of the dividend), `Round(x, n)` banker's, implicit assignment of Double to
   Integer/Long (banker's), Integer overflow at 32768 and Long at 2^31 (error 6), Integer arithmetic
   that overflows in an intermediate (`Integer * Integer` is Integer), Single precision results,
   `Val` on prefixes and spaces, `True = -1` and comparisons yielding -1/0, `And`/`Or`/`Not` bitwise on
   integers, `/` always Double, `^` Double, string `&` with numbers. Add any other op the grep shows.
2. `Rnd`/`Randomize`: VB6's generator is the 24-bit LCG
   `x = (x * 0x43FD43FD + 0xC39EC3) & 0xFFFFFF`, result `x / 2^24`, with documented seed handling for
   `Randomize n` and `Rnd(negative)`. Put sequences for fixed seeds in the table so simulations can
   match MME with a fixed seed.
3. `touchstone-vb6`: typed helpers (`cint`, `clng`, `fix`, `int`, `idiv`, `vmod`, `round`, `val`,
   `vbool`, checked Integer/Long arithmetic returning `Result<_, Vb6Error>`, `Rnd` struct) with rustdoc
   explaining each rule. A test runs every table case.

## Done when
Workspace checks pass and every case in the table is exercised by the crate's test.
