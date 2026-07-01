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
struct PointsInput {
    variant: u8,
    points: Vec<(i16, i16)>,
}

impl PointsInput {
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
}

impl fmt::Debug for PointsInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = self.points.len();
        if n < 3 || n > 200 {
            return f
                .debug_struct("ConstrainedInput (Invalid)")
                .field("points_len", &n)
                .finish();
        }

        let points = self.normalized_points();

        writeln!(
            f,
            "let points: Vec<(i32,i32)> = vec!{:?};",
            points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>()
        )?;

        Ok(())
    }
}

fuzz_target!(|input: PointsInput| {
    if input.points.len() < 3 || input.points.len() > 500 {
        return;
    }

    let points = input.normalized_points();

    let mut tri = Triangulation::new_ct();

    let mut builder = tri.builder();
    builder.add_points(points);

    builder.delaunay_refine_all();
    builder
        .validate()
        .expect("builder check failed after delaunay refinement");
    GeometryChecker::new(builder.tri())
        .check_delaunay()
        .expect("Delaunay condition failed after refinement");
});
