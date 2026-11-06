//! The Item Manager's endpoints (FEAT-LISTS / FEAT-EQ): `POST /api/item-manager/{rows,action,
//! delete,copy,add}`.
//!
//! The list is the page's: every request carries the rows (MME's `lvItemManager`) and the row
//! selection, and answers the new rows. The saved lists of the character (`carried`, `stash`) are
//! rebuilt from the rows after each change the way `SaveCharacter` writes them. Questions MME asks
//! go through the same stateless prompt loop as the paste dialog: a question with no answer yet
//! discards the result and comes back as `needs_answer`.

use std::sync::Arc;

use axum::extract::State;
use serde::{Deserialize, Serialize};
use touchstone_model::World;
use touchstone_rules::game_text::ListRow;
use touchstone_rules::item_manager::{self, action};
use touchstone_rules::paste_char::{Prompter, VB_NO, VB_YES};

use crate::calc::{ApiResult, Body, bad, json};
use crate::character::CharacterDto;
use crate::paste::{
    Answer, Asker, Prompt, RowJson, add_rows_by_item, row_from_json, row_to_json, rows_from_stacks,
    sort_by_value_desc,
};
use crate::state::AppState;

/// Rows one request may carry; the list holds what a player carries, stashes and sees on the floor.
const MAX_ROWS: usize = 2000;
const MAX_ANSWERS: usize = 64;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ItemManagerRequest {
    pub character: CharacterDto,
    /// The list as the page holds it; without it the rows are rebuilt from the saved lists.
    pub rows: Option<Vec<RowJson>>,
    /// Selected row indexes (0-based).
    pub selected: Vec<usize>,
    /// The action button (`cmdCompareNav` index 9..18), for `/action`.
    pub action: i32,
    pub answers: Vec<Answer>,
    /// The item to add, for `/add`.
    pub item: i32,
}

#[derive(Debug, Serialize)]
pub struct ItemManagerResponse {
    pub character: CharacterDto,
    pub rows: Vec<RowJson>,
    /// The first question without an answer (nothing is changed until it is answered).
    pub needs_answer: Option<Prompt>,
    /// Rows selected after the call (delete selects the row that followed).
    pub selected: Vec<usize>,
    pub carried_changed: bool,
    pub stash_changed: bool,
    /// What MME shows in a box that is not a question, or the run-time error that stopped it.
    pub messages: Vec<String>,
}

fn load_rows(
    world: &World,
    req: &ItemManagerRequest,
) -> Result<(Vec<ListRow>, touchstone_rules::character::Character), String> {
    if req.answers.len() > MAX_ANSWERS {
        return Err(format!("at most {MAX_ANSWERS} answers"));
    }
    let ch = req.character.to_character()?;
    let rows = match &req.rows {
        Some(r) if r.len() > MAX_ROWS => return Err(format!("at most {MAX_ROWS} rows")),
        Some(r) => r.iter().map(row_from_json).collect(),
        None => rows_from_stacks(world, &req.character, &ch),
    };
    Ok((rows, ch))
}

fn selection(req: &ItemManagerRequest, count: usize) -> Vec<bool> {
    let mut sel = vec![false; count];
    for &i in &req.selected {
        if let Some(s) = sel.get_mut(i) {
            *s = true;
        }
    }
    sel
}

/// The response for an unchanged list that is waiting for an answer.
fn waiting(req: &ItemManagerRequest, rows: &[ListRow], ask: Asker) -> ItemManagerResponse {
    let prompt = ask.pending.map(|p| {
        let mut q = ask.log[p].clone();
        q.answer = None;
        q
    });
    ItemManagerResponse {
        character: req.character.clone(),
        rows: rows.iter().map(row_to_json).collect(),
        needs_answer: prompt,
        selected: req.selected.clone(),
        carried_changed: false,
        stash_changed: false,
        messages: Vec::new(),
    }
}

fn finish(
    mut character: CharacterDto,
    rows: &[ListRow],
    selected: Vec<usize>,
    carried_changed: bool,
    stash_changed: bool,
    messages: Vec<String>,
) -> Result<ItemManagerResponse, String> {
    character.set_saved_from_rows(rows)?;
    Ok(ItemManagerResponse {
        character,
        rows: rows.iter().map(row_to_json).collect(),
        needs_answer: None,
        selected,
        carried_changed,
        stash_changed,
        messages,
    })
}

