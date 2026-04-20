use std::ops;

use crate::{
    indexed::{IdxArray, IdxVec, TypedIndex},
    math::quadrangulation::{QuadClue, QuadEdge, QuadEdgeType, QuadError, QuadVertex, Rot4Idx, VertexClue},
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

pub struct Quad {
    pub vertices: IdxArray<Rot4Idx, VertIdx, 4>,
    pub neighbors: IdxArray<Rot4Idx, QuadIdx, 4>,
}

impl Quad {
    pub fn new() -> Self {
        Self {
            vertices: IdxArray::from_elem(VertIdx::NONE),
            neighbors: IdxArray::from_elem(QuadIdx::NONE),
        }
    }

    pub fn with_vertices(a: VertIdx, b: VertIdx, c: VertIdx, d: VertIdx) -> Self {
        Self {
            vertices: IdxArray::from([a, b, c, d]),
            neighbors: IdxArray::from_elem(QuadIdx::NONE),
        }
    }

    pub fn find_vertex(&self, v: VertIdx) -> Option<Rot4Idx> {
        self.vertices.iter().position(|&x| x == v).map(Rot4Idx::new)
    }

    pub fn find_neighbor(&self, q: QuadIdx) -> Option<Rot4Idx> {
        self.neighbors.iter().position(|&x| x == q).map(Rot4Idx::new)
    }
}

/// Quad mesh topology with adjacency — no positions.
///
/// The mesh is extended with an infinite vertex and infinite quads to form a topologically
/// closed mesh. The infinite vertex acts as an apex connecting all boundary vertices,
/// enabling consistent CCW navigation around every vertex including boundary vertices.
///
/// ## Infinite Topology
///
/// - **Infinite vertex**: Located at `VertIdx(vertex_count)`, has no geometric position
/// - **Infinite quads**: N/2 quads for N boundary vertices, each containing the infinite vertex
///   and 3 consecutive boundary vertices with reversed winding for twin edge adjacency
///
/// ## Navigation
///
/// - All vertices support full CCW ring traversal via `vertex_ring()`
/// - Boundary edges can be identified via `edge_type()` returning `EdgeType::Boundary`
/// - Infinite quads are detectable via `is_infinite_quad()` and should typically be excluded
///   from geometric operations
pub struct QuadTopology {
    // Number of finite vertices (excluding infinite vertex)
    pub(crate) vertex_count: usize,
    // Number of infinite quads (half the number of boundary edges)
    pub(crate) infinite_quad_count: usize,
    pub(crate) quads: IdxVec<QuadIdx, Quad>,
    // For each vertex, stores position and one adjacent quad reference
    pub(crate) vertices: IdxVec<VertIdx, Vertex>,
    // Start vertex for each anchour (non-subdivided) edge.
    pub(crate) anchor_vertices: Vec<VertIdx>,
}

impl QuadTopology {
    /// Number of all vertices (finite + infinite)
    #[inline]
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Number of finite (non-infinite) vertices
    #[inline]
    pub fn finite_vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// Iterator over all vertices (finite + infinite)
    #[inline]
    pub fn vertex_iter(&self) -> impl Iterator<Item = &Vertex> + '_ {
        self.vertices.iter()
    }

    /// Iterator over all vertex indices (finite + infinite)
    pub fn vertex_index_iter(&self) -> impl Iterator<Item = VertIdx> {
        VertIdx::range(VertIdx::new(0), VertIdx::new(self.vertices.len()))
    }

    /// Iterator over the finite (non-infinite) vertex indices
    pub fn vertex_indices(&self) -> impl Iterator<Item = VertIdx> {
        (0..self.vertex_count).map(VertIdx::new)
    }

    /// Infinite vertex index
    #[inline]
    pub fn infinite_vertex(&self) -> VertIdx {
        VertIdx::new(self.vertex_count)
    }

    #[inline]
    pub fn is_infinite_vertex(&self, id: VertIdx) -> bool {
        id == VertIdx::new(self.vertex_count)
    }

    #[inline]
    pub fn is_finite_vertex(&self, id: VertIdx) -> bool {
        !self.is_infinite_vertex(id)
    }

    pub fn is_boundary_vertex(&self, vi: VertIdx) -> bool {
        self.vertex_ring_ccw(vi).any(|qv| self.is_infinite_quad(qv.quad))
    }

    pub fn boundary_vertex_count(&self) -> usize {
        self.infinite_quad_count * 2
    }

    /// Returns an iterator over boundary edges as vertex index pairs.
    ///
    /// Boundary edges are the two edges of each ghost quad that don't touch the ghost vertex.
    pub fn boundary_edges(&self) -> impl Iterator<Item = [u32; 2]> + '_ {
        self.vertex_ring_cw(self.infinite_vertex()).flat_map(move |qv| {
            // The two edges not touching the ghost vertex
            let e1 = qv.local.increment();
            let e2 = qv.local.increment().increment();
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
        let ghost = self.infinite_vertex();
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
                if self.is_infinite_quad(qv.quad) || self.is_infinite_quad(neighbor.quad) {
                    return QuadEdgeType::Boundary;
                } else {
                    return QuadEdgeType::Interior;
                }
            }
        }

        QuadEdgeType::NotAnEdge
    }

    /// Number of all quads (real + ghost)
    #[inline]
    pub fn quad_count(&self) -> usize {
        self.quads.len()
    }

    /// Number of real (non-ghost) quads
    #[inline]
    pub fn finite_quad_count(&self) -> usize {
        self.quads.len() - self.infinite_quad_count
    }

    /// Iterator over all quad vertex arrays (real + ghost)
    #[inline]
    pub fn quad_iter(&self) -> impl Iterator<Item = &IdxArray<Rot4Idx, VertIdx, 4>> + '_ {
        self.quads.iter().map(|q| &q.vertices)
    }

    /// Iterator over all quad indices (real + ghost)
    pub fn quad_index_iter(&self) -> impl Iterator<Item = QuadIdx> {
        QuadIdx::range(QuadIdx::new(0), QuadIdx::new(self.quads.len()))
    }

    /// Iterator over finite (non-infinite) quad indices
    pub fn quad_indices(&self) -> impl Iterator<Item = QuadIdx> + '_ {
        (0..self.quads.len())
            .map(QuadIdx::new)
            .filter(|&qi| !self.is_infinite_quad(qi))
    }

    /// Number of infinite quads
    #[inline]
    pub fn infinite_quad_count(&self) -> usize {
        self.infinite_quad_count
    }

    /// Iterator over infinite quad indices
    pub fn infinite_quad_indices(&self) -> impl Iterator<Item = QuadIdx> + '_ {
        (0..self.quads.len())
            .map(QuadIdx::new)
            .filter(|&qi| self.is_infinite_quad(qi))
    }

    #[inline]
    pub fn is_infinite_quad(&self, qi: QuadIdx) -> bool {
        let infinite = self.infinite_vertex();
        let verts = &self.quads[qi].vertices;
        verts.iter().any(|&v| v == infinite)
    }

    #[inline]
    pub fn is_finite_quad(&self, qi: QuadIdx) -> bool {
        !self.is_infinite_quad(qi)
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
        let ghost = self.infinite_vertex();
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
        let neighbor_quad = self.quads[qe.quad].neighbors[qe.edge];
        let neighbor_edge = self.quads[neighbor_quad].find_neighbor(qe.quad).unwrap();
        QuadEdge {
            quad: neighbor_quad,
            edge: neighbor_edge,
        }
    }

    pub fn vertex_index(&self, qv: QuadVertex) -> VertIdx {
        self.quads[qv.quad].vertices[qv.local]
    }

    pub fn edge_vertices(&self, qe: QuadEdge) -> (VertIdx, VertIdx) {
        let quad = &self.quads[qe.quad].vertices;
        (quad[qe.edge], quad[qe.edge.increment()])
    }

    pub fn quad_vertices(&self, qi: QuadIdx) -> [VertIdx; 4] {
        let v = &self.quads[qi].vertices;
        [
            v[Rot4Idx::new(0)],
            v[Rot4Idx::new(1)],
            v[Rot4Idx::new(2)],
            v[Rot4Idx::new(3)],
        ]
    }

    /// Find the local index (0..4) of vertex `v` in quad `qi`.
    /// Returns None if the vertex is not part of the quad.
    pub fn find_vertex(&self, qi: QuadIdx, v: VertIdx) -> Option<Rot4Idx> {
        self.quads[qi].find_vertex(v)
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
        assert_ne!(vi, self.infinite_vertex());

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

    /// Convert a VertexClue to a VertIdx
    pub fn vi<T: Into<VertexClue>>(&self, id: T) -> VertIdx {
        let clue: VertexClue = id.into();
        match clue {
            VertexClue::VertexIndex(vi) => vi,
            VertexClue::QuadVertex(quad, local) => self.quads[quad].vertices[local],
            VertexClue::EdgeStart(quad, edge) => self.quads[quad].vertices[edge],
            VertexClue::EdgeEnd(quad, edge) => self.quads[quad].vertices[edge.increment()],
        }
    }

    /// Convert a QuadClue to a QuadIdx
    pub fn qi<T: Into<QuadClue>>(&self, id: T) -> QuadIdx {
        let clue: QuadClue = id.into();
        match clue {
            QuadClue::QuadIndex(qi) => qi,
        }
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
            let neighbor = self.topology.edge_twin(edge);
            self.current = neighbor.end(); // Use end to stay at the same vertex
        } else {
            // CCW: Move via incoming edge (backward around the vertex)
            let edge = self.current.incoming_edge();
            let neighbor = self.topology.edge_twin(edge);
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

impl ops::Index<VertIdx> for QuadTopology {
    type Output = Vertex;

    #[inline]
    fn index(&self, v: VertIdx) -> &Self::Output {
        &self.vertices[v]
    }
}

impl ops::IndexMut<VertIdx> for QuadTopology {
    #[inline]
    fn index_mut(&mut self, v: VertIdx) -> &mut Self::Output {
        &mut self.vertices[v]
    }
}

impl ops::Index<VertexClue> for QuadTopology {
    type Output = Vertex;

    #[inline]
    fn index(&self, v: VertexClue) -> &Self::Output {
        &self.vertices[self.vi(v)]
    }
}

impl ops::IndexMut<VertexClue> for QuadTopology {
    #[inline]
    fn index_mut(&mut self, v: VertexClue) -> &mut Self::Output {
        let vi = self.vi(v);
        &mut self.vertices[vi]
    }
}

impl ops::Index<QuadIdx> for QuadTopology {
    type Output = Quad;

    #[inline]
    fn index(&self, q: QuadIdx) -> &Self::Output {
        &self.quads[q]
    }
}

impl ops::IndexMut<QuadIdx> for QuadTopology {
    #[inline]
    fn index_mut(&mut self, q: QuadIdx) -> &mut Self::Output {
        &mut self.quads[q]
    }
}

impl ops::Index<QuadClue> for QuadTopology {
    type Output = Quad;

    #[inline]
    fn index(&self, q: QuadClue) -> &Self::Output {
        &self.quads[self.qi(q)]
    }
}

impl ops::IndexMut<QuadClue> for QuadTopology {
    #[inline]
    fn index_mut(&mut self, q: QuadClue) -> &mut Self::Output {
        let qi = self.qi(q);
        &mut self.quads[qi]
    }
}
