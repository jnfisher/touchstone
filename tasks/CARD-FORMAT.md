# Card format

One file per card: `tasks/T-NNN-slug.md`.

```
---
id: T-NNN
title: Short imperative title
model: sonnet          # sonnet (default) | haiku | opus
phase: 1
depends: [T-001]
touches:               # the files and directories this card may change
  - crates/touchstone-model/
---

## Goal
What exists when the card is done, in two or three sentences.

## Read
The exact files, MME procedures (through the index) and docs to read. Nothing else is needed.

## Do
The steps, with the decisions already made.

## Done when
Checks an agent can run itself: commands that pass, counts, fixtures.
```

Card agents work in a git worktree on `agent/T-NNN-slug`, run the "Done when" checks and the
repository checks in CLAUDE.md, commit, push the branch and report. They do not open PRs.
