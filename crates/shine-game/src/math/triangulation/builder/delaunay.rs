use super::TriangulationBuilder;
use crate::{
    indexed::TypedIndex,
    math::triangulation::{predicates::in_circle, FaceEdge, FaceIndex, Rot3Idx, VertexIndex},
};

impl<'a, const DELAUNAY: bool> TriangulationBuilder<'a, DELAUNAY> {
    pub fn delaunay_push_vertex(&mut self, vertex: VertexIndex) {
        if !DELAUNAY || self.tri.dimension() != 2 {
            return;
        }

        let start_face = self.tri[vertex].face;
        let start_vi = self.tri[start_face].find_vertex(vertex).unwrap();
        self.delaunay_push_edge(FaceEdge::new(start_face, start_vi));

        let mut cur_face = start_face;
        let mut cur_vi = start_vi;
        loop {
            // Move CCW: cross the edge at cur_vi.decrement() to the next face
            let next_face = self.tri[cur_face].neighbors[cur_vi.decrement()];
            if next_face == start_face {
                break;
            }
            let next_vi = self.tri[next_face].find_vertex(vertex).unwrap();
            self.delaunay_push_edge(FaceEdge::new(next_face, next_vi));
            cur_face = next_face;
            cur_vi = next_vi;
        }
    }

    pub fn delaunay_push_edge(&mut self, edge: FaceEdge) {
        if !DELAUNAY || self.tri.dimension() != 2 {
            return;
        }

        let stack = self.delaunay_stack.as_mut().expect("delaunay_stack lock");
        let twin = self.tri.twin_edge(edge);

        if !stack.contains(&edge) && !stack.contains(&twin) {
            stack.push(edge);
        }
    }

    pub fn delaunay_push_face(&mut self, face: FaceIndex) {
        if !DELAUNAY || self.tri.dimension() != 2 || !face.is_valid() || self.tri.is_infinite_face(face) {
            return;
        }

        for i in 0..3 {
            let edge = Rot3Idx::new(i);
            let fe = FaceEdge::new(face, edge);
            self.delaunay_push_edge(fe);
        }
    }

    // Lawson flip: process edges until the stack is empty.
    pub fn delaunay_run(&mut self) {
        if !DELAUNAY || self.tri.dimension() != 2 {
            return;
        }

        let mut stack = self.delaunay_stack.take().expect("delaunay_stack lock");

        while let Some(FaceEdge { face, edge }) = stack.pop() {
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
            let ic = in_circle(pa, pb, pc, pd);
            if ic > 0 {
                self.flip(face, edge);

                // After flip_face(face=f0, edge=i00):
                //   f0 vertices: [va, vb, vd] at [i00, i01, i02]
                //   f1 vertices: [vd, vc, va] at [ni, ni+1, ni-1]
                //
                // The four newly exposed external edges that might now be non-Delaunay:
                //   - (f0, i01): connects vb to vd
                //   - (f1, ni-1): connects vc to vd
                //   - (f0, i00): connects va to vb (newly oriented)
                //   - (f1, ni): connects va to vc (newly oriented)
                let e1 = FaceEdge::new(face, edge.increment());
                let e2 = FaceEdge::new(neighbor, ni.decrement());
                let e3 = FaceEdge::new(face, edge);
                let e4 = FaceEdge::new(neighbor, ni);

                let edges = [e1, e2, e3, e4];
                for e in edges {
                    let n = self.tri[e.face].neighbors[e.edge];
                    let twin = self.tri[n].find_neighbor(e.face).map(|ni| FaceEdge::new(n, ni));

                    if !stack.contains(&e) && twin.map_or(true, |t| !stack.contains(&t)) {
                        stack.push(e);
                    }
                }

                /*if self.verbosity > 0 {
                    self.delaunay_stack = Some(stack);
                    //self.debug_dump(2, "delaunay_flip");
                    stack = self.delaunay_stack.take().expect("delaunay_stack lock");
                }*/
            }
        }

        self.delaunay_stack = Some(stack);
        if let Some(mut dump) = self.svg_dump.scope(1, "after_delaunay") {
            dump.add_default_styles().add_tri(&self.tri, []);
        }
    }
}
