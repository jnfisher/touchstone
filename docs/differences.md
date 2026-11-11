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
- "Calc. Monster Dmg vs Party" runs with no character loaded, as in MME (its menu item depends on the lair filter's party
  size and the calculation reads only the party boxes). The server needs a class and race to build the lair environment,
  so a party request without them gets the first class and race of the data; the arrays are the same whichever is used.
  The page offers the item whatever the party size, where MME enables it only for a party of two or more.
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

## Chests tab (CH-001)

The tab (Chests, next to Items) is a touchstone addition. The chances and the sell prices are MME's
(`cmdSundryChests_Click` / `GetChestItems` and `GetItemValue`); the average sale is not an MME number.

- **Average sale of one opening** = the sum, over the chest's lines, of `chance * sell price in copper`, at the tab's
  charm (50 unless the box says otherwise). `chance` is the share `GetChestItems` leaves for the item, before MME
  rounds it to one decimal for display; a line's shown chance times its shown price can therefore differ from its
  shown expected value by a little. An item that cannot be sold adds 0.
- **Sell price** is `GetItemValue`'s sell side with no shop and no buying: the item's price in copper (its `Currency`
  column gives the coin, so a price in gold or platinum is at face value in copper), times `fix(charm / 2) + 25` percent
  when the charm is above 0, truncated. At charm 0 a shop pays the full price, as MME's function does. A shop's markup
  only moves the buy price, so it is not used.
- **"Can't be sold"** when no shop takes the item (its Obtained From has no `Shop`, `Shop(sell)` or `Shop(nogen)`
  entry, which is how touchstone knows a shop stocks or buys it) or its sell value is 0 (price 0, or a charm modifier
  that rounds it away).
- **One opening** is what the chances imply: the lines of one chest give each item independently with the chance
  shown, so an opening can give several items, and each adds its own expected value. Chests whose text rolls the same
  textblock more than once (stock chests roll up to a dozen draws) are already folded into one chance per item by
  `GetChestItems`, which MME shows the same way; the average does not draw again.
- **Currency items.** Stock chests give only items, never coins: `GetChestItems` reads `giveitem` lines and the stock
  chest textblocks have no coin-giving lines. An item priced in a coin (the `Currency` column) counts at that price;
  an item with no price counts 0. `failitem` lines (an item given when a roll fails) are not read by MME's chest
  arithmetic and so are not counted here.
- **Dropped by** counts the `Monster #n` entries in the chest's Obtained From. The droppers grid shows the Items
  pane's reference rows for the chest, sorted by chance with the highest first: monsters with their drop chance, and
  the other sources (rooms, textblocks, NPCs) with a blank chance.

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

## Quicksilver: rest and meditate regeneration (Q-008)

Under `Profile::Quicksilver` only; the stock rules are untouched.

- MME's assumption that a character rests (HP) or meditates (mana), never both, no longer holds: Quicksilver
  resting restores mana at 3 times the standing rate, meditating at 5 times and heals as resting does. The Char tab
  magic panel shows a "Rest Ticks" line (mana per 21-second rest tick) and its "Medi. Ticks" counts the mana
  regeneration buffs (MME's meditation tick does not). Both ticks are averages over the carry cycle: the game pays
  whole mana and carries the remainder, the panel shows the exact mean (a rest tick of 4.2 is 4, 4, 4, 4, 5 ...).
- `CalcManaRegen` keeps its MME meaning, `meditating` flag included. The Quicksilver payments are separate functions
  (`calc_rest_mana_regen`, `calc_meditate_mana_regen`), because MME's meditation tick has no Quicksilver counterpart
  and the neutral balance cannot reproduce it.
- `CalcManaRecoveryRounds` under Quicksilver models one mode at a time on MME's 5-second grid: meditating when the
  caller passes a meditation rate, resting otherwise (a character without the Meditate ability rests). The standing
  tick keeps its 30-second phase from the start, which the game's global slow tick does not (it may fall anywhere in
  the first 30 seconds), so a recovery time can differ from the game's by up to one tick's worth of time.
- The recovery model still ignores combat and HP: the game ends rest and meditate when both bars are full, and
  meditating needs HP below full or mana below full to start. The hit point panel is unchanged.

