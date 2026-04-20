use glam::IVec2;

use crate::{
    indexed::TypedIndex,
    math::triangulation::{
        predicates::orient2d, FaceEdge, FaceIndex, Rot3Idx, Triangle, Triangulation, Vertex, VertexClue, VertexIndex,
    },
};

/// Low-level primitive operations.
impl<const DELAUNAY: bool> Triangulation<DELAUNAY> {
    fn store_vertex(&mut self, vert: Vertex) -> VertexIndex {
        let id = self.vertices.len();
        self.vertices.push(vert);
        VertexIndex::new(id)
    }

    fn store_triangle(&mut self, triangle: Triangle) -> FaceIndex {
        let id = self.faces.len();
        self.faces.push(triangle);
        FaceIndex::new(id)
    }

    pub(in crate::math::triangulation) fn set_adjacent<A: Into<FaceEdge>, B: Into<FaceEdge>>(&mut self, a: A, b: B) {
        let FaceEdge { triangle: f0, edge: i0 } = a.into();
        let FaceEdge { triangle: f1, edge: i1 } = b.into();
        assert!(i0.is_valid() && i1.is_valid());
        assert!(u8::from(i0) <= self.dimension() && u8::from(i1) <= self.dimension());
        self[f0].neighbors[i0] = f1;
        self[f1].neighbors[i1] = f0;
    }

    fn clear_constraint<E: Into<FaceEdge>>(&mut self, edge: E) {
        let edge: FaceEdge = edge.into();
        let nf = self[edge.triangle].neighbors[edge.edge];
        let ni = self[nf].find_neighbor(edge.triangle).unwrap();
        self[edge.triangle].constraints[edge.edge] = 0;
        self[nf].constraints[ni] = 0;
    }

    fn copy_constraint_partial(&mut self, f_from: FaceIndex, i_from: Rot3Idx, f_to: FaceIndex, i_to: Rot3Idx) {
        let c = self[f_from].constraints[i_from];
        self[f_to].constraints[i_to] = c;
    }

    fn move_adjacent<T: Into<FaceEdge>, S: Into<FaceEdge>>(&mut self, target: T, source: S) {
        let FaceEdge { triangle: face, edge } = source.into();
        let n = self[face].neighbors[edge];
        let i = self[n].find_neighbor(face).unwrap();
        self.set_adjacent(target, FaceEdge::new(n, i));
    }
}

/// Euler operators for topological mutations.
///
/// These are the fundamental operations for modifying triangulation topology:
/// splitting edges and faces, flipping edges, and extending dimensionality.
impl<const DELAUNAY: bool> Triangulation<DELAUNAY> {
    /// Split an edge by inserting a vertex.
    pub fn split_edge(&mut self, triangle: FaceIndex, edge: Rot3Idx, p: IVec2) -> VertexIndex {
        let vi = self.store_vertex(Vertex::with_position(p));

        match self.dimension() {
            1 => self.split_edge_dim1(triangle, edge, vi),
            2 => self.split_edge_dim2(triangle, edge, vi),
            _ => panic!("invalid dimension for edge split: {}", self.dimension()),
        };

        vi
    }

    /// Split a face by inserting a vertex.
    pub fn split_face(&mut self, triangle: FaceIndex, p: IVec2) -> VertexIndex {
        let vi = self.store_vertex(Vertex::with_position(p));
        match self.dimension() {
            1 => self.split_face_dim1(triangle, vi),
            2 => self.split_face_dim2(triangle, vi),
            _ => panic!("invalid dimension for face split: {}", self.dimension()),
        };
        vi
    }

    /// Extend triangulation dimensionality by adding a vertex.
    pub fn extend_dimension(&mut self, p: IVec2) -> VertexIndex {
        let vi = self.store_vertex(Vertex::with_position(p));

        match self.dimension() {
            u8::MAX => self.extend_to_dim0(vi),
            0 => self.extend_to_dim1(vi),
            1 => self.extend_to_dim2(vi),
            _ => panic!("invalid dimension for face split: {}", self.dimension()),
        };

        vi
    }

    /// Flip an edge and return the shared edges of the two new faces.
    pub fn flip(&mut self, triangle: FaceIndex, edge: Rot3Idx) -> [FaceEdge; 2] {
        assert_eq!(self.dimension(), 2);
        assert!(triangle.is_valid() && edge.is_valid());
        self.flip_face(triangle, edge)
    }

