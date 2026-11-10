//! Realm characters (card R-001): the characters of a live Quicksilver/silvermere database, read-only,
//! mapped to the character input the Characters tab posts and evaluated by the same pipeline, with
//! the game's own stored values beside touchstone's for comparison.
//!
//! Nothing here reads anything private: `touchstone_data::silvermere::Realm` holds game fields only.
//! The roster is read with each load, so a reload shows new characters and changed gear; the
//! evaluated sheets are built on first use of a load and kept until the next.

use std::sync::{Arc, Mutex};

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use serde::Serialize;
use serde_json::{Value, json};
use touchstone_data::silvermere::{Realm, RealmCharacter};
use touchstone_model::{Item, World};
use touchstone_rules::Profile;
use touchstone_rules::ability_text::get_ability_name;
use touchstone_rules::clipboard::equip_caption;
use touchstone_rules::gear::spell_average_cast;
use touchstone_rules::pipeline::CharacterState;
use touchstone_rules::spells::spell_is_bless_spell;

use crate::calc::{ApiResult, json as ok_json, not_found};
use crate::character::{CharacterDto, equip_slot, refresh, state_json};
use crate::state::{AppState, iso_from_unix};

/// MME's weapon slot in the 20-slot equip layout.
const WEAPON_SLOT: usize = 16;
const BLESS_SLOTS: usize = 10;
const LEARNED: usize = 100;
/// MME's three-digit boxes.
const MAX_ADD_WEIGHT: i32 = 999;

/// What the coins weigh: three of a type make one unit, each type counted alone.
pub fn coin_weight(rc: &RealmCharacter) -> i32 {
    let w: i64 = rc.coins.iter().map(|c| c.max(&0) / 3).sum();
    i32::try_from(w).unwrap_or(i32::MAX)
}

/// The realm of one load: the characters as read, and their sheets once someone asks.
pub struct RealmSlot {
    realm: Realm,
    /// The sheets, tagged with the `bHideRecordNumbers` setting they were built under.
    sheets: Mutex<Option<(bool, Arc<Vec<Sheet>>)>>,
}

impl RealmSlot {
    pub fn new(realm: Realm) -> Self {
        Self {
            realm,
            sheets: Mutex::new(None),
        }
    }

    pub fn realm(&self) -> &Realm {
        &self.realm
    }

    /// Every sheet of this load, built on first use (and again if the setting that changes the
    /// names in them has changed). Blocking: a realm of hundreds of characters takes a moment.
    fn sheets(&self, profile: Profile, world: &World, hide: bool) -> Arc<Vec<Sheet>> {
        let mut slot = self
            .sheets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((h, sheets)) = slot.as_ref()
            && *h == hide
        {
            return sheets.clone();
        }
        let sheets = Arc::new(
            self.realm
                .characters
                .iter()
                .map(|c| Sheet::build(profile, world, c, hide))
                .collect::<Vec<_>>(),
        );
        *slot = Some((hide, sheets.clone()));
        sheets
    }
}

/// One character, ready to serve.
struct Sheet {
    id: i64,
    summary: Value,
    detail: Value,
    input: Value,
}

// ---------- the mapping ----------

/// Something the game holds that has no place in MME's character.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Unmapped {
    /// `worn`, `weapon`, `inventory` or `active_spell`.
    pub kind: &'static str,
    pub id: i32,
    pub name: String,
    pub reason: String,
}

/// A realm character as the Characters tab's input, and what could not be placed.
pub struct Mapped {
    pub dto: CharacterDto,
    pub unmapped: Vec<Unmapped>,
    /// Per worn item (game slot order): the MME slot it went to, if any.
    pub worn_slots: Vec<Option<usize>>,
    /// Per active spell (game slot order): the bless slot it took, if any.
    pub bless_slots: Vec<Option<usize>>,
}

/// MME's alignment (1 good, 2 neutral, 3 evil) from the game's evil points: the game calls 30 to 39
/// Seedy and 40 and more an Outlaw, and below -50 Good.
pub fn alignment_of(evil: i32) -> i16 {
    if evil < -50 {
        1
    } else if evil >= 40 {
        3
    } else {
        2
    }
}

fn item_name(world: &World, n: i32) -> String {
    world
        .get_item(n)
        .map_or_else(|| format!("item {n}"), |i| i.name.clone())
}

