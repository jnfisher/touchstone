use crate::Vb6Error;

/// `Format(x, pattern)` for the numeric patterns MME uses: `0` and `#` digit placeholders, an
/// optional `,` thousands separator in the integer part and one `.` (`"0.0"`, `"0.00"`, `"0"`,
/// `"#,##0"`, `"#,##0.000"`, `"#,#.00"`, `"0.########"`).
///
/// Rounding is half away from zero (unlike `CInt`/`Round`), applied to the 15-significant-digit
/// decimal form of `x` rather than its binary value, so `Format(0.15, "0.0")` is `0.2`. `#` in
/// the integer part gives no leading zero (`.50`), and a `.` is kept even if every optional
/// decimal digit is dropped (`Format(3, "0.########")` is `3.`). A negative value that rounds
/// to all zeros is printed without a sign. Patterns with other characters are not supported.
///
/// # Errors
/// Error 5 for an unsupported pattern.
pub fn format(x: f64, pattern: &str) -> Result<String, Vb6Error> {
    let (int_pat, frac_pat) = match pattern.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (pattern, None),
    };
    let int_ok = int_pat.chars().all(|c| matches!(c, '0' | '#' | ','));
    let frac_ok = frac_pat.is_none_or(|f| f.chars().all(|c| matches!(c, '0' | '#')));
    if pattern.is_empty() || !int_ok || !frac_ok {
        return Err(Vb6Error::InvalidProcedureCall);
    }
    let min_int = int_pat.chars().filter(|c| *c == '0').count();
    let comma = int_pat.contains(',');
    let min_frac = frac_pat.map_or(0, |f| f.chars().filter(|c| *c == '0').count());
    let max_frac = frac_pat.map_or(0, str::len);

    let (mut int_digits, mut frac_digits) = decimal_parts(x.abs());
    round_half_up(&mut int_digits, &mut frac_digits, max_frac);
    while frac_digits.len() > min_frac && frac_digits.last() == Some(&b'0') {
        frac_digits.pop();
    }
    while frac_digits.len() < min_frac {
        frac_digits.push(b'0');
    }
    let int_trim: Vec<u8> = int_digits
        .iter()
        .copied()
        .skip_while(|d| *d == b'0')
        .collect();
    let nonzero = !int_trim.is_empty() || frac_digits.iter().any(|d| *d != b'0');
    let mut int_out = int_trim;
    while int_out.len() < min_int {
        int_out.insert(0, b'0');
    }
    let int_str = String::from_utf8(int_out).unwrap_or_default();
    let int_str = if comma { group(&int_str) } else { int_str };
    let mut out = String::new();
    if x < 0.0 && nonzero {
        out.push('-');
    }
    out.push_str(&int_str);
    if frac_pat.is_some() {
        out.push('.');
        out.push_str(&String::from_utf8(frac_digits).unwrap_or_default());
    }
    Ok(out)
}

/// Integer and fraction digits of `x` taken from its 15-significant-digit decimal form.
fn decimal_parts(x: f64) -> (Vec<u8>, Vec<u8>) {
    if x == 0.0 {
        return (b"0".to_vec(), Vec::new());
    }
    let sci = format!("{x:.14e}");
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let d: Vec<u8> = mant.bytes().filter(u8::is_ascii_digit).collect();
    if exp < 0 {
        let mut frac = vec![b'0'; (-exp - 1) as usize];
        frac.extend(d);
        (b"0".to_vec(), frac)
    } else {
        let n = exp as usize + 1;
        let mut int: Vec<u8> = d.iter().copied().take(n).collect();
        int.resize(n, b'0');
        (int, d.iter().copied().skip(n).collect())
    }
}

fn round_half_up(int: &mut Vec<u8>, frac: &mut Vec<u8>, places: usize) {
    if frac.len() <= places {
        return;
    }
    let up = frac[places] >= b'5';
    frac.truncate(places);
    if !up {
        return;
    }
    let mut i = frac.len();
    while i > 0 {
        i -= 1;
        if frac[i] == b'9' {
            frac[i] = b'0';
        } else {
            frac[i] += 1;
            return;
        }
    }
    let mut j = int.len();
    while j > 0 {
        j -= 1;
        if int[j] == b'9' {
            int[j] = b'0';
        } else {
            int[j] += 1;
            return;
        }
    }
    int.insert(0, b'1');
}

fn group(digits: &str) -> String {
    let n = digits.len();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (n - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}
