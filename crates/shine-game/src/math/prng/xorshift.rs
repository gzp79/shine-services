use super::StableRng;

/// Xorshift32 PRNG implementing StableRng.
/// Simple, fast, deterministic — suitable for cross-platform reproducible generation.
pub struct Xorshift32(u32);

impl Xorshift32 {
    pub fn new(seed: u32) -> Self {
        // Avoid zero state which would produce all zeros
        Self(if seed == 0 { 1 } else { seed })
    }
}

impl StableRng for Xorshift32 {
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
}
