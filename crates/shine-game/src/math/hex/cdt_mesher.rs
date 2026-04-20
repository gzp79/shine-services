use crate::{
    indexed::TypedIndex,
    math::{
        hex::AxialCoord,
        prng::StableRng,
        quadrangulation::{self, Quadrangulation},
        triangulation::{self, Rot3Idx, Triangulation},
    },
};
use glam::{IVec2, Vec2};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// CDT grid scale to convert world to integer grid
const GRID_SCALE: i64 = 1000;
/// CDT integer x-scale: `x_cdt = X_SCALE * (2q + r)`  (rounded: √3/2 * 1000 ≈ 866.025)
const X_SCALE: i64 = 866;
/// CDT integer y-scale: `y_cdt = Y_SCALE * r`  (exact: 1.5 * 1000)
const Y_SCALE: i64 = 1500;

/// Generates a quad mesh inside a hexagon using CDT triangulation.
///
/// Algorithm:
/// 1. Place hex corners and subdivided edge points on an integer grid.
/// 2. Scatter random interior points with minimum distance enforcement.
/// 3. Run CDT with hex boundary edges as constraints.
/// 4. Split each triangle into 3 quads via centroid-to-edge-midpoint connections.
///
/// Returns a [`Quadrangulation`] with topology and positions. No smoothing or
/// filtering is applied — use filters on the returned mesh.
pub struct CdtMesher {
    subdivision: u32,
    interior_points: u32,
    hex_size: f32,
    rng: Rc<RefCell<dyn StableRng>>,
}

