use std::path::PathBuf;

use crate::{
    indexed::TypedIndex,
    math::triangulation::{
        builder::state::BuilderState,
        predicates::{test_collinear_points, CollinearTestType},
        Crossing, FaceEdge, FaceIndex, GeometryChecker, Location, Rot3Idx, TopologyChecker, Triangulation, VertexClue,
        VertexIndex,
    },
};
use glam::IVec2;

/// High-level builder for constructing triangulations.
///
/// TriangulationBuilder coordinates between the triangulation data structure
/// and the builder state, providing a clean public API for construction operations.
pub struct TriangulationBuilder<'a, const DELAUNAY: bool> {
    pub(super) tri: &'a mut Triangulation<DELAUNAY>,
    pub(super) state: BuilderState,
}

impl<'a, const DELAUNAY: bool> TriangulationBuilder<'a, DELAUNAY> {
    pub fn new(tri: &'a mut Triangulation<DELAUNAY>) -> Self {
        Self {
            tri,
            state: BuilderState::new(),
        }
    }

    pub fn check(&self) -> Result<(), String> {
        TopologyChecker::new(&self.tri).check()?;
        GeometryChecker::new(&self.tri).check()?;
        Ok(())
    }

    pub fn with_debug<P: Into<PathBuf>>(mut self, verbosity: usize, path: P) -> Self {
        self.state = self.state.with_debug(verbosity, path);
        self
    }

    pub fn add_vertex(&mut self, p: IVec2, hint: Option<FaceIndex>) -> VertexIndex {
        let location = self.tri.locate_position(p, hint).unwrap();
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
        self.add_constraint_between(v0, v1, c);
        self.delaunay_run();
    }

    pub fn add_points<I: IntoIterator<Item = IVec2>>(&mut self, points: I) {
        let mut hint = None;
        for p in points {
            let vi = self.add_vertex(p, hint);
            hint = Some(self.tri[vi].face);
        }
    }

    pub fn add_polygon<I: IntoIterator<Item = IVec2>>(&mut self, points: I, c: u32) {
        // Note: Insertion happens incrementally and delaunay property is maintained at each step.
        // It is required as delaunay edge stack assumes a proper delaunay preoprty before each vertex,edge insertion.

        let mut hint = None;
        let mut vi0 = None;
        let mut vi_prev = None;
        for p in points {
            let vi = self.add_vertex(p, hint);
            if let Some(vi_prev) = vi_prev {
                self.add_constraint_edge(vi_prev, vi, c);
            } else {
                vi0 = Some(vi);
            }

            vi_prev = Some(vi);
            hint = Some(self.tri[vi].face);
        }

        if let (Some(vi_prev), Some(vi0)) = (vi_prev, vi0) {
            self.add_constraint_edge(vi_prev, vi0, c);
        }
    }

    fn add_vertex_at(&mut self, p: IVec2, loc: Location) -> VertexIndex {
        let vi = match loc {
            Location::Empty => {
                let vi = self.tri.create_vertex_with_position(p);
                self.tri.extend_dimension(vi);
                vi
            }
            Location::Vertex(f, v) => self.tri[f].vertices[v],
            Location::Edge(f, e) => {
                let vi = self.tri.create_vertex_with_position(p);
                self.tri.split_edge(f, e, vi);
                self.delaunay_push_vertex(vi);
                vi
            }
            Location::OutsideConvexHull(f) | Location::Face(f) => {
                let vi = self.tri.create_vertex_with_position(p);
                self.tri.split_face(f, vi);
                self.delaunay_push_vertex(vi);
                vi
            }
            Location::OutsideAffineHull => {
                let vi = self.tri.create_vertex_with_position(p);
                self.tri.extend_dimension(vi);
                self.delaunay_push_vertex(vi);
                vi
            }
        };

        self.state
            .dump(1, &format!("after_add_vertex_{}", vi.into_index()), |dump| {
                let delaunay_stack = self.state.delaunay_stack();
                let delaunay_edges = delaunay_stack.map(|stack| stack.as_slice()).unwrap_or(&[]);
                dump.add_tri(&self.tri, [(delaunay_edges, "edge-delaunay", false)]);
            });
        vi
    }

