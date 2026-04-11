#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use shine_game::math::{
    hex::CdtMesher,
    rand::StableRng,
};

/// RNG that replays fuzzer-provided bytes, cycling when exhausted.
struct FuzzRng {
    values: Vec<u32>,
    idx: usize,
}

impl FuzzRng {
    fn new(bytes: &[u8]) -> Self {
        let values: Vec<u32> = bytes
            .chunks(4)
            .map(|chunk| {
                let mut buf = [0u8; 4];
                buf[..chunk.len()].copy_from_slice(chunk);
                u32::from_le_bytes(buf)
            })
            .collect();
        Self { values, idx: 0 }
    }
}

impl StableRng for FuzzRng {
    fn next_u32(&mut self) -> u32 {
        if self.values.is_empty() {
            return 0;
        }
        let val = self.values[self.idx % self.values.len()];
        self.idx += 1;
        val
    }
}

#[derive(Arbitrary, Debug)]
struct CdtMeshInput {
    subdivision: u8,
    /// Interior point count — fuzzed separately from subdivision to explore
    /// sparse (0) through dense (255) interior triangulations.
    interior_points: u8,
    rng_bytes: Vec<u8>,
}

// CDT mesher: fuzzes interior point placement and the resulting quad mesh topology.
fuzz_target!(|input: CdtMeshInput| {
    let subdivision = (input.subdivision % 3) as u32 + 1;
    let interior_points = input.interior_points as u32;
    let rng = FuzzRng::new(&input.rng_bytes);
    let mesh = CdtMesher::new(subdivision, interior_points, rng).generate();
    mesh.topology.validate().expect("CDT mesh topology should be valid");
});
