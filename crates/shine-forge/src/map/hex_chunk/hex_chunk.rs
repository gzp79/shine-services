use crate::map::{AxialCoord, MapChunk};

/// Chunk component for a hexagonal grid of tiles
pub trait HexChunk: MapChunk {
    fn radius(&self) -> u32;

    fn try_get(&self, coord: AxialCoord) -> Option<&Self::Tile>;
    fn get(&self, coord: AxialCoord) -> &Self::Tile;

    fn try_get_mut(&mut self, coord: AxialCoord) -> Option<&mut Self::Tile>;
    fn get_mut(&mut self, coord: AxialCoord) -> &mut Self::Tile;

    fn is_in_bounds(&self, coord: AxialCoord) -> bool {
        coord.distance(&AxialCoord::new(0, 0)) <= self.radius() as i32
    }
}

/// Hexagonal chunk with sparse storage
pub trait HexSparseChunk: HexChunk {
    /// The value of the onoccupied entries.
    fn default(&self) -> &Self::Tile;

    /// Iterator over the occupied entires
    fn occupied(&self) -> impl Iterator<Item = (AxialCoord, &Self::Tile)>;
}

/// Hexagonal chunk with dense storage
pub trait HexDenseChunk: HexChunk {
    fn data(&self) -> &[Self::Tile];
    fn data_mut(&mut self) -> &mut [Self::Tile];
}
