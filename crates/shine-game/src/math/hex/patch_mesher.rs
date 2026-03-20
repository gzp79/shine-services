use crate::math::{
    hex::{AxialCoord, AxialDenseIndexer, PatchCoord, PatchDenseIndexer, PatchOrientation},
    rand::{StableRng, StableRngExt},
};
use glam::Vec2;
use std::collections::HashSet;

/// Generates a quad mesh inside a hexagon using 3-patch subdivision.
pub struct PatchMesher {
    subdivision: u32,
    orientation: PatchOrientation,
    rng: Box<dyn StableRng>,
}

impl PatchMesher {
    pub fn new(subdivision: u32, orientation: PatchOrientation, rng: impl StableRng + 'static) -> Self {
        Self {
            subdivision,
            orientation,
            rng: Box::new(rng),
        }
    }

    fn radius(&self) -> u32 {
        2u32.pow(self.subdivision)
    }

    /// Create a zero-filled vertex buffer with the correct size for this mesh.
    pub fn create_vertex_buffer(&self) -> Vec<Vec2> {
        let indexer = AxialDenseIndexer::new(self.radius());
        vec![Vec2::ZERO; indexer.get_total_size()]
    }

    /// Place all vertices at their axial world positions without jitter or smoothing.
    /// Since `world_coordinate` is linear, every subdivision midpoint lands exactly
    /// at `coord.world_coordinate()`.
    pub fn generate_uniform(&mut self, vertices: &mut [Vec2]) {
        let radius = self.radius();
        let indexer = AxialDenseIndexer::new(radius);
        for coord in AxialCoord::origin().spiral(radius) {
            let idx = indexer.get_dense_index(&coord);
            vertices[idx] = coord.world_coordinate(1.0);
        }
    }

    /// Place hex corners and center, then recursively generate_subdivision each quad into 4
    /// by inserting edge midpoints and face centers.
    pub fn generate_subdivision(&mut self, vertices: &mut [Vec2]) {
        let radius = self.radius();
        let indexer = AxialDenseIndexer::new(radius);
        let orientation = self.orientation;

        // Place 6 hex corner vertices
        let hex_corners = AxialCoord::hex_corners(radius);
        for coord in &hex_corners {
            let idx = indexer.get_dense_index(coord);
            vertices[idx] = coord.world_coordinate(1.0);
        }

        // Place center at origin
        let center_idx = indexer.get_dense_index(&AxialCoord::origin());
        vertices[center_idx] = Vec2::ZERO;

        // Track which vertices have been placed
        let mut placed: HashSet<usize> = HashSet::new();
        for coord in &hex_corners {
            placed.insert(indexer.get_dense_index(coord));
        }
        placed.insert(center_idx);

        for depth in 0..self.subdivision {
            let parent_grid = 2i32.pow(depth);

            for p in 0..3 {
                let (a_idx, b_idx) = PatchCoord::new(p, 0, 0).hex_corner_indices(orientation);
                let ha = hex_corners[a_idx];
                let hb = hex_corners[b_idx];

                let parent_corner = |cu: i32, cv: i32| -> AxialCoord {
                    AxialCoord::new(
                        (cu * ha.q + cv * hb.q) / parent_grid,
                        (cu * ha.r + cv * hb.r) / parent_grid,
                    )
                };

                for pu in 0..parent_grid {
                    for pv in 0..parent_grid {
                        let corners = [
                            parent_corner(pu, pv),
                            parent_corner(pu + 1, pv),
                            parent_corner(pu + 1, pv + 1),
                            parent_corner(pu, pv + 1),
                        ];

                        // Edge midpoints
                        let edge_coords: [AxialCoord; 4] = std::array::from_fn(|i| {
                            let a = corners[i];
                            let b = corners[(i + 1) % 4];
                            AxialCoord::new((a.q + b.q) / 2, (a.r + b.r) / 2)
                        });
                        for edge_idx in 0..4 {
                            let mid_dense = indexer.get_dense_index(&edge_coords[edge_idx]);
                            if !placed.contains(&mid_dense) {
                                let a_pos = vertices[indexer.get_dense_index(&corners[edge_idx])];
                                let b_pos = vertices[indexer.get_dense_index(&corners[(edge_idx + 1) % 4])];
                                vertices[mid_dense] = (a_pos + b_pos) / 2.0;
                                placed.insert(mid_dense);
                            }
                        }

                        // Face point: intersection of lines connecting opposite edge midpoints
                        let e: [Vec2; 4] = std::array::from_fn(|i| vertices[indexer.get_dense_index(&edge_coords[i])]);
                        let face_pos = line_intersection(e[0], e[2], e[1], e[3]);
                        let face_coord = AxialCoord::new(
                            (corners[0].q + corners[1].q + corners[2].q + corners[3].q) / 4,
                            (corners[0].r + corners[1].r + corners[2].r + corners[3].r) / 4,
                        );
                        let face_dense = indexer.get_dense_index(&face_coord);
                        vertices[face_dense] = face_pos;
                        placed.insert(face_dense);
                    }
                }
            }
        }
    }

