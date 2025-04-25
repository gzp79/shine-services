use crate::map2::TileMapConfig;
use bevy::ecs::component::Component;

#[derive(Component)]
pub struct Chunk<C>
where
    C: TileMapConfig,
{
    width: usize,
    height: usize,
    version: usize,
    data: Vec<C::Tile>,
}

impl<C> Chunk<C>
where
    C: TileMapConfig,
{
    pub fn new(width: usize, height: usize) -> Self
    where
        C::Tile: Default + Clone,
    {
        let data = vec![C::Tile::default(); width * height];
        Self {
            width,
            height,
            version: 0,
            data,
        }
    }

    pub fn version(&self) -> usize {
        self.version
    }

    pub fn set_version(&mut self, version: usize) {
        self.version = version;
    }

    pub fn try_get(&self, x: usize, y: usize) -> Option<&C::Tile> {
        if x < self.width && y < self.height {
            Some(&self.data[y * self.width + x])
        } else {
            None
        }
    }

    pub fn get(&self, x: usize, y: usize) -> &C::Tile {
        self.try_get(x, y)
            .unwrap_or_else(|| panic!("Out of bounds access at ({}, {})", x, y))
    }

    pub fn try_get_mut(&mut self, x: usize, y: usize) -> Option<&mut C::Tile> {
        if x < self.width && y < self.height {
            Some(&mut self.data[y * self.width + x])
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, x: usize, y: usize) -> &mut C::Tile {
        self.try_get_mut(x, y)
            .unwrap_or_else(|| panic!("Out of bounds access at ({}, {})", x, y))
    }

    pub fn iter(&self) -> ChunkIterator<C> {
        ChunkIterator { chunk: self, index: 0 }
    }

    pub fn iter_mut(&mut self) -> ChunkIteratorMut<C> {
        ChunkIteratorMut { chunk: self, index: 0 }
    }
}

pub struct ChunkIterator<'a, C>
where
    C: TileMapConfig,
{
    chunk: &'a Chunk<C>,
    index: usize,
}

impl<'a, C> Iterator for ChunkIterator<'a, C>
where
    C: TileMapConfig,
{
    type Item = (usize, usize, &'a C::Tile);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.chunk.data.len() {
            return None;
        }

        let x = self.index % self.chunk.width;
        let y = self.index / self.chunk.width;
        let tile = &self.chunk.data[self.index];
        self.index += 1;

        Some((x, y, tile))
    }
}

pub struct ChunkIteratorMut<'a, C>
where
    C: TileMapConfig,
{
    chunk: &'a mut Chunk<C>,
    index: usize,
}

impl<'a, C> Iterator for ChunkIteratorMut<'a, C>
where
    C: TileMapConfig,
{
    type Item = (usize, usize, &'a mut C::Tile);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.chunk.data.len() {
            return None;
        }

        let x = self.index % self.chunk.width;
        let y = self.index / self.chunk.width;

        let tile = &mut self.chunk.data[self.index];
        // SAFETY: The iterator ensures that only one mutable reference is active at a time.
        let tile: &'a mut C::Tile = unsafe { std::mem::transmute(tile) };

        self.index += 1;

        Some((x, y, tile))
    }
}
