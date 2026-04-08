use crate::{
    indexed::{IdxVec, TypedIndex},
    math::mesh::QuadTopologyError,
};
use glam::Vec2;

crate::define_typed_index!(VertIdx, "Typed index into a vertex array.");
crate::define_typed_index!(QuadIdx, "Typed index into a quad array.");

/// A quad with its local edge index (0..4)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuadEdge {
    pub quad: QuadIdx,
    pub edge: u8,
}

impl QuadEdge {
    /// QuadVertex at the start of this edge
    pub fn start(&self) -> QuadVertex {
        QuadVertex {
            quad: self.quad,
            local: self.edge,
        }
    }

    /// QuadVertex at the end of this edge
    pub fn end(&self) -> QuadVertex {
        QuadVertex {
            quad: self.quad,
            local: (self.edge + 1) % 4,
        }
    }
}

/// A quad with a vertex's local position (0..4) within it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuadVertex {
    pub quad: QuadIdx,
    pub local: u8,
}

impl QuadVertex {
    /// Next vertex CCW around this quad
    pub fn next(&self) -> QuadVertex {
        QuadVertex {
            quad: self.quad,
            local: (self.local + 1) % 4,
        }
    }

    /// Previous vertex CCW around this quad
    pub fn prev(&self) -> QuadVertex {
        QuadVertex {
            quad: self.quad,
            local: (self.local + 3) % 4,
        }
    }

    /// Opposite vertex across the quad
    pub fn opposite(&self) -> QuadVertex {
        QuadVertex {
            quad: self.quad,
            local: (self.local + 2) % 4,
        }
    }

    /// Edge leaving this vertex (outgoing)
    pub fn outgoing_edge(&self) -> QuadEdge {
        QuadEdge {
            quad: self.quad,
            edge: self.local,
        }
    }

    /// Edge entering this vertex (incoming)
    pub fn incoming_edge(&self) -> QuadEdge {
        QuadEdge {
            quad: self.quad,
            edge: (self.local + 3) % 4,
        }
    }
}

/// Classification of an edge in the quad mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeType {
    /// Edge is shared by two real (non-ghost) quads
    Interior,
    /// Edge is on the boundary (shared with a ghost quad)
    Boundary,
    /// The two vertices don't form an edge in the mesh
    NotAnEdge,
}

/// Quad mesh topology with adjacency — no positions.
///
/// The mesh is extended with a ghost vertex and ghost quads to form a topologically
/// closed mesh. The ghost vertex acts as an apex connecting all boundary vertices,
/// enabling consistent CCW navigation around every vertex including boundary vertices.
///
/// ## Ghost Topology
///
/// - **Ghost vertex**: Located at `VertIdx(vertex_count)`, has no geometric position
/// - **Ghost quads**: N/2 quads for N boundary vertices, structured as `[ghost, v2, v1, v0]`
///   where v0-v1-v2 are consecutive boundary vertices in CCW order (reversed in ghost quad for twin edges)
///
/// ## Navigation
///
/// - All vertices support full CCW ring traversal via `vertex_ring()`
/// - Boundary edges can be identified via `edge_type()` returning `EdgeType::Boundary`
/// - Ghost quads are detectable via `is_ghost_quad()` and should typically be excluded
///   from geometric operations
pub struct QuadTopology {
    // Number of real vertices (excluding ghost vertex)
    pub(crate) vertex_count: usize,
    // Number of ghost quads (half the number of boundary edges)
    pub(crate) ghost_quad_count: usize,
    pub(crate) quads: IdxVec<QuadIdx, [VertIdx; 4]>,
    // For each quad, the neighboring quads across each edge
    pub(crate) edge_twins: IdxVec<QuadIdx, [QuadEdge; 4]>,
    // For each vertex, a reference to one of the quads in its ring (arbitrary choice).
    pub(crate) vertex_quad: Vec<QuadVertex>,
    // Start vertex for each anchour (non-subdivided) edge.
    pub(crate) anchor_vertices: Vec<VertIdx>,
}

impl QuadTopology {
    /// Number of (real) vertices
    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// Iterator over the (real) vertex indices
    pub fn vertex_indices(&self) -> impl Iterator<Item = VertIdx> {
        (0..self.vertex_count).map(VertIdx::new)
    }

    /// Ghost vertex index
    pub fn ghost_vertex(&self) -> VertIdx {
        VertIdx::new(self.vertex_count)
    }

