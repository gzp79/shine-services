use crate::{
    indexed::IdxVec,
    math::quadrangulation::{QuadError, QuadIdx, QuadTopology, VertIdx},
};
use glam::Vec2;

/// Quad mesh with positions and topology.
///
/// Combines geometric vertex positions with topological connectivity via ghost vertex
/// and ghost quads for closed manifold traversal.
pub struct QuadMesh {
    pub topology: QuadTopology,
    pub vertices: IdxVec<VertIdx, Vec2>,
    pub quad_centers: IdxVec<QuadIdx, Vec2>,
}

impl QuadMesh {
    pub fn from_polygon(
        positions: Vec<Vec2>,
        polygon: Vec<VertIdx>,
        anchors: Vec<VertIdx>,
        quads: Vec<[VertIdx; 4]>,
    ) -> Result<Self, QuadError> {
        let vertex_count = positions.len();
        let positions = IdxVec::from(positions);
        let topology = QuadTopology::from_polygon(vertex_count, polygon, anchors, quads)?;

        // Compute quad centers for all real quads
        let mut quad_centers = IdxVec::with_capacity(topology.quad_count());
        for qi in topology.quad_indices() {
            let verts = topology.quad_vertices(qi);
            let mut center = Vec2::ZERO;
            for &v in &verts {
                center += positions[v];
            }
            center /= 4.0;
            quad_centers.push(center);
        }

        Ok(Self {
            topology,
            vertices: positions,
            quad_centers,
        })
    }
}
