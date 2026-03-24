use glam::Vec2;
use std::cmp::Ordering;

/// Compare two 2D direction vectors by angle (CCW from +X) using cross/dot products.
///
/// Partitions into upper half-plane (y >= 0) and lower (y < 0), then uses
/// the cross product sign within each half-plane. No trigonometry.
///
/// Both vectors are relative to the same origin (i.e. direction vectors, not positions).
pub fn angular_cmp(a: Vec2, b: Vec2) -> Ordering {
    let ha = half_plane(a);
    let hb = half_plane(b);

    if ha != hb {
        return ha.cmp(&hb);
    }

    // Same half-plane: cross product determines order.
    // cross > 0 means a is before b (CCW).
    let cross = a.x * b.y - a.y * b.x;
    if cross > 0.0 {
        Ordering::Less
    } else if cross < 0.0 {
        Ordering::Greater
    } else {
        // Collinear: further from origin sorts later (arbitrary but stable)
        let la = a.x * a.x + a.y * a.y;
        let lb = b.x * b.x + b.y * b.y;
        la.partial_cmp(&lb).unwrap_or(Ordering::Equal)
    }
}

/// Classify a direction into a half-plane for angular sorting.
/// 0: upper (y > 0, or y == 0 && x >= 0)  → angles [0°, 180°)
/// 1: lower (y < 0, or y == 0 && x < 0)   → angles [180°, 360°)
#[inline]
fn half_plane(v: Vec2) -> u8 {
    if v.y > 0.0 || (v.y == 0.0 && v.x >= 0.0) {
        0
    } else {
        1
    }
}

/// Signed area of a quad (shoelace formula). Positive for CCW winding.
pub fn quad_signed_area(pts: &[Vec2; 4]) -> f32 {
    let mut a2 = 0.0f32;
    for i in 0..4 {
        let j = (i + 1) % 4;
        a2 += pts[i].x * pts[j].y - pts[j].x * pts[i].y;
    }
    a2 * 0.5
}

/// Check if a quad is convex and not degenerate.
/// `min_quality` is the minimum allowed `area / max_edge²` (e.g. 0.15).
pub fn is_quad_well_shaped(pts: &[Vec2; 4], min_quality: f32) -> bool {
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

    // Quality: area / max_edge^2
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

/// Check if a point is strictly inside a flat-top regular hexagon centered at the origin.
///
/// `circumradius` is the distance from center to any corner.
/// `margin` shrinks the hex inward by this amount on each edge (use 0.0 for exact test).
///
/// Uses the half-plane method with 3 symmetry axes. For a flat-top hex the
/// edge outward normals are at 0°, 60°, 120° (and their negatives).
pub fn is_inside_hex(point: Vec2, circumradius: f32, margin: f32) -> bool {
    // Apothem = center-to-edge distance = circumradius * cos(30°)
    let apothem = circumradius * (3.0f32.sqrt() / 2.0);
    let limit = apothem - margin;
    if limit <= 0.0 {
        return false;
    }

    let half_sqrt3 = 3.0f32.sqrt() * 0.5;

    // n0 = (1, 0): right/left edges
    if point.x.abs() >= limit {
        return false;
    }
    // n1 = (1/2, √3/2): upper-right / lower-left edges
    let d1 = point.x * 0.5 + point.y * half_sqrt3;
    if d1.abs() >= limit {
        return false;
    }
    // n2 = (-1/2, √3/2): upper-left / lower-right edges
    let d2 = -point.x * 0.5 + point.y * half_sqrt3;
    if d2.abs() >= limit {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use shine_test::test;
    use std::cmp::Ordering;
    use std::f32::consts::FRAC_1_SQRT_2;

    #[test]
    fn cardinal_directions() {
        let right = Vec2::new(1.0, 0.0);
        let up = Vec2::new(0.0, 1.0);
        let left = Vec2::new(-1.0, 0.0);
        let down = Vec2::new(0.0, -1.0);

        assert_eq!(angular_cmp(right, up), Ordering::Less);
        assert_eq!(angular_cmp(up, left), Ordering::Less);
        assert_eq!(angular_cmp(left, down), Ordering::Less);
        assert_eq!(angular_cmp(down, right), Ordering::Greater);
    }

    #[test]
    fn same_direction() {
        let a = Vec2::new(1.0, 1.0);
        let b = Vec2::new(2.0, 2.0);
        // Collinear, b is further — a < b
        assert_eq!(angular_cmp(a, b), Ordering::Less);
        assert_eq!(angular_cmp(b, a), Ordering::Greater);
    }

    #[test]
    fn full_ring_sort() {
        let dirs = vec![
            Vec2::new(0.0, -1.0),            // down
            Vec2::new(-1.0, 0.0),             // left
            Vec2::new(1.0, 0.0),              // right
            Vec2::new(0.0, 1.0),              // up
            Vec2::new(FRAC_1_SQRT_2, FRAC_1_SQRT_2),   // 45°
            Vec2::new(-FRAC_1_SQRT_2, -FRAC_1_SQRT_2), // 225°
        ];

        let mut sorted = dirs.clone();
        sorted.sort_by(|a, b| angular_cmp(*a, *b));

        // Expected CCW order: right(0°), 45°, up(90°), left(180°), 225°, down(270°)
        let expected = vec![
            Vec2::new(1.0, 0.0),
            Vec2::new(FRAC_1_SQRT_2, FRAC_1_SQRT_2),
            Vec2::new(0.0, 1.0),
            Vec2::new(-1.0, 0.0),
            Vec2::new(-FRAC_1_SQRT_2, -FRAC_1_SQRT_2),
            Vec2::new(0.0, -1.0),
        ];
        assert_eq!(sorted, expected);
    }
}
