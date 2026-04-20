use crate::{
    indexed::TypedIndex,
    math::triangulation::{
        predicates::{orient2d, test_collinear_points, CollinearTestType},
        FaceIndex, Rot3Idx, Triangulation, VertexClue, VertexIndex,
    },
};
use glam::IVec2;

/// Result of a point location query
#[derive(Debug)]
pub enum Location {
    /// Triangulation is empty
    Empty,

    /// Point is on the vertex of a triangle
    Vertex(FaceIndex, Rot3Idx),

    /// Point is on the edge of a triangle
    Edge(FaceIndex, Rot3Idx),

    /// Point is inside a triangle
    Face(FaceIndex),

    /// Point is outside the convex hull
    OutsideConvexHull(FaceIndex),

    /// Point is outside the affine hull, dimension have to be extended
    OutsideAffineHull,
}

#[derive(Debug)]
enum ContainmentResult {
    Continue(FaceIndex),
    Stop(u8),
}

impl ContainmentResult {
    fn set(&mut self, i: Rot3Idx, b: bool) {
        assert!(i.is_valid());
        if b {
            match *self {
                ContainmentResult::Stop(ref mut t) => *t |= 1 << u8::from(i),
                _ => unreachable!(),
            }
        }
    }
}

/// Point location queries
impl<const DELAUNAY: bool> Triangulation<DELAUNAY> {
    /// Locate where a point falls in the triangulation.
    ///
    /// Returns the location of the point relative to the triangulation:
    /// - `Empty`: triangulation has no points
    /// - `Vertex`: point coincides with an existing vertex
    /// - `Edge`: point lies on an edge
    /// - `Face`: point is inside a face
    /// - `OutsideConvexHull`: point is outside the convex hull
    /// - `OutsideAffineHull`: point is outside the affine hull (dimension needs extending)
    pub fn locate_position(&mut self, p: IVec2, hint: Option<FaceIndex>) -> Result<Location, String> {
        match self.dimension() {
            u8::MAX => Ok(Location::Empty),
            0 => self.locate_position_dim0(p),
            1 => self.locate_position_dim1(p),
            2 => self.locate_position_dim2(p, hint),
            dim => unreachable!("Invalid dimension: {}", dim),
        }
    }

    /// Find the location of a point in a single point triangulation (dimension = 0).
    fn locate_position_dim0(&self, p: IVec2) -> Result<Location, String> {
        assert!(self.dimension() == 0);

        // find the (only) finite vertex
        let v0 = {
            let v = VertexIndex::new(1);
            if !self.is_infinite_vertex(v) {
                v
            } else {
                VertexIndex::new(0)
            }
        };
        let p0 = self[v0].position;

        if p == p0 {
            let f0 = self[v0].triangle;
            Ok(Location::Vertex(f0, self[f0].find_vertex(v0).unwrap()))
        } else {
            Ok(Location::OutsideAffineHull)
        }
    }

    /// Find the location of a point in a straight line strip. (dimension = 1)
    fn locate_position_dim1(&self, p: IVec2) -> Result<Location, String> {
        assert!(self.dimension() == 1);

        // calculate the convex hull of the 1-d mesh
        // the convex hull is a segment made up from the two (finite) neighboring vertices of the infinite vertex

        let v_inf = self.infinite_vertex();
        // first point of the convex hull (segments)
        let f0 = self.infinite_face();
        let iv0 = self[f0].find_vertex(v_inf).unwrap();
        let cp0 = self[VertexClue::face_vertex(f0, iv0.mirror(2))].position;

        // last point of the convex hull (segments)
        let f1 = self[f0].neighbors[iv0.mirror(2)];
        let iv1 = self[f1].find_vertex(v_inf).unwrap();
        let cp1 = self[VertexClue::face_vertex(f1, iv1.mirror(2))].position;

        let orient = orient2d(cp0, cp1, p);
        if orient != 0 {
            Ok(Location::OutsideAffineHull)
        } else {
            // point is on the line
            let t = test_collinear_points(cp0, cp1, p);
            if t == CollinearTestType::Before {
                Ok(Location::OutsideConvexHull(f0))
            } else if t == CollinearTestType::First {
                Ok(Location::Vertex(f0, iv0.mirror(2)))
            } else if t == CollinearTestType::Second {
                Ok(Location::Vertex(f1, iv1.mirror(2)))
            } else if t == CollinearTestType::After {
                Ok(Location::OutsideConvexHull(f1))
            } else {
                assert!(t == CollinearTestType::Between);
                // Start from an infinite face(f0) and advance to the neighboring segments while the
                // the edge(face) containing the point is not found
                let mut prev = f0;
                let mut dir = iv0;
                loop {
                    let cur = self[prev].neighbors[dir];
                    assert!(self.is_finite_face(cur));

                    let p0 = self[VertexClue::face_vertex(cur, Rot3Idx::new(0))].position;
                    let p1 = self[VertexClue::face_vertex(cur, Rot3Idx::new(1))].position;

                    let t = test_collinear_points(p0, p1, p);
                    if t == CollinearTestType::First {
                        // identical to p0
                        return Ok(Location::Vertex(cur, Rot3Idx::new(0)));
                    } else if t == CollinearTestType::Second {
                        // identical to p1
                        return Ok(Location::Vertex(cur, Rot3Idx::new(1)));
                    } else if t == CollinearTestType::Between {
                        // inside the (p0,p1) segment
                        return Ok(Location::Edge(cur, Rot3Idx::new(2)));
                    } else {
                        // advance to the next edge
                        let vi = self[cur].find_neighbor(prev).unwrap();
                        prev = cur;
                        dir = vi.mirror(2);
                    }
                }
            }
        }
    }

