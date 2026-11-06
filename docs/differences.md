# Differences from MME

Places where the port reproduces MME's answer although the game (or MME's runtime) would differ.

## Paste party (P3-41)

- The party pattern's `\s` and `\d` are ASCII classes, as in the oracle's regex engine. VBScript's `\s` also
  matches non-breaking and other Unicode spaces, so a paste that separates fields with those would parse
  differently in MME itself. Pasted terminal text does not contain them.
- Weapons that cast (ability 43) are not exercised: their cast text needs the spell-text tables and
  `CalculateAttack` is fixture-tested with them separately. `parse_paste_party` passes empty text tables.