/// `LoadCharacter`'s list, sorted by Value as the Item Manager shows a fresh paste.
pub fn rows(world: &World, req: &ItemManagerRequest) -> Result<ItemManagerResponse, String> {
    let (mut rows, _) = load_rows(world, req)?;
    if req.rows.is_none() {
        sort_by_value_desc(&mut rows);
    }
    finish(
        req.character.clone(),
        &rows,
        Vec::new(),
        false,
        false,
        vec![],
    )
}

/// `LV_SetActionCell` for the Action buttons.
pub fn action(world: &World, req: &ItemManagerRequest) -> Result<ItemManagerResponse, String> {
    if !(action::DROP_HIDE..=action::CLEAR).contains(&req.action) {
        return Err(format!("{} is not an Action button", req.action));
    }
    let (mut rows, _) = load_rows(world, req)?;
    let sel = selection(req, rows.len());
    let mut ask = Asker::new(&req.answers);
    let mut weight = req.character.add_weight;
    let out =
        item_manager::set_action_cell(world, &mut rows, &sel, req.action, &mut weight, &mut |t| {
            ask.msgbox(t, VB_YES) == VB_YES
        });
    if ask.pending.is_some() {
        return Ok(waiting(req, &load_rows(world, req)?.0, ask));
    }
    let mut character = req.character.clone();
    character.add_weight = weight;
    let messages = out
        .error
        .map(|e| format!("The action stopped early: {e}"))
        .into_iter()
        .collect();
    finish(
        character,
        &rows,
        req.selected.clone(),
        out.carried_changed,
        out.stash_changed,
        messages,
    )
}

/// `LV_DeleteSelected`.
pub fn delete(world: &World, req: &ItemManagerRequest) -> Result<ItemManagerResponse, String> {
    let (mut rows, _) = load_rows(world, req)?;
    let sel = selection(req, rows.len());
    let mut ask = Asker::new(&req.answers);
    let out = item_manager::delete_selected(&mut rows, &sel, &mut || {
        ask.msgbox("Are you sure you want to delete the selected rows?", VB_NO) == VB_YES
    });
    if ask.pending.is_some() {
        return Ok(waiting(req, &load_rows(world, req)?.0, ask));
    }
    if out.declined {
        return finish(
            req.character.clone(),
            &rows,
            req.selected.clone(),
            false,
            false,
            vec![],
        );
    }
    finish(
        req.character.clone(),
        &rows,
        out.next_selected.into_iter().collect(),
        out.carried_changed,
        out.stash_changed,
        vec![],
    )
}

#[derive(Debug, Serialize)]
pub struct CopyResponse {
    /// The commands for the clipboard; `None` leaves the clipboard alone.
    pub text: Option<String>,
}

/// `LV_CopySelectedActionsToClipboard`.
pub fn copy(world: &World, req: &ItemManagerRequest) -> Result<CopyResponse, String> {
    let rows: Vec<ListRow> = req
        .rows
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(row_from_json)
        .collect();
    if rows.len() > MAX_ROWS {
        return Err(format!("at most {MAX_ROWS} rows"));
    }
    let sel = selection(req, rows.len());
    let text = item_manager::copy_selected_actions(world, &rows, &sel)
        .map_err(|e| format!("cannot copy: {e}"))?;
    Ok(CopyResponse { text })
}

/// "Add to Item Manager": `LV_AddRowByItemNumber(item, "Manual", "MANUAL", 1)` for a getable item.
pub fn add(world: &World, req: &ItemManagerRequest) -> Result<ItemManagerResponse, String> {
    let (mut rows, ch) = load_rows(world, req)?;
    let mut messages = Vec::new();
    if item_manager::item_is_getable(world, req.item) {
        let log = add_rows_by_item(
            world,
            &req.character,
            &ch,
            &mut rows,
            &[(req.item, "Manual", "MANUAL", 1)],
        );
        for e in log {
            if let touchstone_rules::game_text::Event::HandledError { source, number } = e {
                messages.push(format!("{source}: run-time error {number}"));
            }
        }
    } else {
        messages.push(format!("Item {} not \"getable\"", req.item));
    }
    let selected = req.selected.clone();
    finish(
        req.character.clone(),
        &rows,
        selected,
        false,
        false,
        messages,
    )
}

macro_rules! route {
    ($name:ident, $f:ident) => {
        pub async fn $name(
            State(state): State<Arc<AppState>>,
            Body(req): Body<ItemManagerRequest>,
        ) -> ApiResult {
            let loaded = state.current();
            json(&$f(&loaded.world, &req).map_err(bad)?)
        }
    };
}

route!(rows_route, rows);
route!(action_route, action);
route!(delete_route, delete);
route!(copy_route, copy);
route!(add_route, add);
