use crate::{
    indexed::TypedIndex,
    math::triangulation::{
        builder::state::BuilderState, predicates::in_circle, EdgeCirculator, FaceEdge, Rot3Idx, TriangulationBuilder,
        VertexIndex,
    },
};

impl<'a, const DELAUNAY: bool> TriangulationBuilder<'a, DELAUNAY> {
    pub fn delaunay_push_vertex(&mut self, vertex: VertexIndex) {
        debug_assert!(self.tri.dimension() == 2);

        let mut circulator = EdgeCirculator::new(&self.tri, vertex);
        let start = *circulator.current();

        // Push the edge opposite to the vertex in each triangle
        self.state.delaunay_push_edge(&self.tri, start.next());
        circulator.advance_ccw();

        while *circulator.current() != start {
            self.state.delaunay_push_edge(&self.tri, circulator.current().next());
            circulator.advance_ccw();
        }
    }

    pub fn delaunay_push_edge(&mut self, edge: FaceEdge) {
        debug_assert!(self.tri.dimension() == 2);
        self.state.delaunay_push_edge(&self.tri, edge);
    }

    /// Full mesh Delaunay refinement processes all edges in the triangulation.
    pub fn delaunay_refine_all(&mut self) {
        if self.tri.dimension() != 2 {
            return;
        }

        log::trace!("Running full delaunay process on all edges");

        // Clear the stack - we're doing a full mesh process
        let mut stack = self.state.lock_delaunay_stack();
        stack.clear();

        // Iterate through all faces and push all edges to the stack
        for face_idx in self.tri.face_index_iter() {
            if !face_idx.is_valid() || self.tri.is_infinite_face(face_idx) {
                continue;
            }

            for i in 0..3 {
                let e = Rot3Idx::new(i);
                BuilderState::delaunay_push_edge_into(&mut stack, &self.tri, FaceEdge::new(face_idx, e));
            }
        }
        self.state.unlock_delaunay_stack(stack);

        self.delaunay_run();
    }

    // Lawson flip: process edges until the stack is empty.
    pub fn delaunay_run(&mut self) {
        if self.tri.dimension() != 2 {
            return;
        }

        // Lock the stack for processing
        let mut stack = self.state.lock_delaunay_stack();

        log::trace!("Running delaunay with {} edges in stack", stack.len());
        self.state.dump(1, "before_delaunay", |dump| {
            dump.add_tri(&self.tri, [(stack.as_slice(), "edge-delaunay", false)]);
        });

        while let Some(FaceEdge { triangle: face, edge }) = stack.pop() {
            // Never flip constrained edges
            if self.tri[face].constraints[edge] != 0 {
                log::trace!("Skipping constrained: ({}, {})", face.into_index(), edge.into_index());
                continue;
            }

            let neighbor = self.tri[face].neighbors[edge];

            // Skip edges on the convex hull (adjacent to an infinite face)
            if self.tri.is_infinite_face(face) || self.tri.is_infinite_face(neighbor) {
                log::trace!("Skipping hull edge: ({}, {})", face.into_index(), edge.into_index());
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

            // If vd is strictly inside the circum-circle of CCW triangle (va, vb, vc),
            // the edge is not locally Delaunay → flip.
            // Exact == 0 (co-circular) is left as-is to avoid infinite flip loops.
            let ic = in_circle(pa, pb, pc, pd);
            if ic > 0 {
                log::trace!("Flipping edge: ({}, {})", face.into_index(), edge.into_index());
                let [e1, e2] = self.tri.flip(face, edge);

                BuilderState::delaunay_push_edge_into(&mut stack, &self.tri, e1.next());
                BuilderState::delaunay_push_edge_into(&mut stack, &self.tri, e1.prev());
                BuilderState::delaunay_push_edge_into(&mut stack, &self.tri, e2.next());
                BuilderState::delaunay_push_edge_into(&mut stack, &self.tri, e2.prev());
                stack.retain(|&e| e != e1 && e != e2);
            } else {
                log::trace!("Skipping delaunay edge: ({}, {})", face.into_index(), edge.into_index());
            }

            self.state.dump(
                4,
                &format!("delaunay_check_{}_{}", face.into_index(), u8::from(edge)),
                |dump| {
                    dump.add_tri(&self.tri, [(stack.as_slice(), "edge-delaunay", false)]);
                },
            );
        }

        // Unlock the stack
        self.state.unlock_delaunay_stack(stack);
        self.state.dump(1, "after_delaunay", |dump| {
            dump.add_tri(&self.tri, []);
        });
    }
}
