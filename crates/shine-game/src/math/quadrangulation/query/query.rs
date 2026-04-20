use crate::math::quadrangulation::{QuadEdge, QuadEdgeType, QuadVertex, Quadrangulation, VertIdx};
use std::iter;

impl Quadrangulation {
    /// Get the twin edge (edge on the neighboring quad).
    pub fn edge_twin(&self, qe: QuadEdge) -> QuadEdge {
        let neighbor_quad = self.quads[qe.quad].neighbors[qe.edge];
        let neighbor_edge = self.quads[neighbor_quad].find_neighbor(qe.quad).unwrap();
        QuadEdge {
            quad: neighbor_quad,
            edge: neighbor_edge,
        }
    }

    /// Classify an edge between two vertices.
    pub fn edge_type(&self, a: VertIdx, b: VertIdx) -> QuadEdgeType {
        // Find the quad containing edge a→b
        for qv in self.vertex_ring_ccw(a) {
            if self.vi(qv.next()) == b {
                let edge = qv.outgoing_edge();
                let neighbor = self.edge_twin(edge);

                // Boundary if either side of the edge is a ghost quad
                if self.is_infinite_quad(qv.quad) || self.is_infinite_quad(neighbor.quad) {
                    return QuadEdgeType::Boundary;
                } else {
                    return QuadEdgeType::Interior;
                }
            }
        }

        QuadEdgeType::NotAnEdge
    }

    pub fn vertex_ring_ccw_repeated(&self, vi: VertIdx, max_loops: usize) -> impl Iterator<Item = QuadVertex> + '_ {
        self.vertex_ring_impl(vi, max_loops, true)
    }

    pub fn vertex_ring_ccw(&self, vi: VertIdx) -> impl Iterator<Item = QuadVertex> + '_ {
        self.vertex_ring_impl(vi, 1, true)
    }

    pub fn vertex_ring_cw_repeated(&self, vi: VertIdx, max_loops: usize) -> impl Iterator<Item = QuadVertex> + '_ {
        self.vertex_ring_impl(vi, max_loops, false)
    }

    pub fn vertex_ring_cw(&self, vi: VertIdx) -> impl Iterator<Item = QuadVertex> + '_ {
        self.vertex_ring_impl(vi, 1, false)
    }

    fn vertex_ring_impl(&self, vi: VertIdx, max_loops: usize, ccw: bool) -> impl Iterator<Item = QuadVertex> + '_ {
        let quad = self.vertices[vi].quad;
        let local = self[quad].find_vertex(vi).unwrap();
        let start_qv = QuadVertex { quad, local };

        let mut current = start_qv;
        let mut loops_remaining = max_loops;
        let mut done = false;

        iter::from_fn(move || {
            if done {
                return None;
            }

            let result = current;

            if ccw {
                // CCW: Move via incoming edge (backward around the vertex)
                let edge = current.incoming_edge();
                let neighbor = self.edge_twin(edge);
                current = neighbor.start();
            } else {
                // CW: Move via outgoing edge (forward around the vertex)
                let edge = current.outgoing_edge();
                let neighbor = self.edge_twin(edge);
                current = neighbor.end();
            }

            // Check if we've completed a ring loop
            if current.quad == start_qv.quad {
                assert!(
                    loops_remaining > 0,
                    "vertex_ring completed too many loops - likely skip_while/take_while didn't terminate"
                );
                loops_remaining -= 1;
                if loops_remaining == 0 {
                    done = true;
                }
            }

            Some(result)
        })
    }
}
