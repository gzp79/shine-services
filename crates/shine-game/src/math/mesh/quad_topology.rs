use crate::{
    indexed::{IdxVec, TypedIndex},
    math::geometry::angular_cmp,
};
use glam::Vec2;
use std::collections::HashMap;

crate::define_typed_index!(VertIdx, "Typed index into a vertex array.");
crate::define_typed_index!(QuadIdx, "Typed index into a quad array.");

/// Reference to a vertex within a specific quad.
///
/// Stores the quad and the local index (0..4) of the vertex in that quad.
/// - next neighbor: `quad_vertices(r.quad)[(r.local + 1) % 4]`
/// - prev neighbor: `quad_vertices(r.quad)[(r.local + 3) % 4]`
/// - opposite:      `quad_vertices(r.quad)[(r.local + 2) % 4]`
#[derive(Clone, Copy, Debug)]
pub struct QuadVertRef {
    pub quad: QuadIdx,
    pub local: u8,
}

/// Quad mesh topology with adjacency — no positions.
///
/// Stores quads, boundary flags, quad neighbors, vertex rings (with ghost quads
/// closing boundary rings), and provides position-parameterized operations.
///
/// Ghost quads are added for each boundary edge. Ghost vertices use negative
/// indices (`VertIdx::new_ghost(i)`) and have no real position.
pub struct QuadTopology {
    real_vertex_count: usize,
    ghost_vertex_count: usize,
    is_boundary: IdxVec<VertIdx, bool>,
    quads: IdxVec<QuadIdx, [VertIdx; 4]>,
    quad_neighbors: IdxVec<QuadIdx, [QuadIdx; 4]>,
    real_quad_count: usize,

    // CSR vertex ring
    vertex_ring_offsets: Vec<u32>,
    vertex_ring_data: Vec<QuadVertRef>,
}

impl QuadTopology {
    /// Build topology from quads and boundary flags, adding ghost quads for boundary edges.
    pub fn new(vertex_count: usize, quads: Vec<[VertIdx; 4]>, is_boundary: Vec<bool>) -> Self {
        assert_eq!(is_boundary.len(), vertex_count);

        let real_quad_count = quads.len();
        let mut quads = IdxVec::from_vec(quads);
        let is_boundary = IdxVec::from_vec(is_boundary);

        // Build quad neighbors (real quads only first)
        let mut quad_neighbors = Self::build_quad_neighbors(&quads);

        // Add ghost quads for boundary edges
        let ghost_vertex_count = Self::add_ghost_quads(&mut quads, &mut quad_neighbors);

        // Build vertex rings (only real vertices get rings; ghost quads appear in real vertex rings)
        let (vertex_ring_offsets, vertex_ring_data) = Self::build_vertex_rings(vertex_count, &quads);

        Self {
            real_vertex_count: vertex_count,
            ghost_vertex_count,
            is_boundary,
            quads,
            quad_neighbors,
            real_quad_count,
            vertex_ring_offsets,
            vertex_ring_data,
        }
    }

    pub fn real_vertex_count(&self) -> usize {
        self.real_vertex_count
    }

    pub fn ghost_vertex_count(&self) -> usize {
        self.ghost_vertex_count
    }

    pub fn real_quad_count(&self) -> usize {
        self.real_quad_count
    }

    pub fn quad_count(&self) -> usize {
        self.quads.len()
    }

    pub fn quad_vertices(&self, qi: QuadIdx) -> [VertIdx; 4] {
        self.quads[qi]
    }

    pub fn is_ghost_quad(&self, qi: QuadIdx) -> bool {
        qi.into_index() >= self.real_quad_count
    }

    /// Neighbor across edge `k` (0..4) of quad `qi`.
    pub fn quad_neighbor(&self, qi: QuadIdx, edge: usize) -> QuadIdx {
        debug_assert!(edge < 4);
        self.quad_neighbors[qi][edge]
    }

    /// The ring of quads around vertex `vi`, with local vertex indices.
    /// Ghost vertices return an empty ring.
    pub fn vertex_ring(&self, vi: VertIdx) -> &[QuadVertRef] {
        if vi.is_ghost() {
            return &[];
        }
        let idx = vi.into_index();
        let start = self.vertex_ring_offsets[idx] as usize;
        let end = self.vertex_ring_offsets[idx + 1] as usize;
        &self.vertex_ring_data[start..end]
    }

