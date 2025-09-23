use crate::map::{MapLayer, RectCoord, Tile};

/// 2D rectangular grid layer.
pub trait RectLayer: MapLayer {
    fn width(&self) -> u32;
    fn height(&self) -> u32;

    fn is_in_bounds(&self, coord: RectCoord) -> bool {
        coord.x >= 0 && (coord.x as u32) < self.width() && coord.y >= 0 && (coord.y as u32) < self.height()
    }
}

/// 2D rectangular grid layer of the given Tiles.
pub trait RectTileLayer: RectLayer {
    type Tile: Tile;

    fn try_get(&self, coord: RectCoord) -> Option<&Self::Tile>;

    fn get(&self, coord: RectCoord) -> &Self::Tile {
        self.try_get(coord).expect("Out of bounds access")
    }
}
