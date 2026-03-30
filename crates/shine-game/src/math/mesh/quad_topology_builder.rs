use super::{quad_error::QuadTopologyError, quad_topology::*, QuadEdge, QuadIdx, QuadTopology, VertIdx};
use crate::indexed::{IdxVec, TypedIndex};
use std::collections::HashMap;
use std::collections::HashSet;

impl QuadTopology {
    /// Build topology from a boundary polygon and interior quads.
    pub fn from_polygon(
        vertex_count: usize,
        polygon: Vec<VertIdx>,
        quads: Vec<[VertIdx; 4]>,
    ) -> Result<Self, QuadTopologyError> {
        // Validate boundary length is even
        if !polygon.len().is_multiple_of(2) {
            return Err(QuadTopologyError::OddBoundary(polygon.len()));
        }

        // Validate boundary vertices in range
        for &vi in &polygon {
            let idx = vi.into_index();
            if idx >= vertex_count {
                return Err(QuadTopologyError::BoundaryVertexOutOfRange { vertex: idx, vertex_count });
            }
        }

        // Validate boundary vertices unique
        {
            let mut seen = HashSet::new();
            for &vi in &polygon {
                let idx = vi.into_index();
                if !seen.insert(idx) {
                    return Err(QuadTopologyError::DuplicateBoundaryVertex(idx));
                }
            }
        }

        // Validate quad vertices in range and don't reference ghost
        for quad in &quads {
            for &vi in quad {
                let idx = vi.into_index();
                if idx >= vertex_count {
                    return Err(QuadTopologyError::QuadVertexOutOfRange { vertex: idx, vertex_count });
                }
            }
        }

        // Generate ghost quads
        let mut all_quads = quads;
        let ghost_vertex = VertIdx::new(vertex_count);
        let ghost_quad_count = polygon.len() / 2;
        for j in 0..ghost_quad_count {
            let i = j * 2;
            let v0 = polygon[i];
            let v1 = polygon[(i + 1) % polygon.len()];
            let v2 = polygon[(i + 2) % polygon.len()];
            all_quads.push([ghost_vertex, v2, v1, v0]);
        }
        let quads = IdxVec::from_vec(all_quads);
        let ghost_quad_start = quads.len() - ghost_quad_count;

        // Build edge map: (v0, v1) -> (quad, edge_idx)
        let mut edge_map: HashMap<(VertIdx, VertIdx), (QuadIdx, u8)> = HashMap::new();
        for (qi, quad) in quads.iter().enumerate() {
            for edge_idx in 0..4 {
                let v0 = quad[edge_idx];
                let v1 = quad[(edge_idx + 1) % 4];
                edge_map.insert((v0, v1), (QuadIdx::new(qi), edge_idx as u8));
            }
        }

        // Build edge twin structure (neighbor across each edge)
        let mut edge_twins = IdxVec::new();
        for (qi, quad) in quads.iter().enumerate() {
            let mut twins = [QuadEdge { quad: QuadIdx::NONE, edge: 0 }; 4];
            for edge_idx in 0..4 {
                let v0 = quad[edge_idx];
                let v1 = quad[(edge_idx + 1) % 4];

                if let Some(&(twin_quad, twin_edge)) = edge_map.get(&(v1, v0)) {
                    twins[edge_idx] = QuadEdge {
                        quad: twin_quad,
                        edge: twin_edge,
                    };
                } else {
                    return Err(QuadTopologyError::IncompleteTopology {
                        quad: qi,
                        edge: edge_idx,
                        vertices: (v0.into_index(), v1.into_index()),
                    });
                }
            }

            edge_twins.push(twins);
        }

        // Build vertex → quad map (includes ghost vertex)
        let mut vertex_quad = vec![QuadVertex { quad: QuadIdx::NONE, local: 0 }; vertex_count + 1];
        for (qi, quad) in quads.iter().enumerate() {
            for (local, &vi) in quad.iter().enumerate() {
                let idx = vi.into_index();
                if vertex_quad[idx].quad.is_none() {
                    vertex_quad[idx] = QuadVertex {
                        quad: QuadIdx::new(qi),
                        local: local as u8,
                    };
                }
            }
        }
        vertex_quad[ghost_vertex.into_index()] = QuadVertex {
            quad: QuadIdx::new(ghost_quad_start), // the first ghost-quad
            local: 0,
        };

        let topology = Self {
            vertex_count,
            ghost_quad_count,
            quads,
            edge_twins,
            vertex_quad,
        };

        topology.validate()?;
        Ok(topology)
    }
}
