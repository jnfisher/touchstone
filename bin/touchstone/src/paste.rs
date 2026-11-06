//! The Paste Character dialog's endpoints (FEAT-PASTE): `POST /api/paste/character`,
//! `POST /api/paste/party` and `POST /api/paste/party/continue`.
//!
//! The ports in `touchstone-rules` ask through a `Prompter` and write into a plain state. A request
//! here is stateless: it carries the character, the pasted text and the answers given so far, in
//! the order MME asks its questions. The port is run from the top; when it reaches a question that
//! has no answer yet the result is thrown away and the question is returned as `needs_answer`, so
//! the page asks the user and posts again with the answer appended.

use std::sync::Arc;

use axum::extract::State;
use serde::{Deserialize, Serialize};
use touchstone_model::World;
use touchstone_rules::Profile;
use touchstone_rules::character::Character;
use touchstone_rules::game_text::{Event, ListRow, MatchEnv, SubTag, lv_add_row_by_item_number};
use touchstone_rules::gear::{EQUIP_SLOTS, ItemManagerRow};
use touchstone_rules::item_manager;
use touchstone_rules::paste_char::{
    self, PasteEnv, PasteEvent, PasteState, Prompter, VB_CANCEL, VB_NO, VB_YES,
};
use touchstone_rules::paste_party::{self, ListEntry, PartyForm, PasteInput, PasteStatus};
use touchstone_rules::pipeline::{PipelineOptions, refresh_all};
use touchstone_rules::stats::compute_avg_level_max_stats;
use touchstone_vb6 as vb;

use crate::calc::{ApiResult, Body, bad, json};
use crate::character::{AttackDto, CharacterDto};
use crate::detail::nmr_ver;
use crate::state::AppState;

/// Most answers one request may carry; MME asks a handful of questions per paste.
const MAX_ANSWERS: usize = 64;
/// Longest paste accepted (the party parser's 16-bit counters give up near 32 K anyway).
const MAX_TEXT: usize = 1_000_000;

// ---------- answers and the prompter ----------

/// One reply: the button a message box was closed with (`vbYes` 6, `vbNo` 7, `vbCancel` 2) or the
/// text of an input box (empty is Cancel).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum Answer {
    Button(i32),
    Text(String),
}

/// A box MME showed (or would show), in order.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Prompt {
    /// "info" (OK only), "yes_no", "yes_no_cancel" or "input".
    pub kind: &'static str,
    pub text: String,
    /// The button or text the box defaults to.
    pub default: Answer,
    /// The reply; absent for an information box and for a question not answered yet.
    pub answer: Option<Answer>,
}

pub(crate) struct Asker {
    answers: Vec<Answer>,
    next: usize,
    pub(crate) log: Vec<Prompt>,
    /// Index into `log` of the first question that had no answer.
    pub(crate) pending: Option<usize>,
    /// For each question, its position in `log` (answers are indexed by question).
    questions: Vec<usize>,
}

impl Asker {
    pub(crate) fn new(answers: &[Answer]) -> Self {
        Self {
            answers: answers.to_vec(),
            next: 0,
            log: Vec::new(),
            pending: None,
            questions: Vec::new(),
        }
    }

    fn take(&mut self) -> Option<Answer> {
        let a = self.answers.get(self.next).cloned();
        if a.is_some() {
            self.next += 1;
        }
        a
    }
}

impl Prompter for Asker {
    fn msgbox(&mut self, text: &str, default: i32) -> i32 {
        if default == 1 {
            self.log.push(Prompt {
                kind: "info",
                text: text.to_owned(),
                default: Answer::Button(1),
                answer: None,
            });
            return 1;
        }
        let kind = if default == VB_CANCEL {
            "yes_no_cancel"
        } else {
            "yes_no"
        };
        // Once a question has gone unanswered the rest of the run is only to find out what comes
        // next; it takes the defaults and the answers are not read.
        let answer = if self.pending.is_some() {
            None
        } else {
            self.take()
        };
        let pos = self.log.len();
        self.questions.push(pos);
        let result = match &answer {
            Some(Answer::Button(b)) if [VB_YES, VB_NO, VB_CANCEL].contains(b) => *b,
            _ => default,
        };
        if answer.is_none() && self.pending.is_none() {
            self.pending = Some(pos);
        }
        self.log.push(Prompt {
            kind,
            text: text.to_owned(),
            default: Answer::Button(default),
            answer: answer.map(|_| Answer::Button(result)),
        });
        result
    }

