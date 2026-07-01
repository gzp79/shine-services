#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use shine_game::math::{
    hex::CdtMesher,
    prng::{SequenceRng, StableRng},
};
use std::fmt;

#[derive(Arbitrary)]
struct CdtMeshInput {
    subdivision: u8,
    interior_point_count: u8,
    rng_bytes: Vec<u32>,
}

impl CdtMeshInput {
    fn normalized_subdivision(&self) -> u32 {
        (self.subdivision % 3) as u32 + 1
    }

    fn normalized_interior_point_count(&self) -> u32 {
        self.interior_point_count as u32
    }
}

impl fmt::Debug for CdtMeshInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "let subdivision = {};", self.normalized_subdivision())?;
        writeln!(
            f,
            "let interior_point_count = {};",
            self.normalized_interior_point_count()
        )?;
        writeln!(f, "let rng = SequenceRng::new(vec!(&{:?})).into_rc();", self.rng_bytes)?;

        Ok(())
    }
}

// CDT mesher: fuzzes interior point placement and the resulting quad mesh topology.
fuzz_target!(|input: CdtMeshInput| {
    let subdivision = input.normalized_subdivision();
    let interior_point_count = input.normalized_interior_point_count();
    let rng = SequenceRng::new(input.rng_bytes).into_rc();

    let mesh = CdtMesher::new(subdivision, interior_point_count, rng).generate();
    mesh.topology.validate().expect("CDT mesh topology should be valid");
});