    /// Weighted Lloyd relaxation: assigns a random weight per quad, then iteratively
    /// moves each interior vertex toward the weighted centroid of its neighboring quads.
    /// Boundary vertices stay fixed. Does not enforce convexity —
    /// call `fix_quads` afterwards if needed.
    pub fn smooth_weighted_lloyd(
        &mut self,
        iterations: u32,
        strength: f32,
        weight_range: (f32, f32),
        vertices: &mut [Vec2],
    ) {
        let radius = self.radius();
        let indexer = AxialDenseIndexer::new(radius);
        let grid = 2i32.pow(self.subdivision);

        // Assign a random weight to each quad
        let total_quads = 3 * (grid * grid) as usize;
        let mut quad_weights: Vec<f32> = Vec::with_capacity(total_quads);
        for _ in 0..total_quads {
            quad_weights.push(self.rng.float_range(weight_range.0, weight_range.1));
        }

        // Build vertex-to-quads map (quad index + PatchCoord)
        let mut vert_quads: Vec<Vec<(usize, PatchCoord)>> = vec![Vec::new(); indexer.get_total_size()];
        let mut qi = 0;
        for p in 0..3 {
            for u in 0..grid {
                for v in 0..grid {
                    let patch = PatchCoord::new(p, u, v);
                    let quad = patch.quad_vertices(self.orientation, self.subdivision);
                    for coord in &quad {
                        vert_quads[indexer.get_dense_index(coord)].push((qi, patch));
                    }
                    qi += 1;
                }
            }
        }

        for _iter in 0..iterations {
            let old_vertices = vertices.to_vec();

            for coord in AxialCoord::origin().spiral(radius) {
                if coord.is_boundary(radius) {
                    continue;
                }

                let vert_idx = indexer.get_dense_index(&coord);
                let quads = &vert_quads[vert_idx];
                if quads.is_empty() {
                    continue;
                }

                let mut weighted_centroid = Vec2::ZERO;
                let mut total_weight = 0.0f32;

                for &(qi, patch) in quads {
                    let quad = patch.quad_vertices(self.orientation, self.subdivision);
                    let pts: [Vec2; 4] = std::array::from_fn(|i| old_vertices[indexer.get_dense_index(&quad[i])]);

                    // Shoelace area * random weight
                    let mut area = 0.0f32;
                    for i in 0..4 {
                        let j = (i + 1) % 4;
                        area += pts[i].x * pts[j].y - pts[j].x * pts[i].y;
                    }
                    area = area.abs() / 2.0;
                    let w = area * quad_weights[qi];

                    let centroid = (pts[0] + pts[1] + pts[2] + pts[3]) / 4.0;
                    weighted_centroid += centroid * w;
                    total_weight += w;
                }

                if total_weight > 1e-10 {
                    let target = weighted_centroid / total_weight;
                    let new_pos = old_vertices[vert_idx] + strength * (target - old_vertices[vert_idx]);
                    vertices[vert_idx] = new_pos;
                }
            }
        }
    }

