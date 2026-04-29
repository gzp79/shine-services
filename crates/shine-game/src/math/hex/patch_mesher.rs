use std::array;

use crate::{
    indexed::TypedIndex,
    math::{
        hex::{AxialCoord, AxialDenseIndexer, HexPointyDir, PatchCoord, PatchDenseIndexer, PatchOrientation},
        quadrangulation::{Quadrangulation, VertexIndex},
        SQRT_3,
    },
};
use glam::Vec2;

/// Generates a quad mesh inside a hexagon using 3-patch subdivision.
/// The hexagon is subdivided to 2^subdivision cells along each edge, and each patch is meshed as a grid of quads.
pub struct PatchMesher {
    subdivision: u32,
    orientation: PatchOrientation,
    size: f32,

    pointy_size: f32,
    pointy_indexer: AxialDenseIndexer,
}

impl PatchMesher {
    pub fn new(subdivision: u32, orientation: PatchOrientation) -> Self {
        // The center of the cells in the pointy-top grid forms the subdivision of the flat-top hexagon.
        // Hence we can store to vetices of the resulting mesh in the dense grid corresponding to the pointy-top grid.

        let pointy_radius = 2u32.pow(subdivision);
        let size = 1.0;
        let pointy_size = size * SQRT_3 / 2. / pointy_radius as f32;

        Self {
            subdivision,
            orientation,
            size: 1.0,
            pointy_size,
            pointy_indexer: AxialDenseIndexer::new(pointy_radius),
        }
    }