    pub fn is_ghost_vertex(&self, id: VertIdx) -> bool {
        id == VertIdx::new(self.vertex_count)
    }

    pub fn is_boundary_vertex(&self, vi: VertIdx) -> bool {
        self.vertex_ring_ccw(vi).any(|qv| self.is_ghost_quad(qv.quad))
    }

    pub fn boundary_vertex_count(&self) -> usize {
        self.ghost_quad_count * 2
    }

    /// Returns an iterator over boundary edges as vertex index pairs.
    ///
    /// Ghost quad [ghost, v2, v1, v0] has edges 1:v2->v1 and 2:v1->v0 as boundary edges (reversed).
    pub fn boundary_edges(&self) -> impl Iterator<Item = [u32; 2]> + '_ {
        self.vertex_ring_ccw(self.ghost_vertex()).flat_map(move |qv| {
            // Extract edges 1 and 2 from each ghost quad (the boundary edges in reverse)
            (1..3).filter_map(move |edge_idx| {
                let qe = QuadEdge { quad: qv.quad, edge: edge_idx };
                let (v0, v1) = self.edge_vertices(qe);
                if let (Some(i0), Some(i1)) = (v0.try_into_index(), v1.try_into_index()) {
                    Some([i0 as u32, i1 as u32])
                } else {
                    None
                }
            })
        })
    }

    pub fn boundary_vertices(&self) -> impl Iterator<Item = VertIdx> + '_ {
        // Walk vertex ring around ghost using CW traversal for correct CCW boundary order
        // Ghost quad structure: [ghost, v2, v1, v0] with ghost at position 0
        // Extract v0,v1 (positions 3,2) which gives CCW boundary pairs
        let ghost = self.ghost_vertex();
        self.vertex_ring_cw(ghost).flat_map(move |qv| {
            // Extract boundary vertices at positions 3,2 (v0, v1)
            let quad_verts = self.quad_vertices(qv.quad);
            [quad_verts[3], quad_verts[2]]
        })
    }

    pub fn edge_type(&self, a: VertIdx, b: VertIdx) -> EdgeType {
        // Find the quad containing edge a→b
        for qv in self.vertex_ring_ccw(a) {
            if self.vertex_index(qv.next()) == b {
                // Found the edge, check if its neighbor is a ghost quad
                let edge = qv.outgoing_edge();
                let neighbor = self.edge_twin(edge);

                if self.is_ghost_quad(neighbor.quad) {
                    return EdgeType::Boundary;
                } else {
                    return EdgeType::Interior;
                }
            }
        }

        EdgeType::NotAnEdge
    }

    pub fn quad_count(&self) -> usize {
        self.quads.len() - self.ghost_quad_count
    }

    pub fn quad_indices(&self) -> impl Iterator<Item = QuadIdx> + '_ {
        (0..self.quads.len())
            .map(QuadIdx::new)
            .filter(|&qi| !self.is_ghost_quad(qi))
    }

    pub fn ghost_quad_count(&self) -> usize {
        self.ghost_quad_count
    }

    pub fn ghost_quad_indices(&self) -> impl Iterator<Item = QuadIdx> + '_ {
        (0..self.quads.len())
            .map(QuadIdx::new)
            .filter(|&qi| self.is_ghost_quad(qi))
    }

    pub fn is_ghost_quad(&self, qi: QuadIdx) -> bool {
        let ghost = self.ghost_vertex();
        let verts = self.quads[qi];
        verts.contains(&ghost)
    }

    /// Returns an iterator over vertices along the given anchor edge.
    ///
    /// An anchor edge represents an original boundary edge (before subdivision).
    /// This iterates from the anchor vertex at `edge` to the next anchor vertex,
    /// following the boundary.
    pub fn anchor_edge(&self, edge: usize) -> impl Iterator<Item = VertIdx> + '_ {
        let start = self.anchor_vertices[edge];
        let end = self.anchor_vertices[(edge + 1) % self.anchor_vertices.len()];

        // Collect boundary vertices and slice from start to end anchor
        let boundary: Vec<_> = self.boundary_vertices().collect();
        boundary
            .into_iter()
            .skip_while(move |&v| v != start)
            .take_while(move |&v| v != end)
            .chain(std::iter::once(end))
    }

    pub fn edge_twin(&self, qe: QuadEdge) -> QuadEdge {
        self.edge_twins[qe.quad][qe.edge as usize]
    }

    pub fn vertex_index(&self, qv: QuadVertex) -> VertIdx {
        self.quads[qv.quad][qv.local as usize]
    }

    pub fn edge_vertices(&self, qe: QuadEdge) -> (VertIdx, VertIdx) {
        let quad = self.quads[qe.quad];
        (quad[qe.edge as usize], quad[(qe.edge as usize + 1) % 4])
    }

    pub fn quad_vertices(&self, qi: QuadIdx) -> [VertIdx; 4] {
        self.quads[qi]
    }

    /// The ring of quads around vertex `vi`, with the local vertex index of vi.
    pub fn vertex_ring_ccw(&self, vi: VertIdx) -> impl Iterator<Item = QuadVertex> + '_ {
        let start_qv = self.vertex_quad[vi.into_index()];

        VertexRingIter {
            topology: self,
            start: start_qv,
            current: start_qv,
            max_loops: 1,
            clockwise: false,
            done: false,
        }
    }

    pub fn vertex_ring_cw(&self, vi: VertIdx) -> impl Iterator<Item = QuadVertex> + '_ {
        let start_qv = self.vertex_quad[vi.into_index()];

        VertexRingIter {
            topology: self,
            start: start_qv,
            current: start_qv,
            max_loops: 1,
            clockwise: true,
            done: false,
        }
    }

    /// Average position of real edge neighbors of `vi` (via "next" in each ring quad).
    /// Ghost neighbors are skipped.
    pub fn neighbor_avg(&self, vi: VertIdx, positions: &[Vec2]) -> Vec2 {
        assert_ne!(vi, self.ghost_vertex());

        let mut sum = Vec2::ZERO;
        let mut count = 0u32;

        for qv in self.vertex_ring_ccw(vi) {
            let next = self.vertex_index(qv.next());
            if let Some(idx) = next.try_into_index() {
                sum += positions[idx];
                count += 1;
            }
        }

        if count > 0 {
            sum / count as f32
        } else {
            positions[vi.into_index()]
        }
    }

    /// Validates the topology for consistency.
    pub fn validate(&self) -> Result<(), QuadTopologyError> {
        use crate::math::mesh::QuadTopologyError;

        let ghost_vertex = self.ghost_vertex();

        // 1. Check all vertices have an associated quad
        for vi_idx in 0..=self.vertex_count {
            let qv = self.vertex_quad[vi_idx];
            if qv.quad.is_none() {
                return Err(QuadTopologyError::VertexHasNoQuad(vi_idx));
            }
        }

        // 2. Check edge twin bidirectionality
        for qi_idx in 0..self.quads.len() {
            let qi = QuadIdx::new(qi_idx);
            for edge_idx in 0..4 {
                let qe = QuadEdge { quad: qi, edge: edge_idx as u8 };
                let twin = self.edge_twin(qe);

                // Check twin points back
                let (v0, v1) = self.edge_vertices(qe);
                let (twin_v0, twin_v1) = self.edge_vertices(twin);

                if v0 != twin_v1 || v1 != twin_v0 {
                    return Err(QuadTopologyError::InvalidEdgeTwin { quad: qi_idx, edge: edge_idx });
                }
            }
        }

        // 3. Check ghost quad structure
        for qi in self.ghost_quad_indices() {
            let verts = self.quad_vertices(qi);
            let ghost_count = verts.iter().filter(|&&v| v == ghost_vertex).count();

            if ghost_count != 1 {
                return Err(QuadTopologyError::InvalidGhostQuadStructure {
                    quad: qi.into_index(),
                    count: ghost_count,
                });
            }
        }

        // 4. Check vertex rings form closed loops
        for vi_idx in 0..self.vertex_count {
            let vi = VertIdx::new(vi_idx);
            let ring: Vec<_> = self.vertex_ring_ccw(vi).collect();

            if ring.is_empty() {
                return Err(QuadTopologyError::VertexRingNotClosed { vertex: vi_idx });
            }

            // Check ring closure by verifying last→first connection
            if !ring.is_empty() {
                let last = ring[ring.len() - 1];
                let incoming = last.incoming_edge();
                let neighbor = self.edge_twin(incoming);

                if neighbor.start().quad != ring[0].quad {
                    return Err(QuadTopologyError::VertexRingNotClosed { vertex: vi_idx });
                }
            }
        }

        // 5. Check anchor edges form subsequences of boundary vertices
        if !self.anchor_vertices.is_empty() {
            // Get boundary as a vector
            let boundary: Vec<_> = self.boundary_vertices().collect();

            // Check all anchor vertices are in the boundary (to avoid infinite loop in anchor_edge)
            for (idx, &anchor_v) in self.anchor_vertices.iter().enumerate() {
                if !boundary.contains(&anchor_v) {
                    return Err(QuadTopologyError::InvalidAnchorEdge { edge: idx });
                }
            }

            // Double the boundary to handle wrap-around
            let doubled: Vec<_> = boundary.iter().chain(boundary.iter()).copied().collect();

            // Check each anchor edge (CCW order enforced - no reverse)
            for edge_idx in 0..self.anchor_vertices.len() {
                let anchor_edge: Vec<_> = self.anchor_edge(edge_idx).collect();

                // Find this subsequence in the doubled boundary (forward direction only)
                let found = doubled
                    .windows(anchor_edge.len())
                    .any(|window| window == anchor_edge.as_slice());

                if !found {
                    return Err(QuadTopologyError::InvalidAnchorEdge { edge: edge_idx });
                }
            }
        }

        Ok(())
    }
}