    fn inputbox(&mut self, prompt: &str, default: &str) -> String {
        let answer = if self.pending.is_some() {
            None
        } else {
            self.take()
        };
        let pos = self.log.len();
        self.questions.push(pos);
        let result = match &answer {
            Some(Answer::Text(t)) => t.clone(),
            // a button where text was wanted is taken as "accept the default"
            _ => default.to_owned(),
        };
        if answer.is_none() && self.pending.is_none() {
            self.pending = Some(pos);
        }
        self.log.push(Prompt {
            kind: "input",
            text: prompt.to_owned(),
            default: Answer::Text(default.to_owned()),
            answer: answer.map(|_| Answer::Text(result.clone())),
        });
        result
    }
}

// ---------- the Item Manager on the wire ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SubJson {
    pub text: String,
    /// The Worn column's Integer tag or the Value column's String tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RowJson {
    /// The item number.
    pub text: String,
    /// Name, Flag, QTY, Source, Enc, Type, Worn, Usable, Value, Shop.
    pub subs: Vec<SubJson>,
}

pub(crate) fn row_to_json(r: &ListRow) -> RowJson {
    RowJson {
        text: r.text.clone(),
        subs: r
            .subs
            .iter()
            .map(|s| SubJson {
                text: s.text.clone(),
                tag: s.tag.as_ref().map(|t| match t {
                    SubTag::Int(n) => serde_json::Value::from(*n),
                    SubTag::Text(t) => serde_json::Value::from(t.clone()),
                }),
            })
            .collect(),
    }
}

pub(crate) fn row_from_json(r: &RowJson) -> ListRow {
    ListRow {
        text: r.text.clone(),
        subs: r
            .subs
            .iter()
            .map(|s| touchstone_rules::game_text::SubItem {
                text: s.text.clone(),
                tag: s.tag.as_ref().and_then(|t| match t {
                    serde_json::Value::Number(n) => n
                        .as_i64()
                        .and_then(|n| i32::try_from(n).ok())
                        .map(SubTag::Int),
                    serde_json::Value::String(s) => Some(SubTag::Text(s.clone())),
                    _ => None,
                }),
            })
            .collect(),
    }
}

/// Index of the Value column's sub item (the 9th: Name, Flag, QTY, Source, Enc, Type, Worn,
/// Usable, Value).
const VALUE_SUB: usize = 8;

/// `LV_Sort_ColumnClickOrSticky` on the Value column's tag, largest first. The sort is stable, so
/// rows with the same value stay in the order they were added.
pub fn sort_by_value_desc(rows: &mut [ListRow]) {
    let key = |r: &ListRow| -> f64 {
        match r.subs.get(VALUE_SUB).and_then(|s| s.tag.as_ref()) {
            Some(SubTag::Text(t)) => vb::val(t),
            Some(SubTag::Int(n)) => f64::from(*n),
            None => 0.0,
        }
    };
    rows.sort_by(|a, b| key(b).total_cmp(&key(a)));
}

// ---------- character paste ----------

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PasteCharacterRequest {
    pub character: CharacterDto,
    pub text: String,
    pub answers: Vec<Answer>,
    /// The Item Manager as the page holds it; without it the rows are rebuilt from the
    /// character's `carried` and `stash` lists, as `LoadCharacter` does.
    pub rows: Option<Vec<RowJson>>,
    /// `bCharLoaded`: a saved character is open. Defaults to "the character has a name".
    pub char_loaded: Option<bool>,
    /// `chkUnequipMissing`.
    pub unequip_missing: bool,
    /// `chkEquipHold(x)`: slots the paste leaves alone. Short lists are padded with false.
    pub hold: Vec<bool>,
    /// `bOnlyInGame` (on by default, as in MME's settings).
    pub only_in_game: bool,
    /// `bInvenUse2ndWrist` (on by default).
    pub second_wrist: bool,
}

