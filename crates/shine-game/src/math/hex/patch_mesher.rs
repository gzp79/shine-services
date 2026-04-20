use crate::{
    indexed::TypedIndex,
    math::{
        hex::{AxialCoord, AxialDenseIndexer, PatchCoord, PatchDenseIndexer, PatchOrientation},
        quadrangulation::{Quadrangulation, VertexIndex},
    },
};
use glam::Vec2;

/// Generates a quad mesh inside a hexagon using 3-patch subdivision.
///
/// Returns a [`Quadrangulation`] with topology and positions. No smoothing or
/// filtering is applied — use filters on the returned mesh.
pub struct PatchMesher {
    subdivision: u32,
    orientation: PatchOrientation,
    hex_size: f32,
}

impl PatchMesher {
    pub fn new(subdivision: u32, orientation: PatchOrientation) -> Self {
        Self {
            subdivision,
            orientation,
            hex_size: 1.0,
        }
    }

    #[must_use]
    pub fn with_hex_size(mut self, hex_size: f32) -> Self {
        self.hex_size = hex_size;
        self
    }

    /// Set the world-space circumradius (center to corner) of the hex.
    #[must_use]
    pub fn with_world_size(self, size: f32) -> Self {
        let radius = 2u32.pow(self.subdivision);
        self.with_hex_size(AxialCoord::hex_size_from_world_size(size, radius))
    }

    /// Generate the mesh with uniform vertex placement.
    pub fn generate_uniform(&mut self) -> Quadrangulation {
        let radius = 2u32.pow(self.subdivision);
        let indexer = AxialDenseIndexer::new(radius);

        let mut positions = vec![Vec2::ZERO; indexer.get_total_size()];
        for coord in AxialCoord::origin().spiral(radius) {
            let idx = indexer.get_dense_index(&coord);
            positions[idx] = coord.vertex_position(self.hex_size);
        }

        self.build_quad_mesh(positions)
    }

    /// Generate the mesh with recursive subdivision placement.
    pub fn generate_subdivision(&mut self) -> Quadrangulation {
        let radius = 2u32.pow(self.subdivision);
        let indexer = AxialDenseIndexer::new(radius);
        let orientation = self.orientation;

        let total = indexer.get_total_size();
        let mut positions = vec![Vec2::ZERO; total];
        let mut placed = vec![false; total];

        // Place 6 hex corner vertices
        let hex_corners = AxialCoord::hex_corners(radius);
        for coord in &hex_corners {
            let idx = indexer.get_dense_index(coord);
            positions[idx] = coord.vertex_position(self.hex_size);
            placed[idx] = true;
        }

        // Place center at origin
        let center_idx = indexer.get_dense_index(&AxialCoord::origin());
        positions[center_idx] = Vec2::ZERO;
        placed[center_idx] = true;

        for depth in 0..self.subdivision {
            let parent_grid = 2i32.pow(depth);

            for p in 0..3 {
                let (a_idx, b_idx) = PatchCoord::new(p, 0, 0).hex_corner_indices(orientation);
                let ha = hex_corners[a_idx];
                let hb = hex_corners[b_idx];

                let parent_corner = |cu: i32, cv: i32| -> AxialCoord {
                    AxialCoord::new(
                        (cu * ha.q + cv * hb.q) / parent_grid,
                        (cu * ha.r + cv * hb.r) / parent_grid,
                    )
                };

                for pu in 0..parent_grid {
                    for pv in 0..parent_grid {
                        let corners = [
                            parent_corner(pu, pv),
                            parent_corner(pu + 1, pv),
                            parent_corner(pu + 1, pv + 1),
                            parent_corner(pu, pv + 1),
                        ];

                        // Edge midpoints
                        let edge_coords: [AxialCoord; 4] = std::array::from_fn(|i| {
                            let a = corners[i];
                            let b = corners[(i + 1) % 4];
                            AxialCoord::new((a.q + b.q) / 2, (a.r + b.r) / 2)
                        });
                        for edge_idx in 0..4 {
                            let mid_dense = indexer.get_dense_index(&edge_coords[edge_idx]);
                            if !placed[mid_dense] {
                                let a_pos = positions[indexer.get_dense_index(&corners[edge_idx])];
                                let b_pos = positions[indexer.get_dense_index(&corners[(edge_idx + 1) % 4])];
                                positions[mid_dense] = (a_pos + b_pos) / 2.0;
                                placed[mid_dense] = true;
                            }
                        }

                        // Face point: intersection of lines connecting opposite edge midpoints
                        let e: [Vec2; 4] = std::array::from_fn(|i| positions[indexer.get_dense_index(&edge_coords[i])]);
                        let face_pos = line_intersection(e[0], e[2], e[1], e[3]);
                        let face_coord = AxialCoord::new(
                            (corners[0].q + corners[1].q + corners[2].q + corners[3].q) / 4,
                            (corners[0].r + corners[1].r + corners[2].r + corners[3].r) / 4,
                        );
                        let face_dense = indexer.get_dense_index(&face_coord);
                        positions[face_dense] = face_pos;
                        placed[face_dense] = true;
                    }
                }
            }
        }

        debug_assert!(
            placed.iter().all(|&p| p),
            "subdivision did not place all {} vertices ({} missing)",
            total,
            placed.iter().filter(|&&p| !p).count()
        );

        self.build_quad_mesh(positions)
    }

    /// Build a Quadrangulation from vertex positions using the patch topology.
    fn build_quad_mesh(&self, positions: Vec<Vec2>) -> Quadrangulation {
        let radius = 2u32.pow(self.subdivision);
        let indexer = AxialDenseIndexer::new(radius);
        let grid = 2i32.pow(self.subdivision);

        // Build boundary polygon (in spiral order)
        let mut polygon = Vec::new();
        for coord in AxialCoord::origin().spiral(radius) {
            if coord.is_boundary(radius) {
                polygon.push(VertexIndex::new(indexer.get_dense_index(&coord)));
            }
        }

        // Build quad indices
        let patch_indexer = PatchDenseIndexer::new(self.subdivision);
        let mut quads = Vec::with_capacity(patch_indexer.get_total_size());

        for p in 0..3i32 {
            for u in 0..grid {
                for v in 0..grid {
                    let patch = PatchCoord::new(p, u, v);
                    let quad = patch.quad_vertices(self.orientation, self.subdivision);
                    quads.push(std::array::from_fn(|i| {
                        VertexIndex::new(indexer.get_dense_index(&quad[i]))
                    }));
                }
            }
        }

        let hex_corners = AxialCoord::hex_corners(radius);
        let anchors: Vec<VertexIndex> = hex_corners
            .iter()
            .map(|c| VertexIndex::new(indexer.get_dense_index(c)))
            .collect();

        Quadrangulation::from_polygon(polygon, anchors, quads, positions).expect("valid patch mesh topology")
    }
}

fn line_intersection(p1: Vec2, p2: Vec2, p3: Vec2, p4: Vec2) -> Vec2 {
    let d1 = p2 - p1;
    let d2 = p4 - p3;
    let cross = d1.perp_dot(d2);
    if cross.abs() < 1e-10 {
        (p1 + p2 + p3 + p4) / 4.0
    } else {
        let t = (p3 - p1).perp_dot(d2) / cross;
        p1 + t * d1
    }
}
