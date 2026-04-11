#![no_main]

use arbitrary::Arbitrary;
use glam::IVec2;
use libfuzzer_sys::fuzz_target;
use shine_game::math::triangulation::{GeometryChecker, TopologyChecker, Triangulation};

#[derive(Arbitrary, Debug)]
struct PointsInput {
    points: Vec<(i16, i16)>,
}

fuzz_target!(|input: PointsInput| {
    if input.points.len() < 3 || input.points.len() > 500 {
        return;
    }

    let mut tri = Triangulation::new_ct();

    for &(x, y) in &input.points {
        tri.builder().add_vertex(IVec2::new(x as i32, y as i32), None);

        if TopologyChecker::new(&tri).check().is_err() {
            panic!("topology check failed");
        }
        if GeometryChecker::new(&tri).check().is_err() {
            panic!("geometry check failed");
        }
    }
});