struct VertexRingIter<'a> {
    topology: &'a QuadTopology,
    clockwise: bool, // If true, traverse clockwise using outgoing edges
    start: QuadVertex,
    current: QuadVertex,
    max_loops: usize, // Decremented on each loop completion; panics if reaches 0
    done: bool,
}

impl<'a> Iterator for VertexRingIter<'a> {
    type Item = QuadVertex;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let result = self.current;

        // Move to next quad in ring
        if self.clockwise {
            // CW: Move via outgoing edge (forward around the vertex)
            let edge = self.current.outgoing_edge();
            let neighbor = self.topology.edge_twins[edge.quad][edge.edge as usize];
            self.current = neighbor.end(); // Use end to stay at the same vertex
        } else {
            // CCW: Move via incoming edge (backward around the vertex)
            let edge = self.current.incoming_edge();
            let neighbor = self.topology.edge_twins[edge.quad][edge.edge as usize];
            self.current = neighbor.start(); // Use start to stay at the same vertex
        }

        // Check if we've completed a ring loop
        if self.current.quad == self.start.quad {
            assert!(
                self.max_loops > 0,
                "VertexRingIter completed too many loops - likely skip_while/take_while didn't terminate"
            );
            self.max_loops -= 1;
            if self.max_loops == 0 {
                self.done = true;
            }
        }

        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shine_core::utils::is_rotation;
    use shine_test::test;
    use std::collections::HashSet;

