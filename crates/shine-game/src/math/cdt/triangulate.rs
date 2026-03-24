use crate::math::cdt::{
    contour::{Contour, ContourData},
    half::Half,
    hull::{angle_cmp, angle_less, Hull},
    indexes::{EdgeIndex, HullIndex, PointIndex, PointVec, EMPTY_EDGE},
    predicates::{acute, centroid_sum, distance2, in_circle, orient2d, point_dir},
    CdtError,
};
use glam::{I64Vec2, IVec2};

#[derive(Debug)]
enum Walk {
    Inside(EdgeIndex),
    Done(EdgeIndex),
}

pub struct Triangulation {
    pub(crate) points: PointVec<IVec2>,
    dirs: PointVec<I64Vec2>, // direction from center for each point
    remap: PointVec<usize>,
    next: PointIndex,
    constrained: bool,
    flood_fill: bool,
    skip_crossing: bool,

    endings: PointVec<(usize, usize)>,
    ending_data: Vec<PointIndex>,

    pub(crate) hull: Hull,
    pub(crate) half: Half,
}

impl Triangulation {
    pub fn build(points: &[IVec2]) -> Result<Triangulation, CdtError> {
        let mut t = Self::new(points)?;
        t.run()?;
        Ok(t)
    }

    pub fn build_with_edges<'a, E>(points: &[IVec2], edges: E) -> Result<Triangulation, CdtError>
    where
        E: IntoIterator<Item = &'a (usize, usize)> + Copy,
    {
        let mut t = Self::new_with_edges(points, edges)?;
        t.run()?;
        Ok(t)
    }

    /// Like `build_with_edges`, but keeps all triangles (no flood-fill removal).
    /// Constraint edges are still enforced in the triangulation.
    /// Crossing constraints return `CdtError::CrossingFixedEdge`.
    pub fn build_with_edges_no_fill<'a, E>(points: &[IVec2], edges: E) -> Result<Triangulation, CdtError>
    where
        E: IntoIterator<Item = &'a (usize, usize)> + Copy,
    {
        let mut t = Self::new_with_edges(points, edges)?;
        t.flood_fill = false;
        t.run()?;
        Ok(t)
    }

    /// Like `build_with_edges_no_fill`, but silently skips constraint edges
    /// that would cross an already-inserted fixed edge.
    pub fn build_with_edges_skip_crossing<'a, E>(points: &[IVec2], edges: E) -> Result<Triangulation, CdtError>
    where
        E: IntoIterator<Item = &'a (usize, usize)> + Copy,
    {
        let mut t = Self::new_with_edges(points, edges)?;
        t.flood_fill = false;
        t.skip_crossing = true;
        t.run()?;
        Ok(t)
    }

    pub fn build_from_contours<V>(points: &[IVec2], contours: &[V]) -> Result<Triangulation, CdtError>
    where
        for<'b> &'b V: IntoIterator<Item = &'b usize>,
    {
        let mut t = Self::new_from_contours(points, contours)?;
        t.run()?;
        Ok(t)
    }

    fn validate_input<'a, E>(points: &[IVec2], edges: E) -> Result<(), CdtError>
    where
        E: IntoIterator<Item = &'a (usize, usize)> + Copy,
    {
        if points.is_empty() {
            Err(CdtError::EmptyInput)
        } else if edges
            .into_iter()
            .any(|e| e.0 >= points.len() || e.1 >= points.len() || e.0 == e.1)
        {
            Err(CdtError::InvalidEdge)
        } else if points.len() < 3 {
            Err(CdtError::TooFewPoints)
        } else {
            Ok(())
        }
    }

