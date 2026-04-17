use crate::math::prng::{fnv1a64, Pcg32};

/// Never ever change this value, otherwise all existing data will be scrambled.
pub const MASTER_SEED: u64 = 0xd39a7b1e4f2c906a;

/// A fast 64-bit PRNG based on the SplitMix64 algorithm.
pub struct SplitMix64 {
    seed: u64,
    state: u64,
}

impl SplitMix64 {
    pub fn new() -> Self {
        Self::from_seed(MASTER_SEED)
    }

    pub fn from_seed(seed: u64) -> Self {
        let mut mix = Self { seed, state: seed };
        // call next to avoid starting with a low-entropy state.
        mix.next();
        mix
    }

    pub fn create_seed(&self, seq: u64) -> Self {
        debug_assert_ne!(seq, 0);
        let seed = self.seed ^ seq;
        Self::from_seed(seed)
    }

    pub fn create_seed_for_domain(&self, domain: &str) -> Self {
        Self::create_seed(&self, fnv1a64(domain))
    }

    /// Create a new independent random stream from the current state.
    /// Streams should be created in a consistent order to ensure deterministic results.
    pub fn next_stream(&mut self) -> Pcg32 {
        let seed = self.next();
        let seq = self.next() | 1;
        Pcg32::new(seed, seq)
    }

    pub fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
}

impl Default for SplitMix64 {
    fn default() -> Self {
        Self::new()
    }
}
