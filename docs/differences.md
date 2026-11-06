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

## Monster Attack Simulator window (P4-28b)

- MME calls `Randomize` at the top of `RunSim`, so every run in MME rolls differently. touchstone seeds the
  generator: the default seed is 327 680 (the state VB6 starts in before any `Randomize`), a request may give
  its own (low 24 bits used), and the same request always answers the same. Averages therefore vary with the
  seed by the same amount they vary between MME runs.
- The form fixes the combat log at 100 rounds; the endpoint takes `log_rounds` (0 to 1000, default 100).
- Vile ward is GreaterMUD-only. The endpoint accepts and ignores it, as the form hides the box off GreaterMUD.
- The party defenses (lair filter party of two or more) are not offered; the window runs for one character.