fn spell_name(world: &World, n: i32) -> String {
    world
        .get_spell(n)
        .and_then(|s| s.name.clone())
        .unwrap_or_else(|| format!("spell {n}"))
}

/// Why `equip_slot` found no place for `item`.
fn no_slot_reason(item: &Item) -> String {
    if item.item_type == 1 || matches!(item.worn, 4 | 13 | 14) {
        "a copy of it is already mapped to the pair of slots MME has".to_owned()
    } else {
        format!(
            "MME has no equip slot for an item of type {} worn at location {}",
            item.item_type, item.worn
        )
    }
}

/// The mapping of `rc` into MME's character, under `world`'s data.
pub fn map_character(world: &World, rc: &RealmCharacter) -> Mapped {
    let mut unmapped = Vec::new();
    let mut eq = [0i32; 20];

    // the wielded weapon first: MME's weapon slot holds one, and a weapon in a worn slot is a second
    let mut weapon_taken = false;
    if rc.weapon > 0 {
        match world.get_item(rc.weapon) {
            Some(it) if it.item_type == 1 => {
                eq[WEAPON_SLOT] = it.number;
                weapon_taken = true;
            }
            Some(it) => unmapped.push(Unmapped {
                kind: "weapon",
                id: it.number,
                name: it.name.clone(),
                reason: format!("the wielded item is of type {}, not a weapon", it.item_type),
            }),
            None => unmapped.push(Unmapped {
                kind: "weapon",
                id: rc.weapon,
                name: format!("item {}", rc.weapon),
                reason: "not in the game data".to_owned(),
            }),
        }
    }

    // worn items go where MME's own InvenEquipItem puts them, by the item's worn type
    let mut worn_slots = Vec::new();
    let mut mapped_worn: Vec<i32> = Vec::new();
    for w in &rc.worn {
        let Some(item) = world.get_item(w.item) else {
            unmapped.push(Unmapped {
                kind: "worn",
                id: w.item,
                name: format!("item {}", w.item),
                reason: "not in the game data".to_owned(),
            });
            worn_slots.push(None);
            continue;
        };
        let place = equip_slot(item, &eq, true);
        match place {
            Some(to) if eq[to] == 0 => {
                eq[to] = item.number;
                mapped_worn.push(item.number);
                worn_slots.push(Some(to));
            }
            Some(to) => {
                unmapped.push(Unmapped {
                    kind: "worn",
                    id: item.number,
                    name: item.name.clone(),
                    reason: format!(
                        "MME holds one item in its {} slot, which already has {}",
                        equip_caption(to),
                        item_name(world, eq[to])
                    ),
                });
                worn_slots.push(None);
            }
            None => {
                unmapped.push(Unmapped {
                    kind: "worn",
                    id: item.number,
                    name: item.name.clone(),
                    reason: no_slot_reason(item),
                });
                worn_slots.push(None);
            }
        }
    }

    // what is in the pack and not mapped to a slot is carried; an item MME could not place stays in
    // the pack (it still weighs what it weighs), a placed one is taken out once
    let mut carried: Vec<(i32, i32)> = Vec::new();
    let mut held = mapped_worn;
    if weapon_taken {
        held.push(rc.weapon);
    }
    for inv in &rc.inventory {
        if let Some(k) = held.iter().position(|&h| h == inv.item) {
            held.swap_remove(k);
            continue;
        }
        if world.get_item(inv.item).is_none() {
            unmapped.push(Unmapped {
                kind: "inventory",
                id: inv.item,
                name: format!("item {}", inv.item),
                reason: "not in the game data".to_owned(),
            });
            continue;
        }
        match carried.iter_mut().find(|(n, _)| *n == inv.item) {
            Some((_, q)) => *q += 1,
            None => carried.push((inv.item, 1)),
        }
    }

    // learned spells; running spells that are bless spells fill the bless slots
    let mut learned = vec![0; LEARNED];
    for (k, &s) in rc.spellbook.iter().take(LEARNED).enumerate() {
        learned[k] = s;
    }
    let mut bless = vec![0; BLESS_SLOTS];
    let mut bless_slots = Vec::new();
    let mut next = 0;
    for c in &rc.spells_cast {
        if next < BLESS_SLOTS && spell_is_bless_spell(&world.spells, c.spell) {
            bless[next] = c.spell;
            bless_slots.push(Some(next));
            next += 1;
        } else {
            bless_slots.push(None);
            unmapped.push(Unmapped {
                kind: "active_spell",
                id: c.spell,
                name: spell_name(world, c.spell),
                reason: "not a bless spell, so MME has no slot for it; its effect shows only in the game's stored values"
                    .to_owned(),
            });
        }
    }

    // MME has no coin purse; its extra-weight box takes their weight (three coins of a type weigh one),
    // up to the 999 its boxes hold
    let coin_weight = coin_weight(rc);
    if coin_weight > MAX_ADD_WEIGHT {
        unmapped.push(Unmapped {
            kind: "coins",
            id: 0,
            name: "coins".to_owned(),
            reason: format!(
                "the coins weigh {coin_weight}; MME's extra-weight box holds at most {MAX_ADD_WEIGHT}"
            ),
        });
    }
    let dto = CharacterDto {
        name: rc.first_name.clone(),
        race: world.get_race(rc.race).map(|r| r.number),
        class: world.get_class(rc.class).map(|c| c.number),
        level: rc.level.clamp(0, 999),
        alignment: alignment_of(rc.evil),
        global_filter: true,
        add_weight: coin_weight.min(MAX_ADD_WEIGHT),
        use_add_weight: coin_weight > 0,
        stats: rc.base_stats.to_vec(),
        equipped: eq.to_vec(),
        bless,
        learned_class: if rc.spellbook.is_empty() {
            0
        } else {
            i16::try_from(rc.class).unwrap_or(0)
        },
        learned_spells: learned,
        carried: carried
            .into_iter()
            .map(|(item, qty)| crate::character::StackDto { item, qty })
            .collect(),
        ..CharacterDto::default()
    }
    .padded();
    Mapped {
        dto,
        unmapped,
        worn_slots,
        bless_slots,
    }
}

