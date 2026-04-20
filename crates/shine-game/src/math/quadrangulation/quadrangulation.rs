use crate::{
    indexed::{IdxArray, IdxVec, TypedIndex},
    math::quadrangulation::{QuadClue, QuadEdge, QuadError, Rot4Idx, VertexClue},
};
use glam::Vec2;
use std::ops;

crate::define_typed_index!(VertexIndex, "Typed index into a vertex array.");
crate::define_typed_index!(QuadIndex, "Typed index into a quad array.");
crate::define_typed_index!(AnchorIndex, "Typed index into the anchor vertices array.");

pub struct Vertex {
    pub position: Vec2,
    pub quad: QuadIndex,
}

impl Vertex {
    pub fn new() -> Self {
        Self {
            position: Vec2::ZERO,
            quad: QuadIndex::NONE,
        }
    }
}

pub struct Quad {
    pub vertices: IdxArray<Rot4Idx, VertexIndex, 4>,
    pub neighbors: IdxArray<Rot4Idx, QuadIndex, 4>,
}

impl Quad {
    pub fn new() -> Self {
        Self {
            vertices: IdxArray::from_elem(VertexIndex::NONE),
            neighbors: IdxArray::from_elem(QuadIndex::NONE),
        }
    }

    pub fn with_vertices(a: VertexIndex, b: VertexIndex, c: VertexIndex, d: VertexIndex) -> Self {
        Self {
            vertices: IdxArray::from([a, b, c, d]),
            neighbors: IdxArray::from_elem(QuadIndex::NONE),
        }
    }

    pub fn find_vertex(&self, v: VertexIndex) -> Option<Rot4Idx> {
        self.vertices.iter().position(|&x| x == v).map(Rot4Idx::new)
    }

    pub fn find_neighbor(&self, q: QuadIndex) -> Option<Rot4Idx> {
        self.neighbors.iter().position(|&x| x == q).map(Rot4Idx::new)
    }
}

/// Quadrangulation with vertex positions and topological adjacency.
///
/// The mesh is extended with an infinite vertex and infinite quads to form a topologically
/// closed mesh. The infinite vertex acts as an apex connecting all boundary vertices,
/// enabling consistent CCW navigation around every vertex including boundary vertices.
pub struct Quadrangulation {
    pub(in crate::math::quadrangulation) infinite_vertex: VertexIndex,
    pub(in crate::math::quadrangulation) vertices: IdxVec<VertexIndex, Vertex>,
    pub(in crate::math::quadrangulation) quads: IdxVec<QuadIndex, Quad>,
    pub(in crate::math::quadrangulation) anchor_vertices: IdxVec<AnchorIndex, VertexIndex>,
}

impl Quadrangulation {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    pub fn clear(&mut self) {
        self.infinite_vertex = VertexIndex::NONE;
        self.quads.clear();
        self.vertices.clear();
        self.anchor_vertices.clear();
    }

    #[inline]
    pub fn infinite_vertex(&self) -> VertexIndex {
        self.infinite_vertex
    }

    #[inline]
    pub fn is_infinite_vertex(&self, id: VertexIndex) -> bool {
        id == self.infinite_vertex
    }

    #[inline]
    pub fn is_finite_vertex(&self, id: VertexIndex) -> bool {
        !self.is_infinite_vertex(id)
    }

    #[inline]
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    #[inline]
    pub fn finite_vertex_count(&self) -> usize {
        if self.vertices.len() == 0 {
            0
        } else {
            self.vertices.len() - 1
        }
    }

    #[inline]
    pub fn vertex_index_iter(&self) -> impl Iterator<Item = VertexIndex> {
        VertexIndex::range(VertexIndex::new(0), VertexIndex::new(self.vertices.len()))
    }