    pub fn is_boundary_vertex(&self, vi: VertIdx) -> bool {
        if vi.is_ghost() {
            return true;
        }
        self.is_boundary[vi]
    }

    pub fn is_boundary_edge(&self, qi: QuadIdx, edge: usize) -> bool {
        self.quad_neighbor(qi, edge).is_none()
    }

    /// Returns boundary edges as pairs of real vertex indices `[a, b]`.
    /// A boundary edge is a real quad edge whose neighbor is a ghost quad.
    pub fn border_edges(&self) -> Vec<[u32; 2]> {
        let mut edges = Vec::new();
        for qi in self.real_quad_indices() {
            let verts = self.quads[qi];
            for k in 0..4 {
                let neighbor = self.quad_neighbors[qi][k];
                if self.is_ghost_quad(neighbor) {
                    let a = verts[k].into_index() as u32;
                    let b = verts[(k + 1) % 4].into_index() as u32;
                    edges.push([a, b]);
                }
            }
        }
        edges
    }

    pub fn real_quad_indices(&self) -> impl Iterator<Item = QuadIdx> {
        (0..self.real_quad_count).map(QuadIdx::new)
    }

    pub fn quad_indices(&self) -> impl Iterator<Item = QuadIdx> {
        (0..self.quads.len()).map(QuadIdx::new)
    }

    pub fn vertex_indices(&self) -> impl Iterator<Item = VertIdx> {
        (0..self.real_vertex_count).map(VertIdx::new)
    }

