//! Reader for the text-block scripts NMR parses: the text of a block, its link, and what its
//! lines point at or do. NMR's rules were learned from the stock `.mdb` (docs/data-gaps.md);
//! the clause words are the engine's keywords. Quirks are kept on purpose (parity over
//! correctness): a clause that starts with a number is a reference to that block, and the
//! percentage of a random line is the last word before its first colon.
use std::collections::HashMap;

/// One text block: the parts of a block joined in part order, and the link of its first part.
pub(super) struct Block {
    pub link: i32,
    pub text: String,
    /// Whether the text has an action keyword other than `message` and `addevil`. Blocks that
    /// a `testskill`, `checkspell` or link points at are scripts only when they do.
    pub acts: bool,
}

pub(super) type Blocks = HashMap<i32, Block>;

/// The clause keywords, in the engine's matching order.
const KEYWORDS: [&str; 42] = [
    "price",
    "cast",
    "giveability",
    "addability",
    "teleport",
    "removeability",
    "nomonsters",
    "monsters",
    "needmonster",
    "summon",
    "message",
    "takeitem",
    "addexp",
    "giveitem",
    "hideitem",
    "checkability",
    "class",
    "race",
    "goodaligned",
    "evilaligned",
    "minlevel",
    "maxlevel",
    "checkitem",
    "failitem",
    "text",
    "roomtext",
    "roomitem",
    "failroomitem",
    "clearitem",
    "addevil",
    "testability",
    "failability",
    "remoteaction",
    "random",
    "adddelay",
    "testskill",
    "checkskill",
    "givecoins",
    "test_tournament",
    "checkspell",
    "flag",
    "learnspell",
];

/// The first keyword a word starts with (case folded), if any.
fn keyword(word: &str) -> Option<&'static str> {
    let w = word.to_ascii_lowercase();
    KEYWORDS.iter().copied().find(|k| w.starts_with(k))
}

/// Whether the first word of a segment is a keyword.
fn starts_with_keyword(seg: &str) -> bool {
    seg.split_whitespace()
        .next()
        .is_some_and(|w| keyword(w).is_some())
}

/// Whether any clause of the text is a keyword other than `message` and `addevil`.
pub(super) fn has_action(text: &str) -> bool {
    text.split('\n')
        .flat_map(|l| l.split(':'))
        .filter_map(|seg| seg.split_whitespace().next())
        .filter_map(keyword)
        .any(|k| k != "message" && k != "addevil")
}

/// A signed whole number: an optional sign and ASCII digits, nothing else.
fn number(word: &str) -> Option<i64> {
    let digits = word.strip_prefix(['-', '+']).unwrap_or(word);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    word.parse().ok()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum LineKind {
    /// `pct:clauses`, one roll picks a line.
    Random,
    /// `pattern:clauses`, an ask or command pattern then clauses.
    Pattern,
    /// Clauses only.
    Clauses,
}

pub(super) struct Line<'a> {
    pub kind: LineKind,
    /// For a random line, the difference to the previous random line's number.
    pub pct: i64,
    pub clauses: Vec<&'a str>,
}

/// The lines of a block's text.
pub(super) fn lines(text: &str) -> Vec<Line<'_>> {
    let mut prev = 0i64;
    let mut out = Vec::new();
    for line in text.split('\n') {
        let segs: Vec<&str> = line.split(':').collect();
        let first = segs[0].trim();
        if segs.len() >= 2 && !starts_with_keyword(first) {
            let last = first.split_whitespace().last().and_then(|w| {
                // A percentage has no plus sign.
                if w.starts_with('+') { None } else { number(w) }
            });
            if let Some(n) = last {
                out.push(Line {
                    kind: LineKind::Random,
                    pct: n - prev,
                    clauses: segs[1..].to_vec(),
                });
                prev = n;
            } else {
                out.push(Line {
                    kind: LineKind::Pattern,
                    pct: 0,
                    clauses: segs[1..].to_vec(),
                });
            }
        } else {
            out.push(Line {
                kind: LineKind::Clauses,
                pct: 0,
                clauses: segs,
            });
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum RefKind {
    /// A block that is shown (`text n` or a bare number).
    Text,
    /// A block that is run (`testskill`, `checkspell`).
    Run,
    /// A random block (`random n`).
    Random,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Ref {
    pub kind: RefKind,
    pub target: i32,
    /// `checkspell`: the spell tested.
    pub spell: Option<i32>,
}

fn to_i32(n: i64) -> Option<i32> {
    i32::try_from(n).ok()
}

/// The block a clause points at.
pub(super) fn reference(clause: &str) -> Option<Ref> {
    let w: Vec<&str> = clause.split_whitespace().collect();
    let first = *w.first()?;
    let arg = |i: usize| w.get(i).and_then(|x| number(x)).and_then(to_i32);
    let r = |kind, target| {
        Some(Ref {
            kind,
            target,
            spell: None,
        })
    };
    if let Some(n) = number(first) {
        return to_i32(n).and_then(|t| r(RefKind::Text, t));
    }
    match first.to_ascii_lowercase().as_str() {
        "text" => arg(1).and_then(|t| r(RefKind::Text, t)),
        "random" => arg(1).and_then(|t| r(RefKind::Random, t)),
        "testskill" => arg(3).and_then(|t| r(RefKind::Run, t)),
        "checkspell" => arg(2).map(|t| Ref {
            kind: RefKind::Run,
            target: t,
            spell: arg(1),
        }),
        _ => None,
    }
}

/// What a clause does that NMR records.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Act {
    Summon,
    GiveItem,
    Cast,
    LearnSpell,
}

/// The act and its first number, for the clauses NMR keeps a list for.
pub(super) fn act(clause: &str) -> Option<(Act, i32)> {
    let mut w = clause.split_whitespace();
    let kind = match w.next()?.to_ascii_lowercase().as_str() {
        "summon" => Act::Summon,
        "giveitem" => Act::GiveItem,
        "cast" => Act::Cast,
        "learnspell" => Act::LearnSpell,
        _ => return None,
    };
    let n = number(w.next()?).and_then(to_i32)?;
    Some((kind, n))
}

/// The class number of a `class n` clause.
pub(super) fn class_of(clause: &str) -> Option<i32> {
    let mut w = clause.split_whitespace();
    if !w.next()?.eq_ignore_ascii_case("class") {
        return None;
    }
    number(w.next()?).and_then(to_i32)
}
