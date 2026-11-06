# Testing

Parity with MME is checked against **golden fixtures**: JSON files of inputs and the outputs
MME's own code produces for them. The Rust tests only read fixtures, so they need no network,
no Windows, no MME and no servers, and the whole suite runs in seconds.

## Layers

1. **Unit and property tests** in each crate (`proptest` for numeric rules).
2. **VB6 semantics table.** `fixtures/vb6-semantics.json` lists VB6 operations and their
   documented results (`CInt(2.5) = 2`, `Int(-2.5) = -3`, `5.5 \ 2 = 3`, Integer overflow at
   32768 raises error 6, ...). Both the oracle and `touchstone-vb6` are tested against it.
3. **Formula fixtures.** `fixtures/rules/<family>.json`: produced by the oracle running MME's
   unmodified modules. `touchstone-rules` must reproduce every case. Integers match exactly;
   a `Double` matches within the tolerance written in the fixture header (default exact).
4. **Data fixtures.** `fixtures/data/*.json`: record counts and sampled records from the stock
   `.mdb`, which the loader must reproduce. The silvermere adapter is tested against a small
   SQLite file built in the test from vendored silvermere migrations.
5. **Web.** `svelte-check` and Vitest for logic; no browser end-to-end in CI.

## The oracle

`tools/oracle/` is a Python (stdlib only, run with `python3 -I`) interpreter for the subset of
VB6 that MME's formula modules use. It loads MME's `.bas`/`.cls` files from the pinned commit
unchanged, with shims for DAO recordsets (over the converted stock database), global variables
and the few form controls the formulas read. A scenario file sets the globals (a character), names
the procedure and arguments, and the oracle writes the result into a fixture.

Because the oracle runs MME's code rather than a re-reading of it, fixtures can be regenerated
whenever the MME pin moves, and the fixture diff shows exactly which answers changed.

Generating fixtures is a development step, run in a cloud session and committed. It is never
part of PR CI. A manual workflow (`oracle-check`, `workflow_dispatch`) regenerates everything and
fails if the committed fixtures are stale.

## Quicksilver vectors and the identity harness

The Quicksilver profile has no MME oracle (see `docs/quicksilver-profile.md` part 4). Two things stand
in for it:

- **Engine vectors.** `tools/qs-vectors/run.sh` links Quicksilver's own rule crates (checkout named by
  `QUICKSILVER_DEV`, read-only; build directory `QS_VECTORS_TARGET`, default
  `~/.cache/touchstone/qs-vectors/`) and writes `fixtures/quicksilver/<family>.json`: inputs, the engine's
  answer under the 1.20.0 balance (`v120`) and under stock-equivalent settings (`neutral`). `run.sh --check`
  fails if a committed file is stale. Like the oracle it is a development step, never part of PR CI.
  `tests/all/quicksilver_vectors.rs` screens each `neutral` column against the stock MME function; the
  rule's own card checks the `v120` column.
- **Identity harness.** `tests/all/quicksilver_identity.rs` runs the MME fixtures through
  `Profile::Quicksilver(&Balance::NEUTRAL)` and expects the MME answers. A card that makes a family
  parametric adds that family's fixture test there. The `weave` family (Q-013a) has no MME counterpart: `tests/all/quicksilver_weave.rs` checks its `v120` column through
  `quicksilver::weave` and that Stock and NEUTRAL weave nothing. The `weave_round` family (Q-013b) composes the engine's blow
  resolution (the share of landed blows that deal damage) with the `weave` proc figures, because the balance tool has no closed per-round
  figure; `tests/all/quicksilver_weave_round.rs` checks it and the wiring through `get_damage_output`. The `exphr_d` family (Q-014) has no closed engine figure for a whole exp/hr; it cross-checks Model D's Quicksilver recovery
  (mana exact over a 210-second cycle, the time to restore both bars within a stated tick allowance) against the engine's rest, meditate
  and slow ticks, and `tests/all/quicksilver_exphr.rs` checks it with the model gate and the dispatch. It covers the stats, accuracy, attack, char_panel, spell_cast, character and exp/hr (`exphr_*`, `exp_dispatch`) fixtures (and more, as cards
  add them); the spell_cast run gives the caster a casting stat, class and level so the neutral balance has something to undo.

## No manual verification

Nobody runs MME on Windows to check results. If the oracle and the port disagree and the cause
cannot be found, the case goes in `fixtures/known_gaps.json` with an explanation, and the
orchestrator triages it.

## CI budget

PR CI must stay under 5 minutes. It runs fmt, clippy, tests and cargo-deny in one Rust job, and
svelte-check, Vitest and the web build in a web job that runs only when `web/` changes. No
service containers, no Docker build, no cross-compiling and no oracle on PRs. Each crate's
integration tests are one binary (`tests/all.rs`). Anything slower runs in a manual or release
workflow.
