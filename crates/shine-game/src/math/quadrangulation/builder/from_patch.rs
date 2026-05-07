use crate::{
    indexed::{IdxVec, TypedIndex},
    math::quadrangulation::{
        quad_error::QuadError, AnchorIndex, Quad, QuadIndex, Quadrangulation, Rot4Idx, Vertex, VertexIndex,
    },
};
use glam::Vec2;
use std::collections::HashMap;

impl Quadrangulation {
    /// Build topology from a (boundary) polygon and interior quads.
    pub fn from_polygon(
        positions: Vec<Vec2>,
        polygon: Vec<VertexIndex>,
        quads: Vec<[VertexIndex; 4]>,
        anchors: Vec<VertexIndex>,
    ) -> Result<Self, QuadError> {
        let vertex_count = positions.len();

        // Minimal bounds checks to prevent index-out-of-bounds panics during construction.
        for &vi in &polygon {
            let idx = vi.into_index();
            if idx >= vertex_count {
                return Err(QuadError::Input(format!(
                    "Boundary vertex {} >= vertex_count {}",
                    idx, vertex_count
                )));
            }
        }
        for quad in &quads {
            for &vi in quad {
                let idx = vi.into_index();
                if idx >= vertex_count {
                    return Err(QuadError::Input(format!(
                        "Quad vertex {} >= vertex_count {}",
                        idx, vertex_count
                    )));
                }
            }
        }
        if polygon.len() % 2 != 0 {
            return Err(QuadError::Input(format!(
                "Boundary must have even length, got {}",
                polygon.len()
            )));
        }

        // 1. Build Vertices
        let infinite_vertex = VertexIndex::new(vertex_count);
        let mut vertices = IdxVec::with_capacity(vertex_count + 1);

        // Add finite vertices with positions
        for position in positions {
            let mut vertex = Vertex::new();
            vertex.position = position;
            vertices.push(vertex);
        }

        // Add infinite vertex (no position)
        vertices.push(Vertex::new());

        // 2. Build Quads
        let mut all_quads: Vec<Quad> = Vec::new();

        // Add finite quads
        for verts in quads {
            all_quads.push(Quad::with_vertices(verts[0], verts[1], verts[2], verts[3]));
        }

        // Add ghost for each edge-pair in the boundary polygon
        let infinite_quad_count = polygon.len();
        let infinite_quad_start = all_quads.len();
        for i in 0..infinite_quad_count / 2 {
            let v0 = polygon[2 * i];
            let v1 = polygon[2 * i + 1];
            let v2 = polygon[(2 * i + 2) % polygon.len()];
            all_quads.push(Quad::with_vertices(infinite_vertex, v2, v1, v0));
        }

        let mut quads = IdxVec::from(all_quads);

        // 3. Wire Up Neighbors
        // Build edge map: (v0, v1) -> (quad, edge_idx)
        let mut edge_map: HashMap<(VertexIndex, VertexIndex), (QuadIndex, Rot4Idx)> = HashMap::new();
        let quad_count = quads.len();
        for qi_idx in 0..quad_count {
            let qi = QuadIndex::new(qi_idx);
            for edge_idx in 0..4 {
                let edge = Rot4Idx::new(edge_idx);
                let v0 = quads[qi].vertices[edge];
                let v1 = quads[qi].vertices[edge.increment()];
                if edge_map.insert((v0, v1), (qi, edge)).is_some() {
                    return Err(QuadError::Input(format!(
                        "Duplicate edge ({}, {}) appears more than twice in quads",
                        v0.into_index(),
                        v1.into_index()
                    )));
                }
            }
        }

        // Wire up twin relationships
        for qi_idx in 0..quad_count {
            let qi = QuadIndex::new(qi_idx);
            for edge_idx in 0..4 {
                let edge = Rot4Idx::new(edge_idx);
                let v0 = quads[qi].vertices[edge];
                let v1 = quads[qi].vertices[edge.increment()];

                if let Some(&(twin_quad, _twin_edge)) = edge_map.get(&(v1, v0)) {
                    quads[qi].neighbors[edge] = twin_quad;
                } else {
                    return Err(QuadError::Input(format!(
                        "Incomplete topology: quad {} edge {} has no twin, missing edge ({}, {})",
                        qi_idx,
                        edge_idx,
                        v1.into_index(),
                        v0.into_index()
                    )));
                }
            }
        }

        // Update vertex → quad references
        for qi_idx in 0..quad_count {
            let qi = QuadIndex::new(qi_idx);
            for &vi in quads[qi].vertices.iter() {
                if vertices[vi].quad.is_none() {
                    vertices[vi].quad = qi;
                }
            }
        }
        vertices[infinite_vertex].quad = QuadIndex::new(infinite_quad_start);

        let mut anchor_vertices = IdxVec::<AnchorIndex, VertexIndex>::new();
        for anchor in anchors {
            anchor_vertices.push(anchor);
        }

        let mesh = Self {
            infinite_vertex,
            vertices,
            quads,
            anchor_vertices,
        };

        debug_assert_eq!(mesh.validator().validate_topology(), Ok(()));
        Ok(mesh)
    }

    /// Construct a 2x2 grid of 4 quads:
    /// ```text
    ///  6----7----8
    ///  | Q2 | Q3 |
    ///  3----4----5
    ///  | Q0 | Q1 |
    ///  0----1----2
    /// ```
    /// Q0=[0,1,4,3]  Q1=[1,2,5,4]  Q2=[3,4,7,6]  Q3=[4,5,8,7]  (CCW)
    /// Interior: 4.  Boundary: 8 vertices (0,1,2,5,8,7,6,3).
    /// Simple 2x2 grid topology for testing.
    pub fn new_2x2_grid() -> Self {
        let quads: Vec<_> = [[0, 1, 4, 3], [1, 2, 5, 4], [3, 4, 7, 6], [4, 5, 8, 7]]
            .map(|v| v.map(VertexIndex::new))
            .to_vec();
        let boundaries: Vec<_> = [0, 1, 2, 5, 8, 7, 6, 3].map(VertexIndex::new).to_vec();
        let anchors: Vec<_> = [0, 2, 8, 6].map(VertexIndex::new).to_vec();
        let positions = vec![
            Vec2::new(0.0, 0.0), // 0
            Vec2::new(1.0, 0.0), // 1
            Vec2::new(2.0, 0.0), // 2
            Vec2::new(0.0, 1.0), // 3
            Vec2::new(1.0, 1.0), // 4
            Vec2::new(2.0, 1.0), // 5
            Vec2::new(0.0, 2.0), // 6
            Vec2::new(1.0, 2.0), // 7
            Vec2::new(2.0, 2.0), // 8
        ];
        Self::from_polygon(positions, boundaries, quads, anchors).expect("valid topology")
    }
}
