use crate::map2::{ChunkStore, Tile};
use std::collections::HashMap;

pub struct SparseChunkStore<T>
where
    T: Tile,
{
    width: usize,
    height: usize,
    default: T,
    data: HashMap<(usize, usize), T>,
}

impl<T> ChunkStore for SparseChunkStore<T>
where
    T: Tile,
{
    type Tile = T;

    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: HashMap::new(),
            default: T::default(),
        }
    }

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn try_get(&self, x: usize, y: usize) -> Option<&Self::Tile> {
        if x < self.width && y < self.height {
            self.data.get(&(x, y)).or(Some(&self.default))
        } else {
            None
        }
    }

    fn try_get_mut(&mut self, x: usize, y: usize) -> Option<&mut Self::Tile> {
        if x < self.width && y < self.height {
            let tile = self.data.entry((x, y)).or_insert_with(|| T::default());
            Some(tile)
        } else {
            None
        }
    }
}
