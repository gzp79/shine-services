use crate::{
    indexed::TypedIndex,
    math::quadrangulation::{AnchorIndex, QuadError, QuadIndex, Validator},
};
use glam::Vec2;

impl<'a> Validator<'a> {
    pub fn validate_geometry(&self) -> Result<(), QuadError> {
        self.validate_quad_areas()?;
        self.validate_no_self_intersections()?;
        Ok(())
    }

    /// Validate that anchor points form a regular flat-top hexagon in CW order
    /// and that each anchor edge has the expected number of subdivisions.
    ///
    /// Requirements:
    /// - Exactly 6 anchor points
    /// - Points form a regular hexagon (equal side lengths and angles)
    /// - Flat-top orientation (horizontal top and bottom edges)
    /// - First anchor is the NNW (North-North-West) point
    /// - Points are in CW order (consistent with HexVertex enum)
    ///
    /// Flat-top hexagon vertex order (CW from NNW, matching HexVertex):
    /// ```text
    ///      2 ---- 1
    ///     /        \
    ///    3          0
    ///     \        /
    ///      4 ---- 5
    /// ```
    ///
    /// Returns `Ok(())` if valid, otherwise returns an error describing the issue.
    pub fn validate_regular_flat_top_hexagon(&self, subdivision: usize, tolerance: f32) -> Result<(), QuadError> {
        let anchor_positions: Vec<Vec2> = self.mesh.anchor_vertex_iter().map(|v| v.position).collect();

        // Check anchor count
        if anchor_positions.len() != 6 {
            return Err(QuadError::Geometry(format!(
                "Expected a regular hexagon, but got {} anchor points",
                anchor_positions.len()
            )));
        }

        let positions = &anchor_positions;

        // Calculate center and radius
        let center = positions.iter().fold(Vec2::ZERO, |acc, &p| acc + p) / 6.0;
        let radius = (positions[0] - center).length();

        // Validate all points are equidistant from center (regular hexagon)
        for (i, &pos) in positions.iter().enumerate() {
            let dist = (pos - center).length();
            if (dist - radius).abs() > tolerance {
                return Err(QuadError::Geometry(format!(
                    "Expected a regular hexagon, but anchor {} is not equidistant from center: expected radius {:.6}, got {:.6}",
                    i, radius, dist
                )));
            }
        }

        // Validate each anchor is at the correct angle
        let expected_angles_deg = [0.0, 60.0, 120.0, 180.0, 240.0, 300.0];
        for (i, (&pos, &expected_deg)) in positions.iter().zip(expected_angles_deg.iter()).enumerate() {
            let vec_from_center = pos - center;
            let angle_rad = vec_from_center.y.atan2(vec_from_center.x);
            let angle_deg = angle_rad.to_degrees();

            let angle_normalized = if angle_deg < 0.0 { angle_deg + 360.0 } else { angle_deg };
            let angle_diff = (angle_normalized - expected_deg + 180.0).rem_euclid(360.0) - 180.0;
            let angular_tolerance_deg = tolerance.atan2(radius).to_degrees();

            if angle_diff.abs() > angular_tolerance_deg {
                return Err(QuadError::Geometry(format!(
                    "Expected a regular hexagon, but anchor {} is not at the correct angle: expected {:.1}°, got {:.1}° (diff: {:.1}°)",
                    i, expected_deg, angle_normalized, angle_diff
                )));
            }
        }

        // Validate side lengths are equal
        let first_side_length = (positions[1] - positions[0]).length();
        for i in 0..6 {
            let next_i = (i + 1) % 6;
            let side_length = (positions[next_i] - positions[i]).length();

            if (side_length - first_side_length).abs() > tolerance {
                return Err(QuadError::Geometry(format!(
                    "Expected a regular hexagon, but side {}->{} has different length: expected {:.6}, got {:.6}",
                    i, next_i, first_side_length, side_length
                )));
            }
        }

        for i in 0..6 {
            let count = self.mesh.anchor_edge(AnchorIndex::new(i)).count();
            if count != subdivision {
                return Err(QuadError::Geometry(format!(
                    "Expected a regular hexagon, but anchor edge {} should have {} subdivisions, got {}",
                    i, subdivision, count
                )));
            }
        }

        Ok(())
    }

    /// Validate that all finite quads have positive signed area (CCW winding).
    fn validate_quad_areas(&self) -> Result<(), QuadError> {
        for qi in self.mesh.finite_quad_index_iter() {
            let verts = self.mesh.quad_vertices(qi);
            let positions: [Vec2; 4] = std::array::from_fn(|i| self.mesh[verts[i]].position);

            let area = quad_signed_area(&positions);
            if area <= 0.0 {
                return Err(QuadError::Geometry(format!(
                    "Quad {} has negative or zero area (non-CCW winding)",
                    qi.into_index()
                )));
            }
        }
        Ok(())
    }

    /// Validate that no finite quad edges intersect with other finite quad edges.
    fn validate_no_self_intersections(&self) -> Result<(), QuadError> {
        let quads: Vec<QuadIndex> = self.mesh.finite_quad_index_iter().collect();

        for (i, &qi) in quads.iter().enumerate() {
            let verts_i = self.mesh.quad_vertices(qi);
            let positions_i: [Vec2; 4] = std::array::from_fn(|idx| self.mesh[verts_i[idx]].position);

            for &qj in quads.iter().skip(i + 1) {
                let verts_j = self.mesh.quad_vertices(qj);
                let positions_j: [Vec2; 4] = std::array::from_fn(|idx| self.mesh[verts_j[idx]].position);

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
                            let verts_i_indices: Vec<_> = verts_i.iter().map(|v| v.into_index()).collect();
                            let verts_j_indices: Vec<_> = verts_j.iter().map(|v| v.into_index()).collect();
                            return Err(QuadError::Geometry(format!(
                                "Self-intersection detected between quad {} and quad {}\n\
                                 Quad {} vertices: {:?} (indices: {:?})\n\
                                 Quad {} vertices: {:?} (indices: {:?})\n\
                                 Intersecting edges: quad {} edge {} [{:?} -> {:?}] vs quad {} edge {} [{:?} -> {:?}]",
                                qi.into_index(),
                                qj.into_index(),
                                qi.into_index(),
                                positions_i,
                                verts_i_indices,
                                qj.into_index(),
                                positions_j,
                                verts_j_indices,
                                qi.into_index(),
                                edge_i,
                                a,
                                b,
                                qj.into_index(),
                                edge_j,
                                c,
                                d
                            )));
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
    const EPSILON: f32 = 1e-6;

    let d1 = cross_2d(c - a, b - a);
    let d2 = cross_2d(d - a, b - a);
    let d3 = cross_2d(a - c, d - c);
    let d4 = cross_2d(b - c, d - c);

    if d1.abs() < EPSILON || d2.abs() < EPSILON || d3.abs() < EPSILON || d4.abs() < EPSILON {
        return false;
    }

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