// ---------- the comparison ----------

/// One stored value and what touchstone calculates for it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Comparison {
    pub field: &'static str,
    pub label: &'static str,
    pub game: i64,
    /// `None` when the pipeline could not calculate it.
    pub touchstone: Option<f64>,
    /// MME's range, for a value the game rolls within one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<[i64; 2]>,
    pub differs: bool,
    /// How close counts as agreeing.
    pub tolerance: &'static str,
    /// The known kind of difference this is, when it differs (see docs/differences.md, "Realm
    /// characters"); `None` for a difference with no known cause.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<&'static str>,
}

fn exact(field: &'static str, label: &'static str, game: i64, t: Option<f64>) -> Comparison {
    Comparison {
        field,
        label,
        game,
        touchstone: t,
        range: None,
        // the pipeline's values go through VB6's CStr and Val, so whole numbers are whole
        differs: t.is_none_or(|t| (t - game as f64).abs() >= 0.5),
        tolerance: "exact (whole numbers)",
        cause: None,
    }
}

/// An `exact` comparison whose difference, if any, has the known `cause`.
fn explained(mut c: Comparison, cause: Option<&'static str>) -> Comparison {
    if c.differs && c.cause.is_none() {
        c.cause = cause;
    }
    c
}

/// The causes `Comparison::cause` can carry; docs/differences.md explains each.
pub mod cause {
    pub const NO_MAGIC: &str = "no-magic-class";
    pub const KAI_MANA: &str = "kai-mana";
    pub const NO_STEALTH_SKILL: &str = "no-stealth-skill";
    pub const NO_PICKLOCKS_SKILL: &str = "no-picklocks-skill";
    pub const ACTIVE_SPELL: &str = "active-spell";
    pub const UNMAPPED_ITEM: &str = "unmapped-item";
    pub const COIN_WEIGHT_CAPPED: &str = "coin-weight-capped";
}

