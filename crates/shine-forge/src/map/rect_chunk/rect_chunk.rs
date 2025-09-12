use crate::map::{MapChunk, RectCoord};

/// Trait defining common operations for a 2D rectangular grid chunk of the map.
pub trait RectChunk: MapChunk {
    fn width(&self) -> u32;
    fn height(&self) -> u32;

    fn try_get(&self, coord: RectCoord) -> Option<&Self::Tile>;
    fn get(&self, coord: RectCoord) -> &Self::Tile;

    fn try_get_mut(&mut self, coord: RectCoord) -> Option<&mut Self::Tile>;
    fn get_mut(&mut self, coord: RectCoord) -> &mut Self::Tile;

    fn is_in_bounds(&self, coord: RectCoord) -> bool {
        coord.x >= 0 && (coord.x as u32) < self.width() && coord.y >= 0 && (coord.y as u32) < self.height()
    }
}

/// Trait for rectangular chunks that efficiently store only non-default (occupied) tiles.
/// Useful for representing large, mostly empty grids without allocating memory for every cell.
pub trait SparseRectChunk: RectChunk {
    fn occupied(&self) -> impl Iterator<Item = (RectCoord, &Self::Tile)>;
}

/// Trait for rectangular chunks with dense (contiguous) storage of tiles.
pub trait DenseRectChunk: RectChunk {
    fn data(&self) -> &[Self::Tile];
    fn data_mut(&mut self) -> &mut [Self::Tile];
}