- Rest and meditate regeneration reads as off when a Quicksilver balance's `rest_mana_mult` and
  `meditate_mana_mult` are both `Balance::NEUTRAL`'s (1). MME's meditation tick is not a multiple of the standing
  rate, so NEUTRAL cannot express it; at those values the panel, the ticks and the recovery rounds take the stock
  route (no "Rest Ticks" line, MME's meditation tick), which keeps the identity harness exact. A real retune that
  sets both multiples to 1 therefore also reads as stock. The vector `neutral` column is the engine at multiples of
  1 (standing ticks only), which equals stock with no meditation rate.

## Quicksilver knees (Q-006)

- Under the Quicksilver profile the magic resistance and dodge knees (R12, R13) apply to the character's own
  Intellect/Willpower and Agility wherever MME calls `CalcMR` and `CalcDodge` (the character panel, the gear
  aggregation, the party paste). A monster's magic resistance and dodge are data and are not compressed.
- The crit knee (R11) is applied inside `CalcAttack`'s crit block, so MME's cap of 99 stays after it. The server has
  no such cap on the compressed value; it only matters for a crit chance above 197, far beyond the game's range.
- The "knee out of reach" values of the neutral balance are 2^31 - 1, not the game's 16-bit stat ceiling, because
  MME's formulas (and the fixtures that probe them) work in 32 bits.

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

## Quicksilver: spell damage (Q-012)

- The casting-stat bonus and the class share (R7, R8) scale what `CalculateSpellCast` adds for Inflict Damage,
  Inflict Damage on Armour and a drain that does not last, before magic resistance and elements; a drain's heal is
  scaled with it. The engine scales after the target's element resistance and, for the armour kind, before the
  monster's magic resistance, so the two can differ from MME's order by a point of rounding. Heals, buffs and lasting
  drains are not scaled. Weapon, item and monster casts never reach this function.
- The class and the casting stat come from the loaded character (the class's mana stat with gear); a party member
  pasted in uses the same. The "nobody" profile the spell detail pane uses before a character is loaded has no class
  and no stat, so it shows the unscaled amounts. MME's cast level (the level you ask the spell at) never feeds the
  class share, which reads the character's own level, as the engine does.
- The area resist (R9) is an expectation. An area spell that reaches monsters (targets Divided Area, Divided Area
  including self, Divided Attack Area, Full Area, Full Attack Area) with an Inflict Damage or Inflict Damage on Armour
  slot has its round damage multiplied by `1 - p * cut / 100`. The chance `p` is the engine's: a resist roll of
  `genrdn(1, 100)` (the values 1 to 99) at or under `min(MR / 2, 98)`, so `MR / 2` out of 99, not out of 100. The
  Min/Avg/Max cast, which are single rolls, are not cut.
- MME's own "chance to fully-resist" figure is kept as it is. A Never-resist area spell has none, so it only gets the
  cut; an Always-resist one shows both. Quicksilver's own area roll on a monster is a cut and never a stop, so for
  those the profile is a little harsher than the server; parity with MME's figure was preferred over replacing it.
- An area spell that mixes a Steal Hit Points slot with a damage slot has the whole round damage cut, not only the
  damage slot's share. Such a spell is not expected in the data.
- The divided kinds split the roll between targets in the game; MME's calculator shows one target's full share and
  the profile keeps that.

## Quicksilver: backstab damage and crit (Q-009)

- A Quicksilver stab (R1) reads the level held to `scalecap` in both level terms, divides stealth by
  `backstab_stealth_div` and multiplies by `100 + L * backstab_level_pct / 100` percent, the percent truncated to
  a whole number as the server does. The 75% cut for race-only stealth and MME's 16-bit steps stay. The backstab
  calculator, `CalculateAttack` type 4 and the detail lines that call it all follow.
- A stab can crit (R3) at the character's crit chance after the knee, with MME's cap of 99 and no Quick and Deadly
  bonus. A critical stab rolls from the stab's maximum to `backstab_crit_max_pct` percent of it (the maximum if
  that is lower); the average crit is MME's rounded midpoint minus the target's damage reduction, as for other
  crits. Swings, energy and accuracy of a stab are unchanged until Q-010.
- The neutral balance has no `scalecap` (MME's stab has none) and `backstab_crit_max_pct` 0. touchstone reads a
  crit maximum of 0 as "a stab never crits", which is MME's answer; the engine would read it as a crit that
  always deals the stab's maximum. No real configuration sets 0, so this costs nothing and keeps the identity
  harness exact on the stabs in the attack and party fixtures.

## Quicksilver: the backstab round (Q-010)

Under `Profile::Quicksilver` only; the stock rules are untouched.

- A stab (R2) costs `backstab_energy_pct` percent of one normal blow's energy with the same weapon, never more
  than the round's 1000, and the rest of the opening round is normal blows. The surprise figure that
  `get_damage_output`, the monster pane, the monster rows and the monster filters report (`surprise_damage`,
  `surprise_min_damage`) is the whole opening round: MME's stab-only number plus those blows. The stab itself
  (`calculate_attack` type 4, the backstab calculator, the per-weapon rows and the party attack panel's
  "bs dmg") stays one swing; `attack::calc_stab_opener` adds the blows after it.
- The blows after the stab are counted in whole blows, `(1000 - stab) / normal` with the stab's blow in a
  limit of six per round, because the opening round starts from a full pool with no energy carried and MME
  already counts a first round in whole swings (`first_round_damage`). Their damage is MME's own first-round
  figure for a normal blow (per-swing average with crit, times the hit chance, plus the weapon's cast damage),
  so it differs from the stab's number in kind: MME's surprise figure is the stab's average hit without its
  hit chance, which is carried separately. The two are added as MME's callers add them.
- A stab's own crit (R3) is still left out of the surprise figure, as MME leaves a stab's hit chance out; the
  crit shows in the stab's detail line only.
- `calc_combat_rounds` has no new branch. The surprise credit replaces one normal round with the surprise
  figure, which is now larger; the credit is the same arithmetic.
- MME has no floor on a blow's energy; the engine floors every blow at 200. Where MME's energy is under 200
  (a very fast weapon and a skilled fighter) the stab costs 150% of MME's number, not of 200. The vectors keep
  to the region where both agree for the stab and check the floor separately.
- The engine holds a blow's energy to the round's 1000; MME's energy can pass it (a heavy weapon on a weak
  fighter), and a stab then costs the whole round either way.
- The engine's normal blow costs 10% more right after a cast; MME has no such state, so the vectors use a
  player who has not just cast.
- The neutral balance sets `backstab_energy_pct` to 100000, so a stab always fills the round (the energy is
  capped at the round's 1000) and no blow follows it, which is MME's stab.

## Quicksilver: class melee share and the Thief opportunist (Q-011)

Under `Profile::Quicksilver` only; the stock rules are untouched.

- The class share (R5) scales the damage range of every non-stab blow: normal, bash, smash, punch, kick and
  jumpkick. It reads the class number (1 to 15, other values leave the range whole) and the character's real
  level, and applies to the range as MME holds it after the weapon, the damage bonuses and the martial-arts
  damage are added. The move's own multiplier (kick and jumpkick pre-roll, bash and smash tripling and
  quintupling), the critical range and the target's damage reduction all come after, as in the engine. The
  percentage keeps its fraction between anchors; each end of the range is truncated on its own.
- A manual damage figure (the party panel and `specify_damage`) is not scaled: it is a number the caller
  chose, with no class behind it. The max-level mock profile (level 0) has no class either and keeps its range.
  A stab keeps its own figures; it is not scaled.
- The opportunist (R4) is added to `calculate_attack` as an expectation: per landed blow, the chance
  `opportunist_pct`, times the mean bonus after the target's damage reduction, times the share of landed blows
  that deal damage at all (a fully absorbed blow never finds an opening). It is added to `round_total` and
  `first_round_damage`, so the stab opener's follow-up blows and everything built on `round_total` carry it;
  `round_physical` and `min_round_damage` do not (a proc is a chance, not a floor). The detail line gains
  "Opportunist: N% for M". It never crits and the weapon plays no part. Its level terms are held to
  `scalecap` and its range uses the Backstab Min and Max of the gear, as the engine's does.
- The target is assumed to survive the blow, as the rest of MME's steady-state damage does. A bonus that kills
  the target early is not modelled; whether Model D should stop the procs on a killing blow is card Q-014.
- Bash and smash blows can open too (the engine rolls after any non-stab blow); a bash's tripling does not
  apply to the bonus.
- `calc_true_average` is unchanged: its inputs are numbers the caller supplies, and MME has no class there.
- The neutral balance already holds every class share at 100 and `opportunist_pct` at 0, so nothing is added.

## F-001: follow-ups bundle

- The route dialog lists MME's rows as `ShowRouteResults` prints them, so room names there always carry their
  `(map/room)` numbers; only the spell, item, class and race names inside a step label follow the hide-record-numbers
  setting, as in MME. The page opens the Rooms tab on a row where MME opens the map.
- A cancelled monster damage calculation stores nothing, not even the monsters it had finished; MME's progress form has
  no cancel. The stored arrays and the "already asked" flags stay as they were.
- Under Quicksilver the character editor cuts a typed stat to the training maximum; stock keeps MME's three digits.

## Quicksilver: the weave pick and volley (Q-013a)

Under `Profile::Quicksilver` only; the stock rules are untouched. MME has no weave, so the arithmetic follows
Quicksilver's integer forms and `fixtures/quicksilver/weave.json` pins it exactly.

- Quicksilver's pick considers auto-combat spells in the character's spellbook. MME's spell rows carry no spell
  type and touchstone has no spellbook, so a candidate is an instant single-target damage spell that the class can
  learn and use at the level and alignment (`spell_is_usable` with "learnable"). The two agree on every player
  damage spell in the stock and 1.20 data; a learnable general-type spell would be offered where the engine would not.
- The character is assumed to know every such spell. A character who has not yet learned the best one weaves
  with it all the same.
- A spell's strength range uses the engine's integer growth (`inc * level / levels`). MME's currency form
  (`spells::get_current_spell_min_max`) can come out one lower at some levels, so a weave volley and the spell's
  own detail line may differ by one.
- Magic resistance and the "Alter spell damage" bonus apply in the engine's integer forms (truncating), not
  through `calculate_resist_damage`, which rounds. They differ by under one point per hit.
- A proc is one expected volley with the target assumed to survive every hit and a single resist roll over the
  whole proc, as the engine rolls it. The chance, the killing-blow question and the round are Q-013b and Q-014.
- A slot with its own value deals that value; the proc's power, the class weave share and the target's element
  resistance only reach slots that use the rolled strength, as in the engine.

## Quicksilver: the weave in the round (Q-013b)

Under `Profile::Quicksilver` only; Stock and the neutral balance (weave chance 0) give exactly the answers
they gave before. The pieces (pick, volley, proc) are Q-013a; this is how they enter the round.

- The weave is added as an expectation, the way the opportunist is: per swing, the chance that the blow lands
  and deals damage (hit chance times the share of landed blows, crits included, above the target's damage
  reduction), times the proc chance, times the expected proc against that target. It goes into `round_total` and
  `first_round_damage` (whole swings), so `get_damage_output`, the monster pane and the stab opener's follow-up
  blows carry it. `round_physical` and `min_round_damage` do not (a proc is a chance, not a floor). The detail
  line gains "Weave: N% for M" (M the mean proc after the resist roll); the opener's text gains
  "incl. Weave: N% for M".
- Normal, bash, smash and martial-arts blows all weave (the engine weaves after any non-stab blow); the stab
  itself does not, the blows after it do. A manual or party damage figure is a number the caller chose and has no
  class behind it, so it does not weave.
- Steady state: every damaging landed blow is assumed to leave the target alive, so the weave is never cut off
  by a kill and a blow that kills does not lose its proc. The killing-blow question stays with Q-014.
- The target is built from the request's own numbers: magic resistance, the defence flags (Anti-Magic, Undead,
  Animal, Living), spell immunity and the cold, fire, stone, lightning and water resistances (no poison
  resistance reaches the request). As in MME's spell branch, a request that does not say "living" is a non-living
  target, so an Affects Living spell is not picked against it; the monster pane and monster lists do say.
- The pick is recomputed for every target (it depends on immunity and what a spell spares), which is slow in a
  long monster list for a hybrid under Quicksilver; stock characters pay nothing.
- No exact engine per-round figure exists to compare with: the balance tool works at the middle of each roll,
  on whole kits. The vectors compose the engine's blow resolution (the share of landed blows that deal damage,
  walked over every roll) with the Q-013a proc figures (the exact mean and resist rolls), and the tests check
  that composition and then the wiring end to end through `get_damage_output`.