/// The "Alter ..." totals the game keeps, summed over the same sources the mapped character has:
/// race, class, equipped items, carried items that count, and the bless slots (a spell ability of
/// 0 takes the spell's average cast, as the pipeline's bless fold does).
fn alter_total(world: &World, dto: &CharacterDto, abilities: &[i32], best_of: bool) -> i64 {
    let mut total = 0i64;
    let mut best: Option<i64> = None;
    let mut add = |value: i32| {
        if best_of {
            best = Some(best.map_or(i64::from(value), |b| b.max(i64::from(value))));
        } else {
            total += i64::from(value);
        }
    };
    let slots = |add: &mut dyn FnMut(i32), a: &[touchstone_model::AbilitySlot]| {
        for s in a.iter().filter(|s| abilities.contains(&s.ability)) {
            add(s.value);
        }
    };
    for &n in dto.bless.iter().filter(|&&n| n > 0) {
        if let Some(sp) = world.get_spell(n) {
            for a in sp
                .abilities
                .iter()
                .filter(|a| abilities.contains(&a.ability))
            {
                let v = if a.value != 0 {
                    a.value
                } else {
                    spell_average_cast(sp, dto.level.max(1)).unwrap_or(0)
                };
                add(v);
            }
        }
    }
    if let Some(r) = dto.race.and_then(|n| world.get_race(n)) {
        slots(&mut add, &r.abilities);
    }
    if let Some(c) = dto.class.and_then(|n| world.get_class(n)) {
        slots(&mut add, &c.abilities);
    }
    for &n in &dto.equipped {
        if let Some(it) = world.get_item(n) {
            slots(&mut add, &it.abilities);
        }
    }
    for s in &dto.carried {
        // MME counts only Special items and armour worn nowhere when carried
        if let Some(it) = world.get_item(s.item)
            && (it.item_type == 10 || (it.item_type == 0 && it.worn == 0))
        {
            slots(&mut add, &it.abilities);
        }
    }
    if best_of { best.unwrap_or(0) } else { total }
}

/// The game's perception without abilities: a weighted mean of Intellect, Willpower and Charm.
/// MME's Perception box shows the abilities alone, so the two are compared on the abilities part.
fn perception_base(rc: &RealmCharacter) -> i64 {
    let [_, int, wil, _, _, cha] = rc.current_stats.map(i64::from);
    (int * 5 + wil * 2 + cha) / 8
}

