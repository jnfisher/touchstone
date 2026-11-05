---
id: P4-04
title: "Grid polish: column sizing, copy, find-as-you-type"
model: sonnet
phase: 4
depends: [P4-00]
touches:
  - web/src/lib/grid/
  - web/src/lib/sections.ts
---

## Goal
The shared DataGrid behaves like MME's lists and reads cleanly.

## Do
1. Column widths fit header and content (no truncated headers or numbers such as "3,000,..."):
   size to content on first data load per section, keep user resizes in localStorage per section.
2. Copy: Ctrl+C copies selected rows as tab-separated text with headers (MME's copy-to-clipboard).
3. Find-as-you-type on the name column (MME's find box), right arrow jumps to the next match.
4. Row colouring hook (MME colours rows by state, e.g. not in game); themed tokens only.
5. Vitest for the sizing and copy helpers; Playwright smoke screenshot per tab (not in CI).

## Done when
`npm run check`, `npm test`, `npm run build` pass; screenshots of Monsters and Weapons show no
truncated headers or values at 1600 px width.
