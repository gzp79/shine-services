use crate::map2::{ChunkStore, DenseChunk, Tile};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DenseChunkStore<T>
where
    T: Tile,
{
    width: usize,
    height: usize,
    data: Vec<T>,
}

impl<T> ChunkStore for DenseChunkStore<T>
where
    T: Tile,
{
    type Tile = T;

    fn new(width: usize, height: usize) -> Self {
        let area = width * height;
        let mut data = Vec::with_capacity(area);
        data.resize_with(area, T::default);
        Self { width, height, data }
    }

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn try_get(&self, x: usize, y: usize) -> Option<&Self::Tile> {
        if x < self.width && y < self.height {
            Some(&self.data[y * self.width + x])
        } else {
            None
        }
    }

    fn try_get_mut(&mut self, x: usize, y: usize) -> Option<&mut Self::Tile> {
        if x < self.width && y < self.height {
            Some(&mut self.data[y * self.width + x])
        } else {
            None
        }
    }
}

impl<T> DenseChunk for DenseChunkStore<T>
where
    T: Tile,
{
    fn data(&self) -> &[T] {
        &self.data
    }

    fn data_mut(&mut self) -> &mut [T] {
        &mut self.data
    }
}
