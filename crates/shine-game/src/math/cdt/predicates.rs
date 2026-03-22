// Vendored from cdt crate v0.1.0 — modified for exact integer predicates.

use glam::{I64Vec2, IVec2};

/// Exact orientation test using i64 arithmetic.
/// Returns > 0 if CCW, < 0 if CW, 0 if collinear.
pub fn orient2d(a: IVec2, b: IVec2, c: IVec2) -> i64 {
    (b - a).as_i64vec2().perp_dot((c - a).as_i64vec2())
}

/// Exact incircle test using i128 arithmetic.
/// Returns > 0 if d is inside circumcircle of CCW (a,b,c).
/// Safe for |x|,|y| <= 545_000_000.
pub fn in_circle(a: IVec2, b: IVec2, c: IVec2, d: IVec2) -> i128 {
    let ax = a.x as i128 - d.x as i128;
    let ay = a.y as i128 - d.y as i128;
    let bx = b.x as i128 - d.x as i128;
    let by = b.y as i128 - d.y as i128;
    let cx = c.x as i128 - d.x as i128;
    let cy = c.y as i128 - d.y as i128;

    let a2 = ax * ax + ay * ay;
    let b2 = bx * bx + by * by;
    let c2 = cx * cx + cy * cy;

    ax * (by * c2 - cy * b2) - ay * (bx * c2 - cx * b2) + a2 * (bx * cy - cx * by)
}

/// Checks whether the angle a-b-c is acute (dot product > 0).
pub fn acute(a: IVec2, b: IVec2, c: IVec2) -> i64 {
    (a - b).as_i64vec2().dot((c - b).as_i64vec2())
}

/// Returns the raw sum of triangle vertices (NOT divided by 3).
/// Use with scale=3 for the *3 trick.
pub fn centroid_sum(a: IVec2, b: IVec2, c: IVec2) -> I64Vec2 {
    a.as_i64vec2() + b.as_i64vec2() + c.as_i64vec2()
}

/// Direction vector from a scaled center to a point.
/// center = sum of coordinates, scale = denominator that was skipped.
/// Result = p * scale - center, preserving angular ordering.
pub fn point_dir(p: IVec2, center: I64Vec2, scale: i64) -> I64Vec2 {
    p.as_i64vec2() * scale - center
}

/// Squared magnitude of a direction vector (for sorting by distance).
pub fn distance2(dir: I64Vec2) -> i64 {
    dir.dot(dir)
}
