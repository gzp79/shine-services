use crate::hex::AxialCoord;
use crate::map::{DenseHexChunk, HexChunk, HexChunkTypes, HexConfig, HexDenseIndexer, MapChunk, Tile};
use bevy::ecs::component::Component;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// Chunk component storing a dense hexagonal grid of tiles.
#[derive(Component, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "T::Tile: Serialize + DeserializeOwned")]
pub struct DenseHex<T>
where
    T: HexChunkTypes,
    T::Tile: Tile + Clone,
{
    row_starts: HexDenseIndexer,
    data: Vec<T::Tile>,
}

impl<T> DenseHex<T>
where
    T: HexChunkTypes,
    T::Tile: Clone,
{
    pub fn new<CFG>(config: CFG) -> Self
    where
        CFG: HexConfig,
    {
        let radius = config.radius();
        let row_starts = HexDenseIndexer::new(radius);
        let total_size = row_starts.get_total_size();

        let mut data = Vec::with_capacity(total_size);
        data.resize_with(total_size, <T::Tile as Default>::default);

        Self { row_starts, data }
    }
}

impl<CFG, T> From<CFG> for DenseHex<T>
where
    CFG: HexConfig,
    T: HexChunkTypes,
    T::Tile: Clone,
{
    fn from(config: CFG) -> Self {
        Self::new(config)
    }
}

impl<T> MapChunk for DenseHex<T>
where
    T: HexChunkTypes,
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
            row_starts: HexDenseIndexer::new(0),
            data: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.row_starts.radius() == 0
    }
}

impl<T> HexChunk for DenseHex<T>
where
    T: HexChunkTypes,
    T::Tile: Clone,
{
    type Tile = T::Tile;

    fn radius(&self) -> u32 {
        self.row_starts.radius()
    }

    fn try_get(&self, coord: AxialCoord) -> Option<&Self::Tile> {
        if self.is_in_bounds(coord) {
            let index = self.row_starts.get_dense_index(&coord);
            Some(&self.data[index])
        } else {
            None
        }
    }

    fn try_get_mut(&mut self, coord: AxialCoord) -> Option<&mut Self::Tile> {
        if self.is_in_bounds(coord) {
            let index = self.row_starts.get_dense_index(&coord);
            Some(&mut self.data[index])
        } else {
            None
        }
    }
}

impl<T> DenseHexChunk for DenseHex<T>
where
    T: HexChunkTypes,
    T::Tile: Clone,
{
    fn data(&self) -> &[Self::Tile] {
        &self.data
    }

    fn data_mut(&mut self) -> &mut [Self::Tile] {
        &mut self.data
    }
}