    /// 2×2 grid of 4 quads, 9 vertices, 1 interior vertex (4):
    /// ```text
    ///  6----7----8
    ///  | Q2 | Q3 |
    ///  3----4----5
    ///  | Q0 | Q1 |
    ///  0----1----2
    /// ```
    /// Q0=[0,1,4,3]  Q1=[1,2,5,4]  Q2=[3,4,7,6]  Q3=[4,5,8,7]  (CCW)
    /// Interior: 4.  Boundary: 8 vertices (0,1,2,5,8,7,6,3).
    /// Anchors: 4 corners (0,2,8,6) marking the original boundary before subdivision.
    fn grid_2x2_topo() -> QuadTopology {
        let quads = vec![
            [VertIdx::new(0), VertIdx::new(1), VertIdx::new(4), VertIdx::new(3)],
            [VertIdx::new(1), VertIdx::new(2), VertIdx::new(5), VertIdx::new(4)],
            [VertIdx::new(3), VertIdx::new(4), VertIdx::new(7), VertIdx::new(6)],
            [VertIdx::new(4), VertIdx::new(5), VertIdx::new(8), VertIdx::new(7)],
        ];
        let boundaries: Vec<_> = [0, 1, 2, 5, 8, 7, 6, 3].into_iter().map(VertIdx::new).collect();
        let anchors: Vec<_> = [0, 2, 8, 6].into_iter().map(VertIdx::new).collect();
        QuadTopology::from_polygon(9, boundaries, anchors, quads).expect("valid topology")
    }

    #[test]
    fn test_quad_vertex_navigation() {
        let qv = QuadVertex {
            quad: QuadIdx::new(0),
            local: 0,
        };

        assert_eq!(qv.next().local, 1);
        assert_eq!(qv.prev().local, 3);
        assert_eq!(qv.opposite().local, 2);
        assert_eq!(qv.outgoing_edge().edge, 0);
        assert_eq!(qv.incoming_edge().edge, 3);
    }

