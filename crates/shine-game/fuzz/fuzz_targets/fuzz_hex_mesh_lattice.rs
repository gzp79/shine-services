#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use shine_game::math::{
    hex::LatticeMesher,
    prng::{SequenceRng, StableRng},
};
use std::fmt;

#[derive(Arbitrary)]
struct CdtMeshInput {
    subdivision: u8,
    rng_bytes: Vec<u32>,
}

impl CdtMeshInput {
    fn normalized_subdivision(&self) -> u32 {
        (self.subdivision % 3) as u32 + 1
    }
}

impl fmt::Debug for CdtMeshInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "let subdivision = {};", self.normalized_subdivision())?;
        writeln!(f, "let rng = SequenceRng::new(vec!(&{:?})).into_rc();", self.rng_bytes)?;

        Ok(())
    }
}

// CDT mesher: fuzzes interior point placement and the resulting quad mesh topology.
fuzz_target!(|input: CdtMeshInput| {
    let subdivision = input.normalized_subdivision();
    let rng = SequenceRng::new(input.rng_bytes).into_rc();

    let mesh = LatticeMesher::new(subdivision, rng).generate();
    mesh.topology.validate().expect("CDT mesh topology should be valid");
});
