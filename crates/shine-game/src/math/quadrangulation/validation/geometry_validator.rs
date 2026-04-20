use crate::{
    indexed::TypedIndex,
    math::quadrangulation::{QuadError, QuadIdx, Validator},
};
use glam::Vec2;

impl<'a> Validator<'a> {
    pub fn validate_geometry(&self) -> Result<(), QuadError> {
        self.validate_quad_areas()?;
        self.validate_no_self_intersections()?;
        Ok(())
    }

    /// Validate that all finite quads have positive signed area (CCW winding).
    fn validate_quad_areas(&self) -> Result<(), QuadError> {
        for qi in self.topology.finite_quad_index_iter() {
            let verts = self.topology.quad_vertices(qi);
            let positions: [Vec2; 4] = std::array::from_fn(|i| self.topology[verts[i]].position);

            let area = quad_signed_area(&positions);
            if area <= 0.0 {
                return Err(QuadError::NegativeQuadArea { quad: qi.into_index() });
            }
        }
        Ok(())
    }

    /// Validate that no finite quad edges intersect with other finite quad edges.
    fn validate_no_self_intersections(&self) -> Result<(), QuadError> {
        let quads: Vec<QuadIdx> = self.topology.finite_quad_index_iter().collect();

        for (i, &qi) in quads.iter().enumerate() {
            let verts_i = self.topology.quad_vertices(qi);
            let positions_i: [Vec2; 4] = std::array::from_fn(|idx| self.topology[verts_i[idx]].position);

            for &qj in quads.iter().skip(i + 1) {
                let verts_j = self.topology.quad_vertices(qj);
                let positions_j: [Vec2; 4] = std::array::from_fn(|idx| self.topology[verts_j[idx]].position);

                // Check all edge pairs between the two quads
                for edge_i in 0..4 {
                    let a = positions_i[edge_i];
                    let b = positions_i[(edge_i + 1) % 4];

                    for edge_j in 0..4 {
                        let c = positions_j[edge_j];
                        let d = positions_j[(edge_j + 1) % 4];

                        // Skip if quads share a vertex (adjacent quads share edges)
                        if verts_i.contains(&verts_j[edge_j]) || verts_i.contains(&verts_j[(edge_j + 1) % 4]) {
                            continue;
                        }

                        if segments_intersect(a, b, c, d) {
                            return Err(QuadError::SelfIntersection {
                                quad1: qi.into_index(),
                                quad2: qj.into_index(),
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// Compute signed area of a quad using the shoelace formula.
/// Positive area indicates CCW winding.
fn quad_signed_area(positions: &[Vec2; 4]) -> f32 {
    let mut area = 0.0;
    for i in 0..4 {
        let j = (i + 1) % 4;
        area += positions[i].x * positions[j].y;
        area -= positions[j].x * positions[i].y;
    }
    area * 0.5
}

/// Check if two line segments (a,b) and (c,d) intersect.
/// Returns true only for proper intersections (not touching at endpoints).
fn segments_intersect(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let d1 = cross_2d(c - a, b - a);
    let d2 = cross_2d(d - a, b - a);
    let d3 = cross_2d(a - c, d - c);
    let d4 = cross_2d(b - c, d - c);

    // Proper intersection: points on opposite sides of each line
    if d1 * d2 < 0.0 && d3 * d4 < 0.0 {
        return true;
    }

    false
}

/// 2D cross product (returns z-component of 3D cross product).
fn cross_2d(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}
