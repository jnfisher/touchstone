---
id: P4-22
title: Combat setup dialog (attack choice, healing, live preview)
model: sonnet
phase: 4
depends: [P3-09, P3-10, P3-11, P3-12, P4-20, P4-05]
touches:
  - bin/touchstone/src/
  - bin/touchstone/tests/
  - web/src/lib/
  - web/src/lib/components/character/
  - docs/design.md
---

## Goal
MME's combat setup dialog (frmPopUpOptions, FEAT-COMBAT; rows P4-22 and P4-23 of `docs/inventory.md`
section 8b, done as one card): choose the attack the character uses everywhere (weapon, bash, smash,
backstab with its weapon, martial art, learned spell, any spell at a level, manual physical/magic
damage, one-shot), the healing choice (infinite, none, spell, spell at a level, manual; rounds),
meditate, and see the live preview MME shows (damage per round, rounds to oom, heal and mana per
round). The choice is the character's `attack` (P3-11), saved with it, and already drives the
profile every computed column and calculator is built from.

## Read
- MME through the index: frmMain `PopUpChooseCombatGUI`, `RefreshMonsterCombatGUI`; frmPopUpOptions
  `SetupChooseAttack`, `RefreshSpells`, `RefreshItems`, `RefreshCombatStats`, `cmdContinue_Click`,
  `optAttackType_Click`, `optHealingType_Click`, `cmdHealRoundsMod_Click`, `chkMeditate_Click`.
- `crates/touchstone-rules/src/character.rs` (`AttackConfig`, `populate_character_profile`,
  `refresh_combat_healing_values`, `get_current_attack_name`), `spell_cast.rs`, `attack.rs`,
  `pipeline.rs` (`profile_for`); `bin/touchstone/src/character.rs`, `evaluate.rs`.
- `web/src/lib/character*.ts`, `components/character/CharacterTab.svelte`, `components/paste/PasteDialog.svelte`.

## Do
Decisions made while reading the procedures:
- The dialog has two halves in MME (attack page, then healing page, the latter only for NMR 1.83+).
  The page keeps that as two tabs; below 1.83 the heal tab and the meditate boxes are hidden.
- Saving (`PopUpChooseCombatGUI`) is not a copy of the form: a spell radio with no spell picked
  becomes one-shot and skips the backstab write, martial arts "none" becomes one-shot, a manual attack
  with both boxes 0 becomes one-shot, the backstab flag is only written for a real attack, the
  any-spell level is written only for the any-spell radio, the heal level is clamped 0..9999 and heal
  rounds 1..50 only when a heal spell is picked. This is `apply_choice` on the server, so the page
  holds none of it; it starts from the saved attack and overwrites what MME overwrites.
- The preview (`RefreshCombatStats`) reads the dialog's own choice for the numbers and the *saved*
  attack for the character profile (`PopulateCharacterProfile` reads the globals, not the dialog), so
  the request carries both: `character` (saved attack) and `draft`. The meditate box does not change
  the preview (the local flag is never read). Its heal block differs from `RefreshCombatHealingValues`
  (base heal is rest rate / 18, rounds clamp to 999, cost is rounded to 2 places, a spell attack's
  cost is part of the heal overhead and the other way round), so it is ported here from the dialog.
- `POST /api/character/combat-preview` {character, draft?, recovery_only?} -> numbers, MME's captions,
  the heal rounds the dialog writes back, `applied` (what Save stores) and the attack names.
  `POST /api/character/combat-options` {character} -> the spell and backstab-weapon lists of
  `RefreshSpells` / `RefreshItems`, the current weapon caption and the NMR version.
  The evaluate response gains `attack_name` (`GetCurrentAttackName`) for the Char tab's button.
- Lists and the P4-05 columns: the weapons/spells evaluate endpoints already build the profile from the
  stored character, so the chosen attack (meditate, backstab weapon, heal cost) is used; a test pins that.
- Web: a "Combat" button on the Character tab showing the current attack name; the dialog edits a draft,
  previews with a debounce (MME uses a timer), Save writes `attack` into the store (autosave).

## Done when
`cargo fmt/clippy/test`, `tools/guard.sh`, web `npm run check && npm test && npm run build`;
endpoint tests pass; screenshot `/tmp/P4-22/shots/combat.png` exists.
