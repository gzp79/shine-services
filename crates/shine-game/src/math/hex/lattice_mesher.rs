use crate::{
    indexed::TypedIndex,
    math::{
        hex::{AxialCoord, AxialDenseIndexer},
        mesh::{QuadMesh, VertIdx},
        rand::StableRng,
    },
};
use glam::Vec2;
use std::collections::HashMap;

/// Generates a quad mesh by triangulating axial hex coordinates, then randomly
/// merging triangle pairs into quads, and finally subdividing all faces
/// (remaining triangles → 3 quads, merged quads → 4 quads) via centroid + edge midpoints.
pub struct LatticeMesher {
    subdivision: u32,
    hex_size: f32,
    rng: Box<dyn StableRng>,
}

impl LatticeMesher {
    pub fn new(subdivision: u32, rng: impl StableRng + 'static) -> Self {
        Self {
            subdivision,
            hex_size: 1.0,
            rng: Box::new(rng),
        }
    }

    #[must_use]
    pub fn with_hex_size(mut self, hex_size: f32) -> Self {
        self.hex_size = hex_size;
        self
    }

    /// Set the world-space circumradius (center to corner) of the hex.
    #[must_use]
    pub fn with_world_size(self, size: f32) -> Self {
        let radius = 2u32.pow(self.subdivision - 1);
        self.with_hex_size(AxialCoord::hex_size_from_world_size(size, radius))
    }

    pub fn generate(&mut self) -> QuadMesh {
        let radius = 2u32.pow(self.subdivision - 1);
        let indexer = AxialDenseIndexer::new(radius);

        // Step 1: Vertex positions
        let mut positions = vec![Vec2::ZERO; indexer.get_total_size()];
        let mut is_boundary = vec![false; indexer.get_total_size()];
        for coord in AxialCoord::origin().spiral(radius) {
            let idx = indexer.get_dense_index(&coord);
            positions[idx] = coord.vertex_position(self.hex_size);
            is_boundary[idx] = coord.is_boundary(radius);
        }

        // Step 2: Enumerate all triangles from the axial triangular lattice.
        // For each (q,r), emit two canonical triangles if all 3 vertices are in the hex:
        //   Tri A: (q,r), (q+1,r), (q+1,r-1)
        //   Tri B: (q,r), (q+1,r), (q,r+1)
        let mut triangles: Vec<[usize; 3]> = Vec::new();
        for coord in AxialCoord::origin().spiral(radius) {
            let a = coord;
            // Tri A: (q,r), (q+1,r), (q+1,r-1)
            let b_a = AxialCoord::new(coord.q + 1, coord.r);
            let c_a = AxialCoord::new(coord.q + 1, coord.r - 1);
            if b_a.distance(&AxialCoord::origin()) <= radius as i32
                && c_a.distance(&AxialCoord::origin()) <= radius as i32
            {
                triangles.push([
                    indexer.get_dense_index(&a),
                    indexer.get_dense_index(&b_a),
                    indexer.get_dense_index(&c_a),
                ]);
            }

            // Tri B: (q,r), (q+1,r), (q,r+1)
            let b_b = AxialCoord::new(coord.q + 1, coord.r);
            let c_b = AxialCoord::new(coord.q, coord.r + 1);
            if b_b.distance(&AxialCoord::origin()) <= radius as i32
                && c_b.distance(&AxialCoord::origin()) <= radius as i32
            {
                triangles.push([
                    indexer.get_dense_index(&a),
                    indexer.get_dense_index(&b_b),
                    indexer.get_dense_index(&c_b),
                ]);
            }
        }

        // Step 3: Build edge→triangle adjacency
        // edge key: (min_idx, max_idx) → list of triangle indices sharing that edge
        let mut edge_to_tris: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
        for (ti, tri) in triangles.iter().enumerate() {
            for k in 0..3 {
                let a = tri[k];
                let b = tri[(k + 1) % 3];
                let key = if a < b { (a, b) } else { (b, a) };
                edge_to_tris.entry(key).or_default().push(ti);
            }
        }

        // Collect interior edges (shared by exactly 2 triangles), sort for determinism, then shuffle
        let mut interior_edges: Vec<(usize, usize)> = edge_to_tris
            .iter()
            .filter(|(_, tris)| tris.len() == 2)
            .map(|(&key, _)| key)
            .collect();
        interior_edges.sort();

        // Fisher-Yates shuffle
        for i in (1..interior_edges.len()).rev() {
            let j = (self.rng.next_u32() as usize) % (i + 1);
            interior_edges.swap(i, j);
        }

        // Step 4: Greedily merge triangle pairs into quads
        let mut merged = vec![false; triangles.len()];
        // Faces: either a triangle [a,b,c] or a quad [a,b,c,d]
        let mut faces: Vec<Face> = Vec::new();

        for (ea, eb) in &interior_edges {
            let tris = &edge_to_tris[&(*ea, *eb)];
            let ti0 = tris[0];
            let ti1 = tris[1];
            if merged[ti0] || merged[ti1] {
                continue;
            }

            // Merge: form a quad from the two triangles sharing edge (ea, eb).
            // The quad vertices are the 4 unique vertices in order.
            if let Some(quad) = merge_triangles(&triangles[ti0], &triangles[ti1], *ea, *eb) {
                merged[ti0] = true;
                merged[ti1] = true;
                faces.push(Face::Quad(quad));
            }
        }

        // Remaining unmerged triangles
        for (ti, tri) in triangles.iter().enumerate() {
            if !merged[ti] {
                faces.push(Face::Tri(*tri));
            }
        }

        // Step 5: Subdivide all faces into quads via centroid + edge midpoints
        self.subdivide_faces(&positions, &is_boundary, &faces)
    }

