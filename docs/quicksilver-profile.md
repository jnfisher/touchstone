# The Quicksilver profile

Quicksilver is John's MajorMUD server: the 1.11p game as silvermere carries it, plus the "1.20"
rule changes. touchstone already reads a running Quicksilver database (schema versions 1, 2 and
9001, see `docs/data-gaps.md`), so its **content** (items, monsters, spells, rooms, shops, race
abilities) arrives for free. What does not arrive is the **formulas**: MME's rules are stock, and
Quicksilver changes some of them. This document inventories those changes, says where they hit
`touchstone-rules`, and plans the cards. It is the design for the `Profile::Quicksilver` variant
promised in `docs/design.md`.

Sources, all read-only: the Quicksilver checkout (`quicksilver-dev`, private) and the open-source
silvermere engine it forks, diffed to confirm what is new. This page summarises. It does not copy
Quicksilver's text, tables or code, and it names Quicksilver files by path only. Paths below are
relative to the Quicksilver checkout.

Two facts frame everything:

- **Quicksilver's rule numbers are a config type, not data.** Every tuning number lives in the
  `[balance]` table of the server's config (type `Balance` in
  `crates/silvermere-types/src/config.rs`, documented in `docs/config.md`). The database does not
  carry them. The 1.20.0 values are frozen as `Balance::V120`; a retune is a new content version
  (1.20.1 changed nothing in rules or balance; its release notes say so). So touchstone needs its
  own copy of the balance, picked by content version (part 3).
- **MME is the stock rules, so Quicksilver is a delta on MME.** The Quicksilver profile reproduces
  what MME would answer if MME knew the changes below. Where MME models something differently from
  the real engine, MME's modelling stays (parity rule in CLAUDE.md); only the delta is new.

## 1. Rule changes that touch a number touchstone computes

"Kind": **config** means the formula shape is stock and only a number changes, taken from the
Quicksilver `[balance]` key named; **code** means a new term or a new shape that needs new Rust
(it may also read config keys). Row ids (R1...) are used by the cards in part 5.