    /// Coherent noise displacement: samples a 2D value noise field at each vertex
    /// and displaces it. Boundary vertices stay fixed. Does not enforce convexity —
    /// call `fix_quads` afterwards if needed.
    pub fn smooth_noise(&mut self, amplitude: f32, frequency: f32, vertices: &mut [Vec2]) {
        let radius = self.radius();
        let indexer = AxialDenseIndexer::new(radius);

        let seed_x = self.rng.next_u32();
        let seed_y = self.rng.next_u32();

        for coord in AxialCoord::origin().spiral(radius) {
            if coord.is_boundary(radius) {
                continue;
            }

            let idx = indexer.get_dense_index(&coord);
            let pos = vertices[idx];
            let nx = pos.x * frequency;
            let ny = pos.y * frequency;
            vertices[idx] = pos
                + Vec2::new(
                    value_noise_2d(nx, ny, seed_x) * amplitude,
                    value_noise_2d(nx, ny, seed_y) * amplitude,
                );
        }
    }

    /// Uniform random jitter: displaces each interior vertex by a random offset in x and y,
    /// clamped so the result stays inside the hex.
    /// Boundary vertices stay fixed. Does not enforce convexity —
    /// call `fix_quads` afterwards if needed.
    pub fn smooth_jitter(&mut self, amplitude: f32, vertices: &mut [Vec2]) {
        let radius = self.radius();
        let indexer = AxialDenseIndexer::new(radius);

        for coord in AxialCoord::origin().spiral(radius) {
            if coord.is_boundary(radius) {
                continue;
            }

            let idx = indexer.get_dense_index(&coord);
            let pos = vertices[idx];
            let dx = self.rng.float_signed() * amplitude;
            let dy = self.rng.float_signed() * amplitude;
            let candidate = pos + Vec2::new(dx, dy);
            if is_inside_hex(candidate.x, candidate.y, radius) {
                vertices[idx] = candidate;
            }
        }
    }

    /// Cotangent Laplacian smoothing with random per-quad weights.
    /// For each interior vertex, computes edge weights from the cotangent of opposite
    /// angles in adjacent quads (split into triangles), scaled by a random factor.
    /// Boundary vertices stay fixed. Does not enforce convexity —
    /// call `fix_quads` afterwards if needed.
    pub fn smooth_cotangent(&mut self, iterations: u32, strength: f32, vertices: &mut [Vec2]) {
        let radius = self.radius();
        let indexer = AxialDenseIndexer::new(radius);
        let grid = 2i32.pow(self.subdivision);

        // Assign a random weight per quad
        let total_quads = 3 * (grid * grid) as usize;
        let mut quad_weights: Vec<f32> = Vec::with_capacity(total_quads);
        for _ in 0..total_quads {
            quad_weights.push(self.rng.float_range(0.5, 1.5));
        }

        // Collect all quads as (quad_index, [4 dense vertex indices])
        let mut quads: Vec<(usize, [usize; 4])> = Vec::with_capacity(total_quads);
        let mut qi = 0;
        for p in 0..3 {
            for u in 0..grid {
                for v in 0..grid {
                    let patch = PatchCoord::new(p, u, v);
                    let coords = patch.quad_vertices(self.orientation, self.subdivision);
                    let indices: [usize; 4] = std::array::from_fn(|i| indexer.get_dense_index(&coords[i]));
                    quads.push((qi, indices));
                    qi += 1;
                }
            }
        }

        for _iter in 0..iterations {
            let old = vertices.to_vec();

            let n = indexer.get_total_size();
            let mut edge_sum = vec![Vec2::ZERO; n];
            let mut weight_sum = vec![0.0f32; n];

            for &(qi, vi) in &quads {
                let pts: [Vec2; 4] = std::array::from_fn(|i| old[vi[i]]);
                let rw = quad_weights[qi];

                for k in 0..4 {
                    let k1 = (k + 1) % 4;
                    let k2 = (k + 2) % 4;
                    let k3 = (k + 3) % 4;

                    let cot1 = cot_angle(pts[k], pts[k2], pts[k1]);
                    let cot2 = cot_angle(pts[k], pts[k3], pts[k1]);
                    let w = ((cot1 + cot2) / 2.0).max(0.01) * rw;

                    edge_sum[vi[k]] += w * pts[k1];
                    weight_sum[vi[k]] += w;

                    edge_sum[vi[k1]] += w * pts[k];
                    weight_sum[vi[k1]] += w;
                }
            }

            for coord in AxialCoord::origin().spiral(radius) {
                if coord.is_boundary(radius) {
                    continue;
                }
                let idx = indexer.get_dense_index(&coord);
                if weight_sum[idx] < 1e-10 {
                    continue;
                }
                let target = edge_sum[idx] / weight_sum[idx];
                vertices[idx] = old[idx] + strength * (target - old[idx]);
            }
        }
    }

