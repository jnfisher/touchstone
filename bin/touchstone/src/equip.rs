//! The EQ tab's endpoints: equipping through MME's slot rules (`touchstone_rules::equip`) and the
//! per-slot pick lists with their warning flags.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use touchstone_model::World;
use touchstone_rules::Profile;
use touchstone_rules::equip::{
    EquipOptions, EquipOutcome, ListRule, UsableBy, equip_item, set_slot, slot_flags, slot_list,
};
use touchstone_rules::find_best::{self, Criterion, FindBestRequest, LastFind};
use touchstone_rules::gear::EQUIP_SLOTS;

use crate::calc::{ApiResult, Body, bad, json};
use crate::character::CharacterDto;
use crate::state::AppState;

fn unprocessable(msg: impl Into<String>) -> (StatusCode, String) {
    (StatusCode::UNPROCESSABLE_ENTITY, msg.into())
}

#[derive(Deserialize)]
pub struct EquipRequest {
    pub character: CharacterDto,
    /// The item to equip; 0 empties `slot`.
    pub item: i32,
    /// Pick this slot's entry (`cmbEquip_Click`) instead of letting the item choose its slot.
    pub slot: Option<usize>,
    /// Take the item off when it is already worn (`InvenEquipItem`'s toggle).
    #[serde(default)]
    pub unequip: bool,
    /// `bInvenUse2ndWrist`.
    #[serde(default)]
    pub use_2nd_wrist: bool,
}

