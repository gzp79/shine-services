use crate::math::prng::StableRng;
use rand::RngExt;

/// A wrapper around `rand::rng()` for system-provided randomness.
/// This is not stable/deterministic across versions or platforms, but useful for testing or initialization.
pub struct SysRng(rand::rngs::ThreadRng);

impl SysRng {
    pub fn new() -> Self {
        Self(rand::rng())
    }
}

impl Default for SysRng {
    fn default() -> Self {
        Self::new()
    }
}

impl StableRng for SysRng {
    fn next_u32(&mut self) -> u32 {
        self.0.random()
    }
}
