# CLAUDE.md

touchstone ports MMUD Explorer (MME, a VB6 Windows app) to a Rust server with a Svelte web UI.
Read `docs/design.md` (architecture), `docs/testing.md` (fixtures and the oracle) and
`docs/plan.md` (phases, how agents work) before a card. Cards are in `tasks/`.

## Hard rules

1. **Never commit MME's source or data, or anything from `jnfisher/silvermere-dev`.** No `.bas`,
   `.frm`, `.cls`, `.frx`, `.vbp`, `.mdb`, converted databases, C source or headers.
   `tools/guard.sh` (run by CI) enforces the file types. MME source and data are fetched at the
   pinned commit into `~/.cache/touchstone/`.
2. **silvermere-dev is reference only.** Describe game behaviour in your own words. Never name its
   files, functions or variables, and never quote it.
3. **MME may be named, not pasted.** Comments may name the MME procedure a function ports
   (`// port of modMMudFunc CalcHitChance`). Do not paste VB code blocks; a short expression to
   explain an odd rounding is fine.
4. **Parity over correctness.** Reproduce MME's answer, including its rounding and quirks (use
   `touchstone-vb6`). If the game differs, note it in `docs/differences.md`; do not "fix" it.
5. **No manual testing.** Never ask John to run MME, Windows, or check anything by hand, and do not
   leave TODOs that need a human. An unresolved mismatch goes in `fixtures/known_gaps.json` with
   the case and your best explanation.
6. **Read MME through the index.** Run `tools/mme-fetch.sh` and `tools/mme-index`, then read the
   per-procedure files the card names. Never Read `frmMain.frm` whole (1.6 MB); MME's `.frm`/`.bas`
   files are Windows-1252, so read them with `iconv -f cp1252` or through the index.
7. **CI stays under 5 minutes.** No service containers, Docker, cross-compiles or the oracle in
   PR CI. Slow things go in manual or release workflows.
8. **Stay in the card.** Touch only the files the card's `touches` lists, plus tests and docs for
   them. Something out of scope goes in the PR body under `## Follow-ups`.

## Commands

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
tools/guard.sh
(cd web && npm ci && npm run check && npm test && npm run build)
tools/mme-fetch.sh            # MME source at the pin -> ~/.cache/touchstone/mme/<sha>/
tools/mdb-fetch.sh            # stock .mdb -> ~/.cache/touchstone/mdb/<sha256>.sqlite (needs mdbtools)
python3 -I tools/mme-index/mme_index.py
python3 -I tools/oracle/oracle.py ...
```

## Conventions

- Rust edition 2024, stable toolchain pinned in `rust-toolchain.toml`. `unsafe` is forbidden.
  Library crates use `thiserror`, the binary uses `anyhow`. Third-party versions live once in
  `[workspace.dependencies]`; crates use `name.workspace = true`.
- Dependencies point downward: `model <- vb6 <- rules`, `model <- data`,
  `{rules, data} <- bin/touchstone`. `model`, `vb6` and `rules` do no IO.
- Each crate's integration tests are modules of one binary, `tests/all.rs`.
- A test that pins MME behaviour says on the line above where the expected value came from (the
  fixture file, or the MME procedure).
- Tests are deterministic: seeded RNGs only.
- Python tools are stdlib only and run with `python3 -I`.
- Web: TypeScript strict, Svelte 5 runes, Tailwind, shadcn-svelte components, AG Grid Community
  through the shared grid wrapper. All colours come from theme tokens (dark and light).
- Comments say why, not what.

## Git

`main` is protected by convention: changes arrive as PRs from `agent/T-NNN-slug` branches, which
the orchestrator merges with squash. Card agents push their branch and stop; they do not open or
merge PRs and do not edit `tasks/STATUS.md`.
