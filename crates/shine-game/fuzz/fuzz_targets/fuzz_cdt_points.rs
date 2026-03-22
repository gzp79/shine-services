#![no_main]

use arbitrary::Arbitrary;
use glam::IVec2;
use libfuzzer_sys::fuzz_target;
use shine_game::math::cdt::Triangulation;

#[derive(Arbitrary, Debug)]
struct PointsInput {
    points: Vec<(i16, i16)>,
}

fuzz_target!(|input: PointsInput| {
    if input.points.len() < 3 || input.points.len() > 500 {
        return;
    }

    let points: Vec<IVec2> = input.points.iter()
        .map(|&(x, y)| IVec2::new(x as i32, y as i32))
        .collect();

    if let Ok(t) = Triangulation::build(&points) {
        t.check();
    }
});
