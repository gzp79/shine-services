use crate::{
    indexed::TypedIndex,
    math::quadrangulation::{AnchorIndex, QuadEdge, QuadError, QuadIndex, Rot4Idx, Validator, VertexIndex},
};

impl<'a> Validator<'a> {
    pub fn validate_topology(&self) -> Result<(), QuadError> {
        self.validate_vertices()?;
        self.validate_quads()?;
        self.validate_edge_twins()?;
        self.validate_infinite_structure()?;
        self.validate_vertex_rings()?;
        self.validate_reachability()?;
        self.validate_anchors()?;
        Ok(())
    }

    fn validate_vertices(&self) -> Result<(), QuadError> {
        // Check all vertices have an associated quad that references them correctly
        for vi_idx in 0..=self.mesh.finite_vertex_count() {
            let vi = VertexIndex::new(vi_idx);
            let vertex = &self.mesh.vertices[vi];
            if vertex.quad.is_none() {
                return Err(QuadError::VertexHasNoQuad(vi_idx));
            }
            let local = self.mesh[vertex.quad].find_vertex(vi).unwrap();
            let actual = self.mesh[vertex.quad].vertices[local];
            if actual != vi {
                return Err(QuadError::VertexQuadMismatch {
                    vertex: vi_idx,
                    actual: actual.into_index(),
                });
            }
        }
        Ok(())
    }

