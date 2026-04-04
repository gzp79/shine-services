use crate::{
    indexed::IdxVec,
    math::mesh::{QuadIdx, QuadTopology, QuadTopologyError, VertIdx},
};
use glam::Vec2;

/// Quad mesh with positions and topology.
///
/// Combines geometric vertex positions with topological connectivity via ghost vertex
/// and ghost quads for closed manifold traversal.
pub struct QuadMesh {
    pub topology: QuadTopology,
    pub positions: IdxVec<VertIdx, Vec2>,
    pub quad_centers: IdxVec<QuadIdx, Vec2>,
}

impl QuadMesh {
    pub fn from_polygon(
        positions: Vec<Vec2>,
        polygon: Vec<VertIdx>,
        quads: Vec<[VertIdx; 4]>,
    ) -> Result<Self, QuadTopologyError> {
        let vertex_count = positions.len();
        let positions = IdxVec::from_vec(positions);
        let topology = QuadTopology::from_polygon(vertex_count, polygon, quads)?;

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
            positions,
            quad_centers,
        })
    }

    pub fn position(&self, vi: VertIdx) -> Vec2 {
        self.positions[vi]
    }

    pub fn topology(&self) -> &QuadTopology {
        &self.topology
    }

    pub fn vertex_indices(&self) -> impl Iterator<Item = VertIdx> {
        self.topology.vertex_indices()
    }

    pub fn into_parts(self) -> (QuadTopology, IdxVec<VertIdx, Vec2>, IdxVec<QuadIdx, Vec2>) {
        (self.topology, self.positions, self.quad_centers)
    }
}
