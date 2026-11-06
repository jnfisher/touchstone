# Differences from MME

Places where the port reproduces MME's answer although the game (or MME's runtime) would differ.

## Paste party (P3-41)

- The party pattern's `\s` and `\d` are ASCII classes, as in the oracle's regex engine. VBScript's `\s` also
  matches non-breaking and other Unicode spaces, so a paste that separates fields with those would parse
  differently in MME itself. Pasted terminal text does not contain them.
- Weapons that cast (ability 43) are not exercised: their cast text needs the spell-text tables and
  `CalculateAttack` is fixture-tested with them separately. `parse_paste_party` passes empty text tables.

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