    /// Test which half-space contains the p point.
    fn test_containment_face(&self, pos: IVec2, triangle: FaceIndex) -> ContainmentResult {
        let p0 = self.p(VertexClue::face_vertex(triangle, Rot3Idx::new(0)));
        let p1 = self.p(VertexClue::face_vertex(triangle, Rot3Idx::new(1)));
        let p2 = self.p(VertexClue::face_vertex(triangle, Rot3Idx::new(2)));

        let e01 = orient2d(p0, p1, pos);
        if e01 < 0 {
            let next = self[triangle].neighbors[Rot3Idx::new(2)];
            return ContainmentResult::Continue(next);
        }

        let e20 = orient2d(p2, p0, pos);
        if e20 < 0 {
            let next = self[triangle].neighbors[Rot3Idx::new(1)];
            return ContainmentResult::Continue(next);
        }

        let e12 = orient2d(p1, p2, pos);
        if e12 < 0 {
            let next = self[triangle].neighbors[Rot3Idx::new(0)];
            return ContainmentResult::Continue(next);
        }

        let mut test = ContainmentResult::Stop(0);
        test.set(Rot3Idx::new(2), e01 == 0);
        test.set(Rot3Idx::new(0), e12 == 0);
        test.set(Rot3Idx::new(1), e20 == 0);
        test
    }

    /// Test the containment of the p position with respect to the half-spaces defined by the (a,b) and (b,c) edges.
    fn test_containment(
        &self,
        pos: IVec2,
        triangle: FaceIndex,
        a: Rot3Idx,
        b: Rot3Idx,
        c: Rot3Idx,
        tag: usize,
    ) -> ContainmentResult {
        let pa = self.p(VertexClue::face_vertex(triangle, a));
        let pb = self.p(VertexClue::face_vertex(triangle, b));
        let ab = orient2d(pa, pb, pos);
        if ab < 0 {
            let next = self[triangle].neighbors[c];
            if self[next].tag != tag {
                return ContainmentResult::Continue(next);
            }
        }

        let pc = self.p(VertexClue::face_vertex(triangle, c));
        let bc = orient2d(pb, pc, pos);
        if bc < 0 {
            let next = self[triangle].neighbors[a];
            assert!(self[next].tag != tag);
            return ContainmentResult::Continue(next);
        }

        let mut test = ContainmentResult::Stop(0);
        test.set(c, ab == 0);
        test.set(a, bc == 0);
        test
    }

    /// Find the location of a point in a triangulation. (dimension = 2)
    fn locate_position_dim2(&mut self, p: IVec2, hint: Option<FaceIndex>) -> Result<Location, String> {
        assert_eq!(self.dimension(), 2);

        let start = {
            let hint = hint.unwrap_or_else(|| self.infinite_face());
            match self[hint].find_vertex(self.infinite_vertex()) {
                None => hint,                       // finite face
                Some(i) => self[hint].neighbors[i], // the opposite face to an infinite vertex is finite
            }
        };
        assert!(self.is_finite_face(start));

        let mut prev = FaceIndex::NONE;
        let mut cur = start;

        // keep a mutable reference to tag to avoid any additional interference in tag increment during traverse
        let scope = self.scope_guard();
        let tag = &mut *scope.borrow_mut();
        *tag += 1;

        loop {
            if self.is_infinite_face(cur) {
                return Ok(Location::OutsideConvexHull(cur));
            }

            self[cur].tag = *tag;
            let from = self[cur].find_neighbor(prev);

            let test_result = match from.map(|r| u8::from(r)) {
                None => self.test_containment_face(p, cur),
                Some(0) => self.test_containment(p, cur, Rot3Idx::new(2), Rot3Idx::new(0), Rot3Idx::new(1), *tag),
                Some(1) => self.test_containment(p, cur, Rot3Idx::new(0), Rot3Idx::new(1), Rot3Idx::new(2), *tag),
                Some(2) => self.test_containment(p, cur, Rot3Idx::new(1), Rot3Idx::new(2), Rot3Idx::new(0), *tag),
                Some(i) => unreachable!("Invalid index: {:?}", i),
            };
            match test_result {
                ContainmentResult::Continue(next) => {
                    prev = cur;
                    cur = next;
                }

                ContainmentResult::Stop(0) => return Ok(Location::Face(cur)),
                ContainmentResult::Stop(1) => return Ok(Location::Edge(cur, Rot3Idx::new(0))), // only on 0 edge
                ContainmentResult::Stop(2) => return Ok(Location::Edge(cur, Rot3Idx::new(1))), // only on 1 edge
                ContainmentResult::Stop(4) => return Ok(Location::Edge(cur, Rot3Idx::new(2))), // only on 2 edge
                ContainmentResult::Stop(6) => return Ok(Location::Vertex(cur, Rot3Idx::new(0))), //both on 1,2 edge
                ContainmentResult::Stop(5) => return Ok(Location::Vertex(cur, Rot3Idx::new(1))), //both on 0,2 edge
                ContainmentResult::Stop(3) => return Ok(Location::Vertex(cur, Rot3Idx::new(2))), //both on 0,1 edge

                ContainmentResult::Stop(e) => unreachable!("Invalid test_result: {}", e),
            }
        }
    }
}