    fn split_edge_dim1(&mut self, f: FaceIndex, edge: Rot3Idx, vert: VertexIndex) {
        assert!(self.dimension() == 1);
        assert!(edge == Rot3Idx::new(2));
        self.split_face_dim1(f, vert);
    }

    fn split_edge_dim2(&mut self, triangle: FaceIndex, edge: Rot3Idx, vert: VertexIndex) {
        assert_eq!(self.dimension(), 2);

        //           v0  i02 = edge
        //         /  |2 \
        //       / F0 | N0 \
        // i00 /      |0    1\ i01
        //   v1 ------vp------ v2
        // i11 \      |0    2/ i10
        //       \ F1 | N1 /
        //         \  |1 /
        //           v3  i12

        let vp = vert;
        let n0 = self.store_triangle(Triangle::new());
        let n1 = self.store_triangle(Triangle::new());
        let f0 = triangle;
        let f1 = self[f0].neighbors[edge];
        let i00 = edge.increment();
        let i01 = edge.decrement();
        let i02 = edge;
        let i12 = self[f1].find_neighbor(f0).unwrap();
        let i11 = i12.decrement();
        let i10 = i12.increment();

        let v0 = self[f0].vertices[i02];
        //let v1 = self[f0].vertex(i00);
        let v2 = self[f0].vertices[i01];
        let v3 = self[f1].vertices[i12];

        self[n0].vertices = [vp, v2, v0].into();
        self[n1].vertices = [vp, v3, v2].into();
        self[f0].vertices[i01] = vp;
        self[f1].vertices[i10] = vp;
        self[vp].triangle = n0;
        self[v2].triangle = n0;
        self[v0].triangle = n0;
        self[v3].triangle = n1;

        self.move_adjacent((n0, Rot3Idx::new(0)), (f0, i00));
        self.set_adjacent((n0, Rot3Idx::new(1)), (f0, i00));
        self.set_adjacent((n0, Rot3Idx::new(2)), (n1, Rot3Idx::new(1)));

        self.move_adjacent((n1, Rot3Idx::new(0)), (f1, i11));
        self.set_adjacent((n1, Rot3Idx::new(2)), (f1, i11));

        self.copy_constraint_partial(f0, i00, n0, Rot3Idx::new(0));
        self.copy_constraint_partial(f0, i02, n0, Rot3Idx::new(2));
        self[f0].constraints[i00] = 0;

        self.copy_constraint_partial(f1, i11, n1, Rot3Idx::new(0));
        self.copy_constraint_partial(f1, i12, n1, Rot3Idx::new(1));
        self[f1].constraints[i11] = 0;
    }

    fn split_face_dim1(&mut self, f: FaceIndex, vert: VertexIndex) {
        assert!(self.dimension() == 1);

        // f0 : the face to split
        // f2 : new face
        // v2 : new vertex
        //
        //     v0             v1
        // ----*0-----f0-----1*j--f1---i*---
        //
        //     v0       v2      v1
        // ----*0--f0--1*0-f2--1*j--f1---i*---

        let v2 = vert;
        let f2 = self.store_triangle(Triangle::new()); // new face

        let f0 = f;
        let f1 = self[f0].neighbors[Rot3Idx::new(0)];
        let i = self[f1].find_neighbor(f0).unwrap();
        let v1 = self[f1].vertices[i.mirror(2)]; // j = 1-i

        self[v1].triangle = f1;
        self[v2].triangle = f2;
        self[f0].vertices[Rot3Idx::new(1)] = v2;
        self[f2].vertices = [v2, v1, VertexIndex::NONE].into();
        self.set_adjacent((f2, Rot3Idx::new(1)), (f0, Rot3Idx::new(0)));
        self.set_adjacent((f2, Rot3Idx::new(0)), (f1, i));

        self.copy_constraint_partial(f0, Rot3Idx::new(2), f2, Rot3Idx::new(2));
    }

