use crate::Vb6Error;

/// VB6 `Currency`: a 64-bit integer scaled by 10 000 (four decimals), range about
/// +-922 337 203 685 477.5807. Arithmetic is exact in the scaled integers; multiplication and
/// division round the exact result to four decimals half to even (VB6 documentation does not
/// say how ties break, so the table avoids tie cases).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Currency(i64);

const SCALE: i64 = 10_000;

impl Currency {
    /// Wrap an already scaled value (units of 0.0001).
    #[must_use]
    pub fn from_scaled(scaled: i64) -> Self {
        Self(scaled)
    }

    /// The scaled integer (units of 0.0001).
    #[must_use]
    pub fn scaled(self) -> i64 {
        self.0
    }

    /// `CCur(x)`: rounds to four decimals (half to even on the scaled binary value).
    ///
    /// # Errors
    /// [`Vb6Error::Overflow`] outside the Currency range.
    pub fn from_f64(x: f64) -> Result<Self, Vb6Error> {
        let r = (x * SCALE as f64).round_ties_even();
        if r.is_finite() && (-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&r)
        {
            Ok(Self(r as i64))
        } else {
            Err(Vb6Error::Overflow)
        }
    }

    /// The value as a Double, as VB6 does when a Currency meets a Double.
    #[must_use]
    pub fn to_f64(self) -> f64 {
        self.0 as f64 / SCALE as f64
    }

    /// `a + b`.
    ///
    /// # Errors
    /// [`Vb6Error::Overflow`].
    pub fn checked_add(self, o: Self) -> Result<Self, Vb6Error> {
        self.0.checked_add(o.0).map(Self).ok_or(Vb6Error::Overflow)
    }

    /// `a - b`.
    ///
    /// # Errors
    /// [`Vb6Error::Overflow`].
    pub fn checked_sub(self, o: Self) -> Result<Self, Vb6Error> {
        self.0.checked_sub(o.0).map(Self).ok_or(Vb6Error::Overflow)
    }

    /// `a * b`, rounded to four decimals.
    ///
    /// # Errors
    /// [`Vb6Error::Overflow`].
    pub fn checked_mul(self, o: Self) -> Result<Self, Vb6Error> {
        let p = i128::from(self.0) * i128::from(o.0);
        narrow(div_half_even(p, i128::from(SCALE)))
    }

    /// `a / b`, rounded to four decimals.
    ///
    /// # Errors
    /// Error 11 for a zero divisor, error 6 on overflow.
    pub fn checked_div(self, o: Self) -> Result<Self, Vb6Error> {
        if o.0 == 0 {
            return Err(Vb6Error::DivisionByZero);
        }
        narrow(div_half_even(
            i128::from(self.0) * i128::from(SCALE),
            i128::from(o.0),
        ))
    }
}

fn narrow(v: i128) -> Result<Currency, Vb6Error> {
    i64::try_from(v)
        .map(Currency)
        .map_err(|_| Vb6Error::Overflow)
}

fn div_half_even(n: i128, d: i128) -> i128 {
    let q = n / d;
    let r = n % d;
    let twice = r.abs() * 2;
    let step = if (n < 0) != (d < 0) { -1 } else { 1 };
    match twice.cmp(&d.abs()) {
        std::cmp::Ordering::Greater => q + step,
        std::cmp::Ordering::Equal if q % 2 != 0 => q + step,
        _ => q,
    }
}
