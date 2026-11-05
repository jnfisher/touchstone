# MME inventory: features, formula families, character state, proposed cards

Source: MME pin `28d23d70caf4f971418aa7ea25471cdb0ab0888a` (v2.3.5), index from `tools/mme-index` (T-002): 42 modules, 1,709 procedures, 69,820 procedure lines. Generated from the index by a throwaway script (not committed); procedure names below are MME's own.

Conventions: **FEAT-** = a feature the user sees (event handlers and screen code). **FAM-** = a formula family: computation that can be ported and fixture-tested by itself. **Lines** = procedure lines from the index (blank lines and comments included). "Stock only" means only the non-GreaterMUD branches are ported (design.md); branch counts show how much GreaterMUD/NMR-version code sits inside each family.

## 1. Summary

| class | groups | procedures | lines |
|---|---|---|---|
| port | 55 | 1,089 | 49,253 |
| optional | 7 | 231 | 11,579 |
| skip | 6 | 389 | 8,988 |
| total | 68 | 1,709 | 69,820 |

- **34 features** and **27 formula families**; plus 7 non-port lists (skip/absorbed/harness, section 7).
- Proposed cards: **44 phase 3** (formulas), **34 phase 4** (web UI), 7 optional (section 8c).
- Every procedure is in exactly one group (appendix A); `unclassified` is empty.

## 2. UI map

| Where | What | Feature |
|---|---|---|
| Nav F1 | Char | FEAT-CHAR |
| Nav F2 | "<->" EQ + Char hybrid view | FEAT-SHELL |
| Nav F3 | EQ (equipment, Item Manager, Find Best) | FEAT-EQ |
| Nav F4 | Lists (compare panes) | FEAT-COMPARE |
| Nav F5 to F7 | Weapons, Armour, Spells | FEAT-WEAP, FEAT-ARM, FEAT-SPELL |
| Nav F8 to F9 | Class/Race, Sundry | FEAT-CLS, FEAT-SUND |
| Nav F10 to F12 | Monsters, Shops, Rooms | FEAT-MON (+FEAT-MFILT), FEAT-SHOP, FEAT-ROOMS |
| Top strip | Global class/race/level/alignment filter | FEAT-GFILT |
| File menu | Open Data File, Recent Databases, Load / Revert / Save / Save As / Close Character, Exit | FEAT-SHELL, FEAT-MMEC, FEAT-LOADCHAR |
| Options menu | Paste Character; Calculate Reachability; Calc Monster Dmg vs Char / Party; Clear Calculated Monster Dmg; Clear Learned Spells; Reset All Character Fields; Import/Export NMR; Reload MME; Remove All Filters; Settings; Jump to Equip/Lists on Add; Show shops first | FEAT-PASTE, FEAT-REACH, FEAT-SHELL (dispatch) with FAM-13, FEAT-NMR, FEAT-MFILT, FEAT-SETTINGS, FEAT-COMPARE, FEAT-SHOP |
| Tools menu | Backstab Calc, Coin Converter, External Map, Exp Calc, Hit Calc, Notepad, Swing Calc, Spell Book | FEAT-BS, FEAT-COIN, FEAT-MAP, FEAT-EXPCALC, FEAT-HIT, FEAT-NOTEPAD, FEAT-SWING, FEAT-SPELLBOOK |
| Help menu | About, Donate, Release Notes, Check for Updates, Contribute to Exp/Hour Modeling, What's NOT implemented | FEAT-SHELL (mnuHelp_Click); About/Release-notes forms are skip |
| Right-click menus | Items, Spells, Aux (monsters/shops/references), Map, Find Best | FEAT-CTX, FEAT-EQ |
| Dialogs | Combat setup (attack/heal), Extra monster filters, Load-character options, Paste Character, Settings, Results tree, Edit map preset | FEAT-COMBAT, FEAT-MFILT, FEAT-LOADCHAR, FEAT-PASTE, FEAT-SETTINGS, FEAT-RESULTS, FEAT-MAP |
| Monster simulator | Monster Attack Simulator window; Monster Dmg vs Char column | FEAT-MSIM; FAM-12/13 |

## 3. Features

| ID | Feature | What the user sees | Entry procedures | Own procs / lines | Families reached | Class / phase |
|---|---|---|---|---|---|---|
| FEAT-SHELL | Application shell | Window with 12 nav buttons F1 to F12 (Char, "<->" EQ+Char hybrid, EQ, Lists, Weapons, Armour, Spells, Class/Race, Sundry, Monsters, Shops, Rooms); File/Options/Tools/Help menus; recent databases and character files; back/forward navigation history; command-line .mdb/.mmec; data-version info. | Main, Form_Load, cmdNav_Click, mnuFileOptItem_Click, mnuOptionsItems_Click, mnuToolsItems_Click, mnuOpenDataFile_Click, RefreshAll | 25 / 2,532 | n/a (dispatcher) | port / 4 |
| FEAT-WEAP | Weapons explorer (F5) | Weapon list with computed columns, ability filter, speed/extras options, combo weapon calc, location (reference) list, detail pane. | FilterWeapons, lvWeapons_ItemClick, cmbWeaponCombos_Click, chkWeaponOptions_Click | 9 / 109 | FAM-20 | port / 4 |
| FEAT-ARM | Armour explorer (F6) | Armour list with AC/DR column (alternates AC/DR sort), ability filter, locations, detail. | FilterArmour, lvArmour_ItemClick, cmbArmorAbilityList_Click | 6 / 68 | FAM-20, FAM-21 | port / 4 |
| FEAT-SPELL | Spells explorer (F7) | Spell list, class/magery options, learned toggle, "what casts this", counters finder (negates/dispels), locations, detail, spell note. | FilterSpells, lvSpells_ItemClick, FindNegates, LookUpSpellCast, chkSpellOptions_Click | 13 / 410 | FAM-05, FAM-09, FAM-10, FAM-19, FAM-20, FAM-21, FAM-23 | port / 4 |
| FEAT-MON | Monsters explorer (F10) | Monster list (solo/lair/party modes, exp/hr, damage columns), detail pane with reachability and route, locations, regen lookup, help. | FilterMonsters, lvMonsters_ItemClick, cmdFilter_Click, lvMonsterDetail_DblClick, LookUpMonsterRegen | 8 / 448 | FAM-11, FAM-13, FAM-19, FAM-20, FAM-21, FAM-23 | port / 4 |
| FEAT-MFILT | Monster list filters (inline and extra) | Inline filters on the Monsters tab (regen, magic, damage, HP, exp for mob/solo lair/party lair, lair text) and the Extra Filters dialog (AC, DR, MR, BS def, game limit, accuracy, dodge, lairs, mobs/lair, follow %, abilities, undead, non-hostile, drops cash, reachable only, attack-type exclusions). | cmdMonsterFilterOps_Click, MonsterFilterFormAction, cmdExec_Click, optMonsterFilter_Click, ResetFilterOptions | 84 / 1,291 | FAM-03, FAM-11, FAM-23, FAM-25 | port / 4 |
| FEAT-SHOP | Shops explorer (F11) | Shop list, shop item detail with charm-adjusted price, markup, locations. | lvShops_ItemClick, cmdShopButtons_Click, txtShopCharm_Change, RefreshShops | 13 / 226 | FAM-20, FAM-21 | port / 4 |
| FEAT-SUND | Sundry/Items explorer (F9) | All non-weapon/armour items, chest contents, ability filter, item references. | FilterSundry, lvOtherItems_ItemClick, cmdSundryChests_Click | 8 / 389 | FAM-05, FAM-19, FAM-20, FAM-21, FAM-23 | port / 4 |
| FEAT-CLS | Class/Race explorer (F8) | Class and race lists, details, and the class/race stat builder (min/max stats, CP). | lvClasses_ItemClick, lvRaces_ItemClick, RefreshClassRaceBuilder, cmdClassRace_Click | 8 / 529 | FAM-01, FAM-02, FAM-07, FAM-20, FAM-21 | port / 4 |
| FEAT-ROOMS | Rooms (text) explorer (F12) | Room/map number lookup, text find, rooms-with-exits finder, room details with references. | cmdRoomsButtons_Click, GotoLocation, FindRoomWithDirections, cmdMapFindText_Click | 9 / 782 | FAM-19, FAM-23 | port / 4 |
| FEAT-COMPARE | Lists/Compare panes (F4) | Saved lists: weapon, armour, spell and monster compare panes with difference highlighting; add/remove/clear; jump on add. | CompareAddItem, CompareAddMonster, CompareAddShopItems, cmdCompareNav_Click, lvWeaponCompare_ItemClick | 26 / 972 | FAM-05, FAM-19, FAM-20, FAM-21, FAM-25 | port / 4 |
| FEAT-EQ | EQ tab and Item Manager (F3) | 20 equipment slots, stat totals, inventory manager list (carried/stash), find-best-item search (armour, attack, stats, resist, mystics), copy inventory, item usability colouring. | InvenFindBest, cmdEquipButtons_Click, InvenEquipItem, LoadInvenItems, lvItemManager_ItemClick | 37 / 1,265 | FAM-02, FAM-05, FAM-20, FAM-23, FAM-25 | port / 4 |
| FEAT-CHAR | Char tab (F1) | Name, 6 stats with adjustments, regen, MR, anti-magic, quests, bless slots (10), HP/mana/dodge/MR/CP panels, copy character to clipboard. | cmdCharButtons_Click, txtCharStats_Change, cmbCharBless_Click, chkCharQuests_Click, FilterBlessSpells | 16 / 585 | FAM-01, FAM-04, FAM-05, FAM-08, FAM-09, FAM-10, FAM-11, FAM-13, FAM-25 | port / 4 |
| FEAT-GFILT | Global filter | Class, race, level, alignment, min level filter shown on every explorer; "use character" toggle. | chkGlobalFilter_Click, cmbGlobalClass_Click, cmbGlobalRace_Click, txtGlobalLevel_Change, FilterAll | 9 / 439 | FAM-05, FAM-23, FAM-26 | port / 4 |
| FEAT-COMBAT | Combat setup dialog | Choose attack (weapon, spell level, martial art, manual P/M damage), backstab weapon, heal spell/rounds/manual, meditate; recalculates combat stats live. | PopUpChooseCombatGUI, RefreshMonsterCombatGUI, frmPopUpOptions.SetupChooseAttack, RefreshCombatStats, cmdContinue_Click | 46 / 1,330 | FAM-02, FAM-03, FAM-05, FAM-07, FAM-09, FAM-10 | port / 4 |
| FEAT-CTX | Context menus and clipboard actions | Right-click actions on item, spell, monster, shop, map lists: equip, add to list/manager, copy name/details, calculate swings/backstab, what casts, counters, hit calc vs mob, set combat spell/weapon vs mob, where summoned. | mnuAuxPopUpItem_Click, mnuItemsPopUpItem_Click, mnuSpellsPopUpItem_Click, PopUpAuxMenu | 13 / 1,442 | FAM-01, FAM-05, FAM-10, FAM-13, FAM-20, FAM-21, FAM-22 | port / 4 |
| FEAT-LISTS | List helpers: colours, selection, search, copy | Learned-spell, monster, item-manager, weapon and armour row colouring; Item Manager quantity/action cells; select all/invert/delete; search in list; copy list or line to clipboard. | RefreshListviewItemColors, LV_SetActionCell, SearchLV, CopyWholeLVtoClipboard, ColorListviewRow | 22 / 1,196 | FAM-03, FAM-04 | port / 4 |
| FEAT-MMEC | Character files (.mmec) | Save, save as, load, revert, close; auto-load last; INI sections PlayerInfo, MonsterFilter, Bless, LearnedSpells, Inventory, Compare and the data-version section; INI reader/writer. | LoadCharacter, SaveCharacter, ReadINI, WriteINI, frmLoadChar | 4 / 1,128 | FAM-05, FAM-08, FAM-09, FAM-13, FAM-21, FAM-22, FAM-23 | port / 4 |
| FEAT-LOADCHAR | Load-character options dialog | Choose to also load inventory/compare lists and filter behaviour when loading a character. | cmdContinue_Click, Form_Load | 5 / 68 | none | port / 4 |
| FEAT-NMR | NMR character import/export | Import and export a character in the NMR format (NMR is the sysop tool that exports the game database MME reads). | NMR_Import, NMR_Export | 2 / 830 | FAM-01, FAM-02, FAM-08, FAM-13, FAM-22, FAM-23 | optional / 6 |
| FEAT-PASTE | Paste Character dialog | Paste character/party text, party size, per-member stats, "assume mega damage", apply to Char tab or party defaults. | cmdContinue_Click, cmdPaste_Click, cmdPasteMegaDmg_Click | 52 / 415 | FAM-25 | port / 4 |
| FEAT-HIT | Hit Calculator | Hit chance of attacker vs defender (character or monster), shadow, see hidden, vile ward, evil, monster picker. | DoHitCalc (FAM-26), cmbMonsterList_Click, cmdCharHitCalc_MouseDown | 23 / 241 | FAM-26 | port / 4 |
| FEAT-SWING | Swing Calculator | Swings per round for a weapon from agility, strength, encumbrance, speed, level; paste MegaMUD stat line; copy result. | CalcSwings (FAM-27), cmdPasteMega_Click, cmdCopytoClip_Click, AlterLevel | 43 / 538 | FAM-02, FAM-07, FAM-22, FAM-27 | port / 4 |
| FEAT-BS | Backstab Calculator | Backstab min/max damage from level, stealth, weapon, strength. | CalcBS (FAM-27), cmdCopytoClip_Click, AlterLevel, WriteStealth | 38 / 501 | FAM-02, FAM-27 | port / 4 |
| FEAT-EXPCALC | Exp Calculator | Exp needed between levels for class/race. | CalcExp (FAM-27), cmdCalcExp_Click, txtCalcEXPTable_KeyUp | 15 / 186 | FAM-01, FAM-27 | port / 4 |
| FEAT-COIN | Coin Converter | Converts between runic/platinum/gold/silver/copper, with charm adjustment. | CalcCoin, ConvertCoin, cmdCharm_Click | 13 / 140 | FAM-27 | port / 4 |
| FEAT-SPELLBOOK | Spell Book | Spells a class/level may learn or cast, with the character filter. | cmdListSpells_Click, cmbClass_Click, CopyGlobalChar | 14 / 436 | FAM-05, FAM-09, FAM-21, FAM-25 | port / 4 |
| FEAT-MSIM | Monster Attack Simulator window | Simulate a monster against user-defined AC, DR, dodge, MR, anti-magic, vile ward, resists, rounds; combat log. | cmdRunSim_Click, ResetFields, cmdResetUserDefs_Click | 34 / 419 | FAM-05, FAM-06, FAM-12, FAM-13 | port / 4 |
| FEAT-SETTINGS | Settings and exp/hr knobs | Settings dialog (model checkboxes A-D, XP/DMG/Mana/Move knobs, sim rounds, route waypoint interval, auto-load, recreate INI) and settings.ini load/save. | cmdSave_Click, LoadSettings, SaveSettings, LoadExpPerHourKnobs, ResolveSettingsPath | 31 / 1,104 | FAM-23 | port / 4 |
| FEAT-RESULTS | Results window (textblock trees) | Tree of textblock execution / command nodes for "what leads here", monster summons, find-by-room results; double-click navigates. | SetupResultsWindow, CreateExecutionTree, CreateCommandTree, tvwResults_NodeClick | 21 / 1,422 | FAM-03, FAM-19 | optional / 6 |
| FEAT-NOTEPAD | Notepad | Free-text notes saved inside the character file. | cmdDO_Click, Form_Load | 5 / 183 | none | optional / 6 |
| FEAT-MAP | Graphical map / room explorer | Draws rooms, walk with arrows/cardinals, exits, presets, what-leads-here, external map window. | frmMap procedures, MapStartMapping, MapMapExits, cmdMapLeadsHere_Click, LoadPresets | 72 / 4,952 | FAM-10, FAM-11, FAM-13, FAM-19, FAM-20 | optional / 6 |
| FEAT-MEGA | MegaMUD pathing | Records a walk as a MegaMUD path file; room codes, history, undo, copy; scans a MegaMUD rooms folder. | txtMapMove_KeyPress, cmdMapAddMegaCodes_Click, MapMegaScanPath, cmdMapMegaRoomFind_Click | 30 / 1,347 | FAM-19 | optional / 6 |
| FEAT-REACH | Reachability and routes | Monster reachability by level, route from nearest bank/POI to a room or monster, route text. | ReachEnsureComputed, ComputeReachability, RouteSearch, ShowMonsterRoute, AddMonsterReachRows | 95 / 2,463 | FAM-19 | optional / 6 |

Notes: entry procedures are the event handlers listed in the index; the computation they reach lives in the families (section 4). Row builders (Add*2LV), list filters (Filter*) and detail builders (Pull*Detail) are the families FAM-21, FAM-23 and FAM-20, not feature procedures. "Families reached" follows calls from the feature's own procedures, through the right-click menu and global-filter handlers. FEAT-SHELL's `Form_Load` and `SetUpFormObjects` (937 lines together) are mostly control layout.

## 4. Formula families

Lines are procedure lines; FAM-12 counts only non-accessor procedures (its 136 procedures include ~120 one-line property accessors).

