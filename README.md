# touchstone

touchstone is a web-based database viewer and combat/experience calculator for MajorMUD. It is
a port of [MMUD Explorer](https://github.com/syntax53/MMUD-Explorer) (MME) by **syntax53**, rewritten
in Rust with a Svelte web UI, so it can run headless on a small Linux box (a Raspberry Pi 4) next
to a [silvermere](https://github.com/jnfisher/silvermere) server.

It reads game data from:

- an MME/NMR `.mdb` database export, or
- a silvermere (or silvermere-derived) server's live SQLite database, read-only, including
  non-stock content.

**Status:** early development. Nothing runs yet. See [`docs/plan.md`](docs/plan.md).

## Credit and status of this project

All the game knowledge in this tool comes from MME, built over many years by syntax53 and its
contributors (and Locke Cole's work on the MajorMUD formulas). touchstone is an unofficial,
non-commercial hobby port. It is not affiliated with or endorsed by syntax53 or MME, and it will
be changed or taken down on request.

MME does not currently publish a license, so this repository does not either. Neither MME's
source nor its data files are stored here. The tools that read them download them at a pinned
version.

MajorMUD is a trademark of its owner. touchstone is not affiliated with or endorsed by its owner,
and contains no MajorMUD game files.