    fn subdivide_faces(&self, base_positions: &[Vec2], base_is_boundary: &[bool], faces: &[Face]) -> QuadMesh {
        let base_count = base_positions.len();
        let mut positions: Vec<Vec2> = base_positions.to_vec();
        let mut is_boundary: Vec<bool> = base_is_boundary.to_vec();
        let mut quads: Vec<[VertIdx; 4]> = Vec::new();

        // Cache edge midpoints: (min_idx, max_idx) → vertex index
        let mut midpoint_cache: HashMap<(usize, usize), usize> = HashMap::new();

        let mut get_midpoint = |positions: &mut Vec<Vec2>, is_boundary: &mut Vec<bool>, a: usize, b: usize| -> usize {
            let key = if a < b { (a, b) } else { (b, a) };
            *midpoint_cache.entry(key).or_insert_with(|| {
                let idx = positions.len();
                positions.push((positions[a] + positions[b]) / 2.0);
                is_boundary.push(a < base_count && b < base_count && is_boundary[a] && is_boundary[b]);
                idx
            })
        };

        for face in faces {
            match *face {
                Face::Tri([a, b, c]) => {
                    let centroid_idx = positions.len();
                    positions.push((positions[a] + positions[b] + positions[c]) / 3.0);
                    is_boundary.push(false);

                    let m_ab = get_midpoint(&mut positions, &mut is_boundary, a, b);
                    let m_bc = get_midpoint(&mut positions, &mut is_boundary, b, c);
                    let m_ca = get_midpoint(&mut positions, &mut is_boundary, c, a);

                    quads.push([a, m_ab, centroid_idx, m_ca].map(VertIdx::new));
                    quads.push([b, m_bc, centroid_idx, m_ab].map(VertIdx::new));
                    quads.push([c, m_ca, centroid_idx, m_bc].map(VertIdx::new));
                }
                Face::Quad([a, b, c, d]) => {
                    let centroid_idx = positions.len();
                    positions.push((positions[a] + positions[b] + positions[c] + positions[d]) / 4.0);
                    is_boundary.push(false);

                    let m_ab = get_midpoint(&mut positions, &mut is_boundary, a, b);
                    let m_bc = get_midpoint(&mut positions, &mut is_boundary, b, c);
                    let m_cd = get_midpoint(&mut positions, &mut is_boundary, c, d);
                    let m_da = get_midpoint(&mut positions, &mut is_boundary, d, a);

                    quads.push([a, m_ab, centroid_idx, m_da].map(VertIdx::new));
                    quads.push([b, m_bc, centroid_idx, m_ab].map(VertIdx::new));
                    quads.push([c, m_cd, centroid_idx, m_bc].map(VertIdx::new));
                    quads.push([d, m_da, centroid_idx, m_cd].map(VertIdx::new));
                }
            }
        }

        let mut mesh = QuadMesh::new(positions, quads, is_boundary);
        mesh.sort_vertex_rings();
        mesh
    }
}

enum Face {
    Tri([usize; 3]),
    Quad([usize; 4]),
}

/// Merge two triangles sharing edge (ea, eb) into a quad.
/// Returns the 4 vertices in winding order, or None if degenerate.
fn merge_triangles(tri0: &[usize; 3], tri1: &[usize; 3], ea: usize, eb: usize) -> Option<[usize; 4]> {
    // Find the vertex in each triangle that is NOT on the shared edge
    let opposite0 = tri0.iter().copied().find(|&v| v != ea && v != eb)?;
    let opposite1 = tri1.iter().copied().find(|&v| v != ea && v != eb)?;

    // Build quad: opposite0, ea, opposite1, eb
    // This gives a consistent winding if the original triangles had consistent winding.
    // We need to check that the winding matches tri0.
    // In tri0, the shared edge goes ea→eb or eb→ea. The quad should follow tri0's winding.
    let ea_pos = tri0.iter().position(|&v| v == ea)?;
    let next_in_tri0 = tri0[(ea_pos + 1) % 3];

    if next_in_tri0 == eb {
        // tri0 winds ea → eb → opposite0
        // quad: opposite0 → ea → opposite1 → eb
        Some([opposite0, ea, opposite1, eb])
    } else {
        // tri0 winds ea → opposite0 → eb (i.e. eb → ea is the edge direction)
        // quad: opposite0 → eb → opposite1 → ea
        Some([opposite0, eb, opposite1, ea])
    }
}
