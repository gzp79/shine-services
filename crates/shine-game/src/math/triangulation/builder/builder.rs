use std::path::PathBuf;

use crate::{
    indexed::TypedIndex,
    math::{
        debug::SvgDumpFile,
        triangulation::{
            predicates::{test_collinear_points, CollinearTestType},
            Crossing, FaceEdge, FaceIndex, GeometryChecker, Location, Rot3Idx, TopologyChecker, Triangulation,
            VertexClue, VertexIndex,
        },
    },
};
use glam::IVec2;

pub struct TriangulationBuilder<'a, const DELAUNAY: bool> {
    pub(super) tri: &'a mut Triangulation<DELAUNAY>,

    // Reusable buffers
    pub(super) delaunay_stack: Option<Vec<FaceEdge>>,
    pub(super) edge_chain: Option<Vec<FaceEdge>>,
    pub(super) top_chain: Option<Vec<FaceEdge>>,
    pub(super) bottom_chain: Option<Vec<FaceEdge>>,

    pub(super) svg_dump: SvgDumpFile,
}

impl<'a, const DELAUNAY: bool> TriangulationBuilder<'a, DELAUNAY> {
    pub fn new(tri: &'a mut Triangulation<DELAUNAY>) -> Self {
        Self {
            tri,
            delaunay_stack: if DELAUNAY { Some(Vec::new()) } else { None },
            edge_chain: Some(Vec::new()),
            top_chain: Some(Vec::new()),
            bottom_chain: Some(Vec::new()),
            svg_dump: SvgDumpFile::new(0, ""),
        }
    }

    pub fn check(&self) -> Result<(), String> {
        TopologyChecker::new(&self.tri).check()?;
        GeometryChecker::new(&self.tri).check()?;
        Ok(())
    }

    pub fn with_debug<P: Into<PathBuf>>(mut self, verbosity: usize, path: P) -> Self {
        self.svg_dump = SvgDumpFile::new(verbosity, path);
        self
    }

    pub fn add_contour(&mut self, p: &[IVec2]) {
        assert!(p.len() >= 2);

        if let Some(mut dump) = self.svg_dump.scope(0, "input_contour") {
            dump.add_default_styles()
                .add_contour(p.iter().cloned(), "edge-constraint");
        }

        let location = self.locate_position(p[0], None).unwrap();
        let vi0 = self.add_vertex_at(p[0], location);

        let mut hint = self.tri[vi0].face;
        let mut vi_prev = vi0;

        for i in 1..p.len() {
            let location = self.locate_position(p[i], Some(hint)).unwrap();
            let vi_next = self.add_vertex_at(p[i], location);
            self.add_constraint_edge(vi_prev, vi_next, 1);

            hint = self.tri[vi_next].face;
            vi_prev = vi_next;
        }

        self.add_constraint_edge(vi_prev, vi0, 1);
        //self.debug_dump(1, "contour_close");
        self.delaunay_run();
    }

    pub fn add_vertex(&mut self, p: IVec2, hint: Option<FaceIndex>) -> VertexIndex {
        let location = self.locate_position(p, hint).unwrap();
        let vi = self.add_vertex_at(p, location);

        self.delaunay_run();
        vi
    }

    pub fn add_constraint_segment(&mut self, p0: IVec2, p1: IVec2, c: u32) -> (VertexIndex, VertexIndex) {
        assert!(c != 0);
        let v0 = self.add_vertex(p0, None);
        let start_face = self.tri[v0].face;
        let v1 = self.add_vertex(p1, Some(start_face));
        self.add_constraint_edge(v0, v1, c);
        (v0, v1)
    }

