use crate::map::Tile;
use bevy::ecs::component::{Component, Mutable};
use serde::{de::DeserializeOwned, Serialize};

/// Unique identifier for a chunk in the world.
#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MapChunkId(pub usize, pub usize);

/// Trait to define a chunk in the map.
pub trait MapChunk: Component<Mutability = Mutable> + Serialize + DeserializeOwned {
    type Tile: Tile;

    /// Create an empty chunk component indicating that the chunk is not loaded yet.
    fn new_empty() -> Self
    where
        Self: Sized;

    /// Return if the chunk is empty (on-loaded) or not.
    fn is_empty(&self) -> bool;
}
