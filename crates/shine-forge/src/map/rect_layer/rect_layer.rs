use crate::map::{MapLayer, RectCoord, RectLayerConfig};

/// 2D rectangular grid layer.
pub trait RectLayer: MapLayer + From<RectLayerConfig<Self::Tile>> {
    fn width(&self) -> u32;
    fn height(&self) -> u32;

    fn is_in_bounds(&self, coord: RectCoord) -> bool {
        coord.x >= 0 && (coord.x as u32) < self.width() && coord.y >= 0 && (coord.y as u32) < self.height()
    }

    fn try_get(&self, coord: RectCoord) -> Option<&Self::Tile>;
    fn get(&self, coord: RectCoord) -> &Self::Tile;
}