    pub fn add_constraint_edge(&mut self, v0: VertexIndex, v1: VertexIndex, c: u32) {
        assert!(c != 0);
        assert!(v0.is_valid());
        assert!(v1.is_valid());
        assert!(self.tri.is_finite_vertex(v0));
        assert!(self.tri.is_finite_vertex(v1));
        if v0 == v1 {
            return;
        }

        match self.tri.dimension() {
            1 => self.add_constraint_dim1(v0, v1, c),
            2 => self.add_constraint_dim2(v0, v1, c),
            _ => unreachable!("Inconsistent triangulation"),
        }

        if let Some(mut dump) = self.svg_dump.scope(1, "after_add_constraint") {
            dump.add_default_styles()
                .add_tri(&self.tri, self.delaunay_stack.as_ref().map(|e| (e, "edge-delaunay")));
        }
        self.delaunay_run();
    }

    fn add_vertex_at(&mut self, p: IVec2, loc: Location) -> VertexIndex {
        let vi = match loc {
            Location::Empty => {
                let vi = self.create_vertex_with_position(p);
                self.extend_dimension(vi);
                vi
            }
            Location::Vertex(f, v) => self.tri[f].vertices[v],
            Location::Edge(f, e) => {
                let vi = self.create_vertex_with_position(p);
                self.split_edge(f, e, vi);
                self.delaunay_push_vertex(vi);
                vi
            }
            Location::OutsideConvexHull(f) | Location::Face(f) => {
                let vi = self.create_vertex_with_position(p);
                self.split_face(f, vi);
                self.delaunay_push_vertex(vi);
                vi
            }
            Location::OutsideAffineHull => {
                let vi = self.create_vertex_with_position(p);
                self.extend_dimension(vi);
                self.delaunay_push_vertex(vi);
                vi
            }
        };

        if let Some(mut dump) = self.svg_dump.scope(1, "after_add_vertex") {
            dump.add_default_styles()
                .add_tri(&self.tri, self.delaunay_stack.as_ref().map(|e| (e, "edge-delaunay")));
        }
        vi
    }

    /// Adds the constraining edge between the two vertex when dim=1 (all faces are segments)
    fn add_constraint_dim1(&mut self, v0: VertexIndex, v1: VertexIndex, c: u32) {
        assert!(self.tri.is_finite_vertex(v0));
        assert!(self.tri.is_finite_vertex(v1));
        assert_ne!(v1, v0);

        // start by the face of the first vertex
        let f0 = self.tri[v0].face;
        let i0 = self.tri[f0].find_vertex(v0).unwrap();

        // next vertex
        let vn = self.tri[f0].vertices[i0.mirror(2)];
        if vn == v1 {
            // v0-v1 edge was just found
            self.tri[f0].constraints[Rot3Idx::new(2)] |= c;
            return;
        }

        // find the direction to reach v1 from v0
        let reverse_dir = if self.tri.is_finite_vertex(vn) {
            // test direction to traverse by point order
            let p0 = self.tri[v0].position;
            let p1 = self.tri[v1].position;
            let pn = self.tri[vn].position;

            // p0,p1,pn and any other (finite) point must be collinear as dim==1,
            let direction = test_collinear_points(p0, p1, pn);
            assert!(
                direction == CollinearTestType::Before || direction == CollinearTestType::Between,
                "Internal error, direction test result"
            );
            direction == CollinearTestType::Before
        } else {
            true
        };

        let (mut cur, mut cur_i) = if reverse_dir {
            // opposite direction
            let next = self.tri[f0].neighbors[i0.mirror(2)];
            let next_i = self.tri[next].find_neighbor(f0).unwrap().mirror(2);
            (next, next_i)
        } else {
            (f0, i0)
        };

        // mark all edges constraint until the end vertex is reached
        // no geometry have to be tested, as we are on a straight line and no segment may overlap
        loop {
            self.tri[cur].constraints[Rot3Idx::new(2)] |= c;
            if self.tri[cur].vertices[cur_i.mirror(2)] == v1 {
                break;
            }

            let next = self.tri[cur].neighbors[cur_i];
            cur_i = self.tri[next].find_neighbor(cur).unwrap().mirror(2);
            cur = next;
        }

        //self.debug_dump(2, "dim1_constraint");
    }

