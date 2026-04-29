use super::AxialCoord;

/// A local coordinate frame for navigating a 2D grid in axial coordinates.
/// Represents an origin point and two basis vectors (du, dv) that define a (u, v) grid.
/// Each basis vector is a (Δq, Δr) delta in axial space.
#[derive(Debug, Clone, Copy)]
pub struct AxialBase {
    origin: AxialCoord,
    du: (i32, i32), // (Δq, Δr) for u direction
    dv: (i32, i32), // (Δq, Δr) for v direction
}

impl AxialBase {
    /// Create a new axial base with an origin and two basis vectors.
    /// Each basis vector is specified as (Δq, Δr) deltas.
    pub fn new(origin: AxialCoord, du: (i32, i32), dv: (i32, i32)) -> Self {
        Self { origin, du, dv }
    }

    /// Get the axial coordinate at grid position (u, v).
    /// Computes: origin + u * du + v * dv
    pub fn at(&self, u: i32, v: i32) -> AxialCoord {
        AxialCoord {
            q: self.origin.q + u * self.du.0 + v * self.dv.0,
            r: self.origin.r + u * self.du.1 + v * self.dv.1,
        }
    }

    /// Get the origin of this base.
    #[inline]
    pub fn origin(&self) -> AxialCoord {
        self.origin
    }

    /// Get the u basis vector as (Δq, Δr).
    #[inline]
    pub fn du(&self) -> (i32, i32) {
        self.du
    }

    /// Get the v basis vector as (Δq, Δr).
    #[inline]
    pub fn dv(&self) -> (i32, i32) {
        self.dv
    }

    /// Create a new base with scaled basis vectors.
    /// The origin remains the same, but du and dv are multiplied by the scale factor.
    pub fn scaled(&self, scale: i32) -> AxialBase {
        AxialBase::new(
            self.origin,
            (self.du.0 * scale, self.du.1 * scale),
            (self.dv.0 * scale, self.dv.1 * scale),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::hex::{AxialCoord, HexFlatDir, HexPointyDir};

    #[test]
    fn test_axial_base_at() {
        let origin = AxialCoord::new(0, -4);
        // W direction: (-1, +1), SE direction: (+1, 0)
        let base = AxialBase::new(origin, (-1, 1), (1, 0));

        // Test origin
        let p00 = base.at(0, 0);
        assert_eq!(p00.q, 0);
        assert_eq!(p00.r, -4);

        // Test stepping in u direction (du = -1, +1)
        let p10 = base.at(1, 0);
        assert_eq!(p10.q, -1);
        assert_eq!(p10.r, -3);

        // Test stepping in v direction (dv = +1, 0)
        let p01 = base.at(0, 1);
        assert_eq!(p01.q, 1);
        assert_eq!(p01.r, -4);

        // Test stepping in both directions
        let p22 = base.at(2, 2);
        assert_eq!(p22.q, 0);
        assert_eq!(p22.r, -2);
    }

    #[test]
    fn test_axial_base_with_scaled_basis() {
        let origin = AxialCoord::new(0, 0);
        // Basis vectors scaled by 2
        let base = AxialBase::new(origin, (2, -2), (0, 2));

        let p11 = base.at(1, 1);
        assert_eq!(p11.q, 2); // 0 + 1*2 + 1*0
        assert_eq!(p11.r, 0); // 0 + 1*(-2) + 1*2
    }

    #[test]
    fn test_pointy_base_method() {
        let origin = AxialCoord::new(0, -4);
        let base = origin.pointy().base(HexPointyDir::W, HexPointyDir::SE);

        // Test that base() produces same results as manual construction
        let p10 = base.at(1, 0);
        assert_eq!(p10.q, -1);
        assert_eq!(p10.r, -3);

        let p01 = base.at(0, 1);
        assert_eq!(p01.q, 1);
        assert_eq!(p01.r, -4);
    }

    #[test]
    fn test_flat_base_method() {
        let origin = AxialCoord::new(0, 0);
        let base = origin.flat().base(HexFlatDir::NE, HexFlatDir::SE);

        // Verify base works with flat coordinates
        let p00 = base.at(0, 0);
        assert_eq!(p00.q, 0);
        assert_eq!(p00.r, 0);

        let p11 = base.at(1, 1);
        // NE = (1, -1), SE = (1, 0)
        assert_eq!(p11.q, 2); // 0 + 1*1 + 1*1
        assert_eq!(p11.r, -1); // 0 + 1*(-1) + 1*0
    }

    #[test]
    fn test_axial_base_scaled() {
        let origin = AxialCoord::new(5, -2);
        let base = AxialBase::new(origin, (1, -1), (0, 1));

        let scaled_base = base.scaled(3);

        // Origin should remain unchanged
        assert_eq!(scaled_base.origin().q, 5);
        assert_eq!(scaled_base.origin().r, -2);

        // Basis vectors should be scaled
        assert_eq!(scaled_base.du(), (3, -3));
        assert_eq!(scaled_base.dv(), (0, 3));

        // Test that at() produces correctly scaled positions
        let p11 = scaled_base.at(1, 1);
        assert_eq!(p11.q, 8); // 5 + 1*3 + 1*0
        assert_eq!(p11.r, 1); // -2 + 1*(-3) + 1*3
    }
}
