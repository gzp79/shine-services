use crate::math::rect::QuadFlatDir;
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
///
/// ```text
///  Even orientation:       Odd orientation:
///           <- u               v ->
///       x---x---+             +---x---x
///      / \       \v         u/       / \
///     x   x   0   x         x   1   x   x
///   v/     \       \       /       /     \u
///   +   1   x---x---x     x---x---x   0   +
///   u\     /       /       \       \     /v
///     x   x   2   x         x   2   x   x
///      \ /       /u         v\       \ /
///       x---x---+             +---x---x
///           <- v               u ->
///
/// The origin of each patch is marked with '+' and u, v are orinted so whan u (or v) reaches the patch boundary,
/// the neighbor patch is reached p+1 % 3 (or p-1 % 3)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "(u32, u32, u32)", from = "(u32, u32, u32)")]
pub struct PatchCoord {
    pub p: u32,
    pub u: u32,
    pub v: u32,
}

impl PatchCoord {
    pub const fn new(p: u32, u: u32, v: u32) -> Self {
        debug_assert!(p < 3);
        Self { p, u, v }
    }

    /// Return the neighboring patch coordinate in the given direction.
    /// If the neighbor is out of bounds (u or v < 0 or >= size), return None.
    pub fn neighbor(&self, size: u32, dir: QuadFlatDir) -> Option<Self> {
        debug_assert!(self.u < size);
        debug_assert!(self.v < size);

        let (u, v) = match dir {
            QuadFlatDir::E => (self.u as i32 + 1, self.v as i32),
            QuadFlatDir::S => (self.u as i32, self.v as i32 - 1),
            QuadFlatDir::N => (self.u as i32, self.v as i32 + 1),
            QuadFlatDir::W => (self.u as i32 - 1, self.v as i32),
        };

        if u < 0 || v < 0 {
            None
        } else if u >= size as i32 {
            Some(Self {
                p: (self.p + 1) % 3,
                u: size - 1,
                v: v as u32,
            })
        } else if v >= size as i32 {
            Some(Self {
                p: (self.p + 2) % 3,
                u: u as u32,
                v: size - 1,
            })
        } else {
            Some(Self {
                p: self.p,
                u: u as u32,
                v: v as u32,
            })
        }
    }
}

impl From<(u32, u32, u32)> for PatchCoord {
    fn from((p, u, v): (u32, u32, u32)) -> Self {
        Self { p, u, v }
    }
}

impl From<PatchCoord> for (u32, u32, u32) {
    fn from(coord: PatchCoord) -> Self {
        (coord.p, coord.u, coord.v)
    }
}