| Id | Change | Quicksilver source | touchstone family and functions | Kind |
|---|---|---|---|---|
| R1 | **Backstab damage.** The stealth term divides by a smaller number and the level multiplier grows faster per level. The flat level term and the multiplier both read the level capped at `scalecap`. | `docs/differences-1.20.md` Backstab, Scaling; `crates/silvermere-rules/src/combat/fighter.rs` (`player_fighter`, backstab branch). Keys `backstab_stealth_div`, `backstab_level_pct`, `scalecap`. | FAM-07 `attack::calc_bs_damage`, `attack::calculate_attack` (attack type 4 min/max); FAM-27 `calculators` backstab calculator; FAM-20 backstab lines in the detail builders. | config |
| R2 | **A stab no longer ends the round.** It costs a percentage of a normal blow's energy (never above the round's), and the rest of the round is normal blows with normal damage, accuracy and crits. Still one stab a fight. | `crates/silvermere-rules/src/combat/energy.rs` (`backstab_energy`); `docs/differences-1.20.md` Backstab. Key `backstab_energy_pct`. | FAM-07 `calculate_attack` (type 4 energy and swings); FAM-08 `combat::get_damage_output` (surprise fields) and `calc_combat_rounds` (surprise credit); FAM-14 Model D opener; FAM-20 detail text. | code + config |
| R3 | **A stab can crit,** at the normal crit chance (the crit knee applies, no Quick and Deadly bonus). A critical stab rolls from max up to a set percentage of max; other crits keep their range. | `crates/silvermere-rules/src/combat/attack.rs` (`resolve`, `move_mods`). Key `backstab_crit_max_pct`. | FAM-07 `calculate_attack` (type 4 zeroes crit today; crit amount in the shared tail) and `calc_true_average`. | code + config |
| R4 | **Thief opportunist.** A Thief's normal hit that leaves the target alive may add a bonus hit: `2L + stealth/div + a roll in the backstab min..max`, scaled by a per-level percentage, less a tenth of the target's damage reduction. Never crits; weapon damage plays no part. | `crates/silvermere-rules/src/v120/opportunist.rs` (`opportunist`, `bonus_range`). Keys `opportunist_pct`, `opportunist_level_pct`, `backstab_stealth_div`, `scalecap`. | FAM-08 `get_damage_output` (new expected term per landed normal hit); FAM-07 `calc_true_average`. | code + config |
| R5 | **Class melee share.** Every non-stab blow (including martial-arts moves, bash and smash) deals a class and level dependent percentage of its damage range. A per-class table, one value or level anchors with straight lines between them, truncating. | `crates/silvermere-rules/src/combat/fighter.rs` (`class_melee_pct.scale`); `LevelPct` in `silvermere-types/src/config.rs`. Table `class_melee_pct`. | FAM-07 `calculate_attack` (`dmg_min`/`dmg_max` for every type but 4), so everything downstream: FAM-08, FAM-20, FAM-21 rows. | config |
| R6 | **Weave.** Hybrid classes (magery level 1 or 2, not Kai) proc a free spell volley on a normal hit that leaves the target alive: the best single-target attack spell the character can cast, rolled at a power that holds expected damage to a per-level budget (capped by `scalecap`), through element and magic resistance. A Bard's proc deals the budget as one song hit. | `crates/silvermere-rules/src/v120/weave.rs` (`weaver`, `pick`, `volley`, `budget`, `power_pct`, `roll_hits`). Keys `weave_*`, `class_weave_pct`, `scalecap`. | FAM-08 `get_damage_output`, `calc_combat_rounds`; FAM-14 Model D; reads FAM-09 `SpellCastEnv::spell_is_usable` and FAM-10 spell equations to pick and size the volley. | code + config |
| R7 | **Casting-stat spell damage.** Player damage spells, area spells and non-lasting Steal Hit Points scale by `1 + 0.5% x (stat - 50)`, the stat counting a third above a knee. Heals, buffs, monster casts, item casts and lasting drains are not scaled. The casting stat is the class's mana stat (Mage Int, Priest Wil, Druid mean of both, Bard Cha, 0 for Kai and no magic). | `crates/silvermere-rules/src/spells/mod.rs` (`scale_spell_damage`), `spells/apply.rs` (`stat_scaled`), `regen.rs` (`class_stat`). Keys `spell_stat_permille`, `spell_stat_knee`. | FAM-09 `spell_cast::calculate_spell_cast` (its `SpellCasterProfile` has no stat, class or level today); FAM-20 spell damage lines; FAM-21 spell rows. | config (+ new inputs) |
| R8 | **Class spell share.** A class's damage spells (weave procs included) deal a class and level dependent percentage, applied after the stat bonus. | `spells/apply.rs` (`stat_scaled`, `class_spell_pct.scale`). Table `class_spell_pct`. | Same as R7. | config |
| R9 | **Area spells: monsters can resist.** Every area damage spell rolls a resist per monster at the single-target chance; a monster that resists keeps its share less a capped fraction of its magic resistance. Stock never resists an area spell whose resist mode is Never. | `crates/silvermere-rules/src/spells/resist.rs` (`monster_resists_area`, `apply_area_resist_cut`). Keys `aoe_resist_cut_max_pct`, `aoe_resist_cut_div`. | FAM-09 `calculate_resist_damage` / `calculate_spell_cast` for area spells: an expected-value factor `1 - p x cut/100`, p the resist chance. | code + config |
| R10 | **Rest and meditate restore mana.** `rest` keeps its HP and adds mana at a multiple of the standing rate; `meditate` restores mana at a larger multiple and heals as `rest` does. Both end when HP and mana are full. So HP and mana now recover at the same time. | `crates/silvermere-rules/src/regen.rs` (`rest_mana_regen`, `meditate_mana_regen`, `boosted_mana`); `docs/differences-1.20.md` Spells and mana. Keys `rest_mana_mult`, `meditate_mana_mult`. | FAM-02 `stats::calc_mana_regen` (its `meditating` flag), `calc_resting_rate` (unchanged); FAM-09 `calc_mana_recovery_rounds`; FAM-05 `char_panel` HP and magic panels; FAM-14..17 recovery time (rest or meditate, not both, is an MME assumption). | code + config |
| R11 | **Crit knee** moves from 40 to a configured value; past it a point counts a third. | `combat/attack.rs` (`effective_crit`). Key `crit_knee`. | FAM-07 `calculate_attack`: the literal 40 in the crit block. | config |
| R12 | **Magic resistance knee.** A player's stat part `(Int + 3 Wil) / 4` counts a third past a knee; gear and spells are added after. Monsters unchanged. | `crates/silvermere-engine/src/player/secondary.rs` (`secondary`). Key `mr_stat_knee`. | FAM-02 `stats::calc_mr`, called from FAM-05 `char_panel` and FAM-04 `gear/second.rs`. | config |
| R13 | **Dodge agility knee.** A player's Agility counts a third past a knee in the `(Agi - 50)/3` term. | `combat/fighter.rs` (`player_fighter`). Key `dodge_agi_knee`. | FAM-06 `accuracy::calc_dodge`, called from `gear/second.rs` and `paste_party.rs`. | config |
| R14 | **Stat soft caps.** Training lets a stat go past the race maximum to a soft cap (the larger of the race maximum and a floor) and 30 points beyond: a flat cheap price up to the cap, then three price bands. Creation is unchanged. | `crates/silvermere-rules/src/progression.rs` (`soft_cap`, `train_max`, `train_cost`). Keys `softcap_floor`, `softcap_floor_price`, `softcap_band_prices`. | FAM-05 `char_panel::cps_steps` and its per-stat cost (MME prices points above the ceiling on the stock ladder); FAM-02 `stats::cp_cost_for_raise` for any stat past the race maximum; the stat range the character editor accepts. `compute_avg_level_max_stats` is unchanged (it stops at the race maximum). | config |

