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
pub enum QuadEdgeType {
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
/// - **Ghost quads**: N/2 quads for N boundary vertices, each containing the ghost vertex
///   and 3 consecutive boundary vertices with reversed winding for twin edge adjacency
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
    /// Boundary edges are the two edges of each ghost quad that don't touch the ghost vertex.
    pub fn boundary_edges(&self) -> impl Iterator<Item = [u32; 2]> + '_ {
        self.vertex_ring_cw(self.ghost_vertex()).flat_map(move |qv| {
            // The two edges not touching the ghost vertex
            let e1 = (qv.local + 1) % 4;
            let e2 = (qv.local + 2) % 4;
            [e1, e2].into_iter().filter_map(move |edge_idx| {
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
        // Walk vertex ring around ghost using CW traversal for correct CCW boundary order.
        // For each ghost quad, emit the two boundary vertices going backward from ghost.
        let ghost = self.ghost_vertex();
        self.vertex_ring_cw(ghost).flat_map(move |qv| {
            let p1 = qv.prev();
            let p2 = p1.prev();
            [self.vertex_index(p1), self.vertex_index(p2)]
        })
    }

    pub fn edge_type(&self, a: VertIdx, b: VertIdx) -> QuadEdgeType {
        // Find the quad containing edge a→b
        for qv in self.vertex_ring_ccw(a) {
            if self.vertex_index(qv.next()) == b {
                let edge = qv.outgoing_edge();
                let neighbor = self.edge_twin(edge);

                // Boundary if either side of the edge is a ghost quad
                if self.is_ghost_quad(qv.quad) || self.is_ghost_quad(neighbor.quad) {
                    return QuadEdgeType::Boundary;
                } else {
                    return QuadEdgeType::Interior;
                }
            }
        }

        QuadEdgeType::NotAnEdge
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

    pub fn anchor_count(&self) -> usize {
        self.anchor_vertices.len()
    }

    /// Returns an iterator over vertices along the given anchor edge.
    ///
    /// An anchor edge represents an original boundary edge (before subdivision).
    /// This iterates from the anchor vertex at `edge` to the next anchor vertex,
    /// following the boundary.
    pub fn anchor_edge(&self, edge: usize) -> impl Iterator<Item = VertIdx> + '_ {
        let start = self.anchor_vertices[edge];
        let end = self.anchor_vertices[(edge + 1) % self.anchor_vertices.len()];

        // Use a 2x-looped CW ring around ghost vertex so skip_while/take_while
        // handles the case where the anchor edge wraps past the ring origin.
        let ghost = self.ghost_vertex();
        let start_qv = self.vertex_quad[ghost.into_index()];

        let ring = VertexRingIter::<false> {
            topology: self,
            max_loops: 2,
            start: start_qv,
            current: start_qv,
            done: false,
        };

        ring.flat_map(move |qv| {
            let p1 = qv.prev();
            let p2 = p1.prev();
            [self.vertex_index(p1), self.vertex_index(p2)]
        })
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

        VertexRingIter::<true> {
            topology: self,
            max_loops: 1,
            start: start_qv,
            current: start_qv,
            done: false,
        }
    }

    pub fn vertex_ring_cw(&self, vi: VertIdx) -> impl Iterator<Item = QuadVertex> + '_ {
        let start_qv = self.vertex_quad[vi.into_index()];

        VertexRingIter::<false> {
            topology: self,
            max_loops: 1,
            start: start_qv,
            current: start_qv,
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

    /// Helper to validate a single vertex ring forms a closed loop
    fn validate_vertex_ring(&self, vi: VertIdx) -> Result<(), QuadTopologyError> {
        let vi_idx = vi.into_index();
        let ring: Vec<_> = self.vertex_ring_ccw(vi).collect();

        if ring.is_empty() {
            return Err(QuadTopologyError::VertexRingNotClosed { vertex: vi_idx });
        }

        // Verify all ring elements reference the correct vertex
        for qv in &ring {
            let vertex_at_pos = self.quads[qv.quad][qv.local as usize];
            if vertex_at_pos != vi {
                return Err(QuadTopologyError::VertexRingNotClosed { vertex: vi_idx });
            }
        }

        // Check ring closure: next position after last should reference same vertex
        let last = ring[ring.len() - 1];
        let incoming = last.incoming_edge();
        let neighbor = self.edge_twin(incoming);
        let next_pos = neighbor.start();
        let next_vertex = self.quads[next_pos.quad][next_pos.local as usize];

        // Must be the same vertex (forms a cycle around vi)
        if next_vertex != vi {
            return Err(QuadTopologyError::VertexRingNotClosed { vertex: vi_idx });
        }

        // Next position should be in the ring (forms a closed cycle)
        let next_in_ring = ring
            .iter()
            .any(|qv| qv.quad == next_pos.quad && qv.local == next_pos.local);
        if !next_in_ring {
            return Err(QuadTopologyError::VertexRingNotClosed { vertex: vi_idx });
        }

        Ok(())
    }

    /// Validates the topology for consistency.
    pub fn validate(&self) -> Result<(), QuadTopologyError> {
        use crate::math::mesh::QuadTopologyError;

        let ghost_vertex = self.ghost_vertex();

        // 1. Check all vertices have an associated quad that references them correctly
        for vi_idx in 0..=self.vertex_count {
            let qv = self.vertex_quad[vi_idx];
            if qv.quad.is_none() {
                return Err(QuadTopologyError::VertexHasNoQuad(vi_idx));
            }
            let actual = self.quads[qv.quad][qv.local as usize];
            if actual != VertIdx::new(vi_idx) {
                return Err(QuadTopologyError::VertexQuadMismatch {
                    vertex: vi_idx,
                    actual: actual.into_index(),
                });
            }
        }

        // 2. Check no degenerate quads (all 4 vertices distinct)
        for qi_idx in 0..self.quads.len() {
            let verts = self.quads[QuadIdx::new(qi_idx)];
            for i in 0..4 {
                for j in (i + 1)..4 {
                    if verts[i] == verts[j] {
                        return Err(QuadTopologyError::DegenerateQuad {
                            quad: qi_idx,
                            vertex: verts[i].into_index(),
                        });
                    }
                }
            }
        }

        // 3. Check edge twin bidirectionality and involution
        for qi_idx in 0..self.quads.len() {
            let qi = QuadIdx::new(qi_idx);
            for edge_idx in 0..4 {
                let qe = QuadEdge { quad: qi, edge: edge_idx as u8 };
                let twin = self.edge_twin(qe);

                // Check twin vertices are reversed
                let (v0, v1) = self.edge_vertices(qe);
                let (twin_v0, twin_v1) = self.edge_vertices(twin);

                if v0 != twin_v1 || v1 != twin_v0 {
                    return Err(QuadTopologyError::InvalidEdgeTwin { quad: qi_idx, edge: edge_idx });
                }

                // Check twin of twin points back to original
                let round_trip = self.edge_twin(twin);
                if round_trip.quad != qe.quad || round_trip.edge != qe.edge {
                    return Err(QuadTopologyError::EdgeTwinNotInvolution { quad: qi_idx, edge: edge_idx });
                }
            }
        }

        // 4. Check ghost quad structure
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

        // 4b. Verify ghost_quad_count matches actual ghost quad count
        {
            let actual_ghost_count = self.quads.iter().filter(|verts| verts.contains(&ghost_vertex)).count();
            if actual_ghost_count != self.ghost_quad_count {
                return Err(QuadTopologyError::GhostQuadCountMismatch {
                    expected: self.ghost_quad_count,
                    actual: actual_ghost_count,
                });
            }
        }

        // 4c. Verify ghost quads are contiguous at the end of the quad array
        {
            let real_count = self.quads.len() - self.ghost_quad_count;
            for qi_idx in 0..real_count {
                if self.is_ghost_quad(QuadIdx::new(qi_idx)) {
                    // Find the first real quad after this ghost quad
                    let real_after = (qi_idx + 1..self.quads.len())
                        .find(|&i| !self.is_ghost_quad(QuadIdx::new(i)))
                        .unwrap_or(qi_idx);
                    return Err(QuadTopologyError::GhostQuadsNotCompact {
                        ghost_quad: qi_idx,
                        real_quad: real_after,
                    });
                }
            }
        }

        // 5. Check vertex rings form closed loops (real vertices and ghost vertex)
        for vi_idx in 0..self.vertex_count {
            self.validate_vertex_ring(VertIdx::new(vi_idx))?;
        }
        self.validate_vertex_ring(self.ghost_vertex())?;

        // 6. Check all quads are reachable from vertex rings
        {
            let mut reachable = vec![false; self.quads.len()];
            for vi_idx in 0..=self.vertex_count {
                for qv in self.vertex_ring_ccw(VertIdx::new(vi_idx)) {
                    reachable[qv.quad.into_index()] = true;
                }
            }
            for (qi_idx, &reached) in reachable.iter().enumerate() {
                if !reached {
                    return Err(QuadTopologyError::UnreachableQuad { quad: qi_idx });
                }
            }
        }

        // 7. Check anchor vertices are boundary vertices in correct cyclic order
        if !self.anchor_vertices.is_empty() {
            let boundary: Vec<_> = self.boundary_vertices().collect();

            // Check all anchor vertices are in the boundary
            for (idx, &anchor_v) in self.anchor_vertices.iter().enumerate() {
                if !boundary.contains(&anchor_v) {
                    return Err(QuadTopologyError::InvalidAnchorEdge { edge: idx });
                }
            }

            // Verify cyclic ordering: each anchor must follow the previous along the boundary
            let first_pos = boundary.iter().position(|&b| b == self.anchor_vertices[0]).unwrap();
            let mut search_start = first_pos;

            for edge_idx in 1..self.anchor_vertices.len() {
                let anchor_v = self.anchor_vertices[edge_idx];
                let found =
                    (1..boundary.len()).find(|&offset| boundary[(search_start + offset) % boundary.len()] == anchor_v);
                match found {
                    Some(offset) => search_start = (search_start + offset) % boundary.len(),
                    None => return Err(QuadTopologyError::InvalidAnchorEdge { edge: edge_idx - 1 }),
                }
            }
        }

        Ok(())
    }
}

struct VertexRingIter<'a, const CCW: bool> {
    topology: &'a QuadTopology,
    // Decremented on each loop completion; panics if reaches 0
    max_loops: usize,
    start: QuadVertex,
    current: QuadVertex,
    done: bool,
}

impl<'a, const CCW: bool> Iterator for VertexRingIter<'a, CCW> {
    type Item = QuadVertex;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let result = self.current;

        if !CCW {
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
