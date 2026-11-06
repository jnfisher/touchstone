//! `POST /api/compare/classify`: which Compare list an item belongs to (card P4-14).
//!
//! `CompareAddItem` and `CompareAddShopItems` choose the list by the record's `ItemType` (0 armour, 1 weapon; any
//! other type is ignored) and `CompareAddShopItems` takes armour only when `Worn` is set. The lists themselves are
//! the page's (they travel with the character); this is the one question that needs the item table. The rows of a
//! list come from `POST /api/evaluate/{tab}` with `options.numbers`.

use std::sync::Arc;

use axum::extract::State;
use serde::{Deserialize, Serialize};

use crate::calc::{ApiResult, Body, bad, json};
use crate::state::AppState;

/// Numbers one request may ask about: an equipment screen (20 slots) or a shop's stock.
const MAX_ITEMS: usize = 500;

#[derive(Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct ClassifyRequest {
    pub items: Vec<i32>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Weapon,
    Armour,
    /// A record `CompareAddItem` has no list for (sundries, keys, scrolls).
    Other,
    /// `tabItems.NoMatch`: MME shows nothing and adds nothing.
    Missing,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Classified {
    pub number: i32,
    pub kind: Kind,
    /// `tabItems.Fields("Worn") <> 0`; `CompareAddShopItems` skips armour that is not worn.
    pub worn: bool,
}

#[derive(Debug, Serialize)]
pub struct ClassifyResponse {
    pub items: Vec<Classified>,
}

pub fn classify(
    world: &touchstone_model::World,
    req: &ClassifyRequest,
) -> Result<ClassifyResponse, String> {
    if req.items.len() > MAX_ITEMS {
        return Err(format!("items has more than {MAX_ITEMS} entries"));
    }
    let items = req
        .items
        .iter()
        .map(|&number| match world.get_item(number) {
            None => Classified {
                number,
                kind: Kind::Missing,
                worn: false,
            },
            Some(it) => Classified {
                number,
                kind: match it.item_type {
                    0 => Kind::Armour,
                    1 => Kind::Weapon,
                    _ => Kind::Other,
                },
                worn: it.worn != 0,
            },
        })
        .collect();
    Ok(ClassifyResponse { items })
}

pub async fn classify_route(
    State(state): State<Arc<AppState>>,
    Body(req): Body<ClassifyRequest>,
) -> ApiResult {
    let loaded = state.current();
    json(&classify(&loaded.world, &req).map_err(bad)?)
}