    pub fn new_with_edges<'a, E>(points: &[IVec2], edges: E) -> Result<Triangulation, CdtError>
    where
        E: IntoIterator<Item = &'a (usize, usize)> + Copy,
    {
        Self::validate_input(points, edges)?;

        // *2 trick: store (xmin+xmax, ymin+ymax) to avoid division
        let (x_bounds, y_bounds) = Self::bbox(points);
        let mut center = I64Vec2::new(
            x_bounds.0 as i64 + x_bounds.1 as i64,
            y_bounds.0 as i64 + y_bounds.1 as i64,
        );
        let mut scale: i64 = 2;

        let mut scratch: Vec<(usize, i64)> = (0..points.len())
            .map(|j| {
                let dir = point_dir(points[j], center, scale);
                (j, distance2(dir))
            })
            .collect();

        let arr = min3(&scratch, points).ok_or(CdtError::CannotInitialize)?;

        let pa = arr[0];
        let mut pb = arr[1];
        let mut pc = arr[2];
        if orient2d(points[pa], points[pb], points[pc]) < 0 {
            std::mem::swap(&mut pb, &mut pc);
        }

        // *3 trick: store raw sum, scale becomes 3
        center = centroid_sum(points[pa], points[pb], points[pc]);
        scale = 3;

        // Re-score distances relative to centroid center
        for item in scratch.iter_mut() {
            let dir = point_dir(points[item.0], center, scale);
            item.1 = distance2(dir);
        }

        scratch.sort_unstable_by(|k, r| {
            if k.0 == pa || k.0 == pb || k.0 == pc {
                std::cmp::Ordering::Less
            } else if r.0 == pa || r.0 == pb || r.0 == pc {
                std::cmp::Ordering::Greater
            } else {
                match k.1.cmp(&r.1) {
                    std::cmp::Ordering::Equal => {
                        let dk = point_dir(points[k.0], center, scale);
                        let dr = point_dir(points[r.0], center, scale);
                        angle_cmp(dk, dr)
                    }
                    e => e,
                }
            }
        });

        assert!((scratch[0].0 == pa) as u8 + (scratch[1].0 == pa) as u8 + (scratch[2].0 == pa) as u8 == 1);
        assert!((scratch[0].0 == pb) as u8 + (scratch[1].0 == pb) as u8 + (scratch[2].0 == pb) as u8 == 1);
        assert!((scratch[0].0 == pc) as u8 + (scratch[1].0 == pc) as u8 + (scratch[2].0 == pc) as u8 == 1);

        scratch[0].0 = pa;
        scratch[1].0 = pb;
        scratch[2].0 = pc;

        let mut sorted_points = PointVec::with_capacity(points.len());
        let mut map_forward = vec![PointIndex::empty(); points.len()];
        let mut map_reverse = PointVec::with_capacity(points.len());

        for i in 0..scratch.len() {
            let mut dupe = None;
            let p = scratch[i];
            if i >= 3 {
                for j in &[i - 1, 0, 1, 2] {
                    let pa = points[scratch[*j].0];
                    let pb = points[p.0];
                    if pa == pb {
                        dupe = Some(scratch[*j].0);
                        break;
                    }
                }
            };
            map_forward[p.0] = match dupe {
                None => {
                    sorted_points.push(points[p.0]);
                    map_reverse.push(p.0)
                }
                Some(d) => {
                    assert!(map_forward[d] != PointIndex::empty());
                    map_forward[d]
                }
            };
        }

        ////////////////////////////////////////////////////////////////////////
        let has_edges = edges.into_iter().count() > 0;
        let mut out = Triangulation {
            hull: Hull::new(sorted_points.len(), has_edges),
            half: Half::new(sorted_points.len()),
            constrained: has_edges,
            flood_fill: has_edges,
            skip_crossing: false,

            remap: map_reverse,
            next: PointIndex::new(0),
            dirs: PointVec::of(sorted_points.iter().map(|p| point_dir(*p, center, scale)).collect()),

            endings: PointVec::of(vec![(0, 0); sorted_points.len()]),
            ending_data: vec![],

            points: sorted_points,
        };

        let pa = out.next;
        let pb = out.next + 1;
        let pc = out.next + 2;
        out.next += 3;
        let e_ab = out.half.insert(pa, pb, pc, EMPTY_EDGE, EMPTY_EDGE, EMPTY_EDGE);
        assert!(e_ab == EdgeIndex::new(0));
        let e_bc = out.half.next(e_ab);
        let e_ca = out.half.prev(e_ab);

        out.hull.initialize(pa, out.dirs[pa], e_ca);
        out.hull.insert_bare(out.dirs[pb], pb, e_ab);
        out.hull.insert_bare(out.dirs[pc], pc, e_bc);

        ////////////////////////////////////////////////////////////////////////
        let mut termination_count = PointVec::of(vec![0; out.points.len()]);
        let edge_iter = || {
            edges.into_iter().map(|&(src, dst)| {
                let src = map_forward[src];
                let dst = map_forward[dst];
                assert!(src != PointIndex::empty());
                assert!(dst != PointIndex::empty());

                if src > dst {
                    (dst, src)
                } else {
                    (src, dst)
                }
            })
        };
        for (src, dst) in edge_iter() {
            if (src, dst) == (pa, pb) {
                out.half.toggle_lock_sign(e_ab);
            } else if (src, dst) == (pa, pc) {
                out.half.toggle_lock_sign(e_ca);
            } else if (src, dst) == (pb, pc) {
                out.half.toggle_lock_sign(e_bc);
            }
            termination_count[dst] += 1;
        }
        let mut cumsum = 0;
        for (dst, t) in termination_count.iter().enumerate() {
            out.endings[PointIndex::new(dst)] = (cumsum, cumsum);
            cumsum += t;
        }
        out.ending_data.resize(cumsum, PointIndex::new(0));
        for (src, dst) in edge_iter() {
            let t = &mut out.endings[dst].1;
            out.ending_data[*t] = src;
            *t += 1;
        }

        Ok(out)
    }