Notes on the table:

- **The balance tables are large.** R5, R6 and R8 each take a per-class table (15 stock classes,
  value or level anchors). They are config, but they are most of the numbers. Anchors interpolate
  linearly and truncate toward zero in 64-bit; Quicksilver's `LevelPct` is the reference.
- **Steady state versus kills.** R4 and R6 fire only when the target survives the hit. MME's damage
  output is a steady-state expectation, so the profile uses the expectation with the target
  assumed to survive, as Quicksilver's own balance tool does. Whether Model D's round loop should
  stop the procs on the killing blow is the question for card Q-014; any approximation goes in
  `docs/differences.md`.
- **Exp/hr models.** R10 breaks the assumption inside Models A to C that a character rests or
  meditates, never both. The plan supports Model D (the recommended one, a round-by-round sim) and
  makes A to C return `RulesError::Unsupported` under Quicksilver instead of guessing.

### Changes that touch no number touchstone computes

These exist in Quicksilver but MME has no function for them, or MME already answers the same way.
They are not carded.

- **Rank-weighted monster targeting** (`crates/silvermere-rules/src/monster_ai.rs`, `threshold`):
  which party member a monster attacks. MME's party damage averages over a party and has no front,
  middle or back.
- **`guard <player>`** (`v120/guard.rs`): interception of monster blows. No MME counterpart.
- **No Anti-Magic roll on friendly casts** (`spells/resist.rs`, the `friendly` flag): MME's
  `vs_anti_magic` describes the target of a damage spell only.
- **Signed player magic resistance.** MME's `calculate_resist_damage` already adds damage below 50
  magic resistance for any target, so the Quicksilver fix changes nothing here.
- **Stat-lowering spells stop at 0, energy clamped at 0, maximum HP in 32 bits.** MME has no
  unsigned wrap to fix. MME's `calc_max_hp` already works in Long, and returns 0 on overflow where
  Quicksilver does not overflow; that needs inputs far beyond the game's range.
- **Text, menus, death penalties, sysop and config.** Not game numbers.

## 2. Changes that need no profile (the live data carries them)

A Quicksilver database already holds these, and the existing loader and ability code read them.
Do not port them.

- **Spell Mod v2.1** for the Mage, Priest and Druid player spells (132 spells), including detect
  magic becoming protective aura, dark plague becoming dark binding, and slow and solid fog no
  longer adding energy use from strength. These are spell rows.
- **Monster spell copies** (spell numbers 5000 to 5999 are reserved for them). They are ordinary
  spell rows, and negating items and removers already point at them in the data. Lists that show
  "player spells" should keep ignoring the reserved range; that is a filter question, not a rule.
- **The mana-regeneration spell line** (spells 6000 to 6020 and the scrolls that teach them, shops
  and messages). They grant the stock "Alter Mana Regeneration" ability, which `calc_mana_regen`'s
  `mp_regen` input already takes.