## Quicksilver: exp/hr (Q-014)

Under `Profile::Quicksilver` with rest and meditate mana on (R10: a balance whose rest or meditate multiple is not 1;
`Balance::NEUTRAL` reads as off, as in Q-008). Stock and the neutral balance give exactly the answers they gave before.

- MME's Models A to C assume a character rests (HP) or meditates (mana), never both. Quicksilver restores both at
  once, so those models return `RulesError::Unsupported`. `calc_exp_per_hour` does not fail on them: it runs Model D
  alone, whichever models the settings tick, and sets `s_notice` (plain text, never an error) when A to C were asked
  for. The monster pane prints the notice below the movement text; the settings dialog shows Models A to C as
  unavailable under Quicksilver.
- Model D has one recovery phase instead of two. Mana returns at the rest multiple of the standing rate (the
  meditate multiple for a character with a meditation rate, who meditates), HP at MME's rest rate plus the passive
  rate, and the phase lasts as long as the slower bar. The HP share of the recovery time is the HP time (with
  MME's heavy-hit relief); the mana share is what mana adds beyond it, so the two shares add up to the phase. The
  mana rate is the engine's exact rate per second, not MME's `MEDITATE_EFF` approximation, which has no
  Quicksilver counterpart. Passive recovery while fighting and while walking is unchanged.
- HP keeps MME's 20-second rest tick (the engine rests on a 21-second tick), parity with the stock model and with
  `calc_resting_rate`, which Q-008 left alone. The vectors pin the engine's own pieces.
- The opener (R2) and the opportunist and weave procs (R4, R6) reach Model D through its inputs: the damage per
  round, the first-round damage and the rounds to kill (`n_rtk`) from `get_damage_output` and `calc_combat_rounds`.
  Model D does not read the surprise figure itself, in stock or here, and for a single monster (`n_rtk` absent) it
  derives the rounds from hit points and damage per round as before, so a stab opener does not shorten that path.
- The killing blow: the engine fires the opportunist and the weave only on a hit that leaves the target alive.
  Model D does not track the monster's hit points blow by blow (it takes the rounds to kill as an input and its
  damage per round already holds the procs as a steady-state expectation), so it cannot stop a proc on the last
  blow. Decision: keep the steady state. A fight's final blow is credited a proc it would not get, an overstatement
  of at most one proc per kill, small when a kill takes several rounds and largest for one-round kills.