    fn split_finite_face_dim2(&mut self, triangle: FaceIndex, vert: VertexIndex) {
        assert_eq!(self.dimension(), 2);

        //            v2
        //            x
        //         / 2|2 \
        //        /   |   \
        //       /   vp    \
        //      /N0 1/x\0 N1\
        //     /0  /  2  \  1\
        //      /0   F0    1\
        // v0  x-------------x v1

        let vp = vert;
        let n0 = self.store_triangle(Triangle::new());
        let n1 = self.store_triangle(Triangle::new());
        let f0 = triangle;

        let v0 = self[f0].vertices[Rot3Idx::new(0)];
        let v1 = self[f0].vertices[Rot3Idx::new(1)];
        let v2 = self[f0].vertices[Rot3Idx::new(2)];

        self[n0].vertices = [v0, vp, v2].into();
        self[n1].vertices = [vp, v1, v2].into();
        self[f0].vertices[Rot3Idx::new(2)] = vp;
        self[vp].triangle = f0;
        self[v2].triangle = n0;

        self.set_adjacent((n0, Rot3Idx::new(0)), (n1, Rot3Idx::new(1)));
        self.move_adjacent((n0, Rot3Idx::new(1)), (f0, Rot3Idx::new(1)));
        self.move_adjacent((n1, Rot3Idx::new(0)), (f0, Rot3Idx::new(0)));
        self.set_adjacent((n0, Rot3Idx::new(2)), (f0, Rot3Idx::new(1)));
        self.set_adjacent((n1, Rot3Idx::new(2)), (f0, Rot3Idx::new(0)));

        self.copy_constraint_partial(f0, Rot3Idx::new(1), n0, Rot3Idx::new(1));
        self.copy_constraint_partial(f0, Rot3Idx::new(0), n1, Rot3Idx::new(0));
        self[f0].constraints[Rot3Idx::new(0)] = 0;
        self[f0].constraints[Rot3Idx::new(1)] = 0;
    }

    fn split_face_dim2(&mut self, triangle: FaceIndex, vert: VertexIndex) {
        let f0 = triangle;
        let v_inf = self.infinite_vertex();

        // extract info of the infinite faces to handle the case when the convex hull is extended
        let infinite_info = self[f0].find_vertex(v_inf).map(|i| {
            let f_cw = self[f0].neighbors[i.decrement()];
            let f_ccw = self[f0].neighbors[i.increment()];
            (f_cw, f_ccw)
        });

        // perform a normal split
        self.split_finite_face_dim2(triangle, vert);

        if let Some((mut f_cw, mut f_ccw)) = infinite_info {
            //correct faces by flipping
            loop {
                let i = self[f_cw].find_vertex(v_inf).unwrap();
                let next = self[f_cw].neighbors[i.decrement()];
                if self.get_edge_vertex_orientation(f_cw, i, vert) <= 0 {
                    break;
                }
                self.flip(f_cw, i.increment());
                f_cw = next;
            }

            loop {
                let i = self[f_ccw].find_vertex(v_inf).unwrap();
                let next = self[f_ccw].neighbors[i.increment()];
                if self.get_edge_vertex_orientation(f_ccw, i, vert) <= 0 {
                    break;
                }
                self.flip(f_ccw, i.decrement());
                f_ccw = next;
            }
        }
    }

    /// Extends dimension from none to 0D by creating the infinite vertices.
    fn extend_to_dim0(&mut self, vert: VertexIndex) {
        assert!(self.dimension() == u8::MAX);
        assert!(!self.infinite_vertex().is_valid());
        assert!(self.vertex_count() == 1); // includes the new vertex
        assert!(self.face_count() == 0);

        self.dimension = 0;
        let v0 = self.store_vertex(Vertex::new());
        self.infinite_vertex = v0;
        let v1 = vert;
        let f0 = self.store_triangle(Triangle::with_vertices(v0, VertexIndex::NONE, VertexIndex::NONE));
        let f1 = self.store_triangle(Triangle::with_vertices(v1, VertexIndex::NONE, VertexIndex::NONE));

        self[v0].triangle = f0;
        self[v1].triangle = f1;
        self.set_adjacent((f0, Rot3Idx::new(0)), (f1, Rot3Idx::new(0)));
    }

