use crate::map::{AxialCoord, MapLayer};

/// 2D hexagonal grid layer.
pub trait HexLayer: MapLayer {
    fn radius(&self) -> u32;

    fn is_in_bounds(&self, coord: AxialCoord) -> bool {
        coord.distance(&AxialCoord::new(0, 0)) <= self.radius() as i32
    }

    fn try_get(&self, coord: AxialCoord) -> Option<&Self::Tile>;
    fn get(&self, coord: AxialCoord) -> &Self::Tile;
}
