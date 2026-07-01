use crate::math::prng::StableRng;

/// RNG that replays values from a slice, cycling when exhausted.
/// Useful for fuzzing or replaying specific sequences.
pub struct SequenceRng {
    values: Vec<u32>,
    idx: usize,
}

impl SequenceRng {
    pub fn new(values: Vec<u32>) -> Self {
        Self { values, idx: 0 }
    }

    /// Create from a byte slice, interpreting every 4 bytes as a u32 (little-endian).
    /// Partial trailing chunks are padded with zeros.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let values: Vec<u32> = bytes
            .chunks(4)
            .map(|chunk| {
                let mut buf = [0u8; 4];
                buf[..chunk.len()].copy_from_slice(chunk);
                u32::from_le_bytes(buf)
            })
            .collect();
        Self::new(values)
    }
}

impl StableRng for SequenceRng {
    fn next_u32(&mut self) -> u32 {
        if self.values.is_empty() {
            return 0;
        }
        let val = self.values[self.idx % self.values.len()];
        self.idx = self.idx.wrapping_add(1);
        val
    }
}
