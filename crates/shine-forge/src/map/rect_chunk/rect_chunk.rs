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

/// Rectangular chunk with sparse storage
pub trait RectSparseChunk: RectChunk {
    /// The value of the onoccupied entries.
    fn default(&self) -> &Self::Tile;

    /// Iterator over the occupied entires
    fn occupied(&self) -> impl Iterator<Item = (RectCoord, &Self::Tile)>;
}

/// Rectangular chunk with dense storage
pub trait RectDenseChunk: RectChunk {
    fn data(&self) -> &[Self::Tile];
    fn data_mut(&mut self) -> &mut [Self::Tile];
}
