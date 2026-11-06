# Differences from MME

Places where the port reproduces MME's answer although the game (or MME's runtime) would differ.

## Paste party (P3-41)

- The party pattern's `\s` and `\d` are ASCII classes, as in the oracle's regex engine. VBScript's `\s` also
  matches non-breaking and other Unicode spaces, so a paste that separates fields with those would parse
  differently in MME itself. Pasted terminal text does not contain them.
- Weapons that cast (ability 43) are not exercised: their cast text needs the spell-text tables and
  `CalculateAttack` is fixture-tested with them separately. `parse_paste_party` passes empty text tables.

## .mmec import and export (P4-24)

- Values longer than `GetPrivateProfileString`'s 255-character buffer are not cut. MME silently loses the
  tail of a long `IM_CARRIED` or `IM_STASH` (more than about 36 rows) when it loads; touchstone keeps them.
- `LoadCharacter` ignores the `MR` key (`RefreshMagicRes` recomputes it). touchstone keeps MR as the
  character's `mr_text`, so import carries the file's value and export writes it back.
- An alignment outside 0 to 3 stops MME's load with run-time error 380; import uses 0 instead.
- MME opens a different database when the file's `DataFile` differs from the open one. touchstone keeps
  `DataFile` and `DataFileVer` as extra keys and never switches.
- Item Manager rows are kept when the item exists; MME also drops items that are not gettable
  (`ObtainedFromIsGettable` is not ported).
- A native touchstone character has no MonsterFilter or Compare data, so export leaves those sections out;
  MME fills them with its defaults on load. Sections are separated by a blank line, as
  `WritePrivateProfileString` appends them (assumed; no MME-written sample was available).

## EQ tab (P4-17)

- A two-handed weapon (weapon type 1 or 3) does not clear the off-hand. In the game a two-hander
  excludes a shield; MME keeps both items equipped and only paints the weapon and off-hand boxes red.
  The port does the same and reports the flag.
- Taking an item off through the equip toggle does not mark the character as changed (`bPromptSave`),
  although equipping does. The rules report `changed = false` for it.
- A wrist item (worn type 14) and a finger item (4 or 13) can go in either of their two slots, but
  the armour explorer's wrist filter also accepts worn type 17 while the equipment lists never offer it.

## Reference rows (P3-31)

- ListView sorting is not ported (the grid sorts). `GetLocations` returns rows in the order MME adds them
  plus `LocationList::sorts`, the row count at each `SortListView` request; sorting the first n rows by
  their first cell at those moments reproduces MME's order. How the ListView control compares text
  (case, locale) is not pinned by any fixture.
- Positions in `GetLocations` are Integers in MME and would overflow past 32,767 characters; the port uses
  32-bit positions. No stock text is that long (the longest "Summoned By" is 20,000).
- The oracle answers `GetLairInfo` from invented numbers on the real group indexes, so the lair rows'
  exp and damage text is checked for layout, not for the values the lair cache would give.
## List filters (P3-29)

- MME lists records in the table's physical order and the grid sorts them afterwards. `touchstone_rules::filters`
  returns records in the order of the loaded world (by record number), and its fixture test compares lists in
  record-number order; only the order of a record's own rows (the attack kinds of maximum effort) is kept.
- `SetupClass` (run by `RefreshAll` when the class changes) ticks the "non-magical only" boxes of the weapons and
  armour tabs for an anti-magic class. That is a control refresh, not a filter: the filters take the boxes as
  inputs, and the fixture reads the boxes back after `RefreshAll`.
- `FilterWeapons` asks with message boxes. The port takes the answers from a callback; the evaluate endpoint
  answers yes to every question (a backstab for a class without stealth is calculated anyway, as MME does when
  the user confirms), so its backstab rows for such a class are the forced calculation.
- `FilterSpells` and `FilterMonsters` are not part of `filter_all` (card P3-30), nor is the `RefreshAll` that
  `FilterAll` ends with.

## List filters, spells and monsters (P3-30)

