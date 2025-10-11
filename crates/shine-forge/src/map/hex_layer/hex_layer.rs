use crate::map::{AxialCoord, MapLayer, Tile};

/// 2D hexagonal grid layer.
pub trait HexLayer: MapLayer {
    fn radius(&self) -> u32;

    fn is_in_bounds(&self, coord: AxialCoord) -> bool {
        coord.distance(&AxialCoord::new(0, 0)) <= self.radius() as i32
    }
}

/// 2D hexagonal grid layer of the given Tiles.
pub trait HexTileLayer: HexLayer {
    type Tile: Tile;

    fn try_get(&self, coord: AxialCoord) -> Option<&Self::Tile>;

    fn get(&self, coord: AxialCoord) -> &Self::Tile {
        self.try_get(coord).expect("Out of bounds access")
    }
}