    pub fn new(points: &[IVec2]) -> Result<Triangulation, CdtError> {
        let edges: [(usize, usize); 0] = [];
        Self::new_with_edges(points, &edges)
    }

    pub fn new_from_contours<'a, V>(pts: &[IVec2], contours: &[V]) -> Result<Triangulation, CdtError>
    where
        for<'b> &'b V: IntoIterator<Item = &'b usize>,
    {
        let mut edges = Vec::new();
        for c in contours {
            let next = edges.len();
            for (a, b) in c.into_iter().zip(c.into_iter().skip(1)) {
                edges.push((*a, *b));
            }
            if let Some(start) = edges.get(next) {
                if start.0 != edges.last().unwrap().1 {
                    return Err(CdtError::OpenContour);
                }
            }
        }
        Self::new_with_edges(&pts, &edges)
    }

    pub fn run(&mut self) -> Result<(), CdtError> {
        while !self.done() {
            self.step()?;
        }
        Ok(())
    }

    pub(crate) fn orient2d(&self, pa: PointIndex, pb: PointIndex, pc: PointIndex) -> i64 {
        orient2d(self.points[pa], self.points[pb], self.points[pc])
    }

    fn acute(&self, pa: PointIndex, pb: PointIndex, pc: PointIndex) -> i64 {
        acute(self.points[pa], self.points[pb], self.points[pc])
    }

    pub fn done(&self) -> bool {
        self.next == self.points.len() + 1
    }

    fn make_outer_hull_convex(&mut self) {
        assert!(self.next == self.points.len());
        let mut start = self.hull.start();
        let mut hl = start;
        let mut hr = self.hull.right_hull(hl);
        loop {
            let el = self.hull.edge(hl);
            let er = self.hull.edge(hr);

            let edge_l = self.half.edge(el);
            let edge_r = self.half.edge(er);
            assert!(edge_r.dst == edge_l.src);

            if self.orient2d(edge_l.dst, edge_l.src, edge_r.src) > 0 {
                self.hull.erase(hr);
                let new_edge = self.half.insert(edge_r.src, edge_l.dst, edge_l.src, el, er, EMPTY_EDGE);
                self.hull.update(hl, new_edge);
                self.legalize(self.half.next(new_edge));
                self.legalize(self.half.prev(new_edge));

                hr = hl;
                hl = self.hull.left_hull(hl);
                start = hl;
            } else {
                let next = self.hull.right_hull(hr);
                hl = hr;
                hr = next;
                if hl == start {
                    break;
                }
            }
        }
    }

