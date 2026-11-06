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
