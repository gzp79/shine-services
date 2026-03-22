// Vendored from cdt crate v0.1.0 (https://github.com/Formlabs/foxtrot/tree/master/cdt)
// Original: Copyright (c) 2021 Formlabs, MIT OR Apache-2.0
// See licenses/cdt/ for full license text.
//
// Modified: i32 input coordinates, i64 exact predicates, no external dependencies.

mod contour;
mod error;
mod half;
mod hull;
mod indexes;
mod predicates;
mod triangulate;

pub use error::CdtError;
use glam::IVec2;
pub use triangulate::Triangulation;

/// Triangulates a set of integer points, returning triangles as triples of
/// indexes into the original points list.
pub fn triangulate_points(pts: &[IVec2]) -> Result<Vec<(usize, usize, usize)>, CdtError> {
    let t = Triangulation::build(pts)?;
    Ok(t.triangles().collect())
}

/// Triangulates with constrained edges.
pub fn triangulate_with_edges<'a, E>(pts: &[IVec2], edges: E) -> Result<Vec<(usize, usize, usize)>, CdtError>
where
    E: IntoIterator<Item = &'a (usize, usize)> + Copy + Clone,
{
    let t = Triangulation::build_with_edges(pts, edges)?;
    Ok(t.triangles().collect())
}

/// Triangulates contours.
pub fn triangulate_contours<V>(pts: &[IVec2], contours: &[V]) -> Result<Vec<(usize, usize, usize)>, CdtError>
where
    for<'b> &'b V: IntoIterator<Item = &'b usize>,
{
    let t = Triangulation::build_from_contours(pts, contours)?;
    Ok(t.triangles().collect())
}
