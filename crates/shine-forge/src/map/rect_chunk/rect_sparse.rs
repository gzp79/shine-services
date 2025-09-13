use crate::map::{MapChunk, RectChunk, RectConfig, RectCoord, RectSparseChunk, Tile};
use bevy::ecs::component::Component;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Chunk component storing a sparse 2d rectangular grid of tiles.
#[derive(Component, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "T: Tile")]
pub struct RectSparse<T>
where
    T: Tile,
{
    width: u32,
    height: u32,
    default: T,
    data: HashMap<RectCoord, T>,
}

impl<T> RectSparse<T>
where
    T: Tile,
{
    pub fn new(config: &RectConfig<T>) -> Self {
        Self {
            width: config.width,
            height: config.height,
            default: <T as Default>::default(),
            data: HashMap::new(),
        }
    }
}

impl<T> From<RectConfig<T>> for RectSparse<T>
where
    T: Tile,
{
    fn from(config: RectConfig<T>) -> Self {
        Self::new(&config)
    }
}

impl<T> MapChunk for RectSparse<T>
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
            default: <T as Default>::default(),
            data: HashMap::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.width == 0 && self.height == 0
    }
}

impl<T> RectChunk for RectSparse<T>
where
    T: Tile,
{
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    /// Try to get a reference to a tile at the given coordinates.
    /// Returns the tile if it's occupied, otherwise returns the default value.
    /// If the tile is out of bounds, None is returned.
    fn try_get(&self, coord: RectCoord) -> Option<&Self::Tile> {
        if self.is_in_bounds(coord) {
            self.data.get(&coord).or(Some(&self.default))
        } else {
            None
        }
    }

    /// Get a reference to a tile at the given coordinates.
    /// Returns the tile if occupied, otherwise returns the default value.
    /// # Panics
    /// Panics if the coordinates are out of bounds.
    fn get(&self, coord: RectCoord) -> &Self::Tile {
        if self.is_in_bounds(coord) {
            self.data.get(&coord).unwrap_or(&self.default)
        } else {
            panic!("Out of bounds access");
        }
    }

    /// Get a mutable reference to a tile at the given coordinates.
    /// Returns None if the tile is out of bounds or not occupied (sparse grid).
    fn try_get_mut(&mut self, coord: RectCoord) -> Option<&mut Self::Tile> {
        if self.is_in_bounds(coord) {
            self.data.get_mut(&coord)
        } else {
            None
        }
    }

    /// Get a mutable reference to a tile at the given coordinates.
    /// If the tile is not occupied, it will be inserted with the default value.
    /// # Panics
    /// Panics if the coordinates are out of bounds.
    fn get_mut(&mut self, coord: RectCoord) -> &mut Self::Tile {
        if self.is_in_bounds(coord) {
            self.data.entry(coord).or_insert_with(|| self.default.clone())
        } else {
            panic!("Out of bounds access");
        }
    }
}

impl<T> RectSparseChunk for RectSparse<T>
where
    T: Tile,
{
    fn default(&self) -> &Self::Tile {
        &self.default
    }

    fn occupied(&self) -> impl Iterator<Item = (RectCoord, &Self::Tile)> {
        self.data.iter().map(|(coord, tile)| (*coord, tile))
    }
}
