/// VB6's `Rnd`/`Randomize` generator: a 24-bit linear congruential generator,
/// `x = (x * 0x43FD43FD + 0xC39EC3) & 0xFFFFFF`, with `Rnd` returning `x / 2^24` as a Single.
///
/// A fresh generator has state `0x50000`, so the first `Rnd` is 0.7055475, exactly as in a VB6
/// program that never calls `Randomize`. MME calls `Randomize` with no argument (seeded from
/// `Timer`); to reproduce a run, construct with [`Rnd::new`] or [`Rnd::from_state`] and feed it
/// the same sequence.
///
/// Arguments to [`Rnd::next_with`]: positive behaves like no argument, `0` repeats the last
/// value, and a negative value reseeds from the Single's bit pattern (low 24 bits plus the top
/// byte) before advancing, so the same negative argument always yields the same number. The
/// negative-seed and [`Rnd::randomize`] details are not in the public VB6 documentation and are
/// marked low confidence in the semantics table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rnd {
    state: u32,
}

const MASK: u32 = 0x00FF_FFFF;

impl Default for Rnd {
    fn default() -> Self {
        Self::new()
    }
}

impl Rnd {
    /// The generator as VB6 starts it, before any `Randomize`.
    #[must_use]
    pub fn new() -> Self {
        Self { state: 0x5_0000 }
    }

    /// Start from an explicit 24-bit state (higher bits are dropped).
    #[must_use]
    pub fn from_state(state: u32) -> Self {
        Self {
            state: state & MASK,
        }
    }

    /// The current 24-bit state.
    #[must_use]
    pub fn state(&self) -> u32 {
        self.state
    }

    fn value(&self) -> f32 {
        // 24 significant bits fit an f32 exactly.
        self.state as f32 / 16_777_216.0
    }

    fn advance(&mut self) {
        self.state = ((u64::from(self.state) * 0x43FD_43FD + 0x00C3_9EC3) & u64::from(MASK)) as u32;
    }

    /// `Rnd` with no argument: advance and return the next value in `[0, 1)`.
    pub fn next_value(&mut self) -> f32 {
        self.advance();
        self.value()
    }

    /// `Rnd(n)`.
    pub fn next_with(&mut self, n: f32) -> f32 {
        if n == 0.0 {
            return self.value();
        }
        if n < 0.0 {
            let bits = n.to_bits();
            self.state = ((bits & MASK) + ((bits >> 24) & 0xFF)) & MASK;
        }
        self.next_value()
    }

    /// `Randomize n`: replaces the middle 16 bits of the state with the high half xor the low
    /// half of the Double's two 32-bit words xored together, keeping the low 8 bits.
    pub fn randomize(&mut self, n: f64) {
        let b = n.to_bits();
        let v = ((b >> 32) as u32) ^ (b as u32);
        let mixed = ((v >> 16) ^ v) & 0xFFFF;
        self.state = (self.state & 0xFF) | (mixed << 8);
    }

    /// The `Int((hi - lo + 1) * Rnd + lo)` idiom MME uses for random whole numbers, with the
    /// product and sum rounded to Single as VB6 does for Integer-by-Single arithmetic.
    pub fn random_number(&mut self, lo: i16, hi: i16) -> i32 {
        let span = f32::from(hi) - f32::from(lo) + 1.0;
        let t = span * self.next_value();
        let t = t + f32::from(lo);
        t.floor() as i32
    }
}
