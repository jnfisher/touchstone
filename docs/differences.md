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

- The Monsters list and pane start with MME's start-up damage arrays (nothing calculated) until the Options menu's
  calculation fills them (see "Monster damage actions (P4-30)").
- The Monsters list and pane use `settings.json`'s simulation rounds (500 by default) where the oracle ran 1000, and
  run `CalculateAverageLairs` first as MME does while starting, so a damage limit that falls between simulated values
  can list a monster the oracle did not.
- "Reachable only" is accepted but changes nothing until the reachability module is ported.

## Monster damage actions (P4-30)

- The server keeps the arrays, the stored defense description and the party flag in one store tagged with the load number
  of the data, so a reload starts from "nothing calculated" (MME clears them when it reloads its database). The page asks
  the server to clear them when a character is loaded, pasted, started new, imported or deleted, as MME's `LoadCharacter`
  and paste do; an edit keeps them and the pane's "sims may be stale" note and the lair-mode question compare the stored
  description with the current one, as in MME.
- Every calculation starts the simulation generator at its initial state (MME's one `Rnd` runs on for the whole session),
  so the same request gives the same numbers, and a second calculation in a session differs from MME's second run. The
  fixtures pin the first calculation of a session.
- A calculation uses the saved settings' simulation rounds (MME's `nGlobalMonsterSimRounds`, clamped to 100..10000) and has
  no Cancel (MME's progress form sets `bMapCancelFind`); its progress is the number of monsters done.
- `bDontPromptCalcCharMonsterDamage` and `bDontPromptCalcPartyMonsterDamage` are stored as the defense description / the
  party boxes (AC, DR, MR, dodge, anti-magic) at the moment of the answer, and the question is not asked while they are
  unchanged. MME resets the flags on every change, so a value changed and changed back asks again there and not here.
- The defense description uses the refreshed MR (what `RefreshMagicRes` writes into `txtCharMR`), not the saved
  character's `mr_text`, which only holds what the file stored.
- The party calculation reads only the party boxes, but the request still carries a complete character (a class and a
  race), because the lair environment it runs in is built from one.
- The monster pane writes the damage it simulated on the spot into the stored arrays and the next pane or list reads it, as
  MME's global arrays behave. The start-up lair averages (`CalculateAverageLairs`) still see the uncalculated arrays, as
  MME's do (they run before anything is calculated).
- The oracle family `monster_damage_calc` runs MME's calculation over a small melee-only Monsters table (all 1100+ stock
  monsters would take hours in the interpreter); spell attackers' simulations are pinned by `monster_glue_char`.
- The damage the pane reads (`nMonsterDamageVsDefault`) stays the fixture's invented array only in tests; the server leaves
  it uncalculated, which the table's own average answers identically.

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

## Spell tools, Spell Book and chests (P4-32)

- MME shows "Find counters" and "What casts this spell?" in its Results window; touchstone shows list dialogs whose
  lines open the records. The textblock tree window is not ported.
- The chest button fills the Items reference list in MME, sorted by chance with `SortListViewByTag`; touchstone opens a
  dialog sorted by chance (stable), so the order of equal chances may differ from the Windows ListView.
- "Copy Chest to Clipboard" copies only the chances, one per line, as MME does (the item names are sub-items it does not read).
- The Spell Book takes the character's magery from the global filter's class, not from items that grant magery.

## Reachability (P3-45)

