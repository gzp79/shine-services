use crate::{
    indexed::TypedIndex,
    math::{
        hex::AxialCoord,
        prng::{StableRng, StableRngExt},
        quadrangulation::{self, Quadrangulation, VertexIndex as QuadVertexIndex},
        triangulation::{self, Rot3Idx, Triangulation},
        SQRT_3,
    },
};
use glam::{ivec2, IVec2, Vec2};
use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap},
    rc::Rc,
};
use tracing::info_span;

/// Integer resolution for CDT grid.
/// Independent of subdivision depth, this controls the resolution of the CDT triangulation.
const DENSE_RADIUS: u32 = 32768;

/// Interior point density scale relative to the boundary subdivision grid.
/// 1 = same spacing as boundary, 2 = half the spacing (4x more candidate cells), etc.
const INTERIOR_DENSITY: i32 = 2;

/// Map an axial coordinate to the CDT integer grid.
/// Uses a pointy-top layout scaled by 1024 fixed-point: x = sqrt(3)/2, y = -3/2.
fn axial_to_cdt_pos(p: AxialCoord) -> IVec2 {
    const SCL_X: i32 = 887; // SQRT_3 / 2.0 * 1024
    const SCL_Y: i32 = -1536; // 1.5 * 1024
    ivec2(SCL_X * (p.q - p.r), SCL_Y * (p.q + p.r))
}

/// Generates a quad mesh inside a hexagon using CDT triangulation.
pub struct CdtMesher {
    /// Source of randomness.
    rng: Rc<RefCell<dyn StableRng>>,
    /// Number of random interior points to add inside the hex boundary, in addition to the boundary vertices.
    interior_points: u32,
    /// The circumradius of the hex boundary, which controls the overall scale of the output mesh.
    size: f32,
    /// Hex patch radius in sub-cell units: 2^(subdivision-1)
    patch_radius: u32,
    /// CDT grid distance between adjacent boundary vertices (one sub-cell width)
    grid_step: i32,
}

impl CdtMesher {
    /// Create a new CDT mesher.
    pub fn new(subdivision: u32, interior_points: u32, rng: Rc<RefCell<dyn StableRng>>) -> Self {
        let patch_radius = 2u32.pow(subdivision - 1);
        debug_assert!(DENSE_RADIUS % patch_radius == 0);
        let grid_step = (DENSE_RADIUS / patch_radius) as i32;

        Self {
            interior_points,
            size: 1.0,
            rng,
            patch_radius,
            grid_step,
        }
    }

    #[must_use]
    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Generate the CDT-based quad mesh.
    pub fn generate(&mut self) -> Quadrangulation {
        let _span = info_span!("CdtMesher::generate").entered();

        let mut all_points = Vec::new();
        let corner_indices = self.create_boundary_points(&mut all_points);
        let boundary_count = all_points.len();
        self.create_interior_points(&mut all_points);

        let triangles = {
            let _span = info_span!("CdtMesher::triangulate").entered();
            let mut tri = Triangulation::new_cdt();
            let mut builder = tri.builder();

            let mut vertex_indices: Vec<triangulation::VertexIndex> = Vec::with_capacity(all_points.len());
            let mut tri_to_position: HashMap<triangulation::VertexIndex, usize> = HashMap::new();
            for &p in &all_points {
                let pos = axial_to_cdt_pos(p);
                let vi = builder.add_vertex(pos, None);
                tri_to_position.insert(vi, vertex_indices.len());
                vertex_indices.push(vi);
            }

            for i in 0..boundary_count {
                let v0 = vertex_indices[i];
                let v1 = vertex_indices[(i + 1) % boundary_count];
                builder.add_constraint_edge(v0, v1, 1);
            }
            drop(builder);

            let mut triangles: Vec<(usize, usize, usize)> = Vec::new();
            for f in tri.face_index_iter() {
                if tri.is_infinite_face(f) {
                    continue;
                }
                let v0 = tri[f].vertices[Rot3Idx::new(0)];
                let v1 = tri[f].vertices[Rot3Idx::new(1)];
                let v2 = tri[f].vertices[Rot3Idx::new(2)];
                triangles.push((tri_to_position[&v0], tri_to_position[&v1], tri_to_position[&v2]));
            }
            triangles
        };

        self.split_triangles_to_quad_mesh(all_points, triangles, boundary_count, corner_indices)
    }

    fn create_boundary_points(&self, points: &mut Vec<AxialCoord>) -> [usize; 6] {
        let _span = info_span!("CdtMesher::create_boundary_points").entered();
        // Corners in pointy-hex CCW order: E, NE, NW, W, SW, SE
        let corners = AxialCoord::ORIGIN.pointy().corners(DENSE_RADIUS);

        // Per-edge step deltas using flat-hex directions scaled by grid_step:
        //   E→NE: NW flat (-1,0), NE→NW: SW flat (-1,+1),
        //   NW→W: S flat (0,+1),  W→SW: SE flat (+1,0),
        //   SW→SE: NE flat (+1,-1), SE→E: N flat (0,-1)
        let edge_steps: [(i32, i32); 6] = [
            (-self.grid_step, 0),
            (-self.grid_step, self.grid_step),
            (0, self.grid_step),
            (self.grid_step, 0),
            (self.grid_step, -self.grid_step),
            (0, -self.grid_step),
        ];

        let base = points.len();
        let mut corner_indices = [0usize; 6];
        for (i, &(dq, dr)) in edge_steps.iter().enumerate() {
            corner_indices[i] = base + i * self.patch_radius as usize;
            let start = corners[i];
            for k in 0..self.patch_radius as i32 {
                points.push(AxialCoord::new(start.q + k * dq, start.r + k * dr));
            }
        }
        corner_indices
    }

