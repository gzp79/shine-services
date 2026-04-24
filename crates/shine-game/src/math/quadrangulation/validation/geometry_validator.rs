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

    /// Validate that anchor points form a regular flat-top hexagon in CCW order
    /// and that each anchor edge has the expected number of subdivisions.
    ///
    /// Requirements:
    /// - Exactly 6 anchor points
    /// - Points form a regular hexagon (equal side lengths and angles)
    /// - Flat-top orientation (horizontal top and bottom edges)
    /// - First anchor is the NNW (North-North-West) point
    /// - Points are in CCW order
    ///
    /// Flat-top hexagon vertex order (CCW from NNW):
    /// ```text
    ///      0 ---- 5
    ///     /        \
    ///    1          4
    ///     \        /
    ///      2 ---- 3
    /// ```
    ///
    /// Returns `Ok(())` if valid, otherwise returns an error describing the issue.
    pub fn validate_regular_flat_top_hexagon(&self, subdivision: usize, tolerance: f32) -> Result<(), QuadError> {
        let anchor_positions: Vec<Vec2> = self.mesh.anchor_vertex_iter().map(|v| v.position).collect();

        // Check anchor count
        if anchor_positions.len() != 6 {
            return Err(QuadError::InvalidHexagon(format!(
                "Expected 6 anchor points, got {}",
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
                return Err(QuadError::InvalidHexagon(format!(
                    "Anchor {} is not equidistant from center: expected radius {:.6}, got {:.6}",
                    i, radius, dist
                )));
            }
        }

        // Expected angles for flat-top hexagon starting from NNW (CCW in standard math coordinates)
        // In standard math coordinates (Y up), CCW from NNW (120°): 120° → 180° → 240° → 300° → 0° → 60°
        let expected_angles_deg = [120.0, 180.0, 240.0, 300.0, 0.0, 60.0];

        // Validate each anchor is at the correct angle
        for (i, (&pos, &expected_deg)) in positions.iter().zip(expected_angles_deg.iter()).enumerate() {
            let vec_from_center = pos - center;
            let angle_rad = vec_from_center.y.atan2(vec_from_center.x);
            let angle_deg = angle_rad.to_degrees();

            // Normalize to [0, 360)
            let angle_normalized = if angle_deg < 0.0 { angle_deg + 360.0 } else { angle_deg };

            // Calculate angular difference (handling wraparound)
            let angle_diff = (angle_normalized - expected_deg + 180.0).rem_euclid(360.0) - 180.0;

            // Tolerance in degrees
            let angular_tolerance_deg = tolerance.atan2(radius).to_degrees();

            if angle_diff.abs() > angular_tolerance_deg {
                return Err(QuadError::InvalidHexagon(format!(
                    "Anchor {} is not at the correct angle: expected {:.1}°, got {:.1}° (diff: {:.1}°)",
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
                return Err(QuadError::InvalidHexagon(format!(
                    "Side {}->{} has different length: expected {:.6}, got {:.6}",
                    i, next_i, first_side_length, side_length
                )));
            }
        }

        // Validate CCW winding by checking signed area
        let mut signed_area = 0.0;
        for i in 0..6 {
            let j = (i + 1) % 6;
            signed_area += positions[i].x * positions[j].y;
            signed_area -= positions[j].x * positions[i].y;
        }
        signed_area *= 0.5;

        if signed_area <= 0.0 {
            return Err(QuadError::InvalidHexagon(
                "Hexagon vertices are not in CCW order".to_string(),
            ));
        }

        for i in 0..6 {
            let count = self.mesh.anchor_edge(AnchorIndex::new(i)).count();
            if count != subdivision {
                return Err(QuadError::InvalidHexagon(format!(
                    "Anchor edge {} should have {} subdivisions, got {}",
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
                return Err(QuadError::NegativeQuadArea { quad: qi.into_index() });
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