    fn finalize(&mut self) {
        assert!(self.next == self.points.len());

        if self.constrained && self.flood_fill {
            let h = self.hull.start();
            let e = self.hull.edge(h);
            self.half.flood_erase_from(e);
        } else if self.constrained {
            // Enforce edges but keep all triangles, make hull convex
            self.make_outer_hull_convex();
        } else {
            self.make_outer_hull_convex();
        }

        self.next += 1usize;
    }

    pub fn check(&self) {
        self.hull.check();
        self.half.check();
    }

    pub fn step(&mut self) -> Result<(), CdtError> {
        if self.done() {
            return Err(CdtError::NoMorePoints);
        } else if self.next == self.points.len() {
            self.finalize();
            return Ok(());
        }

        let p = self.next;
        self.next += 1usize;

        let h_ab = self.hull.get(self.dirs[p]);
        let e_ab = self.hull.edge(h_ab);

        let edge = self.half.edge(e_ab);
        let a = edge.src;
        let b = edge.dst;
        assert!(edge.next != EMPTY_EDGE);
        assert!(edge.prev != EMPTY_EDGE);
        assert!(edge.buddy == EMPTY_EDGE);

        assert!(a != b);
        assert!(a != p);
        assert!(b != p);

        let o = self.orient2d(b, a, p);
        let h_p = if o <= 0 {
            if edge.fixed() {
                return Err(CdtError::PointOnFixedEdge(self.remap[p]));
            }
            assert!(edge.buddy == EMPTY_EDGE);
            let edge_bc = self.half.edge(edge.next);
            let edge_ca = self.half.edge(edge.prev);
            let c = edge_bc.dst;
            assert!(c == edge_ca.src);

            let hull_right = self.hull.right_hull(h_ab);
            let hull_left = self.hull.left_hull(h_ab);

            self.half.erase(e_ab);

            let e_pc = self.half.insert(p, c, a, edge_ca.buddy, EMPTY_EDGE, EMPTY_EDGE);
            let e_cp = self.half.insert(c, p, b, EMPTY_EDGE, edge_bc.buddy, e_pc);

            self.hull.update(h_ab, self.half.next(e_cp));

            let h_ap = self.hull.insert(h_ab, self.dirs[p], p, self.half.prev(e_pc));

            if self.hull.edge(hull_right) == edge.prev {
                self.hull.update(hull_right, self.half.next(e_pc));
            }
            if self.hull.edge(hull_left) == edge.next {
                self.hull.update(hull_left, self.half.prev(e_cp));
            }

            self.legalize(self.half.prev(e_cp));
            self.legalize(self.half.next(e_pc));
            h_ap
        } else {
            let f = self.half.insert(b, a, p, EMPTY_EDGE, EMPTY_EDGE, e_ab);
            assert!(o != 0);
            assert!(o > 0);

            self.hull.update(h_ab, self.half.prev(f));

            let da = self.dirs[a];
            let dp = self.dirs[p];
            let same = !angle_less(da, dp) && !angle_less(dp, da);
            let h_p = if !same {
                let h_ap = self.hull.insert(h_ab, self.dirs[p], p, self.half.next(f));
                self.legalize(f);
                h_ap
            } else {
                let h_ca = self.hull.right_hull(h_ab);
                let e_ca = self.hull.edge(h_ca);
                let edge_ca = self.half.edge(e_ca);
                assert!(a == edge_ca.dst);
                let c = edge_ca.src;
                let g = self.half.insert(a, c, p, EMPTY_EDGE, self.half.next(f), e_ca);

                self.hull.update(h_ca, self.half.next(g));
                self.hull.move_point(a, p);

                self.legalize(f);
                self.legalize(g);
                h_ca
            };

            self.check_acute_left(p, h_p);
            self.check_acute_right(p, h_p);
            h_p
        };

        let (start, end) = self.endings[p];
        for i in start..end {
            self.handle_fixed_edge(h_p, p, self.ending_data[i])?;
        }

        Ok(())
    }

