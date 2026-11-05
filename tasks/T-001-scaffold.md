---
id: T-001
title: Scaffold the workspace, web app and CI
model: sonnet
phase: 0
depends: []
touches:
  - Cargo.toml
  - rust-toolchain.toml
  - deny.toml
  - .gitignore
  - .gitattributes
  - crates/
  - bin/
  - web/
  - tools/guard.sh
  - .github/
---

## Goal
A building, tested, empty skeleton of everything in `docs/design.md` "Crates and layout", with
PR CI under 5 minutes.

## Read
`CLAUDE.md`, `docs/design.md`, `docs/testing.md` ("CI budget"). For conventions to copy, the public
`jnfisher/silvermere` repository's `Cargo.toml`, `deny.toml`, `rust-toolchain.toml`,
`.github/workflows/ci.yml` (only to see the style; do not copy its Postgres, Docker or stock-data
steps).

## Do
1. Cargo workspace (resolver 3, edition 2024, `publish = false`, `[workspace.lints.rust]
   unsafe_code = "forbid"`, every crate opts in with `[lints] workspace = true`). Members:
   `crates/touchstone-{model,vb6,rules,data}` and `bin/touchstone`. Third-party versions only in
   `[workspace.dependencies]`. No `license` field (the project has none yet).
2. `rust-toolchain.toml`: pin the newest stable rustup can fetch here (silvermere pins 1.99.1),
   components rustfmt and clippy, profile minimal.
3. Each library: `lib.rs` opening with a doc comment naming its responsibility (from design.md),
   `tests/all.rs` with one smoke test. Respect the dependency direction in CLAUDE.md.
4. `bin/touchstone`: clap CLI `touchstone serve [--config PATH] [--bind ADDR]` (default bind
   `0.0.0.0:8370`), TOML config struct (serde) with `bind`, `data_dir`, and a `[source]` table to be
   filled later. axum + tokio. `GET /api/health` returns `{"ok":true,"version":...}`. Static files:
   with feature `embed-web` the UI is embedded from `web/build` with `rust-embed`; without it,
   `web/build` is served from disk if it exists, and unknown non-`/api` paths fall back to
   `index.html` (SPA). A test hits `/api/health` with `tower::ServiceExt::oneshot`.
5. `web/`: SvelteKit + `@sveltejs/adapter-static` (SPA: `fallback: 'index.html'`, `ssr = false`),
   TypeScript strict, Svelte 5, Tailwind CSS, shadcn-svelte initialised (add button, tabs,
   dropdown-menu, tooltip). AG Grid Community wrapped once as `src/lib/grid/DataGrid.svelte`
   (props: rows, column defs; themed from the app's CSS variables, dark and light). App shell:
   header with title and a dark/light toggle (dark by default, choice kept in localStorage), a tab
   bar with the MME sections as placeholder routes (Weapons, Armour, Spells, Monsters, Shops, Items,
   Races, Classes, Rooms, Character, Compare) and F1 to F11 shortcuts. One placeholder page shows a
   DataGrid of 1000 generated rows to prove virtual scrolling. Vite dev server proxies `/api` to
   `127.0.0.1:8370`. npm scripts: `dev`, `build`, `check` (svelte-check), `test` (Vitest, one test of
   the theme store). Commit `package-lock.json`.
6. `tools/guard.sh`: fails if any tracked file has extension `.bas .frm .cls .frx .ctl .ctx .vbp
   .mdb .accdb .sqlite .db .c .h` (case-insensitive), listing them. Exit 0 otherwise.
7. `.gitignore`: `target/`, `web/node_modules/`, `web/build/`, `web/.svelte-kit/`, `*.mdb`,
   `*.sqlite`, `/touchstone.toml`, `/data/`. `.gitattributes`: LF for text sources.
8. `deny.toml` (cargo-deny v2): advisories deny yanked; licenses allow MIT, Apache-2.0,
   BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0, CDLA-Permissive-2.0 and whatever else
   the graph genuinely needs; `[licenses.private] ignore = true` so unpublished workspace crates
   without a license pass.
9. CI, `.github/workflows/`:
   - `ci.yml` on `pull_request` (to `main`) and `workflow_dispatch` only (no push trigger),
     `concurrency` cancel-in-progress, `permissions: contents: read`. Use `dorny/paths-filter` (or
     equivalent) so the `rust` job runs only when Rust, fixtures, tools or workflow files change
     and `web` only when `web/` changes. Jobs:
     - `guard`: `tools/guard.sh` (always).
     - `rust`: checkout, `Swatinem/rust-cache`, `taiki-e/install-action` for cargo-deny, then fmt,
       clippy `-D warnings`, `cargo test --workspace --locked`, `cargo deny check`. Env
       `CARGO_PROFILE_DEV_DEBUG=0`. `timeout-minutes: 15`.
     - `web`: setup-node 22 with npm cache, `npm ci`, `check`, `test`, `build`. `timeout-minutes: 10`.
   - `.github/pull_request_template.md` with sections: What this changes / Card / How it was
     checked / Follow-ups / Blocked.

## Done when
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `cargo deny check` and `tools/guard.sh` pass.
- `(cd web && npm ci && npm run check && npm test && npm run build)` passes, then
  `cargo run -p touchstone -- serve` serves the built UI at `/` and `/api/health` answers (check
  with curl, then stop it).
- `cargo build -p touchstone --features embed-web` builds.
- Workflow YAML parses (`python3 -c "import yaml"` is not available stdlib-only; use `actionlint`
  if installable, otherwise review by eye).