    /// Adds the constraining edge between the two vertex when dim=2
    fn add_constraint_dim2(&mut self, mut v0: VertexIndex, v1: VertexIndex, c: u32) {
        self.delaunay_push_vertex(v0);
        self.delaunay_push_vertex(v1);

        let mut edge_chain = self.edge_chain.take().expect("edge_chain lock");
        let mut top_chain = self.top_chain.take().expect("top_chain lock");
        let mut bottom_chain = self.bottom_chain.take().expect("bottom_chain lock");

        edge_chain.clear();
        top_chain.clear();
        bottom_chain.clear();

        while v0 != v1 {
            // collect intersecting faces and generate the two (top/bottom) chains
            // The edge-chain is not a whole polygon the new constraining edge is the missing closing edge

            let mut crossing_iter = self.tri.crossing_iterator(v0, v1);
            let mut cross = crossing_iter.next();

            // loop over coincident edges
            while let Some(Crossing::CoincidentEdge { face, edge }) = cross {
                edge_chain.push(FaceEdge { face, edge });
                cross = crossing_iter.next();
            }

            if let Some(Crossing::Start { face, vertex }) = cross {
                top_chain.push(FaceEdge { face, edge: vertex.increment() });
                bottom_chain.push(FaceEdge { face, edge: vertex.decrement() });
                loop {
                    cross = crossing_iter.next();
                    match cross {
                        Some(Crossing::PositiveEdge { face, edge }) => {
                            bottom_chain.push(FaceEdge { face, edge: edge.decrement() });
                        }
                        Some(Crossing::NegativeEdge { face, edge }) => {
                            top_chain.push(FaceEdge { face, edge: edge.increment() });
                        }
                        Some(Crossing::End { face, vertex }) => {
                            top_chain.push(FaceEdge { face, edge: vertex.decrement() });
                            bottom_chain.push(FaceEdge { face, edge: vertex.increment() });
                            break;
                        }
                        _ => unreachable!(),
                    }
                }
            }

            if let Some(mut dump) = self.svg_dump.scope(2, "before_triangulate_hole") {
                dump.add_default_styles().add_tri(
                    &self.tri,
                    [
                        (&top_chain, "edge-top"),
                        (&bottom_chain, "edge-bottom"),
                        (&edge_chain, "edge-chain"),
                    ],
                );
            }

            if !edge_chain.is_empty() {
                v0 = self.tri.vi(VertexClue::end_of(*edge_chain.last().unwrap()));
                for edge in edge_chain.iter() {
                    self.merge_constraint(*edge, c);
                }
            }

            if !top_chain.is_empty() {
                v0 = self.tri.vi(VertexClue::end_of(*bottom_chain.last().unwrap()));
                top_chain.reverse();
                let edge = self.triangulate_hole(&mut top_chain, &mut bottom_chain);
                self.merge_constraint(edge, c);
            }

            for fe in top_chain.iter() {
                self.delaunay_push_face(fe.face);
            }
            for fe in bottom_chain.iter() {
                self.delaunay_push_face(fe.face);
            }
            self.delaunay_push_vertex(v0);
            if let Some(mut dump) = self.svg_dump.scope(2, "after_triangulate_hole") {
                dump.add_default_styles().add_tri(
                    &self.tri,
                    [
                        (&top_chain, "edge-top"),
                        (&bottom_chain, "edge-bottom"),
                        (&edge_chain, "edge-chain"),
                    ],
                );
            }

            edge_chain.clear();
            top_chain.clear();
            bottom_chain.clear();
        }

        self.delaunay_push_vertex(v1);
        //self.debug_dump(2, "dim2_hole_constraint");

        self.edge_chain = Some(edge_chain);
        self.top_chain = Some(top_chain);
        self.bottom_chain = Some(bottom_chain);
    }

