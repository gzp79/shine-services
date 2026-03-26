#![no_main]

use arbitrary::Arbitrary;
use glam::IVec2;
use libfuzzer_sys::fuzz_target;
use shine_game::math::cdt::{CdtError, Triangulation};

#[derive(Arbitrary, Debug)]
struct CdtInput {
    points: Vec<(i16, i16)>,
    edges: Vec<(u8, u8)>,
}

fuzz_target!(|input: CdtInput| {
    if input.points.len() < 3 || input.points.len() > 200 {
        return;
    }

    let points: Vec<IVec2> = input.points.iter()
        .map(|&(x, y)| IVec2::new(x as i32, y as i32))
        .collect();

    let n = points.len();
    let edges: Vec<(usize, usize)> = input.edges.iter()
        .filter_map(|&(a, b)| {
            let a = a as usize % n;
            let b = b as usize % n;
            if a != b { Some((a, b)) } else { None }
        })
        .collect();

    if edges.is_empty() {
        return;
    }

    let result = Triangulation::build_with_edges(&points, &edges);
    match result {
        Ok(t) => { t.check(); }
        Err(CdtError::CrossingFixedEdge) => {}
        Err(CdtError::PointOnFixedEdge(_)) => {}
        Err(CdtError::EmptyInput) => {}
        Err(CdtError::TooFewPoints) => {}
        Err(CdtError::InvalidEdge) => {}
        Err(CdtError::InvalidInput) => {}
        Err(CdtError::CannotInitialize) => {}
        Err(e) => { panic!("unexpected error: {e}"); }
    }
});
