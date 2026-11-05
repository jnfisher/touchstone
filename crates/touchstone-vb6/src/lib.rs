//! VB6 numeric and string semantics (CInt banker's rounding, Fix/Int, `\`, Mod, Integer/Long
//! overflow, Val, Round) so ported formulas match MME exactly. No IO.
//!
//! The behaviours are pinned by `fixtures/vb6-semantics.json`, which the oracle and this crate
//! are both tested against. Run-time errors are returned as [`Vb6Error`] carrying VB6's own
//! error numbers (5, 6, 11).
//!
//! Types MME's formula modules lean on: `Integer` (`i16`), `Long` (`i32`), `Double` (`f64`) and
//! `Currency` ([`Currency`]); `Single` is rare but modelled with `f32`. The functions used are
//! mostly `Fix`, `Round`, `\`, `Val`, `IIf`, `CLng`, `Format` and string slicing.

mod currency;
mod error;
mod format;
mod num;
mod numparse;
mod rnd;
mod text;
mod val;

pub use currency::Currency;
pub use error::Vb6Error;
pub use format::format;
pub use num::{
    FALSE, TRUE, abs, add_i16, add_i32, and, cbool, cint, clng, csng, div, exp, fix, idiv,
    idiv_i16, iif, int, log, mul_i16, mul_i32, neg_i16, not, or, pow, round, round_half_even, sgn,
    sqr, sub_i16, sub_i32, vbool, vmod, xor,
};
pub use numparse::{cdbl_str, is_numeric};
pub use rnd::Rnd;
pub use text::{cstr, cstr_bool, left, len, mid, right, str_num};
pub use val::val;