/// The comparison list for a character whose pipeline ran.
pub fn compare(
    world: &World,
    rc: &RealmCharacter,
    dto: &CharacterDto,
    st: &CharacterState,
    unmapped: &[Unmapped],
) -> Vec<Comparison> {
    let s = &st.stats;
    let has_active = !rc.spells_cast.is_empty();
    // a stored value can differ because an item or spell has no place in MME's character
    let why = |c: Comparison| {
        let cause = if !unmapped.is_empty() && unmapped.iter().any(|u| u.kind != "active_spell") {
            Some(cause::UNMAPPED_ITEM)
        } else if has_active {
            Some(cause::ACTIVE_SPELL)
        } else {
            None
        };
        explained(c, cause)
    };
    let mut out = Vec::new();
    let labels: [&'static str; 6] = [
        "Strength",
        "Intellect",
        "Willpower",
        "Agility",
        "Health",
        "Charm",
    ];
    let fields: [&'static str; 6] = ["str", "int", "wil", "agi", "hea", "cha"];
    for k in 0..6 {
        out.push(why(exact(
            fields[k],
            labels[k],
            i64::from(rc.current_stats[k]),
            Some(s.stat_with_bonus[k]),
        )));
    }
    // max HP: the game rolls hit points at training, MME gives the range of the class and level
    let range = st.hit_points.range.as_ref();
    out.push(why(Comparison {
        field: "max_hp",
        label: "Max HP",
        game: i64::from(rc.max_hp),
        touchstone: range.map(|r| r.average),
        range: range.map(|r| [i64::from(r.min), i64::from(r.max)]),
        differs: range.is_none_or(|r| rc.max_hp < r.min || rc.max_hp > r.max),
        tolerance: "inside MME's min to max range",
        cause: None,
    }));

    let class = world.get_class(rc.class);
    let magery = touchstone_rules::stats::get_class_magery(class);
    let magic_cause = match magery {
        touchstone_model::Magery::None => Some(cause::NO_MAGIC),
        touchstone_model::Magery::Kai => Some(cause::KAI_MANA),
        _ => None,
    };
    out.push(why(explained(
        exact(
            "max_mana",
            "Max mana",
            i64::from(rc.max_mana),
            st.magic.as_ref().and_then(|m| m.max_mana).map(f64::from),
        ),
        magic_cause,
    )));
    out.push(why(exact(
        "max_mr",
        "Magic resistance",
        i64::from(rc.max_mr),
        st.magic_res.ok().map(f64::from),
    )));
    out.push(why(explained(
        exact(
            "spellcast",
            "Spellcasting",
            i64::from(rc.spellcast),
            st.magic
                .as_ref()
                .and_then(|m| m.spellcasting)
                .map(f64::from),
        ),
        magic_cause,
    )));
    out.push(why(Comparison {
        field: "perception_abilities",
        label: "Perception (abilities)",
        game: rc.perception as i64 - perception_base(rc),
        touchstone: Some(s.stat[18]),
        range: None,
        differs: ((rc.perception as i64 - perception_base(rc)) as f64 - s.stat[18]).abs() >= 0.5,
        tolerance: "exact; the game's perception less its Int/Wil/Cha term against MME's ability total",
        cause: None,
    }));
    let stealth = exact(
        "stealth",
        "Stealth",
        i64::from(rc.stealth),
        Some(s.stat[19]),
    );
    let stealth_cause = (!s.class_stealth && !s.race_stealth).then_some(cause::NO_STEALTH_SKILL);
    out.push(why(explained(stealth, stealth_cause)));
    let picks = exact(
        "picklocks",
        "Picklocks",
        i64::from(rc.picklocks),
        st.picklocks.total.map(f64::from),
    );
    let picks_cause = (rc.picklocks == 0).then_some(cause::NO_PICKLOCKS_SKILL);
    out.push(why(explained(picks, picks_cause)));
    // MME's weight includes the coins (the extra-weight box); the game stores the items alone
    let coins_in_box = f64::from(dto.add_weight);
    out.push(why(exact(
        "weight",
        "Weight carried (items)",
        i64::from(rc.weight),
        Some(s.stat[0] - coins_in_box),
    )));
    // the game's percentage counts coins and is not capped; MME's current over maximum
    let pct = if s.stat[1] > 0.0 {
        (s.stat[0] * 100.0 / s.stat[1]).trunc()
    } else {
        0.0
    };
    let capped = unmapped.iter().any(|u| u.kind == "coins");
    out.push(why(explained(
        exact(
            "encumbrance_pct",
            "Encumbrance %",
            i64::from(rc.encumbrance_pct),
            Some(pct),
        ),
        capped.then_some(cause::COIN_WEIGHT_CAPPED),
    )));
    out.push(why(exact(
        "energy_use",
        "Alter Energy Usage",
        i64::from(rc.energy_use),
        Some(f64::from(s.energy_use)),
    )));
    let alter =
        |abilities: &[i32], best_of: bool| alter_total(world, dto, abilities, best_of) as f64;
    for (field, label, game, abilities, best_of) in [
        (
            "av_bonus",
            "Alter AV",
            rc.av_bonus,
            &[22, 105, 106][..],
            true,
        ),
        (
            "ac_bonus",
            "Alter Defence Value",
            rc.ac_bonus,
            &[2][..],
            false,
        ),
        ("dmg_bonus", "Alter Damage", rc.dmg_bonus, &[4][..], false),
        (
            "crit_bonus",
            "Alter Critical Hit Chance",
            rc.crit_bonus,
            &[58][..],
            false,
        ),
        (
            "dr_bonus",
            "Alter Damage Resistance",
            rc.dr_bonus,
            &[7][..],
            false,
        ),
        ("dr_pct", "Alter DR by percent", rc.dr_pct, &[99][..], false),
        ("shield", "Protective Shield", rc.shield, &[10][..], false),
        ("def_mod", "Defence Modifier", rc.def_mod, &[104][..], false),
        (
            "heal_rate",
            "Alter healing rate",
            rc.heal_rate,
            &[123][..],
            false,
        ),
        (
            "mana_rate",
            "Alter Mana Regeneration",
            rc.mana_rate,
            &[145][..],
            false,
        ),
        (
            "room_light",
            "Alter Room Light",
            rc.room_light,
            &[14][..],
            false,
        ),
    ] {
        let mut c = exact(
            field,
            label,
            i64::from(game),
            Some(alter(abilities, best_of)),
        );
        c.tolerance = "exact; touchstone sums the ability over race, class, equipped and carried items and bless spells";
        out.push(why(c));
    }
    out
}

// ---------- the sheet ----------

fn alignment_name(a: i16) -> &'static str {
    match a {
        1 => "good",
        3 => "evil",
        _ => "neutral",
    }
}