    /// Spring-based force simulation: each quad has a random target centroid (its anchor)
    /// and vertices are pushed by three forces per iteration:
    /// 1. **Spring force**: pulls each quad's centroid toward its anchor, distributed to vertices.
    /// 2. **Angle force**: gradient-based force pushing each corner angle toward π/2.
    /// 3. **Area pressure**: outward force from centroid when quad area gets small, preventing
    ///    collapse and inversion.
    /// Boundary vertices stay fixed.
    pub fn smooth_spring(
        &mut self,
        iterations: u32,
        dt: f32,
        spring_strength: f32,
        shape_strength: f32,
        vertices: &mut [Vec2],
    ) {
        let radius = self.radius();
        let indexer = AxialDenseIndexer::new(radius);
        let grid = 2i32.pow(self.subdivision);
        let n = indexer.get_total_size();

        // Identify boundary vertices
        let mut is_boundary = vec![false; n];
        for coord in AxialCoord::origin().spiral(radius) {
            if coord.is_boundary(radius) {
                is_boundary[indexer.get_dense_index(&coord)] = true;
            }
        }

        // Collect quads as dense index arrays
        let total_quads = 3 * (grid * grid) as usize;
        let mut quads: Vec<[usize; 4]> = Vec::with_capacity(total_quads);
        for p in 0..3 {
            for u in 0..grid {
                for v in 0..grid {
                    let patch = PatchCoord::new(p, u, v);
                    let coords = patch.quad_vertices(self.orientation, self.subdivision);
                    quads.push(std::array::from_fn(|i| indexer.get_dense_index(&coords[i])));
                }
            }
        }

        // Snapshot original (uniform) edge rest lengths and reference area
        let mut rest_lengths: Vec<[f32; 4]> = Vec::with_capacity(total_quads);
        let mut total_area = 0.0f32;
        for vi in &quads {
            let pts: [Vec2; 4] = std::array::from_fn(|i| vertices[vi[i]]);
            rest_lengths.push(std::array::from_fn(|i| (pts[(i + 1) % 4] - pts[i]).length()));
            total_area += shoelace_area(&pts);
        }
        let ref_area = total_area / total_quads as f32;

        // Generate random anchor centroids: displace each quad's centroid randomly
        let hex_radius = AxialCoord::new(radius as i32, 0).world_coordinate(1.0).length();
        let anchors: Vec<Vec2> = (0..total_quads)
            .map(|qi| {
                let vi = &quads[qi];
                let centroid = (vertices[vi[0]] + vertices[vi[1]] + vertices[vi[2]] + vertices[vi[3]]) / 4.0;
                loop {
                    let dx = self.rng.float_signed() * hex_radius;
                    let dy = self.rng.float_signed() * hex_radius;
                    let anchor = centroid + Vec2::new(dx, dy) * 0.5;
                    if is_inside_hex(anchor.x, anchor.y, radius) {
                        break anchor;
                    }
                }
            })
            .collect();

        for _iter in 0..iterations {
            let mut forces = vec![Vec2::ZERO; n];

            for (qi, vi) in quads.iter().enumerate() {
                let pts: [Vec2; 4] = std::array::from_fn(|i| vertices[vi[i]]);
                let centroid = (pts[0] + pts[1] + pts[2] + pts[3]) / 4.0;

                // --- 1. Spring force: pull centroid toward anchor ---
                let spring_force = (anchors[qi] - centroid) * spring_strength;
                for i in 0..4 {
                    forces[vi[i]] += spring_force / 4.0;
                }

                // --- 2. Angle force (gradient-based) ---
                // At each corner i, with vectors v1 = prev-i, v2 = next-i:
                //   cos_a = dot(n1, n2) where n1=v1/|v1|, n2=v2/|v2|
                //   Gradient of cos_a²/2 gives forces that push angle toward π/2.
                for i in 0..4 {
                    let prev = (i + 3) % 4;
                    let next = (i + 1) % 4;
                    let v1 = pts[prev] - pts[i];
                    let v2 = pts[next] - pts[i];
                    let len1 = v1.length();
                    let len2 = v2.length();
                    if len1 < 1e-8 || len2 < 1e-8 {
                        continue;
                    }
                    let n1 = v1 / len1;
                    let n2 = v2 / len2;
                    let cos_a = n1.dot(n2);

                    // ∂cos_a/∂prev = (n2 - cos_a * n1) / len1
                    // ∂cos_a/∂next = (n1 - cos_a * n2) / len2
                    // Force = -cos_a * gradient (minimizes cos_a²/2)
                    let f_prev = -cos_a * (n2 - cos_a * n1) / len1 * shape_strength;
                    let f_next = -cos_a * (n1 - cos_a * n2) / len2 * shape_strength;
                    let f_i = -(f_prev + f_next);

                    forces[vi[prev]] += f_prev;
                    forces[vi[next]] += f_next;
                    forces[vi[i]] += f_i;
                }

                // --- 3. Edge spring: pull each edge toward its rest length ---
                // Edges exceeding 1.5× rest length get a much stiffer response.
                for i in 0..4 {
                    let next = (i + 1) % 4;
                    let edge = pts[next] - pts[i];
                    let len = edge.length();
                    if len < 1e-8 {
                        continue;
                    }
                    let rest = rest_lengths[qi][i];
                    let dir = edge / len;
                    let diff = len - rest;
                    let ratio = len / rest.max(1e-8);
                    let stiffness = if ratio > 1.5 {
                        shape_strength * (1.0 + (ratio - 1.5) * 4.0)
                    } else {
                        shape_strength * 0.25
                    };
                    let f = dir * diff * stiffness;
                    forces[vi[i]] += f;
                    forces[vi[next]] -= f;
                }

                // --- 4. Area pressure: prevent collapse ---
                // When area drops below reference, push vertices outward from centroid
                let area = shoelace_area(&pts);
                if area < ref_area * 0.5 {
                    let pressure = (ref_area * 0.5 - area) / ref_area * shape_strength * 4.0;
                    for i in 0..4 {
                        let outward = pts[i] - centroid;
                        let len = outward.length();
                        if len > 1e-8 {
                            forces[vi[i]] += (outward / len) * pressure;
                        }
                    }
                }
            }

            // Apply forces (skip boundary)
            for i in 0..n {
                if !is_boundary[i] {
                    vertices[i] += forces[i] * dt;
                }
            }
        }
    }

