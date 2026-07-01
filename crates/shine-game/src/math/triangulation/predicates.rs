use glam::IVec2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OrientationType {
    CW,
    Collinear,
    CCW,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CollinearTestType {
    Before,
    First,
    Between,
    Second,
    After,
}

/// Exact orientation test using i64 arithmetic.
/// Returns > 0 if CCW, < 0 if CW, 0 if collinear.
pub fn orient2d(a: IVec2, b: IVec2, c: IVec2) -> i64 {
    (b - a).as_i64vec2().perp_dot((c - a).as_i64vec2())
}

pub fn test_collinear_points(a: IVec2, b: IVec2, p: IVec2) -> CollinearTestType {
    let (ax, ay) = (a.x, a.y);
    let (bx, by) = (b.x, b.y);
    let (px, py) = (p.x, p.y);
    debug_assert!(a != b);
    debug_assert!(orient2d(a, b, p) == 0);

    let abx = ax - bx;
    let aby = ay - by;
    if abx.abs() > aby.abs() {
        // x-major line
        let apx = ax - px;
        if apx == 0 {
            CollinearTestType::First
        } else {
            let bpx = bx - px;
            if bpx == 0 {
                CollinearTestType::Second
            } else if abx < 0 {
                if apx > 0 {
                    CollinearTestType::Before
                } else if bpx < 0 {
                    CollinearTestType::After
                } else {
                    CollinearTestType::Between
                }
            } else if apx < 0 {
                CollinearTestType::Before
            } else if bpx > 0 {
                CollinearTestType::After
            } else {
                CollinearTestType::Between
            }
        }
    } else {
        // y-major line
        let apy = ay - py;
        if apy == 0 {
            CollinearTestType::First
        } else {
            let bpy = by - py;
            if bpy == 0 {
                CollinearTestType::Second
            } else if aby < 0 {
                if apy > 0 {
                    CollinearTestType::Before
                } else if bpy < 0 {
                    CollinearTestType::After
                } else {
                    CollinearTestType::Between
                }
            } else if apy < 0 {
                CollinearTestType::Before
            } else if bpy > 0 {
                CollinearTestType::After
            } else {
                CollinearTestType::Between
            }
        }
    }
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
