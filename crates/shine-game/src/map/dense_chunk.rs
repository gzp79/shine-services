use crate::map::{ChunkStore, ChunkType, DenseChunkStore};
use bevy::ecs::component::Component;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// Chunk component storing a dense 2d grid of tiles.
#[derive(Component, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "C::Tile: Serialize + DeserializeOwned")]
pub struct DenseChunk<C>
where
    C: ChunkType,
{
    version: usize,
    width: usize,
    height: usize,
    data: Vec<C::Tile>,
}
impl<C> ChunkStore for DenseChunk<C>
where
    C: ChunkType,
{
    const NAME: &'static str = C::NAME;
    type Tile = C::Tile;
    type Operation = C::Operation;

    fn new_empty() -> Self
    where
        Self: Sized,
    {
        Self {
            version: 0,
            width: 0,
            height: 0,
            data: Vec::new(),
        }
    }

    fn new(width: usize, height: usize) -> Self {
        let area = width * height;
        let mut data = Vec::with_capacity(area);
        data.resize_with(area, <Self::Tile as Default>::default);
        Self {
            version: 0,
            width,
            height,
            data,
        }
    }

    fn is_empty(&self) -> bool {
        self.width == 0 && self.height == 0
    }

    fn version(&self) -> usize {
        self.version
    }

    fn version_mut(&mut self) -> &mut usize {
        &mut self.version
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

impl<C> DenseChunkStore for DenseChunk<C>
where
    C: ChunkType,
{
    fn data(&self) -> &[Self::Tile] {
        &self.data
    }

    fn data_mut(&mut self) -> &mut [Self::Tile] {
        &mut self.data
    }
}
