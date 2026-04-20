use super::{quad_error::QuadError, Quad, QuadEdge, QuadIdx, QuadTopology, VertIdx, Vertex};
use crate::indexed::{IdxVec, TypedIndex};
use std::collections::HashMap;

impl QuadTopology {
    /// Build topology from a boundary polygon and interior quads.
    pub fn from_polygon(
        vertex_count: usize,
        polygon: Vec<VertIdx>,
        anchors: Vec<VertIdx>,
        quads: Vec<[VertIdx; 4]>,
    ) -> Result<Self, QuadError> {
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
        let mut all_quads: Vec<Quad> = quads.into_iter().map(|verts| Quad::with_vertices(verts[0], verts[1], verts[2], verts[3])).collect();
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
        let mut edge_map: HashMap<(VertIdx, VertIdx), (QuadIdx, u8)> = HashMap::new();
        let quad_count = quads.len();
        for qi_idx in 0..quad_count {
            let qi = QuadIdx::new(qi_idx);
            for edge_idx in 0..4 {
                let v0 = quads[qi].vertices[edge_idx];
                let v1 = quads[qi].vertices[(edge_idx + 1) % 4];
                edge_map.insert((v0, v1), (qi, edge_idx as u8));
            }
        }

        // Build edge twin structure (neighbor across each edge).
        // Must check here — leaving NONE twins would panic in validate().
        for qi_idx in 0..quad_count {
            let qi = QuadIdx::new(qi_idx);
            for edge_idx in 0..4 {
                let v0 = quads[qi].vertices[edge_idx];
                let v1 = quads[qi].vertices[(edge_idx + 1) % 4];

                if let Some(&(twin_quad, _twin_edge)) = edge_map.get(&(v1, v0)) {
                    quads[qi].neighbors[edge_idx] = twin_quad;
                } else {
                    return Err(QuadError::IncompleteTopology {
                        quad: qi_idx,
                        edge: edge_idx,
                        vertices: (v0.into_index(), v1.into_index()),
                    });
                }
            }
        }

        // Build vertex → quad map (includes ghost vertex)
        let mut vertices = IdxVec::with_capacity(vertex_count + 1);
        for _ in 0..=vertex_count {
            vertices.push(Vertex::new());
        }
        for qi_idx in 0..quad_count {
            let qi = QuadIdx::new(qi_idx);
            for &vi in &quads[qi].vertices {
                if vertices[vi].quad.is_none() {
                    vertices[vi].quad = qi;
                }
            }
        }

        // Ghost vertex: use first ghost quad (ring traversal visits all regardless of start)
        let infinite_vertex = VertIdx::new(vertex_count);
        vertices[infinite_vertex].quad = QuadIdx::new(infinite_quad_start);

        let topology = Self {
            vertex_count,
            infinite_quad_count,
            quads,
            vertices,
            anchor_vertices: anchors,
        };

        topology.validate()?;
        Ok(topology)
    }
}