impl Sheet {
    fn build(profile: Profile, world: &World, rc: &RealmCharacter, hide: bool) -> Self {
        let m = map_character(world, rc);
        let (sheet, sheet_error, comparison) = match refresh(profile, world, &m.dto, hide) {
            Ok((ch, st)) => {
                let cmp = compare(world, rc, &m.dto, &st, &m.unmapped);
                (
                    serde_json::to_value(state_json(profile, world, &st, &ch, hide)).ok(),
                    None,
                    cmp,
                )
            }
            Err((_, msg)) => (None, Some(msg), Vec::new()),
        };
        let differs = comparison.iter().filter(|c| c.differs).count();
        let race_name = world.get_race(rc.race).map(|r| r.name.clone());
        let class_name = world.get_class(rc.class).map(|c| c.name.clone());
        let last_on_iso = (rc.last_on > 0).then(|| iso_from_unix(rc.last_on as u64));
        let summary = json!({
            "id": rc.id,
            "first_name": rc.first_name,
            "last_name": rc.last_name,
            "race": rc.race,
            "race_name": race_name,
            "class": rc.class,
            "class_name": class_name,
            "level": rc.level,
            "exp": rc.exp,
            "gang": rc.gang,
            "map": rc.map,
            "room": rc.room,
            "last_on": rc.last_on,
            "last_on_iso": last_on_iso,
            "lives": rc.lives,
            "differs": if sheet.is_some() { json!(differs) } else { Value::Null },
        });

        let worn: Vec<Value> = rc
            .worn
            .iter()
            .zip(&m.worn_slots)
            .map(|(w, slot)| {
                json!({
                    "slot": w.slot,
                    "item": w.item,
                    "name": item_name(world, w.item),
                    "uses": w.uses,
                    "mme_slot": slot,
                    "mme_slot_name": slot.map(equip_caption),
                })
            })
            .collect();
        let inventory: Vec<Value> = rc
            .inventory
            .iter()
            .map(|i| {
                json!({
                    "slot": i.slot,
                    "item": i.item,
                    "name": item_name(world, i.item),
                    "uses": i.uses,
                    "readied": i.slot == rc.readied,
                })
            })
            .collect();
        let spellbook: Vec<Value> = rc
            .spellbook
            .iter()
            .map(|&s| json!({ "spell": s, "name": spell_name(world, s) }))
            .collect();
        let active: Vec<Value> = rc
            .spells_cast
            .iter()
            .zip(&m.bless_slots)
            .map(|(c, b)| {
                json!({
                    "slot": c.slot,
                    "spell": c.spell,
                    "name": spell_name(world, c.spell),
                    "value": c.value,
                    "rounds": c.rounds,
                    "bless_slot": b,
                })
            })
            .collect();
        let abilities: Vec<Value> = rc
            .abilities
            .iter()
            .map(|&(a, v)| {
                json!({
                    "ability": a,
                    "name": i16::try_from(a).map_or_else(|_| format!("ability {a}"), |n| get_ability_name(profile, n, true)),
                    "value": v,
                })
            })
            .collect();
        let alignment = m.dto.alignment;
        let identity = json!({
            "first_name": rc.first_name,
            "last_name": rc.last_name,
            "title": rc.title,
            "race": rc.race,
            "race_name": race_name,
            "class": rc.class,
            "class_name": class_name,
            "level": rc.level,
            "exp": rc.exp,
            "gang": rc.gang,
            "evil": rc.evil,
            "alignment": alignment,
            "alignment_name": alignment_name(alignment),
            "lawful": rc.lawful,
            "lives": rc.lives,
            "map": rc.map,
            "room": rc.room,
            "last_on": rc.last_on,
            "last_on_iso": last_on_iso,
            "death_date": rc.death_date,
            "unspent_cp": rc.unspent_cp,
            "total_cp": rc.total_cp,
            "coins": rc.coins,
        });
        let game = json!({
            "base_stats": rc.base_stats,
            "current_stats": rc.current_stats,
            "max_hp": rc.max_hp,
            "hp": rc.hp,
            "max_mana": rc.max_mana,
            "mana": rc.mana,
            "max_mr": rc.max_mr,
            "mr": rc.mr,
            "max_weight": rc.max_weight,
            "weight": rc.weight,
            "max_energy": rc.max_energy,
            "energy": rc.energy,
            "encumbrance_pct": rc.encumbrance_pct,
            "perception": rc.perception,
            "stealth": rc.stealth,
            "martial": rc.martial,
            "thievery": rc.thievery,
            "spellcast": rc.spellcast,
            "traps": rc.traps,
            "disarm": rc.disarm,
            "picklocks": rc.picklocks,
            "tracking": rc.tracking,
            "hp_bonus": rc.hp_bonus,
            "mana_bonus": rc.mana_bonus,
            "random_hp": rc.random_hp,
            "martial_base": rc.martial_base,
            "av_bonus": rc.av_bonus,
            "ac_bonus": rc.ac_bonus,
            "dmg_bonus": rc.dmg_bonus,
            "crit_bonus": rc.crit_bonus,
            "dr_bonus": rc.dr_bonus,
            "dr_pct": rc.dr_pct,
            "energy_use": rc.energy_use,
            "shield": rc.shield,
            "def_mod": rc.def_mod,
            "heal_rate": rc.heal_rate,
            "mana_rate": rc.mana_rate,
            "room_light": rc.room_light,
        });
        let weapon = (rc.weapon > 0).then(|| {
            json!({
                "item": rc.weapon,
                "name": item_name(world, rc.weapon),
                "mme_slot": WEAPON_SLOT,
                "mapped": m.dto.equipped[WEAPON_SLOT] == rc.weapon,
            })
        });
        let mut detail = serde_json::Map::new();
        detail.insert("available".into(), json!(true));
        detail.insert("id".into(), json!(rc.id));
        detail.insert("identity".into(), identity);
        detail.insert("game".into(), game);
        detail.insert("sheet".into(), json!(sheet));
        detail.insert("sheet_error".into(), json!(sheet_error));
        detail.insert("worn".into(), json!(worn));
        detail.insert("weapon".into(), json!(weapon));
        detail.insert("inventory".into(), json!(inventory));
        detail.insert("spellbook".into(), json!(spellbook));
        detail.insert("active_spells".into(), json!(active));
        detail.insert("abilities".into(), json!(abilities));
        detail.insert("comparison".into(), json!(comparison));
        detail.insert("differs".into(), json!(differs));
        detail.insert("unmapped".into(), json!(m.unmapped));
        let detail = Value::Object(detail);
        let input = serde_json::to_value(&m.dto).unwrap_or(Value::Null);
        Self {
            id: rc.id,
            summary,
            detail,
            input,
        }
    }
}

