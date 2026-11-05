//! `IsNumeric(String)` and `CDbl(String)`: one grammar, the one VB6's string-to-Double
//! conversion accepts under the US locale.

use crate::Vb6Error;

enum Fail {
    NotNumber,
    Overflow,
}

fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

/// Strips one leading or trailing sign (or the parentheses form) and a `$`, tracking the sign.
/// More than one sign is not a number.
fn strip_decorations(mut u: &str) -> Result<(bool, &str), Fail> {
    let (mut neg, mut signs) = (false, 0);
    let take_lead = |u: &mut &str, neg: &mut bool, signs: &mut u32| {
        if let Some(r) = u.strip_prefix(['+', '-']) {
            *neg = u.starts_with('-');
            *signs += 1;
            *u = r.trim_start_matches(' ');
        }
    };
    if u.len() >= 2 && u.starts_with('(') && u.ends_with(')') {
        neg = true;
        signs = 1;
        u = u[1..u.len() - 1].trim_matches(' ');
    }
    take_lead(&mut u, &mut neg, &mut signs);
    if let Some(r) = u.strip_prefix('$') {
        u = r.trim_start_matches(' ');
        take_lead(&mut u, &mut neg, &mut signs);
    }
    if let Some(r) = u.strip_suffix(['+', '-']) {
        neg = u.ends_with('-');
        signs += 1;
        u = r.trim_end_matches(' ');
    }
    if signs > 1 || u.is_empty() {
        return Err(Fail::NotNumber);
    }
    Ok((neg, u))
}

fn radix_literal(u: &str) -> Result<f64, Fail> {
    let hex = u.as_bytes()[1] == b'h';
    let mut d = &u[2..];
    let long_form = d.ends_with('&');
    if long_form {
        d = &d[..d.len() - 1];
    }
    let radix = if hex { 16 } else { 8 };
    if d.is_empty() || !d.chars().all(|c| c.is_digit(radix)) {
        return Err(Fail::NotNumber);
    }
    let sig = d.trim_start_matches('0');
    if sig.len() > 8 {
        return Err(Fail::Overflow);
    }
    let n = if sig.is_empty() {
        0
    } else {
        u64::from_str_radix(sig, radix).map_err(|_| Fail::Overflow)?
    };
    if n > 0xFFFF_FFFF {
        return Err(Fail::Overflow);
    }
    // Hex of up to four digits without the `&` suffix is a 16-bit Integer, so it wraps negative.
    let v = if hex && sig.len() <= 4 && !long_form {
        i64::from(n as u16 as i16)
    } else {
        i64::from(n as u32 as i32)
    };
    Ok(v as f64)
}

fn parse(s: &str) -> Result<f64, Fail> {
    let t = s
        .trim_matches(|c: char| matches!(c, ' ' | '\t' | '\r' | '\n' | '\x0b' | '\x0c' | '\u{a0}'));
    if t.is_empty() {
        return Err(Fail::NotNumber);
    }
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("&h") || lower.starts_with("&o") {
        return radix_literal(&lower);
    }
    let (neg, u) = strip_decorations(&lower)?;
    let c: Vec<char> = u.chars().collect();
    let mut i = 0;
    let mut mant = 0;
    let mut int_part = String::new();
    while i < c.len()
        && (is_digit(c[i]) || (c[i] == ',' && i > 0 && (is_digit(c[i - 1]) || c[i - 1] == ',')))
    {
        if c[i] != ',' {
            int_part.push(c[i]);
            mant += 1;
        }
        i += 1;
    }
    let mut frac = String::new();
    if i < c.len() && c[i] == '.' {
        i += 1;
        while i < c.len() && is_digit(c[i]) {
            frac.push(c[i]);
            i += 1;
        }
        mant += frac.len();
    }
    if mant == 0 {
        return Err(Fail::NotNumber);
    }
    let mut exp = String::new();
    if i < c.len() && (c[i] == 'e' || c[i] == 'd') {
        i += 1;
        exp.push('e');
        if i < c.len() && (c[i] == '+' || c[i] == '-') {
            exp.push(c[i]);
            i += 1;
        }
        let start = i;
        while i < c.len() && is_digit(c[i]) {
            exp.push(c[i]);
            i += 1;
        }
        if i == start {
            return Err(Fail::NotNumber);
        }
    }
    if i != c.len() {
        return Err(Fail::NotNumber);
    }
    let text = format!(
        "{}.{}{exp}",
        if int_part.is_empty() { "0" } else { &int_part },
        if frac.is_empty() { "0" } else { &frac }
    );
    let v: f64 = text.parse().map_err(|_| Fail::NotNumber)?;
    if v.is_infinite() {
        return Err(Fail::Overflow);
    }
    Ok(if neg { -v } else { v })
}

/// `IsNumeric(s)` for a String: true when `CDbl(s)` would succeed under the US locale.
///
/// Accepted: outer whitespace; one sign (leading, after a `$`, or trailing) or parentheses for a
/// negative; a leading `$`; digits with `,` separators in the integer part (grouping is not
/// checked); a decimal point; an `E`/`D` exponent; and `&H`/`&O` literals (no sign, at most 8
/// significant digits). Empty text, a lone point and values beyond the Double range are not
/// numeric.
#[must_use]
pub fn is_numeric(s: &str) -> bool {
    parse(s).is_ok()
}

/// `CDbl(s)` for a String: the value under the [`is_numeric`] grammar. Text that is not a number
/// is a type mismatch (13); a number beyond the range is an overflow (6).
///
/// # Errors
/// [`Vb6Error::TypeMismatch`] or [`Vb6Error::Overflow`] as above.
pub fn cdbl_str(s: &str) -> Result<f64, Vb6Error> {
    parse(s).map_err(|f| match f {
        Fail::NotNumber => Vb6Error::TypeMismatch,
        Fail::Overflow => Vb6Error::Overflow,
    })
}
