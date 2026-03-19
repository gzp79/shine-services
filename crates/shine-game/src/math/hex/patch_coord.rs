use serde::{Deserialize, Serialize};

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
