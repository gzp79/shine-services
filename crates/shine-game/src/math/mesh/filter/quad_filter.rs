use crate::math::mesh::QuadMesh;

/// Trait for filters that modify [`QuadMesh`] vertex positions.
///
/// Each filter encapsulates its own parameters (iterations, strength, etc.)
/// and [`apply`](QuadFilter::apply) runs the full operation.
///
/// Composable pipeline example:
///
/// ```rust,ignore
/// let mut filters: Vec<Box<dyn QuadFilter>> = vec![
///     Box::new(Jitter::new(0.3, rng)),
///     Box::new(LaplacianSmoother::new(0.5, 20)),
///     Box::new(QuadRelax::new(0.15, 0.5, 50)),
/// ];
/// for f in &mut filters {
///     f.apply(&mut mesh);
/// }
/// ```
pub trait QuadFilter {
    fn apply(&mut self, mesh: &mut QuadMesh);
}
