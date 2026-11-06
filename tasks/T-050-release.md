---
id: T-050
title: Release workflow and Raspberry Pi packaging
model: sonnet
phase: 5
depends: [T-013]
touches:
  - .github/workflows/release.yml
  - packaging/
  - docs/install-pi.md
  - README.md
---

## Goal
Pushing a `v*` tag (or a `workflow_dispatch` dry run) builds `aarch64-unknown-linux-musl` and
`x86_64-unknown-linux-musl` archives with the web UI embedded plus a `SHA256SUMS` file; a tag also
publishes them as a GitHub release. `docs/install-pi.md` gets touchstone running on a Raspberry Pi 4
beside a silvermere/Quicksilver server, reading its SQLite database read-only.

## Read
`docs/design.md`, `bin/touchstone/src/web.rs` (the `embed-web` feature), `bin/touchstone/src/config.rs`,
`crates/touchstone-data/src/silvermere/mod.rs` (how the game database is opened), `.github/workflows/ci.yml`.

## Do
Decisions made:
1. `release.yml`: `web` job builds `web/build` once and uploads it; a `build` matrix job per target
   uses `cargo-zigbuild` 0.24.1 with `ziglang` 0.15.1 (pip), `--release --locked --features embed-web`,
   and archives `touchstone-<version>-<target>.tar.gz` (binary, example config, systemd unit,
   `packaging/README.txt`; no LICENSE, the repo has none). Older zig (0.13) and cargo-zigbuild 0.20
   fail on aarch64 with an unsupported `--fix-cortex-a53-843419` linker argument; keep the pair
   pinned. The `release` job (the only one with `contents: write`) writes `SHA256SUMS` and publishes
   with `softprops/action-gh-release` v2.6.2 on tags only. A manual run is a dry run
   (`dev-<sha>` version, archives uploaded as a run artifact). `--version` already exists (clap).
2. `packaging/touchstone.service`: dedicated `touchstone` user in the game's group;
   `ProtectSystem=strict`, `ReadOnlyPaths=-/srv/mud`, `StateDirectory=touchstone`, hardening, restart
   on failure, ordered after the game. SQLite rules, tested with a non-owner user: a read-only WAL
   reader needs only read on the db, `-wal` and `-shm` (a read-only directory is fine) while the
   game runs; `-wal` at mode 0600 fails; with the game stopped (no `-wal`/`-shm`) it needs write on
   the directory. Documented in the install guide.
3. `docs/install-pi.md` and a README install section; example config points at a Quicksilver-style
   path and `/var/lib/touchstone`.

## Done when
`cargo build --release --features embed-web` serves `/` and `/api/info` from memory against the stock
db; both musl targets build with `cargo zigbuild`; the aarch64 binary reports `--version` and serves
under `qemu-aarch64`; `actionlint` passes on the workflow; workspace and web checks pass.