    /// Repair non-convex or low-quality quads by iterative local Laplacian relaxation.
    /// For each vertex in a bad quad, move it toward the average of its edge-connected
    /// neighbors. Repeat until all quads pass or `max_iterations` is reached.
    /// Boundary vertices stay fixed.
    pub fn fix_quads(&self, min_quality: f32, max_iterations: u32, vertices: &mut [Vec2]) {
        let radius = self.radius();
        let indexer = AxialDenseIndexer::new(radius);
        let grid = 2i32.pow(self.subdivision);

        // Collect quads as dense index arrays
        let mut quads: Vec<[usize; 4]> = Vec::new();
        for p in 0..3 {
            for u in 0..grid {
                for v in 0..grid {
                    let patch = PatchCoord::new(p, u, v);
                    let coords = patch.quad_vertices(self.orientation, self.subdivision);
                    quads.push(std::array::from_fn(|i| indexer.get_dense_index(&coords[i])));
                }
            }
        }

        // Build vertex-to-quads map
        let n = indexer.get_total_size();
        let mut vert_quads: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (qi, vi) in quads.iter().enumerate() {
            for &v in vi {
                vert_quads[v].push(qi);
            }
        }

        // Build edge-neighbor map: for each vertex, the set of vertices sharing a quad edge
        let mut neighbors: Vec<HashSet<usize>> = vec![HashSet::new(); n];
        for vi in &quads {
            for k in 0..4 {
                let a = vi[k];
                let b = vi[(k + 1) % 4];
                neighbors[a].insert(b);
                neighbors[b].insert(a);
            }
        }

        // Identify boundary vertices (immovable)
        let mut is_boundary = vec![false; n];
        for coord in AxialCoord::origin().spiral(radius) {
            if coord.is_boundary(radius) {
                is_boundary[indexer.get_dense_index(&coord)] = true;
            }
        }

        for _iter in 0..max_iterations {
            // Find bad quads and collect their non-boundary vertices
            let mut bad_verts: HashSet<usize> = HashSet::new();
            for vi in &quads {
                let pts: [Vec2; 4] = std::array::from_fn(|i| vertices[vi[i]]);
                if !is_quad_valid(&pts, min_quality) {
                    for &v in vi {
                        if !is_boundary[v] {
                            bad_verts.insert(v);
                        }
                    }
                }
            }

            if bad_verts.is_empty() {
                break;
            }

            // Move each bad vertex toward the average centroid of its adjacent quads.
            // Unlike edge-neighbor average, quad centroids pull vertices away from
            // degenerate collinear configurations.
            let old = vertices.to_vec();
            for &v in &bad_verts {
                let adj = &vert_quads[v];
                if adj.is_empty() {
                    continue;
                }
                let mut centroid_sum = Vec2::ZERO;
                for &qi in adj {
                    let vi = &quads[qi];
                    let c = (old[vi[0]] + old[vi[1]] + old[vi[2]] + old[vi[3]]) / 4.0;
                    centroid_sum += c;
                }
                let target = centroid_sum / adj.len() as f32;
                vertices[v] = old[v] + 0.5 * (target - old[v]);
            }
        }
    }