    /// Extends dimension from 0D to 1D by creating a segment (face) out of the (two) finite points.
    /// In 1D a face is a segment, and the shell is the triangular face (as described in extend_to_dim2). The
    /// infinite vertex is always the vertex corresponding to the 2nd index in each (finite) faces(segments).
    fn extend_to_dim1(&mut self, vert: VertexIndex) {
        assert!(self.dimension() == 0);
        assert!(self.vertex_count() == 3); // includes the new vertex
        assert!(self.face_count() == 2);

        self.dimension = 1;

        // infinite, finite vertices
        let (v0, v1) = {
            let v0 = VertexIndex::new(0);
            let v1 = VertexIndex::new(1);
            if self.is_infinite_vertex(v0) {
                (v0, v1)
            } else {
                (v1, v0)
            }
        };
        // finite (new) vertex
        let v2 = vert;

        let f0 = self[v0].triangle;
        let f1 = self[v1].triangle;
        let f2 = self.store_triangle(Triangle::with_vertices(v2, v0, VertexIndex::NONE));

        self[f0].vertices[Rot3Idx::new(1)] = v1;
        self[f1].vertices[Rot3Idx::new(1)] = v2;
        self[v2].triangle = f2;

        self.set_adjacent((f0, Rot3Idx::new(0)), (f1, Rot3Idx::new(1)));
        self.set_adjacent((f1, Rot3Idx::new(0)), (f2, Rot3Idx::new(1)));
        self.set_adjacent((f2, Rot3Idx::new(0)), (f0, Rot3Idx::new(1)));
    }

    /// Extends dimension from 1D to 2D by creating triangles (2d fac0es) out of the segments (1D faces).
    /// The infinite vertex and triangulation can be seen as an n+1 dimensional shell. The
    /// edges of the convex hull of an nD object is connected to the infinite vertex, which can be seen as
    /// a normal point in (n+1)D which is "above" the nD points.
    /// For 1D -> 2D lifting we have to extended each segment into a triangle that creates a shell in 3D space.
    /// After transforming each segment int a triangle, we have to add the cap in 3D by generating the infinite faces.
    fn extend_to_dim2(&mut self, vert: VertexIndex) {
        assert_eq!(self.dimension(), 1);

        self.dimension = 2;

        // face neighborhood:
        // It is assumed that all the segments are directed in the same direction:
        // the series of the vertex indices (Index3) is a (closed) chain of ..010101..
        //
        // F0: starting (infinite) face
        // Fm: ending (infinite) face
        // Cj: original (finite) faces extended to 2D, j in [0..n]
        // Nj: new, generated faces in 2D, j in [0..n]
        // n: number of finite faces - 1
        // i: the Index3 of the next neighbor, either 010101.. or 101010.... sequence (See the note above)

        // input constraint:
        //   F0[  i] = Fm[1-i]
        //   F0[1-i] = C1[i]
        //   Fm[  i] = Cm[1-i]
        //   Fm[1-i] = F0[i]
        //   Cj-1[1-i] = Cj[i]

        // F0, start by an infinite face for which the convex hull (segment) and p is in counter-clockwise direction
        // Fm is the other infinite face
        let (f0, i0, fm, im) = {
            let f0 = self.infinite_face();
            let i0 = self[f0].find_vertex(self.infinite_vertex()).unwrap();
            let im = i0.mirror(2);
            let fm = self[f0].neighbors[im];

            let orient = {
                let cp0 = self.p(VertexClue::face_vertex(f0, im));
                let cp1 = self.p(VertexClue::face_vertex(fm, i0));
                let p = self.p(vert);
                orient2d(cp0, cp1, p)
            };
            assert!(orient != 0);

            if orient > 0 {
                (f0, i0, fm, im)
            } else {
                (fm, im, f0, i0)
            }
        };

        let c0 = self[f0].neighbors[i0];

        let mut cur = c0;
        let mut new_face = FaceIndex::NONE;
        while cur != fm {
            let prev_new_face = new_face;
            new_face = self.store_triangle(Triangle::new());

            let v0 = self[cur].vertices[Rot3Idx::new(1)];
            let v1 = self[cur].vertices[Rot3Idx::new(0)];
            let v_inf = self.infinite_vertex();
            if i0 == Rot3Idx::new(1) {
                self[new_face].vertices = [v0, v1, vert].into();
                self[cur].vertices[Rot3Idx::new(2)] = v_inf;
                self[vert].triangle = new_face;
            } else {
                self[new_face].vertices = [v0, v1, v_inf].into();
                self[cur].vertices[Rot3Idx::new(2)] = vert;
                self[vert].triangle = cur;
            }

            self.set_adjacent((cur, Rot3Idx::new(2)), (new_face, Rot3Idx::new(2)));
            if prev_new_face.is_valid() {
                self.set_adjacent((prev_new_face, im), (new_face, i0));
            }

            self.copy_constraint_partial(cur, Rot3Idx::new(2), new_face, Rot3Idx::new(2));

            cur = self[cur].neighbors[i0];
        }

        let cm = self[fm].neighbors[im];
        let n0 = self[c0].neighbors[Rot3Idx::new(2)];
        let nm = self[cm].neighbors[Rot3Idx::new(2)];

        self[f0].vertices[Rot3Idx::new(2)] = vert;
        self[fm].vertices[Rot3Idx::new(2)] = vert;

        if i0 == Rot3Idx::new(1) {
            self[f0].vertices.swap(Rot3Idx::new(2), Rot3Idx::new(1));
            self[f0].neighbors.swap(Rot3Idx::new(2), Rot3Idx::new(1));
            self[f0].constraints.swap(Rot3Idx::new(2), Rot3Idx::new(1));
            self[fm].vertices.swap(Rot3Idx::new(0), Rot3Idx::new(2));
            self[fm].neighbors.swap(Rot3Idx::new(0), Rot3Idx::new(2));
            self[fm].constraints.swap(Rot3Idx::new(0), Rot3Idx::new(2));
            self.set_adjacent((f0, Rot3Idx::new(1)), (c0, Rot3Idx::new(0)));
            self.set_adjacent((fm, Rot3Idx::new(0)), (cm, Rot3Idx::new(1)));
            self.set_adjacent((f0, Rot3Idx::new(2)), (n0, Rot3Idx::new(1)));
            self.set_adjacent((fm, Rot3Idx::new(2)), (nm, Rot3Idx::new(0)));
        } else {
            self.set_adjacent((f0, Rot3Idx::new(2)), (n0, Rot3Idx::new(0)));
            self.set_adjacent((fm, Rot3Idx::new(2)), (nm, Rot3Idx::new(1)));
        }
    }

