use crate::math::cdt::indexes::{EdgeIndex, HullIndex, HullVec, PointIndex, PointVec, EMPTY_HULL};
use glam::I64Vec2;

const N: usize = 1 << 10;

/// Integer bucket index (0..1023) for a direction vector.
/// Matches the ordering of the original f64 pseudo_angle:
/// counterclockwise from (-1, +ε) ≈ 0 through (0,1) ≈ 256, (1,0) ≈ 512,
/// (0,-1) ≈ 768, back to (-1, -ε) ≈ 1023.
///
/// Uses a single integer division: `256 * dx / (|dx| + |dy|)`.
pub fn angle_bucket(dir: I64Vec2) -> usize {
    if dir == I64Vec2::ZERO {
        return 0;
    }
    let denom = dir.x.abs() + dir.y.abs();
    let scaled = (256 * dir.x) / denom; // in [-256, 256]
    let raw = if dir.y > 0 || (dir.y == 0 && dir.x > 0) {
        256 + scaled // [0, 512]
    } else {
        768 - scaled // [512, 1024]
    };
    (raw as usize).min(N - 1)
}

/// Comparison: does direction `a` come before `b` in the angular ordering?
/// Uses half-plane classification + cross product. No division.
pub fn angle_less(a: I64Vec2, b: I64Vec2) -> bool {
    let ha = if a.y > 0 || (a.y == 0 && a.x > 0) { 0u8 } else { 1u8 };
    let hb = if b.y > 0 || (b.y == 0 && b.x > 0) { 0u8 } else { 1u8 };
    if ha != hb {
        return ha < hb;
    }
    // Same half-plane: our ordering is CW in standard math convention,
    // so a × b < 0 means a comes before b.
    a.perp_dot(b) < 0
}

/// Ordering for sort tiebreaker.
pub fn angle_cmp(a: I64Vec2, b: I64Vec2) -> std::cmp::Ordering {
    if angle_less(a, b) {
        std::cmp::Ordering::Less
    } else if angle_less(b, a) {
        std::cmp::Ordering::Greater
    } else {
        std::cmp::Ordering::Equal
    }
}

#[derive(Clone, Copy, Debug)]
struct Node {
    dir: I64Vec2,
    edge: EdgeIndex,
    left: HullIndex,
    right: HullIndex,
}

#[derive(Debug)]
pub struct Hull {
    buckets: [HullIndex; N],
    data: HullVec<Node>,
    points: PointVec<HullIndex>,
    empty: Vec<HullIndex>,
}

impl Hull {
    pub fn new(num_points: usize, constrained: bool) -> Hull {
        Hull {
            data: HullVec::new(),
            buckets: [EMPTY_HULL; N],
            points: if constrained {
                PointVec::of(vec![EMPTY_HULL; num_points])
            } else {
                PointVec::new()
            },
            empty: Vec::new(),
        }
    }

    pub fn initialize(&mut self, p: PointIndex, dir: I64Vec2, edge: EdgeIndex) {
        let h = self.data.push(Node {
            dir,
            left: self.data.next_index(),
            right: self.data.next_index(),
            edge,
        });
        if !self.points.is_empty() {
            self.points[p] = h;
        }

        let b = self.bucket(dir);
        assert!(self.buckets[b] == EMPTY_HULL);
        self.buckets[b] = h;
    }

    pub fn update(&mut self, h: HullIndex, e: EdgeIndex) {
        self.data[h].edge = e;
    }

    pub fn get(&self, dir: I64Vec2) -> HullIndex {
        let b = self.bucket(dir);

        let mut h = self.buckets[b];
        if h == EMPTY_HULL {
            let mut t = b;
            while self.buckets[t] == EMPTY_HULL {
                t = (t + 1) % N;
            }
            h = self.buckets[t];
        } else {
            let start = h;
            while angle_less(self.data[h].dir, dir) && self.bucket_h(h) == b {
                h = self.data[h].right;
                if h == start {
                    break;
                }
            }
        }
        assert!(h != EMPTY_HULL);
        self.data[h].left
    }

