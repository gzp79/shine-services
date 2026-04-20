use crate::math::quadrangulation::{QuadEdge, QuadEdgeType, QuadIndex, QuadVertex, Quadrangulation, VertexIndex};
use glam::Vec2;
use std::iter;

impl Quadrangulation {
    pub fn edge_twin(&self, qe: QuadEdge) -> QuadEdge {
        let neighbor_quad = self.quads[qe.quad].neighbors[qe.edge];
        let neighbor_edge = self.quads[neighbor_quad].find_neighbor(qe.quad).unwrap();
        QuadEdge {
            quad: neighbor_quad,
            edge: neighbor_edge,
        }
    }

    pub fn edge_vertices(&self, qe: QuadEdge) -> (VertexIndex, VertexIndex) {
        let quad = &self.quads[qe.quad].vertices;
        (quad[qe.edge], quad[qe.edge.increment()])
    }

    pub fn quad_vertices(&self, qi: QuadIndex) -> &[VertexIndex] {
        self.quads[qi].vertices.as_slice()
    }

    pub fn edge_type(&self, a: VertexIndex, b: VertexIndex) -> QuadEdgeType {
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

    pub fn adjacent_vertices(&self, vi: VertexIndex) -> impl Iterator<Item = VertexIndex> + '_ {
        self.vertex_ring_ccw(vi).map(move |qv| self.vi(qv.next()))
    }

    /// Compute the average position of adjacent vertices.
    /// Infinite vertices are excluded from the average.
    /// Returns the vertex's own position if it has no finite neighbors.
    pub fn average_adjacent_positions(&self, vi: VertexIndex) -> Vec2 {
        assert_ne!(vi, self.infinite_vertex());

        let (sum, count) = self
            .adjacent_vertices(vi)
            .filter(|&v| !self.is_infinite_vertex(v))
            .fold((Vec2::ZERO, 0), |(sum, count), v| (sum + self.p(v), count + 1));

        if count > 0 {
            sum / count as f32
        } else {
            self.p(vi)
        }
    }

    pub fn vertex_ring_ccw_repeated(&self, vi: VertexIndex, max_loops: usize) -> impl Iterator<Item = QuadVertex> + '_ {
        self.vertex_ring_impl(vi, max_loops, true)
    }

    pub fn vertex_ring_ccw(&self, vi: VertexIndex) -> impl Iterator<Item = QuadVertex> + '_ {
        self.vertex_ring_impl(vi, 1, true)
    }

    pub fn vertex_ring_cw_repeated(&self, vi: VertexIndex, max_loops: usize) -> impl Iterator<Item = QuadVertex> + '_ {
        self.vertex_ring_impl(vi, max_loops, false)
    }

    pub fn vertex_ring_cw(&self, vi: VertexIndex) -> impl Iterator<Item = QuadVertex> + '_ {
        self.vertex_ring_impl(vi, 1, false)
    }

    fn vertex_ring_impl(&self, vi: VertexIndex, max_loops: usize, ccw: bool) -> impl Iterator<Item = QuadVertex> + '_ {
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
