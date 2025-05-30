use crate::map::{DenseGridChunk, GridChunk, GridChunkTypes, GridConfig, MapChunk, Tile};
use bevy::ecs::component::Component;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// Chunk component storing a dense 2d grid of tiles.
#[derive(Component, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "T::Tile: Serialize + DeserializeOwned")]
pub struct DenseGrid<T>
where
    T: GridChunkTypes,
    T::Tile: Tile + Clone,
{
    width: usize,
    height: usize,
    data: Vec<T::Tile>,
}

impl<T> DenseGrid<T>
where
    T: GridChunkTypes,
    T::Tile: Tile + Clone,
{
    pub fn new<CFG>(config: CFG) -> Self
    where
        CFG: GridConfig,
    {
        let width = config.width();
        let height = config.height();

        let area = width * height;
        let mut data = Vec::with_capacity(area);
        data.resize_with(area, <T::Tile as Default>::default);
        Self {
            width,
            height,
            data,
        }
    }
}

impl<CFG, T> From<CFG> for DenseGrid<T>
where
    CFG: GridConfig,
    T: GridChunkTypes,
    T::Tile: Clone,
{
    fn from(config: CFG) -> Self {
        Self::new(config)
    }
}

impl<T> MapChunk for DenseGrid<T>
where
    T: GridChunkTypes,
    T::Tile: Clone,
{
    fn name() -> &'static str {
        T::name()
    }

    fn new_empty() -> Self
    where
        Self: Sized,
    {
        Self {
            width: 0,
            height: 0,
            data: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.width == 0 && self.height == 0
    }
}

impl<T> GridChunk for DenseGrid<T>
where
    T: GridChunkTypes,
    T::Tile: Clone,
{
    type Tile = T::Tile;

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

impl<T> DenseGridChunk for DenseGrid<T>
where
    T: GridChunkTypes,
    T::Tile: Clone,
{
    fn data(&self) -> &[Self::Tile] {
        &self.data
    }

    fn data_mut(&mut self) -> &mut [Self::Tile] {
        &mut self.data
    }
}