- No engine figure exists for a whole exp/hr (the balance tool reports damage bands only). The vectors cross-check
  the recovery component from the engine's own ticks: exact for mana over a 210-second cycle, within one slow tick
  plus one rest tick (51 seconds) and the 20- versus 21-second rest period (5 percent) for the time to restore both bars.

## Graphical map layout (M-001)

The layout engine ports `MapStartMapping`, `MapActivateCell` and `MapMapExits` for frmMain's fixed grid and frmMap's six
sizes, and returns MME's draw list beside a categorised view. Parity is exact on the oracle cases; these are the places
where the port stops short of the window or reads the data on purpose.

- **MegaMUD known rooms.** The fourth "also mark" choice (stars for rooms a MegaMUD rooms file knows) and the
  "MegaMUD Known Room" tooltip lines need a file the server does not read. `AlsoMark` has None, Shops and Spells.
- **Tooltips.** The layout does not build tooltip text. A cell carries its room key and, after an overwrite pass, the
  room it displaced; the text is `rooms::room_tooltip`. A VB run-time error inside MME's tooltip code would end
  `MapMapExits` for that room early (no exits followed). None occurs on the stock data (no oracle case
  shows a handled error), so the port does not model it.
- **Options that only change tooltips** (`chkMapOptions` 5, and 6 or 11 for "show all exits") are not in `MapOptions`.
- **Alignment exits.** frmMain's line colour table tests exit type 30, which its exit-type table never produces (an
  alignment exit is 20), so frmMain draws those lines in the default colour and frmMap in dark red. Both are kept.