    fn add_constraint_between(&mut self, v0: VertexIndex, v1: VertexIndex, c: u32) {
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

        self.state.dump(1, "after_add_constraint", |dump| {
            let delaunay_stack = self.state.delaunay_stack();
            let delaunay_edges = delaunay_stack.map(|stack| stack.as_slice()).unwrap_or(&[]);
            dump.add_tri(&self.tri, [(delaunay_edges, "edge-delaunay", false)]);
        });
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
        let (mut edge_chain, mut top_chain, mut bottom_chain) = self.state.lock_constraint_chains();
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

            self.state.dump(
                2,
                &format!("before_triangulate_hole_{}_{}", v0.into_index(), v1.into_index()),
                |dump| {
                    let delaunay_stack = self.state.delaunay_stack();
                    let delaunay_edges = delaunay_stack.map(|stack| stack.as_slice()).unwrap_or(&[]);
                    dump.add_tri(
                        &self.tri,
                        [
                            (top_chain.as_slice(), "edge-0", true),
                            (bottom_chain.as_slice(), "edge-1", true),
                            (edge_chain.as_slice(), "edge-2", true),
                            (delaunay_edges, "edge-delaunay", false),
                        ],
                    );
                },
            );

            let mut next_v0 = v0;
            if !edge_chain.is_empty() {
                next_v0 = self.tri.vi(VertexClue::end_of(*edge_chain.last().unwrap()));
                // Collect edges to avoid borrow conflict
                let edges: Vec<_> = edge_chain.iter().copied().collect();
                for edge in edges {
                    self.tri.merge_constraint(edge, c);
                }
            }

            if !top_chain.is_empty() {
                next_v0 = self.tri.vi(VertexClue::end_of(*bottom_chain.last().unwrap()));
                top_chain.reverse();
                let [edge1, edge2] = self.triangulate_hole(&mut top_chain, &mut bottom_chain);
                //self.tri.merge_constraint(edge1, c);
                self.tri[edge1.face].constraints[edge1.edge] |= c;
                self.tri[edge2.face].constraints[edge2.edge] |= c;

                self.delaunay_push_edge(edge1.next());
                self.delaunay_push_edge(edge1.prev());
                self.delaunay_push_edge(edge2.next());
                self.delaunay_push_edge(edge2.prev());
            }

            self.state.dump(
                2,
                &format!("after_triangulate_hole_{}_{}", v0.into_index(), v1.into_index()),
                |dump| {
                    let delaunay_stack = self.state.delaunay_stack();
                    let delaunay_edges = delaunay_stack.map(|stack| stack.as_slice()).unwrap_or(&[]);
                    dump.add_tri(
                        &self.tri,
                        [
                            (top_chain.as_slice(), "edge-0", true),
                            (bottom_chain.as_slice(), "edge-1", true),
                            (edge_chain.as_slice(), "edge-2", true),
                            (delaunay_edges, "edge-delaunay", false),
                        ],
                    );
                },
            );

            v0 = next_v0;
            edge_chain.clear();
            top_chain.clear();
            bottom_chain.clear();
        }

        self.state
            .unlock_constraint_chains((edge_chain, top_chain, bottom_chain));
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
                "Edges are not continuous"
            );
            let p2 = self.tri.vi(VertexClue::end_of(next_edge));

            if self.tri.get_vertices_orientation(p0, p1, p2) <= 0 {
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

                log::trace!("Clipping next ear with edges ({cur_edge:?}, {next_edge:?})");

                self.tri[cur_edge.face].vertices[cur_edge.edge.decrement()] = p2;
                self.tri[next_edge.face].vertices[next_edge.edge] = p0;

                let ne = self.tri.twin_edge(cur_edge);
                self.tri
                    .set_adjacent((ne.face, ne.edge), (next_edge.face, next_edge.edge.decrement()));
                self.tri.set_adjacent(
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

                self.delaunay_push_edge(next_edge.prev());
                self.delaunay_push_edge(next_edge);

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

                log::trace!("Clipping current ear with edges ({cur_edge:?}, {next_edge:?})");

                let ne = self.tri.twin_edge(next_edge);
                self.tri
                    .set_adjacent((ne.face, ne.edge), (cur_edge.face, cur_edge.edge.increment()));
                self.tri.set_adjacent(
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

                self.delaunay_push_edge(cur_edge);
                self.delaunay_push_edge(cur_edge.next());

                // step back
                cur -= 1;
            }

            self.state
                .dump(3, &format!("ear_clipped_{}", cur_edge.face.into_index()), |dump| {
                    let delaunay_stack = self.state.delaunay_stack();
                    let delaunay_edges = delaunay_stack.map(|stack| stack.as_slice()).unwrap_or(&[]);
                    dump.add_tri(
                        &self.tri,
                        [
                            (chain.as_slice(), "edge-0", true),
                            (delaunay_edges, "edge-delaunay", false),
                        ],
                    );
                });
        }

        chain.pop().unwrap()
    }

    /// Triangulates an edge-visible hole given by the edge chain of the upper(lower) polygon.
    /// On completion it returns the edge that separates the upper and lower half of the polygon.
    fn triangulate_hole(&mut self, top: &mut Vec<FaceEdge>, bottom: &mut Vec<FaceEdge>) -> [FaceEdge; 2] {
        assert!(top.len() >= 2 && bottom.len() >= 2);
        let top = self.triangulate_half_hole(top);
        let bottom = self.triangulate_half_hole(bottom);
        let top = FaceEdge::new(top.face, top.edge.decrement());
        let bottom = FaceEdge::new(bottom.face, bottom.edge.decrement());
        self.tri.set_adjacent(top, bottom);
        self.tri.flip(top.face, top.edge)
    }
}
