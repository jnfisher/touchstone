---
id: R-003
title: Calculators and simulations on realm characters
model: sonnet
phase: realm
depends: [R-002]
touches:
  - web/src/lib/components/calc/
  - web/src/lib/components/MonsterFilters.svelte
  - web/src/lib/components/MonsterStatus.svelte
  - web/src/lib/components/OptionsMenu.svelte
  - web/src/lib/components/paste/
  - web/src/lib/monsterdamage.svelte.ts
  - web/src/lib/monsterdamage.ts
  - web/src/lib/partyrealm.ts
  - web/src/lib/options.svelte.ts
  - web/src/lib/character.svelte.ts
  - bin/touchstone/src/
  - docs/differences.md
---

## Goal
Every calculator and simulation runs on an actual realm character. The header's active character drives all of
them by default; each calculator also has a small "use another" dropdown that overrides it for that calculator
only, and the party simulations take a multi-select of realm characters. A realm character is read-only:
calculator-local inputs can still be tweaked, nothing is written back to the character.

## Read
- `tasks/R-002-*.md`, `web/src/lib/character.svelte.ts` (the facade over sandbox and the active realm
  character), `components/character/ActivePicker.svelte`, `components/calc/current.svelte.ts`.
- `monsterdamage.svelte.ts`, `options.svelte.ts`, `components/MonsterFilters.svelte`, `paste.ts` (the party form).

## Decisions
- Consumers that read `getCharacterStore()` (the facade) already follow the active character: the hit, swing,
  backstab and coin calculators through `useCurrentCharacter`, the Monsters list and pane (`ExplorerView`),
  damage vs character, combat setup, Find Best, Compare, the spell book, the route level. What they lacked was
  saying which character they use, and the per-calculator override.
- `useCurrentCharacter` becomes a thin wrapper over `createCurrentCharacter`, which owns an optional override: a
  second read-only character store loaded from the realm's input endpoint and evaluated like any character. The
  override is per calculator instance and resets with a clear control; it re-reads when the realm reloads.
  `current.doc`/`options` give the monster simulator the same character the other calculators use.
- `UseCharacter` states the character in use (name, race, class, level, with a link to its sheet) and carries the
  "use another" dropdown (`CharacterDropdown`, the picker's list in a compact dropdown, single or multi).
- Party: MME takes a pasted party and averages its members into the Monsters party boxes. A multi-select of up
  to six realm characters (MME's party-table size) builds the same party form from each character's evaluation
  and combat preview, has the server recount the averages (`/api/paste/party`) and Continue (`/paste/party/continue`),
  and writes the resulting values to the party boxes. Pasting still works in the Paste dialog, whose Continue now
  writes the boxes too. The party size is the number of members picked.
- The damage store remembers which character (or party) the last calculation was for, so the status chip and the
  Options menu items name it. Reachability does not depend on a character, so it has nothing to follow.
- No server change: every number comes from endpoints that exist.

## Do
Store and components, the party builder, the chip and menu wording, tests, screenshots in `/tmp/R-003/shots/`.

## Done when
Workspace and web checks pass; `hit-calc.png`, `monster-sim-override.png` and `party-damage.png` exist.
