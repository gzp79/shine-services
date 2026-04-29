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

/// Minimum scaled Jacobian of a quad — the standard quad quality metric.
///
/// At each corner computes `(e1 × e2) / (|e1| · |e2|) = sin(θ)` where `e1`, `e2`
/// are the two adjacent edge vectors and `θ` is the interior angle. Returns the
/// minimum over all 4 corners.
///
/// | Value | Meaning |
/// |-------|---------|
/// | `1.0` | perfect square |
/// | `> 0` | valid convex quad; quality ∝ min corner angle |
/// | `0.0` | degenerate (collapsed corner) |
/// | `< 0` | invalid: concave or self-intersecting |
pub fn quad_jacobian(pts: &[Vec2; 4]) -> f32 {
    let mut min_sj = f32::MAX;
    for i in 0..4 {
        let p = pts[i];
        let e1 = pts[(i + 1) % 4] - p;
        let e2 = pts[(i + 3) % 4] - p;
        let cross = e1.x * e2.y - e1.y * e2.x;
        let len = e1.length() * e2.length();
        if len < 1e-10 {
            return 0.0;
        }
        min_sj = min_sj.min(cross / len);
    }
    min_sj
}

#[cfg(test)]
mod tests {
    use super::*;
    use shine_test::test;
    use std::cmp::Ordering;
    use std::f32::consts::FRAC_1_SQRT_2;

    #[test]
    fn angular_cmp_cardinal_directions() {
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
    fn angular_cmp_collinear_by_length() {
        let a = Vec2::new(1.0, 1.0);
        let b = Vec2::new(2.0, 2.0);
        // Collinear, b is further — a < b
        assert_eq!(angular_cmp(a, b), Ordering::Less);
        assert_eq!(angular_cmp(b, a), Ordering::Greater);
    }

    #[test]
    fn angular_cmp_full_ring_sort() {
        let dirs = vec![
            Vec2::new(0.0, -1.0),                      // down
            Vec2::new(-1.0, 0.0),                      // left
            Vec2::new(1.0, 0.0),                       // right
            Vec2::new(0.0, 1.0),                       // up
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

/// Return the (normalized) bisector of the two vector
pub fn bisector(u: Vec2, v: Vec2) -> Vec2 {
    const EPS: f32 = 1e-6;

    if u.length_squared() < EPS || v.length_squared() < EPS {
        return Vec2::ZERO; // invalid input
    }

    let u = u.normalize();
    let v = v.normalize();

    let left = Vec2::new(-u.y, u.x);
    let sum = u + v;

    let mut b = if sum.length_squared() < EPS {
        // straight / opposite direction:
        // infinitely many bisectors -> choose CCW normal
        left
    } else {
        sum.normalize()
    };

    // orient toward CCW side
    if b.dot(left) < 0.0 {
        b = -b;
    }

    b
}

#[cfg(test)]
mod test {
    use super::*;
    use glam::vec2;
    use shine_test::test;

    #[test]
    fn test_bisector() {
        let u = vec2(1.0, -1.0);
        let v = vec2(1.0, 1.0);
        let b = bisector(u, v);
        assert!((b - vec2(1.0, 0.0)).length() < 1e-6);

        let u = vec2(1.0, 0.0);
        let v = vec2(-1.0, 0.0);
        let b = bisector(u, v);
        assert!((b - vec2(0.0, -1.0)).length() < 1e-6);
    }
}
