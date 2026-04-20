use crate::math::triangulation::{
    predicates::{orient2d, test_collinear_points, CollinearTestType, OrientationType},
    EdgeCirculator, FaceIndex, FaceVertex, Rot3Idx, Triangulation, VertexClue, VertexIndex,
};
use std::mem;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Crossing {
    Start { triangle: FaceIndex, vertex: Rot3Idx },
    End { triangle: FaceIndex, vertex: Rot3Idx },
    CoincidentEdge { triangle: FaceIndex, edge: Rot3Idx },
    PositiveEdge { triangle: FaceIndex, edge: Rot3Idx },
    NegativeEdge { triangle: FaceIndex, edge: Rot3Idx },
}

/// Iterator to find all the edges of the triangulation crossing the segment defined by v0 and v1.
pub struct CrossingIterator<'a, const DELAUNAY: bool> {
    tri: &'a Triangulation<DELAUNAY>,
    v0: VertexIndex,
    v1: VertexIndex,
    current: Option<Crossing>,
}

impl<'a, const DELAUNAY: bool> CrossingIterator<'a, DELAUNAY> {
    pub fn new(tri: &Triangulation<DELAUNAY>, v0: VertexIndex, v1: VertexIndex) -> CrossingIterator<'_, DELAUNAY> {
        assert_eq!(tri.dimension(), 2);
        assert_ne!(v0, v1);
        assert!(tri.is_finite_vertex(v0));
        assert!(tri.is_finite_vertex(v1));

        let mut iter = CrossingIterator { tri, v0, v1, current: None };
        iter.current = iter.search_vertex(iter.v0, iter.v0);
        iter
    }

    fn advance(&mut self) -> Option<Crossing> {
        let next = match self.current {
            None => None,
            Some(Crossing::Start { triangle: face, vertex }) => self.search_edge(face, vertex),
            Some(Crossing::End { triangle: face, vertex }) => {
                self.search_vertex(self.tri.vi(FaceVertex::new(face, vertex)), self.v0)
            }
            Some(Crossing::CoincidentEdge { triangle: face, edge }) => self.search_vertex(
                self.tri.vi(VertexClue::edge_end(face, edge)),
                self.tri.vi(VertexClue::edge_start(face, edge)),
            ),
            Some(Crossing::PositiveEdge { triangle: face, edge }) => self.search_edge(face, edge),
            Some(Crossing::NegativeEdge { triangle: face, edge }) => self.search_edge(face, edge),
        };

        mem::replace(&mut self.current, next)
    }

    /// Find next crossing edge by circulating the edges around the base_vertex.
    /// start_vertex is used to avoid going backward whan collinear edges are detected.
    fn search_vertex(&self, base_vertex: VertexIndex, start_vertex: VertexIndex) -> Option<Crossing> {
        let mut start_orientation = OrientationType::Collinear;
        let mut circulator = EdgeCirculator::new(self.tri, base_vertex);

        if base_vertex == self.v1 {
            return None;
        }

        loop {
            let vertex = circulator.end_vertex();
            if self.tri.is_infinite_vertex(vertex) || vertex == self.v0 {
                // skip infinite edges
                circulator.advance_cw();
                continue;
            }

            if vertex == self.v1 {
                return Some(Crossing::CoincidentEdge {
                    triangle: circulator.face(),
                    edge: circulator.edge(),
                });
            }

            let orientation = if vertex == start_vertex {
                // we are on the edge (base_vertex, start_vertex) edge which is just the opposite
                // direction of the crosiing edge, thus any orientation can be picked for the rotate
                OrientationType::CCW
            } else {
                let p0 = self.tri[self.v0].position;
                let p1 = self.tri[self.v1].position;
                let pos = self.tri[vertex].position;

                let orient = orient2d(p0, p1, pos);
                if orient == 0 {
                    let collinear_test = test_collinear_points(p0, p1, pos);
                    match collinear_test {
                        CollinearTestType::Before => {
                            // it's an edge just in the other direction on collinear to the v0-v1 segment, select some "random" orientation
                            OrientationType::CCW
                        }
                        CollinearTestType::First => {
                            panic!("invalid triangulation, p0 == pos; p0 == edge.start; edge.end == p, edge has a zero length")
                        }
                        CollinearTestType::Between => {
                            // pe is between p0 and p1
                            return Some(Crossing::CoincidentEdge {
                                triangle: circulator.face(),
                                edge: circulator.edge(),
                            });
                        }
                        CollinearTestType::Second => {
                            panic!("invalid triangulation, p1 == pos, but v1 != vertex, distinct vertices with the same position")
                        }
                        CollinearTestType::After => {
                            panic!("invalid triangulation, collinear, pos is not contained in the (p0,p1) segment")
                        }
                    }
                } else if orient > 0 {
                    OrientationType::CCW
                } else {
                    OrientationType::CW
                }
            };

            if start_orientation == OrientationType::Collinear {
                // "first" loop iteration, find circulating direction
                assert!(orientation == OrientationType::CW || orientation == OrientationType::CCW);
                start_orientation = orientation;
            }

            if start_orientation != orientation {
                // orientation has changed -> we have the edge crossing the query
                if orientation == OrientationType::CCW {
                    // we have just passed our edge, go back
                    circulator.advance_cw();
                }

                return Some(Crossing::Start {
                    triangle: circulator.face(),
                    vertex: circulator.edge().increment(),
                });
            } else if start_orientation == OrientationType::CCW {
                circulator.advance_cw();
            } else {
                assert_eq!(start_orientation, OrientationType::CW);
                circulator.advance_ccw();
            }
        }
    }

    /// Find next crossing edge by checking the opposite face.
    fn search_edge(&self, start_face: FaceIndex, start_edge: Rot3Idx) -> Option<Crossing> {
        let face = self.tri[start_face].neighbors[start_edge];
        let vertex_index = self.tri[face].find_neighbor(start_face).unwrap();
        let vertex = self.tri[face].vertices[vertex_index];

        if vertex == self.v1 {
            return Some(Crossing::End {
                triangle: face,
                vertex: vertex_index,
            });
        };

        let p0 = self.tri[self.v0].position;
        let p1 = self.tri[self.v1].position;
        let pn = self.tri[vertex].position;

        let orientation = orient2d(p0, p1, pn);
        if orientation == 0 {
            Some(Crossing::End {
                triangle: face,
                vertex: vertex_index,
            })
        } else if orientation > 0 {
            Some(Crossing::NegativeEdge {
                triangle: face,
                edge: vertex_index.increment(),
            })
        } else {
            Some(Crossing::PositiveEdge {
                triangle: face,
                edge: vertex_index.decrement(),
            })
        }
    }
}

impl<'a, const DELAUNAY: bool> Iterator for CrossingIterator<'a, DELAUNAY> {
    type Item = Crossing;

    fn next(&mut self) -> Option<Self::Item> {
        self.advance()
    }
}

impl<const DELAUNAY: bool> Triangulation<DELAUNAY> {
    pub fn crossing_iterator(&self, v0: VertexIndex, v1: VertexIndex) -> CrossingIterator<'_, DELAUNAY> {
        CrossingIterator::new(self, v0, v1)
    }
}
