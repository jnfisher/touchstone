# Differences from MME

Places where the port reproduces MME's answer although the game (or MME's runtime) would differ.

## Paste party (P3-41)

- The party pattern's `\s` and `\d` are ASCII classes, as in the oracle's regex engine. VBScript's `\s` also
  matches non-breaking and other Unicode spaces, so a paste that separates fields with those would parse
  differently in MME itself. Pasted terminal text does not contain them.
- Weapons that cast (ability 43) are not exercised: their cast text needs the spell-text tables and
  `CalculateAttack` is fixture-tested with them separately. `parse_paste_party` passes empty text tables.

## EQ tab (P4-17)

- A two-handed weapon (weapon type 1 or 3) does not clear the off-hand. In the game a two-hander
  excludes a shield; MME keeps both items equipped and only paints the weapon and off-hand boxes red.
  The port does the same and reports the flag.
- Taking an item off through the equip toggle does not mark the character as changed (`bPromptSave`),
  although equipping does. The rules report `changed = false` for it.
- A wrist item (worn type 14) and a finger item (4 or 13) can go in either of their two slots, but
  the armour explorer's wrist filter also accepts worn type 17 while the equipment lists never offer it.