| ID | Family | What it computes | Procs | Lines | Key procedures | Outputs | Depends on (calls) | Branch refs (stock only ported) | Shim size |
|---|---|---|---|---|---|---|---|---|---|
| FAM-01 | Exp tables and training cost | Exp needed per level for a race/class exp table, exp modifiers, max level, gold to train. Stock and two GreaterMUD variants. | 11 | 462 | CalcExpNeeded_STOCK, GetExpModifiers_STOCK, CalcExpNeededByRaceClass | exp needed (Double/Currency), money to train | none | 2 procs GMUD, 1 datver | T2 G1 |
| FAM-02 | Stats, vitals, CP, movement | Max HP and mana, spellcasting, MR, picklocks, stealth, resting and mana regen rates, encumbrance and its penalties, movement speed, creation-point costs and stat ceilings per race/class. | 33 | 931 | CalcMaxHP, CalcRestingRate, CalcManaRegen, CalcMovementSpeed, CalcEncum, CalculateStealth, ComputeAvgLevelMaxStats | single numbers (Long/Double); sReturnText for stealth | none | 6 procs GMUD | T2 G1 C4 |
| FAM-03 | Item and spell ability decoding | Ability number to stat effect (what AbilityEffectsCharStats says feeds character stats), ability display text with values, ability lookups on items, spells, races, classes, and the ability-to-stat-slot map. | 10 | 1,164 | GetAbilityName, AbilityEffectsCharStats, GetAbilityStats, GetAbilityStatSlot, ItemHasAbility | booleans, ability text, tAbilityToStatSlot | FAM-10, FAM-19 | 6 procs GMUD | T4 G2 |
| FAM-04 | Gear aggregation (CalcCharacterStats) | Sums equipped gear, bless spells, manual adjustments and quest rewards into the aggregate stat vector (lblInvenCharStat 0..46) and the nGlobalChar* weapon/accuracy globals. Reads and writes controls throughout; the single largest piece of logic outside the calculators. | 10 | 1,824 | CalcCharacterStats, ItemIsUsableByChar, AccumItemStatBonus | aggregate stat vector, nGlobalChar* globals | FAM-02, FAM-03, FAM-05, FAM-06, FAM-07, FAM-08, FAM-09, FAM-26 | 2 procs GMUD | T3 G37 C26 |
| FAM-05 | Character profile and panel derivations | Builds tCharacterProfile from the globals/controls (the input to every damage formula), picks the current attack/heal configuration, and derives the Char-tab panels (HP, mana, MR, dodge, CP cost, picklocks, class quest defaults). | 23 | 1,427 | PopulateCharacterProfile, SetCurrentAttackTypeConfig, RefreshCPs, RefreshHitPoints, RefreshMagic, SetupClass | tCharacterProfile, attack config string, panel values | FAM-01, FAM-02, FAM-03, FAM-04, FAM-06, FAM-08, FAM-09, FAM-10 | 6 procs GMUD, 2 NMR-ver | T2 G39 C41 |
| FAM-06 | Accuracy, hit chance and dodge | Character accuracy from stats/class, hit chance of an attack against AC and dodge, backstab accuracy, dodge vs accuracy, dodge caps and diminishing returns, accuracy needed to land X percent. | 12 | 557 | CalculateAccuracy, CalculateAttackDefense, CalcDodgeVSAccuracy, DodgeMaxAccForPercent | Long percentages | FAM-02 | 11 procs GMUD | pure args |
| FAM-07 | Swings, energy and weapon damage | Swings per round from speed, energy and encumbrance, hit/crit/extra-hit damage per attack type (punch, kick, jumpkick, normal, bash, smash, surprise), Quick and Deadly, backstab damage, true average. | 9 | 991 | CalculateAttack, CalcEnergyUsed, CalcQuickAndDeadlyBonus, CalcBSDamage | tAttackDamage | FAM-01, FAM-02, FAM-03, FAM-06, FAM-10 | 4 procs GMUD, 2 datver | T1 G26 C5 |
| FAM-08 | Damage vs monster and combat rounds | Per-monster damage output of the current attack config (physical, spell, backstab, surprise, min/first round), rounds to kill and to die with mob HP regen and multi-mob rules, killable test. | 11 | 618 | GetDamageOutput, CalcCombatRounds, IsMobKillable | tDamageOutput, tCombatRoundInfo | FAM-03, FAM-05, FAM-07, FAM-09 | 3 procs GMUD, 1 NMR-ver | T2 G8 C5 |
| FAM-09 | Spell casting | Cast chance, damage after resist/anti-magic, rounds to run out of mana, mana recovery, whether a spell is usable or in game, per-cast and per-round results. | 11 | 897 | CalculateSpellCast, CalculateResistDamage, GetSpellCastChance, CalcRoundsToOOM, SpellIsUsable | tSpellCastValues | FAM-02, FAM-03, FAM-10 | 4 procs GMUD, 2 NMR-ver | T1 G4 |
| FAM-10 | Spell damage equations | Spell min/max damage and duration at a cast level (the level-increment equations), spell attack type, heal vs damage, area, bless. | 9 | 961 | PullSpellEQ, GetCurrentSpellMinMax, GetSpellMinDamage, GetSpellMaxDamage | SpellMinMaxDur, display string | FAM-01, FAM-03 | 1 procs GMUD | T1 G2 C2 |
| FAM-11 | Lairs | Lair statistics from the monster Locations text: mobs per lair, regen, average exp/HP/damage, outlier removal and medians, party lair damage; produces the LairInfoType that exp/hr and the monster list consume. | 16 | 1,213 | GetLairInfo, LoadLairInfo, GetLairAveragesFromLocs, CalculateMonsterPartyDamage | LairInfoType | FAM-08, FAM-13 | 1 procs GMUD, 4 NMR-ver | T3 G5 C5 |
| FAM-12 | Monster attack simulator (class) | Self-contained round simulator: monster attacks, between-round spells, hit/dodge/AC/DR/MR/anti-magic, energy per round, optional dynamic rounds and combat log. Inputs are properties set by the caller, so it is the easiest family to fixture. | 136 | 1,120 | RunSim, GetMaxDamage, CalcResistedDamage, IsSpellResisted | average/max damage, combat log text | none | 4 procs GMUD | G1 |
| FAM-13 | Monster sim glue | Loads a monster (attacks, spells, item bonuses) into the simulator, configures the simulator from the loaded character, caches damage vs character/party per monster, attack summary text. | 11 | 816 | PopulateMonsterDataToAttackSim, SetupMonsterAttackSimWithCharStats, CalculateMonsterDamageVsChar, GetMonsterAttackSummary | MonAttackSimReturn, MonsterAttackSummary | FAM-03, FAM-05, FAM-06, FAM-09, FAM-10, FAM-12 | 3 procs GMUD, 3 NMR-ver | T2 G7 C11 |
| FAM-14 | Exp/hr shared helpers and Model D | Clamp/safe-divide helpers used by every model, Model D (round-by-round simulation, the recommended model) and its overkill fraction. | 16 | 463 | ceph_ModelD, cephD_OverkillFrac | tExpPerHourInfo | none | none | pure args |
| FAM-15 | Exp/hr Model C | Per-lair cycle macro-simulation: combat profile, cycle profile, per-hour roll-up. | 3 | 909 | ceph_ModelC, cephC_BuildCycleProfile, cephC_BuildCombatProfile | tExpPerHourInfo | FAM-14 | 1 procs GMUD | G1 |
| FAM-16 | Exp/hr Model A | Closed-form model with HP-recovery rounds and mana pool (one 1,072-line procedure). | 3 | 1,227 | ceph_ModelA, cephA_CalcHPRecoveryRounds | tExpPerHourInfo | FAM-14 | 1 procs GMUD | G1 |
| FAM-17 | Exp/hr Model B | Smoothed band-aware closed form with hand-tuned constants (one 1,014-line procedure). Docs say overfit, do not extend. | 9 | 1,287 | ceph_ModelB, cephB_CalcTravelLoopSecs | tExpPerHourInfo | FAM-14 | 1 procs GMUD | G1 |
| FAM-18 | Item value and shop price | Copper cost of an item at a shop or from the item row (charm, markup, currency types, sell value, shop regen). | 4 | 349 | GetItemValue, GetItemCopperCost, GetShopMarkup | tItemValue | none | 1 procs GMUD | T2 G1 |
| FAM-19 | Textblock, room-command and reference parsing | Parses the NMR text columns (textblocks, room commands, Obtained From, chests, Locations) and builds the "where is this" reference rows. | 22 | 1,635 | GetLocations, SplitRoomCmdSources, GetTextblockCMDS, TextBlockHasTeleport | row text/ids | FAM-11, FAM-18 | 1 procs GMUD, 1 NMR-ver | T3 C2 |
| FAM-20 | Detail pane builders | Builds the text of the detail pane for a monster, item, spell, shop, class, race and the weapon/armour compare text. Much arithmetic is in here (monster exp/hr, damage lines, item value, spell numbers). | 10 | 3,525 | PullMonsterDetail, PullItemDetail, PullSpellDetail, PullShopDetail | detail text | FAM-01, FAM-02, FAM-03, FAM-05, FAM-07, FAM-08, FAM-09, FAM-10 (+5) | 3 procs GMUD, 3 NMR-ver | T11 G9 C18 |
| FAM-21 | Row evaluators (list columns) | One routine per list builds a row: all computed columns (damage, hit%, exp/hr, price, colours, sort tags). This is the body of the planned /api/evaluate. | 11 | 1,146 | AddMonster2LV, AddWeapon2LV, AddSpell2LV, AddArmour2LV | list row cells | FAM-02, FAM-03, FAM-07, FAM-08, FAM-09, FAM-10, FAM-11, FAM-13 (+1) | 1 NMR-ver | T6 G7 C11 |
| FAM-22 | Game-text parsers | Parses pasted MegaMUD inventory/keys/notice text and stat lines into items, quantities and room groups, consolidates, finds best shop price for a hit. | 58 | 1,687 | ParseGameTextInventory, ConsolidateGroundByRoom, ParseCountAndName, ParseMegaStatLine | arrays of parsed items | FAM-04, FAM-18, FAM-19 | none | T1 G2 C2 |
| FAM-23 | List filters | The filter semantics of each explorer tab (which rows pass) given the filter controls, global class/race/level filter and monster lair mode. | 7 | 1,520 | FilterMonsters, FilterWeapons, FilterSpells, FilterArmour | row inclusion | FAM-02, FAM-04, FAM-05, FAM-07, FAM-08, FAM-09, FAM-11, FAM-13 (+2) | 1 procs GMUD, 2 NMR-ver | T4 G54 C48 |
| FAM-24 | Exp/hr dispatcher | CalcExpPerHour: runs the enabled models (A to D), averages them, applies the four knobs. | 1 | 233 | CalcExpPerHour | tExpPerHourInfo | FAM-08, FAM-14, FAM-15, FAM-16, FAM-17 | none | pure args |
| FAM-25 | Paste character and party | Parses pasted character stat blocks and party text into stats, equipment and party averages (writes the Char tab). | 9 | 1,803 | PasteCharacter, ParsePasteParty, CalculateAverageParty, TestPasteChar | char fields, party averages | FAM-02, FAM-03, FAM-04, FAM-06, FAM-07, FAM-08, FAM-09, FAM-13 (+1) | 1 procs GMUD, 1 NMR-ver | T2 G9 C33 |
| FAM-26 | Hit calculator core | Hit calculator numbers (attacker/defender mode, shadow, see-hidden, vile ward, monster data). | 5 | 509 | SetHitCalcVals, DoHitCalc, GetMonsterData | hit chance text/values | FAM-02, FAM-03, FAM-05, FAM-06 | 2 procs GMUD, 1 NMR-ver | T1 G14 C19 |
| FAM-27 | Calculator cores: swings, backstab, coin, exp | The arithmetic of four small calculators that lives in their forms. | 5 | 427 | CalcSwings, CalcBS, CalcCoin, CalcExp | calculator outputs | FAM-02, FAM-03, FAM-05, FAM-07 | 2 procs GMUD | T3 G6 C39 |

Shim size key: T = recordset tables read, G = module globals read, C = form controls touched; "pure args" = all inputs are parameters. Family-level dependency cycles in VB (FAM-04 <-> FAM-05: gear aggregation calls the panel refreshers, and refreshers call back into aggregation) disappear in Rust because rules are pure; the card order in section 8 breaks them: aggregation first, panels after.

### 4a. Family inputs (for sizing the oracle shims)

| ID | Recordset fields read (tabX: Fields) | Globals read | Controls touched |
|---|---|---|---|
| FAM-01 | tabClasses: ExpTable; tabRaces: ExpTable | nGlobalDatVer | - |
| FAM-02 | tabClasses: Abil-N, ArmourType, CombatLVL, MageryLVL, MageryType, MaxHits, MinHits, Number; tabRaces: Abil-N, BaseCP, HPPerLVL | bGreaterMUD | frmMain: chkGlobalFilter, cmbGlobalRace, cmbGlobalClass, lblDatVer |
| FAM-03 | tabClasses: Abil-N, AbilVal-N, Number; tabItems: Abil-N, AbilVal-N, Accy, ArmourClass, DamageResist, Number; tabRaces: Abil-N, AbilVal-N, Number; tabSpells: Abil-N, AbilVal-N, Number | bGreaterMUD, bHideRecordNumbers | - |
| FAM-04 | tabClasses: Abil-N, AbilVal-N, ArmourType, Name, Number, WeaponType; tabItems: Abil-N, AbilVal-N, Accy, ArmourClass, ArmourType, ClassRest-N, DamageResist, Encum, ItemType, Name (+3); tabRaces: Abil-N, AbilVal-N, Name, Number, xAGL, xCHM, xHEA, xINT, xSTR, xWIL | bCharLoaded, bGreaterMUD, nEquippedItem, nGlobalAttackAccyAdj, nGlobalAttackMA, nGlobalAttackTypeMME, nGlobalCharAccyAbils, nGlobalCharAccyItems, nGlobalCharAccyOther, nGlobalCharHitMagicNonWeapon, nGlobalCharPlusDodge, nGlobalCharPlusMR (+25) | frmMain: txtCharStats, txtGlobalLevel, lblInvenCharStat, lblLabelArray, cmbGlobalClass, cmbGlobalRace (+17); frmMonsterAttackSim: txtUserAC, txtUserDR, txtVileWard |
| FAM-05 | tabClasses: Abil-N, MageryLVL, MageryType; tabSpells: Abil-N, AbilVal-N, Cap, ManaCost, Name, Number, ReqLevel, Short | bAppTerminating, bCharLoaded, bGlobal_cephRecoveryOnly, bGreaterMUD, bPromptSave, nGlobalCharAccyAbils, nGlobalCharAccyItems, nGlobalCharAccyOther, nGlobalCharBlessSpells, nGlobalCharMagery, nGlobalCharMageryLVL, nGlobalCharPlusDodge (+27) | frmMain: lblInvenCharStat, txtCharStats, cmbGlobalClass, txtGlobalLevel, chkGlobalFilter, lblInvenStats (+35) |
| FAM-06 | - | - | - |
| FAM-07 | tabItems: Abil-N, AbilVal-N, Accy, Encum, Max, Min, Name, Number, Speed, StrReq (+1) | bHideRecordNumbers, nGlobalCharAccyAbils, nGlobalCharAccyItems, nGlobalCharAccyOther, nGlobalCharQnDbonus, nGlobalCharWeaponAGI, nGlobalCharWeaponAccy, nGlobalCharWeaponBSaccy, nGlobalCharWeaponBSmaxdmg, nGlobalCharWeaponBSmindmg, nGlobalCharWeaponCrit, nGlobalCharWeaponEncum (+14) | frmMain: lblInvenCharStat, chkGlobalFilter, cmbGlobalClass, txtCharStats, txtGlobalLevel |
| FAM-08 | tabMonsters: Abil-N, AbilVal-N, ArmourClass, BSDefense, DamageResist, MagicRes, Number, Undead; tabSpells: Abil-N, AbilVal-N | bGreaterMUD, nCharDamageVsMonster, nCharFirstRoundDamageVsMonster, nCharMinRoundDamageVsMonster, nCharSurpriseChanceVsMonster, nCharSurpriseDamageVsMonster, nCharSurpriseMinDamageVsMonster, sCharDamageVsMonsterConfig | frmMain: chkGlobalFilter, lblInvenCharStat, optMonsterFilter, txtMonsterDamageOUT, txtMonsterLairFilter |
| FAM-09 | tabSpells: Abil-N, AbilVal-N, AttType, Cap, Casted By, Classes, Diff, DurInc, DurIncLVLs, EnergyCost (+13) | bDebugExpPerHour, bDisableKaiAutolearn, bOnlyInGame, nNMRVer | - |
| FAM-10 | tabSpells: Abil-N, AbilVal-N, Ability Value 0, AttType, Cap, Dur, DurInc, DurIncLVLs, EnergyCost, Learnable (+11) | bGreaterMUD, bHideRecordNumbers | frmMain: chkGlobalFilter, txtGlobalLevel |
| FAM-11 | tabLairs: AvgAC, AvgDR, AvgDelay, AvgDmg, AvgDodge, AvgExp, AvgHP, AvgMR, AvgWalk, GroupIndex (+3); tabMonsters: Abil-N, AbilVal-N, Att%-N, AttAcc-N, AttTrue%-N, AttType-N, BSDefense, Summoned By, Undead | bGreaterMUD, bStartup, nNMRVer, sGlobalAttackConfig, tLastAvgLairInfo | frmMain: framNav, lvMonsters, chkGlobalFilter, optMonsterFilter, txtMonsterLairFilter |
| FAM-12 | - | MaxInt | - |
| FAM-13 | tabMonsters: Align, Att%-N, AttAcc-N, AttEnergy-N, AttHitSpell-N, AttMax-N, AttMin-N, AttName-N, AttTrue%-N, AttType-N (+11); tabSpells: AttType, Name, Targets, TypeOfResists | bGreaterMUD, clsMonAtkSim, nGlobalMonsterSimRounds, nMonsterDamageVsChar, nMonsterDamageVsDefault, nMonsterDamageVsParty, nNMRVer | frmMain: txtMonsterLairFilter, timWindowMove, chkGlobalFilter, optMonsterFilter, chkCharAntiMagic, cmbGlobalClass (+3); frmProgressBar: ProgressBar, lblCaption |
| FAM-14 | - | - | - |
| FAM-15 | - | bGreaterMUD | - |
| FAM-16 | - | bGreaterMUD | - |
| FAM-17 | - | bGreaterMUD | - |
| FAM-18 | tabItems: Currency, Number, Price; tabShops: %-N, Amount-N, Item-N, Markup%, Max-N, Number, ShopType, Time-N | bGreaterMUD | - |
| FAM-19 | tabItems: Name, Obtained From; tabRooms: CMD, Map Number, Room Number; tabTBInfo: Action, Called From, LinkTo, Number | - | frmMain: chkGlobalFilter, txtCharStats |
| FAM-20 | tabClasses: Abil-N, AbilVal-N, ArmourType, CombatLVL, ExpTable, MageryLVL, MageryType, MaxHits, MinHits, Name (+2); tabItems: Abil-N, AbilVal-N, Accy, ArmourClass, ArmourType, ClassRest-N, DamageResist, Destroy On Death, Gettable, ItemType (+13); tabMonsters: Abil-N, AbilVal-N, Align, ArmourClass, Att%-N, AttAcc-N, AttEnergy-N, AttHitSpell-N, AttMax-N, AttMin-N (+37); tabRaces: Abil-N, AbilVal-N, ExpTable, HPPerLVL, Name, Number, mAGL, mCHM, mHEA, mINT (+8); tabShops: %-N, Amount-N, Assigned To, ClassRest, Item-N, Markup%, Max-N, MaxLVL, MinLVL, ShopType (+1); tabSpells: Abil-N, AbilVal-N, AttType, Cap, Casted By, Classes, Diff, Learnable, Learned From, Magery (+5); tabTBInfo: Action | UseExpMulti, bGlobal_cephShowAll, bGreaterMUD, bQuickSpell, nMonsterDamageVsChar, nMonsterDamageVsDefault, nMonsterDamageVsParty, nMonsterPossy, nMonsterSpawnChance | frmMain: chkGlobalFilter, lblCharBless, chkWeaponOptions, cmbWeaponCombos, mnuShopsFirst, txtCharStats (+12) |
| FAM-21 | tabClasses: Abil-N, AbilVal-N, ArmourType, CombatLVL, ExpTable, MageryLVL, MageryType, MaxHits, MinHits, Name (+2); tabItems: Abil-N, AbilVal-N, Accy, ArmourClass, ArmourType, DamageResist, Encum, ItemType, Limit, Max (+7); tabMonsters: Abil-N, AbilVal-N, ArmourClass, Att%-N, AttAcc-N, AttHitSpell-N, AttTrue%-N, AttType-N, AvgLairExp, BSDefense (+13); tabRaces: Abil-N, AbilVal-N, ExpTable, HPPerLVL, Name, Number, mAGL, mCHM, mHEA, mINT (+8); tabShops: Name, Number, ShopType; tabSpells: Cap, Diff, Dur, DurInc, DurIncLVLs, Learnable, Learned From, Magery, MageryLVL, ManaCost (+5) | UseExpMulti, bGlobal_cephRecoveryOnly, bQuickSpell, nMonsterDamageVsChar, nMonsterDamageVsParty, nMonsterPossy, nMonsterSpawnChance | frmMain: chkGlobalFilter, chkWeaponOptions, cmbWeaponCombos, txtWeaponExtras, chkSpellOptions, cmbCharBless (+5) |
| FAM-22 | tabItems: Encum, Gettable, ItemType, Name, Number, Obtained From, WeaponType, Worn | bHideRecordNumbers, bOnlyInGame | frmMain: txtCharStats, lvItemManager |
| FAM-23 | tabItems: Abil-N, AbilVal-N, ArmourType, In Game, ItemType, Limit, Name, NegateSpell-N, Number, Speed (+3); tabMonsters: Abil-N, AbilVal-N, Align, ArmourClass, AvgLairExp, BSDefense, C, DamageResist, EXP, ExpMulti (+15); tabSpells: Abil-N, AttType, Classes, Learnable, Magery, MageryLVL, Number, ReqLevel, Targets | UseExpMulti, bDisableKaiAutolearn, bDontPromptCalcCharMonsterDamage, bDontPromptCalcPartyMonsterDamage, bGlobal_cephRecoveryOnly, bGreaterMUD, bMonsterDamageVsPartyCalculated, bOnlyInGame, bPromptSave, filter_Monster_bAtLeastLairs, filter_Monster_bAtkNoConfusion, filter_Monster_bAtkNoFear (+42) | frmMain: cmdNav, chkGlobalFilter, cmdFilter, cmbGlobalClass, txtGlobalLevel, chkArmourNoLimit (+41); frmMonsterFilters: optEnabled |
| FAM-24 | - | - | - |
| FAM-25 | tabItems: Abil-N, AbilVal-N, Accy, Encum, In Game, ItemType, Name, Number, Speed, StrReq (+1); tabSpells: Casted By, Classes, Learnable, Learned From, Magery, MageryLVL, Name, Number, ReqLevel, Short | bCharLoaded, bGreaterMUD, bInvenUse2ndWrist, bOnlyInGame, bPromptSave, nEquippedItem, nLearnedSpellClass, nLearnedSpells, sSessionLastCharFile | frmPasteChar: txtText, fraPasteParty, chkPastePartyAM, optPastyPartyAtkLast, txtPastePartyAC, txtPastePartyACCY (+13); frmMain: cmbGlobalClass, lvItemManager, chkGlobalFilter, cmbGlobalRace, chkEquipHold, chkInvenAddWeight (+6); frmMonsterAttackSim: txtUserAC, txtUserDR |
| FAM-26 | tabMonsters: Abil-N, AbilVal-N, Align, ArmourClass, Att%-N, AttAcc-N, AttType-N, BSDefense, Number | bGlobalAttackBackstab, bGreaterMUD, nGlobalAttackAccyAdj, nGlobalAttackBackstabWeapon, nGlobalAttackMA, nGlobalAttackTypeMME, nGlobalCharAccyAbils, nGlobalCharAccyItems, nGlobalCharAccyOther, nGlobalCharShadowAC, nGlobalCharWeaponAccy, nGlobalCharWeaponBSaccy (+2) | frmHitCalc: txtHitCalc, chkSeeHidden, chkShadow, cmbEvil, lblSubRight, optDefender (+9); frmMain: cmbGlobalClass, lblInvenCharStat, txtCharStats, txtGlobalLevel |
| FAM-27 | tabClasses: ExpTable; tabItems: Abil-N, AbilVal-N, Max, Min, Number, Speed, StrReq, WeaponType; tabRaces: ExpTable | bGreaterMUD, nGlobalCharWeaponBSmaxdmg, nGlobalCharWeaponBSmindmg, nGlobalCharWeaponMaxDmg, nGlobalCharWeaponNumber, nGlobalCharWeaponStealth | frmSwingCalc: chkBashing, chkSlowness, cmbCombat, cmbWeapon, lblEU, lblEncum (+13); frmBSCalc: chkClassStealth, cmbWeapon, lblBSMaxAdj, lblBSMinAdj, lblDMG, lblMaxAdj (+7); frmCoinConvert: optCoinBottom, optCoinTop, txtCoin |