- **Centre cell.** A centre outside the grid's first and last cell falls back to the surface's own centre, as
  `MapStartMapping` and `ResizeMap` do; a cell between the columns of a narrow grid (cell 25 of the 20-column grid) is
  kept and charted from, as MME does. The window cannot produce one (it centres on a clicked cell).
- **Overwrite and duplicates.** `MapGrid.cells` lists the cell scan's survivors: with duplicates off a room that two
  cells hold keeps the higher-numbered cell (the scan visits the lower one first and clears it), not the first one
  reached. MME does this; the card's "first come" is first in cell order.
- **Marks stay on a replaced cell.** Squares, circles and stars MME drew for a room an overwrite pass displaced stay on
  the canvas under the new room. `Marker`s are the union of everything drawn on the cell, as the picture is.
- **Zoom grids** differ from the others in line widths only, which the port does not draw; the cell numbers (from 9001,
  stride 28) are kept.
- **Preset editing** (`EditPreset`, the registry values) is client state. `presets_with` applies `LoadPresets`' rule for
  saved values (a zero map or room, or an empty name, falls back to the default).
- **Spell ranges in "what leads here".** `GetCurrentSpellMinMax` is asked without a level, so `gAvgLevelMaxAllStats` does
  not matter to it; the endpoint passes 0. A textblock spell whose range is a single number is not tested at all
  (MME assigns the number and never uses it).

## Web map view (M-002)

The map draws the server's `MapGrid` as SVG (`web/src/lib/components/map/`) on the Rooms tab and in `/map`, the pop-out
for frmMap. Every control of frmMain's map frame and frmMap is wired except the ones below.

- **Left click.** MME's left click only lists the room's references (`MapGetRoomLoc`). Here a click selects the room in
  the Rooms explorer and re-centres the map on it, as the card asks; right click keeps MME's meaning (redraw from the
  room, follow an up or down exit, ask when a cell has both, shift re-centres on any cell).
- **Window-only chrome.** "Not Always on Top", "Allow Main To Overlap", "Show Map Controls" and the window position
  keys are desktop-window settings with no meaning in a browser window. "Find Room" / "Find Next" of frmMap is the Rooms
  tab's name finder; the pop-out has no copy of it. "Route to Room" is the Rooms tab's button.
- **MegaMUD choices.** "MegaMud Rooms" (also mark) and the "Find MegaMud Room/Group" button need a MegaMUD file the
  server does not read (see M-001).
- **Pop-out and the Rooms tab** do not talk to each other: a click in the pop-out selects the room in that window only.
- **Presets** live in settings.json (`map_presets`, edited entries only), not the registry; "Reset" clears them, which
  `LoadPresets(True)` also ends up as (every saved map is zeroed so the defaults come back). The edit dialog is
  reached through an "Edit" toggle instead of a per-button edit control.
- **Remembered options** (size and boxes, one set for the tab and one for the window) are kept per browser in
  localStorage, not in settings.ini. The tab's size list adds the main window's fixed grid to frmMap's six.
- **Walking keys** are the number pad digits as in frmMap, plus the arrow keys and Page Up / Page Down for U and D, and
  work while the map has focus.
