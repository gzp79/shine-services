use crate::{
    hex::{AxialCoord, SpiralIterator},
    map::{MapChunk, MapConfig, Tile},
};

pub trait HexConfig: MapConfig {
    /// Get the radius of the hexagonal grid chunks.
    fn radius(&self) -> u32;
}

/// Trait for chunk types that can be used in a hexagonal grid
pub trait HexChunkTypes: Send + Sync + 'static {
    type Tile: Tile;

    fn name() -> &'static str;
}

/// Chunk component for a hexagonal grid of tiles
pub trait HexChunk: MapChunk {
    type Tile: Tile;

    /// Get the radius of the hexagonal chunk (number of rings from center)
    fn radius(&self) -> u32;

    /// Try to get a tile at the given axial coordinates
    fn try_get(&self, coord: &AxialCoord) -> Option<&Self::Tile>;

    /// Get a tile at the given axial coordinates, panics if out of bounds
    fn get(&self, coord: &AxialCoord) -> &Self::Tile {
        self.try_get(coord).expect("Out of bounds access")
    }

    /// Try to get a mutable reference to a tile at the given axial coordinates
    fn try_get_mut(&mut self, coord: &AxialCoord) -> Option<&mut Self::Tile>;

    /// Get a mutable reference to a tile at the given axial coordinates, panics if out of bounds
    fn get_mut(&mut self, coord: &AxialCoord) -> &mut Self::Tile {
        self.try_get_mut(coord).expect("Out of bounds access")
    }

    /// Check if the given axial coordinates are within the chunk's bounds
    fn is_in_bounds(&self, coord: &AxialCoord) -> bool {
        AxialCoord::origin().distance(coord) <= self.radius() as i32
    }

    /// Iterator over all valid coordinates and their tiles in the chunk
    fn iter(&self) -> HexChunkIterator<Self>
    where
        Self: Sized,
    {
        HexChunkIterator::new(self)
    }

    /// Mutable iterator over all valid coordinates and their tiles in the chunk
    fn iter_mut(&mut self) -> HexChunkIteratorMut<Self>
    where
        Self: Sized,
    {
        HexChunkIteratorMut::new(self)
    }
}

/// A dense hexagonal grid of tiles
pub trait DenseHexChunk: HexChunk {
    fn data(&self) -> &[Self::Tile];
    fn data_mut(&mut self) -> &mut [Self::Tile];
}

pub struct HexChunkIterator<'a, C>
where
    C: HexChunk,
{
    chunk: &'a C,
    spiral: SpiralIterator,
}

impl<'a, C> HexChunkIterator<'a, C>
where
    C: HexChunk,
{
    fn new(chunk: &'a C) -> Self {
        Self {
            chunk,
            spiral: AxialCoord::origin().spiral(chunk.radius()),
        }
    }
}

impl<'a, C> Iterator for HexChunkIterator<'a, C>
where
    C: HexChunk,
{
    type Item = (AxialCoord, &'a C::Tile);

    fn next(&mut self) -> Option<Self::Item> {
        self.spiral.next().map(|coord| (coord, self.chunk.get(&coord)))
    }
}

pub struct HexChunkIteratorMut<'a, C>
where
    C: HexChunk,
{
    chunk: &'a mut C,
    spiral: SpiralIterator,
}

impl<'a, C> HexChunkIteratorMut<'a, C>
where
    C: HexChunk,
{
    fn new(chunk: &'a mut C) -> Self {
        let radius = chunk.radius();
        Self {
            chunk,
            spiral: AxialCoord::origin().spiral(radius),
        }
    }
}

impl<'a, C> Iterator for HexChunkIteratorMut<'a, C>
where
    C: HexChunk,
{
    type Item = (AxialCoord, &'a mut C::Tile);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(coord) = self.spiral.next() {
            let tile = self.chunk.get_mut(&coord);
            // SAFETY: This is safe because we are iterating over the same mutable reference and subsequent calls cannot access the same tile.
            let tile: &'a mut C::Tile = unsafe { std::mem::transmute(tile) };
            Some((coord, tile))
        } else {
            None
        }
    }
}