    /// Average position of real edge neighbors of `vi` (via "next" in each ring quad).
    /// Ghost neighbors are skipped.
    pub fn neighbor_avg(&self, vi: VertIdx, positions: &[Vec2]) -> Vec2 {
        let ring = self.vertex_ring(vi);
        let mut sum = Vec2::ZERO;
        let mut count = 0u32;

        for r in ring {
            let next = self.quads[r.quad][(r.local as usize + 1) % 4];
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

    /// Sort each vertex's quad ring rotationally (CCW by quad centroid angle).
    pub fn sort_vertex_rings(&mut self, positions: &[Vec2]) {
        for vi in 0..self.real_vertex_count {
            let start = self.vertex_ring_offsets[vi] as usize;
            let end = self.vertex_ring_offsets[vi + 1] as usize;
            if end - start <= 1 {
                continue;
            }

            let v_pos = positions[vi];
            let ring = &mut self.vertex_ring_data[start..end];
            let quads = &self.quads;

            ring.sort_by(|a, b| {
                let da = quad_centroid_partial(a.quad, quads, positions) - v_pos;
                let db = quad_centroid_partial(b.quad, quads, positions) - v_pos;
                angular_cmp(da, db)
            });
        }
    }

    fn build_quad_neighbors(quads: &IdxVec<QuadIdx, [VertIdx; 4]>) -> IdxVec<QuadIdx, [QuadIdx; 4]> {
        let mut neighbors = IdxVec::from_elem([QuadIdx::NONE; 4], quads.len());
        let mut edge_map: HashMap<(usize, usize), (QuadIdx, usize)> = HashMap::new();

        for (qi, verts) in quads.iter_indexed() {
            for k in 0..4 {
                let ai = verts[k].into_index();
                let bi = verts[(k + 1) % 4].into_index();
                let edge_key = if ai < bi { (ai, bi) } else { (bi, ai) };

                if let Some(&(other_qi, other_k)) = edge_map.get(&edge_key) {
                    neighbors[qi][k] = other_qi;
                    neighbors[other_qi][other_k] = qi;
                } else {
                    edge_map.insert(edge_key, (qi, k));
                }
            }
        }

        neighbors
    }

    /// Add ghost quads for boundary edges. Returns ghost vertex count.
    fn add_ghost_quads(
        quads: &mut IdxVec<QuadIdx, [VertIdx; 4]>,
        quad_neighbors: &mut IdxVec<QuadIdx, [QuadIdx; 4]>,
    ) -> usize {
        let real_quad_count = quads.len();
        let mut ghost_idx = 0usize;

        // Collect boundary edges: (quad_idx, local_edge)
        let mut boundary_edges = Vec::new();
        for qi in 0..real_quad_count {
            let qi = QuadIdx::new(qi);
            for k in 0..4 {
                if quad_neighbors[qi][k].is_none() {
                    boundary_edges.push((qi, k));
                }
            }
        }

        for (qi, k) in boundary_edges {
            let verts = quads[qi];
            let a = verts[k]; // edge start
            let b = verts[(k + 1) % 4]; // edge end

            // Ghost quad: [g0, b, a, g1] — edge 2 (a→g1) and edge 0 (g0→b)
            // Shared edge is edge 1: b→a (reverse of real quad's a→b)
            let g0 = VertIdx::new_ghost(ghost_idx);
            let g1 = VertIdx::new_ghost(ghost_idx + 1);
            ghost_idx += 2;

            let ghost_qi = quads.push([g0, b, a, g1]);

            // Link real quad edge k ↔ ghost quad edge 1
            quad_neighbors[qi][k] = ghost_qi;
            quad_neighbors.push([QuadIdx::NONE; 4]);
            quad_neighbors[ghost_qi][1] = qi;
        }

        ghost_idx
    }

    fn build_vertex_rings(
        real_vertex_count: usize,
        quads: &IdxVec<QuadIdx, [VertIdx; 4]>,
    ) -> (Vec<u32>, Vec<QuadVertRef>) {
        // Count quads per vertex (only real vertices get rings)
        let mut counts = vec![0u32; real_vertex_count];
        for verts in quads.iter() {
            for &v in verts {
                if let Some(flat) = v.try_into_index() {
                    counts[flat] += 1;
                }
            }
        }

        let mut offsets = Vec::with_capacity(real_vertex_count + 1);
        let mut cumsum = 0u32;
        for &c in &counts {
            offsets.push(cumsum);
            cumsum += c;
        }
        offsets.push(cumsum);

        let none_ref = QuadVertRef { quad: QuadIdx::NONE, local: 0 };
        let mut cursors = offsets[..real_vertex_count].to_vec();
        let mut data = vec![none_ref; cumsum as usize];

        for (qi, verts) in quads.iter_indexed() {
            for (local, &v) in verts.iter().enumerate() {
                if let Some(flat) = v.try_into_index() {
                    let pos = cursors[flat] as usize;
                    data[pos] = QuadVertRef { quad: qi, local: local as u8 };
                    cursors[flat] += 1;
                }
            }
        }

        (offsets, data)
    }
}

/// Centroid of a quad, skipping ghost vertices (averages only real vertex positions).
fn quad_centroid_partial(qi: QuadIdx, quads: &IdxVec<QuadIdx, [VertIdx; 4]>, positions: &[Vec2]) -> Vec2 {
    let verts = quads[qi];
    let mut sum = Vec2::ZERO;
    let mut count = 0u32;
    for &v in &verts {
        if let Some(idx) = v.try_into_index() {
            sum += positions[idx];
            count += 1;
        }
    }
    if count > 0 {
        sum / count as f32
    } else {
        Vec2::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shine_test::test;

    /// 2×2 grid of 4 quads, 9 vertices, 1 interior vertex (4):
    /// ```text
    ///  6----7----8
    ///  | Q2 | Q3 |
    ///  3----4----5
    ///  | Q0 | Q1 |
    ///  0----1----2
    /// ```
    /// Q0=[0,1,4,3]  Q1=[1,2,5,4]  Q2=[3,4,7,6]  Q3=[4,5,8,7]  (CCW)
    /// Interior: 4.  Boundary edges: 8 → 8 ghost quads → total 12.
    fn grid_2x2_topo() -> QuadTopology {
        let quads = vec![
            [VertIdx::new(0), VertIdx::new(1), VertIdx::new(4), VertIdx::new(3)],
            [VertIdx::new(1), VertIdx::new(2), VertIdx::new(5), VertIdx::new(4)],
            [VertIdx::new(3), VertIdx::new(4), VertIdx::new(7), VertIdx::new(6)],
            [VertIdx::new(4), VertIdx::new(5), VertIdx::new(8), VertIdx::new(7)],
        ];
        let is_boundary = vec![true, true, true, true, false, true, true, true, true];
        QuadTopology::new(9, quads, is_boundary)
    }

    #[test]
    fn test_counts() {
        let topo = grid_2x2_topo();
        assert_eq!(topo.real_vertex_count(), 9);
        assert_eq!(topo.real_quad_count(), 4);
        // 8 boundary edges → 8 ghost quads
        assert_eq!(topo.quad_count(), 4 + 8);
    }

    #[test]
    fn test_ghost_quads_close_rings() {
        let topo = grid_2x2_topo();
        for qi in topo.real_quad_indices() {
            for k in 0..4 {
                assert!(
                    topo.quad_neighbor(qi, k).is_real(),
                    "real quad {:?} edge {} still has NONE neighbor",
                    qi,
                    k
                );
            }
        }
    }

    #[test]
    fn test_vertex_ring_local_indices() {
        let topo = grid_2x2_topo();
        for vi in topo.vertex_indices() {
            for r in topo.vertex_ring(vi) {
                let verts = topo.quad_vertices(r.quad);
                assert_eq!(verts[r.local as usize], vi);
            }
        }
    }

    #[test]
    fn test_quad_vertices() {
        let topo = grid_2x2_topo();
        // Q0 bottom-left
        let v = topo.quad_vertices(QuadIdx::new(0));
        assert_eq!(v, [VertIdx::new(0), VertIdx::new(1), VertIdx::new(4), VertIdx::new(3)]);
        // Q3 top-right
        let v = topo.quad_vertices(QuadIdx::new(3));
        assert_eq!(v, [VertIdx::new(4), VertIdx::new(5), VertIdx::new(8), VertIdx::new(7)]);
    }

    #[test]
    fn test_neighbor_symmetry() {
        let topo = grid_2x2_topo();
        // Q0 edge 1 (1→4) shared with Q1 edge 3 (4→1)
        assert_eq!(topo.quad_neighbor(QuadIdx::new(0), 1), QuadIdx::new(1));
        assert_eq!(topo.quad_neighbor(QuadIdx::new(1), 3), QuadIdx::new(0));
        // Q0 edge 2 (4→3) shared with Q2 edge 0 (3→4)
        assert_eq!(topo.quad_neighbor(QuadIdx::new(0), 2), QuadIdx::new(2));
        assert_eq!(topo.quad_neighbor(QuadIdx::new(2), 0), QuadIdx::new(0));
    }

    #[test]
    fn test_boundary_vertex() {
        let topo = grid_2x2_topo();
        // All corners and edges are boundary; only center (4) is interior
        for i in [0usize, 1, 2, 3, 5, 6, 7, 8] {
            assert!(
                topo.is_boundary_vertex(VertIdx::new(i)),
                "vertex {i} should be boundary"
            );
        }
        assert!(!topo.is_boundary_vertex(VertIdx::new(4)), "vertex 4 should be interior");
    }

    #[test]
    fn test_border_edges() {
        let topo = grid_2x2_topo();
        let border = topo.border_edges();
        // 2x2 grid has 8 boundary edges (4 sides of perimeter)
        // Each edge is a pair of vertex indices [a, b]
        assert_eq!(border.len(), 8);

        // All border edge vertices should be boundary vertices
        for &[a, b] in &border {
            assert!(
                topo.is_boundary_vertex(VertIdx::new(a as usize)),
                "border edge vertex {a} should be boundary"
            );
            assert!(
                topo.is_boundary_vertex(VertIdx::new(b as usize)),
                "border edge vertex {b} should be boundary"
            );
        }

        // Check that the expected perimeter edges are present (unordered)
        // Bottom: 0-1, 1-2  Right: 2-5, 5-8  Top: 8-7, 7-6  Left: 6-3, 3-0
        let mut edge_set: std::collections::HashSet<(u32, u32)> = border
            .iter()
            .map(|&[a, b]| if a < b { (a, b) } else { (b, a) })
            .collect();
        for (a, b) in [(0, 1), (1, 2), (2, 5), (5, 8), (7, 8), (6, 7), (3, 6), (0, 3)] {
            assert!(edge_set.remove(&(a, b)), "expected border edge ({a}, {b}) not found");
        }
        assert!(edge_set.is_empty(), "unexpected extra border edges: {:?}", edge_set);
    }
}