- **Race regeneration** (Human, Dwarf, Elf, Gaunt One): race ability slots with the stock healing
  rate and mana regeneration abilities. They feed `calc_resting_rate` and `calc_mana_regen` through
  the ability sums the gear and pipeline code already do.
- **Spell level caps** (player spells capped at `scalecap + 5`), **new and changed items,
  monsters, rooms, shops, texts and actions**, and renamed things. All rows.
- **Class records.** The 1.20 class file is empty; classes are stock rows.

One check belongs to data, not rules: confirm every ability number the 1.20 content uses is known
to touchstone's ability tables. Card Q-002 does that against a real Quicksilver database.

## 3. How touchstone learns it is talking to Quicksilver

**Detection (already half there).** `touchstone-data`'s silvermere adapter reads
`world_import.content_version`, a column that exists only when Quicksilver's migration 9001 has
run, and today puts it into `Info.dat_file_version` (stock silvermere gets the string
"silvermere"). That string is a display caption and also feeds MME's `calc_encum` caption check, so
the profile must not be derived from it by string compare. Card Q-004 adds a structured
`Info.content_version: Option<String>` (the mdb loader sets `None`) and decides from that:

| Source | `content_version` | Profile |
|---|---|---|
| `.mdb` or converted MME database | n/a | `Stock` |
| silvermere SQLite, migration 9001 absent | n/a | `Stock` |
| Quicksilver, a `1.20.N` value | `1.20.N` | `Quicksilver` with the 1.20 balance |
| Quicksilver, any other value | other | refuse to start with a clear message |

The last row follows the adapter's existing precedent (an unknown schema version refuses), and
matches Quicksilver, which itself refuses to serve a world it has no rules for. On a live reload
that finds an unknown version, the server keeps the previous world and logs the reason.

**Where the server picks it.** In `bin/touchstone/src/state.rs`, `Loaded::build` already holds the
fresh `World`; it gains `profile: Profile` computed once per load. Every call site that passes
`Profile::Stock` today (about 55 in `bin/touchstone/src`) reads `loaded.profile` instead; the rules
crate's irrefutable `let Profile::Stock = profile;` lines become exhaustive matches, so the
compiler lists every place that needs a decision. `/api/info` reports the profile and, for
Quicksilver, the content version, and the web header shows it.

**Config.** `touchstone.toml` gains an optional section:

```toml
[rules]
profile = "auto"            # auto (default) | stock | quicksilver
[rules.quicksilver_balance] # optional: only the keys that differ from the version's balance
```

`stock` forces the stock rules even on a Quicksilver database, for comparisons. The balance
override uses Quicksilver's own `[balance]` key names, so John can paste the table from the
Quicksilver config unchanged. An override that differs from the version's frozen balance is shown
in `/api/info`, as Quicksilver itself prints "overrides set". Unknown keys fail startup, as there.

**Type.** `Profile` is `Copy` and matched everywhere, and the balance is large (per-class tables),
so the variant carries a reference: `Profile::Quicksilver(&'static Balance)`. The 1.20 balance is a
`static`; a config override is built once at startup and leaked. `Balance` lives in
`touchstone-rules` (no IO; serde only), with `Balance::V120`, validation matching Quicksilver's,
and the neutral values listed in part 4.

**Decision to note: the V120 numbers are embedded.** They are the rules every player sees with
`HELP 1.20` and John's own. touchstone is public and Quicksilver is private, so embedding publishes
the balance numbers (not Quicksilver text or code). If that is unwanted, the type already supports
the alternative: ship no V120 constant and require `[rules.quicksilver_balance]` in the deployment
config; the tests in part 4 then use the synthetic balance only. The cards below assume embedding.

## 4. Testing a Quicksilver branch when the oracle is MME

MME knows nothing about Quicksilver, so MME fixtures can only prove the **stock** side: they keep
passing with `Profile::Stock`, and every Q card must leave them byte-for-byte untouched. For the
Quicksilver side the reference is Quicksilver's own engine, in three layers.

