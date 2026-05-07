use std::array;

use crate::{
    indexed::TypedIndex,
    math::{
        hex::{AxialCoord, AxialDenseIndexer, HexPointyDir, PatchOrientation},
        quadrangulation::{Quadrangulation, VertexIndex},
        SQRT_3,
    },
};
use glam::Vec2;

/// Generates a quad mesh inside a hexagon using 3-patch subdivision.
/// The hexagon is subdivided to 2^subdivision cells along each edge, and each patch is meshed as a grid of quads.
pub struct PatchMesher {
    subdivision: u32,
    /// The major orientation of the hexagon patch determining the direction of the 3 main patches.
    orientation: PatchOrientation,
    /// The circumradius of the hex boundary, which controls the overall scale of the output mesh.
    size: f32,
    /// The radius of the (pointy) hex-grid patch forming the interior of the mesh.
    patch_radius: u32,
    /// Hex patch size that sums up to `size` for the enclosed hex patches.
    patch_size: f32,
    /// Indexer for axial coordinates, which provides a dense index for each coordinate within the cell radius.
    patch_indexer: AxialDenseIndexer,
}

impl PatchMesher {
    pub fn new(subdivision: u32, orientation: PatchOrientation) -> Self {
        // The center of the cells in the pointy-top grid forms the subdivision of the flat-top hexagon.
        // Hence we can store to vetices of the resulting mesh in the dense grid corresponding to the pointy-top grid.

        let size = 1.0;
        let patch_radius = 2u32.pow(subdivision);
        let patch_size = size * SQRT_3 / 3. / patch_radius as f32;

        Self {
            subdivision,
            orientation,
            size: 1.0,
            patch_radius,
            patch_size,
            patch_indexer: AxialDenseIndexer::new(patch_radius),
        }
    }

