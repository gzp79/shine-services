use crate::{
    indexed::TypedIndex,
    math::{
        cdt::Triangulation,
        geometry::is_inside_hex,
        hex::AxialCoord,
        mesh::{QuadMesh, VertIdx},
        rand::StableRng,
    },
};
use glam::{IVec2, Vec2};
use std::collections::HashMap;

/// Scale factor for converting world-space hex coordinates to integer CDT coordinates.
const GRID_SCALE: f32 = 1000.0;

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
    edge_subdivisions: u32,
    interior_points: u32,
    hex_size: f32,
    rng: Box<dyn StableRng>,
}

impl CdtMesher {
    /// Create a new CDT mesher.
    ///
    /// - `edge_subdivisions`: number of segments per hex edge (e.g. 4 → 4 segments, 24 boundary points)
    /// - `interior_points`: target number of random interior points
    pub fn new(edge_subdivisions: u32, interior_points: u32, rng: impl StableRng + 'static) -> Self {
        Self {
            edge_subdivisions: edge_subdivisions.max(1),
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
    /// so the hex extent stays constant regardless of edge subdivision count.
    /// `size = 1.0` matches a single-cell hex with `hex_size = 1.0`.
    #[must_use]
    pub fn with_world_size(self, size: f32) -> Self {
        let edge_subdivisions = self.edge_subdivisions;
        self.with_hex_size(size / edge_subdivisions as f32)
    }

    /// Generate the CDT-based quad mesh.
    pub fn generate(&mut self) -> QuadMesh {
        // Step 1: Compute boundary points on integer grid
        let (boundary_points, boundary_edges) = self.hex_boundary_points();
        let boundary_count = boundary_points.len();

        // Step 2: Generate random interior points
        let interior_points = self.random_interior_points();

        // Step 3: Combine all points
        let mut all_points: Vec<IVec2> = Vec::with_capacity(boundary_points.len() + interior_points.len());
        all_points.extend_from_slice(&boundary_points);
        all_points.extend_from_slice(&interior_points);

        // Step 4: CDT triangulation with boundary constraint edges
        let triangulation =
            Triangulation::build_with_edges(&all_points, &boundary_edges).expect("CDT triangulation failed");
        let triangles: Vec<(usize, usize, usize)> = triangulation.triangles().collect();

        // Step 5: Convert integer points to world-space f32
        let base_vertices: Vec<Vec2> = all_points
            .iter()
            .map(|p| Vec2::new(p.x as f32 / GRID_SCALE, p.y as f32 / GRID_SCALE))
            .collect();

        // Step 6: Split each triangle into 3 quads and build QuadMesh
        self.split_triangles_to_quad_mesh(&base_vertices, &triangles, boundary_count)
    }

    /// Hex world-space radius (distance from center to corner).
    fn hex_world_radius(&self) -> f32 {
        // The hex is sized so that boundary points span edge_subdivisions segments.
        // Use edge_subdivisions as the axial radius for world_coordinate.
        AxialCoord::new(self.edge_subdivisions as i32, 0)
            .world_coordinate(self.hex_size)
            .length()
    }

    /// Compute hex corner and subdivided edge points on the integer grid.
    /// Returns (points, constraint_edges) where edges form the closed hex boundary.
    fn hex_boundary_points(&self) -> (Vec<IVec2>, Vec<(usize, usize)>) {
        let corners = AxialCoord::hex_corners(self.edge_subdivisions);
        let corner_world: Vec<Vec2> = corners.iter().map(|c| c.world_coordinate(self.hex_size)).collect();

        let mut points = Vec::new();

        // For each hex edge, add the start corner + intermediate subdivision points.
        // The end corner is the start of the next edge, so we don't duplicate it.
        for edge_idx in 0..6 {
            let a = corner_world[edge_idx];
            let b = corner_world[(edge_idx + 1) % 6];

            for seg in 0..self.edge_subdivisions {
                let t = seg as f32 / self.edge_subdivisions as f32;
                let p = a + (b - a) * t;
                points.push(IVec2::new(
                    (p.x * GRID_SCALE).round() as i32,
                    (p.y * GRID_SCALE).round() as i32,
                ));
            }
        }

        // Constraint edges: consecutive boundary points forming a closed loop
        let n = points.len();
        let edges: Vec<(usize, usize)> = (0..n).map(|i| (i, (i + 1) % n)).collect();

        (points, edges)
    }

    /// Generate random interior points with minimum distance enforcement.
    ///
    /// Two coordinate spaces are used:
    /// - **World space** (`f32`): hex geometry, `is_inside_hex` test.
    /// - **Integer grid** (`i32`/`i64`): world × `GRID_SCALE`, used by CDT and distance checks.
    fn random_interior_points(&mut self) -> Vec<IVec2> {
        let target_count = self.interior_points as usize;
        if target_count == 0 {
            return Vec::new();
        }

        let world_circumradius = self.hex_world_radius();
        let world_segment = world_circumradius / self.edge_subdivisions as f32;

        let int_radius = (world_circumradius * GRID_SCALE) as i64;
        let int_min_dist = (world_segment * 0.5 * GRID_SCALE) as i64;
        let int_min_dist_sq = int_min_dist * int_min_dist;

        let mut interior = Vec::with_capacity(target_count);
        let max_attempts = target_count * 20;
        let mut attempts = 0;

        while interior.len() < target_count && attempts < max_attempts {
            attempts += 1;

            let ix = (self.rng.next_u32() as i64 % (2 * int_radius) - int_radius) as i32;
            let iy = (self.rng.next_u32() as i64 % (2 * int_radius) - int_radius) as i32;

            let wx = ix as f32 / GRID_SCALE;
            let wy = iy as f32 / GRID_SCALE;
            if !is_inside_hex(Vec2::new(wx, wy), world_circumradius, world_segment * 0.5) {
                continue;
            }

            // Minimum distance check in integer grid (avoids f32 precision loss)
            let candidate = IVec2::new(ix, iy);
            let too_close = interior.iter().any(|p: &IVec2| {
                let dx = (candidate.x - p.x) as i64;
                let dy = (candidate.y - p.y) as i64;
                dx * dx + dy * dy < int_min_dist_sq
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
