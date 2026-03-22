use crate::math::cdt::{
    indexes::{ContourIndex, ContourVec, EdgeIndex, HullIndex, PointIndex, EMPTY_CONTOUR, EMPTY_EDGE},
    triangulate::Triangulation,
};

#[derive(Copy, Clone, Debug)]
pub enum ContourData {
    None,
    Buddy(EdgeIndex),
    Hull(HullIndex, Option<bool>),
}

#[derive(Copy, Clone, Debug)]
struct Node {
    point: PointIndex,
    data: ContourData,
    prev: ContourIndex,
    next: ContourIndex,
}

pub struct Contour {
    pts: ContourVec<Node>,
    end: ContourIndex,
    index: ContourIndex,
    sign: bool,
}

/// A contour marks a set of points that form the boundary of a pseudopolygon
/// during fixed edge insertion.
///
/// Triangulation is based on ["Triangulating Monotone Mountains"](http://www.ams.sunysb.edu/~jsbm/courses/345/13/triangulating-monotone-mountains.pdf)
impl Contour {
    fn new(point: PointIndex, data: ContourData, sign: bool) -> Self {
        let n = Node {
            point,
            data,
            prev: EMPTY_CONTOUR,
            next: EMPTY_CONTOUR,
        };
        Contour {
            pts: ContourVec::of(vec![n]),
            end: ContourIndex::new(0),
            index: ContourIndex::new(0),
            sign,
        }
    }

    pub fn new_pos(point: PointIndex, data: ContourData) -> Self {
        Self::new(point, data, true)
    }

    pub fn new_neg(point: PointIndex, data: ContourData) -> Self {
        Self::new(point, data, false)
    }

    pub fn push(&mut self, t: &mut Triangulation, point: PointIndex, data: ContourData) -> Option<EdgeIndex> {
        let i = self.pts.push(Node {
            point,
            data,
            next: EMPTY_CONTOUR,
            prev: self.end,
        });
        assert!(self.pts[self.end].next == EMPTY_CONTOUR);
        self.pts[self.end].next = i;
        self.end = i;

        let mut out = None;
        while let Some(e) = self.try_clip(t) {
            out = Some(e);
        }
        self.index = self.pts[self.index].next;
        assert!(self.pts[self.index].next == EMPTY_CONTOUR);
        out
    }

    fn try_clip(&mut self, t: &mut Triangulation) -> Option<EdgeIndex> {
        let c = self.pts[self.index];
        assert!(c.next != EMPTY_CONTOUR);
        if c.prev == EMPTY_CONTOUR {
            return None;
        }

        let new_edge = if self.sign {
            let (a, b) = (self.pts[c.next], self.pts[c.prev]);

            if t.orient2d(a.point, b.point, c.point) <= 0 {
                return None;
            }

            let e_ab = t
                .half
                .insert(a.point, b.point, c.point, EMPTY_EDGE, EMPTY_EDGE, EMPTY_EDGE);
            let edge_ab = t.half.edge(e_ab);
            let e_ca = edge_ab.prev;
            let e_bc = edge_ab.next;
            match a.data {
                ContourData::None => (),
                ContourData::Hull(hull_index, sign) => {
                    t.hull.update(hull_index, e_ca);
                    t.half.set_sign(e_bc, sign);
                }
                ContourData::Buddy(b) => t.half.link_new(b, e_ca),
            };
            match c.data {
                ContourData::None => (),
                ContourData::Hull(hull_index, sign) => {
                    t.hull.update(hull_index, e_bc);
                    t.half.set_sign(e_bc, sign);
                }
                ContourData::Buddy(b) => t.half.link_new(b, e_bc),
            };

            e_ab
        } else {
            let (a, b) = (self.pts[c.next], self.pts[c.prev]);
            assert!(a.point != b.point);
            assert!(a.point != c.point);
            assert!(b.point != c.point);

            if t.orient2d(a.point, c.point, b.point) <= 0 {
                return None;
            }

            let e_ba = t
                .half
                .insert(b.point, a.point, c.point, EMPTY_EDGE, EMPTY_EDGE, EMPTY_EDGE);
            let edge_ba = t.half.edge(e_ba);
            let e_cb = edge_ba.prev;
            let e_ac = edge_ba.next;
            match a.data {
                ContourData::None => (),
                ContourData::Hull(hull_index, sign) => {
                    t.hull.update(hull_index, e_ac);
                    t.half.set_sign(e_ac, sign);
                }
                ContourData::Buddy(b) => t.half.link_new(b, e_ac),
            };
            match c.data {
                ContourData::None => (),
                ContourData::Hull(hull_index, sign) => {
                    t.hull.update(hull_index, e_cb);
                    t.half.set_sign(e_cb, sign);
                }
                ContourData::Buddy(b) => t.half.link_new(b, e_cb),
            };
            e_ba
        };

        {
            let edge = t.half.edge(new_edge);
            t.legalize(edge.next);
            t.legalize(edge.prev);
        }

        self.pts[self.index] = Node {
            point: PointIndex::new(0),
            data: ContourData::None,
            prev: EMPTY_CONTOUR,
            next: EMPTY_CONTOUR,
        };
        self.pts[c.next].prev = c.prev;
        self.pts[c.prev].next = c.next;

        self.pts[c.next].data = ContourData::Buddy(new_edge);

        self.index = c.prev;

        Some(new_edge)
    }
}
