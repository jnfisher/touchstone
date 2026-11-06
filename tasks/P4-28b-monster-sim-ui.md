---
id: P4-28b
title: Calculators UI II: Monster Attack Simulator
model: sonnet
phase: 4
depends: [P3-13, P3-14a, P3-14b, P4-20, P4-27b, P4-28a]
touches:
  - bin/touchstone/src/
  - bin/touchstone/tests/
  - web/src/lib/calc.ts
  - web/src/lib/calc.test.ts
  - web/src/lib/components/calc/
  - web/src/routes/calc/
  - web/scripts/
  - docs/differences.md
  - tasks/P4-28b-monster-sim-ui.md
---

## Goal
MME's Monster Attack Simulator window (frmMonsterAttackSim, FEAT-MSIM) exists as the sixth entry of the
Calculators menu, backed by `POST /api/calc/monster-sim` over `monster_sim.rs` and `monster_glue.rs`.
Pick a monster, set the defender (the current character, or typed AC / DR / dodge / MR / anti-magic /
protection from evil / elemental resists), rounds and options, run, and read what MME shows: average and
maximum damage per round, the per-attack table and the combat log.

## Read
`docs/porting.md`, `docs/testing.md`; MME frmMonsterAttackSim `cmdRunSim_Click`, `cmdResetUserDefs_Click`,
`Form_Load`, `LoadMonsters`, `ResetFields`, `GetPartySize`, through the index;
`crates/touchstone-rules/src/monster_sim.rs` (`MonsterSim::run`, `max_damage`), `monster_glue.rs`
(`populate_monster_data_to_attack_sim`, `reset_values`, `CharSimInputs`), `CharacterState::sim_inputs`;
`tasks/P4-27b-*.md` for the "use current character" pattern; `bin/touchstone/src/calc.rs` and
`web/src/lib/components/calc/HitCalc.svelte` for style.

## Do
1. Server (`bin/touchstone/src/monster_sim.rs`, routes in `api.rs`):
   - `POST /api/calc/monster-sim` `{monster, rounds, seed?, dynamic?, log_rounds?, log_max_round_only?,
     hide_energy?, always_dodge?, defender}`. `defender` is either `{character: <evaluate body>}` or the
     typed boxes (`ac dr dodge mr prot_evil vile_ward resist_cold resist_fire resist_stone
     resist_lightning resist_water` as text read with `Val`, `anti_magic`, `class`). The sequence is
     `cmdRunSim_Click`: ResetValues, caps from the class, a box above 0 replaces the default (MR 50),
     populate from the monster, `RunSim` when the rounds are above 0. Rounds above 500 000 become 500 000
     (the form's limit). A box that overflows a Long, an unknown monster or class, or a bad body is a 400.
   - With a character the boxes are filled as `cmdResetUserDefs_Click(1)` does for a party of one: AC, DR,
     MR and dodge rounded, protection and resists from the tags (an empty tag is 0), anti-magic from the
     check box. The class comes from the character (hit minimum and dodge caps).
   - The response carries the five attack rows with `Form`'s per-attack percentages (cast, average per
     hit, success, damage resisted, resisted or dodged), the three result captions, the combat log
     (trimmed, CRLF), the effective defender boxes, the generator state after the run and a VB error
     (number and text) when `RunSim` was abandoned, with the partial results.
   - `POST /api/calc/monster-sim/defender` `{character}` answers the boxes alone, for the page to show
     while the character switch is on.
   - Determinism: MME calls `Randomize`; the port seeds. The default seed is 327 680 (0x50000, VB6's
     generator before any `Randomize`), `seed` overrides it (low 24 bits are used). Noted in
     `docs/differences.md`. The combat log holds `log_rounds` rounds (MME fixes 100; capped at 1000).
   - GreaterMUD is not ported, so vile ward is accepted and ignored, as the form hides it off GreaterMUD.
     Party defenses (the lair filter's party) are not offered.
2. Tests in `bin/touchstone/tests/all/monster_sim.rs` pinned to `fixtures/rules/monster_glue_char.json`
   (`CalculateMonsterDamageVsChar` runs use the same setup, populate and generator, so the average damage
   and the generator state after the run are the fixture's), to `monster_sim.json` structure where it
   applies, and 400 cases.
3. Web: `monster-sim` joins `CALCULATORS`; `calc.ts` gets the request mapper and response types (vitest);
   `MonsterSimCalc.svelte` has the monster picker (the Hit calculator's), the defender section with the
   shared "use current character" switch, rounds, seed, options, a results table and a scrollable log.
4. Screenshot `/tmp/P4-28b/shots/monster-sim.png` (Playwright, 1440x900, dsf 2, dark) by
   `web/scripts/calc-shots.cjs monster-sim`.

## Done when
`cargo fmt/clippy/test`, `tools/guard.sh`, `npm run check`, `npm test`, `npm run build` pass; the endpoint
tests pass; the screenshot exists.
