# Plan

## Phases

| Phase | Goal | Cards |
|---|---|---|
| 0 | Repository, CI, fetch tools, MME index | T-001, T-002 |
| 1 | Data: model, `.mdb` loader, live silvermere adapter, tables API, feature inventory | T-003, T-010 to T-013 |
| 2 | Oracle: VB6 semantics, interpreter, shims, first fixtures | T-020 to T-022 |
| 3 | Formulas ported against fixtures, one card per family | written after T-003 and T-022 |
| 4 | Web UI: shell, explorers, character, compare, calculators, exp/hr | written after T-013 |
| 5 | Release: aarch64/x86_64 musl binaries, systemd unit, Pi install doc | T-050 |
| 6 | Optional: map, pathing, reachability, live game state | decided at the end of phase 4 |

The phase 3 and 4 cards come from `docs/inventory.md` section 8 (T-003), which maps every MME
screen and calculation to the procedures that implement it.

## Waves

1. T-001, T-002
2. T-003, T-010, T-020
3. T-011, T-012, T-021
4. T-013, T-022
5. Phase 3 starts with the families that need the fewest shims (P3-13a/b monster attack
   simulator, P3-01 exp tables, P3-20 exp/hr Model D). In parallel, phase 4 starts with the UI
   that needs only the tables API: P4-02a shell, P4-04 grid wrapper, P4-03 data source, and the
   explorers showing raw columns; computed columns are added when their phase 3 cards merge.

## How agents work

- **Orchestrator** (Opus, one long-lived cloud session): writes and updates cards, starts card
  agents, reviews, merges, keeps `tasks/STATUS.md`, triages `fixtures/known_gaps.json`. It is the
  only one that merges to `main`.
- **Card agents** (Sonnet by default, Haiku for transcription a script can verify): one card
  each, in their own git worktree, on branch `agent/T-NNN-slug`. They finish with the card's
  "Done when" checks passing locally and the branch pushed.
- **Merging:** the orchestrator opens the PR. Cards with a mechanical check (fixtures, counts,
  tests) merge when CI is green. Cards that decide semantics (the oracle, rules where the fixtures
  do not cover a branch) also get an orchestrator diff review.
- A card agent that cannot finish pushes what it has and writes a `## Blocked` section in the PR
  body saying exactly why. The orchestrator re-scopes the card or escalates it to Opus.
- John is not asked to test anything by hand. Questions go to John only for product choices.

## Token discipline

- Agents read MME through `tools/mme-index` output (one file per procedure, plus an index), never
  `frmMain.frm` whole (1.6 MB).
- Cards name the files and procedures to read, so agents do not explore.
- Fixtures are the acceptance test: an agent iterates against `cargo test`, not against review.