Recordset fields come from `tabX.Fields("...")` and `tabX!field` in the family's own procedures; callee accessors (GetItemName, GetSpellName, ...) add Name/Number lookups not repeated here. FAM-12 is the only family with no globals and no recordsets: the simulator is configured through properties, and only `RandomNumber` (seeded Rnd) needs a decision for fixtures. Calls through the `clsMonAtkSim` instance are not in the index `calls`; its users are PopulateMonsterDataToAttackSim, CalculateMonsterAvgDmg, SetupMonsterAttackSimWithCharStats, CalculateMonsterDamageVsChar, PullMonsterDetail, frmMonsterAttackSim.cmdRunSim_Click, and Form_Load (creates the shared instance).

## 5. Character state

### 5a. Globals that carry character and attack state (modMain / modMMudFunc)

| Variables (with dimensions) | Type | Written by | Read in modules |
|---|---|---|---|
| bCharLoaded | Boolean | Form_Load, LoadCharacter, NMR_Import, PasteCharacter, SaveCharacter, SaveSettings, mnuFileOptItem_Click, mnuOptionsItems_Click | frmBSCalc, frmMain, frmNotepad, frmSettings, modListViewExt |
| nGlobalDatVer, nNMRVer, bLegit, bDisableKaiAutolearn | Boolean; Double | LoadInfo | frmHitCalc, frmMain, frmMap, frmMonsterFilters, frmPasteChar, frmPopUpOptions (+3) |
| sMonsterDamageVsCharDefenseConfig | String | CalculateMonsterDamageVsCharALL, ClearMonsterDamageVsCharALL, LoadMonsters | frmMain, modMain |
| sGlobalCharDefenseDescription | String | SetCharDefenseDescription | frmMain, modMain |
| nGlobalCharMagery, nGlobalCharMageryLVL | Integer | SetupClass | frmSpellBook |
| nGlobalCharHitMagicNonWeapon | Long | CalcCharacterStats | modMain |
| nGlobalCharWornArmourType, nGlobalCharAccyItems, nGlobalCharAccyAbils, nGlobalCharAccyOther, nGlobalCharPlusDodge, nGlobalCharShadowAC, nGlobalCharPlusMR, nGlobalCharQnDbonus, nGlobalCharWeaponNumber(1), nGlobalCharWeaponEncum(1), nGlobalCharWeaponAccy(1), nGlobalCharWeaponCrit(1), nGlobalCharWeaponSTR(1), nGlobalCharWeaponAGI(1), nGlobalCharWeaponMaxDmg(1), nGlobalCharWeaponBSaccy(1), nGlobalCharWeaponBSmindmg(1), nGlobalCharWeaponBSmaxdmg(1), nGlobalCharWeaponPunchSkill(1), nGlobalCharWeaponPunchAccy(1), nGlobalCharWeaponPunchDmg(1), nGlobalCharWeaponKickSkill(1), nGlobalCharWeaponKickAccy(1), nGlobalCharWeaponKickDmg(1), nGlobalCharWeaponJkSkill(1), nGlobalCharWeaponJkAccy(1), nGlobalCharWeaponJkDmg(1), nGlobalCharWeaponStealth(1) | Integer; Long | CalcCharacterStats, InvenResetStats | frmBSCalc, frmHitCalc, frmMain, frmPopUpOptions, modMMudFunc, modMain |
| nGlobalCharVileWard | Long | CalcCharacterStats, InvenResetStats, RefreshCharBless | modMain |
| nGlobalCharSavedWeaponNumber | Long | InvenResetStats, LoadCharacter, SaveCharacter | modListViewExt |
| nGlobalCharBlessSpells(9) | Long | EquipBlessSpell, LoadCharacter, RefreshCharBless, cmdCharResetBless_Click | frmMain |
| nGlobalAttackTypeMME, bGlobalAttackBackstab, nGlobalAttackBackstabWeapon, nGlobalAttackMA, nGlobalAttackSpellNum, nGlobalAttackSpellLVL, nGlobalAttackManualP, nGlobalAttackManualM, bGlobalAttackUseMeditate, nGlobalAttackHealType, nGlobalAttackHealSpellNum, nGlobalAttackHealManual | Boolean; Integer; Long; eAttackTypeMME | LoadCharacter, PopUpChooseCombatGUI, mnuOptionsItems_Click | frmHitCalc, frmMain, frmPopUpOptions, modMain |
| nGlobalAttackAccyAdj | Integer | CalcCharacterStats, mnuOptionsItems_Click | frmHitCalc |
| sGlobalAttackConfig | String | SetCurrentAttackTypeConfig, mnuOptionsItems_Click | modMMudDatabase, modMain |
| nGlobalAttackHealSpellLVL, nGlobalAttackHealRounds, nGlobalAttackHealCost | Double; Integer | LoadCharacter, PopUpChooseCombatGUI, RefreshCombatHealingValues, mnuOptionsItems_Click | frmMain, frmPopUpOptions, modMain |
| nGlobalAttackHealValue | Long | RefreshCombatHealingValues, mnuOptionsItems_Click | modMain |
| bInvenUse2ndWrist | Boolean | LoadSettings, cmdSave_Click | frmMain, frmPasteChar, modListViewExt |
| nEquippedItem(0 To 19) | Long | CalcCharacterStats, InvenEquipItem, InvenSetupEquip, cmbEquip_Click | frmBSCalc, frmMain, frmSwingCalc, modListViewExt, modMain |
| nLearnedSpells(0 To 99) | Long | ClearLearnedSpells, LearnSpell, LoadCharacter, NMR_Import, PasteSpells, UnLearnSpell, cmbGlobalClass_Click | frmMain, frmPopUpOptions, modListViewExt, modMain |
| nLearnedSpellClass | Integer | ClearLearnedSpells, LearnSpell, LoadCharacter, NMR_Import, PasteSpells, cmbGlobalClass_Click |  |

`bGreaterMUD`, `nGlobalDatVer`, `nNMRVer` are set once at database load (LoadInfo) and select the formula variant; with the stock database they are False / the stock version, so they become the `Profile` and a data-version field. `tCharacterProfile` (FAM-05) is the only structured copy of the character; every other family receives it, or reads the globals above directly (CalculateAttack reads the nGlobalChar weapon arrays, GetDamageOutput and GetLairInfo read nGlobalAttack config and write the nChar*VsMonster result arrays).

### 5b. Aggregate stat vector (lblInvenCharStat index, output of CalcCharacterStats)

0=Current Encumbrance; 1=Max Encumbrance; 2=Armour Class; 3=Damage Resistance; 4=Encumbrance Bonus %; 5=HitPoints; 6=Mana; 7=Crits; 8=Dodge; 9=SpellCast; 10=Accuracy; 11=Max Damage; 12=Hit Magic; 13=BS Accuracy; 14=BS Min Damage; 15=BS Max Damage; 16=HitPoint Regen; 17=Mana Regen; 18=Perception; 19=Stealth; 20=Prot. Evil; 21=Traps; 22=Picklocks; 23=Illuminate; 24=Magic Resist; 25=Stone Resist; 26=Water Resist; 27=Fire Resist; 28=Cold Resist; 29=Lightning Res; 30=Min Damage; 31=Quickness; 32=Prot. Good; 33=Spell Damage; 34=Punch DMG; 35=Kick DMG; 36=Jumpkick DMG; 37=Punch Skill; 38=Kick Skill; 39=Jumpkick Skill; 40=Punch Accy; 41=Kick Accy; 42=Jumpkick Accy. Index 43 and up are not documented in the banner (up to TOTAL_STAT_LBLS = 46).

### 5c. Character inputs held in controls (not globals)

| Control state | Meaning | Read by |
|---|---|---|
| txtCharStats(0..5) .Text and .Tag | Six base stats (STR INT WIL AGL HEA CHM); .Tag holds the stat with bless and gear applied | CalcCharacterStats, Refresh* (FAM-04/05), FEAT-CHAR |
| txtCharMaxStats(0..5) .Tag | Race/class stat ceilings | RefreshCPs (creation points) |
| txtGlobalLevel(0), cmbGlobalClass(0), cmbGlobalRace(0), cmbGlobalAlignment | Level, class, race, alignment (global filter doubles as the character) | everything |
| char_StatAdjustments(0..46) | Manual per-stat adjustments typed by the user | CalcCharacterStats, InvenColorCodeStats, SaveCharacter |
| nEquippedItem(0..19) | 20 equipment slots (item numbers) | CalcCharacterStats and EQ tab |
| nLearnedSpells(0..99), nLearnedSpellClass | Learned spell list | spell rows, SpellIsUsable colouring |
| cmbCharBless(0..9) | Ten bless slots (spell numbers) | CalcCharacterStats, nGlobalCharBlessSpells |
| chkCharQuests / cmbCharQuestOpts | Quest reward toggles and options (2nd, 6th, extras) | CalcCharacterStats (quest bonuses) |
| txtCharHPRegen, txtCharManaRegen, txtCharMR, chkCharAntiMagic | Regen and MR inputs, anti-magic flag | RefreshHitPoints, RefreshMagic, RefreshMagicRes |
| txtInvenStrength, txtInvenAddWeight, chkInvenAddWeight | Strength override and extra carried weight for encumbrance | InvenCalcEncum |
| filter_* globals, filter_Monster_* | Monster filter state (saved in the .mmec MonsterFilter section) | FilterMonsters, FEAT-MFILT |

### 5d. .mmec file keys (written by SaveCharacter; LoadCharacter reads the same)

| INI section | Keys | Key names |
|---|---|---|
| PlayerInfo | 43 | Name, DataFile, DataFileVer, Class, Race, Level, Alignment, GlobalMinLVL, Strength, Intellect, Widsom, Agility, Health, Charm, WeaponVSCharAC, WeaponVSCharDR, WeaponVSCharDodge, SpellVSMR, AddWeight, UseAddWeight, MR, AntiMagic, UnequipMissing, Quest<n>, Quest_2nd, Quest_6th, Quest_Extra1, Quest_Extra2, CurrentAttackType, CurrentAttackMA, CurrentAttackSpellNum, CurrentAttackSpellLVL, CurrentAttackManual, CurrentAttackManualMag, nGlobalAttackHealType, nGlobalAttackHealSpellNum, nGlobalAttackHealSpellLVL, nGlobalAttackHealManual, bGlobalAttackUseMeditate, bGlobalAttackBackstab, nGlobalAttackHealRounds, nGlobalAttackBackstabWeapon, char_StatAdjustments |
| <DataVersion> | 3 | LastSaveDir, LastSaveName, LastCharFile |
| MonsterFilter | 47 | RegenOpt, RegenVal, MonMagicOpt, MonMagicVal, Damage, DamageParty, HP, HPParty, EXP, EXPParty, MonDmgOUT, MonMagDmgOUT, MonPartyDmgOUT, MonPartyMagDmgOUT, MobRegenOpt, MobRegenVal, MobMagicOpt, MobMagicVal, MobDamage, MobHP, MobEXP, MonLairFilterTXT<n>, MonExtraFilterAC, MonExtraFilterDR, MonExtraFilterMR, MonExtraFilterBSDef, MonExtraFilterGL, MonExtraFilterLairExp, MonExtraFilterAccMaj, MonExtraFilterAccMax, MonExtraFilterNumLairs, MonExtraFilterMbLt, MonExtraFilterMbGt, MonExtraFilterFollowGt, MonExtraFilterFollowLt, MonExtraFilterAtLeastLairs, MonExtraFilterAtLeastLairsNum, MonExtraFilterDodge, MonExtraFilterCash, MonExtraFilterUndead, MonExtraFilterNonHostile_vEvil, MonExtraFilterNonHostile_vNG, MonExtraFilterNoPoison, MonExtraFilterNoConfusion, MonExtraFilterNoFear, MonExtraFilterShowAll, MonExtraFilterReachableOnly |
| Inventory | 22 | Head, Ears, Neck, Back, Torso, Arms, Wrist, Wrist2, Hands, Finger1, Finger2, Waist, Legs, Feet, Worn, Off-Hand, Weapon, Eyes, Face, Everywhere, IM_CARRIED, IM_STASH |
| Bless | 1 | Bless<n> |
| LearnedSpells | 1 | LearnedSpell<n> |
| Compare | 4 | WeaponCompare, ArmourCompare, SpellCompare, MonsterCompare |