    #[test]
    fn test_quad_edge_navigation() {
        let qe = QuadEdge { quad: QuadIdx::new(0), edge: 1 };

        assert_eq!(qe.start().local, 1);
        assert_eq!(qe.end().local, 2);
    }

    #[test]
    fn test_topology_counts() {
        let topo = grid_2x2_topo();

        assert_eq!(topo.vertex_count(), 9, "9 real vertices");
        assert_eq!(topo.quad_count(), 4, "4 real quads");
        assert_eq!(topo.ghost_quad_count(), 4, "4 ghost quads (8 boundary edges / 2)");
        assert_eq!(topo.boundary_vertex_count(), 8, "8 boundary vertices");

        let ghost_vertex = topo.ghost_vertex();
        assert_eq!(ghost_vertex, VertIdx::new(9), "ghost vertex at index 9");
    }

    #[test]
    fn test_vertex_rings() {
        let topo = grid_2x2_topo();

        // Verify interior vertex 4 ring is [Q0, Q1, Q3, Q2] in CCW order
        let vert_4_ring: Vec<_> = topo.vertex_ring_ccw(VertIdx::new(4)).map(|qv| qv.quad).collect();
        let expected_rotations = [
            vec![QuadIdx::new(0), QuadIdx::new(1), QuadIdx::new(3), QuadIdx::new(2)],
            vec![QuadIdx::new(1), QuadIdx::new(3), QuadIdx::new(2), QuadIdx::new(0)],
            vec![QuadIdx::new(3), QuadIdx::new(2), QuadIdx::new(0), QuadIdx::new(1)],
            vec![QuadIdx::new(2), QuadIdx::new(0), QuadIdx::new(1), QuadIdx::new(3)],
        ];
        assert!(
            expected_rotations.contains(&vert_4_ring),
            "vertex 4 ring {:?} should be a rotation of [Q0, Q1, Q3, Q2]",
            vert_4_ring
        );

        // Verify all real vertices have complete rings
        for vi in topo.vertex_indices() {
            let ring: Vec<_> = topo.vertex_ring_ccw(vi).collect();
            assert!(!ring.is_empty(), "vertex {:?} should have non-empty ring", vi);

            // All ring entries should reference the same vertex
            for qv in &ring {
                assert_eq!(
                    topo.vertex_index(*qv),
                    vi,
                    "ring entry should reference vertex {:?}",
                    vi
                );
            }

            // Ring should be connected - each quad's neighbor should lead to next quad in ring
            for i in 0..ring.len() {
                let current = ring[i];
                let next_in_ring = ring[(i + 1) % ring.len()];

                // Get the neighbor across the incoming edge
                let incoming = current.incoming_edge();
                let neighbor = topo.edge_twin(incoming);

                assert_eq!(
                    neighbor.start(),
                    next_in_ring,
                    "neighbor.start() should match next in ring for vertex {:?}",
                    vi
                );
            }
        }
    }

    #[test]
    fn test_boundary_detection() {
        let topo = grid_2x2_topo();

        for vi in topo.vertex_indices() {
            let is_boundary = topo.is_boundary_vertex(vi);
            let ring_has_ghost = topo.vertex_ring_ccw(vi).any(|qv| topo.is_ghost_quad(qv.quad));

            assert_eq!(
                is_boundary, ring_has_ghost,
                "vertex {:?}: is_boundary_vertex and ring_has_ghost should match",
                vi
            );
        }
    }

    #[test]
    fn test_edge_classification() {
        let topo = grid_2x2_topo();
        let ghost_vertex = topo.ghost_vertex();
        // Interior vertex 4 should have all interior edges to its neighbors
        for qv in topo.vertex_ring_ccw(VertIdx::new(4)) {
            let next_v = topo.vertex_index(qv.next());
            if next_v != ghost_vertex {
                assert_eq!(
                    topo.edge_type(VertIdx::new(4), next_v),
                    EdgeType::Interior,
                    "edge from interior vertex 4 to {:?} should be Interior",
                    next_v
                );
            }
        }

        // Boundary edges should be detected correctly
        let boundary_edges = [(0, 1), (1, 2), (2, 5), (5, 8), (8, 7), (7, 6), (6, 3), (3, 0)];
        for (a, b) in boundary_edges {
            assert_eq!(
                topo.edge_type(VertIdx::new(a), VertIdx::new(b)),
                EdgeType::Boundary,
                "edge {:?}->{:?} should be Boundary",
                a,
                b
            );
        }
    }