    /// Returns true if every quad in the mesh is convex and above `min_quality`.
    pub fn is_all_quads_valid(&self, min_quality: f32, vertices: &[Vec2]) -> bool {
        let radius = self.radius();
        let indexer = AxialDenseIndexer::new(radius);
        let patch_indexer = PatchDenseIndexer::new(self.subdivision);
        (0..patch_indexer.get_total_size()).all(|i| {
            let patch = patch_indexer.get_coord(i);
            let quad = patch.quad_vertices(self.orientation, self.subdivision);
            let pts: [Vec2; 4] = std::array::from_fn(|j| vertices[indexer.get_dense_index(&quad[j])]);
            is_quad_valid(&pts, min_quality)
        })
    }
}

/// Cotangent of the angle at vertex `b` in triangle (a, b, c).
fn cot_angle(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    let ba = a - b;
    let bc = c - b;
    let dot = ba.dot(bc);
    let cross = ba.perp_dot(bc).abs();
    if cross < 1e-10 {
        0.0
    } else {
        dot / cross
    }
}

/// Check if a quad is valid: convex and not degenerate (close to a triangle).
/// `min_quality` is the minimum allowed `area / max_edge²` (e.g. 0.15).
fn is_quad_valid(pts: &[Vec2; 4], min_quality: f32) -> bool {
    // Convexity: all cross products at corners must have the same sign
    let mut sign = None;
    for i in 0..4 {
        let a = pts[i];
        let b = pts[(i + 1) % 4];
        let c = pts[(i + 2) % 4];
        let cross = (b - a).perp_dot(c - b);
        if cross.abs() < 1e-10 {
            continue;
        }
        match sign {
            None => sign = Some(cross > 0.0),
            Some(s) => {
                if s != (cross > 0.0) {
                    return false;
                }
            }
        }
    }

    // Quality: area / max_edge² — catches triangle-like degenerate quads
    let mut area = 0.0f32;
    for i in 0..4 {
        let j = (i + 1) % 4;
        area += pts[i].x * pts[j].y - pts[j].x * pts[i].y;
    }
    area = area.abs() / 2.0;

    let mut max_edge_sq = 0.0f32;
    for i in 0..4 {
        let j = (i + 1) % 4;
        max_edge_sq = max_edge_sq.max((pts[j] - pts[i]).length_squared());
    }

    if max_edge_sq < 1e-20 {
        return false;
    }

    area / max_edge_sq >= min_quality
}

