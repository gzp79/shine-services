use crate::math::hex::AxialCoord;
use serde::{Deserialize, Serialize};

/// Which of the 2 base orientations for the 3-patch split.
/// Even: patches span hex vertices 0-1-2, 2-3-4, 4-5-0
/// Odd: patches span hex vertices 1-2-3, 3-4-5, 5-0-1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchOrientation {
    Even,
    Odd,
}

/// Patch coordinates for hexagonal grid subdivided into quads.
/// p selects one of the 3 main quads (patches) and u,v indexes the grid of quads within each patch
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "(i32, i32, i32)", from = "(i32, i32, i32)")]
pub struct PatchCoord {
    pub p: i32,
    pub u: i32,
    pub v: i32,
}

impl PatchCoord {
    pub const fn new(p: i32, u: i32, v: i32) -> Self {
        debug_assert!(p >= 0 && p < 3);
        Self { p, u, v }
    }

    /// Returns the two hex corner indices (a, b) that anchor this patch's triangle.
    pub fn hex_corner_indices(&self, orientation: PatchOrientation) -> (usize, usize) {
        let start = match orientation {
            PatchOrientation::Even => 0,
            PatchOrientation::Odd => 1,
        };
        let a = (start + self.p as usize * 2) % 6;
        let b = (start + self.p as usize * 2 + 2) % 6;
        (a, b)
    }

    /// Returns the 4 corner AxialCoords of this quad in CCW winding order.
    ///
    /// Each patch is a triangular region anchored by two hex vertices (H_a, H_b) and the center.
    /// Within the patch, (u, v) addresses a quad cell in a grid_size x grid_size grid.
    /// The corners are computed by affine interpolation:
    ///   corner(cu, cv) = (cu * H_a + cv * H_b) / grid_size
    pub fn quad_vertices(&self, orientation: PatchOrientation, subdivision: u32) -> [AxialCoord; 4] {
        let grid = 2i32.pow(subdivision);
        let (a_idx, b_idx) = self.hex_corner_indices(orientation);
        let radius = grid as u32;
        let corners = AxialCoord::hex_corners(radius);
        let ha = corners[a_idx];
        let hb = corners[b_idx];

        let corner = |cu: i32, cv: i32| -> AxialCoord {
            AxialCoord::new((cu * ha.q + cv * hb.q) / grid, (cu * ha.r + cv * hb.r) / grid)
        };

        let (u, v) = (self.u, self.v);
        [corner(u, v), corner(u + 1, v), corner(u + 1, v + 1), corner(u, v + 1)]
    }
}

impl From<(i32, i32, i32)> for PatchCoord {
    fn from((p, u, v): (i32, i32, i32)) -> Self {
        Self { p, u, v }
    }
}

impl From<PatchCoord> for (i32, i32, i32) {
    fn from(coord: PatchCoord) -> Self {
        (coord.p, coord.u, coord.v)
    }
}
