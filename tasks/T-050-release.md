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
---

## Goal
Pushing a `v*` tag publishes `aarch64-unknown-linux-musl` and `x86_64-unknown-linux-musl` archives
with the web UI embedded, and `docs/install-pi.md` gets touchstone running beside Quicksilver.

## Do
1. `release.yml`: build the web once, then the two targets (`cargo-zigbuild` or `cross`), with
   `--features embed-web`, archives with SHA256SUMS, GitHub release.
2. `packaging/touchstone.service` (systemd, runs as the game's user or a user in its group,
   read-only access to the game's data directory, `ProtectSystem=strict`), example config pointing
   at the Quicksilver database.
3. `docs/install-pi.md`: download, verify, install, configure, enable, update; `mdbtools` only if
   loading a `.mdb`.

## Done when
A tag on a test branch (or `workflow_dispatch` dry run) produces both archives; the aarch64 binary
reports `--version` under `qemu-aarch64` if available.