// ---------- the endpoints ----------

fn unavailable() -> ApiResult {
    ok_json(&json!({
        "available": false,
        "reason": "this data source has no realm: only a live silvermere or Quicksilver database has characters",
    }))
}

/// The sheets of the current load, or `None` for a source with no realm.
async fn ready(state: &Arc<AppState>) -> Option<Arc<Vec<Sheet>>> {
    let loaded = state.current();
    loaded.realm.as_ref()?;
    let hide = state.hide_record_numbers();
    tokio::task::spawn_blocking(move || {
        loaded
            .realm
            .as_ref()
            .map(|r| r.sheets(loaded.profile, &loaded.world, hide))
    })
    .await
    .ok()
    .flatten()
}

fn find(sheets: &[Sheet], id: i64) -> Result<&Sheet, (axum::http::StatusCode, String)> {
    sheets
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| not_found(format!("no realm character {id}")))
}

/// `GET /api/realm/characters`
pub async fn list_route(State(state): State<Arc<AppState>>) -> ApiResult {
    let Some(sheets) = ready(&state).await else {
        return unavailable();
    };
    let rows: Vec<&Value> = sheets.iter().map(|s| &s.summary).collect();
    ok_json(&json!({ "available": true, "characters": rows }))
}

/// `GET /api/realm/characters/{id}`
pub async fn get_route(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> ApiResult {
    let Some(sheets) = ready(&state).await else {
        return unavailable();
    };
    Ok(axum::Json(&find(&sheets, id)?.detail).into_response())
}

/// `GET /api/realm/characters/{id}/input`: the character as the Characters tab's input, to copy into
/// the sandbox editor or post to the calculators.
pub async fn input_route(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> ApiResult {
    let Some(sheets) = ready(&state).await else {
        return unavailable();
    };
    Ok(axum::Json(&find(&sheets, id)?.input).into_response())
}