- Edge order, and so "first found" in route ties and which gate is reported, follows the order the tables are read
  in. MME reads `Rooms`, `Spells` and `TBInfo` as DAO returns them; touchstone uses the order of the loaded tables
  (the converted database's row order for the oracle). `Monsters` is read ordered by `Number`, as MME does.
- MME shows a progress bar and a dialog if the build fails and leaves reachability uncomputed. The port cannot fail
  on loaded tables, so there is no failure path.
- `ReachIsMonsterOK` computes on demand in MME (the filter freezes the window for a few seconds). The server does the
  same for the "Reachable only" list filter; the monster pane shows "(double-click to compute)" until the user runs
  Calculate Reachability, and the header's Options menu item stands in for the double-click (the Monsters tab keeps a "Reachability computed" chip).
- The result belongs to one load: reloading the data drops it, as loading another database does in MME.
- The fixture runs on a slice of the stock database (about 5,000 rooms; the whole map is too large to commit). The
  port was also compared with the oracle on the full stock map (26,694 rooms, 87,147 edges): every edge column, the
  level breakpoints and every monster's level bands matched.

## Rooms tab (P4-13)

- The graphical map (frmMap, the cell grid, its drawing and the map buttons) is not built; the Rooms tab shows the text MME
  produces for the centre room: the title, the reference list (`MapGetRoomLoc`) and the cell tooltip text (`MapMapExits`).
  "Route to room" needs the reachability module and is not offered.
- Find (`cmdMapFindText_Click`) jumps to one hit at a time in MME. The server lists every hit up to a limit (default 100,
  at most 1000) by running the same scan; the limit is not MME's.
- The tooltip's lair numbers come from the lair cache as loaded and its damage lines from each monster's stored average
  damage ("(default)"), because nothing calculates monster damage here. MME caches its last `GetLairInfo` answer between
  rooms; the port asks afresh, which gives the same text.
- MapGetRoomLoc compares each existing row's Tag with the teleport's `map/room` text; a numeric Tag (NPC, shop, placed item,
  room spell) makes that a type mismatch that ends the procedure, so such rooms lose their command rows. Reproduced.

## Spells filters, hide record numbers, builder width (P4-33)

- `bHideRecordNumbers` is the `hide_record_numbers` key of `settings.json` (MME: `HideRecordNumbers` in settings.ini, off
  unless 1). The server reads it for the explorer lists, detail panes, the ability list, spell casts, chests and the Rooms
  tab; the Rooms tab's own checkbox is gone. The monster-heal text of the character's combat setup and the Item Manager and
  paste paths still show record numbers (they pass false; follow-up).
- The Spells list's "remove filters" call (the Spell Book's) and the Kai auto-learn switch are not endpoint options.
- The class/race builder's text is shrunk (8 to 12 px) to fit the detail pane instead of scrolling; the text is unchanged.

## Options menu and shell (P4-31)

- Not ported: Import Character from NMR, Export Character to NMR and Reload MME. The web app imports `.mmec` files from
  the Character tab and the server reloads its data on its own timer.
- Reset All Character Fields does not ask "Unload character file first (no save)?". The browser autosaves the open
  character, so a reset that stayed linked to the file would overwrite it; the port always starts a blank, unsaved
  character and leaves the stored file alone (MME's Yes answer). The other two questions are kept. MME asks the
  compare-lists question after it has already cleared the learned spells, so a Cancel there leaves half a reset; the
  port asks both questions first and changes nothing on Cancel.
- Reset All Character Fields also puts the global filter bar back to its defaults, as MME's `CharReset` does with the
  class, race, level and alignment boxes.
- Remove All Filters resets the global filter bar and the Monsters boxes, then every explorer's search box and grid
  column filters. MME's `FilterAll(False)` resets its own per-tab controls (armour types, weapon speed, the ability
  filters, ...), which the web lists do not have; it leaves the global filter alone.
- The two checked items are stored in `settings.json` (`jump_to_compare`, `shops_first`, off when absent as in
  `LoadSettings`). "Jump to Equip/Lists on Add" opens the Compare tab on the list that was added to.
- The menu shortcuts (Ctrl+P, Ctrl+D, Ctrl+J, Ctrl+F2, Ctrl+F5) are not bound: the browser owns most of them.
- Back/forward history keeps the tab and the record selected on it. MME keeps only a back stack of 50 tabs (mouse
  back button); the port adds a forward direction, Alt+Left/Alt+Right and toolbar buttons. A move replaces the browser
  history entry, so the browser's back button keeps its own list.
- The data version (`LoadInfo`'s version label, creation date, NMR version) is shown in an info popover, with the row
  counts. MME's "this database lacks features" warnings for old NMR versions are not shown.

## Quicksilver: stat soft caps (Q-007)

- Under a Quicksilver profile the creation-point panel prices a stat above the race maximum with the
  soft-cap rule (10 CP a point up to the soft cap, then the three bands), as `TRAIN STATS` does. MME's
  `RefreshCPs` prices every point above the race minimum on the one stock ladder, so the profile changes
  only the part above the maximum. With no race selected the stock ladder stands.
- A balance whose soft-cap keys all equal `Balance::NEUTRAL`'s (floor 0, floor price 10, bands 10/10/10) is
  read as "soft caps off" and prices on the stock ladder with no training maximum. In Quicksilver's own engine
  that balance would price points above the race maximum at a flat 10, which agrees with the ladder only from
  90 points above the minimum; the profile prefers MME's answer so the identity harness stays exact.
- `cp_cost_for_raise` is unchanged: its only caller (`compute_avg_level_max_stats`) never raises past the race
  maximum. `stats::train_cost`, `soft_cap` and `train_max` are the Quicksilver functions; the character editor
  does not use `train_max` yet.
