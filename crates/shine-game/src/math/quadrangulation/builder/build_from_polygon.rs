use crate::{
    indexed::{IdxVec, TypedIndex},
    math::quadrangulation::{
        builder::QuadBuilder, quad_error::QuadError, AnchorIndex, Quad, QuadIndex, Quadrangulation, Rot4Idx, Vertex,
        VertexIndex,
    },
};
use glam::Vec2;
use std::collections::HashMap;

impl<'a> QuadBuilder<'a> {
    /// Build topology from a (boundary) polygon and interior quads into an empty quadrangulation.
    ///
    /// - `positions`: vertex positions
    /// - `polygon`: boundary vertex indices in CCW order (must have even length)
    /// - `quads`: interior quad index quads (each 4 CCW vertex indices)
    /// - `anchors`: corner vertices marking anchor edges
    pub fn build_from_polygon(
        &mut self,
        positions: Vec<Vec2>,
        polygon: Vec<VertexIndex>,
        quads: Vec<[VertexIndex; 4]>,
        anchors: Vec<VertexIndex>,
    ) -> Result<(), QuadError> {
        assert!(
            self.quad.is_empty(),
            "build_from_polygon requires an empty quadrangulation"
        );

        self.state.dump(1, "build_from_polygon_input", |svg| {
            svg.add_default_styles();
            let edges: Vec<(usize, usize)> = quads
                .iter()
                .flat_map(|q| [(0, 1), (1, 2), (2, 3), (3, 0)].map(|(a, b)| (q[a].into_index(), q[b].into_index())))
                .collect();
            svg.add_points_and_edges(&positions, &edges, "vert", "vert-text", "edge");

            let poly_edges: Vec<(usize, usize)> = (0..polygon.len())
                .map(|i| (polygon[i].into_index(), polygon[(i + 1) % polygon.len()].into_index()))
                .collect();
            svg.add_points_and_edges(&positions, &poly_edges, "vert", "", "edge-constraint");
        });

        let vertex_count = positions.len();

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
        if !polygon.len().is_multiple_of(2) {
            return Err(QuadError::Input(format!(
                "Boundary must have even length, got {}",
                polygon.len()
            )));
        }

        // 1. Build Vertices
        let mut vertices = IdxVec::with_capacity(vertex_count + 1);
        for position in positions {
            let mut vertex = Vertex::new();
            vertex.position = position;
            vertices.push(vertex);
        }
        let infinite_vertex = VertexIndex::new(vertices.len());
        vertices.push(Vertex::new());

        // 2. Build Quads
        let mut all_quads: Vec<Quad> = Vec::new();

        for verts in &quads {
            all_quads.push(Quad::with_vertices(verts[0], verts[1], verts[2], verts[3]));
        }

        let infinite_quad_start = all_quads.len();
        for i in 0..polygon.len() / 2 {
            let v0 = polygon[2 * i];
            let v1 = polygon[2 * i + 1];
            let v2 = polygon[(2 * i + 2) % polygon.len()];
            all_quads.push(Quad::with_vertices(infinite_vertex, v2, v1, v0));
        }

        let mut all_quads_idx = IdxVec::from(all_quads);

        // 3. Wire Up Neighbors
        let mut edge_map: HashMap<(VertexIndex, VertexIndex), (QuadIndex, Rot4Idx)> = HashMap::new();
        let quad_count = all_quads_idx.len();
        for qi_idx in 0..quad_count {
            let qi = QuadIndex::new(qi_idx);
            for edge_idx in 0..4 {
                let edge = Rot4Idx::new(edge_idx);
                let v0 = all_quads_idx[qi].vertices[edge];
                let v1 = all_quads_idx[qi].vertices[edge.increment()];
                if edge_map.insert((v0, v1), (qi, edge)).is_some() {
                    return Err(QuadError::Input(format!(
                        "Duplicate edge ({}, {}) appears more than twice in quads",
                        v0.into_index(),
                        v1.into_index()
                    )));
                }
            }
        }

        for qi_idx in 0..quad_count {
            let qi = QuadIndex::new(qi_idx);
            for edge_idx in 0..4 {
                let edge = Rot4Idx::new(edge_idx);
                let v0 = all_quads_idx[qi].vertices[edge];
                let v1 = all_quads_idx[qi].vertices[edge.increment()];
                if let Some(&(twin_quad, _)) = edge_map.get(&(v1, v0)) {
                    all_quads_idx[qi].neighbors[edge] = twin_quad;
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
            for &vi in all_quads_idx[qi].vertices.iter() {
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

        self.quad.infinite_vertex = infinite_vertex;
        self.quad.vertices = vertices;
        self.quad.quads = all_quads_idx;
        self.quad.anchor_vertices = anchor_vertices;

        debug_assert_eq!(self.quad.validator().validate_topology(), Ok(()));

        self.state.dump(1, "build_from_polygon_result", |svg| {
            svg.add_quad(self.quad, std::iter::empty());
        });

        Ok(())
    }
}

impl Quadrangulation {
    /// Build topology from a (boundary) polygon and interior quads.
    pub fn from_polygon(
        positions: Vec<Vec2>,
        polygon: Vec<VertexIndex>,
        quads: Vec<[VertexIndex; 4]>,
        anchors: Vec<VertexIndex>,
    ) -> Result<Self, QuadError> {
        let mut mesh = Self::new();
        mesh.builder().build_from_polygon(positions, polygon, quads, anchors)?;
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
