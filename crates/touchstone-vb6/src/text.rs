use crate::Vb6Error;

/// `CStr(x)` for a Double (also what `&` does with a number): 15 significant digits with
/// trailing zeros dropped, in exponent form (`1E-05`, `1E+15`) when the decimal exponent is
/// below -4 or at least 15. Negative zero prints as `0`.
#[must_use]
pub fn cstr(x: f64) -> String {
    if x == 0.0 {
        return "0".into();
    }
    let sci = format!("{:.14e}", x.abs());
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mant.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let sign = if x < 0.0 { "-" } else { "" };
    if !(-4..15).contains(&exp) {
        let (head, tail) = digits.split_at(1);
        let m = if tail.is_empty() {
            head.to_string()
        } else {
            format!("{head}.{tail}")
        };
        let es = if exp < 0 { '-' } else { '+' };
        return format!("{sign}{m}E{es}{:02}", exp.abs());
    }
    let n = digits.len() as i32;
    let body = if exp < 0 {
        format!("0.{}{}", "0".repeat((-exp - 1) as usize), digits)
    } else if n <= exp + 1 {
        format!("{}{}", digits, "0".repeat((exp + 1 - n) as usize))
    } else {
        let (i, f) = digits.split_at((exp + 1) as usize);
        format!("{i}.{f}")
    };
    format!("{sign}{body}")
}

/// `CStr(b)` for a Boolean (and `&` with one): `True` or `False`.
#[must_use]
pub fn cstr_bool(b: bool) -> &'static str {
    if b { "True" } else { "False" }
}

/// `Str(x)`: like [`cstr`] but non-negative numbers get a leading space for the sign.
#[must_use]
pub fn str_num(x: f64) -> String {
    let s = cstr(x);
    if s.starts_with('-') {
        s
    } else {
        format!(" {s}")
    }
}

/// `Len(s)` in characters.
#[must_use]
pub fn len(s: &str) -> i32 {
    s.chars().count() as i32
}

/// `Left(s, n)`; `n` larger than the string gives the whole string.
///
/// # Errors
/// Error 5 for negative `n`.
pub fn left(s: &str, n: i32) -> Result<String, Vb6Error> {
    let n = usize::try_from(n).map_err(|_| Vb6Error::InvalidProcedureCall)?;
    Ok(s.chars().take(n).collect())
}

/// `Right(s, n)`.
///
/// # Errors
/// Error 5 for negative `n`.
pub fn right(s: &str, n: i32) -> Result<String, Vb6Error> {
    let n = usize::try_from(n).map_err(|_| Vb6Error::InvalidProcedureCall)?;
    let total = s.chars().count();
    Ok(s.chars().skip(total.saturating_sub(n)).collect())
}

/// `Mid(s, start[, len])`, 1-based. A start past the end gives an empty string.
///
/// # Errors
/// Error 5 when `start < 1` or `len < 0`.
pub fn mid(s: &str, start: i32, length: Option<i32>) -> Result<String, Vb6Error> {
    if start < 1 {
        return Err(Vb6Error::InvalidProcedureCall);
    }
    let take = match length {
        Some(l) => usize::try_from(l).map_err(|_| Vb6Error::InvalidProcedureCall)?,
        None => usize::MAX,
    };
    Ok(s.chars().skip((start - 1) as usize).take(take).collect())
}
