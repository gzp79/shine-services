use crate::map::proto;

/// Coordinates for a rectangular grid.
/// Origin is the bottom left corner, with x increasing to the right and y increasing upwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RectCoord {
    pub x: i32,
    pub y: i32,
}

impl RectCoord {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub const fn origin() -> Self {
        Self { x: 0, y: 0 }
    }

    /// Calculate the distance between two hexes in the axial coordinate system
    pub fn distance(&self, other: &RectCoord) -> i32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx.abs() + dy.abs()) / 2
    }
}

impl From<(i32, i32)> for RectCoord {
    fn from((x, y): (i32, i32)) -> Self {
        Self { x, y }
    }
}

impl From<proto::RectCoord> for RectCoord {
    fn from(proto: proto::RectCoord) -> Self {
        Self { x: proto.x, y: proto.y }
    }
}