**Layer 1: stock identity (uses the existing MME fixtures, runs in PR CI).** Define a neutral
balance in which every parameter is set to its stock-equivalent (crit knee 40, backstab stealth
divisor 10 and level percentage 100, knees out of reach, class shares 100, opportunist and weave
chance 0, area cut 0, rest and meditate multiples as stock, soft-cap prices off). For each parametric
rule (R1, R5, R7 to R9, R11 to R14) a test runs the existing MME fixture cases with
`Profile::Quicksilver(&NEUTRAL)` and expects the MME answer. This proves the profile changes
only what it should. Structural rules with no neutral value (R2, R3, R10) are covered by layers 2
and 3 only.

**Layer 2: engine vectors (local generator, committed fixtures, never in PR CI).** Quicksilver's
rules crate is pure Rust with no IO, and its random source is a trait, so a scripted source can
enumerate every outcome of a roll and give exact expectations. A small generator links that crate
and writes JSON test vectors:

- **Where it lives.** `tools/qs-vectors/` holds only the generator's source and a shell script. The
  script builds it in `~/.cache/touchstone/qs-vectors/` with path dependencies on a Quicksilver
  checkout named by the environment variable `QUICKSILVER_DEV` (default `/home/user/quicksilver-dev`,
  read-only; the target directory is outside it). It is not a workspace member, never builds in PR
  CI, and contains no Quicksilver code, only calls into its public functions. Like `tools/mme-fetch.sh`
  it fetches nothing and commits no source from the other repository.
- **What it emits.** `fixtures/quicksilver/<family>.json`, with a header carrying the Quicksilver
  commit and content version. Each case has the inputs in touchstone's terms, the engine's answer
  under V120, and, where the rule has a neutral setting, the engine's answer under the neutral
  balance. Deterministic: seeded inputs, scripted rolls, sorted output, byte-identical on rerun,
  at most 1.5 MB per family like the MME fixtures.
- **What the tests check.** touchstone under `Profile::Quicksilver(V120)` equals the V120 answer;
  touchstone under `Profile::Stock` equals the neutral answer. The second check also screens the
  vectors: if stock touchstone disagrees with the engine run at neutral settings, the case is in a
  region where MME and the engine differ for reasons that have nothing to do with Quicksilver
  (MME's 16-bit quirks, for example), and the scenario is moved out of that region rather than
  explained away.
- **Building blocks versus assembled paths.** Vectors for the small pure functions (knees, spell
  stat scaling, training cost, mana-per-minute, the area cut) are exact. For assembled MME-shaped
  outputs (backstab min and max, swings, damage per round) the inputs are the ones MME takes
  (level, stealth, weapon min and max, speed), mapped onto Quicksilver's fighter builder.
- **Random rules.** R3, R4, R6 and R9 are expectations in touchstone and rolls in the engine. The
  generator runs the engine's own roll code under a scripted source over the whole roll space and
  writes the exact mean; touchstone's closed form must match within a stated tolerance (a rounding
  allowance, not statistical).

**Layer 3: end-to-end cells (optional, local).** Quicksilver's balance tool prints, for a race,
class, level and kit, the stats, gear and damage per round against a band target. A few cells per
class group are re-created through touchstone's character pipeline and compared with a loose
tolerance. The tool's definitions differ (steady state, whole-fight kill rates), so this guards
against gross error and wiring mistakes, not formula detail.

**Within the hard rules.** Fixtures hold numbers only: inputs and outputs, no Quicksilver source,
text or tables, and the generator source holds no pasted code. Quicksilver files are named by path
in comments and docs. No MME source or data is involved. No manual steps: the generator runs in a
cloud session that has the Quicksilver checkout, and a mismatch it cannot resolve goes in
`fixtures/known_gaps.json` with the case and the best explanation. The `Balance` struct and V120
constant in `touchstone-rules` are written from the documented key list and checked against the
vectors, not copied from source.

## 5. Cards

Sizes as in `docs/inventory.md` section 8: S under 300 lines, M under 1,000, L over 1,000 (split).
Every rule card (Q-006 on) adds its fixtures under `fixtures/quicksilver/`, a test module under
`crates/touchstone-rules/tests/all/`, and a note in `docs/differences.md` for any approximation. All
touch `docs/differences.md` only to append.