    fn create_interior_points(&mut self, points: &mut Vec<AxialCoord>) {
        let _span = info_span!("CdtMesher::create_interior_points").entered();
        if self.interior_points == 0 {
            points.push(AxialCoord::ORIGIN);
            return;
        }

        let step = self.grid_step / INTERIOR_DENSITY;
        let max_coord = INTERIOR_DENSITY * self.patch_radius as i32 - 1;
        let slack = (self.interior_points / 10).min(100);
        let max_attempts = self.interior_points + slack;

        let mut seen: BTreeSet<AxialCoord> = BTreeSet::new();
        let mut rng = self.rng.borrow_mut();
        let mut attempts = 0u32;
        while seen.len() < self.interior_points as usize && attempts < max_attempts {
            attempts += 1;

            // Pick the first axis uniformly; derive the second axis range from it so
            // every candidate is guaranteed inside the hex — no rejection needed.
            let a = rng.i32_range(-max_coord, max_coord + 1);
            let b_min = (-max_coord).max(-max_coord - a);
            let b_max = max_coord.min(max_coord - a);
            let b = rng.i32_range(b_min, b_max + 1);

            // Randomly assign (a, b) to (q, r) or (r, q) to keep marginals symmetric.
            let (q, r) = if rng.next_u32() & 1 == 0 { (a, b) } else { (b, a) };

            seen.insert(AxialCoord::new(q * step, r * step));
        }
        points.extend(seen);
    }

    fn split_triangles_to_quad_mesh(
        &self,
        axial_positions: Vec<AxialCoord>,
        triangles: Vec<(usize, usize, usize)>,
        boundary_count: usize,
        corner_indices: [usize; 6],
    ) -> Quadrangulation {
        let _span = info_span!("CdtMesher::split_triangles_to_quad_mesh").entered();

        let patch_size = self.size * SQRT_3 / 3. / DENSE_RADIUS as f32;
        let mut positions: Vec<Vec2> = axial_positions
            .iter()
            .map(|&p| p.pointy().to_position(patch_size))
            .collect();
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

        // Build quads by splitting each triangle into 3 quads via centroid-to-edge-midpoint connections.
        for (a, b, c) in triangles {
            // store new (centroid) vertex
            let centroid_idx = positions.len();
            positions.push((positions[a] + positions[b] + positions[c]) / 3.0);

            // compute (cached) midpoints for the 3 edges
            let m_ab = get_midpoint(&mut positions, a, b);
            let m_bc = get_midpoint(&mut positions, b, c);
            let m_ca = get_midpoint(&mut positions, c, a);

            // 3 quads per triangle
            quads.push([a, m_ab, centroid_idx, m_ca].map(quadrangulation::VertexIndex::new));
            quads.push([b, m_bc, centroid_idx, m_ab].map(quadrangulation::VertexIndex::new));
            quads.push([c, m_ca, centroid_idx, m_bc].map(quadrangulation::VertexIndex::new));
        }

        // Build boundary polygon by subdividing each edge with a midpoint vertex.
        let mut polygon = Vec::new();
        for i in 0..boundary_count {
            polygon.push(QuadVertexIndex::new(i));
            polygon.push(QuadVertexIndex::NONE); // placeholder for midpoint
        }
        for i in 0..boundary_count {
            let v0 = polygon[2 * i].into_index();
            let v1 = polygon[(2 * (i + 1)) % polygon.len()].into_index();
            let mid_idx = get_midpoint(&mut positions, v0, v1);
            polygon[2 * i + 1] = QuadVertexIndex::new(mid_idx);
        }
        debug_assert!(polygon.iter().all(|v| v.is_valid()));

        let anchors = corner_indices.iter().map(|&i| QuadVertexIndex::new(i)).collect();
        Quadrangulation::from_polygon(positions, polygon, quads, anchors).expect("valid CDT mesh topology")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shine_test::test;

    /// Returns > 0 if (a, b, c) is CCW, < 0 if CW, 0 if collinear.
    fn orient2d(a: IVec2, b: IVec2, c: IVec2) -> i64 {
        (b - a).as_i64vec2().perp_dot((c - a).as_i64vec2())
    }

    /// Verify that all boundary points lie on the convex hull (no point is strictly inside
    /// the polygon formed by its neighbors), and that consecutive points are never collinear
    /// with a skip (which would indicate a missing subdivision point creating a degenerate edge).
    #[test]
    fn boundary_points_form_convex_hull() {
        for subdivision in 1u32..10 {
            let mesher = CdtMesher::new(subdivision, 0, crate::math::prng::XorShift32::new(1).into_rc());

            let mut points = Vec::new();
            mesher.create_boundary_points(&mut points);

            let n = points.len();
            assert_eq!(n, 6 * mesher.patch_radius as usize);

            let cdt: Vec<IVec2> = points.iter().map(|&p| axial_to_cdt_pos(p)).collect();

            for i in 0..n {
                let prev = cdt[(i + n - 1) % n];
                let curr = cdt[i];
                let next = cdt[(i + 1) % n];

                assert_ne!(curr, next);

                // Every turn must be CCW or straight (convex hull condition).
                let o = orient2d(prev, curr, next);
                assert!(o >= 0);
            }
        }
    }
}