- **Caption wording.** The tab uses frmMain's short box captions ("Not Hidden"), the pop-out frmMap's ("Don't Follow
  Hidden").

## Quicksilver: end-to-end cells (Q-016)

Under `Profile::Quicksilver` only. `fixtures/quicksilver/cells.json` holds 45 whole characters from Quicksilver's balance
tool (15 classes at levels 10, 30 and 60, races rotating) with the tool's melee damage a round against the band's
reference monster. `tests/all/quicksilver_cells.rs` rebuilds each through `refresh_all` and `get_damage_output`. 39 of the
43 cells that deal damage agree within 7% (35 before Q-017 applied Alter Energy Usage); the median ratio is 0.99. Layer 3 guards against gross error and wiring, not
formula detail; the structural differences below are why the tolerance is not tighter.

- **Steady state against a simulation.** The tool plays 200,000 rounds of a target that never dies and averages the
  damage; touchstone gives MME's expectation (hit chance times the average blow, crits and the opportunist and weave as
  expectations). A killing blow is not modelled by either side here (the tool tops the target up every round), so it does
  not appear as a difference in these cells; it matters only for kills and Model D (Q-014).
- **The energy floor of 200 and the cap of 1000.** The engine floors a blow at 200 energy; MME has no floor. With
  both at 200 or more the swings agree to the hundredth in every cell. The engine also holds a blow to the round's 1000;
  MME's swings fall below one for a very slow weapon (a level 10 bash at 1,100 to 1,200 energy gives 0.8 to 0.9
  swings). Those cells (Warrior L10, Cleric L10) come out 12 to 16% low in touchstone.
- **Alter Energy Usage (ability 87) on a standing buff.** The tool's hunting buffs include hasten-type spells that
  cut a blow's energy to 85% (and a Mystic's buff list the same). Q-017 applies it under Quicksilver (next section), which
  moved Warlock L30 from 0.82 to 1.00, Warlock L60 from 0.81 to 0.98, Mystic L10 from 0.83 to 1.01 and Bard L60 from 0.80
  to 0.94. The Bard's remaining 6% is not energy: its blows and the tool's swing equally (both at the five-swing cap) and
  the cell is a bash, a move whose accuracy and crit steps differ between the tool and MME. The cells need no allowance
  for it now.
- **Weapons that cast.** The tool plays every cast of a weapon's One Time Cast spells at the character's level; MME prices
  them from the spell text with the weapon's Auto-use chance. The Cleric L60 cell is 10% low.
- **Cannot hit.** A weapon that does not meet the monster's magic requirement does nothing in the tool (damage 0); MME
  reports it as `-9998`. The Mage and Druid L60 cells agree in meaning, not in number.
- **Hit points.** For a few casters touchstone's maximum hit points are 25 under the tool's (a Quicksilver bonus
  that is not an item or spell stat in MME's tables); damage does not read them.
- Not modelled in the comparison: the tool's per-class training ladder is taken as given (the trained stats are in the
  cell), the kit is the tool's, and every cell is one move (the tool's best by damage).

## Quicksilver: Alter Energy Usage (Q-017)

Ability 87, "Alter Energy Usage", is a percentage of a blow's energy. The stock MajorMUD engine applies it to the
energy a character's normal blow costs, so MME's omission of it as a character stat (only the item detail applies a fixed
85% to a displayed weapon) is a difference from the game, not from Quicksilver. Under `Profile::Stock` and under
Quicksilver's neutral balance, touchstone follows MME and ignores it (parity over correctness). Under any other
Quicksilver balance, `refresh_all` totals it into `CharStats.energy_use` and `calculate_attack` takes that percentage of
a normal blow's energy (when positive, and not for the fixed-1000 attacks).

- **A running average, not a sum.** Each source that has the ability averages into the total: the value itself for the
  first, then `(total + value) / 2` (truncating). The order is the engine's: bless spells, race, class, the worn items in
  slot order, the weapon, then carried items that are neither weapons nor wearable. Two sources at 85 and 60 give 72, three
  give a different answer in another order. A bless slot of 0 takes the spell's average cast; an item slot of 0 averages in
  as a 0. "Negate ability" is not modelled.
- **Before Quick and Deadly.** The crit bonus for a fast weapon is judged on the energy after the ability. The engine's
  own check for it is built without abilities, but the balance tool's cells (Warlock L30 and L60) agree with the bonus
  following the cost a blow really has, so that is the order used.
- **No balance key.** The balance keys mirror the engine's, which has none for this, so the neutral balance
  (`Balance::is_neutral`) is the setting that turns it off; a balance that is neutral except for one key still applies it.
- **The floor stays unmodelled.** The engine floors a blow at 200; a cost below that gives the same five swings through
  MME's cap, so swings agree, and `fixtures/quicksilver/energy.json` (255 characters, two levels each) is compared on
  swings. The engine's cost sits within one of the percentage of the cost without the ability, from later truncating steps.

