/// `Val(s)`: reads the leading number and ignores the rest.
///
/// Leading whitespace and any spaces, tabs or line breaks inside the number are skipped, so
/// `Val("1 2 3")` is 123. A sign, digits, one decimal point and an `E`/`D` exponent are
/// accepted; anything else ends the number (`Val("1,000")` is 1) and no digits gives 0.
/// `&H` and `&O` literals are read as integers: without a trailing `&`, up to four hex digits
/// (six octal) are a 16-bit Integer, so `Val("&HFFFF")` is -1, otherwise a 32-bit Long.
/// Results beyond the Double range are not modelled.
#[must_use]
pub fn val(s: &str) -> f64 {
    let chars: Vec<char> = s.chars().filter(|c| !is_space(*c)).collect();
    if chars.first() == Some(&'&') {
        return radix_literal(&chars[1..]);
    }
    let mut i = 0;
    let mut out = String::new();
    if matches!(chars.first(), Some('+' | '-')) {
        if chars[0] == '-' {
            out.push('-');
        }
        i = 1;
    }
    let mut digits = 0;
    while i < chars.len() && chars[i].is_ascii_digit() {
        out.push(chars[i]);
        digits += 1;
        i += 1;
    }
    if i < chars.len() && chars[i] == '.' {
        out.push('.');
        i += 1;
        while i < chars.len() && chars[i].is_ascii_digit() {
            out.push(chars[i]);
            digits += 1;
            i += 1;
        }
    }
    if digits == 0 {
        return 0.0;
    }
    if i < chars.len() && matches!(chars[i], 'e' | 'E' | 'd' | 'D') {
        let mut j = i + 1;
        let mut exp = String::from("e");
        if j < chars.len() && matches!(chars[j], '+' | '-') {
            exp.push(chars[j]);
            j += 1;
        }
        let start = j;
        while j < chars.len() && chars[j].is_ascii_digit() {
            exp.push(chars[j]);
            j += 1;
        }
        if j > start {
            out.push_str(&exp);
        }
    }
    let v = out.parse::<f64>().unwrap_or(0.0);
    if v == 0.0 { 0.0 } else { v }
}

fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{a0}')
}

fn radix_literal(rest: &[char]) -> f64 {
    let (radix, max_short) = match rest.first() {
        Some('h' | 'H') => (16, 4),
        Some('o' | 'O') => (8, 6),
        _ => return 0.0,
    };
    let digits: Vec<u32> = rest[1..].iter().map_while(|c| c.to_digit(radix)).collect();
    if digits.is_empty() {
        return 0.0;
    }
    let after = rest.get(1 + digits.len());
    let long_suffix = after == Some(&'&');
    let value = digits
        .iter()
        .fold(0u64, |a, d| (a << radix.trailing_zeros()) | u64::from(*d));
    if long_suffix || digits.len() > max_short {
        let v = value as u32;
        f64::from(v as i32)
    } else {
        // Integer-typed literal: the top bit of 16 (or of 18 for octal 6 digits) is the sign.
        let v = value as u16;
        f64::from(v as i16)
    }
}
