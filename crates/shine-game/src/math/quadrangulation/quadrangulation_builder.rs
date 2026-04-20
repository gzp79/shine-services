use super::{quad_error::QuadError, Quad, QuadIdx, Quadrangulation, Rot4Idx, VertIdx, Vertex};
use crate::indexed::{IdxVec, TypedIndex};
use glam::Vec2;
use std::collections::HashMap;

impl Quadrangulation {
    /// Build topology from a boundary polygon and interior quads.
    pub fn from_polygon(
        polygon: Vec<VertIdx>,
        anchors: Vec<VertIdx>,
        quads: Vec<[VertIdx; 4]>,
        positions: Vec<Vec2>,
    ) -> Result<Self, QuadError> {
        let vertex_count = positions.len();

        // Minimal bounds checks to prevent index-out-of-bounds panics during construction.
        // All other invariants (odd boundary, duplicates, degenerates, etc.) are caught by validate().
        for &vi in &polygon {
            let idx = vi.into_index();
            if idx >= vertex_count {
                return Err(QuadError::BoundaryVertexOutOfRange { vertex: idx, vertex_count });
            }
        }
        for quad in &quads {
            for &vi in quad {
                let idx = vi.into_index();
                if idx >= vertex_count {
                    return Err(QuadError::QuadVertexOutOfRange { vertex: idx, vertex_count });
                }
            }
        }

        // Generate ghost quads: [ghost, v2, v1, v0]
        // Boundary edges are reversed (v2->v1, v1->v0) to match twin edges from real quads
        let mut all_quads: Vec<Quad> = quads
            .into_iter()
            .map(|verts| Quad::with_vertices(verts[0], verts[1], verts[2], verts[3]))
            .collect();
        let infinite_vertex = VertIdx::new(vertex_count);
        let infinite_quad_count = polygon.len() / 2;
        for j in 0..infinite_quad_count {
            let i = j * 2;
            let v0 = polygon[i];
            let v1 = polygon[(i + 1) % polygon.len()];
            let v2 = polygon[(i + 2) % polygon.len()];
            all_quads.push(Quad::with_vertices(infinite_vertex, v2, v1, v0));
        }
        let mut quads = IdxVec::from(all_quads);
        let infinite_quad_start = quads.len() - infinite_quad_count;

        // Build edge map: (v0, v1) -> (quad, edge_idx)
        let mut edge_map: HashMap<(VertIdx, VertIdx), (QuadIdx, Rot4Idx)> = HashMap::new();
        let quad_count = quads.len();
        for qi_idx in 0..quad_count {
            let qi = QuadIdx::new(qi_idx);
            for edge_idx in 0..4 {
                let edge = Rot4Idx::new(edge_idx);
                let v0 = quads[qi].vertices[edge];
                let v1 = quads[qi].vertices[edge.increment()];
                edge_map.insert((v0, v1), (qi, edge));
            }
        }

        // Build edge twin structure (neighbor across each edge).
        // Must check here — leaving NONE twins would panic in validate().
        for qi_idx in 0..quad_count {
            let qi = QuadIdx::new(qi_idx);
            for edge_idx in 0..4 {
                let edge = Rot4Idx::new(edge_idx);
                let v0 = quads[qi].vertices[edge];
                let v1 = quads[qi].vertices[edge.increment()];

                if let Some(&(twin_quad, _twin_edge)) = edge_map.get(&(v1, v0)) {
                    quads[qi].neighbors[edge] = twin_quad;
                } else {
                    return Err(QuadError::IncompleteTopology {
                        quad: qi_idx,
                        edge: edge_idx,
                        vertices: (v0.into_index(), v1.into_index()),
                    });
                }
            }
        }

        // Build vertex → quad map (includes ghost vertex) and set positions
        let mut vertices = IdxVec::with_capacity(vertex_count + 1);
        for i in 0..vertex_count {
            let mut vertex = Vertex::new();
            vertex.position = positions[i];
            vertices.push(vertex);
        }
        // Add infinite vertex with no position
        vertices.push(Vertex::new());

        for qi_idx in 0..quad_count {
            let qi = QuadIdx::new(qi_idx);
            for &vi in quads[qi].vertices.iter() {
                if vertices[vi].quad.is_none() {
                    vertices[vi].quad = qi;
                }
            }
        }

        // Ghost vertex: use first ghost quad (ring traversal visits all regardless of start)
        let infinite_vertex = VertIdx::new(vertex_count);
        vertices[infinite_vertex].quad = QuadIdx::new(infinite_quad_start);

        let topology = Self {
            infinite_vertex,
            vertices,
            quads,
            anchor_vertices: anchors,
        };

        topology.validate()?;
        Ok(topology)
    }
}
