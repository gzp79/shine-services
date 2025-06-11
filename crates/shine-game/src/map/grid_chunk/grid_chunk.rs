use crate::map::{MapChunk, MapConfig, Tile};

pub trait GridConfig: MapConfig {
    fn width(&self) -> usize;
    fn height(&self) -> usize;
}

/// Trait for chunk types that can be used in a grid
pub trait GridChunkTypes: Send + Sync + 'static {
    type Tile: Tile;

    fn name() -> &'static str;
}

/// Chunk component for a grid of tiles
pub trait GridChunk: MapChunk {
    type Tile: Tile;

    fn width(&self) -> usize;
    fn height(&self) -> usize;

    fn try_get(&self, x: usize, y: usize) -> Option<&Self::Tile>;
    fn get(&self, x: usize, y: usize) -> &Self::Tile {
        self.try_get(x, y).expect("Out of bounds access")
    }

    fn try_get_mut(&mut self, x: usize, y: usize) -> Option<&mut Self::Tile>;
    fn get_mut(&mut self, x: usize, y: usize) -> &mut Self::Tile {
        self.try_get_mut(x, y).expect("Out of bounds access")
    }

    fn iter(&self) -> GridChunkIterator<Self>
    where
        Self: Sized,
    {
        GridChunkIterator {
            chunk: self,
            size: (self.width(), self.height()),
            index: (0, 0),
        }
    }

    fn iter_mut(&mut self) -> GridChunkIteratorMut<Self>
    where
        Self: Sized,
    {
        let size = (self.width(), self.height());
        GridChunkIteratorMut {
            chunk: self,
            size,
            index: (0, 0),
        }
    }
}

/// A sparse 2d grid of tiles
pub trait SparseGridChunk: GridChunk {
    fn occupied(&self) -> impl Iterator<Item = (usize, usize, &Self::Tile)>;
}

/// A dense 2d grid of tiles
pub trait DenseGridChunk: GridChunk {
    fn data(&self) -> &[Self::Tile];
    fn data_mut(&mut self) -> &mut [Self::Tile];
}

pub struct GridChunkIterator<'a, C>
where
    C: GridChunk,
{
    chunk: &'a C,
    size: (usize, usize),
    index: (usize, usize),
}

impl<'a, C> Iterator for GridChunkIterator<'a, C>
where
    C: GridChunk,
{
    type Item = (usize, usize, &'a C::Tile);

    fn next(&mut self) -> Option<Self::Item> {
        let (x, y) = self.index;
        let tile = self.chunk.try_get(x, y)?;

        self.index.0 += 1;
        if self.index.0 >= self.size.0 {
            self.index.0 = 0;
            self.index.1 += 1;
        }

        Some((x, y, tile))
    }
}

pub struct GridChunkIteratorMut<'a, C>
where
    C: GridChunk,
{
    chunk: &'a mut C,
    size: (usize, usize),
    index: (usize, usize),
}

impl<'a, C> Iterator for GridChunkIteratorMut<'a, C>
where
    C: GridChunk,
{
    type Item = (usize, usize, &'a mut C::Tile);

    fn next(&mut self) -> Option<Self::Item> {
        let (x, y) = self.index;
        let tile = self.chunk.try_get_mut(x, y)?;

        self.index.0 += 1;
        if self.index.0 >= self.size.0 {
            self.index.0 = 0;
            self.index.1 += 1;
        }

        // SAFETY: This is safe because we are iterating over the same mutable reference and subsequent calls cannot access the same tile.
        let tile: &'a mut C::Tile = unsafe { std::mem::transmute(tile) };
        Some((x, y, tile))
    }
}
