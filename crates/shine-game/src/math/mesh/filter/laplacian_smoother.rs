use crate::{
    indexed::TypedIndex,
    math::mesh::QuadMesh,
};
use super::quad_filter::QuadFilter;
use glam::Vec2;

/// Laplacian smoothing for [`QuadMesh`].
///
/// [`apply`](QuadFilter::apply) runs `iterations` Jacobi-style relaxation
/// steps, moving interior vertices toward the average of their edge-connected
/// neighbors. Boundary vertices are never moved.
pub struct LaplacianSmoother {
    strength: f32,
    iterations: u32,
    buf: Vec<Vec2>,
}

impl LaplacianSmoother {
    pub fn new(strength: f32, iterations: u32) -> Self {
        debug_assert!((0.0..=1.0).contains(&strength));
        Self {
            strength,
            iterations,
            buf: Vec::new(),
        }
    }

    fn step(&mut self, mesh: &mut QuadMesh) {
        self.buf.resize(mesh.vertex_count(), Vec2::ZERO);

        for vi in mesh.vertex_indices() {
            self.buf[vi.into_index()] = mesh.position(vi);
        }

        for vi in mesh.vertex_indices() {
            if mesh.is_boundary_vertex(vi) {
                continue;
            }
            let avg = mesh.topology().neighbor_avg(vi, &self.buf);
            let old = self.buf[vi.into_index()];
            mesh.positions_mut()[vi] = old + self.strength * (avg - old);
        }
    }
}

impl QuadFilter for LaplacianSmoother {
    fn apply(&mut self, mesh: &mut QuadMesh) {
        for _ in 0..self.iterations {
            self.step(mesh);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{geometry::is_quad_well_shaped, mesh::{QuadRelax, VertIdx}};
    use shine_test::test;

    /// Build a 2×2 grid of 4 quads:
    /// ```text
    ///  6---7---8
    ///  |   |   |
    ///  3---4---5
    ///  |   |   |
    ///  0---1---2
    /// ```
    fn grid_2x2() -> QuadMesh {
        let positions = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(2.0, 1.0),
            Vec2::new(0.0, 2.0),
            Vec2::new(1.0, 2.0),
            Vec2::new(2.0, 2.0),
        ];
        let quads = vec![
            [VertIdx::new(0), VertIdx::new(1), VertIdx::new(4), VertIdx::new(3)],
            [VertIdx::new(1), VertIdx::new(2), VertIdx::new(5), VertIdx::new(4)],
            [VertIdx::new(3), VertIdx::new(4), VertIdx::new(7), VertIdx::new(6)],
            [VertIdx::new(4), VertIdx::new(5), VertIdx::new(8), VertIdx::new(7)],
        ];
        let is_boundary = vec![true, true, true, true, false, true, true, true, true];
        QuadMesh::new(positions, quads, is_boundary)
    }

    #[test]
    fn smooth_does_not_move_boundary() {
        let mut mesh = grid_2x2();
        let boundary_before: Vec<Vec2> = mesh
            .vertex_indices()
            .filter(|&vi| mesh.is_boundary_vertex(vi))
            .map(|vi| mesh.position(vi))
            .collect();

        let mut smoother = LaplacianSmoother::new(0.5, 10);
        smoother.apply(&mut mesh);

        let boundary_after: Vec<Vec2> = mesh
            .vertex_indices()
            .filter(|&vi| mesh.is_boundary_vertex(vi))
            .map(|vi| mesh.position(vi))
            .collect();

        assert_eq!(boundary_before, boundary_after);
    }

    #[test]
    fn smooth_moves_perturbed_interior() {
        let mut mesh = grid_2x2();
        mesh.positions_mut()[VertIdx::new(4)] = Vec2::new(1.3, 1.4);

        let mut smoother = LaplacianSmoother::new(0.5, 20);
        smoother.apply(&mut mesh);

        let pos = mesh.position(VertIdx::new(4));
        assert!((pos.x - 1.0).abs() < 0.01, "x = {}", pos.x);
        assert!((pos.y - 1.0).abs() < 0.01, "y = {}", pos.y);
    }

    #[test]
    fn quad_relax_repairs_bad_quad() {
        let mut mesh = grid_2x2();
        mesh.positions_mut()[VertIdx::new(4)] = Vec2::new(0.1, 0.1);

        let mut fixer = QuadRelax::new(0.15, 0.5, 50);
        fixer.apply(&mut mesh);

        for qi in mesh.quad_indices() {
            let verts = mesh.quad_vertices(qi);
            let pts: [Vec2; 4] = std::array::from_fn(|i| mesh.position(verts[i]));
            assert!(is_quad_well_shaped(&pts, 0.15), "quad {:?} still invalid after fix", qi);
        }
    }

    #[test]
    fn smooth_noop_on_regular_grid() {
        let mut mesh = grid_2x2();
        let before = mesh.position(VertIdx::new(4));

        let mut smoother = LaplacianSmoother::new(0.5, 5);
        smoother.apply(&mut mesh);

        let after = mesh.position(VertIdx::new(4));
        assert!((after - before).length() < 1e-6);
    }

    #[test]
    fn single_step_moves_perturbed_interior() {
        let mut mesh = grid_2x2();
        mesh.positions_mut()[VertIdx::new(4)] = Vec2::new(1.3, 1.4);

        let mut smoother = LaplacianSmoother::new(0.5, 1);
        smoother.apply(&mut mesh);

        let pos = mesh.position(VertIdx::new(4));
        assert!((pos - Vec2::new(1.0, 1.0)).length() < (Vec2::new(1.3, 1.4) - Vec2::new(1.0, 1.0)).length());
    }
}
