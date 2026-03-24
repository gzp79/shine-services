use crate::{indexed::TypedIndex, math::mesh::QuadMesh};
use super::quad_filter::QuadFilter;
use glam::Vec2;

/// Edge-length equalization relaxation for [`QuadMesh`].
///
/// Each iteration, every interior vertex moves toward the position where all its
/// incident edge lengths would be equal (their local average). Boundary vertices
/// are never moved.
///
/// The update for vertex v with edge-neighbors u₁…uₖ is:
/// ```text
///   L       = avg(|v − uᵢ|)
///   ideal   = avg(uᵢ + L · normalize(v − uᵢ))
///   v_new   = v + strength · (ideal − v)
/// ```
/// `ideal` is the centroid of the points at distance L from each neighbor in
/// the current direction. Moving toward it equalizes edge lengths from v without
/// requiring a global rest length or spring forces. `strength ≤ 0.5` is stable.
pub struct VertexRepulsion {
    strength: f32,
    iterations: u32,
    buf: Vec<Vec2>,
}

impl VertexRepulsion {
    pub fn new(strength: f32, iterations: u32) -> Self {
        Self { strength, iterations, buf: Vec::new() }
    }
}

impl QuadFilter for VertexRepulsion {
    fn apply(&mut self, mesh: &mut QuadMesh) {
        let n = mesh.vertex_count();
        if n == 0 {
            return;
        }

        for _ in 0..self.iterations {
            // Snapshot positions so all updates read from the frozen state.
            self.buf.resize(n, Vec2::ZERO);
            for vi in mesh.vertex_indices() {
                self.buf[vi.into_index()] = mesh.position(vi);
            }

            for vi in mesh.vertex_indices() {
                if mesh.is_boundary_vertex(vi) {
                    continue;
                }
                let i = vi.into_index();

                // Pass 1: compute local average edge length.
                let mut sum_len = 0.0f32;
                let mut count = 0u32;
                for r in mesh.vertex_ring(vi) {
                    let verts = mesh.quad_vertices(r.quad);
                    let vj = verts[(r.local as usize + 1) % 4];
                    let Some(j) = vj.try_into_index() else { continue };
                    let dist = (self.buf[i] - self.buf[j]).length();
                    if dist < 1e-6 {
                        continue;
                    }
                    sum_len += dist;
                    count += 1;
                }
                if count == 0 {
                    continue;
                }
                let avg_len = sum_len / count as f32;

                // Pass 2: compute ideal position — centroid of points at avg_len
                // from each neighbor along the current direction.
                let mut ideal_sum = Vec2::ZERO;
                let mut ideal_count = 0u32;
                for r in mesh.vertex_ring(vi) {
                    let verts = mesh.quad_vertices(r.quad);
                    let vj = verts[(r.local as usize + 1) % 4];
                    let Some(j) = vj.try_into_index() else { continue };
                    let delta = self.buf[i] - self.buf[j]; // direction: j → i
                    let dist = delta.length();
                    if dist < 1e-6 {
                        continue;
                    }
                    ideal_sum += self.buf[j] + avg_len * (delta / dist);
                    ideal_count += 1;
                }
                if ideal_count == 0 {
                    continue;
                }
                let ideal = ideal_sum / ideal_count as f32;
                mesh.positions_mut()[vi] = self.buf[i] + self.strength * (ideal - self.buf[i]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{indexed::TypedIndex, math::mesh::VertIdx};
    use shine_test::test;

    /// 1×2 strip of 2 quads:
    /// ```text
    /// 3---2---5
    /// |   |   |
    /// 0---1---4
    /// ```
    /// Interior: 1, 2. Boundary: 0, 3, 4, 5.
    fn two_interior_mesh(pos1: Vec2, pos2: Vec2) -> QuadMesh {
        let positions = vec![
            Vec2::new(0.0, 0.0), // 0 boundary
            pos1,                 // 1 interior
            pos2,                 // 2 interior
            Vec2::new(0.0, 1.0), // 3 boundary
            Vec2::new(2.0, 0.0), // 4 boundary
            Vec2::new(2.0, 1.0), // 5 boundary
        ];
        let quads = vec![
            [VertIdx::new(0), VertIdx::new(1), VertIdx::new(2), VertIdx::new(3)],
            [VertIdx::new(1), VertIdx::new(4), VertIdx::new(5), VertIdx::new(2)],
        ];
        let is_boundary = vec![true, false, false, true, true, true];
        QuadMesh::new(positions, quads, is_boundary)
    }

    #[test]
    fn boundary_vertices_do_not_move() {
        let mut mesh = two_interior_mesh(Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0));
        let boundary_before: Vec<Vec2> = mesh
            .vertex_indices()
            .filter(|&vi| mesh.is_boundary_vertex(vi))
            .map(|vi| mesh.position(vi))
            .collect();

        let mut filter = VertexRepulsion::new(0.2, 10);
        filter.apply(&mut mesh);

        let boundary_after: Vec<Vec2> = mesh
            .vertex_indices()
            .filter(|&vi| mesh.is_boundary_vertex(vi))
            .map(|vi| mesh.position(vi))
            .collect();

        assert_eq!(boundary_before, boundary_after);
    }

    #[test]
    fn nearby_vertices_are_pushed_apart() {
        // Vertices 1 and 2 placed very close; their edge is shorter than the
        // local average so the ideal position pushes them apart.
        let close = Vec2::new(1.0, 0.5);
        let mut mesh = two_interior_mesh(close, close + Vec2::new(0.05, 0.0));
        let before = (mesh.position(VertIdx::new(1)) - mesh.position(VertIdx::new(2))).length();

        let mut filter = VertexRepulsion::new(0.2, 20);
        filter.apply(&mut mesh);

        let after = (mesh.position(VertIdx::new(1)) - mesh.position(VertIdx::new(2))).length();
        assert!(after > before, "distance should increase: before={before}, after={after}");
    }

    #[test]
    fn all_boundary_mesh_no_panic() {
        let positions = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ];
        let quads = vec![[VertIdx::new(0), VertIdx::new(1), VertIdx::new(2), VertIdx::new(3)]];
        let is_boundary = vec![true, true, true, true];
        let mut mesh = QuadMesh::new(positions.clone(), quads, is_boundary);

        let mut filter = VertexRepulsion::new(0.2, 5);
        filter.apply(&mut mesh); // must not panic

        for (i, &expected) in positions.iter().enumerate() {
            assert_eq!(mesh.position(VertIdx::new(i)), expected);
        }
    }

    #[test]
    fn coincident_vertices_no_panic() {
        let same = Vec2::new(1.0, 0.5);
        let mut mesh = two_interior_mesh(same, same);

        let mut filter = VertexRepulsion::new(0.2, 10);
        filter.apply(&mut mesh); // must not panic
    }

    #[test]
    fn regular_mesh_does_not_move() {
        // A mesh with perfectly regular spacing — ideal == current, so no movement.
        let mut mesh = two_interior_mesh(Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0));
        // Interior vertices 1 and 2 are already at the symmetric positions
        // relative to their boundary neighbors; they should be stable.
        let pos1_before = mesh.position(VertIdx::new(1));
        let pos2_before = mesh.position(VertIdx::new(2));

        let mut filter = VertexRepulsion::new(0.2, 10);
        filter.apply(&mut mesh);

        let pos1_after = mesh.position(VertIdx::new(1));
        let pos2_after = mesh.position(VertIdx::new(2));
        assert!(
            (pos1_after - pos1_before).length() < 1e-4,
            "vertex 1 should barely move: {:?} → {:?}", pos1_before, pos1_after
        );
        assert!(
            (pos2_after - pos2_before).length() < 1e-4,
            "vertex 2 should barely move: {:?} → {:?}", pos2_before, pos2_after
        );
    }
}
