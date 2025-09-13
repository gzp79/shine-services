use crate::map::{MapChunk, RectChunk, RectConfig, RectCoord, RectDenseChunk, Tile};
use bevy::ecs::component::Component;
use serde::{Deserialize, Serialize};

/// Chunk component storing a dense 2d rectangular grid of tiles.
#[derive(Component, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "T: Tile")]
pub struct RectDense<T>
where
    T: Tile,
{
    width: u32,
    height: u32,
    data: Vec<T>,
}

impl<T> RectDense<T>
where
    T: Tile,
{
    pub fn new(config: &RectConfig<T>) -> Self {
        let width = config.width;
        let height = config.height;

        let area = (width * height) as usize;
        let mut data = Vec::with_capacity(area);
        data.resize_with(area, <T as Default>::default);
        Self { width, height, data }
    }
}

impl<T> From<RectConfig<T>> for RectDense<T>
where
    T: Tile,
{
    fn from(config: RectConfig<T>) -> Self {
        Self::new(&config)
    }
}

impl<T> MapChunk for RectDense<T>
where
    T: Tile,
{
    type Tile = T;

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

impl<T> RectChunk for RectDense<T>
where
    T: Tile,
{
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn try_get(&self, coord: RectCoord) -> Option<&Self::Tile> {
        if self.is_in_bounds(coord) {
            let index = (coord.y * (self.width as i32) + coord.x) as usize;
            Some(&self.data[index])
        } else {
            None
        }
    }

    fn get(&self, coord: RectCoord) -> &Self::Tile {
        self.try_get(coord).expect("Out of bounds access")
    }

    fn try_get_mut(&mut self, coord: RectCoord) -> Option<&mut Self::Tile> {
        if self.is_in_bounds(coord) {
            let index = (coord.y * (self.width as i32) + coord.x) as usize;
            Some(&mut self.data[index])
        } else {
            None
        }
    }

    fn get_mut(&mut self, coord: RectCoord) -> &mut Self::Tile {
        self.try_get_mut(coord).expect("Out of bounds access")
    }
}

impl<T> RectDenseChunk for RectDense<T>
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