impl Default for PasteCharacterRequest {
    fn default() -> Self {
        Self {
            character: CharacterDto::default(),
            text: String::new(),
            answers: Vec::new(),
            rows: None,
            char_loaded: None,
            unequip_missing: false,
            hold: Vec::new(),
            only_in_game: true,
            second_wrist: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventJson {
    /// A box that did not go through the prompter (an error inside the item matcher).
    Message {
        text: String,
    },
    /// `SaveCharacter`; the page's autosave covers it.
    Save {
        prompt: bool,
        file: String,
    },
    /// The Char tab was reset ("Unload current character file first?" answered Yes).
    ClearCharacter,
    HandledError {
        source: String,
        number: u32,
    },
}

#[derive(Debug, Serialize)]
pub struct PasteCharacterResponse {
    /// The character after the paste (the request's, when a question is still open).
    pub character: CharacterDto,
    pub events: Vec<EventJson>,
    /// Every box MME showed, in order, with the answers given.
    pub prompts: Vec<Prompt>,
    /// The first question without an answer: the page asks it and posts again.
    pub needs_answer: Option<Prompt>,
    /// The Item Manager was empty and now has rows; they are already sorted by Value, largest first.
    pub sort_after: bool,
    pub rows: Vec<RowJson>,
    /// `frmMonsterAttackSim` AC/DR boxes the paste would set (the page has no such form open).
    pub sim_ac: Option<i32>,
    pub sim_dr: Option<i32>,
}

/// What a charm is worth to the shops: the charm tag and its bonus from the gear and bless
/// bonuses, as the Char tab holds them when a paste starts.
fn charm_of(
    world: &World,
    ch: &Character,
    min_item_level: i32,
    item_manager: Vec<ItemManagerRow>,
) -> (String, f64) {
    let base = f64::from(ch.stats[5]);
    if ch.class.is_none() || ch.race.is_none() {
        return (vb::cstr(base), 0.0);
    }
    let opts = PipelineOptions {
        min_item_level,
        item_manager,
        dat_ver_caption: world.info.dat_file_version.clone(),
        nmr_ver: nmr_ver(&world.info.nmr_version),
        avg_level_max_all_stats: compute_avg_level_max_stats(Profile::Stock, &world.races)
            .all_stats,
        ..PipelineOptions::default()
    };
    match refresh_all(Profile::Stock, world, ch, &opts) {
        Ok(st) => (
            vb::cstr(st.stats.stat_with_bonus[5]),
            st.stats.label_bonus[5],
        ),
        Err(_) => (vb::cstr(base), 0.0),
    }
}

/// The equipment combos with what is equipped in them: MME's combos hold the items added to the
/// Char tab; a ring or wrist item is listed in both of its slots' combos.
fn combos_for(equipped: &[i32; EQUIP_SLOTS]) -> (Vec<Vec<i32>>, [usize; EQUIP_SLOTS]) {
    let mut combos = vec![Vec::new(); EQUIP_SLOTS];
    let mut sel = [0usize; EQUIP_SLOTS];
    for pair in [(9, 10), (6, 7)] {
        let mut shared: Vec<i32> = Vec::new();
        for s in [pair.0, pair.1] {
            let n = equipped[s];
            if n > 0 && !shared.contains(&n) {
                shared.push(n);
            }
        }
        for s in [pair.0, pair.1] {
            combos[s].clone_from(&shared);
            if let Some(p) = shared.iter().position(|&n| n == equipped[s] && n > 0) {
                sel[s] = p + 1;
            }
        }
    }
    for (s, &n) in equipped.iter().enumerate() {
        if [6, 7, 9, 10].contains(&s) || n <= 0 {
            continue;
        }
        combos[s].push(n);
        sel[s] = 1;
    }
    (combos, sel)
}

fn stat_texts(stats: &[i32; 6]) -> [String; 6] {
    stats.map(|n| n.to_string())
}

/// The Char tab a paste starts from.
fn start_state(
    world: &World,
    req: &PasteCharacterRequest,
    ch: Character,
    rows: Vec<ListRow>,
    char_loaded: bool,
) -> PasteState {
    let (combos, sel) = combos_for(&ch.equipped);
    let (charm_tag, charm_bonus) = charm_of(
        world,
        &ch,
        req.character.options.min_item_level,
        item_manager::gear_rows(&rows),
    );
    PasteState {
        stat_text: stat_texts(&ch.stats),
        charm_tag,
        charm_bonus,
        combos,
        sel,
        hold: std::array::from_fn(|i| req.hold.get(i).copied().unwrap_or(false)),
        unequip_missing: req.unequip_missing,
        rows,
        char_loaded,
        prompt_save: false,
        session_char_file: String::new(),
        refresh_delay: false,
        character: ch,
    }
}

/// `LoadCharacter`'s Item Manager: the carried rows, then the stashed ones.
pub(crate) fn rows_from_stacks(world: &World, dto: &CharacterDto, ch: &Character) -> Vec<ListRow> {
    let (charm_tag, _) = charm_of(world, ch, dto.options.min_item_level, dto.gear_rows(world));
    let who = who_for(ch);
    let menv = MatchEnv {
        world,
        only_in_game: true,
        hide_record_numbers: false,
        charm_tag: &charm_tag,
        who: &who,
        equipped: &ch.equipped,
    };
    let mut rows = Vec::new();
    let mut log: Vec<Event> = Vec::new();
    for (flag, list) in [("CARRIED", &dto.carried), ("STASH", &dto.stash)] {
        for s in list {
            lv_add_row_by_item_number(&menv, &mut rows, s.item, "Saved", flag, s.qty, 0, &mut log);
        }
    }
    rows
}
/// `LV_AddRowByItemNumber` for the character's current charm and usability; the handled errors
/// MME would report come back as events.
pub(crate) fn add_rows_by_item(
    world: &World,
    dto: &CharacterDto,
    ch: &Character,
    rows: &mut Vec<ListRow>,
    adds: &[(i32, &str, &str, i32)],
) -> Vec<Event> {
    let (charm_tag, _) = charm_of(
        world,
        ch,
        dto.options.min_item_level,
        item_manager::gear_rows(rows),
    );
    let who = who_for(ch);
    let menv = MatchEnv {
        world,
        only_in_game: true,
        hide_record_numbers: false,
        charm_tag: &charm_tag,
        who: &who,
        equipped: &ch.equipped,
    };
    let mut log: Vec<Event> = Vec::new();
    for &(item, source, flag, qty) in adds {
        lv_add_row_by_item_number(&menv, rows, item, source, flag, qty, 0, &mut log);
    }
    log
}

fn who_for(ch: &Character) -> touchstone_rules::gear::Character {
    touchstone_rules::gear::Character {
        level: ch.level,
        class: ch.class.unwrap_or(0),
        race: ch.race.unwrap_or(0),
        alignment: i32::from(ch.alignment),
        min_item_level: 0,
    }
}

struct Run {
    st: PasteState,
    out: paste_char::PasteOutcome,
    ask_log: Vec<Prompt>,
    questions: Vec<usize>,
    pending: Option<usize>,
}

fn run_once(
    world: &World,
    req: &PasteCharacterRequest,
    st: &mut PasteState,
    answers: &[Answer],
) -> (paste_char::PasteOutcome, Asker) {
    let mut ask = Asker::new(answers);
    let name = st.character.name.clone();
    let exists = |f: &str| !f.is_empty();
    let env = PasteEnv {
        world,
        only_in_game: req.only_in_game,
        second_wrist: req.second_wrist,
        hide_record_numbers: false,
        nmr_ver: nmr_ver(&world.info.nmr_version),
        disable_kai_autolearn: world.info.disable_kai,
        min_item_level: req.character.options.min_item_level,
        alignment: i32::from(st.character.alignment),
        attack_sim_loaded: false,
        // touchstone saves a character as it is edited, so MME's "save first" is automatic
        auto_save: true,
        last_char_file: &name,
        file_exists: &exists,
    };
    let out = paste_char::paste_character(&env, st, &req.text, &mut ask);
    (out, ask)
}

/// A blank Char tab, as `LoadCharacter` with clear-selections leaves it. The saved Item Manager
/// lists and the unmodelled `.mmec` keys belong to the file and stay.
fn blank_character(keep: &CharacterDto) -> CharacterDto {
    CharacterDto {
        level: 1,
        alignment: keep.alignment,
        attack: AttackDto::default(),
        carried: keep.carried.clone(),
        stash: keep.stash.clone(),
        extra: keep.extra.clone(),
        options: keep.options.clone(),
        ..CharacterDto::default()
    }
}

fn lossless_dto(base: &CharacterDto, st: &PasteState) -> CharacterDto {
    let c = &st.character;
    let mut d = base.clone();
    d.name.clone_from(&c.name);
    d.race = c.race;
    d.class = c.class;
    d.level = c.level;
    d.global_filter = c.global_filter;
    d.stats = c.stats.to_vec();
    d.equipped = c.equipped.to_vec();
    d.add_weight = c.add_weight;
    d.use_add_weight = c.use_add_weight;
    d.learned_spells = c.learned.spells.to_vec();
    d.learned_class = c.learned.class;
    d
}

fn events_json(events: &[PasteEvent], prompts: &[Prompt]) -> Vec<EventJson> {
    // A box the port pushed as an event and also asked through the prompter is one box.
    let mut seen: Vec<bool> = vec![false; prompts.len()];
    let mut out = Vec::new();
    for e in events {
        match e {
            PasteEvent::MsgBox(t) => {
                let hit = prompts
                    .iter()
                    .enumerate()
                    .find(|(k, p)| !seen[*k] && p.kind != "input" && p.text == *t);
                if let Some((k, _)) = hit {
                    seen[k] = true;
                } else {
                    out.push(EventJson::Message { text: t.clone() });
                }
            }
            PasteEvent::InputBox { .. } => {}
            PasteEvent::Save { prompt, file } => out.push(EventJson::Save {
                prompt: *prompt,
                file: file.clone(),
            }),
            PasteEvent::ClearCharacter => out.push(EventJson::ClearCharacter),
            PasteEvent::HandledError { source, number } => out.push(EventJson::HandledError {
                source: (*source).to_owned(),
                number: *number,
            }),
        }
    }
    out
}

/// The paste, with the Char-tab reset MME does in the middle of it.
pub fn paste_character(
    world: &World,
    req: &PasteCharacterRequest,
) -> Result<PasteCharacterResponse, String> {
    if req.text.len() > MAX_TEXT {
        return Err(format!("the paste is longer than {MAX_TEXT} bytes"));
    }
    if req.answers.len() > MAX_ANSWERS {
        return Err(format!("at most {MAX_ANSWERS} answers"));
    }
    let ch = req.character.to_character()?;
    let char_loaded = req.char_loaded.unwrap_or(!ch.name.is_empty());
    let rows = match &req.rows {
        Some(r) => r.iter().map(row_from_json).collect(),
        None => rows_from_stacks(world, &req.character, &ch),
    };
    let mut st = start_state(world, req, ch, rows, char_loaded);
    let (mut out, ask) = run_once(world, req, &mut st, &req.answers);
    let mut run = Run {
        st,
        out: std::mem::take(&mut out),
        ask_log: ask.log,
        questions: ask.questions,
        pending: ask.pending,
    };
    let mut base = req.character.clone();
    let mut events_prefix: Vec<EventJson> = Vec::new();
    let mut prompts_prefix: Vec<Prompt> = Vec::new();

    let clear_at = run
        .out
        .events
        .iter()
        .position(|e| matches!(e, PasteEvent::ClearCharacter));
    if let (Some(at), None) = (clear_at, run.pending) {
        // The answer that caused the reset is the "different class" question; the second run starts
        // from the blank tab, where the character is no longer loaded and it is not asked again.
        let q = run
            .questions
            .iter()
            .position(|&p| {
                run.ask_log[p]
                    .text
                    .starts_with("It appears that you are pasting")
            })
            .ok_or("internal: a reset without its question")?;
        let cut = run.questions[q] + 1;
        prompts_prefix = run.ask_log[..cut].to_vec();
        events_prefix = events_json(&run.out.events[..=at], &run.ask_log);
        let mut answers2 = req.answers.clone();
        if q < answers2.len() {
            answers2.remove(q);
        }
        base = blank_character(&req.character);
        let blank = base.to_character()?;
        let rows = run.st.rows.clone();
        let mut st2 = start_state(world, req, blank, rows, false);
        // the reset also puts the combos and what is in them back to empty
        st2.sel = [0; EQUIP_SLOTS];
        let (out2, ask2) = run_once(world, req, &mut st2, &answers2);
        run = Run {
            st: st2,
            out: out2,
            ask_log: ask2.log,
            questions: ask2.questions,
            pending: ask2.pending,
        };
    }

    let prompts: Vec<Prompt> = prompts_prefix
        .into_iter()
        .chain(run.ask_log.iter().cloned())
        .collect();
    let needs_answer = run.pending.map(|p| run.ask_log[p].clone()).map(|mut p| {
        p.answer = None;
        p
    });
    let events: Vec<EventJson> = events_prefix
        .into_iter()
        .chain(events_json(&run.out.events, &run.ask_log))
        .collect();

    if needs_answer.is_some() {
        return Ok(PasteCharacterResponse {
            character: req.character.clone(),
            events,
            prompts,
            needs_answer,
            sort_after: false,
            rows: Vec::new(),
            sim_ac: None,
            sim_dr: None,
        });
    }
    let mut rows = run.st.rows.clone();
    if run.out.sort_after {
        sort_by_value_desc(&mut rows);
    }
    let mut character = lossless_dto(&base, &run.st).padded();
    // the saved lists follow the Item Manager as SaveCharacter writes them
    character.set_saved_from_rows(&rows)?;
    Ok(PasteCharacterResponse {
        character,
        events,
        prompts,
        needs_answer: None,
        sort_after: run.out.sort_after,
        rows: rows.iter().map(row_to_json).collect(),
        sim_ac: run.out.sim_ac,
        sim_dr: run.out.sim_dr,
    })
}

pub async fn character_route(
    State(state): State<Arc<AppState>>,
    Body(req): Body<PasteCharacterRequest>,
) -> ApiResult {
    let loaded = state.current();
    json(&paste_character(&loaded.world, &req).map_err(bad)?)
}

// ---------- party paste ----------

/// The party dialog's boxes: index 0 is the average box, 1 to 6 the members.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct PartyFormJson {
    pub name: Vec<String>,
    pub ac: Vec<String>,
    pub dr: Vec<String>,
    pub mr: Vec<String>,
    pub dodge: Vec<String>,
    pub hitpoints: Vec<String>,
    pub rest_hp: Vec<String>,
    pub regen_hp: Vec<String>,
    pub accy: Vec<String>,
    pub heals: Vec<String>,
    pub dmg: Vec<String>,
    pub spell_dmg: Vec<String>,
    pub swings: Vec<String>,
    pub anti_magic: Vec<i16>,
    pub atk_last: Vec<bool>,
    pub atk_last_tag: Vec<String>,
    pub am_total: String,
    pub party_total: String,
}

fn strings(v: &[String]) -> [String; paste_party::SLOTS] {
    std::array::from_fn(|i| v.get(i).cloned().unwrap_or_default())
}

impl PartyFormJson {
    fn to_form(&self) -> PartyForm {
        PartyForm {
            name: strings(&self.name),
            ac: strings(&self.ac),
            dr: strings(&self.dr),
            mr: strings(&self.mr),
            dodge: strings(&self.dodge),
            hitpoints: strings(&self.hitpoints),
            rest_hp: strings(&self.rest_hp),
            regen_hp: strings(&self.regen_hp),
            accy: strings(&self.accy),
            heals: strings(&self.heals),
            dmg: strings(&self.dmg),
            spell_dmg: strings(&self.spell_dmg),
            swings: strings(&self.swings),
            anti_magic: std::array::from_fn(|i| self.anti_magic.get(i).copied().unwrap_or(0)),
            atk_last: std::array::from_fn(|i| self.atk_last.get(i).copied().unwrap_or(false)),
            atk_last_tag: strings(&self.atk_last_tag),
            am_total: self.am_total.clone(),
            party_total: self.party_total.clone(),
            hold_refresh: false,
        }
    }

    fn from_form(f: &PartyForm) -> Self {
        Self {
            name: f.name.to_vec(),
            ac: f.ac.to_vec(),
            dr: f.dr.to_vec(),
            mr: f.mr.to_vec(),
            dodge: f.dodge.to_vec(),
            hitpoints: f.hitpoints.to_vec(),
            rest_hp: f.rest_hp.to_vec(),
            regen_hp: f.regen_hp.to_vec(),
            accy: f.accy.to_vec(),
            heals: f.heals.to_vec(),
            dmg: f.dmg.to_vec(),
            spell_dmg: f.spell_dmg.to_vec(),
            swings: f.swings.to_vec(),
            anti_magic: f.anti_magic.to_vec(),
            atk_last: f.atk_last.to_vec(),
            atk_last_tag: f.atk_last_tag.to_vec(),
            am_total: f.am_total.clone(),
            party_total: f.party_total.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct PasteAttackPrompt {
    pub member: usize,
    pub who: String,
    pub default: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct PastePartyRequest {
    /// The pasted text. Empty with a `form` recounts the averages of an edited form.
    pub text: String,
    /// The dialog's boxes before the paste (`None` is an empty dialog).
    pub form: Option<PartyFormJson>,
    /// Answers to the attack prompts in the order MME asks them; `null` accepts the default.
    pub attack_answers: Vec<Option<String>>,
    /// The answer to "Continue to party screen anyway?"; `null` while it has not been asked.
    pub no_data_continue: Option<bool>,
    /// `bOnlyInGame` and `bInvenUse2ndWrist`; both default to on.
    pub only_in_game: Option<bool>,
    pub second_wrist: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct AttackAsk {
    pub member: usize,
    pub who: String,
    pub default: String,
}

#[derive(Debug, Serialize)]
pub struct PastePartyResponse {
    pub form: PartyFormJson,
    /// "canceled" (fewer than 10 characters), "no_data_exit" or "done".
    pub status: &'static str,
    pub messages: Vec<String>,
    /// Every attack prompt MME showed, in order.
    pub prompts: Vec<AttackAsk>,
    /// The next question without an answer.
    pub needs_answer: Option<PartyQuestion>,
    pub error: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PartyQuestion {
    /// "Continue to party screen anyway?" (default No).
    NoData { text: String },
    /// An attack prompt (an InputBox; an empty reply is Cancel).
    Attack {
        index: usize,
        member: usize,
        who: String,
        default: String,
    },
}

const NO_DATA: &str = "No matching data pasted. Continue to party screen anyway?";

fn class_list(world: &World) -> Vec<ListEntry> {
    std::iter::once(ListEntry {
        name: "Any".into(),
        number: 0,
    })
    .chain(world.classes.iter().map(|c| ListEntry {
        name: c.name.clone(),
        number: c.number,
    }))
    .collect()
}

fn race_list(world: &World) -> Vec<ListEntry> {
    std::iter::once(ListEntry {
        name: "Any".into(),
        number: 0,
    })
    .chain(world.races.iter().map(|r| ListEntry {
        name: r.name.clone(),
        number: r.number,
    }))
    .collect()
}

pub fn paste_party(world: &World, req: &PastePartyRequest) -> Result<PastePartyResponse, String> {
    if req.text.len() > MAX_TEXT {
        return Err(format!("the paste is longer than {MAX_TEXT} bytes"));
    }
    if req.attack_answers.len() > MAX_ANSWERS {
        return Err(format!("at most {MAX_ANSWERS} answers"));
    }
    let mut form = req
        .form
        .as_ref()
        .map(PartyFormJson::to_form)
        .unwrap_or_default();
    if req.text.is_empty() {
        // the boxes were edited by hand: MME recounts every column
        paste_party::calculate_average_party(&mut form, 0);
        return Ok(PastePartyResponse {
            form: PartyFormJson::from_form(&form),
            status: "done",
            messages: Vec::new(),
            prompts: Vec::new(),
            needs_answer: None,
            error: false,
        });
    }
    let classes = class_list(world);
    let races = race_list(world);
    let env = paste_party::PasteEnv {
        profile: Profile::Stock,
        items: &world.items,
        classes: &world.classes,
        races: &world.races,
        spells: &world.spells,
        class_list: &classes,
        race_list: &races,
        only_in_game: req.only_in_game.unwrap_or(true),
        use_second_wrist: req.second_wrist.unwrap_or(true),
        nmr_ver: nmr_ver(&world.info.nmr_version),
        avg_level_max_all_stats: compute_avg_level_max_stats(Profile::Stock, &world.races)
            .all_stats,
        dat_ver_caption: &world.info.dat_file_version,
        no_data_continue: req.no_data_continue.unwrap_or(false),
    };
    let input = PasteInput {
        text: &req.text,
        attack_answers: &req.attack_answers,
    };
    let out = paste_party::parse_paste_party(&env, &input, &mut form);
    let status = match out.status {
        PasteStatus::Canceled => "canceled",
        PasteStatus::NoDataExit => "no_data_exit",
        PasteStatus::Done => "done",
    };
    let needs_answer =
        if out.messages.iter().any(|m| m == NO_DATA) && req.no_data_continue.is_none() {
            Some(PartyQuestion::NoData {
                text: NO_DATA.to_owned(),
            })
        } else {
            out.prompts
                .get(req.attack_answers.len())
                .map(|p| PartyQuestion::Attack {
                    index: req.attack_answers.len(),
                    member: p.member,
                    who: p.who.clone(),
                    default: p.default.clone(),
                })
        };
    Ok(PastePartyResponse {
        form: PartyFormJson::from_form(&form),
        status,
        messages: out
            .messages
            .iter()
            .filter(|m| m.as_str() != NO_DATA)
            .cloned()
            .collect(),
        prompts: out
            .prompts
            .iter()
            .map(|p| AttackAsk {
                member: p.member,
                who: p.who.clone(),
                default: p.default.clone(),
            })
            .collect(),
        needs_answer,
        error: out.error,
    })
}

pub async fn party_route(
    State(state): State<Arc<AppState>>,
    Body(req): Body<PastePartyRequest>,
) -> ApiResult {
    let loaded = state.current();
    json(&paste_party(&loaded.world, &req).map_err(bad)?)
}

// ---------- party: Continue ----------

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct PartyContinueRequest {
    pub form: PartyFormJson,
    /// `frmMain.txtMonsterDamage`: 99999 means "off".
    pub current_damage: f64,
    /// `frmMain.txtMonsterDamageOUT(0)` and `(1)`.
    pub current_damage_out: [f64; 2],
    /// The reply to the "Update the [HEALS] field?" question (6 yes, 7 no, 2 cancel).
    pub answer: Option<i32>,
}

/// What `cmdContinue_Click` writes into the monster filter; a field is present when MME writes it.
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct PartyFilter {
    pub party_size: Option<String>,
    pub damage: Option<String>,
    pub ac: Option<String>,
    pub dr: Option<String>,
    pub mr: Option<String>,
    pub dodge: Option<String>,
    pub hitpoints: Option<String>,
    pub anti_magic: Option<String>,
    pub rest_hp: Option<String>,
    pub accy: Option<String>,
    pub swings: Option<String>,
    pub damage_out_phys: Option<String>,
    pub damage_out_spell: Option<String>,
}

#[derive(Debug, Serialize, Default)]
pub struct PartyContinueResponse {
    /// "applied", "canceled" (the question was answered Cancel), "needs_answer" or "info".
    pub status: &'static str,
    pub filter: Option<PartyFilter>,
    pub question: Option<Prompt>,
    pub message: Option<String>,
}

fn filled(s: &str) -> Option<String> {
    let t = s.trim_matches(' ');
    (!t.is_empty()).then(|| t.to_owned())
}

/// Port of frmPasteChar `cmdContinue_Click` for the party screen.
pub fn party_continue(
    world: &World,
    req: &PartyContinueRequest,
) -> Result<PartyContinueResponse, String> {
    let f = req.form.to_form();
    let nmr = nmr_ver(&world.info.nmr_version);
    if vb::val(&f.party_total) <= 1.0 {
        return Ok(PartyContinueResponse {
            status: "info",
            message: Some("Data only updated when party size > 1.".into()),
            ..Default::default()
        });
    }
    let mut filter = PartyFilter {
        party_size: Some(vb::cstr(vb::val(&f.party_total))),
        ..PartyFilter::default()
    };
    let regen_set = filled(&f.regen_hp[0]).is_some();
    let heals_set = filled(&f.heals[0]).is_some();
    if regen_set || heals_set {
        let mut update = true;
        let v6 =
            |r: Result<f64, vb::Vb6Error>| r.map_err(|e| format!("calculation error {}", e.code()));
        let healing_f = if nmr < 1.83 && !heals_set {
            v6(vb::round(vb::val(&f.regen_hp[0]) / 2.0, 0))?
                + v6(vb::round(vb::val(&f.rest_hp[0]) / 3.0, 0))?
        } else {
            v6(vb::round(vb::val(&f.regen_hp[0]) / 6.0, 0))? + vb::val(&f.heals[0])
        };
        let healing = vb::clng(healing_f).map_err(|e| format!("calculation error {}", e.code()))?;
        if healing > 0
            && (req.current_damage - 99999.0).abs() > f64::EPSILON
            && (!regen_set || !heals_set)
            && (req.current_damage - f64::from(healing)).abs() > f64::EPSILON
        {
            match req.answer {
                None => {
                    let temp = format!(
                        "Current value of {} would be overwritten to {}",
                        vb::cstr(vb::val(&vb::cstr(req.current_damage))),
                        healing
                    );
                    let (title, text) = if nmr < 1.83 {
                        (
                            "[DMG <=]",
                            "Either regen rate or healing spells not specified. These two fields are normally computed together to update the [DMG <=] field.\r\n\r\nThis is your sustainable damage IN/healing amount before requiring to rest. This is an older database and therefore does not scale this field with more damage. Instead, the filter will simply exclude mobs that deal more damage than this. The in-combat resting rate has been adjusted to compensate for this limitation.",
                        )
                    } else {
                        (
                            "[HEALS]",
                            "Either regen rate or healing spells not specified. These two fields are normally computed together to update the [HEALS] field.\r\n\r\nThis is your sustainable damage IN/healing amount before requiring to rest. If you have no additional healing, you probably want to answer yes.",
                        )
                    };
                    return Ok(PartyContinueResponse {
                        status: "needs_answer",
                        question: Some(Prompt {
                            kind: "yes_no_cancel",
                            text: format!("Update the {title} field? \r\n\r\n{text}\r\n\r\n{temp}"),
                            default: Answer::Button(VB_CANCEL),
                            answer: None,
                        }),
                        ..Default::default()
                    });
                }
                Some(VB_CANCEL) => {
                    return Ok(PartyContinueResponse {
                        status: "canceled",
                        ..Default::default()
                    });
                }
                Some(VB_YES) => {}
                Some(_) => update = false,
            }
        }
        if update && healing > 0 {
            filter.damage = Some(healing.to_string());
        }
    }
    filter.ac = filled(&f.ac[0]);
    filter.dr = filled(&f.dr[0]);
    filter.mr = filled(&f.mr[0]);
    filter.dodge = filled(&f.dodge[0]);
    filter.hitpoints = filled(&f.hitpoints[0]);
    filter.anti_magic = filled(&f.am_total);
    filter.rest_hp = filled(&f.rest_hp[0]);
    filter.accy = filled(&f.accy[0]);
    filter.swings = filled(&f.swings[0]);
    let dmg = filled(&f.dmg[0]);
    let spell = filled(&f.spell_dmg[0]);
    if let Some(d) = dmg.clone() {
        filter.damage_out_phys = Some(d);
        if spell.is_none() && req.current_damage_out[1] >= 9999.0 {
            filter.damage_out_spell = Some("0".into());
        }
    }
    if let Some(s) = spell.clone() {
        filter.damage_out_spell = Some(s);
        if dmg.is_none() && req.current_damage_out[0] >= 9999.0 {
            filter.damage_out_phys = Some("0".into());
        }
    }
    Ok(PartyContinueResponse {
        status: "applied",
        filter: Some(filter),
        ..Default::default()
    })
}

pub async fn party_continue_route(
    State(state): State<Arc<AppState>>,
    Body(req): Body<PartyContinueRequest>,
) -> ApiResult {
    let loaded = state.current();
    json(&party_continue(&loaded.world, &req).map_err(bad)?)
}
