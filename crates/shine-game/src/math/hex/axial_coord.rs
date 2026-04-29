use crate::math::{
    hex::{AxialBase, HexFlatDir, HexPointyDir},
    SQRT_3,
};
use glam::Vec2;
use serde::{Deserialize, Serialize};
use std::{array, ops};

/// Axial coordinates for hexagonal grid.
///
/// ```text
///                    ___
///                  /  0  \
///             ___ /       \ ___
///           / -1  \    -2 /  1  \
///      ___ /       \ ___ /       \ ___
///    / -2  \    -1 /  0  \    -2 /  2  \
///   /       \ ___ /       \ ___ /       \
///   \     0 / -1  \    -1 /  1  \    -2 /
///    \ ___ /       \ ___ /       \ ___ /
///    / -2  \     0 /  q  \    -1 /  2  \
///   /       \ ___ /       \ ___ /       \
///   \     1 / -1  \ s   r /  1  \    -1 /
///    \ ___ /       \ ___ /       \ ___ /
///    / -2  \     1 /  0  \     0 /  2  \
///   /       \ ___ /       \ ___ /       \
///   \     2 / -1  \     1 /  1  \     0 /
///    \ ___ /       \ ___ /       \ ___ /
///          \     2 /  0  \     1 /
///           \ ___ /       \ ___ /
///                 \     2 /
///                  \ ___ /
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "(i32, i32)", from = "(i32, i32)")]
pub struct AxialCoord {
    pub q: i32,
    pub r: i32,
}

impl AxialCoord {
    const NEIGHBOR_DIRECTIONS: [(i32, i32); 6] = [
        (1, -1), //
        (0, -1),
        (-1, 0),
        (-1, 1),
        (0, 1),
        (1, 0),
    ];

    pub const ORIGIN: AxialCoord = AxialCoord { q: 0, r: 0 };

    pub const fn new(q: i32, r: i32) -> Self {
        Self { q, r }
    }

    /// Get the third cube coordinate (s = -q-r)
    pub const fn s(&self) -> i32 {
        -self.q - self.r
    }

    /// Get the cube coordinates (x, y, z) where x + y + z = 0
    pub const fn to_cube(&self) -> (i32, i32, i32) {
        let x = self.q;
        let z = self.r;
        let y = -x - z;
        (x, y, z)
    }

    /// Calculate the distance between two hexes in the axial coordinate system
    pub fn distance(&self, other: &AxialCoord) -> i32 {
        let a_cube = self.to_cube();
        let b_cube = other.to_cube();
        ((a_cube.0 - b_cube.0).abs() + (a_cube.1 - b_cube.1).abs() + (a_cube.2 - b_cube.2).abs()) / 2
    }

    /// Get a navigator for stepping in flat-top hex directions
    pub fn flat(self) -> FlatAxialCoord {
        FlatAxialCoord(self)
    }

    /// Get a navigator for stepping in flat-top hex directions
    pub fn pointy(self) -> PointyAxialCoord {
        PointyAxialCoord(self)
    }

    /// Returns true if this coordinate lies on the boundary of a hex grid of given radius.
    pub fn is_boundary(&self, radius: u32) -> bool {
        self.distance(&AxialCoord::ORIGIN) == radius as i32
    }

    /// Get the coordinates of all hexes in a ring at the given radius
    /// Starting from direction 0 (HexPointyDir, HexFlatDir) and proceeding CCW
    pub fn ring(&self, radius: u32) -> RingIterator {
        RingIterator::new(*self, radius)
    }

    /// Get the coordinates of all hexes within the given radius (inclusive)
    /// Starting from radius 0 (center) and proceeding outward in rings
    pub fn spiral(&self, radius: u32) -> SpiralIterator {
        SpiralIterator::new(*self, radius)
    }
}

impl From<(i32, i32)> for AxialCoord {
    fn from((q, r): (i32, i32)) -> Self {
        Self { q, r }
    }
}

impl From<AxialCoord> for (i32, i32) {
    fn from(coord: AxialCoord) -> Self {
        (coord.q, coord.r)
    }
}

/// Navigate on a flat-top hex grid
#[derive(Clone, Copy)]
pub struct FlatAxialCoord(AxialCoord);

impl FlatAxialCoord {
    /// Get the (Δq, Δr) delta for a direction in flat-top coordinates.
    #[inline]
    pub fn delta(direction: HexFlatDir) -> (i32, i32) {
        AxialCoord::NEIGHBOR_DIRECTIONS[direction as usize]
    }

    /// Create an AxialBase with this coordinate as origin and the given directions as basis vectors.
    #[inline]
    pub fn base(self, du: HexFlatDir, dv: HexFlatDir) -> AxialBase {
        AxialBase::new(self.0, Self::delta(du), Self::delta(dv))
    }