impl CdtMesher {
    /// Create a new CDT mesher.
    ///
    /// - `subdivision`: number of segments per hex edge (e.g. 4 → 4 segments, 24 boundary points)
    /// - `interior_points`: target number of random interior points
    pub fn new(subdivision: u32, interior_points: u32, rng: Rc<RefCell<dyn StableRng>>) -> Self {
        Self {
            subdivision,
            interior_points,
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

    /// Generate the CDT-based quad mesh.
    pub fn generate(&mut self) -> Quadrangulation {
        // Step 1: Compute boundary points and internal points on integer grid
        // CDT coords (flat-top vertex positions):
        //   `x = X_SCALE * (2q + r)`,
        //   `y = Y_SCALE * r`,
        // where q,r are the axial coordinates. Scales are expressed as const integers including
        // the rounded irrational parts, so this allows performing CDT using only stable integer math.

        let boundary_points = self.hex_boundary_points();
        let boundary_count = boundary_points.len();
        let interior_points = self.random_interior_points();
        let mut all_points: Vec<IVec2> = Vec::with_capacity(boundary_count + interior_points.len());
        all_points.extend_from_slice(&boundary_points);
        all_points.extend_from_slice(&interior_points);

        // Step 2: CDT triangulation with boundary constraint edges
        let mut tri = Triangulation::new_cdt();
        let mut builder = tri.builder();

        // Add all vertices
        let mut vertex_indices: Vec<triangulation::VertexIndex> = Vec::with_capacity(all_points.len());
        for &p in &all_points {
            let vi = builder.add_vertex(p, None);
            vertex_indices.push(vi);
        }

        // Add boundary constraint edges
        for i in 0..boundary_count {
            let v0 = vertex_indices[i];
            let v1 = vertex_indices[(i + 1) % boundary_count];
            builder.add_constraint_edge(v0, v1, 1);
        }
        drop(builder);

        // Extract finite triangles
        let mut triangles: Vec<(usize, usize, usize)> = Vec::new();
        for f in tri.face_index_iter() {
            if tri.is_infinite_face(f) {
                continue;
            }

            let v0 = tri[f].vertices[Rot3Idx::new(0)];
            let v1 = tri[f].vertices[Rot3Idx::new(1)];
            let v2 = tri[f].vertices[Rot3Idx::new(2)];

            // Find the original indices
            let i0 = vertex_indices.iter().position(|&v| v == v0).unwrap();
            let i1 = vertex_indices.iter().position(|&v| v == v1).unwrap();
            let i2 = vertex_indices.iter().position(|&v| v == v2).unwrap();

            triangles.push((i0, i1, i2));
        }

        // Step 3: Convert CDT integer points to world-space f32.
        let scale = self.hex_size / GRID_SCALE as f32;
        let base_vertices: Vec<Vec2> = all_points
            .iter()
            .map(|p| Vec2::new(p.x as f32, p.y as f32) * scale)
            .collect();

        // Step 4: Split each triangle into 3 quads and build Quadrangulation
        self.split_triangles_to_quad_mesh(&base_vertices, &triangles, boundary_count)
    }

    fn hex_boundary_points(&self) -> Vec<IVec2> {
        let corners = AxialCoord::hex_corners(1);
        let n = 2u32.pow(self.subdivision - 1) as i32;

        let total = (6 * n) as usize;
        let mut points = Vec::with_capacity(total);

        for edge_idx in 0..6 {
            let a = corners[edge_idx];
            let b = corners[(edge_idx + 1) % 6];
            for k in 0..n {
                let q = n * a.q + k * (b.q - a.q);
                let r = n * a.r + k * (b.r - a.r);
                points.push(IVec2::new(
                    (X_SCALE * (2 * q + r) as i64) as i32,
                    (Y_SCALE * r as i64) as i32,
                ));
            }
        }
        debug_assert_eq!(points.len(), total);

        points
    }

    fn random_interior_points(&mut self) -> Vec<IVec2> {
        let target_count = self.interior_points as usize;
        if target_count == 0 {
            return Vec::new();
        }

        let n = 2u32.pow(self.subdivision - 1) as i64;
        let x_bound = 2 * X_SCALE * n;
        let y_bound = Y_SCALE * n;
        let hp_bound = 2 * n * X_SCALE * Y_SCALE;

        const INT_MIN_DIST: i64 = GRID_SCALE / 2;
        const INT_MIN_DIST_SQ: i64 = INT_MIN_DIST * INT_MIN_DIST;

        // Inset the hex boundary by INT_MIN_DIST so interior points stay away from edges.
        // For the y condition (|r| < n): edge normal is along y, so inset by INT_MIN_DIST directly.
        // For the oblique conditions: the edge normals are (Y_SCALE, ±X_SCALE) with length
        // sqrt(X_SCALE² + Y_SCALE²). The hp_bound is in units of X_SCALE·Y_SCALE, so the
        // inset in hp_bound units = INT_MIN_DIST · sqrt(X_SCALE² + Y_SCALE²).
        let edge_normal_len = ((X_SCALE * X_SCALE + Y_SCALE * Y_SCALE) as f64).sqrt() as i64;
        let y_bound_inset = y_bound - INT_MIN_DIST;
        let hp_bound_inset = hp_bound - INT_MIN_DIST * edge_normal_len;

        let mut interior = Vec::<IVec2>::with_capacity(target_count);
        let max_attempts = target_count * 20;
        let mut attempts = 0;

        while interior.len() < target_count && attempts < max_attempts {
            attempts += 1;

            // Sample uniformly in the CDT bounding rectangle of the hex
            let ix = (self.rng.next_u32() as i64 % (2 * x_bound) - x_bound) as i32;
            let iy = (self.rng.next_u32() as i64 % (2 * y_bound) - y_bound) as i32;

            // Inside the inset hex — 3 half-plane conditions (flat-top), shrunk by min distance:
            let ix64 = ix as i64;
            let iy64 = iy as i64;
            if iy64.abs() >= y_bound_inset
                || (ix64 * Y_SCALE - iy64 * X_SCALE).abs() >= hp_bound_inset
                || (ix64 * Y_SCALE + iy64 * X_SCALE).abs() >= hp_bound_inset
            {
                continue;
            }

            let candidate = IVec2::new(ix, iy);
            let too_close = interior.iter().any(|p: &IVec2| {
                let dx = (candidate.x - p.x) as i64;
                let dy = (candidate.y - p.y) as i64;
                dx * dx + dy * dy < INT_MIN_DIST_SQ
            });
            if too_close {
                continue;
            }

            interior.push(candidate);
        }

        interior
    }

    /// Split each triangle into 3 quads and build a Quadrangulation.
    fn split_triangles_to_quad_mesh(
        &self,
        base_vertices: &[Vec2],
        triangles: &[(usize, usize, usize)],
        boundary_count: usize,
    ) -> Quadrangulation {
        let mut positions: Vec<Vec2> = base_vertices.to_vec();
        let mut quads: Vec<[quadrangulation::VertexIndex; 4]> = Vec::with_capacity(triangles.len() * 3);

        // Cache edge midpoints: (min_idx, max_idx) -> vertex index
        let mut midpoint_cache: HashMap<(usize, usize), usize> = HashMap::new();

        let mut get_midpoint = |positions: &mut Vec<Vec2>, a: usize, b: usize| -> usize {
            let key = if a < b { (a, b) } else { (b, a) };
            *midpoint_cache.entry(key).or_insert_with(|| {
                let idx = positions.len();
                positions.push((positions[a] + positions[b]) / 2.0);
                idx
            })
        };

        for &(a, b, c) in triangles {
            let centroid_idx = positions.len();
            positions.push((positions[a] + positions[b] + positions[c]) / 3.0);

            let m_ab = get_midpoint(&mut positions, a, b);
            let m_bc = get_midpoint(&mut positions, b, c);
            let m_ca = get_midpoint(&mut positions, c, a);

            // 3 quads per triangle
            quads.push([a, m_ab, centroid_idx, m_ca].map(quadrangulation::VertexIndex::new));
            quads.push([b, m_bc, centroid_idx, m_ab].map(quadrangulation::VertexIndex::new));
            quads.push([c, m_ca, centroid_idx, m_bc].map(quadrangulation::VertexIndex::new));
        }

        // Build subdivided boundary polygon by inserting midpoints between base boundary vertices
        let mut polygon = Vec::with_capacity(boundary_count * 2);
        for i in 0..boundary_count {
            polygon.push(quadrangulation::VertexIndex::new(i));
            let mid_idx = get_midpoint(&mut positions, i, (i + 1) % boundary_count);
            polygon.push(quadrangulation::VertexIndex::new(mid_idx));
        }

        // The 6 hex corner vertices are still in the polygon at their original indices.
        let n = 2u32.pow(self.subdivision - 1) as usize;
        let anchors: Vec<quadrangulation::VertexIndex> = (0..6).map(|i| quadrangulation::VertexIndex::new(i * n)).collect();

        Quadrangulation::from_polygon(polygon, anchors, quads, positions).expect("valid CDT mesh topology")
    }
}