    fn validate_quads(&self) -> Result<(), QuadError> {
        // Check no degenerate quads (all 4 vertices distinct)
        for qi_idx in 0..self.mesh.quads.len() {
            let verts = &self.mesh.quads[QuadIndex::new(qi_idx)].vertices;
            for i in 0..4 {
                let i_idx = Rot4Idx::new(i);
                for j in (i + 1)..4 {
                    let j_idx = Rot4Idx::new(j);
                    if verts[i_idx] == verts[j_idx] {
                        return Err(QuadError::DegenerateQuad {
                            quad: qi_idx,
                            vertex: verts[i_idx].into_index(),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_edge_twins(&self) -> Result<(), QuadError> {
        // Check edge twin bidirectionality and involution
        for qi_idx in 0..self.mesh.quads.len() {
            let qi = QuadIndex::new(qi_idx);
            for edge_idx in 0..4 {
                let qe = QuadEdge {
                    quad: qi,
                    edge: Rot4Idx::new(edge_idx),
                };
                let twin = self.mesh.edge_twin(qe);

                // Check twin vertices are reversed
                let (v0, v1) = self.mesh.edge_vertices(qe);
                let (twin_v0, twin_v1) = self.mesh.edge_vertices(twin);

                if v0 != twin_v1 || v1 != twin_v0 {
                    return Err(QuadError::InvalidEdgeTwin { quad: qi_idx, edge: edge_idx });
                }

                // Check twin of twin points back to original
                let round_trip = self.mesh.edge_twin(twin);
                if round_trip.quad != qe.quad || round_trip.edge != qe.edge {
                    return Err(QuadError::EdgeTwinNotInvolution { quad: qi_idx, edge: edge_idx });
                }
            }
        }
        Ok(())
    }

    fn validate_infinite_structure(&self) -> Result<(), QuadError> {
        let infinite_vertex = self.mesh.infinite_vertex();

        // Check each infinite quad has exactly one infinite vertex
        for qi in self.mesh.infinite_quad_index_iter() {
            let verts = self.mesh.quad_vertices(qi);
            let infinite_count = verts.iter().filter(|&&v| v == infinite_vertex).count();

            if infinite_count != 1 {
                return Err(QuadError::InvalidInfiniteQuadStructure {
                    quad: qi.into_index(),
                    count: infinite_count,
                });
            }
        }

        // Verify infinite_quad_count matches actual infinite quad count
        let actual_infinite_count = self
            .mesh
            .quads
            .iter()
            .filter(|quad| quad.vertices.iter().any(|&v| v == infinite_vertex))
            .count();
        if actual_infinite_count != self.mesh.infinite_quad_count() {
            return Err(QuadError::InfiniteQuadCountMismatch {
                expected: self.mesh.infinite_quad_count(),
                actual: actual_infinite_count,
            });
        }

        // Verify infinite quads are contiguous at the end of the quad array
        let finite_count = self.mesh.quads.len() - self.mesh.infinite_quad_count();
        for qi_idx in 0..finite_count {
            if self.mesh.is_infinite_quad(QuadIndex::new(qi_idx)) {
                // Find the first finite quad after this infinite quad
                let finite_after = (qi_idx + 1..self.mesh.quads.len())
                    .find(|&i| !self.mesh.is_infinite_quad(QuadIndex::new(i)))
                    .unwrap_or(qi_idx);
                return Err(QuadError::InfiniteQuadsNotCompact {
                    infinite_quad: qi_idx,
                    finite_quad: finite_after,
                });
            }
        }

        Ok(())
    }

    fn validate_vertex_rings(&self) -> Result<(), QuadError> {
        // Check vertex rings form closed loops (real vertices and ghost vertex)
        for vi_idx in 0..self.mesh.finite_vertex_count() {
            self.validate_vertex_ring(VertexIndex::new(vi_idx))?;
        }
        self.validate_vertex_ring(self.mesh.infinite_vertex())?;
        Ok(())
    }

    /// Helper to validate a single vertex ring forms a closed loop
    fn validate_vertex_ring(&self, vi: VertexIndex) -> Result<(), QuadError> {
        let vi_idx = vi.into_index();
        let ring: Vec<_> = self.mesh.vertex_ring_ccw(vi).collect();

        if ring.is_empty() {
            return Err(QuadError::VertexRingNotClosed { vertex: vi_idx });
        }

        // Verify all ring elements reference the correct vertex
        for qv in &ring {
            let vertex_at_pos = self.mesh.quads[qv.quad].vertices[qv.local];
            if vertex_at_pos != vi {
                return Err(QuadError::VertexRingNotClosed { vertex: vi_idx });
            }
        }

        // Check ring closure: next position after last should reference same vertex
        let last = ring[ring.len() - 1];
        let incoming = last.incoming_edge();
        let neighbor = self.mesh.edge_twin(incoming);
        let next_pos = neighbor.start();
        let next_vertex = self.mesh.quads[next_pos.quad].vertices[next_pos.local];

        // Must be the same vertex (forms a cycle around vi)
        if next_vertex != vi {
            return Err(QuadError::VertexRingNotClosed { vertex: vi_idx });
        }

        // Next position should be in the ring (forms a closed cycle)
        let next_in_ring = ring
            .iter()
            .any(|qv| qv.quad == next_pos.quad && qv.local == next_pos.local);
        if !next_in_ring {
            return Err(QuadError::VertexRingNotClosed { vertex: vi_idx });
        }

        Ok(())
    }

    fn validate_reachability(&self) -> Result<(), QuadError> {
        // Check all quads are reachable from vertex rings
        let mut reachable = vec![false; self.mesh.quads.len()];
        for vi_idx in 0..=self.mesh.finite_vertex_count() {
            for qv in self.mesh.vertex_ring_ccw(VertexIndex::new(vi_idx)) {
                reachable[qv.quad.into_index()] = true;
            }
        }
        for (qi_idx, &reached) in reachable.iter().enumerate() {
            if !reached {
                return Err(QuadError::UnreachableQuad { quad: qi_idx });
            }
        }
        Ok(())
    }

    fn validate_anchors(&self) -> Result<(), QuadError> {
        // Check anchor vertices are boundary vertices in correct cyclic order
        if !self.mesh.anchor_vertices.is_empty() {
            let boundary: Vec<_> = self.mesh.boundary_vertices().collect();

            // Check all anchor vertices are in the boundary
            for anchor_idx in self.mesh.anchor_index_iter() {
                let anchor_v = self.mesh.anchor_vertices[anchor_idx];
                if !boundary.contains(&anchor_v) {
                    return Err(QuadError::InvalidAnchorEdge { edge: anchor_idx.into_index() });
                }
            }

            // Verify cyclic ordering: each anchor must follow the previous along the boundary
            let first_anchor_v = self.mesh.anchor_vertices[AnchorIndex::new(0)];
            let first_pos = boundary.iter().position(|&b| b == first_anchor_v).unwrap();
            let mut search_start = first_pos;

            for anchor_idx in 1..self.mesh.anchor_vertices.len() {
                let anchor_v = self.mesh.anchor_vertices[AnchorIndex::new(anchor_idx)];
                let found =
                    (1..boundary.len()).find(|&offset| boundary[(search_start + offset) % boundary.len()] == anchor_v);
                match found {
                    Some(offset) => search_start = (search_start + offset) % boundary.len(),
                    None => return Err(QuadError::InvalidAnchorEdge { edge: anchor_idx - 1 }),
                }
            }
        }
        Ok(())
    }
}