    #[inline]
    pub fn step(mut self, direction: HexFlatDir, step: i32) -> FlatAxialCoord {
        let (dq, dr) = Self::delta(direction);
        self.0.q += dq * step;
        self.0.r += dr * step;
        self
    }

    #[inline]
    pub fn neighbor(self, direction: HexFlatDir) -> FlatAxialCoord {
        self.step(direction, 1)
    }

    #[inline]
    pub fn corner(self, direction: HexFlatDir, radius: u32) -> FlatAxialCoord {
        self.step(direction, radius as i32)
    }

    /// The 6 corenrs of a hexagonal ring at the given radius, starting from direction 0 (HexFlatDir) and proceeding CCW
    pub fn corners(self, radius: u32) -> [FlatAxialCoord; 6] {
        let mut corners = [self; 6];
        for i in HexFlatDir::all() {
            corners[i.into_index()] = self.corner(i, radius);
        }
        corners
    }

    /// World position of the cell center, when a cell has the given size (radius)
    pub fn to_position(&self, size: f32) -> Vec2 {
        let x = size * 1.5 * (self.q as f32);
        let y = -size * SQRT_3 * (self.r as f32 + self.q as f32 / 2.0);
        Vec2::new(x, y)
    }
}

impl ops::Deref for FlatAxialCoord {
    type Target = AxialCoord;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Navigate in a pointy-top hex grid
#[derive(Clone, Copy)]
pub struct PointyAxialCoord(AxialCoord);

impl PointyAxialCoord {
    /// Get the (Δq, Δr) delta for a direction in pointy-top coordinates.
    #[inline]
    pub fn delta(direction: HexPointyDir) -> (i32, i32) {
        AxialCoord::NEIGHBOR_DIRECTIONS[direction as usize]
    }

    /// Create an AxialBase with this coordinate as origin and the given directions as basis vectors.
    pub fn base(self, du: HexPointyDir, dv: HexPointyDir) -> AxialBase {
        AxialBase::new(self.0, Self::delta(du), Self::delta(dv))
    }

    #[inline]
    pub fn step(mut self, direction: HexPointyDir, step: i32) -> PointyAxialCoord {
        let (dq, dr) = Self::delta(direction);
        self.0.q += dq * step;
        self.0.r += dr * step;
        self
    }

    #[inline]
    pub fn neighbor(self, direction: HexPointyDir) -> PointyAxialCoord {
        self.step(direction, 1)
    }

    #[inline]
    pub fn neighbors(self) -> [AxialCoord; 6] {
        array::from_fn(|i| self.neighbor(HexPointyDir::from_index(i)).0)
    }

    #[inline]
    pub fn corner(self, direction: HexPointyDir, radius: u32) -> PointyAxialCoord {
        self.step(direction, radius as i32)
    }

    #[inline]
    pub fn corners(self, radius: u32) -> [AxialCoord; 6] {
        array::from_fn(|i| self.corner(HexPointyDir::from_index(i), radius).0)
    }

    /// World position of the cell center, when a cell has the given size (radius)
    pub fn to_position(&self, size: f32) -> Vec2 {
        // This is a +30° CCW rotation of the flat-top formula
        let x = size * SQRT_3 * ((self.q - self.r) as f32 / 2.0);
        let y = -size * 1.5 * ((self.q + self.r) as f32);
        Vec2::new(x, y)
    }
}

impl ops::Deref for PointyAxialCoord {
    type Target = AxialCoord;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Iterator that yields coordinates in a hexagonal ring
#[derive(Debug)]
pub struct RingIterator {
    radius: u32,
    current: AxialCoord,
    direction_idx: usize,
    steps_taken: u32,
}

impl RingIterator {
    fn new(center: AxialCoord, radius: u32) -> Self {
        let mut current = center;
        let (dq, dr) = AxialCoord::NEIGHBOR_DIRECTIONS[0];
        current.q += dq * radius as i32;
        current.r += dr * radius as i32;

        Self {
            radius,
            current,
            direction_idx: 0,
            steps_taken: 0,
        }
    }
}

impl Iterator for RingIterator {
    type Item = AxialCoord;