    #[inline]
    pub fn finite_vertex_index_iter(&self) -> impl Iterator<Item = VertexIndex> + '_ {
        (0..self.vertices.len())
            .map(VertexIndex::new)
            .filter(|&vi| !self.is_infinite_vertex(vi))
    }

    #[inline]
    pub fn vertex_iter(&self) -> impl Iterator<Item = &Vertex> + '_ {
        self.vertices.iter()
    }

    #[inline]
    pub fn finite_vertex_iter(&self) -> impl Iterator<Item = &Vertex> + '_ {
        self.vertices.iter().enumerate().filter_map(|(i, v)| {
            if self.is_finite_vertex(VertexIndex::new(i)) {
                Some(v)
            } else {
                None
            }
        })
    }

    #[inline]
    pub fn quad_count(&self) -> usize {
        self.quads.len()
    }

    #[inline]
    pub fn finite_quad_count(&self) -> usize {
        self.finite_quad_index_iter().count()
    }

    #[inline]
    pub fn infinite_quad_count(&self) -> usize {
        self.infinite_quad_index_iter().count()
    }

    #[inline]
    pub fn quad_iter(&self) -> impl Iterator<Item = &Quad> + '_ {
        self.quads.iter()
    }

    #[inline]
    pub fn finite_quad_iter(&self) -> impl Iterator<Item = &Quad> + '_ {
        self.quads
            .iter()
            .filter(|q| q.find_vertex(self.infinite_vertex).is_none())
    }

    #[inline]
    pub fn quad_index_iter(&self) -> impl Iterator<Item = QuadIndex> {
        QuadIndex::range(QuadIndex::new(0), QuadIndex::new(self.quads.len()))
    }

    #[inline]
    pub fn finite_quad_index_iter(&self) -> impl Iterator<Item = QuadIndex> + '_ {
        (0..self.quads.len())
            .map(QuadIndex::new)
            .filter(|&qi| !self.is_infinite_quad(qi))
    }

    #[inline]
    pub fn infinite_quad_index_iter(&self) -> impl Iterator<Item = QuadIndex> + '_ {
        (0..self.quads.len())
            .map(QuadIndex::new)
            .filter(|&qi| self.is_infinite_quad(qi))
    }

    #[inline]
    pub fn is_infinite_quad(&self, qi: QuadIndex) -> bool {
        let infinite = self.infinite_vertex();
        let verts = &self.quads[qi].vertices;
        verts.iter().any(|&v| v == infinite)
    }

    #[inline]
    pub fn is_finite_quad(&self, qi: QuadIndex) -> bool {
        !self.is_infinite_quad(qi)
    }

    pub fn is_boundary_vertex(&self, vi: VertexIndex) -> bool {
        self.vertex_ring_ccw(vi).any(|qv| self.is_infinite_quad(qv.quad))
    }

    pub fn boundary_vertex_count(&self) -> usize {
        self.infinite_quad_count() * 2
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

    pub fn boundary_vertices(&self) -> impl Iterator<Item = VertexIndex> + '_ {
        // Walk vertex ring around ghost using CW traversal for correct CCW boundary order.
        // For each ghost quad, emit the two boundary vertices going backward from ghost.
        let ghost = self.infinite_vertex();
        self.vertex_ring_cw(ghost).flat_map(move |qv| {
            let p1 = qv.prev();
            let p2 = p1.prev();
            [self.vi(p1), self.vi(p2)]
        })
    }

    pub fn anchor_count(&self) -> usize {
        self.anchor_vertices.len()
    }

    pub fn anchor_index_iter(&self) -> impl Iterator<Item = AnchorIndex> {
        AnchorIndex::range(AnchorIndex::new(0), AnchorIndex::new(self.anchor_vertices.len()))
    }

    pub fn anchor_vertex(&self, anchor_idx: AnchorIndex) -> VertexIndex {
        self.anchor_vertices[anchor_idx]
    }

    /// Returns an iterator over vertices along the given anchor edge.
    ///
    /// An anchor edge represents an original boundary edge (before subdivision).
    /// This iterates from the anchor vertex at `edge` to the next anchor vertex,
    /// following the boundary.
    pub fn anchor_edge(&self, edge: AnchorIndex) -> impl Iterator<Item = VertexIndex> + '_ {
        let start = self.anchor_vertices[edge];
        let next_idx = AnchorIndex::new((edge.into_index() + 1) % self.anchor_vertices.len());
        let end = self.anchor_vertices[next_idx];

        // Use a 2x-looped CW ring around ghost vertex so skip_while/take_while
        // handles the case where the anchor edge wraps past the ring origin.
        let ghost = self.infinite_vertex();
        self.vertex_ring_cw_repeated(ghost, 2)
            .flat_map(move |qv| {
                let p1 = qv.prev();
                let p2 = p1.prev();
                [self.vi(p1), self.vi(p2)]
            })
            .skip_while(move |&v| v != start)
            .take_while(move |&v| v != end)
            .chain(std::iter::once(end))
    }

    /// Average position of real edge neighbors of `vi` (via "next" in each ring quad).
    /// Ghost neighbors are skipped.
    pub fn neighbor_avg(&self, vi: VertexIndex, positions: &[Vec2]) -> Vec2 {
        assert_ne!(vi, self.infinite_vertex());

        let mut sum = Vec2::ZERO;
        let mut count = 0u32;

        for qv in self.vertex_ring_ccw(vi) {
            let next = self.vi(qv.next());
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

    /// Convert a VertexClue to a VertexIndex
    pub fn vi<T: Into<VertexClue>>(&self, id: T) -> VertexIndex {
        let clue: VertexClue = id.into();
        match clue {
            VertexClue::VertexIndex(vi) => vi,
            VertexClue::QuadVertex(quad, local) => self.quads[quad].vertices[local],
            VertexClue::EdgeStart(quad, edge) => self.quads[quad].vertices[edge],
            VertexClue::EdgeEnd(quad, edge) => self.quads[quad].vertices[edge.increment()],
        }
    }

    /// Convert a QuadClue to a QuadIndex
    pub fn qi<T: Into<QuadClue>>(&self, id: T) -> QuadIndex {
        let clue: QuadClue = id.into();
        match clue {
            QuadClue::QuadIndex(qi) => qi,
        }
    }

    /// Get position for a vertex clue
    pub fn p<T: Into<VertexClue>>(&self, id: T) -> Vec2 {
        let vi = self.vi(id);
        self.vertices[vi].position
    }

    /// Get mutable position reference for a vertex clue
    pub fn p_mut<T: Into<VertexClue>>(&mut self, id: T) -> &mut Vec2 {
        let vi = self.vi(id);
        &mut self.vertices[vi].position
    }

    /// Get average position of quad vertices.
    /// Returns None for infinite quads.
    pub fn dual_p<T: Into<QuadClue>>(&self, id: T) -> Option<Vec2> {
        let qi = self.qi(id);
        if self.is_infinite_quad(qi) {
            return None;
        }

        let verts = self.quad_vertices(qi);
        let mut sum = Vec2::ZERO;
        for &v in verts {
            sum += self.vertices[v].position;
        }

        Some(sum / 4.0)
    }
}

impl ops::Index<VertexIndex> for Quadrangulation {
    type Output = Vertex;

    #[inline]
    fn index(&self, v: VertexIndex) -> &Self::Output {
        &self.vertices[v]
    }
}

impl ops::IndexMut<VertexIndex> for Quadrangulation {
    #[inline]
    fn index_mut(&mut self, v: VertexIndex) -> &mut Self::Output {
        &mut self.vertices[v]
    }
}

impl ops::Index<VertexClue> for Quadrangulation {
    type Output = Vertex;

    #[inline]
    fn index(&self, v: VertexClue) -> &Self::Output {
        &self.vertices[self.vi(v)]
    }
}

impl ops::IndexMut<VertexClue> for Quadrangulation {
    #[inline]
    fn index_mut(&mut self, v: VertexClue) -> &mut Self::Output {
        let vi = self.vi(v);
        &mut self.vertices[vi]
    }
}

impl ops::Index<QuadIndex> for Quadrangulation {
    type Output = Quad;

    #[inline]
    fn index(&self, q: QuadIndex) -> &Self::Output {
        &self.quads[q]
    }
}

impl ops::IndexMut<QuadIndex> for Quadrangulation {
    #[inline]
    fn index_mut(&mut self, q: QuadIndex) -> &mut Self::Output {
        &mut self.quads[q]
    }
}

impl ops::Index<QuadClue> for Quadrangulation {
    type Output = Quad;

    #[inline]
    fn index(&self, q: QuadClue) -> &Self::Output {
        &self.quads[self.qi(q)]
    }
}

impl ops::IndexMut<QuadClue> for Quadrangulation {
    #[inline]
    fn index_mut(&mut self, q: QuadClue) -> &mut Self::Output {
        let qi = self.qi(q);
        &mut self.quads[qi]
    }
}