    #[must_use]
    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size;
        self.patch_size = self.size * SQRT_3 / 3. / self.patch_radius as f32;
        self
    }

    /// Generate the mesh with uniform vertex placement.
    /// The centers of the cells in a pointy-top hex gird forms the subdivision of a flat-top hexagon, where each
    /// vertex is the center of  a cell.
    pub fn generate_uniform(&mut self) -> Quadrangulation {
        let mut positions = vec![Vec2::ZERO; self.patch_indexer.get_total_size()];
        for coord in AxialCoord::ORIGIN.spiral(self.patch_radius) {
            let idx = self.patch_indexer.get_dense_index(&coord);
            positions[idx] = coord.pointy().to_position(self.patch_size);
        }
        self.build_quad_mesh(positions)
    }

    /// Generate the mesh with recursive subdivision placement.
    pub fn generate_subdivision(&mut self) -> Quadrangulation {
        let total = self.patch_indexer.get_total_size();
        let mut positions = vec![Vec2::NAN; total];

        // Place center
        let center_idx = self.patch_indexer.get_dense_index(&AxialCoord::ORIGIN);
        positions[center_idx] = Vec2::ZERO;

        // Place 6 hex corner vertices
        for coord in AxialCoord::ORIGIN.pointy().corners(self.patch_radius) {
            let idx = self.patch_indexer.get_dense_index(&coord);
            positions[idx] = coord.pointy().to_position(self.patch_size);
        }

        for depth in 0..self.subdivision {
            let parent_grid = 2i32.pow(depth);
            let step = self.patch_radius as i32 / parent_grid;

            for p in 0..3 {
                let (origin_dir, du_dir, dv_dir) = match (self.orientation, p) {
                    (PatchOrientation::Even, 0) => (HexPointyDir::NE, HexPointyDir::W, HexPointyDir::SE),
                    (PatchOrientation::Even, 1) => (HexPointyDir::W, HexPointyDir::SE, HexPointyDir::NE),
                    (PatchOrientation::Even, 2) => (HexPointyDir::SE, HexPointyDir::NE, HexPointyDir::W),
                    (PatchOrientation::Odd, 0) => (HexPointyDir::E, HexPointyDir::NW, HexPointyDir::SW),
                    (PatchOrientation::Odd, 1) => (HexPointyDir::NW, HexPointyDir::SW, HexPointyDir::E),
                    (PatchOrientation::Odd, 2) => (HexPointyDir::SW, HexPointyDir::E, HexPointyDir::NW),
                    _ => unreachable!(),
                };

                let origin = AxialCoord::ORIGIN.pointy().corner(origin_dir, self.patch_radius);
                let base_sys = origin.base(du_dir, dv_dir);
                let half_step_sys = base_sys.scaled(step / 2);
                eprintln!("depth={depth} parent_grid={parent_grid} step={step}");

                for pu in 0..parent_grid {
                    for pv in 0..parent_grid {
                        // Convert to half-step coordinates (each full step = 2 half-steps)
                        let u = pu * 2;
                        let v = pv * 2;

                        let corner = [
                            half_step_sys.at(u, v),
                            half_step_sys.at(u + 2, v),
                            half_step_sys.at(u + 2, v + 2),
                            half_step_sys.at(u, v + 2),
                        ];
                        let edge_mid = [
                            half_step_sys.at(u + 1, v),     // edge 0->1
                            half_step_sys.at(u + 2, v + 1), // edge 1->2
                            half_step_sys.at(u + 1, v + 2), // edge 2->3
                            half_step_sys.at(u, v + 1),     // edge 3->0
                        ];
                        let mid = half_step_sys.at(u + 1, v + 1);

                        let mid_idx = self.patch_indexer.get_dense_index(&mid);
                        let edge_mid_idx: [usize; 4] =
                            array::from_fn(|i| self.patch_indexer.get_dense_index(&edge_mid[i]));
                        let corner_idx: [usize; 4] = array::from_fn(|i| self.patch_indexer.get_dense_index(&corner[i]));

                        // place edge midpoints
                        let mut edge_mid_pos = [Vec2::NAN; 4];
                        for e in 0..4 {
                            let em_idx = edge_mid_idx[e];
                            if positions[em_idx].is_nan() {
                                let a = positions[corner_idx[e]];
                                let b = positions[corner_idx[(e + 1) % 4]];
                                debug_assert!(!a.is_nan());
                                debug_assert!(!b.is_nan());
                                positions[em_idx] = (a + b) / 2.0;
                            }
                            edge_mid_pos[e] = positions[em_idx];
                            debug_assert!(!edge_mid_pos[e].is_nan());
                        }

                        // place face midpoint
                        let mid_pos = mid_point(edge_mid_pos);
                        positions[mid_idx] = mid_pos;
                        //positions[mid_idx] = mid.pointy().to_position(self.patch_size);
                        debug_assert!(!positions[mid_idx].is_nan());
                    }
                }
            }
        }

        debug_assert!(positions.iter().all(|&p| !p.is_nan()), "Some vertices were not placed!");
        self.build_quad_mesh(positions)
    }

    /// Build a Quadrangulation from vertex positions using the patch topology.
    fn build_quad_mesh(&self, positions: Vec<Vec2>) -> Quadrangulation {
        let mut polygon = Vec::new();
        for coord in AxialCoord::ORIGIN.ring(self.patch_radius) {
            polygon.push(VertexIndex::new(self.patch_indexer.get_dense_index(&coord)));
        }

        // Build quad indices
        let mut quads = Vec::new();
        for p in 0..3u32 {
            let (origin_dir, du_dir, dv_dir) = match (self.orientation, p) {
                (PatchOrientation::Even, 0) => (HexPointyDir::NE, HexPointyDir::W, HexPointyDir::SE),
                (PatchOrientation::Even, 1) => (HexPointyDir::W, HexPointyDir::SE, HexPointyDir::NE),
                (PatchOrientation::Even, 2) => (HexPointyDir::SE, HexPointyDir::NE, HexPointyDir::W),
                (PatchOrientation::Odd, 0) => (HexPointyDir::E, HexPointyDir::NW, HexPointyDir::SW),
                (PatchOrientation::Odd, 1) => (HexPointyDir::NW, HexPointyDir::SW, HexPointyDir::E),
                (PatchOrientation::Odd, 2) => (HexPointyDir::SW, HexPointyDir::E, HexPointyDir::NW),
                _ => unreachable!(),
            };

            let origin = AxialCoord::ORIGIN.pointy().corner(origin_dir, self.patch_radius);
            let base_sys = origin.base(du_dir, dv_dir);

            for u in 0..self.patch_radius {
                for v in 0..self.patch_radius {
                    let i0 = base_sys.at(u as i32, v as i32);
                    let i1 = base_sys.at(u as i32 + 1, v as i32);
                    let i2 = base_sys.at(u as i32 + 1, v as i32 + 1);
                    let i3 = base_sys.at(u as i32, v as i32 + 1);
                    quads.push([
                        VertexIndex::new(self.patch_indexer.get_dense_index(&i0)),
                        VertexIndex::new(self.patch_indexer.get_dense_index(&i1)),
                        VertexIndex::new(self.patch_indexer.get_dense_index(&i2)),
                        VertexIndex::new(self.patch_indexer.get_dense_index(&i3)),
                    ]);
                }
            }
        }

        let anchors: Vec<VertexIndex> = AxialCoord::ORIGIN
            .pointy()
            .corners(self.patch_radius)
            .iter()
            .map(|c| VertexIndex::new(self.patch_indexer.get_dense_index(c)))
            .collect();

        let mesh =
            Quadrangulation::from_polygon(positions, polygon, quads, anchors).expect("valid patch mesh topology");
        mesh.dump();
        mesh
    }
}

fn mid_point(quad: [Vec2; 4]) -> Vec2 {
    let (p1, p2, p3, p4) = (quad[0], quad[1], quad[2], quad[3]);
    (p1 + p2 + p3 + p4) / 4.0
}