`<DataVersion>` is the data-version caption with spaces removed (for example the stock database's version string); when SaveCharacter is called without a file name, every key is written to that section of settings.ini instead of to its own section; only LastSaveDir, LastSaveName and LastCharFile belong to the data-version section proper. Indexed keys are `Bless<n>`, `LearnedSpell<n>`. Equipment slots are keyed by slot name (Head, Ears, Neck, Back, Torso, Arms, Wrist, Wrist2, Hands, Finger1, Finger2, Waist, Legs, Feet, Worn, Off-Hand, Weapon, Eyes, Face, Everywhere). `IM_CARRIED` and `IM_STASH` hold the Item Manager rows. NMR import/export (FEAT-NMR) uses a different, text-based format. Files are CRLF INI; ReadINI/WriteINI (modSettings) are the only file I/O the format needs.

## 6. Formula logic outside the formula modules, and state that makes fixtures hard

| Where | Lines | Why it matters | Cards |
|---|---|---|---|
| frmMain.CalcCharacterStats | 1334 | The core character computation is a form routine that touches controls on over 200 lines and about 35 module globals. Port as pure function over (class, race, level, stats, slots, bless, quests, adjustments). | P3-17, P3-18 |
| frmMain Refresh* (HP, Magic, CPs, Dodge, Picklocks, Bless, MagicRes, MainStatBonuses), SetupClass, lblInvenCharStat_Change | ~700 | HP range, mana, creation-point cost and class quest defaults are computed in event code and stored in control .Tag strings. | P3-19 |
| frmMain.FilterMonsters / FilterWeapons / FilterSpells / FilterArmour | 1282 | Filter semantics and row building interleaved with control reads; they call the Add*2LV evaluators per recordset row. | P3-27..P3-30 |
| modMain.AddMonster2LV / PullMonsterDetail | 2244 | Exp/hr scripting value is computed in both ("a lot of this repeated in addmonsterlv"): the two copies must be fixtured against each other. | P3-27, P3-32..P3-34 |
| frmHitCalc.SetHitCalcVals / DoHitCalc | 399 | Hit calculator arithmetic is in the form. | P3-42 |
| frmSwingCalc.CalcSwings, frmBSCalc.CalcBS, frmCoinConvert.CalcCoin, frmExpCalc.CalcExp | 412 | Calculator arithmetic is in the forms. | P3-43 |
| frmPopUpOptions.RefreshCombatStats | 249 | Live combat preview calls CalculateAttack and CalculateSpellCast from the dialog. | P4-23 |
| frmMain.PasteCharacter / frmPasteChar.ParsePasteParty / CalculateAverageParty | 1391 | Text parsers write directly into controls. | P3-40, P3-41 |
| frmMain.InvenFindBest family | 448 | Optimiser over slots and candidates; the evaluation order decides ties. | P4-18 |

| Hard-to-fixture state | Detail | Mitigation |
|---|---|---|
| Recordset cursor position | Procedures Seek on tabItems/tabSpells/tabMonsters/tabRooms and rely on the current row (comments note that reach rows "may have moved tabRooms/tabMonsters"); GetXName helpers re-seek, others do not. | The Rust model is keyed lookups. Fixtures must not depend on cursor leftovers; a mismatch traced to a stale cursor is a known gap, not a port bug. |
| Module-level result caches | nCharDamageVsMonster(), nMonsterDamageVsChar(), nMonsterDamageVsParty, LairInfo cache (SetLairInfo/GetLairInfo), sCharDamageVsMonsterConfig as cache key. | Port without caches; fixtures call the uncached function. Cache invalidation (ClearSavedDamageVsMonster) is a UI concern. |
| nGlobalAttack* config | Which attack/spell/heal the "current character" uses is a global (and a config string) that GetDamageOutput, GetLairInfo and the lair cache read implicitly. | Make it an explicit argument (AttackConfig) in P3-11; the oracle shim sets the globals. |
| Values stored in control .Tag | Stats with bless/gear applied, regen results, CP costs and HP ranges are kept in .Tag strings and re-read by other routines. | Replace by a struct; the oracle shim reads the same .Tag after the call. |
| Randomness | clsMonsterAttackSim.RandomNumber and RunSim use Rnd; dynamic-calc mode averages instead. | Compare fixtures in average or seeded mode only. |
| Data-version gates | 43 procedures test nNMRVer (export version) and 5 test nGlobalDatVer; the NMR export version of the stock database therefore decides which branches run (for example BS Defense columns need NMR 1.83+, room-command sources need 1.84+). | Carry nmr_ver in the model; fixtures cover the stock database version only. |

## 7. Classification

| Class | Meaning | Groups | Procedures | Lines |
|---|---|---|---|---|
| port | Reproduced in touchstone (phase 3 formulas, phase 4 UI) | 55 | 1,089 | 49,253 |
| optional | Decided at the end of phase 4 (map, pathing, reachability, MegaMUD, results tree, NMR format, notepad, harness) | 7 | 231 | 11,579 |
| skip | Windows-only chrome, VB input plumbing, ListView sorting, helpers and accessors absorbed into the model/data layer, DAO connection management | 6 | 389 | 8,988 |

| ID | Name | Class | Procs | Lines | Why |
|---|---|---|---|---|---|
| FEAT-NMR | NMR character import/export | optional | 2 | 830 | Interop with another tool; not needed for parity of numbers |
| FEAT-RESULTS | Results window (textblock trees) | optional | 21 | 1,422 | Navigation aid for map/pathing tooling; also used for regen lookup and spell-cast lookup |
| FEAT-NOTEPAD | Notepad | optional | 5 | 183 | Convenience; can be a textarea on the character |
| FEAT-MAP | Graphical map / room explorer | optional | 72 | 4,952 | Design.md defers map to the optional phase |
| FEAT-MEGA | MegaMUD pathing | optional | 30 | 1,347 | External tool integration |
| FEAT-REACH | Reachability and routes | optional | 95 | 2,463 | Graph search over textblocks; large and separable |
| OPT-HARNESS | Exp/hr calibration harness | optional | 6 | 382 | RunAllSimulations, LoadSimRows and debug dump read a CSV of observed rows and compare models. Dev tool; the oracle harness (T-020..) supersedes it. |
| SKIP-WIN | Windows chrome | skip | 131 | 3,068 | Dark-mode painting and subclassing (modTheme), menu bar subclass, multi-monitor/DPI (modMonitors, modForms, DPI proxies), OS version, registry class, tool-tips, About/splash/progress forms, debug log, exit/reload, SetTopMost/clipboard/form helpers, HandleError. |
| SKIP-VBUI | VB6 input plumbing in frmMain | skip | 139 | 1,862 | GotFocus select-all, KeyPress numeric filters, KeyUp enter-to-search, MouseUp right-click popups, splitters, resize, timers (debounce/keep-alive), fonts and tooltips layout. Browser inputs, CSS and AG Grid replace them. The right-click menus they open are FEAT-CTX. |
| SKIP-LV | ListView sorting (modListViewExt) | skip | 43 | 1,614 | Custom column sort with sticky/bucket/composite keys. AG Grid sorting replaces it; the only behaviours to carry are "AC/DR column alternates" (FAM-21) and "shops first in item references" (FEAT-SHOP). |
| UTIL | String/regex/number helpers | skip | 34 | 1,010 | modSyntaxsFunc and modSQL helpers (PutCommas, RegExpFind, ExtractNumbers, ...). Replaced by Rust std and regex where a formula needs one; touchstone-vb6 owns RoundUp/Truncate semantics. |
| DB-ACC | Database accessors | skip | 38 | 1,211 | Name/field lookups by number (GetSpellName, GetItemWeight, ItemIsWeapon, ...) and enum mappers (GetArmourTypeEnum, ...). They become model field access or one-line functions inside the family that needs them; no card of their own. |
| CONN | DB connection management | skip | 4 | 223 | OpenTables/CloseDatabases/EnsureDatabaseConnection: DAO recordset lifecycle. Replaced by the World loader (T-010..T-013). |

Port features and their one-line reason are in section 3 (column "Class / phase" and the cards in section 8). No port feature is classified from the changelog alone; each is reachable from a menu item, nav button or dialog above.

## 8. Proposed cards

Size: S under 300 lines, M under 1,000, L over 1,000 (split). Lines are procedure lines of what the card ports (`~` = slice of one procedure, from its section banners). `Depends` lists cards that must be merged first; every phase 3 card is also gated by T-022 (oracle) and T-013 (world loader) for its inputs.

### 8a. Phase 3: formulas (touchstone-rules, fixture-tested against the oracle)

| Card | Title | Phase | Depends | Ports (procedures) | Lines | Size | Fixture / notes |
|---|---|---|---|---|---|---|---|
| P3-01 | Exp tables and training cost | 3 | - | FAM-01: CalcExpNeeded_STOCK, CalcExpNeeded_GMUD, GetExpModifiers_GMUD, CalcExpNeeded_GMUD_1_8_5 and 7 more | 462 | M | Pure integer/float math; GreaterMUD exp variants not ported. CalcExpNeededByRaceClass needs Races and Classes exp fields. |
| P3-02 | Stats, vitals, CP and movement | 3 | - | FAM-02: ComputeAvgLevelMaxStats, CalcRestingRate, CalcManaRegen, CalculateStealth and 29 more | 931 | M | Largest set of small pure functions; CP cost and race/class stat ceilings read Races/Classes rows. |
| P3-03 | Ability effects and lookups | 3 | - | FAM-03: RaceHasAbility, ClassHasAbility, ItemHasAbility, SpellHasAbility, AbilityEffectsCharStats, GetAbilityStatSlot | 673 | M | Item ability slots 0..19 (ability number + value pairs); data-driven, table-test it. |
| P3-04 | Spell damage equations | 3 | P3-01, P3-03 | FAM-10: PullSpellEQ, GetCurrentSpellMinMax, GetSpellMinDamage, GetSpellMaxDamage and 5 more | 961 | M | Spells rows (MinInc, MaxInc, *IncLVLs, DurInc, Cap). Output strings are part of the fixture. |
| P3-05 | Item value and shop price | 3 | - | FAM-18: GetShopMarkup, GetItemShopRegenPCT, GetItemCopperCost, GetItemValue | 349 | M | Items price/currency fields, Shops markup; GMUD ganghouse markup not ported. |
| P3-06 | Textblock and room-command parsing | 3 | - | FAM-19: GetRoomCMDTB, GetTextblockAction, GetTextblockLinkTo, GetTextblockCMDS, GetTextblockTrigger, GetTextblockCMDLine, GetTextblockCMDText (+12) | 942 | M | Text parsing over TBInfo and Rooms columns; no globals. |
| P3-07 | Ability text | 3 | P3-04, P3-06 | FAM-03: GetAbilityStats, GetAbilityList, GetAbilityName, GetAbilDiffText | 491 | M | GetAbilityName is a 319-line number-to-text table; GetAbilityStats prints values and calls the spell equations. |
| P3-08 | Accuracy and dodge | 3 | P3-02 | FAM-06: CalculateAttackDefense, CalculateAccuracy, DodgeMaxAccForPercent, CalcDodgeVSAccuracy and 8 more | 557 | M | Pure args (class, level, stats, encumbrance). Stock caps 8/99/98/95; GMUD caps not ported. |
| P3-09 | Swings, energy and weapon damage | 3 | P3-01, P3-02, P3-03, P3-04, P3-08 | FAM-07: CalculateAttack, CalcQuickAndDeadlyBonus, GetQuickAndDeadlyBonus, CalcBSDamage and 5 more | 991 | M | CalculateAttack reads tCharacterProfile plus nGlobalChar weapon arrays and Items rows: the biggest oracle shim. |
| P3-10 | Spell casting | 3 | P3-02, P3-03, P3-04 | FAM-09: CalculateSpellCast, SpellIsUsable, CalcRoundsToOOM, CalcManaRecoveryRounds and 7 more | 897 | M | Reads Spells rows and a tCharacterProfile; resist and anti-magic rules. |
| P3-11 | Character profile and attack config | 3 | P3-02, P3-03, P3-08, P3-10 | FAM-05: RefreshCombatHealingValues, bCharIsEvilAligned, GetCharActiveVileWard, SetCharDefenseDescription, SetCurrentAttackTypeConfig, ClearLearnedSpells, LearnOrUnlearnSpell (+4) | 585 | M | Converts the nGlobalChar*/nGlobalAttack* state into tCharacterProfile; defines the Rust Character -> Profile mapping. |
| P3-12 | Damage vs monster and combat rounds | 3 | P3-03, P3-09, P3-10, P3-11 | FAM-08: GetDamageOutput, CalcCombatRounds, IsMobKillable, CalcAverageNonZero and 7 more | 618 | M | GetDamageOutput reads Monsters/Spells rows and fills module-level result arrays (cache); port without the cache. |
| P3-13a | Monster attack simulator: state, max damage, resist helpers | 3 | - | FAM-12: GetMaxDamage, ResetValues, ProgressBarSetRange, CalcResistedDamage, ProgressBarIncrease and 129 more (incl. 121 property accessors) | 420 | M | Class state as a struct (about 120 one-line property accessors collapse into fields); GetMaxDamage, ResetValues, IsSpellResisted, CalcResistedDamage. |
| P3-13b | Monster attack simulator: RunSim | 3 | P3-13a | FAM-12: RunSim, AddToCombatLog | 700 | M | Round loop with seeded RNG (RandomNumber): fixtures use dynamic/average mode or a seeded sequence. |
| P3-14 | Monster sim glue | 3 | P3-13a, P3-13b, P3-04, P3-03, P3-08, P3-10, P3-11 | FAM-13: PopulateMonsterDataToAttackSim, GetMonsterAttackSummary, CalculateMonsterDamageVsCharALL, SetupMonsterAttackSimWithCharStats and 7 more | 816 | M | Reads Monsters rows (attack columns, spells, drops) and character globals; includes the damage cache. |
| P3-15 | Lair statistics | 3 | P3-12 | FAM-11: GetLairAveragesFromLocs, CountLairsByMaxRegen, RemoveOutliers, GetMedian, GetMedianAbsDev, GetStdDev, QuickSort (+4) | 532 | M | Statistics over the lair list parsed from Monsters.Locations. |
| P3-16 | Lair info and party damage | 3 | P3-15, P3-12, P3-14 | FAM-11: CalculateMonsterPartyDamage, GetLairInfoIndex, GetLairInfo, SetLairInfo, LoadLairInfo | 681 | M | LoadLairInfo builds the per-monster-group cache from Monsters and Rooms; GetLairInfo calls CalcCombatRounds. |
| P3-17 | Gear aggregation I: encumbrance, bless, equipped items | 3 | P3-02, P3-03 | FAM-04: InvenCalcEncum, PastedStatIsBuffed, ItemIsCarryActive, ItemIsFlaggedCarried, AccumItemStatBonus ; CalcCharacterStats sections: manual and bless stats, encumbrance, equipped items (source lines 1-698) | ~830 | M | Input is the 20 nEquippedItem slots, manual adjustments (char_StatAdjustments), 10 bless slots and Items rows. Output is lblInvenCharStat 0..46, which the port replaces with a struct. |
| P3-18 | Gear aggregation II: stealth, quests, accuracy, speed, MR, dodge, weapon globals | 3 | P3-17, P3-08, P3-09 | FAM-04: AdjMainStatBonus, GetDodgeSummary, ModifyCharStats, ItemIsUsableByChar ; CalcCharacterStats sections: stealth, quest bonuses, accuracy, quickness, MR, dodge, nGlobalChar weapon outputs (source lines 699-1334) | ~994 | M | The tooltip/damage-preview calls at the end of CalcCharacterStats, including SetHitCalcVals which refreshes an open Hit Calculator window, are display only and drop out of the port. |
| P3-19 | Char panel derivations | 3 | P3-17, P3-18, P3-01, P3-02, P3-04 | FAM-05: InvenResetStats, lblInvenCharStat_Change, CharStatAdjustmentPrompt, RefreshMainStatBonuses, RefreshCharBless, RefreshCPs, RefreshHitPoints (+5) | 842 | M | Formulas that live in frmMain event code (HP/mana/MR/CP cost, class quest defaults); read txtCharStats .Text and .Tag values. |
| P3-20 | Exp/hr shared helpers and Model D | 3 | - | FAM-14: ceph_ModelD, cephC_EstimateMoveSecs, cephD_OverkillFrac, cephC_Ceil and 12 more | 463 | M | Model D is a pure function of CalcExpPerHour arguments (25 args). Includes the number formatters used in model text. |
| P3-21 | Exp/hr Model C | 3 | P3-20 | FAM-15: cephC_BuildCombatProfile, cephC_BuildCycleProfile, ceph_ModelC | 909 | M | Pure function of the same arguments. |
| P3-22 | Exp/hr Model A, part 1: setup, rounds, surprise, damage | 3 | P3-20 | FAM-16: cephA_CalcHPRecoveryRounds, cephA_InCombatMPFrac ; ceph_ModelA first half (source lines 1-~400) | ~555 | M | Single 1,072-line procedure split at its section banners; port as one unit if the split proves artificial. |
| P3-23 | Exp/hr Model A, part 2: HP/mana recovery, movement, pack | 3 | P3-22 | ceph_ModelA second half (source lines ~400-1072) | ~672 | M | Same note as part 1. |
| P3-24 | Exp/hr Model B, part 1: effective RTK, surprise, damage, HP/rest | 3 | P3-20 | FAM-17: cephB_DebugLog, cephB_ApplySlackWindow, cephB_Lerp, cephB_MulBlend, cephB_BandWeight, cephB_CalcOverkill, cephB_CalcDensity ; ceph_ModelB first part (source lines 1-~540) | ~637 | M | Hand-tuned patches (MB1..MB5) are behaviour; keep them verbatim. |
| P3-25 | Exp/hr Model B, part 2: mana, loop time, pack | 3 | P3-24 | FAM-17: cephB_CalcTravelLoopSecs ; ceph_ModelB second part (source lines ~541-1014) | ~650 | M | Same note. |
| P3-26 | Exp/hr dispatcher | 3 | P3-20, P3-21, P3-22, P3-23, P3-24, P3-25, P3-12 | FAM-24: CalcExpPerHour | 233 | S | Default blend: when no model is ticked, A+B+C are enabled; LoadExpPerHourKnobs defaults all four to on when the INI keys are absent while docs say D is off by default. Pin the default in the first fixture. |
| P3-27 | Row evaluators: monsters | 3 | P3-12, P3-14, P3-16, P3-26, P3-10, P3-04, P3-02 | FAM-21: AddMonster2LV | 454 | M | Computed monster columns (damage, exp/hr, kill time) in solo, lair and party modes; the body of /api/evaluate for monsters. |
| P3-28 | Row evaluators: weapons, armour, spells, sundry, shops, classes, races | 3 | P3-09, P3-10, P3-05, P3-11, P3-04 | FAM-21: SetArmourACDRSortTags, ArmourACDRTagsAreByDR, AddArmour2LV, AddOtherItem2LV, AddWeapon2LV, AddSpell2LV, AddRace2LV (+3) | 692 | M | Weapon row calls CalculateAttack; spell row calls CalculateSpellCast. |
| P3-29 | List filters: weapons, armour, sundry, inventory, global | 3 | P3-28, P3-11, P3-03 | FAM-23: FilterAll, FilterArmour, FilterInvenItems, FilterSundry, FilterWeapons | 783 | M | Filter controls become a Filter struct; fixture the row set, not the UI. |
| P3-30 | List filters: spells and monsters | 3 | P3-29, P3-27, P3-16, P3-14 | FAM-23: FilterSpells, FilterMonsters | 737 | M | FilterMonsters (494 lines) holds lair-mode and extra-filter semantics. |
| P3-31 | Reference rows (GetLocations) | 3 | P3-06, P3-05, P3-16 | FAM-19: GetLocations, AddRoomCmdLocationRow, ExtractRoomActions | 693 | M | Builds the "where is this" rows for items, monsters, spells. |
| P3-32 | Monster detail I: attributes, attacks, abilities | 3 | P3-14, P3-07, P3-31 | PullMonsterDetail part 1 (source lines 1-~700) | ~700 | M | Emit structured rows, not VB text; the oracle fixture is the row list. |
| P3-33 | Monster detail II: drops, regen, damage vs character | 3 | P3-32, P3-12 | PullMonsterDetail part 2 (source lines ~701-1376) | ~676 | M | Same. |
| P3-34 | Monster detail III: exp/hr scripting value, reach rows | 3 | P3-33, P3-26, P3-16 | PullMonsterDetail part 3 (source lines ~1377-1790) ; FAM-20: AddMonsterDamageOutText | ~486 | M | Reach rows depend on the optional reachability module; stub them behind a trait. |
| P3-35 | Item detail I | 3 | P3-05, P3-07, P3-09, P3-31 | PullItemDetail part 1 (source lines 1-~500) | ~500 | M | Weapon damage block starts near source line 847. |
| P3-36 | Item detail II and compare text | 3 | P3-35 | PullItemDetail part 2 (source lines ~501-985) ; FAM-20: CompareArmor, CompareWeapons | ~581 | M |  |
| P3-37 | Spell, shop, class, race and NPC-command detail | 3 | P3-04, P3-10, P3-05, P3-31 | FAM-20: PullClassDetail, PullRaceDetail, AddRoomNPCCommandRefs, PullShopDetail, PullSpellDetail | 582 | M |  |
| P3-38 | Game-text inventory parser | 3 | - | FAM-22: ParseGameTextInventory, ItemNameInParseResult, CollectInventoryBlob, ParseInventoryBlob, CollectKeysBlob, ParseKeysBlob, CollectNoticeBlob (+35) | 984 | M | Pure text in, parsed groups out; no recordsets. |
| P3-39 | Parsed-item matching, best shop price, MegaMUD stat-line parser | 3 | P3-38, P3-05, P3-18 | FAM-22: ParseMegaStatLine, GetMegaStatLine, FirstMegaToken, LastMegaToken, IsMegaPlaceholder, IsMegaWholeNumberToken, CleanMegaNumber (+9) | 703 | M | Items rows by name; Obtained From text. AddOneRow calls ItemIsUsableByChar (P3-18). |
| P3-40 | Paste character | 3 | P3-38, P3-39, P3-17, P3-02 | FAM-25: PasteInventoryManager, PasteCharacter, ApplyPastedStat, PasteParty, PasteSpells, TestPasteChar, TestAlphaChar | 941 | M | Writes straight into Char-tab controls; in Rust it returns a Character. |
| P3-41 | Paste party | 3 | P3-38 | FAM-25: ParsePasteParty, CalculateAverageParty | 862 | M | Party text to per-member stats and averages used by the party monster-damage calc. |
| P3-42 | Hit calculator core | 3 | P3-08, P3-11, P3-02 | FAM-26: DoHitCalc, SetHitCalcVals, GetVileWardValue, ModifyHitCalcValues, GetMonsterData | 509 | M | Includes vile ward and shadow handling. |
| P3-43 | Calculator cores: swings, backstab, coin, exp | 3 | P3-09, P3-01, P3-02 | FAM-27: CalcBS, CalcCoin, ConvertCoin, CalcExp, CalcSwings | 427 | M | Four small procedures that live inside their forms. |

### 8b. Phase 4: web UI (after T-013; each also needs the phase 3 cards it displays)

| Card | Title | Phase | Depends (cards) | Families it displays | Ports (procedures) | Lines | Size | Notes |
|---|---|---|---|---|---|---|---|---|
| P4-01 | Server API: tables, info, evaluate, calc endpoints | 4 | P3-27, P3-28, P3-29, P3-30 | - | none (new code) | 0 | M* | No MME procedures; wires T-010..T-013 data to touchstone-rules (/api/evaluate, /api/calc/*). Size M by new code. |
| P4-02a | Web shell: tabs, F-keys, nav history, hybrid view | 4 | P4-01 | - | FEAT-SHELL: cmdNav_Click, NavHistoryBack, NavHistory_RecordNavigation, NavHistory_Push, NavHistory_IsValidIndex, Form_Unload, LoadInfo (+2) | 596 | M | 12 nav buttons F1..F12, EQ+Char hybrid view, back/forward history (mouse back button). |
| P4-02b | Startup sequence: data load, settings, control population | 4 | P4-02a | - | FEAT-SHELL: Form_Load, SetUpFormObjects | 937 | M | 937 lines of mostly control layout; what survives is the order: settings, database open, lists loaded, character auto-load. |
| P4-03 | Menus, data-file selection, recent files | 4 | P4-02a | - | FEAT-SHELL: mnuExit_Click, mnuFileOptItem_Click, mnuHelp_Click, mnuOpenDataFile_Click, OpenNewDataFile, mnuOptionsItems_Click, mnuRecentDB_Click (+7) | 999 | M | Recent files become server state; Open Data File becomes a data-source picker. |
| P4-04 | Grid wrapper, row colours, search, copy | 4 | P4-02a | FAM-03, FAM-04 | FEAT-LISTS: RefreshLearnedSpellColors, RefreshLearnedSpellColors_byLV, RefreshMonsterColors, RefreshMonsterColors_byLV, RefreshListviewItemColors, RefreshListviewItemColors_ItemManager, RefreshListviewItemColors_Weapon (+8) | 834 | M | Shared AG Grid wrapper (theme tokens, column filters, copy, find-as-you-type). |
| P4-05 | Weapons explorer | 4 | P3-28, P3-29, P3-35, P3-36, P4-04 | FAM-20 | FEAT-WEAP: chkWeaponOptions_Click, lvWeapons_ColumnClick, cmbWeaponCombos_Click, lvWeaponLoc_ColumnClick and 5 more | 109 | S | Columns come from the P3-28 weapon evaluator; options: speed, extras, ability filter, combos. |
| P4-06 | Armour explorer | 4 | P3-28, P3-29, P3-35, P3-36, P4-04 | FAM-20, FAM-21 | FEAT-ARM: cmbArmorAbilityList_Click, lvArmour_ColumnClick, lvArmour_ItemClick, lvArmourLoc_ColumnClick, lvArmourLoc_DblClick, txtArmourFind_Change | 68 | S | AC/DR column alternate sort. |
| P4-07 | Spells explorer | 4 | P3-04, P3-10, P3-28, P3-29, P3-30, P3-37, P4-04 | FAM-05, FAM-09, FAM-10, FAM-19, FAM-20, FAM-21, FAM-23 | FEAT-SPELL: FindNegates, LoadSpells, GotoSpell, LookUpSpellCast and 9 more | 410 | M | Includes counters finder (FindNegates) and what-casts lookup. |
| P4-08 | Monsters explorer and detail pane | 4 | P3-16, P3-26, P3-27, P3-30, P3-31, P3-32, P3-33, P3-34, P4-04, P4-09a, P4-09b | FAM-11, FAM-13, FAM-19, FAM-20, FAM-21, FAM-23 | FEAT-MON: LoadMonsters, cmdMonHelp_Click, lvMonsterDetail_DblClick, LookUpMonsterRegen and 4 more | 448 | M | Modes: all, solo lair, party lair; detail pane from P3-32..34; locations list. |
| P4-09a | Monster filters: inline controls and filter state | 4 | P3-30, P4-04 | FAM-11, FAM-23, FAM-25 | FEAT-MFILT: RefreshExtraMonsterFilterButton, chkMonMagic_Click, cmbMonsterRegen_Click, cmdMonsterFilterOps_Click, RemoveFilters, cmdFilterAll_Click, MonsterFilterFormAction (+12) | 681 | M | Regen, magic, damage, HP, exp for mob / solo lair / party lair; lair text; save and restore in the character file. |
| P4-09b | Monster filters: extra filters dialog | 4 | P4-09a | FAM-03, FAM-23 | FEAT-MFILT: chkAtkNoConfusion_Click, chkAtkNoFear_Click, chkAtkNoPoison_Click, chkAtLeastLairs_Click, chkIsNonHostile_vNG_Click, chkIsUndead_Click, chkNonHostile_vEvil_Click (+58) | 610 | M | AC, DR, MR, BS def, game limit, accuracy, dodge, lairs, mobs/lair, follow %, abilities, undead, non-hostile, drops, attack-type exclusions, reachable only. |
| P4-10 | Shops explorer | 4 | P3-05, P3-28, P3-37, P4-04 | FAM-20, FAM-21 | FEAT-SHOP: LoadShops, cmdShopButtons_Click, lvShops_ItemClick, lvShopDetail_ColumnClick and 9 more | 226 | S | Charm field changes prices live. |
| P4-11 | Sundry/items explorer | 4 | P3-28, P3-29, P3-31, P3-35, P3-36, P4-04 | FAM-05, FAM-19, FAM-20, FAM-21, FAM-23 | FEAT-SUND: cmdSundryChests_Click, GotoItem, LoadItems, lvOtherItems_ItemClick and 4 more | 389 | M | Chest contents button. |
| P4-12 | Class/Race explorer and stat builder | 4 | P3-01, P3-02, P3-28, P3-37, P4-04 | FAM-01, FAM-02, FAM-07, FAM-20, FAM-21 | FEAT-CLS: RefreshClassRaceBuilder, cmdClassRace_Click, LoadClasses, LoadRaces and 4 more | 529 | M |  |
| P4-13 | Rooms explorer (text) and room finder | 4 | P3-06, P3-31, P4-04 | FAM-19, FAM-23 | FEAT-ROOMS: GotoLocation, FindRoomWithDirections, cmdRoomsButtons_Click, cmdMapFindText_Click and 5 more | 782 | M | Room finder by exits is the frmPopUpOptions room-find half. |
| P4-14 | Lists/Compare panes | 4 | P3-27, P3-28, P3-35, P3-36, P3-37, P4-05, P4-06, P4-07, P4-08 | FAM-05, FAM-19, FAM-20, FAM-21, FAM-25 | FEAT-COMPARE: cmdCompareNav_Click, RemovePopupItemCompare, RemovePopUpSpellCompare, CompareAddItem and 22 more | 972 | M | Depends on the four explorers it compares. |
| P4-15 | Context menus: aux, clipboard, character copy | 4 | P3-14, P3-32, P3-33, P3-34, P3-37, P4-14 | FAM-01, FAM-05, FAM-10, FAM-13, FAM-21, FAM-22 | FEAT-CTX: CopyChartoClip, CopyMonsterToClipboard, CopyShopToClipboard, mnuAuxPopUpItem_Click, PopUpAuxMenu | 849 | M |  |
| P4-16 | Context menus: items, spells, map, goto | 4 | P3-04, P3-09, P3-10, P3-42, P3-43, P4-14 | FAM-05, FAM-10, FAM-20, FAM-21, FAM-22 | FEAT-CTX: mnuEquipGotoItem_Click, mnuItemsPopUpItem_Click, mnuMapPopUpItem_Click, mnuSpellsPopUpItem_Click, PopUpItemsMenu, PopUpMapMenu, PopUpSpellsMenu (+1) | 593 | M |  |
| P4-17 | EQ tab: equipment slots and stat totals | 4 | P3-17, P3-18, P4-04 | FAM-02, FAM-05, FAM-23, FAM-25 | FEAT-EQ: cmdArmNextSlot_Click, cmdEquipButtons_Click, chkInvenAddWeight_Click, chkInvenHideCharStats_Click, cmbEquip_Click, cmdEquipGoto_Click, InvenAddEquip (+9) | 715 | M | Slot rules (two-handed, paired rings) live in InvenEquipItem. |
| P4-18 | Find Best | 4 | P3-05, P3-17, P3-18, P4-17 | FAM-02 | FEAT-EQ: InvenFindBest, InvenFindBestAbils, InvenFindBestItemValue, InvenFindBestEval, InvenFindBestIsExcluded, InvenFindBestExclude, InvenFindBestSelectItem (+8) | 469 | M | An optimiser over equipment slots; parity needs the same candidate order and tie-breaks. |
| P4-19 | Item Manager | 4 | P3-38, P3-39, P4-17 | FAM-20 | FEAT-EQ: LoadInvenItems, lvItemManager_ColumnClick, lvItemManager_DblClick, lvItemManager_ItemClick, lvItemManagerLoc_ColumnClick, lvItemManagerLoc_DblClick ; FEAT-LISTS: LV_DeleteSelected, LV_InvertSelection, LV_SetActionCell, LV_CopySelectedActionsToClipboard, ParseActionAndQty, LV_SetCellText, ParseActionBase | 443 | M | Carried/stash quantities and action cells. |
| P4-20 | Char tab | 4 | P3-11, P3-19, P4-02a | FAM-01, FAM-04, FAM-05, FAM-08, FAM-09, FAM-10, FAM-11, FAM-13, FAM-25 | FEAT-CHAR: FilterBlessSpells, cmdCharButtons_Click, cmdCharResetBless_Click, EquipBlessSpell and 12 more | 585 | M | Stat boxes, regen, MR, quests, ten bless slots, HP/mana/dodge/CP panels. |
| P4-21 | Global filter | 4 | P3-29, P3-30, P4-20 | FAM-05, FAM-23, FAM-26 | FEAT-GFILT: cmdFilter_Click, chkGlobalFilter_Click, cmbGlobalRace_Click, txtGlobalLevel_Change and 5 more | 439 | M | Class, race, level, alignment, use-character. |
| P4-22 | Combat setup dialog: attack choice | 4 | P3-09, P3-10, P3-11, P4-20 | FAM-02, FAM-03, FAM-05, FAM-07, FAM-09, FAM-10 | FEAT-COMBAT: chkBackstab_Click, chkBashing_Click, chkMeditate_Click, chkSmashing_Click, cmbAttackMA_Click, cmbAttackSpell_Click, cmbAttackSpell_KeyPress (+15) | 550 | M | Sets the nGlobalAttack* state (weapon, spell level, martial art, manual P/M, backstab weapon). |
| P4-23 | Combat setup dialog: healing and preview | 4 | P3-04, P3-10, P3-12, P4-22 | FAM-02, FAM-03, FAM-05, FAM-07, FAM-09, FAM-10 | FEAT-COMBAT: PopUpChooseCombatGUI, RefreshMonsterCombatGUI, cmbHealingSpell_Click, cmbHealingSpell_KeyPress, cmdCancel_Click, cmdContinue_Click, cmdHelp_Click (+17) | 780 | M | Heal spell, rounds, manual heal, live combat-stat preview. |
| P4-24 | Load character, load options, .mmec reader | 4 | P3-11, P3-17, P3-18, P4-09a, P4-09b, P4-14, P4-20 | FAM-05, FAM-08, FAM-09, FAM-13, FAM-21, FAM-22, FAM-23 | FEAT-MMEC: LoadCharacter, ReadINI ; FEAT-LOADCHAR: chkCompareLoad_Click, chkInvenLoad_Click, cmdCancel_Click, cmdContinue_Click, Form_Load | 795 | M | LoadCharacter restores everything: stats, bless, learned spells, inventory, compare lists, monster filters. Sections and keys in section 6. |
| P4-25 | Save character, .mmec writer | 4 | P4-24 | - | FEAT-MMEC: SaveCharacter, WriteINI | 401 | M | Round-trip fixtures: load then save must reproduce the file. |
| P4-26 | Paste Character dialog | 4 | P3-40, P3-41, P4-20 | FAM-25 | FEAT-PASTE: cmdContinue_Click, cmdPaste_Click, cmdPasteQ_Click, cmdPasteMegaDmg_Click and 48 more | 415 | M |  |
| P4-27a | Calculators I: hit and backstab | 4 | P3-42, P3-43, P4-02a | FAM-02, FAM-26, FAM-27 | FEAT-HIT: LoadMonsters, Form_Load, cmbMonsterList_Click, GotoMonster and 19 more ; FEAT-BS: cmdAlterLevel_MouseDown, AlterLevel, Form_Load, cmdCopytoClip_Click and 34 more | 742 | M | Two windows become panels or routes; monster picker, shadow, see hidden, vile ward; stealth, level, weapon. |
| P4-27b | Calculators II: swing | 4 | P3-39, P3-43, P4-02a | FAM-02, FAM-07, FAM-22, FAM-27 | FEAT-SWING: cmdAlterLevel_MouseDown, Form_Load, cmdCopytoClip_Click, AlterLevel and 39 more | 538 | M | Includes Paste MegaMUD stat line (parser in P3-39). |
| P4-28a | Calculators III: exp, coin, spell book | 4 | P3-10, P3-43, P4-02a | FAM-01, FAM-05, FAM-09, FAM-21, FAM-25, FAM-27 | FEAT-EXPCALC: Form_Load, cmdCalcExp_Click, txtCalcEXPTable_KeyUp, Form_Unload and 11 more ; FEAT-COIN: Form_Load, RefreshCharmButton, cmdCharm_Click, txtCoin_Change and 9 more ; FEAT-SPELLBOOK: cmdListSpells_Click, Form_Load, lvSpellBook_ColumnClick, cmbClass_Click and 10 more | 762 | M | Spell book uses SpellIsUsable and the global character filter. |
| P4-28b | Monster Attack Simulator window | 4 | P3-13b, P3-14, P4-02a | FAM-05, FAM-06, FAM-12, FAM-13 | FEAT-MSIM: cmdRunSim_Click, cmdResetUserDefs_Click, Form_Load, LoadMonsters and 30 more | 419 | M | User-defined AC/DR/dodge/MR/anti-magic/vile ward and rounds; combat log. |
| P4-29 | Settings dialog and exp/hr knobs | 4 | P3-26, P4-02a | - | FEAT-SETTINGS: chkAutoLoadChar_Click, chkExpHrCalcByCharacter_Click, cmdAll_Click, cmdCEPHA_Q_Click, cmdCEPHB_Q_Click, cmdModelQ_Click, cmdNone_Click (+17) ; FEAT-SETTINGS: LoadExpPerHourKnobs | 503 | M | Model checkboxes A-D, four knobs, per-character or global knobs, sim rounds, route waypoint interval. |
| P4-30 | Persisted settings (settings.ini semantics) | 4 | P4-29 | FAM-23 | FEAT-SETTINGS: LoadSettings, SaveSettings, CleanSettings, ResolveSettingsPath, CreateSettings, CheckINIReadOnly | 601 | M | Replaced by touchstone.toml for server options and browser storage for view options; the card enumerates which options survive. |

Size counts only the UI procedures: the heavy computation for an explorer is in P3-27..P3-37. "Families it displays" lists the families the procedures in the card call directly; use it with section 4 to size what the card must expose through the API.

### 8c. Optional (decide at the end of phase 4)

| Card | Title | Phase | Procs | Lines | Size | Groups |
|---|---|---|---|---|---|---|
| O-01 | Graphical map and room explorer | 6 | 72 | 4,952 | L (split at start) | FEAT-MAP |
| O-02 | MegaMUD pathing | 6 | 30 | 1,347 | L (split at start) | FEAT-MEGA |
| O-03 | Reachability and routes | 6 | 95 | 2,463 | L (split at start) | FEAT-REACH |
| O-04 | Results (textblock tree) window | 6 | 21 | 1,422 | L (split at start) | FEAT-RESULTS |
| O-05 | NMR character import/export | 6 | 2 | 830 | M | FEAT-NMR |
| O-06 | Notepad | 6 | 5 | 183 | S | FEAT-NOTEPAD |
| O-07 | Exp/hr calibration harness | 6 | 6 | 382 | M | OPT-HARNESS |

Totals: phase 3 cards port 29,701 procedure lines across 44 cards; phase 4 cards 19,189 lines across 34 cards. Sizes: phase 3 M x43, S x1.

## Appendix A. Procedure assignment

Every procedure appears under exactly one group, listed as `module: names`. Duplicate names in one module (Property Get/Let/Set, same-name handlers) are listed once with a multiplier. Group sizes (procedures / lines) are in sections 3, 4 and 7.

**FAM-01 Exp tables and training cost** (11 procs, 462 lines)

- modMMudDatabase: GetMaxLevel, CalcExpNeededByRaceClass
- modMMudFunc: CanI64Mul, IDiv, CalcExpNeeded, CalcExpNeeded_GMUD_1_8_5, CalcExpNeeded_GMUD, GetExpModifiers_GMUD, CalcExpNeeded_STOCK, GetExpModifiers_STOCK, CalcMoneyRequiredToTrain

**FAM-02 Stats, vitals, CP, movement** (33 procs, 931 lines)

- modMMudDatabase: GetRaceHPBonus, GetClassMaxHP, GetClassMinHP, GetClassMageryLVL, GetClassMagery, GetClassArmourType, GetClassCombat, ComputeAvgLevelMaxStats, RaceStat_FieldDelta, RaceStat_SafeFieldLong, CP_CostForRaise, LevelNeededForCP, GetRaceCP_Safe, GetRaceCP, GetRaceStealth, GetClassStealth
- modMMudFunc: CalcEncum, GetMageryEnum, CalcMovementSpeed, CalcRestingRate, CalcManaRegen, CalcMR, CalcMaxHP, CalcMaxMana, CalcSpellCasting, GetEncumPercents, CalcPicklocks, CalcCPLevel, CalcEncumbrancePercent, AdjustSpeedForSlowness, CalculateStealth, RoundUpToNearest5
- modMain: Get_Enc_Ratio

**FAM-03 Item and spell ability decoding** (10 procs, 1,164 lines)

- modMMudDatabase: RaceHasAbility, ClassHasAbility, ItemHasAbility, SpellHasAbility
- modMMudFunc: GetAbilityStats, GetAbilityList, GetAbilityName, AbilityEffectsCharStats
- modMain: GetAbilDiffText, GetAbilityStatSlot

**FAM-04 Gear aggregation (CalcCharacterStats)** (10 procs, 1,824 lines)

- frmMain: InvenCalcEncum, AdjMainStatBonus, CalcCharacterStats, GetDodgeSummary, ModifyCharStats, PastedStatIsBuffed, ItemIsCarryActive, ItemIsFlaggedCarried, AccumItemStatBonus, ItemIsUsableByChar

**FAM-05 Character profile and panel derivations** (23 procs, 1,427 lines)

- frmMain: InvenResetStats, lblInvenCharStat_Change, CharStatAdjustmentPrompt, RefreshMainStatBonuses, RefreshCharBless, RefreshCPs, RefreshHitPoints, RefreshDodge, RefreshMagic, RefreshPicklocks, SetupClass, RefreshMagicRes
- modMain: RefreshCombatHealingValues, bCharIsEvilAligned, GetCharActiveVileWard, SetCharDefenseDescription, SetCurrentAttackTypeConfig, ClearLearnedSpells, LearnOrUnlearnSpell, LearnSpell, UnLearnSpell, PopulateCharacterProfile, GetCurrentAttackName

**FAM-06 Accuracy, hit chance and dodge** (12 procs, 557 lines)

- modMMudFunc: CalculateAttackDefense, CalculateBackstabAccuracy, CalculateAccuracy, CalcDodgeVSAccuracy, CalcDodge, GMUD_DiminishingReturns, GetDodgeCap, GetSpellHitCap, GetHitMin, GMUD_IsLightArmourClass, GetHitCap, DodgeMaxAccForPercent

**FAM-07 Swings, energy and weapon damage** (9 procs, 991 lines)

- modMMudFunc: CalculateAttack, CalcBSDamage, CalcTrueAverage, GetQuickAndDeadlyBonus, CalcQuickAndDeadlyBonus, CalcEnergyUsed, CalcEnergyUsedWithEncum, AdjustEnergyUsedWithSpeed, AdjustEnergyUsedWithEncum

**FAM-08 Damage vs monster and combat rounds** (11 procs, 618 lines)

- modMMudFunc: CalcCombatRounds, ccr_Saturate, ccr_SmoothStep, ccr_Lerp, ccr_SafeDiv, ccr_Min, ccr_Max
- modMain: CalcAverageNonZero, GetDamageOutput, IsMobKillable, ClearSavedDamageVsMonster

**FAM-09 Spell casting** (11 procs, 897 lines)

- modMMudFunc: GMUD_GetSpDmgMultiplierFromSC, CalculateSpellCast, ResistPct_SignedOfBase, NegResistPct_ShareOfTotal, SpellIsInGame, SpellIsUsable, SpellAttackTypeEnum, GetSpellCastChance, CalculateResistDamage
- modMain: CalcRoundsToOOM, CalcManaRecoveryRounds

**FAM-10 Spell damage equations** (9 procs, 961 lines)

- modMMudDatabase: GetSpellAttackType, GetCurrentSpellMinMax, PullSpellEQ, GetSpellMinDamage, GetSpellMaxDamage, GetSpellDuration, SpellDoesDamage, SpellIsAreaAttack, SpellIsBlessSpell

**FAM-11 Lairs** (16 procs, 1,213 lines)

- frmMain: CalculateMonsterPartyDamage
- modMMudDatabase: GetLairAveragesFromLocs, CountLairsByMaxRegen, RemoveOutliers, GetMedian, GetMedianAbsDev, GetStdDev, QuickSort, GetLairInfoIndex, GetLairInfo, SetLairInfo, LoadLairInfo, DICT_ModeFromCounts, DICT_AddToCount, DICT_BumpCount, CalculateAverageLairs

**FAM-12 Monster attack simulator (class)** (136 procs, 1,483 lines)

- clsMonsterAttackSim: HIT_MIN x2, HIT_CAP x2, SPELL_HIT_CAP x2, DODGE_SOFTCAP x2, DODGE_CAP x2, bDodgeBeforeAC x2, bHideEnergyInfo x2, bCombatLogMaxRoundOnly x2, nMaxEnergyPerRound, nAverageDamage, nAverageDamagePhys, nAverageDamageSpell, bGreaterMUD x2, nDynamicCalcDifference x2, bDynamicCalc x2, nAtkDuration x2, nBetweenRoundDuration x2, nBetweenRoundSpellType x2, nBetweenRoundResistDmgMR x2, nBetweenRoundResistType x2, sBetweenRoundName x2, nBetweenRoundMin x2, nBetweenRoundMax x2, nBetweenRoundChance x2, nMaxRoundDamage, nCombatLogMaxRounds x2, sCombatLog, nTotalDamage, nTotalAttacks, bUseCPU x2, RandomNumber, AddToCombatLog, GetMaxDamage, RunSim, ResetActiveAtkSpell, ResetActiveBetweenSpell, IsSpellResisted, CalcResistedDamage, cProgressBar x2, ProgressBarSetRange, ProgressBarIncrease, ResetValues, ResetActiveSpells, privHandleError, nNumberOfRounds x2, nEnergyPerRound x2, nUserAntiMagic x2, nUserMR x2, nUserProtEvil x2, bMobIsEvil x2, nUserVileWard x2, nUserDodge x2, nUserRFIR x2, nUserRCOL x2, nUserRLIT x2, nUserRWAT x2, nUserRSTO x2, nUserDR x2, nUserAC x2, nStatBetweenRound_AtkTotalDamage, nStatBetweenRound_AtkDmgResisted, nStatHitSpell_AtkTotalDamage, nStatHitSpell_AtkDmgResisted, nStatAtkTotalDamage, nStatAtkAttemptDodgedOrResisted, nStatAtkDmgResisted, nStatAtkHits, nStatAtkAttempted, nAtkMRdmgResist x2, nAtkResist x2, nAtkHitSpellMax x2, nAtkHitSpellType x2, nAtkHitSpellMin x2, nAtkSuccess x2, nAtkChance x2, nAtkMax x2, nAtkMin x2, nAtkEnergy x2, nAtkSpellType x2, nAtkType x2, sAtkName x2, sAtkHitSpellName x2, Class_Initialize, Apply_GMUD_DiminishingReturns

**FAM-13 Monster sim glue** (11 procs, 816 lines)

- modMMudDatabase: TestMonExpMulti, GetMonsterAttackSummary, GetMonsterAvgDmgFromDB, CalculateMonsterItemBonuses, PopulateMonsterDataToAttackSim, CalculateMonsterAvgDmg
- modMain: ClearMonsterDamageVsCharALL, CalculateMonsterDamageVsCharALL, CalculateMonsterDamageVsChar, GetPreCalculatedMonsterDamage, SetupMonsterAttackSimWithCharStats

**FAM-14 Exp/hr shared helpers and Model D** (16 procs, 463 lines)

- modExpPerHour: cephC_EstimateMoveSecs, cephC_DebugPrint, cephC_Ceil, cephD_OverkillFrac, ceph_ModelD, cephB_Saturate, cephB_SmoothStep, MinDbl, MaxDbl, ClampDbl, SafeDiv
- modMain: pct, F6, F3, F2, F1

**FAM-15 Exp/hr Model C** (3 procs, 909 lines)

- modExpPerHour: cephC_BuildCombatProfile, cephC_BuildCycleProfile, ceph_ModelC

**FAM-16 Exp/hr Model A** (3 procs, 1,227 lines)

- modExpPerHour: ceph_ModelA, cephA_CalcHPRecoveryRounds, cephA_InCombatMPFrac

**FAM-17 Exp/hr Model B** (9 procs, 1,287 lines)

- modExpPerHour: ceph_ModelB, cephB_DebugLog, cephB_ApplySlackWindow, cephB_Lerp, cephB_MulBlend, cephB_BandWeight, cephB_CalcOverkill, cephB_CalcDensity, cephB_CalcTravelLoopSecs

**FAM-18 Item value and shop price** (4 procs, 349 lines)

- modMMudDatabase: GetShopMarkup, GetItemShopRegenPCT, GetItemCopperCost, GetItemValue

**FAM-19 Textblock, room-command and reference parsing** (22 procs, 1,635 lines)

- modMMudDatabase: GetRoomCMDTB, GetTextblockAction, GetTextblockLinkTo, GetTextblockCMDS, GetTextblockTrigger, GetTextblockCMDLine, GetTextblockCMDText, GetChestItems, TextBlockHasTeleport, GetTextblockTeleport
- modMMudFunc: ExtractTextCommand, ExtractMapRoom, SplitRoomCmdSources, ParseRoomCmdEntry, ParseRoomCmdToken, IsRoomCmdNumber, IsRoomCmdAction, ExpandRoomCmdRooms, ObtainedFromIsGettable
- modMain: GetLocations, AddRoomCmdLocationRow, ExtractRoomActions

**FAM-20 Detail pane builders** (10 procs, 3,525 lines)

- modMain: PullItemDetail, CompareArmor, CompareWeapons, PullClassDetail, PullRaceDetail, AddRoomNPCCommandRefs, PullMonsterDetail, AddMonsterDamageOutText, PullShopDetail, PullSpellDetail

**FAM-21 Row evaluators (list columns)** (11 procs, 1,146 lines)

- modMain: SetArmourACDRSortTags, ArmourACDRTagsAreByDR, AddArmour2LV, AddOtherItem2LV, AddWeapon2LV, AddSpell2LV, AddRace2LV, AddMonster2LV, AddShop2LV, AddClass2LV, RaceColorCode

**FAM-22 Game-text parsers** (58 procs, 1,687 lines)

- frmSwingCalc: ParseMegaStatLine, GetMegaStatLine, FirstMegaToken, LastMegaToken, IsMegaPlaceholder, IsMegaWholeNumberToken, CleanMegaNumber
- modItemParse: ParseGameTextInventory, ItemNameInParseResult, CollectInventoryBlob, ParseInventoryBlob, CollectKeysBlob, ParseKeysBlob, CollectNoticeBlob, ParseNoticeBlob, ConsolidateGroundByRoom, AddNoticeToRoom, GetOrCreateRoomIndex, AccumulateMax, AccumulateSum, IsMovementCommand, IsSectionBoundary, ShouldStopSection, FindInlineBoundaryPos, StartsWithCI, ConsolidateList, ParseCountAndName, SingularizeSimple, FindKeyIndex, DedupeEquipped, IndexOf, PopulateItemManagerFromParsed, ArrayCount, AddSectionItems, SafeSubText, SumInvOrCarriedQtyForName, GetItemsByExactNameArr, AddListViewRowsForItem, EvaluateBestPriceForHit, AddOneRow, HasAnyContent, ParseNameAndQty, NormalizeEquippedName, SplitToArray, SqueezeSpaces, SafeArrayHasData, AddString, IsArrayInitialized, StripBracketedChunks, IsCashItem, IsEquippedItem, IsBracketLine, ExtractShopsFromObtainedFrom, ExtractFirstNumber, NzStr, NzLong, GetBestShopNumForItem, LV_AddRowByItemNumber

**FAM-23 List filters** (7 procs, 1,520 lines)

- frmMain: FilterAll, FilterArmour, FilterInvenItems, FilterSundry, FilterSpells, FilterMonsters, FilterWeapons

**FAM-24 Exp/hr dispatcher** (1 procs, 233 lines)

- modExpPerHour: CalcExpPerHour

**FAM-25 Paste character and party** (9 procs, 1,803 lines)

- frmMain: PasteInventoryManager, PasteCharacter, ApplyPastedStat, PasteParty, PasteSpells
- frmPasteChar: ParsePasteParty, CalculateAverageParty
- modMMudFunc: TestPasteChar, TestAlphaChar

**FAM-26 Hit calculator core** (5 procs, 509 lines)

- frmHitCalc: DoHitCalc, SetHitCalcVals, GetVileWardValue, ModifyHitCalcValues, GetMonsterData

**FAM-27 Calculator cores: swings, backstab, coin, exp** (5 procs, 427 lines)

- frmBSCalc: CalcBS
- frmCoinConvert: CalcCoin, ConvertCoin
- frmExpCalc: CalcExp
- frmSwingCalc: CalcSwings

**FEAT-SHELL Application shell** (25 procs, 2,532 lines)

- frmMain: Form_Load, cmdNav_Click, NavHistoryBack, NavHistory_RecordNavigation, NavHistory_Push, NavHistory_IsValidIndex, Form_Unload, LoadInfo, mnuExit_Click, mnuFileOptItem_Click, mnuHelp_Click, mnuOpenDataFile_Click, OpenNewDataFile, mnuOptionsItems_Click, mnuRecentDB_Click, mnuToolsItems_Click, RecentFileAdd, RecentFilesLoad, RecentFilesUpdate, UpdateRecentDBs, RefreshAll, SetUpFormObjects
- modMain: Main, ParseCommandLineFiles, GetAppDataDir

**FEAT-WEAP Weapons explorer (F5)** (9 procs, 109 lines)

- frmMain: chkWeaponOptions_Click, cmbWeaponAbilityList_Click, cmbWeaponCombos_Click, lvWeaponLoc_ColumnClick, lvWeaponLoc_DblClick, lvWeapons_ColumnClick, lvWeapons_ItemClick, txtWeaponExtras_Change, txtWeaponFind_Change

**FEAT-ARM Armour explorer (F6)** (6 procs, 68 lines)

- frmMain: cmbArmorAbilityList_Click, lvArmour_ColumnClick, lvArmour_ItemClick, lvArmourLoc_ColumnClick, lvArmourLoc_DblClick, txtArmourFind_Change

**FEAT-SPELL Spells explorer (F7)** (13 procs, 410 lines)

- frmMain: chkSpellOptions_Click, cmbSpellMagery_Click, cmdSpellNote_Click, GotoSpell, LoadSpells, LookUpSpellCast, lvSpellLoc_ColumnClick, lvSpellLoc_DblClick, lvSpells_ColumnClick, lvSpells_ItemClick, txtSpellFind_Change, txtSpellOptions_Change, FindNegates

**FEAT-MON Monsters explorer (F10)** (8 procs, 448 lines)

- frmMain: cmdMonHelp_Click, GotoMonster, LoadMonsters, LookUpMonsterRegen, lvMonsterDetail_DblClick, lvMonsters_ColumnClick, lvMonsters_ItemClick, txtMonsterFind_Change

**FEAT-MFILT Monster list filters (inline and extra)** (84 procs, 1,291 lines)

- frmMain: RefreshExtraMonsterFilterButton, chkMonMagic_Click, cmbMonsterRegen_Click, cmdMonsterFilterOps_Click, RemoveFilters, cmdFilterAll_Click, MonsterFilterFormAction, ReadINIFilterNum, ReadINIFilterTxt, ReadINIFilterEXP, ResetFilterOptions, optMonsterFilter_Click, txtMonMagic_Change, txtMonsterDamage_Change, txtMonsterDamageOUT_Change, txtMonsterEXP_Change, txtMonsterHP_Change, txtMonsterLairFilter_Change, txtMonsterRegen_Change
- frmMonsterFilters: chkAtkNoConfusion_Click, chkAtkNoFear_Click, chkAtkNoPoison_Click, chkAtLeastLairs_Click, chkIsNonHostile_vNG_Click, chkIsUndead_Click, chkNonHostile_vEvil_Click, chkReachableOnly_Click, cmbAbilities_Click, cmbAbilities_KeyPress, cmdAbilClear_Click, cmdExec_Click, cmdQ_Click, Form_Load, lblLabelArray_Click, lblLairEXP_Click, optCash_Click, optEnabled_Click, timWindowMove_Timer, txtAbilityVal_Change, txtAbilityVal_GotFocus, txtAbilityVal_KeyPress, CheckForceEnable, txtAC_Change, txtAC_GotFocus, txtAC_KeyPress, txtAtkAccuracyMaj_Change, txtAtkAccuracyMaj_GotFocus, txtAtkAccuracyMax_Change, txtAtkAccuracyMax_GotFocus, txtAtLeastLairs_GotFocus, txtAtLeastLairs_KeyPress, txtBSDef_Change, txtBSDef_GotFocus, txtBSDef_KeyPress, txtDodge_Change, txtDodge_GotFocus, txtDodge_KeyPress, txtDR_Change, txtDR_GotFocus, txtDR_KeyPress, txtFollow_Change, txtFollow_GotFocus, txtFollow_KeyPress, txtFollowLTE_Change, txtFollowLTE_GotFocus, txtFollowLTE_KeyPress, txtGameLimit_Change, txtGameLimit_GotFocus, txtGameLimit_KeyPress, txtLairEXP_Change, txtLairEXP_GotFocus, txtLairEXP_KeyPress, txtMR_Change, txtMR_GotFocus, txtMR_KeyPress, txtNumLairs_Change, txtNumLairs_GotFocus, txtNumLairs_KeyPress, txtNumMobsGTE_Change, txtNumMobsGTE_GotFocus, txtNumMobsGTE_KeyPress, txtNumMobsLTE_Change, txtNumMobsLTE_GotFocus, txtNumMobsLTE_KeyPress

**FEAT-SHOP Shops explorer (F11)** (13 procs, 226 lines)

- frmMain: cmdShopButtons_Click, chkShopShowCharm_Click, LoadShops, lvShopDetail_ColumnClick, lvShopDetail_DblClick, lvShopDetail_ItemClick, lvShopLoc_DblClick, lvShops_ColumnClick, lvShops_ItemClick, mnuShopsFirst_Click, RefreshShops, txtShopCharm_Change, txtShopFind_Change

**FEAT-SUND Sundry/Items explorer (F9)** (8 procs, 389 lines)

- frmMain: cmdSundryChests_Click, GotoItem, LoadItems, lvOtherItemLoc_ColumnClick, lvOtherItemLoc_DblClick, lvOtherItems_ColumnClick, lvOtherItems_ItemClick, txtOtherItemsFind_Change

**FEAT-CLS Class/Race explorer (F8)** (8 procs, 529 lines)

- frmMain: cmdClassRace_Click, LoadClasses, LoadRaces, lvClasses_ColumnClick, RefreshClassRaceBuilder, lvClasses_ItemClick, lvRaces_ColumnClick, lvRaces_ItemClick

**FEAT-ROOMS Rooms (text) explorer (F12)** (9 procs, 782 lines)

- frmMain: cmdRoomsButtons_Click, cmdMapFindText_Click, FindRoomWithDirections, GotoLocation, ShowRoomCmdRoomList
- frmPopUpOptions: ResetRoomFind, cmdRoomFindDir_Click, optRoomFindMatch_Click, txtRoomName_GotFocus

**FEAT-COMPARE Lists/Compare panes (F4)** (26 procs, 972 lines)

- frmMain: ClearAllSavedListsNoPrompt, cmdCompareClear_Click, cmdCompareNav_Click, CompareAddItem, CompareAddMonster, CompareAddShopItems, ReloadMonsterCompare, lvArmourCompare_ColumnClick, lvArmourCompare_ItemClick, lvArmourCompareLoc_ColumnClick, lvArmourCompareLoc_DblClick, lvMonsterCompare_ColumnClick, lvMonsterCompare_ItemClick, lvMonsterCompareLoc_DblClick, lvSpellCompare_ColumnClick, lvSpellCompare_ItemClick, lvSpellCompareLoc_ColumnClick, lvSpellCompareLoc_DblClick, lvWeaponCompare_ColumnClick, lvWeaponCompare_ItemClick, lvWeaponCompareLoc_ColumnClick, lvWeaponCompareLoc_DblClick, RemovePopupItemCompare, mnuJumpToCompare_Click, RemovePopUpMonsterCompare, RemovePopUpSpellCompare

**FEAT-EQ EQ tab and Item Manager (F3)** (37 procs, 1,265 lines)

- frmMain: cmdArmNextSlot_Click, cmdEquipButtons_Click, chkInvenAddWeight_Click, chkInvenHideCharStats_Click, cmbEquip_Click, cmdEquipGoto_Click, InvenAddEquip, InvenClear, InvenColorCodeStats, InvenCopytoClipboard, InvenEquipItem, InvenFindBest, InvenFindBestAbils, InvenFindBestItemValue, InvenFindBestEval, InvenFindBestIsExcluded, InvenFindBestExclude, InvenFindBestSelectItem, InvenFindBestDupeOK, InvenSetupEquip, lblInvenCharStat_Click, lblInvenStats_Click, LoadInvenItems, lvItemManager_ColumnClick, lvItemManager_DblClick, lvItemManager_ItemClick, lvItemManagerLoc_ColumnClick, lvItemManagerLoc_DblClick, mnuFindBestArmour_Click, mnuFindBestAttrib_Click, mnuFindBestAttack_Click, mnuFindBestItem_Click, mnuFindBestMystics_Click, mnuFindBestResist_Click, mnuFindBestStats_Click, txtInvenAddWeight_Change, txtInvenStrength_Change

**FEAT-CHAR Char tab (F1)** (16 procs, 585 lines)

- frmMain: chkCharAntiMagic_Click, cmbCharQuestOpts_Click, chkCharQuests_Click, cmbCharBless_Click, cmdCharBlessJump_Click, cmdCharChangeStats_Click, cmdCharButtons_Click, FilterBlessSpells, EquipBlessSpell, cmdCharResetBless_Click, ResetCharBlessDropdowns, txtCharHPRegen_Change, txtCharManaRegen_Change, txtCharMR_Change, txtCharName_Change, txtCharStats_Change

**FEAT-GFILT Global filter** (9 procs, 439 lines)

- frmMain: cmbGlobalAlignment_Click, cmbGlobalAlignment_Change, chkGlobalFilter_Click, cmbGlobalClass_Click, cmbGlobalRace_Click, cmdFilter_Click, cmdFind_Click, txtGlobalLevel_Change, txtGlobalMinLVL_Change

**FEAT-COMBAT Combat setup dialog** (46 procs, 1,330 lines)

- frmMain: PopUpChooseCombatGUI, RefreshMonsterCombatGUI
- frmPopUpOptions: chkBackstab_Click, chkBashing_Click, chkMeditate_Click, chkSmashing_Click, cmbAttackMA_Click, cmbAttackSpell_Click, cmbAttackSpell_KeyPress, cmbBackstabWeapon_Click, cmbBackstabWeapon_KeyPress, cmbHealingSpell_Click, cmbHealingSpell_KeyPress, cmdCancel_Click, cmdContinue_Click, cmdHelp_Click, cmdHealRoundsMod_Click, cmdPaste_Click, SetupChooseAttack, Form_Load, RefreshSpells, RefreshItems, Form_Resize, optAttackType_Click, optHealingType_Click, RefreshCombatStats, timRefreshCombatStats_Timer, timWindowMove_Timer, txtAttackManual_Change, txtAttackManual_GotFocus, txtAttackManual_KeyPress, txtAttackManualMagic_Change, txtAttackManualMagic_GotFocus, txtAttackManualMagic_KeyPress, txtAttackSpellLevel_Change, txtAttackSpellLevel_GotFocus, txtAttackSpellLevel_KeyPress, txtHealingCastNumRounds_Change, txtHealingCastNumRounds_GotFocus, txtHealingCastNumRounds_KeyPress, txtHealingManual_Change, txtHealingManual_GotFocus, txtHealingManual_KeyPress, txtHealingSpellLVL_Change, txtHealingSpellLVL_GotFocus, txtHealingSpellLVL_KeyPress

**FEAT-CTX Context menus and clipboard actions** (13 procs, 1,442 lines)

- frmMain: CopyChartoClip, CopyMonsterToClipboard, CopyShopToClipboard, mnuAuxPopUpItem_Click, mnuEquipGotoItem_Click, mnuItemsPopUpItem_Click, mnuMapPopUpItem_Click, mnuSpellsPopUpItem_Click, PopUpAuxMenu, PopUpItemsMenu, PopUpMapMenu, PopUpSpellsMenu, ProcessListViewClick

**FEAT-LISTS List helpers: colours, selection, search, copy** (22 procs, 1,196 lines)

- modListViewExt: LV_DeleteSelected, LV_InvertSelection, LV_SetActionCell, LV_CopySelectedActionsToClipboard, ParseActionAndQty, LV_SetCellText, ParseActionBase, RefreshLearnedSpellColors, RefreshLearnedSpellColors_byLV, RefreshMonsterColors, RefreshMonsterColors_byLV, RefreshListviewItemColors, RefreshListviewItemColors_ItemManager, RefreshListviewItemColors_Weapon, RefreshListviewItemColors_Armour
- modMain: SearchLV, CopyWholeLVtoClipboard, CopyLVLinetoClipboard, GetEquipCaption, StringOfNumbersToArray, ColorListviewRow, InstrCount

**FEAT-MMEC Character files (.mmec)** (4 procs, 1,128 lines)

- frmMain: LoadCharacter, SaveCharacter
- modSettings: ReadINI, WriteINI

**FEAT-LOADCHAR Load-character options dialog** (5 procs, 68 lines)

- frmLoadChar: chkCompareLoad_Click, chkInvenLoad_Click, cmdCancel_Click, cmdContinue_Click, Form_Load

**FEAT-NMR NMR character import/export** (2 procs, 830 lines)

- frmMain: NMR_Export, NMR_Import

**FEAT-PASTE Paste Character dialog** (52 procs, 415 lines)

- frmPasteChar: chkPastePartyAM_Click, cmdCancel_Click, cmdContinue_Click, cmdPaste_Click, cmdPasteMegaDmg_Click, cmdPasteQ_Click, Form_Load, Form_Resize, optPastyPartyAtkLast_Click, timWindowMove_Timer, txtPastePartyAC_Change, txtPastePartyAC_GotFocus, txtPastePartyAC_KeyPress, txtPastePartyACCY_Change, txtPastePartyACCY_GotFocus, txtPastePartyACCY_KeyPress, txtPastePartyAMTotal_GotFocus, txtPastePartyAMTotal_KeyPress, txtPastePartyDMG_Change, txtPastePartyDMG_GotFocus, txtPastePartyDMG_KeyPress, txtPastePartyDodge_Change, txtPastePartyDodge_GotFocus, txtPastePartyDodge_KeyPress, txtPastePartyDR_Change, txtPastePartyDR_GotFocus, txtPastePartyDR_KeyPress, txtPastePartyHeals_Change, txtPastePartyHeals_GotFocus, txtPastePartyHeals_KeyPress, txtPastePartyHitpoints_Change, txtPastePartyHitpoints_GotFocus, txtPastePartyHitpoints_KeyPress, txtPastePartyMR_Change, txtPastePartyMR_GotFocus, txtPastePartyMR_KeyPress, txtPastePartyName_GotFocus, txtPastePartyPartyTotal_GotFocus, txtPastePartyRegenHP_Change, txtPastePartyRegenHP_GotFocus, txtPastePartyRegenHP_KeyPress, txtPastePartyRestHP_Change, txtPastePartyRestHP_GotFocus, txtPastePartyRestHP_KeyPress, txtPastePartySpellDMG_Change, txtPastePartySpellDMG_GotFocus, txtPastePartySpellDMG_KeyPress, txtPastePartySwings_Change, txtPastePartySwings_GotFocus, txtPastePartySwings_KeyPress, txtText_KeyDown, txtText_KeyPress

**FEAT-HIT Hit Calculator** (23 procs, 241 lines)

- frmHitCalc: chkSeeHidden_Click, chkShadow_Click, cmbEvil_Click, cmbMonsterList_Click, cmbMonsterList_KeyPress, cmdCharHitCalc_Click, cmdCharHitCalc_MouseDown, cmdCharHitCalc_MouseUp, cmdMobGoto_Click, cmdQ_Click, cmdRefreshMonster_Click, Form_Load, LoadMonsters, GotoMonster, Form_Unload, optAttacker_Click, optDefender_Click, optHitCalcType_Click, timWindowMove_Timer, timButtonPress_Timer, txtHitCalc_Change, txtHitCalc_GotFocus, txtHitCalc_KeyPress

**FEAT-SWING Swing Calculator** (43 procs, 538 lines)

- frmSwingCalc: chkBashing_Click, chkSlowness_Click, cmbCombat_Click, cmbWeapon_Click, cmbWeapon_KeyPress, cmdAlterLevel_Click, AlterLevel, cmdAlterLevel_MouseDown, cmdAlterLevel_MouseUp, cmdClose_Click, cmdCopytoClip_Click, cmdJump_Click, cmdPasteMega_Click, Form_Load, GotoWeapon, LoadWeapons, Form_Resize, Form_Unload, optSpeed_Click, timButtonPress_Timer, timCalc_Timer, timWindowMove_Timer, txtAgility_GotFocus, txtAgility_KeyPress, txtAgility_KeyUp, txtEncum_GotFocus, txtEncum_KeyPress, txtEncum_KeyUp, txtLevel_GotFocus, txtLevel_KeyPress, txtLevel_KeyUp, txtMaxEncum_GotFocus, txtMaxEncum_KeyPress, txtMaxEncum_KeyUp, txtSpeed_GotFocus, txtSpeed_KeyPress, txtSpeed_KeyUp, txtStrength_Change, txtStrength_GotFocus, txtStrength_KeyPress, txtStrength_KeyUp, txtTrueAVG_Change, txtTrueAVG_GotFocus

**FEAT-BS Backstab Calculator** (38 procs, 501 lines)

- frmBSCalc: chkClassStealth_Click, cmbWeapon_Click, cmbWeapon_KeyPress, cmdNote_Click, cmdReset_Click, Form_Load, WriteStealth, cmdAlterLevel_Click, AlterLevel, cmdAlterLevel_MouseDown, cmdAlterLevel_MouseUp, cmdClose_Click, cmdCopytoClip_Click, cmdJump_Click, GotoWeapon, LoadWeapons, Form_Resize, Form_Unload, timButtonPress_Timer, timCalc_Timer, timWindowMove_Timer, txtBSMaxDMG_Change, txtBSMinDMG_Change, txtLevel_Change, txtMaxDMG_Change, txtStealth_Change, txtStealth_GotFocus, txtStealth_KeyPress, txtBSMinDMG_GotFocus, txtBSMinDMG_KeyPress, txtLevel_GotFocus, txtLevel_KeyPress, txtBSMaxDMG_GotFocus, txtBSMaxDMG_KeyPress, txtMaxDMG_GotFocus, txtMaxDMG_KeyPress, txtStrength_Change, txtStrength_GotFocus

**FEAT-EXPCALC Exp Calculator** (15 procs, 186 lines)

- frmExpCalc: Form_Load, cmbClass_Click, cmbRace_Click, cmdCalcExp_Click, Form_Resize, Form_Unload, lvCalcExp_MouseUp, timWindowMove_Timer, txtCalcEXPTable_GotFocus, txtCalcEXPTable_KeyPress, txtCalcEXPTable_KeyUp, txtEndLVL_GotFocus, txtEndLVL_KeyPress, txtStartLVL_GotFocus, txtStartLVL_KeyPress

**FEAT-COIN Coin Converter** (13 procs, 140 lines)

- frmCoinConvert: RefreshCharmButton, cmdCharm_Click, Form_Load, optCoinBottom_Click, optCoinTop_Click, Timer1_Timer, timWindowMove_Timer, txtCoin_Change, txtCoin_GotFocus, txtCoin_KeyPress, txtCoin_KeyUp, CalcDelay, txtCoin_MouseUp

**FEAT-SPELLBOOK Spell Book** (14 procs, 436 lines)

- frmSpellBook: cmdListSpells_Click, Form_Load, CopyGlobalChar, cmbClass_Click, cmdPasteChar_Click, Form_Resize, Form_Unload, lvSpellBook_ColumnClick, lvSpellBook_DblClick, lvSpellBook_ItemClick, lvSpellBook_MouseUp, timResize_Timer, timWindowMove_Timer, txtLevel_GotFocus

**FEAT-MSIM Monster Attack Simulator window** (34 procs, 419 lines)

- frmMain: cmdMonsterAttackSim_Click
- frmMonsterAttackSim: chkAlwaysDodge_Click, chkUserAntiMagic_Click, cmbMonsterList_Click, cmbMonsterList_KeyPress, cmdResetUserDefs_Click, GetPartySize, cmdRunSim_Click, cmdAlwaysDodgeQ_Click, Form_Load, ResetFields, LoadMonsters, Form_Resize, Form_Unload, timWindowMove_Timer, txtElementalResist_GotFocus, txtElementalResist_KeyPress, txtProtEvil_GotFocus, txtProtEvil_KeyPress, txtVileWard_GotFocus, txtVileWard_KeyPress, txtUserAC_GotFocus, txtUserAC_KeyPress, txtUserDodge_GotFocus, txtUserDodge_KeyPress, txtUserMR_GotFocus, txtUserMR_KeyPress, txtUserDR_GotFocus, txtUserDR_KeyPress, cmdMRNote_Click, chkDynamicRounds_Click, txtNumRounds_GotFocus, txtNumRounds_KeyPress, GotoMonster

**FEAT-SETTINGS Settings and exp/hr knobs** (31 procs, 1,104 lines)

- frmMain: LoadExpPerHourKnobs, LoadSettings, SaveSettings
- frmSettings: chkAutoLoadChar_Click, chkExpHrCalcByCharacter_Click, cmdAll_Click, cmdCEPHA_Q_Click, cmdCEPHB_Q_Click, cmdModelQ_Click, cmdNone_Click, Form_Load, PopulateExpFields, cmdCancel_Click, cmdRecreateINI_Click, cmdSave_Click, txtCEPH_DMG_GotFocus, txtCEPH_DMG_KeyPress, txtCEPH_Mana_GotFocus, txtCEPH_Mana_KeyPress, txtCEPH_Move_GotFocus, txtCEPH_Move_KeyPress, txtCEPH_XP_GotFocus, txtCEPH_XP_KeyPress, txtMonsterSimRounds_GotFocus, txtMonsterSimRounds_KeyPress, txtRouteWaypointInterval_GotFocus, txtRouteWaypointInterval_KeyPress
- modSettings: CleanSettings, ResolveSettingsPath, CreateSettings, CheckINIReadOnly

**FEAT-RESULTS Results window (textblock trees)** (21 procs, 1,422 lines)

- frmResults: Form_Load, chkHideTextblocks_Click, cmdCollapse_Click, cmdFind_Click, cmdQ_Click, SetupResultsWindow, Form_Resize, Form_Unload, lvResults_ItemClick, lvResults_KeyUp, GotoSelectedResult, CreateExecutionTree, AddExecutionNode, CreateCommandTree, AddCommandNode, mnuExpandItem_Click, ExpandBranch, timWait_Timer, timWindowMove_Timer, tvwResults_MouseUp, tvwResults_NodeClick

**FEAT-NOTEPAD Notepad** (5 procs, 183 lines)

- frmNotepad: cmdDO_Click, Form_Load, Form_Resize, Form_Unload, timWindowMove_Timer

**FEAT-MAP Graphical map / room explorer** (72 procs, 4,952 lines)

- frmEditPreset: cmdCancel_Click, cmdGetStats_Click, cmdOK_Click, Form_Load, Form_Unload, txtCaption_GotFocus, txtMap_GotFocus, txtRoom_GotFocus
- frmMain: chkMapOptions_Click, optAlsoMark_Click, cmdMapLeadsHere_Click, cmdDrawMap_Click, cmdEditPreset_Click, EditPreset, cmdMapPreset_Click, cmdMapPresetSelect_Click, cmdResetPresets_Click, lblRoomCell_MouseDown, LoadPresets, lvMapLoc_DblClick, MapActivateCell, MapDrawOnRoom, MapGetRoomLoc, MapGoDirection, MapMapExits, MapStartMapping
- frmMap: cmbMapSize_Click, Form_Activate, Form_Load, chkMapOptions_Click, optAlsoMark_Click, cmdMove_Click, Form_KeyPress, MapGoDirection, fraMapControls_OLEDragDrop, lvMapLoc_MouseUp, mnuMapPopUpItem_Click, cmdMapPresetSelect_Click, cmdQ_Click, cmdNavigate_Click, cmdMapFindText_Click, cmdMapShowUnused_Click, cmdViewMapLegend_Click, cmdDrawMap_Click, ResizeMap, MapStartMapping, MapMapExits, MapActivateCell, MapDrawOnRoom, MapGetRoomLoc, cmdOptions_Click, cmdPresets_Click, Form_Unload, lblRoomCell_MouseDown, PopUpMapMenu, lvMapLoc_DblClick, timWindowMove_Timer, txtRoomMap_GotFocus, txtRoomRoom_GotFocus, LoadPresets, cmdResetPresets_Click, cmdMapPreset_Click, cmdEditPreset_Click
- modMain: Get_MegaMUD_ExitsCode, Get_MegaMUD_RoomHash, MegaRooms_EnsurePopulated, MegaRooms_ScanFolderRecursive, MegaRooms_ParseFile, in_array_long_md, in_array_long_3d, getval_array_long_3d, getindex_array_long_3d

**FEAT-MEGA MegaMUD pathing** (30 procs, 1,347 lines)

- frmMain: MapMegaScanRoomsDatabase, MapMegaScanPath, cmdMapMegaRoomFind_Click
- frmMegaMUDPath: cmdGotoLast_Click, cmdGotoStart_Click, cmdMapCommand_Click, cmdMapAddMegaCodes_Click, ScanRoomsDatabase, PromptNewMegaRoomCode, NextFreeMegaRoomCode, AddSavedNewRoomCodes, GetSavedNewRoom, cmdMapSwitch_Click, cmdMapCopyToClip_Click, cmdMapMoveClear_Click, cmdMove_Click, cmdResetCurrent_Click, cmdResetStart_Click, cmdUndoNote_Click, cmdUndoStep_Click, Form_Load, ResetStartingRoom, Form_Resize, Form_Unload, lvHistory_DblClick, timWindowMove_Timer, txtMapMove_KeyPress, AddHistory, SetCurrentPosition, txtPickLock_GotFocus

**FEAT-REACH Reachability and routes** (95 procs, 2,463 lines)

- frmMain: GetMonsterDetailMap, ShowMonsterRoute, ShowRoomRoute, RouteCaptionText, ShowRouteResults, MonsterRouteRoomText, AddMonsterRouteResultRow
- modMain: AddRouteRows, AddRouteAltRow, RouteCoinText, RouteKindText, FormatRouteLabel, RouteLabelNames, RouteLabelClassRace, AddMonsterReachRows
- modReachability: ReachInvalidate, ReachRecompute, ReachIsComputing, ReachIsComputed, ReachIsMonsterOK, ReachMonsterInfo, ReachRoute, ReachRoomStatus, ReachRoomRoute, ReachEnsureComputed, ReachWriteDebugCSV, SetProgress, FieldExists, FldStr, FldLng, LoadTextblocks, SpellOnlyCasters, LoadSpells, GrowAbils, LoadRooms, ResolvePOIs, SetPOI, Nz0, RoomIndex, AddItemRoom, LoadItemRooms, LoadMonsters, TrimWS, IsDigits, IsInt, SplitWords, NumbersIn, NextMapRoom, CsvField, OpKind, SplitLine, IsCommandLine, TBIndex, SpellIndex, AddCapped, AddOut, FromOps, FromTB, FromSpell, ChainOutputs, AddEdge, AddChainEdges, ExitToll, ExitLevel, ExitRestriction, ParseExit, BuildEdges, ParseRefs, Resolve, SummonRefs, SummonAfterTeleport, ResolveMonsterRooms, ComputeReachability, VisitAtLevel, VisitFromSeeds, RouteSearch, RouteSources, RouteBuild, RouteDest, RoutePathInfo, RouteSteps, POIName, FindRoomIdx, HeapPush, HeapPop, BuildGroupAdjacency, FindGateEdge, RouteCoinWeight, EdgeHasLevel, StepFlags, CmdText, StepLabel, BuildReverseAdjacency, SCCIds, BandsText

**OPT-HARNESS Exp/hr calibration harness** (6 procs, 382 lines)

- modExpPerHour: DebugPrintExpHrGlobals, RunAllSimulations, ParseNum, SplitFields, LoadSimRows, FormatResult

**SKIP-WIN Windows chrome** (131 procs, 3,068 lines)

- clsToolTip: Font x2, TxtColor x2, BkColor x2, VisibleTime x2, DelayTime x2, TipWidth x2, Style x2, InitComctl32, SetToolTipObj, SetToolTipItem, DelToolTip, AjustItemRect, Class_Initialize, Class_Terminate
- clsRegistryRoutines: hkey x2, KeyRoot x2, Subkey x2, KeyExists, CreateKey, DeleteKey, DeleteValue, EnumRegistryKeys, EnumRegistryValues, GetRegistryValue, SetRegistryValue, ReadRemoteRegistryValue, WriteRemoteRegistryValue, TrimNull
- frmAbout: cmdClose_Click, Form_Load, Form_Unload, lblMMESource_Click, lblMudinfo_Click, lblSynEmail_Click
- frmHelpChangeLog: Form_Load, Form_Resize, timWindowMove_Timer
- frmLoad: Form_Load
- frmMapLegend: Form_Load, Form_Unload
- frmProgressBar: cmdCancel_Click, Form_Load, SetRange, IncreaseProgress, Form_Resize, Form_Unload, timWindowMove_Timer
- modForms: UnSubclassSomeWindow, UN_SubclassFormSizeRestriction, SubclassSomeWindow, SubclassFormMinMaxSize, SubClassListView, UnSubClassListView, ListViewSubclassProc, MinMaxSize_Proc, CalculateWindowFrame, ResizeForm, ConvertFixedSizeForm, ConvertScale, AdjustWindowRectExForDpi_Proxy, GetDpiForWindow_Proxy
- modMain: InitDebugLog, DebugLogPrint, DebugCloseLog, IsDllAvailable, ExpandCombo, AutoSizeDropDownWidth, SelectAll, AppReload, AppTerminate, ExitApp, RegCreateKeyPath, CountListviewSelections, ControlExists
- modMenuSubClass: HiWordUnsigned, GetAppCommand, MenuWindowProc, DarkMenuBarBackground, DarkMenuBarItem, DarkMenuBarBottomLine
- modMonitors: IsOurAppForeground, EnsureAppForeground, ScanSystemDPI, EnumMonitorsForDPI_Proc, LoadPosition, GetCurrentMonitor, CheckIfFormOffScreen, CheckPosition, MonitorFormTimer
- modSyntaxsFunc: GetOwner, SetOwner, SetTopMostWindow, HandleError, UnloadForms, FormIsLoaded, SetClipboardText
- modTheme: ApplyDarkTitleBar, DKT_CanSubclass, MuteSunkenBorder, PackFrameCaptionInfo, ApplyDarkTheme, SetFrameCaption, SetFrameForeColor, SetFrameFontBold, DarkComboBorderProc, UnsubclassDarkComboBorder, UnsubclassDarkFrameBorder, DarkFrameBorderProc, TColor, TBtnColor, InvertLightness, Hue2RGB
- modWin32Ver: GetWin32Ver, isNT, is9x, isWinXP, isWin2k, isWinNT4, isWinME, isWin98, isWin95

**SKIP-VBUI VB6 input plumbing in frmMain** (139 procs, 1,862 lines)

- frmMain: cmbArmorAbilityList_KeyPress, cmbSundryAbilityList_KeyPress, cmbWeaponAbilityList_KeyPress, cmbCharBless_KeyPress, cmbCharBless_KeyUp, cmbEquip_KeyPress, cmdCharChangeStats_MouseDown, cmdCharChangeStats_MouseUp, cmdInvenModStrength_MouseDown, cmdInvenModStrength_MouseUp, NavHistory_HiWordUnsigned, NavHistory_GetAppCommand, NavHistory_IsMouseBackMessage, ToggleControls, cmdShopAlterCharm_MouseDown, cmdShopAlterCharm_MouseUp, cmdDebug_Click, LoadPictureResource, Form_KeyDown, Form_Resize, Form_Resize_Event, TruncateStatTips, SortInvenToolTips, lvArmour_MouseUp, lvArmourCompare_MouseUp, lvArmourCompareLoc_MouseUp, lvArmourLoc_MouseUp, lvClasses_MouseUp, lvItemManager_MouseUp, lvItemManagerLoc_MouseUp, lvMapLoc_MouseUp, lvMonsterCompare_MouseUp, lvMonsterCompareLoc_MouseUp, lvMonsterDetail_MouseUp, lvMonsters_MouseDown, lvMonsters_MouseUp, lvOtherItemLoc_MouseUp, lvOtherItems_MouseUp, lvRaces_MouseUp, lvShopDetail_MouseUp, lvShopLoc_MouseUp, lvShops_MouseUp, lvSpellCompare_MouseUp, lvSpellCompareLoc_MouseUp, lvSpellLoc_MouseUp, lvSpells_MouseUp, lvWeaponCompare_MouseUp, lvWeaponCompareLoc_MouseUp, lvWeaponLoc_MouseUp, lvWeapons_MouseUp, SetLabelFonts, SetStatFonts, SetupSplitters, SetupSplitterSizes, splSplitterNS_MouseUp, splSplitterWE_MouseUp, splSplitterWE_Resize, SyncSplitters, timDatabaseKeepAlive_Timer, timRefreshDelay_Timer, timButtonPress_Timer, timSelectAll_Timer, timWindowMove_Timer, txtArmorAbilityVal_GotFocus, txtArmourFind_GotFocus, txtArmourFind_KeyPress, txtArmourFind_KeyUp, txtCharHPRegen_GotFocus, txtCharHPRegen_KeyPress, txtCharManaRegen_GotFocus, txtCharManaRegen_KeyPress, txtCharMR_GotFocus, txtCharMR_KeyPress, txtCharName_GotFocus, txtCharStats_GotFocus, txtCharStats_KeyPress, txtClassRaceDetail_GotFocus, txtGlobalLevel_GotFocus, txtGlobalLevel_KeyPress, txtGlobalMinLVL_GotFocus, txtInvenAddWeight_GotFocus, txtInvenAddWeight_KeyPress, txtInvenStrength_GotFocus, txtInvenStrength_KeyPress, txtMapMove_KeyPress, txtMonMagic_GotFocus, txtMonMagic_KeyPress, txtMonsterDamage_GotFocus, txtMonsterDamage_KeyPress, txtMonsterDamageOUT_GotFocus, txtMonsterDamageOUT_KeyPress, txtMonsterEXP_GotFocus, txtMonsterEXP_KeyPress, txtMonsterFind_GotFocus, txtMonsterFind_KeyPress, txtMonsterFind_KeyUp, txtMonsterHP_GotFocus, txtMonsterHP_KeyPress, txtMonsterLairFilter_GotFocus, txtMonsterLairFilter_KeyPress, txtMonsterLairFilter_KeyUp, txtMonsterRegen_GotFocus, txtMonsterRegen_KeyPress, txtMonsterRegen_KeyUp, txtOtherItemDetail_GotFocus, txtOtherItemsFind_GotFocus, txtOtherItemsFind_KeyPress, txtOtherItemsFind_KeyUp, txtRoomMap_GotFocus, txtRoomMap_KeyPress, txtRoomRoom_GotFocus, txtRoomRoom_KeyPress, txtShopCharm_GotFocus, txtShopCharm_KeyPress, txtShopDetail_GotFocus, txtShopFind_GotFocus, txtShopFind_KeyPress, txtShopFind_KeyUp, txtSpellFind_GotFocus, txtSpellFind_KeyPress, txtSpellFind_KeyUp, txtSpellOptions_GotFocus, txtSpellOptions_KeyPress, txtSundryAbilityVal_GotFocus, txtWeaponAbilityVal_GotFocus, txtWeaponCompareDetail_GotFocus, txtWeaponDetail_GotFocus, txtWeaponExtras_GotFocus, txtWeaponExtras_KeyPress, txtWeaponFind_GotFocus, txtWeaponFind_KeyPress, txtWeaponFind_KeyUp, txtWeaponSpeed_GotFocus, txtWeaponSpeed_KeyPress, SubclassListViews, UnSubclassListViews, ListViewOnKillFocus, ListViewOnSetFocus, ListViewSubclassProc

**SKIP-LV ListView sorting (modListViewExt)** (43 procs, 1,614 lines)

- modListViewExt: LV_GetFlagColIndex, LV_MapSortBucket, LV_SortBucketRank, LV_GetPriFromLI, GetFlagBase, LV_GetCell, LV_FlagBase, DBG_DumpAfterSort, DBG_DumpBeforeSort, SortListView, SortListViewByTag, InvNumber, LV_ReadSortState, LV_WriteSortState, LV_GetNextAscending, LV_Sort_ColumnClick, LV_RefreshSort, LV_RefreshSort_ShopsFirst, LV_DoSort_ShopsFirst, LV_UnstashText, LV_RowIsShop, LV_Sort_ColumnClickOrSticky, LV_RefreshSort_RespectingSticky, LV_EnsureHiddenRowSeqColumn, LV_GetRowSeq, LV_AssignRowSeqIfMissing, LV_EnableSticky, LV_IsStickyEnabled, LV_RemoveToken, LV_SortByCompositeTagAscending, LV_Sort_WithStickyByAction, LV_Sort_WithStickyByTag, LV_EnsureHiddenSortColumn, LV_SortByCompositeKey_HiddenColumn, EnsureSubItemExists, StripNumFmt, BuildSecondaryKey, SanitizeKey, PadNumberKey, LV_Sort_WithStickyByTagCol, GetCellTagFromColumn, InvertStringForDesc, GetListViewColMinMax

**UTIL String/regex/number helpers** (34 procs, 1,010 lines)

- modSQL: SQL_NumArray
- modSyntaxsFunc: RemoveDuplicateNumbersFromString, in_long_arr, in_str_arr, ClearListViewSelections, ExtractNumbersFromString, ExtractValueFromString, GetFirstWord, FileExists, FolderExists, NumberKeysOnly, PutCommas, FormatWithCommas, AutoPrepend, AutoAppend, RemoveCharacter, RemoveVowles, RoundUp, RoundUpTo5, PutCrLF, AutoComplete, IsAlphaNumeric, IsAlphaBetical, RegExpFind, FindStringIndex, RegExpFindv2, EscapeRegex, SortLettersWithSeparator, SortLetters, FormatBigIntWithCommas, ToPlainIntegerString, KeepDigits, InsertThousands, Truncate

**DB-ACC Database accessors** (38 procs, 1,211 lines)

- modMMudDatabase: GetShopName, GetShopRoomNames, GetShopLocation, GetSpellName, SpellSeek, GetSpellManaCost, GetSpellShort, GetSpellByShort, GetClassName, GetRaceName, GetMultiMonsterNames, GetMonsterMapNumber, GetMonsterName, MonsterIsEvil, GetRoomName, GetRoomRegen, GetItemLimit, ItemIsGetable, ItemIsChest, GetItemStrReq, GetItemWeight, GetItemType, GetItemUses, GetItemName, ItemIsWeapon, IsTwoHandedWeapon, IsDimmed
- modMMudFunc: GetArmourTypeEnum, GetWeaponTypeEnum, GetClassWeaponTypeEnum, GetWornTypeEnum, GetItemTypeEnum, GetCostTypeEnum, GetSpellTargetsEnum, GetShopTypeEnum, GetMonAttackTypeEnum, GetMonTypeEnum, GetMonAlignmentEnum

**CONN DB connection management** (4 procs, 223 lines)

- modMMudDatabase: OpenTables, CloseDatabases, IsDatabaseConnectionError, EnsureDatabaseConnection

## Appendix B. Unclassified

None. A script check against `index.json` (procedure names the document does not mention) returns an empty list; see the T-003 card for the check.
