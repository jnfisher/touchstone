use crate::Vb6Error;

type Res<T> = Result<T, Vb6Error>;

/// VB6 `True`: all bits set, so it is -1 as an Integer.
pub const TRUE: i16 = -1;
/// VB6 `False`.
pub const FALSE: i16 = 0;

/// A Boolean as VB6 stores it: `True` is -1, `False` is 0. Comparisons yield these values.
#[must_use]
pub fn vbool(b: bool) -> i16 {
    if b { TRUE } else { FALSE }
}

/// `CBool(x)`: any non-zero number is `True` (-1).
#[must_use]
pub fn cbool(x: f64) -> i16 {
    vbool(x != 0.0)
}

/// `IIf(cond, a, b)`. Unlike VB6 both branches are already evaluated by the caller, which is
/// also true in VB6, so side effects match.
#[must_use]
pub fn iif<T>(cond: bool, a: T, b: T) -> T {
    if cond { a } else { b }
}

/// Banker's rounding (round half to even), the rule behind `CInt`, `CLng`, implicit
/// Double-to-integer assignment, `\`, `Mod` and `Round`: 0.5 -> 0, 1.5 -> 2, 2.5 -> 2, -2.5 -> -2.
#[must_use]
pub fn round_half_even(x: f64) -> f64 {
    x.round_ties_even()
}

/// `CInt(x)`: round half to even, error 6 if the result is outside -32768..=32767. So
/// `CInt(32767.5)` is an overflow (it rounds to 32768) while `CInt(32766.5)` is 32766.
///
/// # Errors
/// [`Vb6Error::Overflow`] when out of range.
pub fn cint(x: f64) -> Res<i16> {
    let r = round_half_even(x);
    if (-32768.0..=32767.0).contains(&r) {
        Ok(r as i16)
    } else {
        Err(Vb6Error::Overflow)
    }
}

/// `CLng(x)`: round half to even, error 6 outside the 32-bit range. Implicit assignment of a
/// Double to an Integer or Long variable uses the same rule, so use [`cint`]/[`clng`] there too.
///
/// # Errors
/// [`Vb6Error::Overflow`] when out of range.
pub fn clng(x: f64) -> Res<i32> {
    let r = round_half_even(x);
    if (-2_147_483_648.0..=2_147_483_647.0).contains(&r) {
        Ok(r as i32)
    } else {
        Err(Vb6Error::Overflow)
    }
}

/// `Fix(x)`: truncate toward zero (`Fix(-2.5)` = -2). Returns a Double like VB6 does for Double input.
#[must_use]
pub fn fix(x: f64) -> f64 {
    let t = x.trunc();
    if t == 0.0 { 0.0 } else { t }
}

/// `Int(x)`: floor (`Int(-2.5)` = -3, unlike [`fix`]).
#[must_use]
pub fn int(x: f64) -> f64 {
    x.floor()
}

/// `Abs(x)`.
#[must_use]
pub fn abs(x: f64) -> f64 {
    x.abs()
}

/// `Sgn(x)`: -1, 0 or 1 as an Integer.
#[must_use]
pub fn sgn(x: f64) -> i16 {
    if x > 0.0 {
        1
    } else if x < 0.0 {
        -1
    } else {
        0
    }
}

/// `a \ b`. Both operands are first rounded to Long with banker's rounding (so `5.5 \ 2` is
/// `6 \ 2` = 3), then the quotient is truncated toward zero.
///
/// # Errors
/// Error 6 if an operand is outside Long or the quotient overflows, error 11 if `b` rounds to 0.
pub fn idiv(a: f64, b: f64) -> Res<i32> {
    let (a, b) = (clng(a)?, clng(b)?);
    if b == 0 {
        return Err(Vb6Error::DivisionByZero);
    }
    a.checked_div(b).ok_or(Vb6Error::Overflow)
}

/// `a \ b` for two Integer operands: the result is an Integer, so `-32768 \ -1` overflows.
///
/// # Errors
/// Error 11 for a zero divisor, error 6 for `-32768 \ -1`.
pub fn idiv_i16(a: i16, b: i16) -> Res<i16> {
    if b == 0 {
        return Err(Vb6Error::DivisionByZero);
    }
    a.checked_div(b).ok_or(Vb6Error::Overflow)
}

/// `a Mod b`. Operands are rounded to Long like [`idiv`]; the result has the sign of the
/// dividend (`-7 Mod 3` = -1, `7 Mod -3` = 1).
///
/// # Errors
/// Error 6 for an out-of-range operand, error 11 if `b` rounds to 0.
pub fn vmod(a: f64, b: f64) -> Res<i32> {
    let (a, b) = (clng(a)?, clng(b)?);
    if b == 0 {
        return Err(Vb6Error::DivisionByZero);
    }
    a.checked_rem(b).ok_or(Vb6Error::Overflow)
}

/// `Round(x, digits)`: banker's rounding to `digits` decimals (`Round(2.5)` = 2,
/// `Round(1.25, 1)` = 1.2). The value is scaled by a power of ten, rounded half to even and
/// scaled back, so ties on decimals that are not exact in binary (2.675) follow the scaled
/// binary value; the semantics table only pins exactly representable ties.
///
/// # Errors
/// Error 5 for negative `digits`.
pub fn round(x: f64, digits: i32) -> Res<f64> {
    if digits < 0 {
        return Err(Vb6Error::InvalidProcedureCall);
    }
    if digits == 0 {
        return Ok(round_half_even(x));
    }
    let scale = 10f64.powi(digits);
    let scaled = x * scale;
    if !scaled.is_finite() {
        return Ok(x);
    }
    Ok(round_half_even(scaled) / scale)
}