    #[test]
    fn test_ghost_quad_structure() {
        let topo = grid_2x2_topo();
        let ghost_vertex = topo.ghost_vertex();

        for qi in topo.ghost_quad_indices() {
            let verts = topo.quad_vertices(qi);

            // Ghost quad should have exactly one ghost vertex
            let ghost_count = verts.iter().filter(|&&v| v == ghost_vertex).count();
            assert_eq!(
                ghost_count, 1,
                "ghost quad {:?} should contain exactly 1 ghost vertex",
                qi
            );

            // Other 3 vertices should be real boundary vertices
            let real_verts: Vec<_> = verts.iter().filter(|&&v| v != ghost_vertex).copied().collect();
            assert_eq!(real_verts.len(), 3, "ghost quad should have 3 real vertices");

            for &v in &real_verts {
                assert!(
                    topo.is_boundary_vertex(v),
                    "real vertex {:?} in ghost quad should be boundary",
                    v
                );
            }
        }
    }

    #[test]
    fn test_quad_neighbor_consistency() {
        let topo = grid_2x2_topo();
        for qi in topo.quad_indices() {
            for edge in 0..4 {
                let qe = QuadEdge { quad: qi, edge: edge as u8 };
                let neighbor = topo.edge_twin(qe);

                // Get the reverse edge
                let (v0, v1) = topo.edge_vertices(qe);

                // Find the edge in the neighbor that connects back
                let neighbor_verts = topo.quad_vertices(neighbor.quad);
                let mut found_reverse = false;
                for i in 0..4 {
                    if neighbor_verts[i] == v1 && neighbor_verts[(i + 1) % 4] == v0 {
                        found_reverse = true;
                        break;
                    }
                }

                assert!(
                    found_reverse,
                    "quad {:?} edge {} neighbor should have reverse edge",
                    qi, edge
                );
            }
        }
    }

    #[test]
    fn test_boundary_vertices_ccw_order() {
        let topo = grid_2x2_topo();

        let boundary: Vec<_> = topo.boundary_vertices().collect();
        assert_eq!(boundary.len(), 8, "should have 8 boundary vertices");

        // All boundary vertices should be unique
        let mut seen = HashSet::new();
        for v in &boundary {
            assert!(seen.insert(*v), "boundary vertex {:?} appears multiple times", v);
        }

        let expected = [0usize, 1, 2, 5, 8, 7, 6, 3];
        let boundary_indices: Vec<_> = boundary.iter().map(|v| v.into_index()).collect();
        assert!(
            is_rotation(&expected, &boundary_indices),
            "boundary {:?} should be a rotation of [0, 1, 2, 5, 8, 7, 6, 3]",
            boundary_indices
        );
    }

    #[test]
    fn test_anchor_edges_ccw_order() {
        let topo = grid_2x2_topo();

        // Edge 0: anchor 0 -> 2, should be [0, 1, 2]
        let edge0: Vec<_> = topo.anchor_edge(0).collect();
        let expected0: Vec<_> = [0, 1, 2].into_iter().map(VertIdx::new).collect();
        assert_eq!(edge0, expected0, "anchor edge 0 should be [0, 1, 2]");

        // Edge 1: anchor 2 -> 8, should be [2, 5, 8]
        let edge1: Vec<_> = topo.anchor_edge(1).collect();
        let expected1: Vec<_> = [2, 5, 8].into_iter().map(VertIdx::new).collect();
        assert_eq!(edge1, expected1, "anchor edge 1 should be [2, 5, 8]");

        // Edge 2: anchor 8 -> 6, should be [8, 7, 6]
        let edge2: Vec<_> = topo.anchor_edge(2).collect();
        let expected2: Vec<_> = [8, 7, 6].into_iter().map(VertIdx::new).collect();
        assert_eq!(edge2, expected2, "anchor edge 2 should be [8, 7, 6]");

        // Edge 3: anchor 6 -> 0, should be [6, 3, 0]
        let edge3: Vec<_> = topo.anchor_edge(3).collect();
        let expected3: Vec<_> = [6, 3, 0].into_iter().map(VertIdx::new).collect();
        assert_eq!(edge3, expected3, "anchor edge 3 should be [6, 3, 0]");
    }

    #[test]
    fn test_topology_validation() {
        let topo = grid_2x2_topo();

        // Valid topology should pass validation
        topo.validate().expect("valid topology should pass validation");
    }
}
