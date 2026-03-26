use crate::math::mesh::QuadMesh;

/// Trait for filters that modify [`QuadMesh`] vertex positions.
///
/// Each filter encapsulates its own parameters (iterations, strength, etc.)
/// and [`apply`](QuadFilter::apply) runs the full operation.
pub trait QuadFilter {
    fn apply(&mut self, mesh: &mut QuadMesh);
}
