//! MegaMUD stat-line parser (FAM-22, frmSwingCalc). Card P3-39.
//!
//! The Swing calculator's "Paste MegaMUD stat line" reads three lines (`Hit:`, `Extra:`, `Crit:`)
//! from the clipboard text. Each line has a percentage (or a `---` / `N/A` placeholder) and an
//! average, in the old layout (`Hit: 43% Rng:11-58 Avg:34`) or the new one (`Hit: 43% 11-58 34`).
//!
//! Positions are characters; `Trim$` strips spaces only, so a tab survives it and ends a token.

use crate::game_text::{instr_ci, lcase, left, len, mid_from, right, trim};
use touchstone_vb6::{Vb6Error, clng, cstr, val};

/// What `ParseMegaStatLine` leaves in its ByRef outputs, and its Boolean result.
///
/// The outputs are kept even when `ok` is false: `Extra:` is parsed without looking at the result,
/// so a line that is rejected after its percentage was read still contributes that percentage.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MegaStatLine {
    pub ok: bool,
    pub pct: f64,
    pub avg: i32,
}

/// Port of frmSwingCalc `CleanMegaNumber`: trims and drops thousands separators.
#[must_use]
pub fn clean_mega_number(value: &str) -> String {
    trim(value).replace(',', "")
}

/// Port of frmSwingCalc `IsMegaPlaceholder`: `---` or `N/A`, any case.
#[must_use]
pub fn is_mega_placeholder(value: &str) -> bool {
    let v = trim(value);
    v == "---" || v.eq_ignore_ascii_case("N/A")
}

/// Port of frmSwingCalc `IsMegaWholeNumberToken`: digits only after cleaning, and not empty.
#[must_use]
pub fn is_mega_whole_number_token(value: &str) -> bool {
    let v = clean_mega_number(value);
    !v.is_empty() && v.chars().all(|c| c.is_ascii_digit())
}

fn is_gap(c: char) -> bool {
    c == ' ' || c == '\t'
}

/// Port of frmSwingCalc `FirstMegaToken`: text up to the first space or tab.
#[must_use]
pub fn first_mega_token(value: &str) -> String {
    let v = trim(value);
    match v.chars().position(is_gap) {
        Some(i) => v.chars().take(i).collect(),
        None => v.to_string(),
    }
}

/// Port of frmSwingCalc `LastMegaToken`: text after the last space or tab (trimmed of spaces).
#[must_use]
pub fn last_mega_token(value: &str) -> String {
    let v = trim(value);
    let chars: Vec<char> = v.chars().collect();
    match chars.iter().rposition(|&c| is_gap(c)) {
        Some(i) => trim(&chars[i + 1..].iter().collect::<String>()).to_string(),
        None => v.to_string(),
    }
}

/// Port of frmSwingCalc `GetMegaStatLine`: the first line (CRLF, CR or LF separated, trimmed)
/// that starts with `label`, ignoring case; empty when there is none.
#[must_use]
pub fn get_mega_stat_line(text: &str, label: &str) -> String {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let want = lcase(label);
    let n = len(label);
    for raw in text.split('\n') {
        let line = trim(raw);
        if lcase(&left(line, n)) == want {
            return line.to_string();
        }
    }
    String::new()
}

/// Port of frmSwingCalc `ParseMegaStatLine`.
///
/// # Errors
/// Overflow (6) when the average does not fit a Long; MME has no handler there, so the error
/// reaches `cmdPasteMega_Click`'s.
pub fn parse_mega_stat_line(text: &str, label: &str) -> Result<MegaStatLine, Vb6Error> {
    let mut out = MegaStatLine::default();
    let line = get_mega_stat_line(text, label);
    if line.is_empty() {
        return Ok(out);
    }
    let rest = trim(&mid_from(&line, len(label) + 1)).to_string();
    if rest.is_empty() {
        return Ok(out);
    }
    let first = first_mega_token(&rest);

    if right(&first, 1) == "%" {
        out.pct = val(&clean_mega_number(&first));
    } else if !is_mega_placeholder(&first) {
        return Ok(out);
    }

    let avg_pos = instr_ci(&rest, "Avg:");
    let avg_token = if avg_pos > 0 {
        // old layout: the token right after "Avg:"
        first_mega_token(&mid_from(&rest, avg_pos + 4))
    } else {
        // new layout: the last token; a one-token line like "Hit: 43%" is not accepted
        let last = last_mega_token(&rest);
        if last == first {
            return Ok(out);
        }
        last
    };

    if !is_mega_whole_number_token(&avg_token) {
        return Ok(out);
    }
    out.avg = clng(val(&clean_mega_number(&avg_token)))?;
    out.ok = true;
    Ok(out)
}

/// The six text boxes `cmdPasteMega_Click` fills: Hit %, Hit avg, Extra %, Extra avg, Crit %,
/// Crit avg, each as `CStr` of the parsed number.
pub type MegaPaste = [String; 6];

/// Port of frmSwingCalc `cmdPasteMega_Click` without the clipboard and message box: `None` is
/// MME's "Incomplete/Missing MegaMUD Statistics in Clipboard" message (nothing is changed).
///
/// # Errors
/// A VB6 error from [`parse_mega_stat_line`]; MME's handler reports it and changes nothing.
pub fn paste_mega_stat_line(clip: &str) -> Result<Option<MegaPaste>, Vb6Error> {
    if trim(clip).is_empty() {
        return Ok(None);
    }
    let hit = parse_mega_stat_line(clip, "Hit:")?;
    if !hit.ok {
        return Ok(None);
    }
    // optional line: whatever it read stays, parsed or not
    let extra = parse_mega_stat_line(clip, "Extra:")?;
    let crit = parse_mega_stat_line(clip, "Crit:")?;
    if !crit.ok {
        return Ok(None);
    }
    Ok(Some([
        cstr(hit.pct),
        cstr(f64::from(hit.avg)),
        cstr(extra.pct),
        cstr(f64::from(extra.avg)),
        cstr(crit.pct),
        cstr(f64::from(crit.avg)),
    ]))
}
