#![no_main]

use arbitrary::Arbitrary;
use glam::IVec2;
use libfuzzer_sys::fuzz_target;
use shine_game::math::triangulation::{GeometryChecker, Triangulation};
use std::fmt;

const MAX1_X: i32 = 100;
const MAX1_Y: i32 = 100;
const MAX2_X: i32 = 65536;
const MAX2_Y: i32 = 65536;

#[derive(Arbitrary)]
struct ConstrainedInput {
    variant: u8,
    points: Vec<(i16, i16)>,
    edges: Vec<(u8, u8)>,
}

impl ConstrainedInput {
    fn normalized_points(&self) -> Vec<IVec2> {
        self.points
            .iter()
            .map(|&(x, y)| {
                if self.variant % 2 == 0 {
                    IVec2::new(x as i32 % MAX1_X, y as i32 % MAX1_Y)
                } else {
                    IVec2::new(x as i32 % MAX2_X, y as i32 % MAX2_Y)
                }
            })
            .collect()
    }

    fn normalized_edges(&self) -> Vec<(usize, usize)> {
        let n = self.points.len();
        self.edges
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
            .collect()
    }
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

        let points = self.normalized_points();
        let edges = self.normalized_edges();

        writeln!(
            f,
            "let points: Vec<(i32,i32)> = vec!{:?};",
            points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>()
        )?;
        write!(f, "let edges: Vec<(usize, usize)> = vec!{:?};", edges)?;

        Ok(())
    }
}

fuzz_target!(|input: ConstrainedInput| {
    if input.points.len() < 3 || input.points.len() > 200 || input.edges.len() > 200 {
        return;
    }

    let points = input.normalized_points();
    let edges = input.normalized_edges();

    let mut tri = Triangulation::new_ct();
    let mut vertices = Vec::new();

    let mut builder = tri.builder();

    for &p in &points {
        let vi = builder.add_vertex(p, None);
        vertices.push(vi);
    }
    builder.check().expect("builder check failed after adding points");

    for &(a, b) in &edges {
        builder.add_constraint_edge(vertices[a], vertices[b], 1);
        builder
            .check()
            .expect("builder check failed after adding constraint edge");
    }

    builder.delaunay_refine_all();
    builder.check().expect("builder check failed after delaunay refinement");
    GeometryChecker::new(builder.tri())
        .check_delaunay()
        .expect("Delaunay condition failed after refinement");
});