    fn check_acute_left(&mut self, p: PointIndex, h_p: HullIndex) {
        let mut h_b = h_p;
        loop {
            h_b = self.hull.left_hull(h_b);
            let e_pb = self.hull.edge(h_b);
            let edge_pb = self.half.edge(e_pb);
            let b = edge_pb.dst;

            let h_q = self.hull.left_hull(h_b);
            let e_bq = self.hull.edge(h_q);
            let edge_bq = self.half.edge(e_bq);
            let q = edge_bq.dst;

            if (!self.constrained && self.acute(p, b, q) <= 0) || self.orient2d(p, b, q) >= 0 {
                break;
            }

            self.hull.erase(h_b);

            let e_pq = self.half.insert(p, q, b, e_bq, e_pb, EMPTY_EDGE);
            self.hull.update(h_q, e_pq);
            h_b = h_p;

            self.legalize(self.half.next(e_pq));
            self.legalize(self.half.prev(e_pq));
        }
    }

    fn check_acute_right(&mut self, p: PointIndex, h_p: HullIndex) {
        let mut h_a = h_p;
        loop {
            let e_ap = self.hull.edge(h_a);
            let edge_ap = self.half.edge(e_ap);
            let a = edge_ap.src;
            assert!(a != p);

            h_a = self.hull.right_hull(h_a);
            let e_qa = self.hull.edge(h_a);
            let edge_qa = self.half.edge(e_qa);
            let q = edge_qa.src;

            if (!self.constrained && self.acute(p, a, q) <= 0) || self.orient2d(p, a, q) <= 0 {
                break;
            }

            self.hull.erase(h_a);
            let edge_qp = self.half.insert(q, p, a, e_ap, e_qa, EMPTY_EDGE);
            self.hull.update(h_p, edge_qp);
            h_a = h_p;

            self.legalize(self.half.next(edge_qp));
            self.legalize(self.half.prev(edge_qp));
        }
    }

    fn find_hull_walk_mode(&self, h: HullIndex, src: PointIndex, dst: PointIndex) -> Result<Walk, CdtError> {
        let e_right = self.hull.edge(h);
        let h_left = self.hull.left_hull(h);
        let e_left = self.hull.edge(h_left);

        let wedge_left = self.half.edge(e_left).dst;
        let wedge_right = self.half.edge(e_right).src;

        if dst == wedge_left {
            return Ok(Walk::Done(e_left));
        } else if dst == wedge_right {
            return Ok(Walk::Done(e_right));
        }

        let o_left = self.orient2d(src, wedge_left, dst);
        let o_right = self.orient2d(src, dst, wedge_right);

        if o_left == 0 {
            return Err(CdtError::PointOnFixedEdge(self.remap[wedge_left]));
        } else if o_right == 0 {
            return Err(CdtError::PointOnFixedEdge(self.remap[wedge_right]));
        }

        let mut index_a_src = self.half.edge(e_left).prev;

        loop {
            let edge_a_src = self.half.edge(index_a_src);
            let a = edge_a_src.src;
            if a == dst {
                return Ok(Walk::Done(index_a_src));
            }

            let intersected_index = edge_a_src.prev;

            let o = self.orient2d(src, dst, a);
            if o > 0 {
                return Ok(Walk::Inside(intersected_index));
            } else if o < 0 {
                let buddy = edge_a_src.buddy;
                if buddy == EMPTY_EDGE {
                    return Err(CdtError::WedgeEscape);
                }
                index_a_src = self.half.edge(buddy).prev;
            } else {
                return Err(CdtError::PointOnFixedEdge(self.remap[a]));
            }
        }
    }