- MME lists spells in the table's physical order, which is not by number for some rows; the port lists by number and the
  fixture test compares spell lists sorted. Monsters are physically in number order, so their lists are compared in order
  (the no-character exp/hr filter overwrites the profile's HP for the rest of the pass, so order matters there).
- `FilterMonsters` asks, in lair mode, whether to calculate the monster damage first. The port takes the answer from a
  callback and reports a Yes (`recalc`) instead of running `CalculateMonsterDamageVsCharALL`: the caller owns the damage
  caches. `/api/evaluate/monsters` never asks (the prompt state says "already asked") and lists solo mode with the default
  filters; the controls, lair mode and the extra filters are P4-08/P4-09.
- Reachability (`modReachability`) is a trait; the default says every monster is reachable, which is also MME's answer
  when nothing is computed. The fixture scripts a rule (multiples of 7 are out of reach below level 30) to pin the plumbing.
- `/api/evaluate/monsters` with no character uses a placeholder first-class, first-race level-1 character to build the
  damage environment; the list itself is made for "no character" (the global filter off), as MME does. The
  `nMonsterDamageVs*` caches start uncalculated, so damage is the Monsters table's average until a calculation fills them.
- In the fixture's database a Monsters table of whole lairs stands for the stock one (`db_only`): MME's `GetDamageOutput`
  moves the cursor to the first record when a monster is missing, which restarts `FilterMonsters`' own loop forever. A
  missing mob cannot happen in a real database; the port does not model it.

## Item Manager (P4-19)

- `LV_CopySelectedActionsToClipboard` builds its text in memory; a flag with a quantity near 2 billion would
  make VB6 run out of memory (error 7) long before it finished. The port returns error 7 past one million
  commands instead of allocating gigabytes.
- The Item Manager request takes at most 2000 rows and 64 answers; MME's ListView has no limit but the list
  holds what one character carries, stashes or sees.
- `SaveCharacter` keeps the saved quantity in an Integer, so a CARRIED/STASH row with a quantity past 32767 is
  run-time error 6 there. The Item Manager and paste endpoints leave the saved `carried`/`stash` lists as they
  were for such a list and say why in `messages`; the rows themselves still change.
- The selection is the page's state and `lvItemManager_ItemClick` (the detail pane) is not ported: delete reports
  the row MME selects next.

## Monster Attack Simulator window (P4-28b)

- MME calls `Randomize` at the top of `RunSim`, so every run in MME rolls differently. touchstone seeds the
  generator: the default seed is 327 680 (the state VB6 starts in before any `Randomize`), a request may give
  its own (low 24 bits used), and the same request always answers the same. Averages therefore vary with the
  seed by the same amount they vary between MME runs.
- The form fixes the combat log at 100 rounds; the endpoint takes `log_rounds` (0 to 1000, default 100).
- Vile ward is GreaterMUD-only. The endpoint accepts and ignores it, as the form hides the box off GreaterMUD.
- The party defenses (lair filter party of two or more) are not offered; the window runs for one character.

## Item detail (P3-35)

- The ListView rows `GetAbilityStats` and `PullSpellEQ` add (spell, summon, textblock and teleport rows) are walked in
  `detail/item.rs` for rows only. The fixture exercises spell, summon and textblock rows; the
  teleport branch (spells with both a room and a map ability) is ported from the same MME code but no stock item casts
  such a spell, so it is not exercised by the fixture.
- ListView sorting is not run: the row counts of each `SortListView` / `LV_RefreshSort` request are returned.
- GreaterMUD and Paramud branches (aura-item AC/DR line, cast-on-kill percent, Ganghouse value) are not ported.

## Detail panes (P4-DP)

- Reference lists are sorted by the server (`/api/detail`): each `SortListView` request sorts the rows held at that
  moment by first-column text, ignoring case; an item's final refresh sorts by the first column's tag, highest first
  (shop rows ahead with Shops First). Rows with equal keys keep their order; MME's ListView leaves that order undefined.
- Lair rows in reference lists use the lair cache as loaded (mobs, spawn chance, average exp). The damage figures
  ("Dmg Out", clear-round damage) are what a monster-tab calculation stores in the cache, so they read as zero here
  until the monster explorer computes them.
- The weapon list's per-row attack type (the row tag MME passes as `nAttackTypeMUD`) is not on the evaluate wire;
  the item pane takes it as `attack_type` and the page leaves it unset (normal attack).
- A character used through the global filter bar always counts as `chkGlobalFilter` on, whatever flag it was saved with.

## Monsters explorer (P4-08)

- The server has no "calculate monster damage" action, so the Monsters list and pane run with MME's start-up damage
  arrays (nothing calculated). Panes of monsters MME had calculated beforehand read differently there, and the party
  wording "vs party defenses" (which needs `bMonsterDamageVsPartyCalculated`) never appears.
- The Monsters list and pane use `settings.json`'s simulation rounds (500 by default) where the oracle ran 1000, and
  run `CalculateAverageLairs` first as MME does while starting, so a damage limit that falls between simulated values
  can list a monster the oracle did not.
- "Reachable only" is accepted but changes nothing until the reachability module is ported.

## Find Best (P4-18)

- MME's equipment combos are `Sorted`, and a tie in Find Best (same score, same encumbrance ratio) goes to the
  first entry in that order. The server orders each slot's list by the lower-cased text "Name (number)", which
  approximates the Windows sort (it differs on punctuation and accents); the oracle's combos keep table order,
  so the fixture pins "first in the given list wins" rather than the Windows ordering.
- MME's Find Best never marks the character as changed (it runs with `bDontRefresh` on); the port does the same,
  so the web client does not flag an unsaved change differently from other equipment edits.
- VileWard (armour entry 6) is GreaterMUD and Paramud only; on the stock profile the entry is not offered and a
  replay of it does nothing but record the criterion for Next Best.
- The "last find best" values of each slot are not kept between clicks in MME either: Next Best recomputes them
  from the worn items. Only the last menu entry and the exclusion list persist.
