use crate::map::{hex_chunk::HexDenseIndexer, AxialCoord, HexChunk, HexConfig, HexDenseChunk, MapChunk, Tile};
use bevy::ecs::component::Component;
use serde::{Deserialize, Serialize};

/// Chunk component storing a dense hexagonal grid of tiles.
#[derive(Component, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "T: Tile")]
pub struct HexDense<T>
where
    T: Tile,
{
    row_starts: HexDenseIndexer,
    data: Vec<T>,
}

impl<T> HexDense<T>
where
    T: Tile,
{
    pub fn new(config: &HexConfig<T>) -> Self {
        let radius = config.radius;
        let row_starts = HexDenseIndexer::new(radius);
        let total_size = row_starts.get_total_size();

        let mut data = Vec::with_capacity(total_size);
        data.resize_with(total_size, <T as Default>::default);

        Self { row_starts, data }
    }
}

impl<T> From<HexConfig<T>> for HexDense<T>
where
    T: Tile,
{
    fn from(config: HexConfig<T>) -> Self {
        Self::new(&config)
    }
}

impl<T> MapChunk for HexDense<T>
where
    T: Tile,
{
    type Tile = T;

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

impl<T> HexChunk for HexDense<T>
where
    T: Tile,
{
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

    fn get(&self, coord: AxialCoord) -> &Self::Tile {
        self.try_get(coord).expect("Out of bounds access")
    }

    fn try_get_mut(&mut self, coord: AxialCoord) -> Option<&mut Self::Tile> {
        if self.is_in_bounds(coord) {
            let index = self.row_starts.get_dense_index(&coord);
            Some(&mut self.data[index])
        } else {
            None
        }
    }

    fn get_mut(&mut self, coord: AxialCoord) -> &mut Self::Tile {
        self.try_get_mut(coord).expect("Out of bounds access")
    }
}

impl<T> HexDenseChunk for HexDense<T>
where
    T: Tile,
{
    fn data(&self) -> &[Self::Tile] {
        &self.data
    }

    fn data_mut(&mut self) -> &mut [Self::Tile] {
        &mut self.data
    }
}