    /// Read-only walk: returns true if the path from src to dst via edge `e`
    /// would cross a fixed edge. Does not modify the triangulation.
    fn walk_would_cross_fixed(&self, src: PointIndex, dst: PointIndex, mut e: EdgeIndex) -> bool {
        let edge_ba = self.half.edge(e);
        if edge_ba.fixed() {
            return true;
        }
        assert!(edge_ba.buddy != EMPTY_EDGE);
        e = edge_ba.buddy;

        loop {
            let edge_ab = self.half.edge(e);
            let e_bc = edge_ab.next;
            let e_ca = edge_ab.prev;
            let edge_bc = self.half.edge(e_bc);
            let edge_ca = self.half.edge(e_ca);
            let c = edge_bc.dst;

            if c == dst {
                return false;
            }

            let o_psc = self.orient2d(src, dst, c);
            if o_psc > 0 {
                if edge_bc.fixed() {
                    return true;
                }
                assert!(edge_bc.buddy != EMPTY_EDGE);
                e = edge_bc.buddy;
            } else if o_psc < 0 {
                if edge_ca.fixed() {
                    return true;
                }
                assert!(edge_ca.buddy != EMPTY_EDGE);
                e = edge_ca.buddy;
            } else {
                return true;
            }
        }
    }

    fn walk_fill(&mut self, src: PointIndex, dst: PointIndex, mut e: EdgeIndex) -> Result<(), CdtError> {
        let mut steps_left = Contour::new_pos(src, ContourData::None);
        let mut steps_right = Contour::new_neg(src, ContourData::None);

        let edge_ba = self.half.edge(e);
        let e_ac = edge_ba.next;
        let e_cb = edge_ba.prev;
        let edge_ac = self.half.edge(e_ac);
        let edge_cb = self.half.edge(e_cb);

        self.half.erase(e);

        steps_left.push(
            self,
            edge_ba.src,
            if edge_cb.buddy != EMPTY_EDGE {
                ContourData::Buddy(edge_cb.buddy)
            } else {
                let hl = self.hull.index_of(edge_cb.dst);
                assert!(self.hull.edge(hl) == e_cb);
                ContourData::Hull(hl, edge_cb.sign)
            },
        );
        steps_right.push(
            self,
            edge_ba.dst,
            if edge_ac.buddy != EMPTY_EDGE {
                ContourData::Buddy(edge_ac.buddy)
            } else {
                let hr = self.hull.index_of(edge_ac.dst);
                assert!(self.hull.edge(hr) == e_ac);
                ContourData::Hull(hr, edge_ac.sign)
            },
        );

        if edge_ba.fixed() {
            return Err(CdtError::CrossingFixedEdge);
        }
        assert!(edge_ba.buddy != EMPTY_EDGE);
        e = edge_ba.buddy;

        loop {
            let edge_ab = self.half.edge(e);
            let e_bc = edge_ab.next;
            let e_ca = edge_ab.prev;
            let edge_bc = self.half.edge(e_bc);
            let edge_ca = self.half.edge(e_ca);
            let c = edge_bc.dst;

            self.half.erase(e);

            if c == dst {
                let e_dst_src = steps_left
                    .push(
                        self,
                        c,
                        if edge_bc.buddy == EMPTY_EDGE {
                            let h = self.hull.index_of(edge_bc.dst);
                            assert!(self.hull.edge(h) == e_bc);
                            ContourData::Hull(h, edge_bc.sign)
                        } else {
                            ContourData::Buddy(edge_bc.buddy)
                        },
                    )
                    .expect("Failed to create fixed edge");

                assert!(self.half.edge(e_dst_src).dst == src);
                assert!(self.half.edge(e_dst_src).src == dst);

                let e_src_dst = steps_right
                    .push(
                        self,
                        c,
                        if edge_ca.buddy == EMPTY_EDGE {
                            let h = self.hull.index_of(edge_ca.dst);
                            assert!(self.hull.edge(h) == e_ca);
                            ContourData::Hull(h, edge_ca.sign)
                        } else {
                            ContourData::Buddy(edge_ca.buddy)
                        },
                    )
                    .expect("Failed to create second fixed edge");

                assert!(self.half.edge(e_src_dst).src == src);
                assert!(self.half.edge(e_src_dst).dst == dst);

                self.half.link(e_src_dst, e_dst_src);
                self.half.toggle_lock_sign(e_src_dst);

                break;
            }

            let o_psc = self.orient2d(src, dst, c);
            e = if o_psc > 0 {
                steps_right.push(
                    self,
                    c,
                    if edge_ca.buddy == EMPTY_EDGE {
                        let h = self.hull.index_of(edge_ca.dst);
                        assert!(self.hull.edge(h) == e_ca);
                        ContourData::Hull(h, edge_ca.sign)
                    } else {
                        ContourData::Buddy(edge_ca.buddy)
                    },
                );

                if edge_bc.fixed() {
                    return Err(CdtError::CrossingFixedEdge);
                }
                assert!(edge_bc.buddy != EMPTY_EDGE);
                edge_bc.buddy
            } else if o_psc < 0 {
                steps_left.push(
                    self,
                    c,
                    if edge_bc.buddy == EMPTY_EDGE {
                        let h = self.hull.index_of(edge_bc.dst);
                        assert!(self.hull.edge(h) == e_bc);
                        ContourData::Hull(h, edge_bc.sign)
                    } else {
                        ContourData::Buddy(edge_bc.buddy)
                    },
                );

                if edge_ca.fixed() {
                    return Err(CdtError::CrossingFixedEdge);
                }
                assert!(edge_ca.buddy != EMPTY_EDGE);
                edge_ca.buddy
            } else {
                return Err(CdtError::PointOnFixedEdge(self.remap[c]));
            }
        }
        Ok(())
    }

