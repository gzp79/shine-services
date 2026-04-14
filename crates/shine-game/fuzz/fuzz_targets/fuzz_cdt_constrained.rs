#![no_main]

use arbitrary::Arbitrary;
use glam::IVec2;
use libfuzzer_sys::fuzz_target;
use shine_game::math::triangulation::{GeometryChecker, TopologyChecker, Triangulation};
use std::fmt;

const MAX_X: i32 = 100;
const MAX_Y: i32 = 100;

#[derive(Arbitrary)]
struct ConstrainedInput {
    points: Vec<(i16, i16)>,
    edges: Vec<(u8, u8)>,
}

impl fmt::Debug for ConstrainedInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = self.points.len();
        if n < 3 || n > 200 {
            return f
                .debug_struct("ConstrainedInput (Invalid)")
                .field("points_len", &n)
                .finish();
        }

        let points: Vec<(i32, i32)> = self
            .points
            .iter()
            .map(|&(x, y)| (x as i32 % MAX_X, y as i32 % MAX_Y))
            .collect();

        let edges: Vec<(usize, usize)> = self
            .edges
            .iter()
            .filter_map(|&(a, b)| {
                let a = a as usize % n;
                let b = b as usize % n;
                if a != b {
                    Some((a, b))
                } else {
                    None
                }
            })
            .collect();

        writeln!(f, "let points: Vec<(i32,i32)> = vec!{:?};", points)?;
        write!(f, "let edges: Vec<(usize, usize)> = vec!{:?};", edges)
    }
}

fuzz_target!(|input: ConstrainedInput| {
    if input.points.len() < 3 || input.points.len() > 200 {
        return;
    }

    let mut tri = Triangulation::new_cdt();
    let mut vertices = Vec::new();

    for &(x, y) in &input.points {
        let vi = tri
            .builder()
            .add_vertex(IVec2::new(x as i32 % MAX_X, y as i32 % MAX_Y), None);
        vertices.push(vi);
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
