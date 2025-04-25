use crate::map2::ChunkSizes;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RegionId(pub usize, pub usize);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Part {
    Inner,
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChunkId(pub usize, pub usize);
impl ChunkId {
    pub fn part(&self) -> Part {
        match (self.0 % 3, self.1 % 3) {
            (1, 1) => Part::Inner,
            (0, 1) => Part::Left,
            (2, 1) => Part::Right,
            (1, 0) => Part::Top,
            (1, 2) => Part::Bottom,
            (0, 0) => Part::TopLeft,
            (2, 0) => Part::TopRight,
            (0, 2) => Part::BottomLeft,
            (2, 2) => Part::BottomRight,
            _ => unreachable!(),
        }
    }

    pub fn get_size(&self, config: &ChunkSizes) -> (usize, usize) {
        match self.part() {
            Part::Inner => (config.inner_width, config.inner_height),
            Part::Left => (config.side_width, config.inner_height),
            Part::Right => (config.side_width, config.inner_height),
            Part::Top => (config.inner_width, config.side_height),
            Part::Bottom => (config.inner_width, config.side_height),
            Part::TopLeft => (config.side_width, config.side_height),
            Part::TopRight => (config.side_width, config.side_width),
            Part::BottomLeft => (config.side_width, config.side_width),
            Part::BottomRight => (config.side_width, config.side_width),
        }
    }
}

impl From<(RegionId, Part)> for ChunkId {
    fn from((region, part): (RegionId, Part)) -> Self {
        match part {
            Part::Inner => Self(3 * region.0 + 1, 3 * region.1 + 1),
            Part::Left => Self(3 * region.0, 3 * region.1 + 1),
            Part::Right => Self(3 * region.0 + 2, 3 * region.1 + 1),
            Part::Top => Self(3 * region.0 + 1, 3 * region.1),
            Part::Bottom => Self(3 * region.0 + 1, 3 * region.1 + 2),
            Part::TopLeft => Self(3 * region.0, 3 * region.1),
            Part::TopRight => Self(3 * region.0 + 2, 3 * region.1),
            Part::BottomLeft => Self(3 * region.0, 3 * region.1 + 2),
            Part::BottomRight => Self(3 * region.0 + 2, 3 * region.1 + 2),
        }
    }
}