    fn handle_fixed_edge(&mut self, h: HullIndex, src: PointIndex, dst: PointIndex) -> Result<(), CdtError> {
        match self.find_hull_walk_mode(h, src, dst)? {
            Walk::Done(e) => {
                self.half.toggle_lock_sign(e);
                Ok(())
            }
            Walk::Inside(e) => {
                if self.skip_crossing && self.walk_would_cross_fixed(src, dst, e) {
                    return Ok(());
                }
                self.walk_fill(src, dst, e)
            }
        }
    }

    pub(crate) fn legalize(&mut self, e_ab: EdgeIndex) {
        let edge = self.half.edge(e_ab);
        if edge.fixed() || edge.buddy == EMPTY_EDGE {
            return;
        }
        let a = edge.src;
        let b = edge.dst;
        let c = self.half.edge(self.half.next(e_ab)).dst;

        let e_ba = edge.buddy;
        let e_ad = self.half.next(e_ba);
        let d = self.half.edge(e_ad).dst;

        if in_circle(self.points[a], self.points[b], self.points[c], self.points[d]) > 0 {
            let e_db = self.half.prev(e_ba);

            self.half.swap(e_ab);
            self.legalize(e_ad);
            self.legalize(e_db);
        }
    }

    pub(crate) fn bbox(points: &[IVec2]) -> ((i32, i32), (i32, i32)) {
        let (mut xmin, mut xmax) = (i32::MAX, i32::MIN);
        let (mut ymin, mut ymax) = (i32::MAX, i32::MIN);
        for p in points.iter() {
            xmin = xmin.min(p.x);
            ymin = ymin.min(p.y);
            xmax = xmax.max(p.x);
            ymax = ymax.max(p.y);
        }
        ((xmin, xmax), (ymin, ymax))
    }

