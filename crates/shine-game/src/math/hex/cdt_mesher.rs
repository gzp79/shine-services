use crate::{
    indexed::TypedIndex,
    math::{
        cdt::Triangulation,
        hex::AxialCoord,
        mesh::{QuadMesh, VertIdx},
        rand::StableRng,
    },
};
use glam::{IVec2, Vec2};
use std::collections::HashMap;

/// CDT grid scale to convet word to integer grid
const GRID_SCALE: i64 = 1000;
/// CDT integer x-scale: `x_cdt = X_SCALE * q`  (exact: 1.5 * 1000)
const X_SCALE: i64 = 1500;
/// CDT integer y-scale: `y_cdt = Y_SCALE * (2r + q)`  (rounded: √3/2 * 1000 ≈ 866.025)
const Y_SCALE: i64 = 866;

/// Generates a quad mesh inside a hexagon using CDT triangulation.
///
/// Algorithm:
/// 1. Place hex corners and subdivided edge points on an integer grid.
/// 2. Scatter random interior points with minimum distance enforcement.
/// 3. Run CDT with hex boundary edges as constraints.
/// 4. Split each triangle into 3 quads via centroid-to-edge-midpoint connections.
///
/// Returns a [`QuadMesh`] with topology and positions. No smoothing or
/// filtering is applied — use filters on the returned mesh.
pub struct CdtMesher {
    subdivision: u32,
    interior_points: u32,
    hex_size: f32,
    rng: Box<dyn StableRng>,
}

impl CdtMesher {
    /// Create a new CDT mesher.
    ///
    /// - `subdivision`: number of segments per hex edge (e.g. 4 → 4 segments, 24 boundary points)
    /// - `interior_points`: target number of random interior points
    pub fn new(subdivision: u32, interior_points: u32, rng: impl StableRng + 'static) -> Self {
        Self {
            subdivision,
            interior_points,
            hex_size: 1.0,
            rng: Box::new(rng),
        }
    }

    #[must_use]
    pub fn with_hex_size(mut self, hex_size: f32) -> Self {
        self.hex_size = hex_size;
        self
    }

    /// Set the world-space size of the hex, compensating for the axial radius
    /// so the hex extent stays constant regardless of subdivision count.
    /// `size = 1.0` matches a single-cell hex with `hex_size = 1.0`.
    #[must_use]
    pub fn with_world_size(self, size: f32) -> Self {
        let radius = 2u32.pow(self.subdivision - 1);
        self.with_hex_size(size / radius as f32)
    }

    /// Generate the CDT-based quad mesh.
    pub fn generate(&mut self) -> QuadMesh {
        // Step 1: Compute boundary points and internal points on integer grid
        // CDT coords:
        //   `x = x_world * GRId_SCALE = X_SCALE * q`,
        //   `y = y_world * GRID_SCALE = Y_SCALE * (2r + q)`,
        //  where q,r are the axial coordinates. Scales are express as const integers including
        //  the rounded irracional parts. so this allows to perform cdt using only stable integer math.

        let (boundary_points, boundary_edges) = self.hex_boundary_points();
        let boundary_count = boundary_points.len();
        let interior_points = self.random_interior_points();
        let mut all_points: Vec<IVec2> = Vec::with_capacity(boundary_count + interior_points.len());
        all_points.extend_from_slice(&boundary_points);
        all_points.extend_from_slice(&interior_points);

        // Step 2: CDT triangulation with boundary constraint edges
        let triangulation =
            Triangulation::build_with_edges(&all_points, &boundary_edges).expect("CDT triangulation failed");
        let triangles: Vec<(usize, usize, usize)> = triangulation.triangles().collect();

        // Step 3: Convert CDT integer points to world-space f32.
        let scale = self.hex_size / GRID_SCALE as f32;
        let base_vertices: Vec<Vec2> = all_points
            .iter()
            .map(|p| Vec2::new(p.x as f32, p.y as f32) * scale)
            .collect();

        // Step 4: Split each triangle into 3 quads and build QuadMesh
        self.split_triangles_to_quad_mesh(&base_vertices, &triangles, boundary_count)
    }

