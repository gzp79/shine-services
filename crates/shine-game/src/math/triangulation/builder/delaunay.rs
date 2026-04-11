use super::TriangulationBuilder;
use crate::{
    indexed::TypedIndex,
    math::triangulation::{predicates::in_circle, FaceIndex, Rot3Idx, VertexIndex},
};

impl<'a, const DELAUNAY: bool> TriangulationBuilder<'a, DELAUNAY> {
    /// Restores the Delaunay property globally across the entire triangulation
    /// by checking all edges iteratively until no violations remain.
    /// This is necessary after operations that can affect multiple regions of
    /// the triangulation (e.g., constraint edge insertion).
    pub fn delaunay_restore_global(&mut self) {
        if self.tri.dimension() != 2 {
            return;
        }

        // Collect all non-constrained edges to check
        let mut edges_to_check: Vec<(FaceIndex, Rot3Idx)> = Vec::new();

        for f in self.tri.face_index_iter() {
            if self.tri.is_infinite_face(f) {
                continue;
            }

            for i in 0..=2 {
                let edge = Rot3Idx::new(i);

                // Skip constrained edges
                if self.tri[f].constraints[edge] != 0 {
                    continue;
                }

                let neighbor = self.tri[f].neighbors[edge];

                // Skip edges on the convex hull
                if self.tri.is_infinite_face(neighbor) {
                    continue;
                }

                edges_to_check.push((f, edge));
            }
        }

        // Process edges until no more flips are needed
        // Use a simple iterative approach: keep checking all edges until a full pass
        // finds no violations
        let mut flipped = true;
        while flipped {
            flipped = false;

            for &(face, edge) in &edges_to_check {
                // Re-check validity after previous flips may have modified topology
                if !face.is_valid() || face.into_index() >= self.tri.face_count() {
                    continue;
                }

                // Skip constrained edges (may have changed)
                if self.tri[face].constraints[edge] != 0 {
                    continue;
                }

                let neighbor = self.tri[face].neighbors[edge];

                // Skip edges on the convex hull
                if self.tri.is_infinite_face(face) || self.tri.is_infinite_face(neighbor) {
                    continue;
                }

                let ni = self.tri[neighbor].find_neighbor(face).unwrap();

                let va = self.tri[face].vertices[edge];
                let vb = self.tri[face].vertices[edge.increment()];
                let vc = self.tri[face].vertices[edge.decrement()];
                let vd = self.tri[neighbor].vertices[ni];

                let pa = self.tri.p(va);
                let pb = self.tri.p(vb);
                let pc = self.tri.p(vc);
                let pd = self.tri.p(vd);

                // If vd is strictly inside the circumcircle, flip
                if in_circle(pa, pb, pc, pd) > 0 {
                    self.flip(face, edge);
                    flipped = true;
                }
            }
        }
    }

    /// Restores the Delaunay property around a newly inserted vertex
    /// using Lawson's edge-flip algorithm. Constrained edges are never
    /// flipped, making this suitable for Constrained Delaunay Triangulations.
    ///
    /// Call after `add_vertex` to get a (constrained) Delaunay triangulation.
    /// This is a no-op when dimension < 2.
    pub fn delaunay_restore_vertex(&mut self, vertex: VertexIndex) {
        if self.tri.dimension() != 2 {
            return;
        }

        // Collect all edges opposite to `vertex` (one per adjacent face).
        // These are the initial candidates for the Delaunay check.
        let mut stack: Vec<(FaceIndex, Rot3Idx)> = Vec::new();

        let start_face = self.tri[vertex].face;
        let start_vi = self.tri[start_face].find_vertex(vertex).unwrap();
        stack.push((start_face, start_vi));

        let mut cur_face = start_face;
        let mut cur_vi = start_vi;
        loop {
            // Move CCW: cross the edge at cur_vi.decrement() to the next face
            let next_face = self.tri[cur_face].neighbors[cur_vi.decrement()];
            if next_face == start_face {
                break;
            }
            let next_vi = self.tri[next_face].find_vertex(vertex).unwrap();
            stack.push((next_face, next_vi));
            cur_face = next_face;
            cur_vi = next_vi;
        }

        // Lawson flip: process edges until the stack is empty.
        while let Some((face, edge)) = stack.pop() {
            // Never flip constrained edges
            if self.tri[face].constraints[edge] != 0 {
                continue;
            }

            let neighbor = self.tri[face].neighbors[edge];

            // Skip edges on the convex hull (adjacent to an infinite face)
            if self.tri.is_infinite_face(face) || self.tri.is_infinite_face(neighbor) {
                continue;
            }

            let ni = self.tri[neighbor].find_neighbor(face).unwrap();

            // Vertices of the quad across the shared edge:
            //   va = opposite vertex in `face`   (= the inserted vertex, initially)
            //   vb, vc = shared edge endpoints
            //   vd = opposite vertex in `neighbor`
            let va = self.tri[face].vertices[edge];
            let vb = self.tri[face].vertices[edge.increment()];
            let vc = self.tri[face].vertices[edge.decrement()];
            let vd = self.tri[neighbor].vertices[ni];

            let pa = self.tri.p(va);
            let pb = self.tri.p(vb);
            let pc = self.tri.p(vc);
            let pd = self.tri.p(vd);

            // If vd is strictly inside the circumcircle of CCW triangle (va, vb, vc),
            // the edge is not locally Delaunay → flip.
            // Exact == 0 (co-circular) is left as-is to avoid infinite flip loops.
            if in_circle(pa, pb, pc, pd) > 0 {
                self.flip(face, edge);

                // After flip_face(face=f0, edge=i00):
                //   f0 vertices: [va, vb, vd] at [i00, i01, i02]
                //   f1 vertices: [vd, vc, va] at [ni, ni+1, ni-1]
                //
                // The two newly exposed edges opposite `va`:
                //   - (f0, i00): connects vb to vd  (was an edge of the old neighbor)
                //   - (f1, ni-1): connects vd to vc (was an edge of the old neighbor)
                stack.push((face, edge));
                stack.push((neighbor, ni.decrement()));
            }
        }
    }
}
