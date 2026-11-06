---
id: Q-015
title: Web: show the profile
model: sonnet
phase: 6
depends: [Q-004, Q-014]
touches:
  - web/src/                  # header badge, info popover section, Monsters list notice, small shared helpers
  - web/scripts/              # a local screenshot helper (not run in CI)
---

## Goal
Row Q-015 of `docs/quicksilver-profile.md` part 5. When the loaded database is Quicksilver the page says so: a
header badge ("Quicksilver 1.20.0" or "Stock", tooltip naming balance overrides), a "Rules profile" section in
the info popover, and the exp/hr model notice once on the Monsters list when the profile hides Models A to C.

## Read
- `web/src/lib/info.ts`, `components/InfoPopover.svelte`, `routes/+layout.svelte`
- `web/src/lib/settings.ts` (`shownModels`, `QUICKSILVER_MODEL_NOTE`), `components/settings/SettingsDialog.svelte`
- `web/src/lib/components/character/VitalsPanel.svelte` (the Quicksilver "Rest Ticks" caption line)

## Do
1. Pure helpers in `info.ts` (`isQuicksilver`, `profileBadge`, `profileBadgeTitle`, `profileLines`), unit tested
   under both profiles with `/api/info` mocked as the other tests do.
2. `ProfileBadge.svelte` in the header next to the info button; `InfoPopover` gains the "Rules profile" section.
3. A one-line notice above the Monsters list when `shownModels` differs from the stored block.
4. Stock-only assumptions: the mana caption is CRLF text that the Vitals tile put in one div, so the extra
   "Rest Ticks" line ran into its neighbours; render the lines as lines. No web parser reads the "Weave:" or
   "Opportunist:" fragments (they are plain pane text).
5. Screenshots with `web/scripts/profile-shots.cjs` into `/tmp/Q-015/shots/`.

## Done when
Workspace and web checks pass; `qs-badge.png`, `qs-monster.png` and `stock-badge.png` exist.