    pub fn start(&self) -> HullIndex {
        self.buckets
            .iter()
            .filter(|b| **b != EMPTY_HULL)
            .copied()
            .next()
            .unwrap()
    }

    pub fn check(&self) {
        let point = self.buckets.iter().filter(|b| **b != EMPTY_HULL).copied().next();
        assert!(point.is_some());

        let start = point.unwrap();
        assert!(self.buckets[self.bucket_h(start)] == start);

        let mut index = start;
        loop {
            let next = self.data[index].right;
            assert!(index == self.data[next].left);

            let my_bucket = self.bucket_h(index);
            let next_bucket = self.bucket_h(next);
            if next_bucket != my_bucket {
                assert!(self.buckets[next_bucket] == next);
            }

            if next == start {
                break;
            } else {
                let my_dir = self.data[index].dir;
                let next_dir = self.data[next].dir;
                assert!(!angle_less(next_dir, my_dir));
                index = next;
            }
        }
    }

    pub fn left_hull(&self, h: HullIndex) -> HullIndex {
        self.data[h].left
    }

    pub fn right_hull(&self, h: HullIndex) -> HullIndex {
        self.data[h].right
    }

    pub fn edge(&self, h: HullIndex) -> EdgeIndex {
        self.data[h].edge
    }

    pub fn index_of(&self, p: PointIndex) -> HullIndex {
        assert!(!self.points.is_empty());
        let h = self.points[p];
        assert!(h != EMPTY_HULL);
        assert!(self.data[h].left != EMPTY_HULL || self.data[h].right != EMPTY_HULL);
        h
    }

    pub fn move_point(&mut self, old: PointIndex, new: PointIndex) {
        if !self.points.is_empty() {
            self.points[new] = self.points[old];
            self.points[old] = EMPTY_HULL;
        }
    }

    pub fn insert_bare(&mut self, dir: I64Vec2, point: PointIndex, e: EdgeIndex) -> HullIndex {
        self.insert(self.get(dir), dir, point, e)
    }

    pub fn insert(&mut self, left: HullIndex, dir: I64Vec2, point: PointIndex, edge: EdgeIndex) -> HullIndex {
        let right = self.right_hull(left);

        let h = if let Some(h) = self.empty.pop() {
            self.data[h] = Node { dir, edge, left, right };
            h
        } else {
            self.data.push(Node { dir, edge, left, right })
        };

        let b = self.bucket(dir);
        if self.buckets[b] == EMPTY_HULL || (self.buckets[b] == right && !angle_less(self.data[right].dir, dir)) {
            self.buckets[b] = h;
        }

        self.data[right].left = h;
        self.data[left].right = h;

        if !self.points.is_empty() {
            self.points[point] = h;
        }

        h
    }

    pub fn erase(&mut self, h: HullIndex) {
        let next = self.data[h].right;
        let prev = self.data[h].left;

        self.data[next].left = prev;
        self.data[prev].right = next;
        self.data[h].right = EMPTY_HULL;
        self.data[h].left = EMPTY_HULL;

        let b = self.bucket_h(h);
        if self.buckets[b] == h {
            if self.bucket_h(next) == b {
                self.buckets[b] = next;
            } else {
                self.buckets[b] = EMPTY_HULL;
            }
        }

        self.empty.push(h);
    }

    pub fn values(&self) -> impl Iterator<Item = EdgeIndex> + '_ {
        let mut point: HullIndex = self
            .buckets
            .iter()
            .filter(|b| **b != EMPTY_HULL)
            .copied()
            .next()
            .unwrap();
        let start = point;
        let mut started = false;
        std::iter::from_fn(move || {
            let out = self.data[point].edge;
            if point == start && started {
                None
            } else {
                point = self.data[point].right;
                started = true;
                Some(out)
            }
        })
    }

    pub fn bucket_h(&self, h: HullIndex) -> usize {
        self.bucket(self.data[h].dir)
    }

    pub fn bucket(&self, dir: I64Vec2) -> usize {
        angle_bucket(dir)
    }
}