#[derive(Serialize)]
pub struct EquipResponse {
    pub character: CharacterDto,
    pub messages: Vec<String>,
    /// The slot that was filled or cleared.
    pub slot: Option<usize>,
    pub changed: bool,
    pub flags: Vec<bool>,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct Choice {
    pub number: i32,
    /// MME's combo text, "Name (number)".
    pub label: String,
}

#[derive(Serialize)]
pub struct EquipState {
    /// Per slot, what its combo offers (in table order, without "(none)").
    pub choices: Vec<Vec<Choice>>,
    /// Slots MME paints red.
    pub flags: Vec<bool>,
}

/// The character the global filter judges by. Without a class nothing is flagged, as in MME.
fn usable_by(dto: &CharacterDto) -> UsableBy {
    UsableBy {
        level: dto.level,
        class: dto.class.unwrap_or(0),
        race: dto.race.unwrap_or(0),
        alignment: i32::from(dto.alignment),
        min_item_level: dto.options.min_item_level,
    }
}

fn rule(dto: &CharacterDto) -> ListRule {
    ListRule {
        // MME's default: items marked not in game are not offered
        only_in_game: true,
        usable_by: dto.global_filter.then(|| usable_by(dto)),
    }
}

fn flags(world: &World, dto: &CharacterDto, eq: &[i32; EQUIP_SLOTS]) -> Vec<bool> {
    let who = dto.class.map(|_| usable_by(dto));
    slot_flags(Profile::Stock, world, who.as_ref(), eq).to_vec()
}

fn slots(dto: &CharacterDto) -> Result<[i32; EQUIP_SLOTS], (StatusCode, String)> {
    Ok(dto.to_character().map_err(bad)?.equipped)
}

pub fn equip(world: &World, req: EquipRequest) -> Result<EquipResponse, (StatusCode, String)> {
    let mut dto = req.character;
    let eq = slots(&dto)?;
    let out: EquipOutcome = match req.slot {
        Some(slot) => set_slot(Profile::Stock, world, &eq, slot, req.item, &rule(&dto))
            .map_err(|e| unprocessable(e.to_string()))?,
        None => {
            if req.item != 0 && world.get_item(req.item).is_none() {
                return Err(unprocessable(format!(
                    "item {} is not in the item table",
                    req.item
                )));
            }
            equip_item(
                world,
                &eq,
                req.item,
                &EquipOptions {
                    use_2nd_wrist: req.use_2nd_wrist,
                    unequip_if_equipped: req.unequip,
                    dialogs: true,
                },
            )
        }
    };
    dto.equipped = out.equipped.to_vec();
    let flags = flags(world, &dto, &out.equipped);
    Ok(EquipResponse {
        character: dto,
        messages: out.messages,
        slot: out.slot,
        changed: out.changed,
        flags,
    })
}

pub fn equip_state(world: &World, dto: &CharacterDto) -> Result<EquipState, (StatusCode, String)> {
    let eq = slots(dto)?;
    let rule = rule(dto);
    let choices = (0..EQUIP_SLOTS)
        .map(|slot| {
            slot_list(Profile::Stock, world, slot, &rule)
                .into_iter()
                .filter_map(|n| world.get_item(n))
                .map(|i| Choice {
                    number: i.number,
                    label: format!("{} ({})", i.name, i.number),
                })
                .collect()
        })
        .collect();
    Ok(EquipState {
        choices,
        flags: flags(world, dto, &eq),
    })
}

pub async fn equip_route(
    State(state): State<Arc<AppState>>,
    Body(req): Body<EquipRequest>,
) -> ApiResult {
    let loaded = state.current();
    json(&equip(&loaded.world, req)?)
}

pub async fn equip_state_route(
    State(state): State<Arc<AppState>>,
    Body(dto): Body<CharacterDto>,
) -> ApiResult {
    let loaded = state.current();
    json(&equip_state(&loaded.world, &dto)?)
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LastFindDto {
    pub index: i32,
    /// `enmFindBest`: 0 armour, 1 attack, 2 resist, 3 stats, 4 mystics, 5 attribs.
    pub criterion: i32,
}

#[derive(Deserialize)]
pub struct FindBestBody {
    pub character: CharacterDto,
    /// The item numbers each slot's list offers, in combo order. Without it the lists are the ones the
    /// EQ tab shows for this character, ordered like MME's sorted combos.
    pub slot_lists: Option<Vec<Vec<i32>>>,
    /// The menu entry to run; leave out with `next` to repeat `last`.
    pub criterion: Option<i32>,
    pub index: Option<i32>,
    /// Next Best instead of a fresh Find Best.
    #[serde(default)]
    pub next: bool,
    /// `chkEquipHold`.
    #[serde(default)]
    pub hold: Vec<bool>,
    /// `chkInvenNoLimited`.
    #[serde(default)]
    pub no_limited: bool,
    /// `bInvenUse2ndWrist`.
    #[serde(default)]
    pub use_2nd_wrist: bool,
    /// `nInvenExcludedItems`; `null` before the first Find Best.
    pub excluded: Option<Vec<i32>>,
    #[serde(default)]
    pub last: LastFindDto,
}

#[derive(Serialize)]
pub struct WinnerDto {
    pub slot: usize,
    pub number: i32,
    pub value: i32,
    pub list_index: usize,
}

#[derive(Serialize)]
pub struct FindBestResponse {
    pub character: CharacterDto,
    pub messages: Vec<String>,
    pub winners: Vec<WinnerDto>,
    pub slot_values: Vec<i32>,
    pub excluded: Option<Vec<i32>>,
    pub last: LastFindDto,
    pub ran: bool,
    pub found: bool,
    pub error: Option<i32>,
    pub flags: Vec<bool>,
}

fn criterion_of(code: i32) -> Result<Criterion, (StatusCode, String)> {
    Criterion::from_code(code).ok_or_else(|| unprocessable(format!("no Find Best menu {code}")))
}

/// MME's combos are sorted by their text; a case-insensitive compare of "Name (number)" stands in for the
/// Windows sort (docs/differences.md).
fn combo_lists(world: &World, dto: &CharacterDto) -> Vec<Vec<i32>> {
    let rule = rule(dto);
    (0..EQUIP_SLOTS)
        .map(|slot| {
            let mut v: Vec<(String, i32)> = slot_list(Profile::Stock, world, slot, &rule)
                .into_iter()
                .filter_map(|n| world.get_item(n))
                .map(|i| {
                    (
                        format!("{} ({})", i.name, i.number).to_lowercase(),
                        i.number,
                    )
                })
                .collect();
            v.sort();
            v.into_iter().map(|(_, n)| n).collect()
        })
        .collect()
}

pub fn find_best(
    world: &World,
    body: FindBestBody,
) -> Result<FindBestResponse, (StatusCode, String)> {
    let mut dto = body.character;
    let equipped = slots(&dto)?;
    let lists = match body.slot_lists {
        Some(l) => l,
        None => combo_lists(world, &dto),
    };
    if lists.len() > EQUIP_SLOTS {
        return Err(unprocessable("more than 20 slot lists"));
    }
    if body.hold.len() > EQUIP_SLOTS {
        return Err(unprocessable("more than 20 hold flags"));
    }
    let mut hold = [false; EQUIP_SLOTS];
    for (h, v) in hold.iter_mut().zip(&body.hold) {
        *h = *v;
    }
    let req = FindBestRequest {
        slot_lists: &lists,
        equipped,
        hold,
        no_limited: body.no_limited,
        use_2nd_wrist: body.use_2nd_wrist,
        excluded: body.excluded,
        last: LastFind {
            index: body.last.index,
            criterion: criterion_of(body.last.criterion)?,
        },
    };
    let res = if body.next {
        find_best::next_best(Profile::Stock, world, &req)
    } else {
        let c = body
            .criterion
            .ok_or_else(|| unprocessable("a Find Best needs a criterion"))?;
        find_best::find_best(
            Profile::Stock,
            world,
            &req,
            criterion_of(c)?,
            body.index.unwrap_or(0),
        )
    }
    .map_err(|e| unprocessable(e.to_string()))?;
    dto.equipped = res.equipped.to_vec();
    let flags = flags(world, &dto, &res.equipped);
    let winners = res
        .winners
        .iter()
        .enumerate()
        .filter_map(|(slot, w)| {
            w.map(|w| WinnerDto {
                slot,
                number: w.number,
                value: w.value,
                list_index: w.list_index,
            })
        })
        .collect();
    Ok(FindBestResponse {
        character: dto,
        messages: res.messages,
        winners,
        slot_values: res.slot_values.to_vec(),
        excluded: res.excluded,
        last: LastFindDto {
            index: res.last.index,
            criterion: res.last.criterion as i32,
        },
        ran: res.ran,
        found: res.found,
        error: res.error,
        flags,
    })
}

pub async fn find_best_route(
    State(state): State<Arc<AppState>>,
    Body(req): Body<FindBestBody>,
) -> ApiResult {
    let loaded = state.current();
    json(&find_best(&loaded.world, req)?)
}
