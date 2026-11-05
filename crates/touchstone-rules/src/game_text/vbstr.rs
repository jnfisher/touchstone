//! The VB6 string primitives the text parsers lean on, on `&str` with character positions
//! (1-based like `InStr`). Case mapping is per character so lengths never change; `Trim`
//! strips spaces only, as VB6 does.

/// `Trim$`: removes leading and trailing spaces (U+0020 only; tabs stay).
#[must_use]
pub fn trim(s: &str) -> &str {
    s.trim_matches(' ')
}

/// `RTrim$`.
#[must_use]
pub fn rtrim(s: &str) -> &str {
    s.trim_end_matches(' ')
}

/// `LCase$`, one character in, one character out.
#[must_use]
pub fn lcase(s: &str) -> String {
    s.chars()
        .map(|c| {
            let mut it = c.to_lowercase();
            match (it.next(), it.next()) {
                (Some(l), None) => l,
                _ => c,
            }
        })
        .collect()
}

/// `Len`, in characters.
#[must_use]
pub fn len(s: &str) -> usize {
    s.chars().count()
}

/// `Left$(s, n)` for `n >= 0`; a longer `n` returns the whole string.
#[must_use]
pub fn left(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// `Right$(s, n)` for `n >= 0`; a longer `n` returns the whole string.
#[must_use]
pub fn right(s: &str, n: usize) -> String {
    let l = len(s);
    s.chars().skip(l.saturating_sub(n)).collect()
}

/// `Mid$(s, start)` (1-based start, to the end).
#[must_use]
pub fn mid_from(s: &str, start: usize) -> String {
    s.chars().skip(start.saturating_sub(1)).collect()
}

/// `Mid$(s, start, length)`.
#[must_use]
pub fn mid(s: &str, start: usize, length: usize) -> String {
    s.chars()
        .skip(start.saturating_sub(1))
        .take(length)
        .collect()
}

/// `InStr(1, s, find)`: 1-based character position of the first match, 0 when absent.
#[must_use]
pub fn instr(s: &str, find: &str) -> usize {
    match s.find(find) {
        Some(b) => s[..b].chars().count() + 1,
        None => 0,
    }
}

/// `InStr(1, s, find, vbTextCompare)`: the same, ignoring case.
#[must_use]
pub fn instr_ci(s: &str, find: &str) -> usize {
    instr(&lcase(s), &lcase(find))
}

/// `InStrRev(s, find, start)`: 1-based position of the last match that begins at or before
/// `start` (default: the end), 0 when absent.
#[must_use]
pub fn instrrev(s: &str, find: &str, start: Option<usize>) -> usize {
    let chars: Vec<char> = s.chars().collect();
    let f: Vec<char> = find.chars().collect();
    if f.is_empty() || f.len() > chars.len() {
        return 0;
    }
    let last_start = chars.len() - f.len() + 1;
    let top = start.map_or(last_start, |st| st.min(last_start));
    (1..=top)
        .rev()
        .find(|&p| chars[p - 1..p - 1 + f.len()] == f[..])
        .unwrap_or(0)
}