    fn next(&mut self) -> Option<Self::Item> {
        const DIR: [usize; 6] = [
            HexFlatDir::NW as usize,
            HexFlatDir::SW as usize,
            HexFlatDir::S as usize,
            HexFlatDir::SE as usize,
            HexFlatDir::NE as usize,
            HexFlatDir::N as usize,
        ];
        if self.direction_idx >= DIR.len() {
            return None;
        }

        if self.radius == 0 {
            self.direction_idx = DIR.len();
            return Some(self.current);
        }
        // Move to next position
        if self.steps_taken >= self.radius {
            self.direction_idx += 1;
            self.steps_taken = 0;
        }

        if let Some((dq, dr)) = DIR
            .get(self.direction_idx)
            .map(|&dir| AxialCoord::NEIGHBOR_DIRECTIONS[dir])
        {
            let result = self.current;
            self.current = AxialCoord::new(self.current.q + dq, self.current.r + dr);
            self.steps_taken += 1;
            Some(result)
        } else {
            None
        }
    }
}

pub struct SpiralIterator {
    center: AxialCoord,
    radius: u32,
    current_radius: u32,
    ring: RingIterator,
}

impl SpiralIterator {
    fn new(center: AxialCoord, radius: u32) -> Self {
        Self {
            center,
            radius,
            current_radius: 0,
            ring: center.ring(0),
        }
    }
}

impl Iterator for SpiralIterator {
    type Item = AxialCoord;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(coord) = self.ring.next() {
            Some(coord)
        } else if self.current_radius < self.radius {
            self.current_radius += 1;
            self.ring = self.center.ring(self.current_radius);
            self.ring.next()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use itertools::assert_equal;
    use shine_test::test;
    use std::iter::repeat_n;

    #[rustfmt::skip]
    const RING0: [(i32, i32); 1] = [(0, 0)];
    #[rustfmt::skip]
    const RING1: [(i32, i32); 6] = [(1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1), (1, 0)];
    #[rustfmt::skip]
    const RING2: [(i32, i32); 12] = [(2, -2), (1, -2), (0, -2), (-1, -1), (-2, 0), (-2, 1), (-2, 2), (-1, 2), (0, 2), (1, 1), (2, 0), (2, -1)];
    #[rustfmt::skip]
    const RING3: [(i32, i32); 18] = [(3, -3), (2, -3), (1, -3), (0, -3), (-1, -2), (-2, -1), (-3, 0), (-3, 1), (-3, 2), (-3, 3), (-2, 3), (-1, 3), (0, 3), (1, 2), (2, 1), (3, 0), (3, -1), (3, -2)];

    #[test]
    fn test_distance() {
        assert_eq!(AxialCoord::new(0, 0).distance(&AxialCoord::new(0, 0)), 0);

        // ring 1
        assert_eq!(AxialCoord::new(0, 0).distance(&AxialCoord::new(1, 0)), 1);
        assert_eq!(AxialCoord::new(0, 0).distance(&AxialCoord::new(1, -1)), 1);

        // ring 4
        assert_eq!(AxialCoord::new(0, 0).distance(&AxialCoord::new(-4, 4)), 4);
        assert_eq!(AxialCoord::new(0, 0).distance(&AxialCoord::new(-4, 2)), 4);
        assert_eq!(AxialCoord::new(0, 0).distance(&AxialCoord::new(2, 2)), 4);
    }

    fn test_ring(radius: u32, center: AxialCoord, centered_expected: Option<&[(i32, i32)]>) {
        let ring: Vec<_> = center.ring(radius).collect();
        let dist = radius as i32;
        let count = (radius * 6).max(1) as usize;
        assert_equal(ring.iter().map(|c| c.distance(&center)), repeat_n(dist, count));

        if let Some(expected) = centered_expected {
            assert_equal(
                ring.iter().cloned(),
                expected
                    .iter()
                    .map(|(q, r)| AxialCoord::new(center.q + *q, center.r + *r)),
            );
        }
    }

    #[test]
    fn test_ring_0() {
        test_ring(0, AxialCoord::ORIGIN, Some(&RING0));
        test_ring(0, AxialCoord::new(13, -51), Some(&RING0));
    }

    #[test]
    fn test_ring_1() {
        test_ring(1, AxialCoord::ORIGIN, Some(&RING1));
        test_ring(1, AxialCoord::new(13, -51), Some(&RING1));
    }

    #[test]
    fn test_ring_2() {
        test_ring(2, AxialCoord::ORIGIN, Some(&RING2));
        test_ring(2, AxialCoord::new(13, -51), Some(&RING2));
    }

    #[test]
    fn test_ring_3() {
        test_ring(3, AxialCoord::ORIGIN, Some(&RING3));
        test_ring(3, AxialCoord::new(13, -51), Some(&RING3));
    }

    #[test]
    fn test_ring_big() {
        // test for both even and odd radius
        test_ring(31, AxialCoord::ORIGIN, None);
        test_ring(31, AxialCoord::new(13, -51), None);
        test_ring(32, AxialCoord::ORIGIN, None);
        test_ring(32, AxialCoord::new(13, -51), None);
    }

    #[test]
    fn test_spiral_0() {
        // Test spiral of radius 1
        let spiral: Vec<_> = AxialCoord::ORIGIN.spiral(0).collect();
        assert_equal(
            spiral.iter().cloned(),
            (RING0.iter()).map(|(q, r)| AxialCoord::new(*q, *r)),
        );
    }

    #[test]
    fn test_spiral_1() {
        // Test spiral of radius 1
        let spiral: Vec<_> = AxialCoord::ORIGIN.spiral(1).collect();
        assert_equal(
            spiral.iter().cloned(),
            (RING0.iter().chain(RING1.iter())).map(|(q, r)| AxialCoord::new(*q, *r)),
        );
    }

    #[test]
    fn test_spiral_2() {
        // Test spiral of radius 2
        let spiral: Vec<_> = AxialCoord::ORIGIN.spiral(2).collect();
        assert_equal(
            spiral.iter().cloned(),
            (RING0.iter().chain(RING1.iter()).chain(RING2.iter())).map(|(q, r)| AxialCoord::new(*q, *r)),
        );
    }

    #[test]
    fn test_is_boundary_radius_0() {
        assert!(AxialCoord::new(0, 0).is_boundary(0));
    }

    #[test]
    fn test_is_boundary_radius_1() {
        assert!(!AxialCoord::new(0, 0).is_boundary(1));
        for coord in AxialCoord::ORIGIN.ring(1) {
            assert!(coord.is_boundary(1), "expected boundary: {:?}", coord);
        }
    }

    #[test]
    fn test_is_boundary_radius_2() {
        assert!(!AxialCoord::new(0, 0).is_boundary(2));
        for coord in AxialCoord::ORIGIN.ring(1) {
            assert!(!coord.is_boundary(2), "expected interior: {:?}", coord);
        }
        for coord in AxialCoord::ORIGIN.ring(2) {
            assert!(coord.is_boundary(2), "expected boundary: {:?}", coord);
        }
    }

    #[test]
    fn test_is_boundary_radius_4() {
        assert!(!AxialCoord::new(0, 0).is_boundary(4));
        assert!(!AxialCoord::new(1, 1).is_boundary(4));
        assert!(AxialCoord::new(4, 0).is_boundary(4));
        assert!(AxialCoord::new(0, 4).is_boundary(4));
        assert!(AxialCoord::new(-4, 4).is_boundary(4));
        assert!(AxialCoord::new(2, -4).is_boundary(4));
        assert!(!AxialCoord::new(3, 0).is_boundary(4));
    }

    #[test]
    fn test_spiral_3() {
        // Test spiral of radius 3
        let spiral: Vec<_> = AxialCoord::ORIGIN.spiral(3).collect();
        assert_equal(
            spiral.iter().cloned(),
            (RING0.iter().chain(RING1.iter()).chain(RING2.iter()).chain(RING3.iter()))
                .map(|(q, r)| AxialCoord::new(*q, *r)),
        );
    }

    #[test]
    fn test_flat_position() {
        let ring: Vec<_> = AxialCoord::ORIGIN.ring(1).collect();
        let positions: Vec<_> = ring.iter().map(|coord| coord.flat().to_position(1.0)).collect();
        let expected = [
            Vec2::new(1.500, 0.866),
            Vec2::new(0.000, 1.732),
            Vec2::new(-1.500, 0.866),
            Vec2::new(-1.500, -0.866),
            Vec2::new(0.000, -1.732),
            Vec2::new(1.500, -0.866),
        ];
        for (pos, exp) in positions.iter().zip(expected.iter()) {
            assert!(
                (pos.x - exp.x).abs() < 0.001 && (pos.y - exp.y).abs() < 0.001,
                "expected: {:?}, got: {:?}",
                exp,
                pos
            );
        }
    }

    #[test]
    fn test_pointy_position() {
        let ring: Vec<_> = AxialCoord::ORIGIN.ring(1).collect();
        let positions: Vec<_> = ring.iter().map(|coord| coord.pointy().to_position(1.0)).collect();
        let expected = [
            Vec2::new(1.732, 0.000),
            Vec2::new(0.866, 1.500),
            Vec2::new(-0.866, 1.500),
            Vec2::new(-1.732, 0.000),
            Vec2::new(-0.866, -1.500),
            Vec2::new(0.866, -1.500),
        ];
        for (pos, exp) in positions.iter().zip(expected.iter()) {
            assert!(
                (pos.x - exp.x).abs() < 0.001 && (pos.y - exp.y).abs() < 0.001,
                "expected: {:?}, got: {:?}",
                exp,
                pos
            );
        }
    }
}
