use glam::Vec2;
use serde::{Deserialize, Serialize};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HexVertex {
    NNW = 0, // Top left
    NNE = 1, // Top right
    E = 2,   // Right
    SSE = 3, // Bottom right
    SSW = 4, // Bottom left
    W = 5,   // Left
}

impl From<u8> for HexVertex {
    fn from(value: u8) -> Self {
        match value {
            0 => HexVertex::NNW,
            1 => HexVertex::NNE,
            2 => HexVertex::E,
            3 => HexVertex::SSE,
            4 => HexVertex::SSW,
            5 => HexVertex::W,
            _ => panic!("Invalid vertex index: {}", value),
        }
    }
}

impl From<HexVertex> for u8 {
    fn from(vertex: HexVertex) -> Self {
        vertex as u8
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HexNeighbor {
    N = 0,  // Top
    NE = 1, // Top-Right
    SE = 2, // Bottom-Right
    S = 3,  // Bottom
    SW = 4, // Bottom-Left
    NW = 5, // Top-Left
}

impl From<u8> for HexNeighbor {
    fn from(value: u8) -> Self {
        match value {
            0 => HexNeighbor::N,
            1 => HexNeighbor::NE,
            2 => HexNeighbor::SE,
            3 => HexNeighbor::S,
            4 => HexNeighbor::SW,
            5 => HexNeighbor::NW,
            _ => panic!("Invalid neighbor index: {}", value),
        }
    }
}

impl From<HexNeighbor> for u8 {
    fn from(neighbor: HexNeighbor) -> Self {
        neighbor as u8
    }
}

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
    pub const fn new(q: i32, r: i32) -> Self {
        Self { q, r }
    }

    pub const fn origin() -> Self {
        Self { q: 0, r: 0 }
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

    /// Get the coordinates of all hexes in a ring at the given radius
    pub fn ring(&self, radius: u32) -> RingIterator {
        RingIterator::new(*self, radius)
    }

    /// Get the coordinates of all hexes within the given radius (inclusive)
    pub fn spiral(&self, radius: u32) -> SpiralIterator {
        SpiralIterator::new(*self, radius)
    }

    /// Get neighbor chunk in direction 0..5
    /// 0: North, 1: NorthEast, 2: SouthEast, 3: South, 4: SouthWest, 5: NorthWest
    pub fn neighbor(&self, direction: HexNeighbor) -> Self {
        const DIRECTIONS: [(i32, i32); 6] = [
            (0, -1), // North
            (1, -1), // NorthEast
            (1, 0),  // SouthEast
            (0, 1),  // South
            (-1, 1), // SouthWest
            (-1, 0), // NorthWest
        ];
        let (dq, dr) = DIRECTIONS[direction as usize];
        Self::new(self.q + dq, self.r + dr)
    }

    /// Get the coordinates of the hex neighbors
    pub fn neighbors(&self) -> impl Iterator<Item = AxialCoord> + 'static {
        let coord = *self;
        (0u8..6).map(move |dir| coord.neighbor(HexNeighbor::from(dir)))
    }

    /// Returns the 6 corner coordinates of a flat-top hexagon at the given radius.
    /// v0=(R,0), v1=(0,R), v2=(-R,R), v3=(-R,0), v4=(0,-R), v5=(R,-R)
    pub fn hex_corners(radius: u32) -> [AxialCoord; 6] {
        let r = radius as i32;
        [
            AxialCoord::new(r, 0),
            AxialCoord::new(0, r),
            AxialCoord::new(-r, r),
            AxialCoord::new(-r, 0),
            AxialCoord::new(0, -r),
            AxialCoord::new(r, -r),
        ]
    }

    /// Returns true if this coordinate lies on the boundary of a hex grid of given radius.
    /// Operates on the integer coordinate address, not the jittered vertex position.
    pub fn is_boundary(&self, radius: u32) -> bool {
        self.distance(&AxialCoord::origin()) == radius as i32
    }

    /// Derive per-cell hex_size from a world-space circumradius and axial grid radius.
    /// circumradius = sqrt(3) * hex_size * radius → hex_size = circumradius / (sqrt(3) * radius).
    pub fn hex_size_from_world_size(world_size: f32, radius: u32) -> f32 {
        const SQRT_3: f32 = 1.732050807568877293527446341505872367_f32;
        world_size / (SQRT_3 * radius as f32)
    }

    /// World position for placing vertices in a flat-top hex grid.
    /// Produces a flat-top overall hex shape.
    pub fn vertex_position(&self, hex_size: f32) -> Vec2 {
        const SQRT_3: f32 = 1.732050807568877293527446341505872367_f32;
        let x = hex_size * SQRT_3 * (self.q as f32 + self.r as f32 / 2.0);
        let y = hex_size * 1.5 * (self.r as f32);
        Vec2::new(x, y)
    }

    /// World position for placing hex centers (chunk tiling).
    /// Correct spacing to tile flat-top hexes without overlap.
    pub fn center_position(&self, hex_size: f32) -> Vec2 {
        const SQRT_3: f32 = 1.732050807568877293527446341505872367_f32;
        let x = hex_size * 1.5 * (self.q as f32);
        let y = hex_size * SQRT_3 * (self.r as f32 + self.q as f32 / 2.0);
        Vec2::new(x, y)
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
        // Start at the north neighbor
        current.r -= radius as i32;

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
        if self.direction_idx >= DIRECTIONS.len() {
            return None;
        }

        if self.radius == 0 {
            self.direction_idx = DIRECTIONS.len();
            return Some(self.current);
        }

        const DIRECTIONS: [(i32, i32); 6] = [
            (1, 0),  // SouthEast
            (0, 1),  // South
            (-1, 1), // SouthWest
            (-1, 0), // NorthWest
            (0, -1), // North
            (1, -1), // NorthEast
        ];

        // Move to next position
        if self.steps_taken >= self.radius {
            self.direction_idx += 1;
            self.steps_taken = 0;
        }

        if let Some((dq, dr)) = DIRECTIONS.get(self.direction_idx) {
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
    const RING1: [(i32, i32); 6] = [(0, -1), (1, -1), (1, 0), (0, 1), (-1, 1), (-1, 0)];
    #[rustfmt::skip]
    const RING2: [(i32, i32); 12] = [(0, -2), (1, -2), (2, -2), (2, -1), (2, 0), (1, 1), (0, 2), (-1, 2), (-2, 2), (-2, 1), (-2, 0), (-1, -1)];
    #[rustfmt::skip]
    const RING3: [(i32, i32); 18] = [(0, -3), (1, -3), (2, -3), (3, -3), (3, -2), (3, -1), (3, 0), (2, 1), (1, 2), (0, 3), (-1, 3), (-2, 3), (-3, 3), (-3, 2), (-3, 1), (-3, 0), (-2, -1), (-1, -2)];

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
        test_ring(0, AxialCoord::origin(), Some(&RING0));
        test_ring(0, AxialCoord::new(13, -51), Some(&RING0));
    }

    #[test]
    fn test_ring_1() {
        test_ring(1, AxialCoord::origin(), Some(&RING1));
        test_ring(1, AxialCoord::new(13, -51), Some(&RING1));
    }

    #[test]
    fn test_ring_2() {
        test_ring(2, AxialCoord::origin(), Some(&RING2));
        test_ring(2, AxialCoord::new(13, -51), Some(&RING2));
    }

    #[test]
    fn test_ring_3() {
        test_ring(3, AxialCoord::origin(), Some(&RING3));
        test_ring(3, AxialCoord::new(13, -51), Some(&RING3));
    }

    #[test]
    fn test_ring_big() {
        // test for both even and odd radius
        test_ring(31, AxialCoord::origin(), None);
        test_ring(31, AxialCoord::new(13, -51), None);
        test_ring(32, AxialCoord::origin(), None);
        test_ring(32, AxialCoord::new(13, -51), None);
    }

    #[test]
    fn test_spiral_0() {
        // Test spiral of radius 1
        let spiral: Vec<_> = AxialCoord::origin().spiral(0).collect();
        assert_equal(
            spiral.iter().cloned(),
            (RING0.iter()).map(|(q, r)| AxialCoord::new(*q, *r)),
        );
    }

    #[test]
    fn test_spiral_1() {
        // Test spiral of radius 1
        let spiral: Vec<_> = AxialCoord::origin().spiral(1).collect();
        assert_equal(
            spiral.iter().cloned(),
            (RING0.iter().chain(RING1.iter())).map(|(q, r)| AxialCoord::new(*q, *r)),
        );
    }

    #[test]
    fn test_spiral_2() {
        // Test spiral of radius 2
        let spiral: Vec<_> = AxialCoord::origin().spiral(2).collect();
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
        for coord in AxialCoord::origin().ring(1) {
            assert!(coord.is_boundary(1), "expected boundary: {:?}", coord);
        }
    }

    #[test]
    fn test_is_boundary_radius_2() {
        assert!(!AxialCoord::new(0, 0).is_boundary(2));
        for coord in AxialCoord::origin().ring(1) {
            assert!(!coord.is_boundary(2), "expected interior: {:?}", coord);
        }
        for coord in AxialCoord::origin().ring(2) {
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
        let spiral: Vec<_> = AxialCoord::origin().spiral(3).collect();
        assert_equal(
            spiral.iter().cloned(),
            (RING0.iter().chain(RING1.iter()).chain(RING2.iter()).chain(RING3.iter()))
                .map(|(q, r)| AxialCoord::new(*q, *r)),
        );
    }
}
