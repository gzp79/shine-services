use crate::map::{ChunkStore, ChunkType};
use bevy::ecs::component::Component;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;

/// Chunk component storing a sparse 2d grid of tiles.
#[derive(Component, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "C::Tile: Serialize + DeserializeOwned")]
pub struct SparseChunk<C>
where
    C: ChunkType,
    C::Tile: Clone,
{
    version: usize,
    width: usize,
    height: usize,
    default: C::Tile,
    data: HashMap<(usize, usize), C::Tile>,
}

impl<C> ChunkStore for SparseChunk<C>
where
    C: ChunkType,
    C::Tile: Clone,
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
            default: <Self::Tile as Default>::default(),
            data: HashMap::new(),
        }
    }

    fn new(width: usize, height: usize) -> Self {
        Self {
            version: 0,
            width,
            height,
            default: <Self::Tile as Default>::default(),
            data: HashMap::new(),
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
            self.data.get(&(x, y)).or(Some(&self.default))
        } else {
            None
        }
    }

    fn try_get_mut(&mut self, x: usize, y: usize) -> Option<&mut Self::Tile> {
        if x < self.width && y < self.height {
            let tile = self.data.entry((x, y)).or_insert_with(|| self.default.clone());
            Some(tile)
        } else {
            None
        }
    }
}
