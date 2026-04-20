use crate::{
    indexed::{IdxVec, TypedIndex},
    math::quadrangulation::QuadError,
};
use glam::Vec2;

crate::define_typed_index!(VertIdx, "Typed index into a vertex array.");
crate::define_typed_index!(QuadIdx, "Typed index into a quad array.");

pub struct Vertex {
    pub position: Vec2,
    pub quad: QuadIdx,
}

impl Vertex {
    pub fn new() -> Self {
        Self {
            position: Vec2::ZERO,
            quad: QuadIdx::NONE,
        }
    }
}

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
    // For each vertex, stores position and one adjacent quad reference
    pub(crate) vertices: IdxVec<VertIdx, Vertex>,
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
        let quad = self.vertices[ghost].quad;
        let local = self.find_vertex(quad, ghost).unwrap();
        let start_qv = QuadVertex { quad, local };

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

    /// Find the local index (0..4) of vertex `v` in quad `qi`.
    /// Returns None if the vertex is not part of the quad.
    pub fn find_vertex(&self, qi: QuadIdx, v: VertIdx) -> Option<u8> {
        self.quads[qi].iter().position(|&x| x == v).map(|i| i as u8)
    }

    /// The ring of quads around vertex `vi`, with the local vertex index of vi.
    pub fn vertex_ring_ccw(&self, vi: VertIdx) -> impl Iterator<Item = QuadVertex> + '_ {
        let quad = self.vertices[vi].quad;
        let local = self.find_vertex(quad, vi).unwrap();
        let start_qv = QuadVertex { quad, local };

        VertexRingIter::<true> {
            topology: self,
            max_loops: 1,
            start: start_qv,
            current: start_qv,
            done: false,
        }
    }

    pub fn vertex_ring_cw(&self, vi: VertIdx) -> impl Iterator<Item = QuadVertex> + '_ {
        let quad = self.vertices[vi].quad;
        let local = self.find_vertex(quad, vi).unwrap();
        let start_qv = QuadVertex { quad, local };

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

    /// Validates the topology for consistency.
    pub fn validate(&self) -> Result<(), QuadError> {
        use crate::math::quadrangulation::Validator;
        Validator::new(self).validate()
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