| Card | Title | Depends | Rows | Touches | Size |
|---|---|---|---|---|---|
| Q-002 | Quicksilver database fetch and data census | Q-001 | section 2 | `tools/qs-fetch.sh`, `tools/qs-census.py`, `docs/data-gaps.md`, `crates/touchstone-data/tests/` | S |
| Q-003 | Profile variant, Balance type and V120 | Q-001 | all | `crates/touchstone-rules/src/lib.rs`, `crates/touchstone-rules/src/quicksilver/` (new), every `let Profile::Stock` site in `crates/touchstone-rules/src/` made exhaustive with no behaviour change, tests | M |
| Q-004 | Profile detection, selection and API | Q-003 | part 3 | `crates/touchstone-model/src/tables.rs` (`Info.content_version`), `crates/touchstone-data/src/`, `bin/touchstone/src/` (`state.rs`, `config.rs`, `api.rs` and the `Profile::Stock` call sites), `docs/design.md` | M |
| Q-005 | Engine vector generator and fixture layout | Q-003 | part 4 | `tools/qs-vectors/`, `fixtures/quicksilver/` (layout and the knee family as the first vectors), `crates/touchstone-rules/tests/all/quicksilver_identity.rs` (layer 1 harness) | M |
| Q-006 | Knees: crit, magic resistance, dodge | Q-003, Q-005 | R11, R12, R13 | `attack.rs`, `stats.rs` (`calc_mr`), `accuracy.rs` (`calc_dodge`), `gear/second.rs`, `paste_party.rs`, `char_panel.rs` (callers), tests, `fixtures/quicksilver/knees.json` | M |
| Q-007 | Stat soft caps and training prices | Q-003, Q-005 | R14 | `stats.rs`, `char_panel.rs`, tests, `fixtures/quicksilver/training.json` | S |
| Q-008 | Rest and meditate regeneration | Q-003, Q-005 | R10 (panels and recovery rounds) | `stats.rs`, `spell_cast.rs` (`calc_mana_recovery_rounds`), `char_panel.rs`, tests, `fixtures/quicksilver/regen.json` | M |
| Q-009 | Backstab damage and crit | Q-003, Q-005, Q-006 | R1, R3 | `attack.rs`, `calculators.rs`, `detail/`, tests, `fixtures/quicksilver/backstab.json` | M |
| Q-010 | Backstab round: energy and the opener | Q-009 | R2 | `attack.rs`, `combat.rs`, `detail/monster.rs`, tests, `fixtures/quicksilver/opener.json` | M |
| Q-011 | Class melee share and Thief opportunist | Q-003, Q-005, Q-009 | R4, R5 | `attack.rs`, `combat.rs`, tests, `fixtures/quicksilver/melee.json` | M |
| Q-012 | Spell damage: casting stat, class share, area resist | Q-003, Q-005, Q-006 | R7, R8, R9 | `spell_cast.rs` (`SpellCasterProfile` gains casting stat, class and level), `character.rs`, `detail/spell.rs`, tests, `fixtures/quicksilver/spells.json` | M |
| Q-013 | Weave | Q-011, Q-012 | R6 | `crates/touchstone-rules/src/quicksilver/weave.rs`, `combat.rs`, `spell_cast.rs`, tests, `fixtures/quicksilver/weave.json` | L (split: pick and volley expectation, then integration into `get_damage_output`) |
| Q-014 | Exp/hr under Quicksilver (Model D) | Q-008, Q-010, Q-013 | R2, R6, R10 | `exp_per_hour/` (profile gate for Models A to C, Model D recovery and procs), `docs/differences.md`, tests, `fixtures/quicksilver/exphr_d.json` | M |
| Q-015 | Web: show the profile | Q-004, Q-014 | part 3 | `web/` (header badge, hide Models A to C and Quicksilver-only text under the profile), web tests | S |
| Q-016 | End-to-end cells against the balance tool | Q-013, Q-014 | layer 3 | `tools/qs-vectors/`, `fixtures/quicksilver/cells.json`, `crates/touchstone-rules/tests/all/quicksilver_cells.rs` | M |

Order: Q-002 to Q-005 first (Q-002 is independent and can run beside Q-003), then Q-006 to Q-008 in
parallel, then the combat chain Q-009, Q-010, Q-011, the spell card Q-012, then Q-013, Q-014 and
the optional Q-015 and Q-016. Q-006, Q-009 and Q-011 all edit `calculate_attack`; the dependencies
above serialise them so they do not conflict.
