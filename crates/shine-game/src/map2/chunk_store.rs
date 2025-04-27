use serde::{de::DeserializeOwned, Serialize};

use crate::map2::Tile;

pub trait ChunkOperation: 'static + Serialize + DeserializeOwned + Send + Sync {
    type Tile: Tile;

    fn apply<C>(self, chunk: &mut C)
    where
        C: ChunkStore<Tile = Self::Tile>;
}

/// A 2d grid of tile data with a specific scope and size.
pub trait ChunkStore: 'static + Send + Sync {
    type Tile: Tile;

    fn new(width: usize, height: usize) -> Self
    where
        Self: Sized;

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

    fn iter(&self) -> ChunkStoreIterator<Self>
    where
        Self: Sized,
    {
        ChunkStoreIterator {
            chunk: self,
            size: (self.width(), self.height()),
            index: (0, 0),
        }
    }

    fn iter_mut(&mut self) -> ChunkStoreIteratorMut<Self>
    where
        Self: Sized,
    {
        let size = (self.width(), self.height());
        ChunkStoreIteratorMut {
            chunk: self,
            size,
            index: (0, 0),
        }
    }
}

/// A dense 2d grid of tile data with a specific scope and size.
pub trait DenseChunk: ChunkStore {
    fn data(&self) -> &[Self::Tile];
    fn data_mut(&mut self) -> &mut [Self::Tile];
}

pub struct ChunkStoreIterator<'a, C>
where
    C: ChunkStore,
{
    chunk: &'a C,
    size: (usize, usize),
    index: (usize, usize),
}

impl<'a, C> Iterator for ChunkStoreIterator<'a, C>
where
    C: ChunkStore,
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

pub struct ChunkStoreIteratorMut<'a, C>
where
    C: ChunkStore,
{
    chunk: &'a mut C,
    size: (usize, usize),
    index: (usize, usize),
}

impl<'a, C> Iterator for ChunkStoreIteratorMut<'a, C>
where
    C: ChunkStore,
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
