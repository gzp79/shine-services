#![no_main]

use arbitrary::Arbitrary;
use glam::IVec2;
use libfuzzer_sys::fuzz_target;
use shine_game::math::triangulation::{GeometryChecker, TopologyChecker, Triangulation};

#[derive(Arbitrary, Debug)]
struct ConstrainedInput {
    points: Vec<(i16, i16)>,
    edges: Vec<(u8, u8)>,
}

fuzz_target!(|input: ConstrainedInput| {
    if input.points.len() < 3 || input.points.len() > 200 {
        return;
    }

    let mut tri = Triangulation::new_cdt();
    let mut vertices = Vec::new();

    {
        let mut builder = tri.builder();
        for &(x, y) in &input.points {
            let vi = builder.add_vertex(IVec2::new(x as i32, y as i32), None);
            vertices.push(vi);
        }
    }

    if TopologyChecker::new(&tri).check().is_err() {
        panic!("topology check failed after adding points");
    }
    if GeometryChecker::new(&tri).check().is_err() {
        panic!("geometry check failed after adding points");
    }

    let n = vertices.len();
    for &(a, b) in &input.edges {
        let a = a as usize % n;
        let b = b as usize % n;
        if a == b {
            continue;
        }

        tri.builder().add_constraint_edge(vertices[a], vertices[b], 1);

        if TopologyChecker::new(&tri).check().is_err() {
            panic!("topology check failed after constraint edge");
        }
        if GeometryChecker::new(&tri).check().is_err() {
            panic!("geometry check failed after constraint edge");
        }
    }
});
