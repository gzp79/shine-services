use crate::{
    indexed::TypedIndex,
    math::{
        hex::{AxialCoord, AxialDenseIndexer},
        mesh::{QuadMesh, VertIdx},
        prng::StableRng,
    },
};
use glam::Vec2;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

/// Generates a quad mesh by triangulating axial hex coordinates, then randomly
/// merging triangle pairs into quads, and finally subdividing all faces
/// (remaining triangles → 3 quads, merged quads → 4 quads) via centroid + edge midpoints.
pub struct LatticeMesher {
    subdivision: u32,
    hex_size: f32,
    rng: Rc<RefCell<dyn StableRng>>,
}

impl LatticeMesher {
    pub fn new(subdivision: u32, rng: Rc<RefCell<dyn StableRng>>) -> Self {
        Self {
            subdivision,
            hex_size: 1.0,
            rng,
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
        let mut boundary_polygon = Vec::new();
        for coord in AxialCoord::origin().spiral(radius) {
            let idx = indexer.get_dense_index(&coord);
            positions[idx] = coord.vertex_position(self.hex_size);
            if coord.is_boundary(radius) {
                boundary_polygon.push(VertIdx::new(idx));
            }
        }

        // 6 hex corner vertices as anchors
        let hex_corners = AxialCoord::hex_corners(radius);
        let anchors: Vec<VertIdx> = hex_corners
            .iter()
            .map(|c| VertIdx::new(indexer.get_dense_index(c)))
            .collect();

        // Step 2: Enumerate all triangles from the axial triangular lattice.
        // Each vertex generates 6 triangles (one per adjacent neighbor pair).
        // Deduplicate via sorted canonical form.
        let origin = AxialCoord::origin();
        let mut triangle_set: HashSet<[usize; 3]> = HashSet::new();

        for coord in origin.spiral(radius) {
            let a_idx = indexer.get_dense_index(&coord);
            let neighbors: Vec<_> = coord.neighbors().collect();

            for i in 0..6 {
                let b = neighbors[i];
                let c = neighbors[(i + 1) % 6];

                if b.distance(&origin) <= radius as i32 && c.distance(&origin) <= radius as i32 {
                    let mut tri = [a_idx, indexer.get_dense_index(&b), indexer.get_dense_index(&c)];
                    tri.sort_unstable();
                    triangle_set.insert(tri);
                }
            }
        }

        // Convert to vec with proper CCW winding
        let triangles: Vec<[usize; 3]> = triangle_set
            .into_iter()
            .map(|[a, b, c]| {
                let signed_area = (positions[b] - positions[a]).perp_dot(positions[c] - positions[a]);
                if signed_area > 0.0 {
                    [a, b, c]
                } else {
                    [a, c, b]
                }
            })
            .collect();

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
        self.subdivide_faces(positions, &faces, boundary_polygon, anchors)
    }

    fn subdivide_faces(
        &self,
        mut positions: Vec<Vec2>,
        faces: &[Face],
        base_boundary_polygon: Vec<VertIdx>,
        anchors: Vec<VertIdx>,
    ) -> QuadMesh {
        let mut quads: Vec<[VertIdx; 4]> = Vec::new();

        // Cache edge midpoints: (min_idx, max_idx) → vertex index
        let mut midpoint_cache: HashMap<(usize, usize), usize> = HashMap::new();

        let mut get_midpoint = |positions: &mut Vec<Vec2>, a: usize, b: usize| -> usize {
            let key = if a < b { (a, b) } else { (b, a) };
            *midpoint_cache.entry(key).or_insert_with(|| {
                let idx = positions.len();
                positions.push((positions[a] + positions[b]) / 2.0);
                idx
            })
        };

        for face in faces {
            match *face {
                Face::Tri([a, b, c]) => {
                    let centroid_idx = positions.len();
                    positions.push((positions[a] + positions[b] + positions[c]) / 3.0);

                    let m_ab = get_midpoint(&mut positions, a, b);
                    let m_bc = get_midpoint(&mut positions, b, c);
                    let m_ca = get_midpoint(&mut positions, c, a);

                    quads.push([a, m_ab, centroid_idx, m_ca].map(VertIdx::new));
                    quads.push([b, m_bc, centroid_idx, m_ab].map(VertIdx::new));
                    quads.push([c, m_ca, centroid_idx, m_bc].map(VertIdx::new));
                }
                Face::Quad([a, b, c, d]) => {
                    let centroid_idx = positions.len();
                    positions.push((positions[a] + positions[b] + positions[c] + positions[d]) / 4.0);

                    let m_ab = get_midpoint(&mut positions, a, b);
                    let m_bc = get_midpoint(&mut positions, b, c);
                    let m_cd = get_midpoint(&mut positions, c, d);
                    let m_da = get_midpoint(&mut positions, d, a);

                    quads.push([a, m_ab, centroid_idx, m_da].map(VertIdx::new));
                    quads.push([b, m_bc, centroid_idx, m_ab].map(VertIdx::new));
                    quads.push([c, m_cd, centroid_idx, m_bc].map(VertIdx::new));
                    quads.push([d, m_da, centroid_idx, m_cd].map(VertIdx::new));
                }
            }
        }

        // Build subdivided boundary polygon by inserting midpoints between base boundary vertices
        let mut boundary_polygon = Vec::new();
        for i in 0..base_boundary_polygon.len() {
            let v0 = base_boundary_polygon[i].into_index();
            let v1 = base_boundary_polygon[(i + 1) % base_boundary_polygon.len()].into_index();
            boundary_polygon.push(VertIdx::new(v0));
            let mid_idx = get_midpoint(&mut positions, v0, v1);
            boundary_polygon.push(VertIdx::new(mid_idx));
        }

        QuadMesh::from_polygon(positions, boundary_polygon, anchors, quads).expect("valid lattice mesh topology")
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
