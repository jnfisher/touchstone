---
id: P4-24
title: .mmec character import and export
model: sonnet
phase: 4
depends: [P3-11, P3-17, P3-18, P4-20]
touches:
  - crates/touchstone-rules/src/mmec.rs
  - crates/touchstone-rules/src/lib.rs
  - crates/touchstone-rules/tests/
  - bin/touchstone/
  - web/src/lib/
  - web/src/lib/components/character/
  - fixtures/data/mmec/
  - docs/differences.md
  - docs/design.md
---

## Goal
Users can import an MME `.mmec` file as a saved touchstone character and export a saved
character as an `.mmec` that MME loads. Loading follows `LoadCharacter` (defaults, clamps,
order); saving follows `SaveCharacter`'s exact write order and number formatting, so a file MME
wrote loads and saves back byte for byte.

## Read
- MME through the index (frmMain): `SaveCharacter`, `LoadCharacter`, `EquipBlessSpell`,
  `SetUpFormObjects` (quest option lists); modSettings `ReadINI`, `WriteINI`.
- `docs/inventory.md` section 5d, `crates/touchstone-rules/src/character.rs` (`Character`),
  `bin/touchstone/src/character.rs` (P4-20's DTO and store).

## Do
- `touchstone_rules::mmec` (no IO): `parse(&[u8]) -> Mmec` and `write(&Mmec) -> Vec<u8>`.
  Windows-1252 text, CRLF, INI semantics of `GetPrivateProfileString` (case-insensitive section
  and key, first duplicate wins, `;` comments, value trimmed, matching quotes stripped, an empty
  value reads as missing so the default applies).
  - `Mmec` = the rules `Character` + Item Manager `carried` / `stash` (`item|qty` lists) + `extra`,
    an ordered list of `(section, key, value)` for everything touchstone does not model: the PlayerInfo keys
    DataFile, DataFileVer, GlobalMinLVL, WeaponVSChar*, SpellVSMR, UnequipMissing; the whole
    MonsterFilter and Compare sections; a `<DataVersion>` section; unknown keys and sections.
  - Write order is `SaveCharacter`'s: PlayerInfo (Name first, because MME creates the file with it),
    MonsterFilter, Inventory, Bless, LearnedSpells, Compare; then unknown sections. Unmodelled keys
    take their value from `extra`; when absent, PlayerInfo gets LoadCharacter's defaults and the
    MonsterFilter / Compare sections are left out (MME fills them with defaults when it loads).
  - Load clamps as `LoadCharacter` does: quest options past the stock list go to 0, attack manual
    values 0..9999999, heal spell level 0..9999, heal rounds 1..50, heal manual 0..999999, the attack
    block is read only when `CurrentAttackType` > 0 (else one-shot defaults), the heal block only when
    `nGlobalAttackHealType` > 0 (else type 0, rounds 1), spell attacks without a spell fall back to
    one-shot, `bGlobalAttackBackstab`/`UseMeditate` follow the same gates. Levels and stats clamp
    to the 0..999 boxes; alignment outside 0..3 becomes 0 (MME would stop with error 380).
- Server (`bin/touchstone`): the DTO gains `carried`, `stash` and `extra`. World-dependent rules of
  `LoadCharacter` run in the import handler: learned spells the class cannot use (`SpellIsUsable`)
  are dropped and `learned_class` set; a bless number is kept only for a bless spell and only once
  (`EquipBlessSpell`); a class or race number not in the database becomes "nothing picked".
  - `POST /api/characters/import?name=` raw body (the page sends the file's bytes; the file name goes in `file`).
    Name: `name` query, else PlayerInfo Name, else the uploaded file name without `.mmec`; an
    invalid name gets a 422 that says so. Replaces an existing character of that name only with
    `overwrite=true` (409 otherwise). Answers the saved DTO.
  - `GET /api/characters/{name}/export.mmec`: `application/octet-stream`, `Content-Disposition:
    attachment`, CRLF cp1252 body.
- Web: Import (file picker) and Export (download link) on the Character tab; pure helpers in
  `$lib/character.ts` (file name, import URL, error text) with vitest.
- Tests: rules round trip for four hand-written files in `fixtures/data/mmec/` (these are test
  inputs, not MME data) plus parsed values, defaults, clamps, cp1252, quirks; endpoint tests for
  import/export/overwrite/bad body.

## Decisions
- `MR` is not read by `LoadCharacter` (`RefreshMagicRes` recomputes it). touchstone's `mr_text` is
  user text, so import carries the file's MR into it; export writes `Val(mr_text)`.
- Values beyond `GetPrivateProfileString`'s 255-character buffer are not cut (MME silently loses the
  tail of a long `IM_CARRIED`); recorded in `docs/differences.md`.
- Data-file switching (`DataFile` differs from the open database) is not done; the keys are kept.

## Done when
`cargo fmt/clippy/test` pass, `tools/guard.sh`, web `npm run check && npm test && npm run build`;
round-trip and mapping tests pass.