## Realm characters (R-001)

`GET /api/realm/characters/{id}` shows a character of the live Quicksilver or silvermere database through touchstone's
own pipeline (`refresh_all` under the source's detected profile) beside the values the game stores, with each stored
value that has a touchstone counterpart compared. The mapping and the tolerances below are touchstone's choices; MME has
no realm, so nothing here is parity with MME, only a statement of where the game's record and MME's calculation part.

**The mapping** (`bin/touchstone/src/realm.rs`):

- A worn item goes where MME's own equip rule puts it by the item's worn type (the game's worn list is just the first
  free slot, so its order says nothing); the wielded weapon takes the weapon slot. A second item for a slot MME holds one
  in, a weapon in a worn slot beside the wielded one, a pair already full, and an item whose type has no MME slot (a
  thrown item with a hands location, say) go in `unmapped` with the reason, and stay in the pack, so their weight counts.
- The pack, less one copy of each mapped worn and wielded item, is the carried list (MME counts a carried Special item's
  abilities and its weight). A readied light is carried like any other item; MME has no light slot.
- The coins go in MME's extra-weight box (three coins of a type weigh one), which holds at most 999; a purse heavier
  than that is capped and listed in `unmapped` as `coins`.
- Alignment comes from the evil points: below -50 good, from 40 evil, otherwise neutral. A character's quest flags,
  Item Manager stash, key ring and attack choice are not mapped (the attack choice is MME's default).
- A running spell that is a bless spell takes a bless slot, in game order; any other running spell (a curse, a
  summon, a debuff) has no MME slot and is `unmapped`, though its effect is in the stored values.
- A class or race the data has no row for gives no sheet: the character is still listed, with `sheet_error`.

**The comparison.** Whole-number values (stats, mana, magic resistance, spellcasting, stealth, picklocks, weight,
the percentage of the load, the Alter totals) must agree exactly. Max HP agrees when the stored value is inside MME's
min to max range for the class and level, because the game rolls hit points at training and MME gives the range. The
game's perception is compared on its ability part: MME's Perception box shows the ability total alone, so the game's
Int, Wil and Cha term is taken off the stored value first. The Alter totals (AV, defence, damage, critical, damage
resistance, DR percent, shield, defence modifier, heal and mana rate, room light) are plain sums over race, class,
equipped items, carried items that count and the bless slots (the best of the three for AV), which is what the game's
dynamic-stat fold does; Alter Energy Usage is the pipeline's own running average. Weight is the items alone (the game
stores the coins apart). Not compared, because MME has no counterpart: martial arts, thievery, disarm traps, tracking and
traps (MME shows only the ability bonus for traps), energy per round, and the current (not maximum) HP and mana.

**Known kinds of difference** (the `cause` of a differing field; any difference without one is unexplained and the
fixture test fails on it):

- `no-magic-class`: the game stores 0 mana and a casting base of -150 for a class with no magic; MME computes mana and
  spellcasting from the formulas for any class (6 mana at level 1, and so on).
- `kai-mana`: the game gives a Kai class a mana of its level less one; MME treats Kai as any caster.
- `no-stealth-skill`: with no class or race stealth skill the game stores 0; MME shows the ability total alone (armour
  that gives a stealth penalty shows it for a Warrior, say).
- `no-picklocks-skill`: the game gives picklocks only to a character with the picklocks ability (stored 0 otherwise);
  MME calculates a picklocks figure for every class.
- `coin-weight-capped`: the load percentage counts the coins, which MME can only take up to 999 weight of.
- `active-spell`: the game stores the effect of its running spells at the value each cast rolled; touchstone's bless
  slot takes the spell's average cast at the character's level (and ignores a spell that is not a bless spell, and
  "Negate ability"). Any stored value can differ for this reason while a spell runs; so can current stats.
- `unmapped-item`: an item `unmapped` still counts in the game (its abilities, its slot) and is not in the calculation.

Stored current stats include temporary effects (stat spells, poison) that the base stats and the gear do not, which is the
same `active-spell` difference. A stored value that touchstone has no cause for is a finding, not an error: it is shown,
flagged, and the card's pins show none for the fixture characters.

**Fixture result** (`fixtures/quicksilver/realm_characters.sql`, 12 characters built by Quicksilver's own training ladder
and engine, `realm_pins.json`): 302 of 324 compared fields agree (93%). Every stat, max HP, magic resistance, perception,
weight, Alter Energy Usage and every other Alter total agrees for all 12; max mana and spellcasting differ for the
four non-casters and the Kai (stored 0 or the Kai rule against MME's formula), stealth differs for the five characters
with no stealth skill, picklocks for the nine with no picklocks ability, and the load percentage for the one
character whose purse is over the box. The Alter totals agree even with running spells because the fixture's spells are
cast at the strength touchstone's bless slot assumes; a real realm's rolled values will show as `active-spell`.

## PullSpellEQ integer edges (P3-04b)

- MME's `For y = nMin + 1 To nMax` over EndCast and Summon ranges keeps the limit as the Currency it is and
  converts only the start to a Long. The port does the same (`for_to`), as the oracle's interpreter does; real
  VB6 may convert the limit too and raise error 6 for a limit beyond Long. Stock data never reaches it.
- A Spells value outside Integer that is passed to `GetAbilityStats` (an Integer parameter) makes `PullSpellEQ`
  return an empty string (error 6, handled by `Resume out`), as in MME; no other path converts it.

## Active realm character (R-002)

The header's active character can be a realm character. The app's calculators and filters then read its mapped input
(`/api/realm/characters/{id}/input`), which is touchstone's mapping, not the game's own record: the differences listed
under "Realm characters" apply to anything calculated from it. The attack choice, quest flags and Item Manager stash
are not in the game's record, so they are MME's defaults. A menu action that edits "the character" (set as combat
spell, bless slots) changes an in-memory copy only; it is dropped when the character changes in the realm or another
is picked, and nothing is written to the realm or to the saved sandbox characters.

## Calculators on realm characters (R-003)
- MME has one loaded character that every calculator reads. Here the header's active character plays that part, and
  each calculator can borrow another realm character for itself; MME has no such override. A borrowed character is
  read-only and is dropped if it leaves the realm.
- A party can be built from up to six realm characters. Each member's defenses and recovery come from the character's
  evaluation and its damage and swings from the combat preview of its saved attack (the assumed normal attack). The server's paste-party code
  then averages them. MME builds the same table from pasted text, and its heals box stays empty here.
- The game stores no attack choice for a character, and MME's default for a fresh one is one-shot, which has no damage
  figure (the party's damage output and exp/hr would be placeholders). The realm input therefore carries a normal
  attack: the wielded weapon, or bare hands when none is wielded, for every class, casters included. The sheet and
  each calculator say so. A punch default was rejected: it gives no damage for classes without a martial art.
- The paste dialog's Continue now writes the party boxes of the Monsters tab too (it only displayed them before).
- The status chip and the Options items name the character or party the damage was calculated for; MME shows no name.
- Reachability does not depend on a character, so there is nothing for it to follow.

## Weapon simulator (W-001)

Calculators > Weapon Simulator is a touchstone addition; MME has no such window. What it rolls is MME's:

- **The blow.** For each weapon and attack the simulator calls `CalculateAttack` as the Weapons tab's maximum-effort
  list does (the character's profile for that attack, the weapon swapped in for the one in hand, the target's AC, DR,
  dodge and backstab defense) and rolls from its answer: the hit chance, the crit chance, a plain hit between the
  minimum and maximum damage, a crit between the crit minimum and maximum. MME's quirks come along, among them damage
  reduction taken off the ends of the range (a roll is never clamped on its own) and a bash that cannot crit. The
  mean of the simulation therefore lands on MME's damage a round, within the rounding of MME's whole-number averages
  and the dice's noise; the page shows both.
- **A weapon's cast spell** fires on a landed blow with the chance `per-swing average / average when it fires` (the two
  figures MME keeps) and then adds the latter. Its damage is not rolled.
- **Quicksilver's opportunist** fires on a blow that dealt damage, at its percentage, for its mean bonus.
- **Swings a round** follow frmSwingCalc's `CalcSwings`: a round has 1000 energy plus what the last round left, a swing
  costs the attack's energy, and a round holds at most five. MME's damage a round multiplies by the fractional swings
  instead; over many rounds the two agree.
- **A backstab round** is an opening round on its own: the stab (in stock the whole round) and, under Quicksilver, the
  whole normal blows the rest of the round pays for (`calc_stab_opener`). Only weapons with backstab accuracy
  (ability 116) stab and only for a class or race with stealth, as in MME's weapon lists.
- **Rounds to kill** are not an MME figure: fights to the target's hit points with no regeneration, the stab's
  opening round followed by normal attacks in the backstab view. A fight that would take more than 60 rounds is not
  rolled; the page shows hit points over the simulated damage a round, marked `~`.
- **Obtainable only** (on by default) leaves out a weapon whose Obtained From is empty: no room, shop, monster, chest
  or script yields it. MME's database marks such leftovers "not in game", but a live silvermere or Quicksilver
  database lists every row as in the game, so a test item (the stock "bow of silver", 90-900 damage) would
  otherwise top the list. The weapon in hand and pinned weapons are kept either way.
- **Which weapons.** `ItemIsUsableByChar` for the character's class, level and alignment, in-game weapons only; the
  global filter's minimum item level is not applied. "Ignore level" tests a weapon at the higher of the character's
  level and the weapon's minimum level, so a maximum-level rule still applies; the damage is still calculated with the
  character's current level and stats. Weapons kept only because a restriction is ignored are greyed and say why.