    fn hex_boundary_points(&self) -> (Vec<IVec2>, Vec<(usize, usize)>) {
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
                    (X_SCALE * q as i64) as i32,
                    (Y_SCALE * (2 * r + q) as i64) as i32,
                ));
            }
        }
        debug_assert_eq!(points.len(), total);

        let edges: Vec<(usize, usize)> = (0..total).map(|i| (i, (i + 1) % total)).collect();
        (points, edges)
    }

    fn random_interior_points(&mut self) -> Vec<IVec2> {
        let target_count = self.interior_points as usize;
        if target_count == 0 {
            return Vec::new();
        }

        let n = 2u32.pow(self.subdivision - 1) as i64;
        let x_bound = X_SCALE * n;
        let y_bound = 2 * Y_SCALE * n;
        let hp_bound = 2 * X_SCALE * Y_SCALE * n;

        const INT_MIN_DIST_SQ: i64 = GRID_SCALE * GRID_SCALE / 4;

        let mut interior = Vec::<IVec2>::with_capacity(target_count);
        let max_attempts = target_count * 20;
        let mut attempts = 0;

        while interior.len() < target_count && attempts < max_attempts {
            attempts += 1;

            // Sample uniformly in the CDT bounding rectangle of the hex
            let ix = (self.rng.next_u32() as i64 % (2 * x_bound) - x_bound) as i32;
            let iy = (self.rng.next_u32() as i64 % (2 * y_bound) - y_bound) as i32;

            // Strictly inside the hex — 3 half-plane conditions derived from the hex edge cross-products:
            //   1. `|ix| < X_SCALE·n`
            //   2. `|iy·X_SCALE − ix·Y_SCALE| < 2·n·X_SCALE·Y_SCALE`   (maps to |r| < n)
            //   3. `|ix·Y_SCALE + iy·X_SCALE| < 2·n·X_SCALE·Y_SCALE`   (maps to |q+r| < n)
            let ix64 = ix as i64;
            let iy64 = iy as i64;
            if ix64.abs() >= x_bound
                || (iy64 * X_SCALE - ix64 * Y_SCALE).abs() >= hp_bound
                || (ix64 * Y_SCALE + iy64 * X_SCALE).abs() >= hp_bound
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

    /// Split each triangle into 3 quads and build a QuadMesh.
    fn split_triangles_to_quad_mesh(
        &self,
        base_vertices: &[Vec2],
        triangles: &[(usize, usize, usize)],
        boundary_count: usize,
    ) -> QuadMesh {
        let mut positions: Vec<Vec2> = base_vertices.to_vec();
        let mut quads: Vec<[VertIdx; 4]> = Vec::with_capacity(triangles.len() * 3);
        let mut is_boundary: Vec<bool> = (0..positions.len()).map(|i| i < boundary_count).collect();

        // Cache edge midpoints: (min_idx, max_idx) -> vertex index
        let mut midpoint_cache: HashMap<(usize, usize), usize> = HashMap::new();

        let mut get_midpoint = |positions: &mut Vec<Vec2>, is_boundary: &mut Vec<bool>, a: usize, b: usize| -> usize {
            let key = if a < b { (a, b) } else { (b, a) };
            *midpoint_cache.entry(key).or_insert_with(|| {
                let idx = positions.len();
                positions.push((positions[a] + positions[b]) / 2.0);
                // Midpoint on a boundary edge (both endpoints are boundary) is also boundary
                is_boundary.push(a < boundary_count && b < boundary_count);
                idx
            })
        };

        for &(a, b, c) in triangles {
            // Centroid (never boundary)
            let centroid_idx = positions.len();
            positions.push((positions[a] + positions[b] + positions[c]) / 3.0);
            is_boundary.push(false);

            // Edge midpoints
            let m_ab = get_midpoint(&mut positions, &mut is_boundary, a, b);
            let m_bc = get_midpoint(&mut positions, &mut is_boundary, b, c);
            let m_ca = get_midpoint(&mut positions, &mut is_boundary, c, a);

            // 3 quads per triangle
            quads.push([a, m_ab, centroid_idx, m_ca].map(VertIdx::new));
            quads.push([b, m_bc, centroid_idx, m_ab].map(VertIdx::new));
            quads.push([c, m_ca, centroid_idx, m_bc].map(VertIdx::new));
        }

        let mut mesh = QuadMesh::new(positions, quads, is_boundary);
        mesh.sort_vertex_rings();
        mesh
    }
}
