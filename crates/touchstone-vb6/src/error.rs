use thiserror::Error;

/// A VB6 run-time error. The numbers are VB6's own, which the semantics table records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum Vb6Error {
    /// Error 5, for example `Sqr(-1)` or `Left("a", -1)`.
    #[error("invalid procedure call or argument (5)")]
    InvalidProcedureCall,
    /// Error 6: the result does not fit the target type.
    #[error("overflow (6)")]
    Overflow,
    /// Error 11: `/`, `\`, `Mod` or `^` with a zero divisor.
    #[error("division by zero (11)")]
    DivisionByZero,
}

impl Vb6Error {
    /// The VB6 `Err.Number`.
    #[must_use]
    pub fn code(self) -> u32 {
        match self {
            Self::InvalidProcedureCall => 5,
            Self::Overflow => 6,
            Self::DivisionByZero => 11,
        }
    }
}
