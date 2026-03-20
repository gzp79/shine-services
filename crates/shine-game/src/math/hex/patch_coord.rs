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

#[cfg(test)]
mod tests {
    use super::*;
    use shine_test::test;
    use std::collections::HashSet;

    #[test]
    fn test_quad_vertices_subdivision_0() {
        let orientation = PatchOrientation::Even;
        let subdivision = 0;

        for p in 0..3 {
            let coord = PatchCoord::new(p, 0, 0);
            let verts = coord.quad_vertices(orientation, subdivision);
            let unique: HashSet<_> = verts.iter().collect();
            assert_eq!(unique.len(), 4, "patch {p}: vertices not distinct: {:?}", verts);
            assert!(verts.contains(&AxialCoord::origin()), "patch {p}: missing center");
        }
    }

    #[test]
    fn test_quad_vertices_subdivision_1_even() {
        let orientation = PatchOrientation::Even;
        let subdivision = 1;

        // Patch 0: H_a=v0=(2,0), H_b=v2=(-2,2)
        // corner(u,v) = (u/2)*(2,0) + (v/2)*(-2,2) = axial (u-v, v)
        let verts = PatchCoord::new(0, 0, 0).quad_vertices(orientation, subdivision);
        assert_eq!(verts[0], AxialCoord::new(0, 0));
        assert_eq!(verts[1], AxialCoord::new(1, 0));
        assert_eq!(verts[2], AxialCoord::new(0, 1));
        assert_eq!(verts[3], AxialCoord::new(-1, 1));

        let verts = PatchCoord::new(0, 1, 0).quad_vertices(orientation, subdivision);
        assert_eq!(verts[0], AxialCoord::new(1, 0));
        assert_eq!(verts[1], AxialCoord::new(2, 0));
        assert_eq!(verts[2], AxialCoord::new(1, 1));
        assert_eq!(verts[3], AxialCoord::new(0, 1));
    }

    #[test]
    fn test_quad_vertices_shared_across_patches() {
        let orientation = PatchOrientation::Even;
        let subdivision = 1;
        let grid_size = 2;

        let mut patch0_verts = HashSet::new();
        let mut patch1_verts = HashSet::new();
        for u in 0..grid_size {
            for v in 0..grid_size {
                for vert in PatchCoord::new(0, u, v).quad_vertices(orientation, subdivision) {
                    patch0_verts.insert(vert);
                }
                for vert in PatchCoord::new(1, u, v).quad_vertices(orientation, subdivision) {
                    patch1_verts.insert(vert);
                }
            }
        }
        let shared: HashSet<_> = patch0_verts.intersection(&patch1_verts).collect();
        assert!(!shared.is_empty(), "patches should share vertices along common edge");
        assert!(shared.contains(&AxialCoord::new(0, 0)));
        assert!(shared.contains(&AxialCoord::new(-2, 2)));
    }

    #[test]
    fn test_quad_vertices_cover_all_spiral() {
        let orientation = PatchOrientation::Even;
        let subdivision = 1;
        let radius = 2u32;
        let grid_size = 2;

        let mut all_verts = HashSet::new();
        for p in 0..3 {
            for u in 0..grid_size {
                for v in 0..grid_size {
                    for vert in PatchCoord::new(p, u, v).quad_vertices(orientation, subdivision) {
                        all_verts.insert(vert);
                    }
                }
            }
        }

        let spiral_verts: HashSet<_> = AxialCoord::origin().spiral(radius).collect();
        assert_eq!(all_verts, spiral_verts, "quad vertices should cover exact hex spiral");
    }

    #[test]
    fn test_quad_vertices_cover_all_spiral_subdiv2() {
        for orientation in [PatchOrientation::Even, PatchOrientation::Odd] {
            let subdivision = 2;
            let radius = 2u32.pow(subdivision);
            let grid_size = 2i32.pow(subdivision);

            let mut all_verts = HashSet::new();
            for p in 0..3 {
                for u in 0..grid_size {
                    for v in 0..grid_size {
                        for vert in PatchCoord::new(p, u, v).quad_vertices(orientation, subdivision) {
                            all_verts.insert(vert);
                        }
                    }
                }
            }

            let spiral_verts: HashSet<_> = AxialCoord::origin().spiral(radius).collect();
            assert_eq!(
                all_verts, spiral_verts,
                "{:?}: quad vertices should cover exact hex spiral at subdivision {subdivision}",
                orientation
            );
        }
    }

    #[test]
    fn test_quad_vertices_odd_orientation() {
        let orientation = PatchOrientation::Odd;
        let subdivision = 1;

        let verts = PatchCoord::new(0, 0, 0).quad_vertices(orientation, subdivision);
        assert!(verts.contains(&AxialCoord::origin()));
        let unique: HashSet<_> = verts.iter().collect();
        assert_eq!(unique.len(), 4);
    }
}