    pub fn triangles(&self) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
        self.half
            .iter_triangles()
            .map(move |(a, b, c)| (self.remap[a], self.remap[b], self.remap[c]))
    }

    pub fn inside(&self, p: IVec2) -> bool {
        self.half.iter_triangles().any(|(a, b, c)| {
            orient2d(self.points[a], self.points[b], p) >= 0
                && orient2d(self.points[b], self.points[c], p) >= 0
                && orient2d(self.points[c], self.points[a], p) >= 0
        })
    }

    pub fn save_svg(&self, filename: &str) -> std::io::Result<()> {
        std::fs::write(filename, self.to_svg(false))
    }

    pub fn save_debug_svg(&self, filename: &str) -> std::io::Result<()> {
        std::fs::write(filename, self.to_svg(true))
    }

    /// SVG output — the only place f64 is used (display-only).
    pub fn to_svg(&self, debug: bool) -> String {
        let (x_bounds, y_bounds) = Self::bbox(&self.points);
        let x_range = (x_bounds.0 as f64, x_bounds.1 as f64);
        let y_range = (y_bounds.0 as f64, y_bounds.1 as f64);
        let scale = 800.0 / (x_range.1 - x_range.0).max(y_range.1 - y_range.0);
        let line_width = 2.0;
        let dx = |x: i32| scale * (x as f64 - x_range.0) + line_width;
        let dy = |y: i32| scale * (y_range.1 - y as f64) + line_width;

        let mut out = String::new();
        out.push_str(&format!(
            r#"<svg viewbox="auto" xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">
    <rect x="0" y="0" width="{}" height="{}"
     style="fill:rgb(0,0,0)" />"#,
            scale * (x_range.1 - x_range.0) + line_width * 2.0,
            scale * (y_range.1 - y_range.0) + line_width * 2.0,
            dx(x_bounds.1) + line_width,
            dy(y_bounds.0) + line_width
        ));

        if debug {
            for (p, (start, end)) in self.endings.iter().enumerate() {
                for i in *start..*end {
                    let dst = PointIndex::new(p);
                    let src = self.ending_data[i];
                    out.push_str(&format!(
                        r#"
            <line x1="{}" y1="{}" x2="{}" y2="{}"
             style="stroke:rgb(0,255,0)"
             stroke-width="{}" stroke-linecap="round" />"#,
                        dx(self.points[src].x),
                        dy(self.points[src].y),
                        dx(self.points[dst].x),
                        dy(self.points[dst].y),
                        line_width
                    ));
                }
            }
        }

        for (pa, pb, fixed) in self.half.iter_edges() {
            out.push_str(&format!(
                r#"
    <line x1="{}" y1="{}" x2="{}" y2="{}"
     style="{}"
     stroke-width="{}"
     stroke-linecap="round" />"#,
                dx(self.points[pa].x),
                dy(self.points[pa].y),
                dx(self.points[pb].x),
                dy(self.points[pb].y),
                if fixed {
                    "stroke:rgb(255,255,255)"
                } else {
                    "stroke:rgb(255,0,0)"
                },
                line_width
            ))
        }

        if debug {
            for e in self.hull.values() {
                let edge = self.half.edge(e);
                let (pa, pb) = (edge.src, edge.dst);
                out.push_str(&format!(
                    r#"
        <line x1="{}" y1="{}" x2="{}" y2="{}"
         style="stroke:rgb(255,255,0)"
         stroke-width="{}" stroke-dasharray="{}"
         stroke-linecap="round" />"#,
                    dx(self.points[pa].x),
                    dy(self.points[pa].y),
                    dx(self.points[pb].x),
                    dy(self.points[pb].y),
                    line_width,
                    line_width * 2.0
                ))
            }
        }

        for p in self.points.iter() {
            out.push_str(&format!(
                r#"
        <circle cx="{}" cy="{}" r="{}" style="fill:rgb(255,128,128)" />"#,
                dx(p.x),
                dy(p.y),
                line_width
            ));
        }

        out.push_str("\n</svg>");
        out
    }
}

fn min3(buf: &[(usize, i64)], points: &[IVec2]) -> Option<[usize; 3]> {
    let mut array = [(0, i64::MAX); 3];
    for &(p, score) in buf.iter() {
        if score < array[0].1 {
            array[0] = (p, score);
        }
    }
    if array[0].1 == i64::MAX {
        return None;
    }
    for &(p, score) in buf.iter() {
        if score < array[1].1 {
            if points[array[0].0] != points[p] {
                array[1] = (p, score);
            }
        }
    }
    if array[1].1 == i64::MAX {
        return None;
    }
    for &(p, score) in buf.iter() {
        if score < array[2].1 {
            let p0 = points[array[0].0];
            let p1 = points[array[1].0];
            if orient2d(p0, p1, points[p]) != 0 {
                array[2] = (p, score);
            }
        }
    }
    if array[2].1 == i64::MAX {
        return None;
    }

    Some([array[0].0, array[1].0, array[2].0])
}