    fn triangulate_half_hole(&mut self, chain: &mut Vec<FaceEdge>) -> FaceEdge {
        assert!(chain.len() >= 2);
        let mut cur = 0;
        while chain.len() > 2 {
            let next = cur + 1;
            let cur_edge = chain[cur];
            let next_edge = chain[next];

            let p0 = self.tri.vi(VertexClue::start_of(cur_edge));
            let p1 = self.tri.vi(VertexClue::end_of(cur_edge));
            assert_eq!(
                p1,
                self.tri.vi(VertexClue::start_of(next_edge)),
                "Edges are not continouous"
            );
            let p2 = self.tri.vi(VertexClue::end_of(next_edge));

            if self.get_vertices_orientation(p0, p1, p2) <= 0 {
                // cannot clip, not an ear
                cur += 1;
                continue;
            }

            // found an ear, clip it
            // Remove the edge only if it is not part of the first or last crossed triangle.
            // These edges are shared by both the upper and lower polygon parts and handled outside

            if next + 1 < chain.len() {
                // remove next from the list and make it the clipped ear
                chain.remove(next);

                self.tri[cur_edge.face].vertices[cur_edge.edge.decrement()] = p2;
                self.tri[next_edge.face].vertices[next_edge.edge] = p0;

                let ne = self.tri.twin_edge(cur_edge);
                self.set_adjacent((ne.face, ne.edge), (next_edge.face, next_edge.edge.decrement()));
                self.set_adjacent(
                    (cur_edge.face, cur_edge.edge),
                    (next_edge.face, next_edge.edge.increment()),
                );
                self.tri[p0].face = next_edge.face;
                self.tri[p1].face = next_edge.face;
                self.tri[p2].face = next_edge.face;

                let c = self.tri[cur_edge.face].constraints[cur_edge.edge];
                self.tri[next_edge.face].constraints[next_edge.edge.decrement()] = c;
                self.tri[cur_edge.face].constraints[cur_edge.edge] = 0;
                self.tri[next_edge.face].constraints[next_edge.edge.increment()] = 0;

                if cur > 0 {
                    // step back
                    cur -= 1;
                }
            } else {
                // remove cur from the list and make it the clipped ear
                assert!(cur > 0);
                chain.remove(cur);

                self.tri[cur_edge.face].vertices[cur_edge.edge] = p2;
                self.tri[next_edge.face].vertices[next_edge.edge.increment()] = p0;

                let ne = self.tri.twin_edge(next_edge);
                self.set_adjacent((ne.face, ne.edge), (cur_edge.face, cur_edge.edge.increment()));
                self.set_adjacent(
                    (next_edge.face, next_edge.edge),
                    (cur_edge.face, cur_edge.edge.decrement()),
                );
                self.tri[p0].face = cur_edge.face;
                self.tri[p1].face = cur_edge.face;
                self.tri[p2].face = cur_edge.face;

                let c = self.tri[next_edge.face].constraints[next_edge.edge];
                self.tri[cur_edge.face].constraints[cur_edge.edge.increment()] = c;
                self.tri[cur_edge.face].constraints[cur_edge.edge.decrement()] = 0;
                self.tri[next_edge.face].constraints[next_edge.edge] = 0;

                // step back
                cur -= 1;
            }
        }

        chain.pop().unwrap()
    }

    /// Triangulates an edge-visible hole given by the edge chain of the upper(lower) polygon.
    /// On completion it returns the edge that separates the upper and lower half of the polygon.
    fn triangulate_hole(&mut self, top: &mut Vec<FaceEdge>, bottom: &mut Vec<FaceEdge>) -> FaceEdge {
        assert!(top.len() >= 2 && bottom.len() >= 2);
        let top = self.triangulate_half_hole(top);
        let bottom = self.triangulate_half_hole(bottom);
        let top = FaceEdge::new(top.face, top.edge.decrement());
        let bottom = FaceEdge::new(bottom.face, bottom.edge.decrement());
        self.set_adjacent(top, bottom);
        self.flip(top.face, top.edge);

        FaceEdge::new(top.face, top.edge.increment())
    }
}
