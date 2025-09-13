use bevy::{
    math::Vec3,
    render::primitives::{Frustum, HalfSpace},
};

pub trait FrustumExt {
    /// Calculates the corners of this frustum. Returns `None` if the frustum isn't properly defined.
    fn corners(&self) -> Option<[Vec3; 8]>;
}

/// Returns the intersection position if the three half-spaces all intersect at a single point.
fn intersect(a: HalfSpace, b: HalfSpace, c: HalfSpace) -> Option<Vec3> {
    let an = a.normal();
    let bn = b.normal();
    let cn = c.normal();

    let x = Vec3::new(an.x, bn.x, cn.x);
    let y = Vec3::new(an.y, bn.y, cn.y);
    let z = Vec3::new(an.z, bn.z, cn.z);

    let d = -Vec3::new(a.d(), b.d(), c.d());

    let u = y.cross(z);
    let v = x.cross(d);

    let denom = x.dot(u);

    if denom.abs() < f32::EPSILON {
        return None;
    }

    Some(Vec3::new(d.dot(u), z.dot(v), -y.dot(v)) / denom)
}

impl FrustumExt for Frustum {
    fn corners(&self) -> Option<[Vec3; 8]> {
        let [left, right, top, bottom, near, far] = self.half_spaces;
        Some([
            intersect(top, left, near)?,
            intersect(top, right, near)?,
            intersect(bottom, right, near)?,
            intersect(bottom, left, near)?,
            intersect(top, left, far)?,
            intersect(top, right, far)?,
            intersect(bottom, right, far)?,
            intersect(bottom, left, far)?,
        ])
    }
}
