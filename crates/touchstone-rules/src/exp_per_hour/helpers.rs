//! Shared small helpers of modExpPerHour.

/// `SEC_PER_ROUND`: one combat round.
pub const SEC_PER_ROUND: f64 = 5.0;
/// `SEC_PER_REST_TICK`.
pub const SEC_PER_REST_TICK: f64 = 20.0;
/// `SEC_PER_REGEN_TICK`.
pub const SEC_PER_REGEN_TICK: f64 = 30.0;
/// `SEC_PER_MEDI_TICK`.
pub const SEC_PER_MEDI_TICK: f64 = 10.0;

/// Port of modExpPerHour `cephC_Ceil`: ceiling via Fix (truncation toward zero).
#[must_use]
pub fn ceph_c_ceil(value: f64) -> f64 {
    if value == value.trunc() {
        value
    } else if value > 0.0 {
        value.trunc() + 1.0
    } else {
        value.trunc()
    }
}

/// Port of modExpPerHour `cephB_Saturate`.
#[must_use]
pub fn ceph_b_saturate(x: f64) -> f64 {
    x.clamp(0.0, 1.0)
}

/// Port of modExpPerHour `cephB_SmoothStep`.
#[must_use]
pub fn ceph_b_smooth_step(edge0: f64, edge1: f64, x: f64) -> f64 {
    if edge0 == edge1 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ceph_b_saturate((x - edge0) / (edge1 - edge0));
    t * t * (3.0 - 2.0 * t)
}

/// Port of modExpPerHour `MinDbl`.
#[must_use]
pub fn min_dbl(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}

/// Port of modExpPerHour `MaxDbl`.
#[must_use]
pub fn max_dbl(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

/// Port of modExpPerHour `ClampDbl`.
#[must_use]
pub fn clamp_dbl(v: f64, lo: f64, hi: f64) -> f64 {
    max_dbl(lo, min_dbl(v, hi))
}

/// Port of modExpPerHour `SafeDiv`.
#[must_use]
pub fn safe_div(n: f64, d: f64, def: f64) -> f64 {
    if d == 0.0 { def } else { n / d }
}