/// Integer `a + b`; the sum is an Integer, so 32767 + 1 is error 6.
///
/// # Errors
/// [`Vb6Error::Overflow`].
pub fn add_i16(a: i16, b: i16) -> Res<i16> {
    a.checked_add(b).ok_or(Vb6Error::Overflow)
}

/// Integer `a - b`.
///
/// # Errors
/// [`Vb6Error::Overflow`].
pub fn sub_i16(a: i16, b: i16) -> Res<i16> {
    a.checked_sub(b).ok_or(Vb6Error::Overflow)
}

/// Integer `a * b`. The product is an Integer even when it is assigned to a Long or sits in a
/// larger expression, so `200 * 200` fails while `200 * 200&` (a Long operand) does not.
///
/// # Errors
/// [`Vb6Error::Overflow`].
pub fn mul_i16(a: i16, b: i16) -> Res<i16> {
    a.checked_mul(b).ok_or(Vb6Error::Overflow)
}

/// Integer negation; `-(-32768)` overflows.
///
/// # Errors
/// [`Vb6Error::Overflow`].
pub fn neg_i16(a: i16) -> Res<i16> {
    a.checked_neg().ok_or(Vb6Error::Overflow)
}

/// Long `a + b`.
///
/// # Errors
/// [`Vb6Error::Overflow`] past 2^31.
pub fn add_i32(a: i32, b: i32) -> Res<i32> {
    a.checked_add(b).ok_or(Vb6Error::Overflow)
}

/// Long `a - b`.
///
/// # Errors
/// [`Vb6Error::Overflow`].
pub fn sub_i32(a: i32, b: i32) -> Res<i32> {
    a.checked_sub(b).ok_or(Vb6Error::Overflow)
}

/// Long `a * b`.
///
/// # Errors
/// [`Vb6Error::Overflow`].
pub fn mul_i32(a: i32, b: i32) -> Res<i32> {
    a.checked_mul(b).ok_or(Vb6Error::Overflow)
}

/// `a / b`: always a Double, never an integer quotient.
///
/// # Errors
/// Error 11 when `b` is zero. (`0 / 0` is not pinned by the table.)
pub fn div(a: f64, b: f64) -> Res<f64> {
    if b == 0.0 {
        return Err(Vb6Error::DivisionByZero);
    }
    finite(a / b)
}

/// `a ^ b` as a Double. `0 ^ 0` is 1, `0 ^ negative` is error 11 and a negative base with a
/// non-integer exponent is error 5.
///
/// # Errors
/// See above, plus error 6 on overflow.
pub fn pow(a: f64, b: f64) -> Res<f64> {
    if a == 0.0 && b < 0.0 {
        return Err(Vb6Error::DivisionByZero);
    }
    if a < 0.0 && b.fract() != 0.0 {
        return Err(Vb6Error::InvalidProcedureCall);
    }
    finite(a.powf(b))
}

/// `Sqr(x)`.
///
/// # Errors
/// Error 5 for negative `x`.
pub fn sqr(x: f64) -> Res<f64> {
    if x < 0.0 {
        return Err(Vb6Error::InvalidProcedureCall);
    }
    Ok(x.sqrt())
}

/// `Log(x)`, the natural logarithm.
///
/// # Errors
/// Error 5 for `x <= 0`.
pub fn log(x: f64) -> Res<f64> {
    if x <= 0.0 {
        return Err(Vb6Error::InvalidProcedureCall);
    }
    Ok(x.ln())
}

/// `Exp(x)`.
///
/// # Errors
/// Error 6 when the result exceeds the Double range (x above about 709.78).
pub fn exp(x: f64) -> Res<f64> {
    finite(x.exp())
}

/// `CSng(x)`: nearest Single; error 6 beyond the Single range. Single arithmetic is plain `f32`
/// arithmetic in Rust, which matches VB6 for Single-by-Single operations.
///
/// # Errors
/// [`Vb6Error::Overflow`].
pub fn csng(x: f64) -> Res<f32> {
    let s = x as f32;
    if s.is_finite() {
        Ok(s)
    } else {
        Err(Vb6Error::Overflow)
    }
}

fn finite(x: f64) -> Res<f64> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(Vb6Error::Overflow)
    }
}

/// Bitwise `And`. Double operands are rounded to Long first (`5.5 And 3` is `6 And 3`).
///
/// # Errors
/// Error 6 if an operand is outside Long.
pub fn and(a: f64, b: f64) -> Res<i32> {
    Ok(clng(a)? & clng(b)?)
}

/// Bitwise `Or`; operands rounded like [`and`].
///
/// # Errors
/// Error 6 if an operand is outside Long.
pub fn or(a: f64, b: f64) -> Res<i32> {
    Ok(clng(a)? | clng(b)?)
}

/// Bitwise `Xor`; operands rounded like [`and`].
///
/// # Errors
/// Error 6 if an operand is outside Long.
pub fn xor(a: f64, b: f64) -> Res<i32> {
    Ok(clng(a)? ^ clng(b)?)
}

/// Bitwise `Not` (`Not 5` = -6). Booleans work because `True` is all ones. Integer-range
/// inputs give the same value as a 16-bit complement because of sign extension.
///
/// # Errors
/// Error 6 if the operand is outside Long.
pub fn not(a: f64) -> Res<i32> {
    Ok(!clng(a)?)
}
