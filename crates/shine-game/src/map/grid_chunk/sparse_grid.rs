use crate::map::{GridChunk, GridChunkTypes, GridConfig, MapChunk, SparseGridChunk, Tile};
use bevy::ecs::component::Component;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;

/// Chunk component storing a sparse 2d grid of tiles.
#[derive(Component, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "T::Tile: Serialize + DeserializeOwned")]
pub struct SparseGrid<T>
where
    T: GridChunkTypes,
    T::Tile: Tile + Clone,
{
    width: usize,
    height: usize,
    default: T::Tile,
    data: HashMap<(usize, usize), T::Tile>,
}

impl<T> SparseGrid<T>
where
    T: GridChunkTypes,
    T::Tile: Tile + Clone,
{
    fn new<CFG>(config: CFG) -> Self
    where
        CFG: GridConfig,
    {
        Self {
            width: config.width(),
            height: config.height(),
            default: <T::Tile as Default>::default(),
            data: HashMap::new(),
        }
    }
}

impl<CFG, T> From<CFG> for SparseGrid<T>
where
    CFG: GridConfig,
    T: GridChunkTypes,
    T::Tile: Clone,
{
    fn from(config: CFG) -> Self {
        Self::new(config)
    }
}

impl<T> MapChunk for SparseGrid<T>
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
            default: <T::Tile as Default>::default(),
            data: HashMap::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.width == 0 && self.height == 0
    }
}

impl<T> GridChunk for SparseGrid<T>
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
            self.data.get(&(x, y)).or(Some(&self.default))
        } else {
            None
        }
    }

    fn try_get_mut(&mut self, x: usize, y: usize) -> Option<&mut Self::Tile> {
        if x < self.width && y < self.height {
            let tile = self
                .data
                .entry((x, y))
                .or_insert_with(|| self.default.clone());
            Some(tile)
        } else {
            None
        }
    }
}

impl<T> SparseGridChunk for SparseGrid<T>
where
    T: GridChunkTypes,
    T::Tile: Clone,
{
    fn occupied(&self) -> impl Iterator<Item = (usize, usize, &Self::Tile)> {
        self.data.iter().map(|(&(x, y), tile)| (x, y, tile))
    }
}
