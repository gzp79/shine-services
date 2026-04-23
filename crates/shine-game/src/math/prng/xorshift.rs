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

#[cfg(test)]
mod test {
    use super::*;
    use shine_test::test;

    #[test]
    fn test_xorshift32() {
        let expected = [
            3967631044, 1058470358, 3852825024, 943183310, 2470503849, 1900398643, 3225046610, 3086557162, 344369384,
            4074949765,
        ];

        let mut rng = Xorshift32::new(745541);
        for i in 0..10 {
            let val = rng.next_u32();
            assert_eq!(val, expected[i]);
        }
    }
}