    #[must_use]
    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size;
        self.pointy_size = self.size * SQRT_3 / 2. / self.pointy_indexer.radius() as f32;
        self
    }

    /// Generate the mesh with uniform vertex placement.
    /// The centers of the cells in a pointy-top hex gird forms the subdivision of a flat-top hexagon, where each
    /// vertex is the center of  a cell.
    pub fn generate_uniform(&mut self) -> Quadrangulation {
        let mut positions = vec![Vec2::ZERO; self.pointy_indexer.get_total_size()];
        for coord in AxialCoord::ORIGIN.spiral(self.pointy_indexer.radius()) {
            let idx = self.pointy_indexer.get_dense_index(&coord);
            positions[idx] = coord.pointy().to_position(self.pointy_size);
        }
        self.build_quad_mesh(positions)
    }

    /// Generate the mesh with recursive subdivision placement.
    pub fn generate_subdivision(&mut self) -> Quadrangulation {
        let total = self.pointy_indexer.get_total_size();
        let mut positions = vec![Vec2::NAN; total];

        // Place center
        let center_idx = self.pointy_indexer.get_dense_index(&AxialCoord::ORIGIN);
        positions[center_idx] = Vec2::ZERO;

        // Place 6 hex corner vertices
        for coord in AxialCoord::ORIGIN.pointy().corners(self.pointy_indexer.radius()) {
            let idx = self.pointy_indexer.get_dense_index(&coord);
            positions[idx] = coord.pointy().to_position(self.pointy_size);
        }

        for depth in 0..self.subdivision {
            let parent_grid = 2i32.pow(depth);
            let step = self.pointy_indexer.radius() as i32 / parent_grid;

            for p in 0..3 {
                let (origin, du, dv) = match (self.orientation, p) {
                    (PatchOrientation::Even, 0) => (HexPointyDir::NE, HexPointyDir::W, HexPointyDir::SE),
                    (PatchOrientation::Even, 1) => (HexPointyDir::W, HexPointyDir::SE, HexPointyDir::NE),
                    (PatchOrientation::Even, 2) => (HexPointyDir::SE, HexPointyDir::NE, HexPointyDir::W),
                    (PatchOrientation::Odd, 0) => (HexPointyDir::E, HexPointyDir::NW, HexPointyDir::SW),
                    (PatchOrientation::Odd, 1) => (HexPointyDir::NW, HexPointyDir::SW, HexPointyDir::E),
                    (PatchOrientation::Odd, 2) => (HexPointyDir::SW, HexPointyDir::E, HexPointyDir::NW),
                    _ => unreachable!(),
                };

                let origin = AxialCoord::ORIGIN.pointy().corner(origin, self.pointy_indexer.radius());

                for pu in 0..parent_grid {
                    for pv in 0..parent_grid {
                        let base = origin.step(du, pu * step).step(dv, pv * step);
                        let corner = [
                            base,
                            base.step(du, step),
                            base.step(du, step).step(dv, step),
                            base.step(dv, step),
                        ];
                        let edge_mid = [
                            base.step(du, step / 2),
                            base.step(du, step).step(dv, step / 2),
                            base.step(du, step / 2).step(dv, step),
                            base.step(dv, step / 2),
                        ];
                        let mid = base.step(du, step / 2).step(dv, step / 2);

                        let mid_idx = self.pointy_indexer.get_dense_index(&mid);
                        let edge_mid_idx: [usize; 4] =
                            array::from_fn(|i| self.pointy_indexer.get_dense_index(&edge_mid[i]));
                        let corner_idx: [usize; 4] =
                            array::from_fn(|i| self.pointy_indexer.get_dense_index(&corner[i]));

                        let mut edge_mid_pos = [Vec2::NAN; 4];

                        // place edge midpoints
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
        // The external (anchor) vertices of the mesh correspond to the cells of the outer ring in a pointy-top hex grid.
        let mut polygon = Vec::new();
        for coord in AxialCoord::ORIGIN.ring(self.pointy_indexer.radius()) {
            polygon.push(VertexIndex::new(self.pointy_indexer.get_dense_index(&coord)));
        }

        // Build quad indices
        let mut quads = Vec::new();
        for p in 0..3u32 {
            let (origin, du, dv) = match (self.orientation, p) {
                (PatchOrientation::Even, 0) => (HexPointyDir::NE, HexPointyDir::W, HexPointyDir::SE),
                (PatchOrientation::Even, 1) => (HexPointyDir::W, HexPointyDir::SE, HexPointyDir::NE),
                (PatchOrientation::Even, 2) => (HexPointyDir::SE, HexPointyDir::NE, HexPointyDir::W),
                (PatchOrientation::Odd, 0) => (HexPointyDir::E, HexPointyDir::NW, HexPointyDir::SW),
                (PatchOrientation::Odd, 1) => (HexPointyDir::NW, HexPointyDir::SW, HexPointyDir::E),
                (PatchOrientation::Odd, 2) => (HexPointyDir::SW, HexPointyDir::E, HexPointyDir::NW),
                _ => unreachable!(),
            };

            let origin = AxialCoord::ORIGIN.pointy().corner(origin, self.pointy_indexer.radius());
            for u in 0..self.pointy_indexer.radius() {
                for v in 0..self.pointy_indexer.radius() {
                    let i0 = origin.step(du, u as i32).step(dv, v as i32);
                    let i1 = i0.step(du, 1);
                    let i2 = i1.step(dv, 1);
                    let i3 = i0.step(dv, 1);
                    quads.push([
                        VertexIndex::new(self.pointy_indexer.get_dense_index(&i0)),
                        VertexIndex::new(self.pointy_indexer.get_dense_index(&i1)),
                        VertexIndex::new(self.pointy_indexer.get_dense_index(&i2)),
                        VertexIndex::new(self.pointy_indexer.get_dense_index(&i3)),
                    ]);
                }
            }
        }

        let anchors: Vec<VertexIndex> = AxialCoord::ORIGIN
            .pointy()
            .corners(self.pointy_indexer.radius())
            .iter()
            .map(|c| VertexIndex::new(self.pointy_indexer.get_dense_index(c)))
            .collect();

        Quadrangulation::from_polygon(positions, polygon, quads, anchors).expect("valid patch mesh topology")
    }
}

fn mid_point(quad: [Vec2; 4]) -> Vec2 {
    let (p1, p2, p3, p4) = (quad[0], quad[1], quad[2], quad[3]);

    let d1 = p2 - p1;
    let d2 = p4 - p3;
    let cross = d1.perp_dot(d2);
    if cross.abs() < 1e-6 {
        (p1 + p2 + p3 + p4) / 4.0
    } else {
        let t = (p3 - p1).perp_dot(d2) / cross;
        p1 + t * d1
    }
}
