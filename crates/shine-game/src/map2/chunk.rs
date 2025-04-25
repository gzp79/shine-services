use crate::map2::Tile;
use bevy::ecs::component::Component;

#[derive(Component)]
pub struct Chunk<T>
where
    T: Tile,
{
    width: usize,
    height: usize,
    data: Vec<T>,
}

impl<T: Tile> Chunk<T> {
    pub fn new(width: usize, height: usize) -> Self
    where
        T: Default + Clone,
    {
        let data = vec![T::default(); width * height];
        Self { width, height, data }
    }

    pub fn try_get(&self, x: usize, y: usize) -> Option<&T> {
        if x < self.width && y < self.height {
            Some(&self.data[y * self.width + x])
        } else {
            None
        }
    }

    pub fn get(&self, x: usize, y: usize) -> &T {
        self.try_get(x, y)
            .unwrap_or_else(|| panic!("Out of bounds access at ({}, {})", x, y))
    }

    pub fn try_get_mut(&mut self, x: usize, y: usize) -> Option<&mut T> {
        if x < self.width && y < self.height {
            Some(&mut self.data[y * self.width + x])
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, x: usize, y: usize) -> &mut T {
        self.try_get_mut(x, y)
            .unwrap_or_else(|| panic!("Out of bounds access at ({}, {})", x, y))
    }

    pub fn iter(&self) -> ChunkIterator<T> {
        ChunkIterator { chunk: self, index: 0 }
    }

    pub fn iter_mut(&mut self) -> ChunkIteratorMut<T> {
        ChunkIteratorMut { chunk: self, index: 0 }
    }
}

pub struct ChunkIterator<'a, T>
where
    T: Tile,
{
    chunk: &'a Chunk<T>,
    index: usize,
}

impl<'a, T> Iterator for ChunkIterator<'a, T>
where
    T: Tile,
{
    type Item = (usize, usize, &'a T);

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

pub struct ChunkIteratorMut<'a, T>
where
    T: Tile,
{
    chunk: &'a mut Chunk<T>,
    index: usize,
}

impl<'a, T> Iterator for ChunkIteratorMut<'a, T>
where
    T: Tile,
{
    type Item = (usize, usize, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.chunk.data.len() {
            return None;
        }

        let x = self.index % self.chunk.width;
        let y = self.index / self.chunk.width;

        let tile = &mut self.chunk.data[self.index];
        // SAFETY: The iterator ensures that only one mutable reference is active at a time.
        let tile: &'a mut T = unsafe { std::mem::transmute(tile) };

        self.index += 1;

        Some((x, y, tile))
    }
}
