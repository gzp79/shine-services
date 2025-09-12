use crate::map::{AxialCoord, HexChunk, HexConfig, MapChunk, SparseHexChunk, Tile};
use bevy::ecs::component::Component;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Chunk component storing a sparse 2d grid of tiles.
#[derive(Component, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "T: Tile")]
pub struct SparseHex<T>
where
    T: Tile,
{
    radius: u32,
    default: T,
    data: HashMap<AxialCoord, T>,
}

impl<T> SparseHex<T>
where
    T: Tile,
{
    pub fn new(config: &HexConfig<T>) -> Self {
        Self {
            radius: config.radius,
            default: <T as Default>::default(),
            data: HashMap::new(),
        }
    }
}

impl<T> From<HexConfig<T>> for SparseHex<T>
where
    T: Tile,
{
    fn from(config: HexConfig<T>) -> Self {
        Self::new(&config)
    }
}

impl<T> MapChunk for SparseHex<T>
where
    T: Tile,
{
    type Tile = T;

    fn new_empty() -> Self
    where
        Self: Sized,
    {
        Self {
            radius: 0,
            default: <T as Default>::default(),
            data: HashMap::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.radius == 0
    }
}

impl<T> HexChunk for SparseHex<T>
where
    T: Tile,
{
    fn radius(&self) -> u32 {
        self.radius
    }

    /// Try to get a reference to a tile at the given coordinates.
    /// Returns the tile if it's occupied, otherwise returns the default value.
    /// If the tile is out of bounds, None is returned.
    fn try_get(&self, coord: AxialCoord) -> Option<&Self::Tile> {
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
    fn get(&self, coord: AxialCoord) -> &Self::Tile {
        if self.is_in_bounds(coord) {
            self.data.get(&coord).unwrap_or(&self.default)
        } else {
            panic!("Out of bounds access");
        }
    }

    /// Get a mutable reference to a tile at the given coordinates.
    /// Returns None if the tile is out of bounds or not occupied (sparse hex).
    fn try_get_mut(&mut self, coord: AxialCoord) -> Option<&mut Self::Tile> {
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
    fn get_mut(&mut self, coord: AxialCoord) -> &mut Self::Tile {
        if self.is_in_bounds(coord) {
            self.data.entry(coord).or_insert_with(|| self.default.clone())
        } else {
            panic!("Out of bounds access");
        }
    }
}

impl<T> SparseHexChunk for SparseHex<T>
where
    T: Tile,
{
    fn occupied(&self) -> impl Iterator<Item = (AxialCoord, &Self::Tile)> {
        self.data.iter().map(|(coord, tile)| (*coord, tile))
    }
}
