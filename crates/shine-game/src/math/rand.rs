/// Minimal RNG trait for deterministic, cross-platform random generation.
/// Uses only `next_u32` to produce random values, avoiding platform-specific
/// float conversion or library API differences.
pub trait StableRng {
    fn next_u32(&mut self) -> u32;
}

/// Extension methods for converting RNG output to floats.
pub trait StableRngExt: StableRng {
    /// Returns a deterministic float in [0, 1) with uniform spacing.
    fn float_unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / (1u32 << 24) as f32)
    }

    /// Returns a deterministic float in [-1, 1).
    fn float_signed(&mut self) -> f32 {
        self.float_unit() * 2.0 - 1.0
    }

    /// Returns a deterministic float in [min, max).
    fn float_range(&mut self, min: f32, max: f32) -> f32 {
        min + self.float_unit() * (max - min)
    }
}

impl<T: StableRng + ?Sized> StableRngExt for T {}