    fn flip_face(&mut self, triangle: FaceIndex, edge: Rot3Idx) -> [FaceEdge; 2] {
        assert_eq!(self.dimension(), 2);
        assert!(triangle.is_valid() && edge.is_valid());

        //            v3                       v3
        //          2 * 1                      *
        //         /  |   \                 /  1  \
        //       /    |     \             /2  F1   0\
        //  v0 *0  F0 | F1  0* v2    v0 * ----------- * v2
        //       \    |    /              \0  F0   2/
        //         \  |  /                  \  1  /
        //          1 * 2                      *
        //            v1                      v1

        let f0 = triangle;
        let i00 = edge;
        let i01 = i00.increment();
        let i02 = i00.decrement();

        let f1 = self[f0].neighbors[i00];
        let i10 = self[f1].find_neighbor(f0).unwrap();
        let i11 = i10.increment();
        let i12 = i10.decrement();

        let v0 = self[f0].vertices[i00];
        let v1 = self[f0].vertices[i01];
        let v3 = self[f0].vertices[i02];
        let v2 = self[f1].vertices[i10];
        assert!(self[f1].vertices[i11] == v3);
        assert!(self[f1].vertices[i12] == v1);

        self[f0].vertices[i02] = v2;
        self[f1].vertices[i12] = v0;
        self[v0].triangle = f0;
        self[v1].triangle = f0;
        self[v2].triangle = f0;
        self[v3].triangle = f1;

        self.move_adjacent((f0, i00), (f1, i11));
        self.move_adjacent((f1, i10), (f0, i01));
        self.set_adjacent((f0, i01), (f1, i11));

        self.copy_constraint_partial(f1, i11, f0, i00);
        self.copy_constraint_partial(f0, i01, f1, i10);
        self.clear_constraint((f0, i01));
        self.clear_constraint((f1, i11));

        [FaceEdge::new(f0, i01), FaceEdge::new(f1, i11)]
    }
}