/// Check if a world-space point (x, y) is inside a pointy-top hex of the given axial radius.
/// Converts to fractional axial coords and checks hex distance < radius.
fn is_inside_hex(x: f32, y: f32, radius: u32) -> bool {
    const SQRT_3: f32 = 1.732050807568877_f32;
    // Inverse of world_coordinate (hex_size=1):
    //   x = sqrt(3) * (q + r/2),  y = 1.5 * r
    let r = y / 1.5;
    let q = x / SQRT_3 - r / 2.0;
    // Hex distance = max(|q|, |r|, |q+r|)
    let dist = q.abs().max(r.abs()).max((q + r).abs());
    dist < radius as f32
}

/// Signed area of a quad via the shoelace formula (absolute value).
fn shoelace_area(pts: &[Vec2; 4]) -> f32 {
    let mut area = 0.0f32;
    for i in 0..4 {
        let j = (i + 1) % 4;
        area += pts[i].x * pts[j].y - pts[j].x * pts[i].y;
    }
    area.abs() / 2.0
}

fn line_intersection(p1: Vec2, p2: Vec2, p3: Vec2, p4: Vec2) -> Vec2 {
    let d1 = p2 - p1;
    let d2 = p4 - p3;
    let cross = d1.perp_dot(d2);
    if cross.abs() < 1e-10 {
        (p1 + p2 + p3 + p4) / 4.0
    } else {
        let t = (p3 - p1).perp_dot(d2) / cross;
        p1 + t * d1
    }
}

/// Hash-based 2D value noise in [-1, 1] with smoothstep interpolation.
fn value_noise_2d(x: f32, y: f32, seed: u32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - x.floor();
    let fy = y - y.floor();

    // Smoothstep for C1 continuity
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);

    let n00 = hash_2d(ix, iy, seed);
    let n10 = hash_2d(ix + 1, iy, seed);
    let n01 = hash_2d(ix, iy + 1, seed);
    let n11 = hash_2d(ix + 1, iy + 1, seed);

    let nx0 = n00 + sx * (n10 - n00);
    let nx1 = n01 + sx * (n11 - n01);
    nx0 + sy * (nx1 - nx0)
}

/// Deterministic hash returning a float in [-1, 1].
fn hash_2d(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x85ebca6b);
    h = h.wrapping_add((y as u32).wrapping_mul(0xc2b2ae35));
    h = h.wrapping_add(seed);
    h = (h ^ (h >> 16)).wrapping_mul(0x85ebca6b);
    h = (h ^ (h >> 13)).wrapping_mul(0xc2b2ae35);
    h = h ^ (h >> 16);
    (h as f32 / u32::MAX as f32) * 2.0 - 1.0
}
