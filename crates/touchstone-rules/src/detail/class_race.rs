//! Class and race detail text (modMain `PullClassDetail`, `PullRaceDetail`).

use touchstone_model::{AbilitySlot, World};

use super::{DetailKind, DetailRow};
use crate::ability_text::AbilityEnv;

/// What the class or race text box shows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClassRaceDetail {
    /// `DetailTB.Text`: the abilities joined by ", ", `"Class not found"`, or empty.
    pub text: String,
    pub rows: Vec<DetailRow>,
}

fn abilities(
    env: &AbilityEnv,
    slots: &[AbilitySlot],
    skip_class_ok: bool,
) -> Option<ClassRaceDetail> {
    let mut text = String::new();
    let mut rows = Vec::new();
    for s in slots {
        if s.ability == 0 || (skip_class_ok && s.ability == 59) {
            continue;
        }
        // An Integer conversion error ends the procedure before the text box is assigned.
        let a = i16::try_from(s.ability).ok()?;
        let v = i16::try_from(s.value).ok()?;
        if !text.is_empty() {
            text.push_str(", ");
        }
        let piece = env.get_ability_stats(a, v, true, false);
        text.push_str(&piece);
        if text.ends_with(", ") {
            text.truncate(text.len() - 2);
        }
        if !piece.is_empty() {
            rows.push(DetailRow::new("Ability", piece, DetailKind::Ability));
        }
    }
    Some(ClassRaceDetail { text, rows })
}

fn message(text: &str) -> ClassRaceDetail {
    ClassRaceDetail {
        text: text.to_string(),
        rows: vec![DetailRow::new("", text, DetailKind::Message)],
    }
}

/// Port of modMain `PullClassDetail`: the class's abilities except "class ok" (59). `startup` is
/// `bStartup`.
#[must_use]
pub fn pull_class_detail(
    env: &AbilityEnv,
    world: &World,
    number: i32,
    startup: bool,
) -> ClassRaceDetail {
    if startup {
        return ClassRaceDetail::default();
    }
    let Some(c) = world.get_class(number) else {
        return message("Class not found");
    };
    abilities(env, &c.abilities, true).unwrap_or_default()
}

/// Port of modMain `PullRaceDetail`: the race's abilities.
#[must_use]
pub fn pull_race_detail(
    env: &AbilityEnv,
    world: &World,
    number: i32,
    startup: bool,
) -> ClassRaceDetail {
    if startup {
        return ClassRaceDetail::default();
    }
    let Some(r) = world.get_race(number) else {
        return message("Race not found");
    };
    abilities(env, &r.abilities, false).unwrap_or_default()
}
